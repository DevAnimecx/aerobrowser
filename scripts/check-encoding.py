"""Report any remaining mojibake across the project's text files.

Prints ASCII-only so it can run on any console, and exits non-zero if any file
still contains corruption markers.
"""

import pathlib
import sys

MARKERS = {
    "U+00C3 (Ã)": "Ã",
    "U+00C2 (Â)": "Â",
    "U+201A-201E/2039 (â€)": "â€",
    "U+FFFD (replacement)": "�",
    "BOM mid-file": "﻿",
}

SUFFIXES = {".md", ".rs", ".sh", ".ps1", ".py", ".yml", ".yaml", ".json", ".toml", ".html", ".aero"}
SKIP_DIRS = {"target", "node_modules", ".git", "artifacts", "package-v04", ".opencode"}

# These files name the corruption markers as test data, so they must contain them.
MARKER_FILES = {"scripts/check-encoding.py", "scripts/repair-encoding.py"}
MARKER_ASSIGNMENT = "for marker in ["


def main():
    root = pathlib.Path(__file__).resolve().parent.parent
    findings = []
    for path in sorted(root.rglob("*")):
        if not path.is_file() or path.suffix.lower() not in SUFFIXES:
            continue
        if any(part in SKIP_DIRS for part in path.relative_to(root).parts):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            findings.append((path, ["not valid UTF-8"]))
            continue
        relative = path.relative_to(root).as_posix()
        hits = []
        for label, marker in MARKERS.items():
            if marker not in text:
                continue
            if relative in MARKER_FILES:
                continue
            # Occurrences that exist to assert on the markers are legitimate.
            lines = [
                n
                for n, line in enumerate(text.splitlines(), 1)
                if marker in line and MARKER_ASSIGNMENT not in line
            ]
            if lines:
                hits.append("%s x%d (line %d)" % (label, len(lines), lines[0]))
        if hits:
            findings.append((path, hits))

    for path, hits in findings:
        print("%-56s %s" % (path.relative_to(root), "; ".join(hits)))
    if findings:
        print("\n%d file(s) still contain corruption" % len(findings))
        return 1
    print("clean: no mojibake markers found")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())