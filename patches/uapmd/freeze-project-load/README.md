# Project replacement with frozen tracks

Validated upstream: `9f708d3300b4a6f8109eee0fadccf286db8b7a45`.

AppModel validates the incoming project, then destroys outgoing plugins before
TimelineProjectSerializer checks whether freeze is busy. Plugin removal itself
invalidates frozen tracks and starts new renders. Loading therefore fails with
`Unfreeze the busy track before loading a project`, potentially after deleting
outgoing plugin instances. Waiting before calling the loader cannot prevent it.

After successful parsing and the pending-history guard, this repair turns off
outgoing freeze policies and drains active render cancellation and plugin state
restoration before teardown. Both busy state and engine offline rendering are
checked because queued invalidations can temporarily mark an active track Live.
Incoming freeze settings and cached audio are restored by the normal loader.
Invalid paths, malformed archives and parse failures leave outgoing policies alone.
No storage format changes or data migration are required.

The CMake include generates AppModel.cpp under `target/shim/freeze-project-load`
and substitutes only that target source. The dependency checkout, versions,
existing repairs and build method remain unchanged. Both existing caches and
fresh checkouts regenerate through the same configure step.

Run `python -B patches/uapmd/freeze-project-load/test_generate.py`, the normal
release build, `python -B scripts/freeze-test.py`, and
`python -B scripts/project-save-test.py`. Runtime tests use isolated local data
outside the repository and cover rendering/completed freeze, failed sources,
repeated replacement, plugin state and fresh-process restoration.

Missing/repeated anchors and missing target entries stop configuration with the
repair name, source, failure condition, upstream revision and this document.
Inspect upstream load validation, teardown and freeze cancellation before adapting
the generator. Remove the include and this directory once upstream quiesces
outgoing freeze before plugin teardown; retain the runtime regressions.

Report status: **未報告**; draft: [UPSTREAM_REPORT.md](UPSTREAM_REPORT.md).
