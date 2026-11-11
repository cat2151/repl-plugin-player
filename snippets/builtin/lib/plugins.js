(() => {
    function trackFor(instanceId, errorKey = 'snippet_15') {
        var track = uapmd.sequencer.getTrackInfos()
            .find(t => t.nodes.some(n => n.instanceId === instanceId));
        if (!track) throw new Error(repl.text(errorKey));
        return track;
    }
    return {trackFor};
})();
