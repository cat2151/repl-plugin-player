# CLAP input note dialect repair

UAPMD `4ec5c3b1c30130fdd3b7d4b375082273247e561f` sends every note as a native CLAP Note event.
sforzando 2.1.2.4 accepts MIDI on input port 0, so an SFZ state can restore correctly while playback remains silent.
This repair reuses cat-plugin-player's existing dialect selection implementation: collect supported port dialects before activation, prefer native CLAP notes when supported, otherwise send MIDI, once per event. CC uses MIDI-capable ports.
UAPMD gives each instance on a track its own UMP group (`SequencerTrack::findAvailableGroup`), so an instance created while another is on the track receives group 1. The group selects the note port only when the plugin has that many input note ports; otherwise the event goes to port 0, so a single-port plugin is not silenced.

`apply.py` validates normalized source SHA256s and generates all translation units sharing the changed class header under `target/shim/generated/clap-note-dialect`.
`native/CMakeLists.txt` includes the repair on every configure; source and generator changes trigger reconfiguration.
The UAPMD checkout and existing patches are untouched. A fresh clone matching the baselines uses the same generation; unchanged generated files keep their timestamps on cached builds.
The generated Parameters translation unit retains the existing parameter-metadata overlay and switches its class-header include to the same generated header as all other CLAP units.
If upstream changes, generation stops with the source, revision and this README path. Inspect the change for an upstream fix before updating baselines or removing the repair.
Remove this repair and its include after upstream correctly handles MIDI-only, native-only and both-capable ports without duplicate notes and delivers group 1 notes to single-port plugins, and the regression tests pass without the repair.

Validation: `test_apply.py --source <UAPMD CLAP source directory> --work-dir <new directory outside repositories>` checks generation and drift handling; CMake target `clap_note_dialect_test` checks dialect selection, velocities, CC and the group-to-port mapping. `clap_note_native_ab` compares a real plugin through a device-free direct CLAP host. `cat-smf-editor/scripts/uapmd-effects-test.py` verifies exported SFZ states and graphs in fresh repl sessions.

Upstream report: not sent; see `UPSTREAM_REPORT.md`.
