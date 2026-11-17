// %% @text name_set_buffer_multiplier
{
    const baseFrames = 1024;
    const multiplier = 4;
    const frames = baseFrames * multiplier;
    if (!Number.isFinite(multiplier) || !Number.isInteger(frames) || frames < 1 || frames > 8192)
        throw new Error('baseFrames × multiplier: 1..8192 integer frames');
    uapmd.sequencer.reconfigureAudioDevice(-1, -1, 0, frames);
    const sampleRate = uapmd.sequencer.getSampleRate();
    const ms = frames / sampleRate * 1000;
    `1x = ${baseFrames} frames; ${multiplier}x; ${baseFrames} × ${multiplier} = ${frames} frames; ${sampleRate} Hz; ${ms.toFixed(2)} ms; 80% = ${(ms * 0.8).toFixed(2)} ms`;
}
