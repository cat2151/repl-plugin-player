"""Open the audio device for playback only, layered on device-buffer-size's output."""
from pathlib import Path
import sys

NAME = "playback-only-device"
REVISION = "9f708d3300b4a6f8109eee0fadccf286db8b7a45"
OLD = "    const bool enableInput = inputDeviceIndex != AudioIODeviceManager::kNoDeviceIndex;"
NEW = ("    // This app has no audio input. A duplex device writes playback only when capture\n"
       "    // data arrives, which underruns the render buffer shortly after every start.\n"
       "    const bool enableInput = false;")


def transform(source):
    if source.count(OLD) != 1:
        raise ValueError(f"expected exactly one occurrence of {OLD!r}; got {source.count(OLD)}")
    return source.replace(OLD, NEW)


def main():
    source, output = map(Path, sys.argv[1:3])
    revision = sys.argv[3] if len(sys.argv) > 3 else "unknown"
    try:
        patched = transform(source.read_text(encoding="utf-8"))
        output.parent.mkdir(parents=True, exist_ok=True)
        if not output.exists() or output.read_text(encoding="utf-8") != patched:
            output.write_text(patched, encoding="utf-8")
    except (OSError, ValueError) as error:
        sys.exit(f"{NAME}: {source}: {error}; upstream {revision} (validated {REVISION}); "
                 f"see {Path(__file__).with_name('README.md')}")


if __name__ == "__main__":
    main()
