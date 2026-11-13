// uapmd names a track by its trackId; cells and lib name it by its track number,
// the track's position in uapmd's track list.
(() => {
    function info(index) {
        const track = Number.isInteger(index) ? uapmd.sequencer.getTrackInfos()[index] : undefined;
        if (!track) throw new Error(repl.text('patch_browse_00', {track:index}));
        return track;
    }

    function idOf(index) {
        return info(index).trackId;
    }

    return {idOf, info};
})();
