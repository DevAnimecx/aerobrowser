"""Check that the version and license story agree across every file.

A mismatch between Cargo.toml, the SBOM, the LICENSE header and THIRD-PARTY-NOTICES
is the kind of inconsistency that surfaces in a security review, so it is a
build failure rather than a note.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MIN_LICENSE_CHARACTERS = 400
BSL_REQUIRED_FIELDS = ["Licensor:", "Licensed Work:", "Change Date:", "Change License:", "Additional Use Grant"]


def cargo_version():
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
    return match.group(1) if match else None


def main():
    failures = []
    version = cargo_version()
    if not version:
        print("could not read version from Cargo.toml")
        return 1
    major_minor = ".".join(version.split(".")[:2])

    # SBOM must describe the version being built.
    sbom = ROOT / "docs" / "SBOM.cdx.json"
    if sbom.exists():
        content = sbom.read_text(encoding="utf-8")
        match = re.search(r'"version"\s*:\s*"(\d+\.\d+\.\d+)"', content)
        if not match:
            failures.append("docs/SBOM.cdx.json has no component version")
        elif match.group(1) != version:
            failures.append(
                "docs/SBOM.cdx.json describes %s but Cargo.toml is %s" % (match.group(1), version)
            )

    # LICENSE must be the real BSL-1.1 text, with an Additional Use Grant.
    license_text = (ROOT / "LICENSE").read_text(encoding="utf-8")
    if len(license_text) < MIN_LICENSE_CHARACTERS:
        failures.append("LICENSE is too short to be a real license text")
    for field in BSL_REQUIRED_FIELDS:
        if field not in license_text:
            failures.append("LICENSE is missing the required BSL-1.1 field %r" % field)
    if "Apache License, Version 2.0" not in license_text:
        failures.append("LICENSE does not name the Change License")

    # The notices file must not contradict the license.
    notices = (ROOT / "THIRD-PARTY-NOTICES.md").read_text(encoding="utf-8").lower()
    if "mit licensed" in notices or "is mit " in notices:
        failures.append("THIRD-PARTY-NOTICES.md claims MIT while LICENSE is BSL-1.1")
    if "business source license" not in notices:
        failures.append("THIRD-PARTY-NOTICES.md does not name the actual license")

    # Every readme must agree on the version line if it quotes one.
    for name in ("README.md", "README.es.md", "README.zh.md"):
        readme = ROOT / name
        if not readme.exists():
            continue
        for quoted in re.findall(r"aero (\d+\.\d+\.\d+)", readme.read_text(encoding="utf-8")):
            if quoted != version:
                failures.append("%s quotes version %s but Cargo.toml is %s" % (name, quoted, version))

    # Release notes must exist for this version.
    if not (ROOT / "docs" / ("RELEASE-%s.md" % major_minor)).exists():
        failures.append("docs/RELEASE-%s.md is missing for the shipping version" % major_minor)

    for failure in failures:
        print("inconsistent: %s" % failure)
    if failures:
        return 1
    print("metadata consistent at version %s" % version)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())