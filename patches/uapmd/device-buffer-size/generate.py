"""Honor the requested device period without changing the dependency checkout."""
from pathlib import Path
import subprocess
import sys

NAME = "device-buffer-size"
REVISION = "4ec5c3b1c30130fdd3b7d4b375082273247e561f"
REPLACEMENTS = (
    ("    (void) bufferSize;", "    audio.setPreferredCallbackSize(bufferSize);"),
    ("    deviceConfig.periodSizeInFrames = config.periodSizeInFrames;",
     "    config.periodSizeInFrames = preferred_callback_frames_ > 0 ? preferred_callback_frames_ : 256;\n"
     "    deviceConfig.periodSizeInFrames = config.periodSizeInFrames;"),
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
