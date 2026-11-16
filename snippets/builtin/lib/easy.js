// The easy layer: functions for global names; play and playLoop always start at
// track 0. The caller passes what a track without an instrument gets and the tempo.
// Loading only defines functions.
(() => {
    const plugins = include('builtin/lib/plugins.js');
    const tracks = include('builtin/lib/tracks.js');

    // Stops playback and loops, and removes the clips of every track not in `keep`.
    function silence(keep) {
        include('builtin/lib/tutorial.js').stop();
        repl.timeline.clearLoop();
        // Discard script references to the previous clips.
        delete globalThis.loopParts;
        uapmd.sequencer.getTrackInfos().forEach((track, index) => {
            if (keep.includes(index)) return;
            for (const clip of uapmd.timeline.getClips(track.trackId)) {
                if (!uapmd.timeline.removeClip(clip.clipId))
                    throw new Error(repl.text('snippet_14', {clipId:clip.clipId}));
            }
        });
    }

    // options: {format, plugin, patch, bpm}; a track without an instrument gets
    // the plugin with the patch.
    function create(options) {
        // The i-th input goes to track i.
        function parts(inputs) {
            if (inputs.length === 0) throw new Error(repl.text('easy_00'));
            return inputs.map((input, track) => {
                tracks.info(track);
                return {track, input, instrument:plugins.ensureInstrument(track, options)};
            });
        }

        // Plays once; only the given tracks sound.
        function play(...inputs) {
            include('builtin/lib/evolution-mode.js').end();
            const demo = include('builtin/lib/tutorial.js');
            const list = parts(inputs);
            silence(list.map(p => p.track));
            for (const p of list) {
                demo.replaceMml(p.track, cmrt_chord.resolve_chord_or_mml(p.input).mml, options.bpm);
                uapmd.sequencer.setPluginBypassed(p.instrument, false);
            }
            // startPlayback owns position zero and preroll (see tutorial.js play).
            uapmd.audio.setEngineEnabled(true);
            uapmd.sequencer.startPlayback();
            return list.map(p => p.track);
        }

        // Loops; only the given tracks sound.
        function playLoop(...inputs) {
            include('builtin/lib/evolution-mode.js').end();
            const loop = include('builtin/lib/loop.js');
            const list = parts(inputs);
            const next = {};
            for (const p of list) next['track' + p.track] = loop.prepareLoopPart(p.track, p.input);
            loop.playLoopParts(next, options.bpm);
            return list.map(p => p.track);
        }

        // The start of the easy layer: silent, no clips, no plugins.
        // Tracks stay, so track numbers keep their meaning.
        function reset() {
            include('builtin/lib/evolution-mode.js').end();
            silence([]);
            for (const track of uapmd.sequencer.getTrackInfos()) {
                for (const node of track.nodes.filter(n => n.instanceId >= 0))
                    uapmd.instance(node.instanceId).dispose();
            }
            // The next created instrument goes on the scope (plugins.js ensureInstrument).
            repl.scope.setTarget(-1);
            // A disposed instance leaves its track only when the host's queued tasks run;
            // a play in the same cell must already find the tracks empty.
            repl.processEvents();
        }

        function changePatch(track) {
            tracks.info(track);
            include('builtin/lib/patch.js').change({track, cell:false});
        }

        function changeEffect(track) {
            tracks.info(track);
            include('builtin/lib/effect.js').change({track, cell:false});
        }

        return {play, playLoop, reset, changePatch, changeEffect};
    }

    return {create};
})();
