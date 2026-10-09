"""Check that documentation links point at files that exist.

Catches the common failure where a README advertises a translated readme or a
release-notes page that was never written.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DOCS = [p for p in ROOT.rglob("*.md") if "target" not in p.parts and ".opencode" not in p.parts]

LINK_RE = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
BARE_MD_RE = re.compile(r"`([A-Za-z0-9_./-]+\.md)`")


def main():
    problems = []
    for path in DOCS:
        text = path.read_text(encoding="utf-8")
        candidates = [m.group(1) for m in LINK_RE.finditer(text)]
        # Backticked filenames, excluding any that are already the text of a
        # markdown link (those are covered by LINK_RE with their real target).
        without_links = LINK_RE.sub(" ", text)
        candidates += [m.group(1) for m in BARE_MD_RE.finditer(without_links)]
        for target in candidates:
            target = target.split("#")[0].strip()
            if not target or target.startswith(("http://", "https://", "mailto:")):
                continue
            resolved = (path.parent / target).resolve()
            if not resolved.exists():
                problems.append("%s -> %s" % (path.relative_to(ROOT), target))

    for problem in sorted(set(problems)):
        print("broken link: %s" % problem)
    if problems:
        print("\n%d broken link(s)" % len(set(problems)))
        return 1
    print("all internal documentation links resolve")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())