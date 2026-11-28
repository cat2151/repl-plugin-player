use crate::{Progress, DISCARD, FINISH, RECORD};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, SampleFormat, SizedSample,
};
use std::{
    fs::File,
    io::BufWriter,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

const CHUNK_SAMPLES: usize = 1024;
const QUEUE_CHUNKS: usize = 128;
const MAX_SAMPLES: u32 = i32::MAX as u32 / 4;
const PREVIEW_INTERVAL: Duration = Duration::from_millis(20);
pub(crate) const NO_SAMPLES: u32 = u32::MAX;

struct Chunk {
    samples: [f32; CHUNK_SAMPLES],
    length: usize,
}

fn input<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    queue: Option<mpsc::SyncSender<Chunk>>,
    failed: Arc<AtomicBool>,
    level: Arc<AtomicU32>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let overflow = failed.clone();
    let error_level = level.clone();
    device
        .build_input_stream(
            config,
            move |samples: &[T], _| {
                if samples.is_empty() {
                    return;
                }
                let mut peak = 0.0_f32;
                let Some(queue) = &queue else {
                    for sample in samples {
                        peak = sample_peak(peak, sample.to_sample::<f32>());
                    }
                    level.store(peak.to_bits(), Ordering::Relaxed);
                    return;
                };
                for segment in samples.chunks(CHUNK_SAMPLES) {
                    let mut chunk = Chunk {
                        samples: [0.0; CHUNK_SAMPLES],
                        length: segment.len(),
                    };
                    for (output, input) in chunk.samples.iter_mut().zip(segment) {
                        *output = input.to_sample::<f32>();
                        peak = sample_peak(peak, *output);
                    }
                    if queue.try_send(chunk).is_err() {
                        overflow.store(true, Ordering::Release);
                        break;
                    }
                }
                level.store(peak.to_bits(), Ordering::Relaxed);
            },
            move |_| {
                failed.store(true, Ordering::Release);
                error_level.store(NO_SAMPLES, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|error| error.to_string())
}

fn sample_peak(peak: f32, sample: f32) -> f32 {
    if sample.is_finite() {
        peak.max(sample.abs())
    } else {
        peak
    }
}

fn default_input() -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("No default microphone")?;
    let supported = device
        .default_input_config()
        .map_err(|error| error.to_string())?;
    Ok((device, supported))
}

fn input_stream(
    device: &cpal::Device,
    supported: &cpal::SupportedStreamConfig,
    queue: Option<mpsc::SyncSender<Chunk>>,
    failed: Arc<AtomicBool>,
    level: Arc<AtomicU32>,
) -> Result<cpal::Stream, String> {
    let config = supported.config();
    macro_rules! stream {
        ($sample:ty) => {
            input::<$sample>(device, &config, queue, failed, level)
        };
    }
    match supported.sample_format() {
        SampleFormat::I8 => stream!(i8),
        SampleFormat::I16 => stream!(i16),
        SampleFormat::I24 => stream!(cpal::I24),
        SampleFormat::I32 => stream!(i32),
        SampleFormat::U8 => stream!(u8),
        SampleFormat::U16 => stream!(u16),
        SampleFormat::U32 => stream!(u32),
        SampleFormat::F32 => stream!(f32),
        SampleFormat::F64 => stream!(f64),
        format => Err(format!("Unsupported microphone sample format {format:?}")),
    }
}

pub(crate) fn monitor(level: Arc<AtomicU32>) -> Result<cpal::Stream, String> {
    let (device, supported) = default_input()?;
    let failed = Arc::new(AtomicBool::new(false));
    let stream = input_stream(&device, &supported, None, failed, level)?;
    stream.play().map_err(|error| error.to_string())?;
    Ok(stream)
}

pub(crate) fn record(
    file: File,
    command: &AtomicU8,
    level: Arc<AtomicU32>,
    progress: &Progress,
) -> Result<bool, String> {
    if command.load(Ordering::Acquire) == DISCARD {
        return Ok(false);
    }
    let (device, supported) = default_input()?;
    let config = supported.config();
    let spec = hound::WavSpec {
        channels: config.channels,
        sample_rate: config.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer =
        hound::WavWriter::new(BufWriter::new(file), spec).map_err(|error| error.to_string())?;
    progress.rate.store(spec.sample_rate, Ordering::Relaxed);
    let (send, receive) = mpsc::sync_channel(QUEUE_CHUNKS);
    let failed = Arc::new(AtomicBool::new(false));
    let stream = input_stream(&device, &supported, Some(send), failed.clone(), level)?;
    match command.load(Ordering::Acquire) {
        DISCARD => return Ok(false),
        FINISH => return Err("No microphone samples captured".into()),
        _ => {}
    }
    stream.play().map_err(|error| error.to_string())?;
    let mut last_flush = None;
    let mut published = 0;
    while command.load(Ordering::Acquire) == RECORD && !failed.load(Ordering::Acquire) {
        match receive.recv_timeout(PREVIEW_INTERVAL) {
            Ok(chunk) => write(&mut writer, chunk)?,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Microphone stream disconnected".into())
            }
        }
        let frames = writer.duration();
        if frames > published
            && writer.len().is_multiple_of(u32::from(spec.channels))
            && last_flush.is_none_or(|time: Instant| time.elapsed() >= PREVIEW_INTERVAL)
        {
            writer.flush().map_err(|error| error.to_string())?;
            progress.frames.store(frames, Ordering::Release);
            last_flush = Some(Instant::now());
            published = frames;
        }
    }
    drop(stream);
    if command.load(Ordering::Acquire) == DISCARD {
        return Ok(false);
    }
    if failed.load(Ordering::Acquire) {
        return Err("Microphone stream failed or the recording buffer overflowed".into());
    }
    for chunk in receive.try_iter() {
        write(&mut writer, chunk)?;
    }
    let frames = writer.duration();
    if frames == 0 {
        return Err("No microphone samples captured".into());
    }
    writer.finalize().map_err(|error| error.to_string())?;
    progress.frames.store(frames, Ordering::Release);
    Ok(true)
}

fn write(writer: &mut hound::WavWriter<BufWriter<File>>, chunk: Chunk) -> Result<(), String> {
    if writer.len() + chunk.length as u32 > MAX_SAMPLES {
        return Err("Recording reached the WAV size limit".into());
    }
    for &sample in &chunk.samples[..chunk.length] {
        writer
            .write_sample(sample)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
