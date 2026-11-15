// Display only the measurements that apply to the current freeze state.
(() => {
    function describe(trackId) {
        const state = uapmd.sequencer.getTrackFreezeState(trackId);
        let status;
        const details = [];
        if (state.runtimeState === 'error') {
            status = 'Render error / 生成エラー';
            if (state.error) details.push(state.error);
        } else if (state.runtimeState === 'frozen') {
            status = 'Ready / 生成済み・freeze音声を使用';
            if (state.frozenAudio) {
                const audio = state.frozenAudio;
                const seconds = (audio.frames / uapmd.sequencer.getSampleRate()).toFixed(2);
                details.push(`${seconds} s / ${audio.frames} frames / ${audio.channels} ch`);
            }
        } else if (state.runtimeState === 'rendering') {
            status = 'Rendering frozen audio / freeze音声を生成中';
            if (state.totalFrames > 0) {
                details.push(`${Math.round(state.renderProgress * 100)}% / `
                    + `${state.renderedSeconds.toFixed(2)} / ${state.totalSeconds.toFixed(2)} s`);
            }
        } else if (state.policy === 'off') {
            status = 'Live (freeze disabled) / 通常のplugin処理・freeze無効';
        } else {
            status = uapmd.timeline.getState().isPlaying
                ? 'Waiting for playback to stop / 再生停止待ち・音声は未生成'
                : 'Waiting to render / 生成待ち（余韻処理・他trackの生成完了待ちを含む）';
        }
        return [`${trackId}: auto freeze ${state.policy} / ${status}`,
            ...details].join('\n');
    }
    return {describe};
})();
