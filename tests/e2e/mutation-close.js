// Closing right after a Run that edited the model (a new track) cleans up.

// %% add a track, then the runner closes
{
    const job = uapmd.sequencer.addTrack();
    e2e.assert(job.state === 'completed' && !!job.result.trackId,
        `addTrack did not add a track: ${JSON.stringify(job)}`);
    job;
}
