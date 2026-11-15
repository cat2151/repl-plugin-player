# Loading a project with frozen tracks can fail after plugin teardown

Revision: `4ec5c3b1c30130fdd3b7d4b375082273247e561f`.
Status: 未報告; draft only.

Create MIDI content, enable track freeze, then load a valid project while rendering
or after the cache is ready. AppModel::loadProject destroys outgoing plugins before
TimelineProjectSerializer::canReplaceDocument checks hasBusyTrack. Removal
invalidates the frozen track and queues rendering, so loading fails with
`Unfreeze the busy track before loading a project` after plugin removal began.

The local repair validates the project and pending history first, disables outgoing
freeze, drains cancellation and plugin restoration, then tears down instances.
Checking engine offline rendering as well as hasBusyTrack handles an active render
whose runtime state became Live during queued invalidation. Incoming policies and
caches are restored normally. Invalid sources preserve outgoing freeze.

Regression coverage loads during rendering and after completion, checks failed
sources preserve freeze, and verifies saved cache restoration in a fresh process.
