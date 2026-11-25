"""Honor the requested device period without changing the dependency checkout."""
from pathlib import Path
import subprocess
import sys

NAME = "device-buffer-size"
REVISION = "9f708d3300b4a6f8109eee0fadccf286db8b7a45"
REPLACEMENTS = (
    ("    (void) bufferSize;", "    audio.setPreferredCallbackSize(bufferSize);"),
    ("    deviceConfig.periodSizeInFrames = config.periodSizeInFrames;",
     "    config.periodSizeInFrames = preferred_callback_frames_ > 0 ? preferred_callback_frames_ : 256;\n"
     "    deviceConfig.periodSizeInFrames = config.periodSizeInFrames;"),
    ("    // Stop the engine if it's running\n    if (isPlaying())\n        stop();\n",
     "    // Stop the engine if it's running\n    const bool wasPlaying = isPlaying();\n    if (wasPlaying)\n        stop();\n"),
    ("    // Always start after reinit: noAutoStart = MA_TRUE means ma_engine_init\n"
     "    // never auto-starts, so we must call start() explicitly here.\n    start();\n",
     "    // A stopped device stays stopped; the audio engine owns starting it.\n"
     "    if (wasPlaying)\n        start();\n"),
)


def transform(source):
    for old, new in REPLACEMENTS:
        if source.count(old) != 1:
            raise ValueError(f"expected exactly one occurrence of {old!r}; got {source.count(old)}")
        source = source.replace(old, new)
    return source


def main():
    source, output, checkout = map(Path, sys.argv[1:])
    try:
        patched = transform(source.read_text(encoding="utf-8"))
        output.parent.mkdir(parents=True, exist_ok=True)
        if not output.exists() or output.read_text(encoding="utf-8") != patched:
            output.write_text(patched, encoding="utf-8")
    except (OSError, ValueError) as error:
        revision = subprocess.run(["git", "-C", str(checkout), "rev-parse", "HEAD"],
                                  capture_output=True, text=True).stdout.strip() or "unknown"
        sys.exit(f"{NAME}: {source}: {error}; upstream {revision} (validated {REVISION}); "
                 f"see {Path(__file__).with_name('README.md')}")


if __name__ == "__main__":
    main()
