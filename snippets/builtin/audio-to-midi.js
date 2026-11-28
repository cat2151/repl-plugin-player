// @search audio MIDI2 transcription pitch recording wav オーディオ 録音 音程 ピッチ 採譜
{
    if (__replCall('transcription.start', {})) {
        const task = 'audio-to-midi';
        const instrument = {format:'CLAP', plugin:'Surge XT', patch:'patches_factory/Templates/Init Saw.fxp'};
        const patch = include('builtin/lib/patch.js');
        const created = [];
        let sources, jobId;

        function finish() {
            repl.tasks.cancel(task);
            __replCall('transcription.clear', {});
        }

        repl.tasks.every(task, 25, () => {
            try {
                if (!sources) {
                    sources = __replCall('transcription.poll', {});
                    if (!sources) return;
                }
                if (!sources.length) {
                    finish();
                    return;
                }
                jobId ??= uapmd.sequencer.addTrack().jobId;
                const job = uapmd.mutations.getJob(jobId);
                if (job.state === 'pending') return;
                uapmd.mutations.clearJob(jobId);
                jobId = undefined;
                if (job.state !== 'completed') throw new Error(job.error || 'Could not add an instrument track');
                const {trackId} = job.result;
                created.push(trackId);
                const instrumentTrack = uapmd.sequencer.getTrackInfos().findIndex(track => track.trackId === trackId);
                patch.use({...instrument, track:instrumentTrack});
                __replCall('transcription.write', {sourceTrackId:sources.shift(), trackId});
                if (!sources.length) finish();
            } catch (error) {
                finish();
                for (const trackId of created) uapmd.sequencer.removeTrack(trackId);
                throw error;
            }
        });
    }
}
