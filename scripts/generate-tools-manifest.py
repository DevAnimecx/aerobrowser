"""Regenerate docs/tools.json from the binary that is actually built.

The manifest is what external consumers read, and the README's headline
claim is a tool count. If the manifest is hand-maintained it drifts, and a
drifted tool count is worse than no count: `batch` was missing from it
entirely while the server happily served it.

Run from the repository root:

    cargo build --release
    python scripts/generate-tools-manifest.py [path-to-binary]
"""

import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "docs" / "tools.json"


def main():
    if len(sys.argv) > 1:
        binary = sys.argv[1]
    else:
        for candidate in ("target/release/delta", "target/release/delta.exe"):
            path = ROOT / candidate
            if path.exists():
                binary = str(path)
                break
        else:
            print("no built binary found; run `cargo build --release` first")
            return 1

    raw = subprocess.run(
        [binary, "tools"], capture_output=True, text=True, check=True
    ).stdout
    tools = json.loads(raw)["tools"]
    tools.sort(key=lambda t: t["name"])

    # newline="\n" is not optional. Path.write_text translates "\n" to the
    # platform newline, so generating on Windows produced a CRLF file that a
    # Linux CI run then rewrote as LF and reported as a change, even though
    # the content was byte-identical.
    with OUTPUT.open("w", encoding="utf-8", newline="\n") as fh:
        fh.write(json.dumps({"tools": tools}, indent=2, sort_keys=True) + "\n")
    print("wrote %s (%d tools)" % (OUTPUT.relative_to(ROOT), len(tools)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
