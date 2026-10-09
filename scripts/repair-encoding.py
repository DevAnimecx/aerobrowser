"""Repair UTF-8 text that was decoded as cp1252 or latin-1.

Byte values 0x80-0x9F are unmapped in latin-1 but meaningful in cp1252, and a
round trip can mix both depending on which tool touched the file. So each
character is encoded independently: latin-1 when it has a byte below 0x100,
otherwise the cp1252 slot for that code point. The result is only written back
if it decodes as clean UTF-8, so real text is never damaged.
"""

import sys

# Byte sequences that only appear when UTF-8 was decoded as cp1252/latin-1.
CORRUPTION_MARKERS = ("Ã", "â€", "Â", "ï¿½")

CP1252_HIGH = {
    0x20AC: 0x80, 0x201A: 0x82, 0x0192: 0x83, 0x201E: 0x84,
    0x2026: 0x85, 0x2020: 0x86, 0x2021: 0x87, 0x02C6: 0x88,
    0x2030: 0x89, 0x0160: 0x8A, 0x2039: 0x8B, 0x0152: 0x8C,
    0x017D: 0x8E, 0x2018: 0x91, 0x2019: 0x92, 0x201C: 0x93,
    0x201D: 0x94, 0x2022: 0x95, 0x2013: 0x96, 0x2014: 0x97,
    0x02DC: 0x98, 0x2122: 0x99, 0x0161: 0x9A, 0x203A: 0x9B,
    0x0153: 0x9C, 0x017E: 0x9E, 0x0178: 0x9F,
}


def to_bytes(text):
    out = bytearray()
    for ch in text:
        code = ord(ch)
        if code < 0x100:
            out.append(code)
        elif code in CP1252_HIGH:
            out.append(CP1252_HIGH[code])
        else:
            raise ValueError("no source encoding for %r" % ch)
    return bytes(out)


def repair(text):
    """Return the repaired text, or None when no clean round trip exists."""
    body = text.lstrip("﻿")
    try:
        raw = to_bytes(body)
    except ValueError:
        # A character that could not come from a single-byte decoding: the file
        # either is already correct or was corrupted lossily.
        return None
    try:
        candidate = raw.decode("utf-8")
    except UnicodeDecodeError:
        return None
    if candidate == body:
        return None
    if any(marker in candidate for marker in CORRUPTION_MARKERS):
        # One pass was not enough; refuse rather than write something half-fixed.
        return None
    return candidate


def main(paths):
    changed = []
    for path in paths:
        with open(path, encoding="utf-8") as handle:
            original = handle.read()
        fixed = repair(original)
        if fixed is None:
            print("skip  %s" % path)
            continue
        with open(path, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(fixed)
        print("fixed %s" % path)
        changed.append(path)
    print("\n%d file(s) repaired" % len(changed))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))