mod live;
mod peaks;
mod pitch;

use super::projection::{Audio, Clip, View};
use eframe::egui::{self, Color32, Rect};
use peaks::Waveform;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
};

type Job = (PathBuf, egui::Context);
type ResultEntry = (PathBuf, Option<Waveform>);

#[derive(Default)]
pub(super) struct Cache {
    entries: HashMap<PathBuf, Option<Waveform>>,
    live: HashMap<PathBuf, live::Preview>,
    worker: Option<(mpsc::Sender<Job>, mpsc::Receiver<ResultEntry>)>,
}

impl Cache {
    pub(super) fn for_view(ui: &egui::Ui, view: &View) -> Arc<Mutex<Self>> {
        let id = ui.make_persistent_id("timeline_waveforms");
        let cache = ui.data_mut(|data| {
            data.get_temp::<Arc<Mutex<Self>>>(id).unwrap_or_else(|| {
                let cache = Arc::new(Mutex::new(Self::default()));
                data.insert_temp(id, cache.clone());
                cache
            })
        });
        {
            let paths: HashSet<_> = view
                .tracks
                .iter()
                .flat_map(|track| &track.clips)
                .filter_map(|clip| clip.audio.as_ref().map(|audio| audio.path.as_path()))
                .collect();
            let mut cache = cache.lock().unwrap();
            cache
                .entries
                .retain(|path, _| paths.contains(path.as_path()));
            cache.live.retain(|path, _| paths.contains(path.as_path()));
            for audio in view
                .tracks
                .iter()
                .flat_map(|track| &track.clips)
                .filter_map(|clip| clip.audio.as_ref())
            {
                if audio.live_frames.is_some() && !cache.live.contains_key(&audio.path) {
                    if let Some(preview) =
                        live::Preview::start(audio.path.clone(), ui.ctx().clone())
                    {
                        cache.live.insert(audio.path.clone(), preview);
                    }
                }
                if let Some(preview) = cache.live.get(&audio.path) {
                    preview.update(audio.live_frames);
                }
            }
            if let Some((_, results)) = &cache.worker {
                let finished: Vec<_> = results.try_iter().collect();
                for (path, waveform) in finished {
                    if let Some(entry) = cache.entries.get_mut(&path) {
                        *entry = waveform;
                    }
                }
            }
        }
        cache
    }

    fn get(&mut self, path: &Path, ctx: &egui::Context) -> Option<&Waveform> {
        if path.as_os_str().is_empty() {
            return None;
        }
        if !self.entries.contains_key(path) {
            if self.worker.is_none() {
                self.worker = spawn_worker();
            }
            if let Some((jobs, _)) = &self.worker {
                jobs.send((path.to_owned(), ctx.clone())).ok()?;
                self.entries.insert(path.to_owned(), None);
            }
        }
        self.entries.get(path)?.as_ref()
    }
}

fn spawn_worker() -> Option<(mpsc::Sender<Job>, mpsc::Receiver<ResultEntry>)> {
    let (jobs, requests) = mpsc::channel::<Job>();
    let (ready, results) = mpsc::channel();
    std::thread::Builder::new()
        .name("timeline-waveforms".into())
        .spawn(move || {
            let mut detector = None;
            for (path, ctx) in requests {
                let detector = detector.get_or_insert_with(pitch::Detector::load);
                let waveform = Waveform::read(&path, detector.as_ref()).ok();
                if ready.send((path, waveform)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        })
        .ok()?;
    Some((jobs, results))
}

pub(super) fn draw(
    painter: &egui::Painter,
    window: Rect,
    tint: Color32,
    clip: &Clip,
    audio: &Audio,
    cache: &mut Cache,
) {
    let tint = if audio.live_frames.is_some() {
        super::RECORDING_COLOR
    } else {
        tint
    };
    let mut block = window.shrink2(egui::vec2(0.0, 3.0));
    block.max.x = block.max.x.max(block.min.x + 2.0);
    if !block.intersects(painter.clip_rect()) {
        return;
    }
    painter.rect_filled(block, 3.0, tint.gamma_multiply(0.25));
    if let Some(preview) = cache.live.get(&audio.path) {
        if let Some(waveform) = preview.waveform.lock().unwrap().as_ref() {
            draw_preview(painter, block, tint, clip, audio, waveform);
        }
    } else if audio.live_frames.is_none() {
        if let Some(waveform) = cache.get(&audio.path, painter.ctx()) {
            draw_preview(painter, block, tint, clip, audio, waveform);
        }
    }
}

fn draw_preview(
    painter: &egui::Painter,
    block: Rect,
    tint: Color32,
    clip: &Clip,
    audio: &Audio,
    waveform: &Waveform,
) {
    let inner = block.shrink2(egui::vec2(0.0, 2.0));
    let visible = inner.intersect(painter.clip_rect());
    if !visible.is_positive() {
        return;
    }
    let step = 1.0 / painter.ctx().pixels_per_point();
    let source = |x: f32| {
        let fraction = f64::from((x - block.left()) / block.width());
        audio.source_seconds(clip.start + fraction * (clip.end - clip.start))
    };
    // Scale the preview to the recording so quiet microphone takes remain legible.
    let scale = inner.height() * 0.5 / waveform.amplitude().max(0.01);
    let center = inner.center().y;
    let mut mesh = egui::Mesh::default();
    let mut left = visible.left();
    let mut start = source(left);
    let columns = (visible.width() / step).ceil() as usize;
    for column in 1..=columns {
        let right = (visible.left() + column as f32 * step).min(visible.right());
        let end = source(right);
        if let Some((min, max)) = waveform.extrema(start, end) {
            let top = (center - max * scale).clamp(inner.top(), inner.bottom() - step);
            let bottom = (center - min * scale).max(top + step).min(inner.bottom());
            mesh.add_colored_rect(
                Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom)),
                tint,
            );
        }
        left = right;
        start = end;
    }
    if !mesh.is_empty() {
        painter.add(egui::Shape::mesh(mesh));
    }
    if let Some(pitch) = waveform.pitch() {
        pitch.draw(painter, inner, visible, step, source, clip.muted);
    }
}
