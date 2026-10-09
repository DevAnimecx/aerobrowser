#!/usr/bin/env python3
"""Generate the declared-dependency CycloneDX inventory for AeroBrowser.

Scope, stated plainly: this is an inventory of the crates Cargo resolved for the
workspace plus the pinned Moli engine executable, using the licenses those crates
declare. It is not a full engine or OS SBOM, not a vulnerability assessment, and
not signed provenance.

    scripts/generate-sbom.py           # write docs/SBOM.cdx.json
    scripts/generate-sbom.py --check   # fail if the committed file is stale
"""

import argparse
import datetime
import json
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "docs" / "SBOM.cdx.json"

# Cargo.toml ships a single SPDX identifier, so the component can carry an
# `id` rather than a free-text name. Downstream tooling can validate it.
PROJECT_LICENSE = "BSL-1.1"
PROJECT_LICENSE_URL = "https://spdx.org/licenses/BSL-1.1.html"

SCOPE_PROPERTY = (
    "Cargo resolved dependencies across platforms plus the pinned Moli executable. "
    "Declared licenses only, not a legal audit. Not a full engine or OS SBOM, "
    "vulnerability assessment, or signed provenance."
)


def run(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True)


def cargo_version():
    match = re.search(r'^version\s*=\s*"([^"]+)"', (ROOT / "Cargo.toml").read_text(encoding="utf-8"), re.M)
    if not match:
        raise SystemExit("could not read version from Cargo.toml")
    return match.group(1)


def purl_for(name, version):
    return "pkg:cargo/%s@%s" % (quote(name), quote(version, safe=""))


def build():
    version = cargo_version()
    metadata = json.loads(run("cargo", "metadata", "--locked", "--format-version", "1"))
    refs = {p["id"]: purl_for(p["name"], p["version"]) for p in metadata["packages"]}
    root_id = metadata["resolve"]["root"]

    components = []
    application = None
    for package in metadata["packages"]:
        ref = refs[package["id"]]
        component = {
            "type": "application" if package["id"] == root_id else "library",
            "bom-ref": ref,
            "name": package["name"],
            "version": package["version"],
            "purl": ref,
        }
        if package["id"] == root_id:
            # SPDX identifier, not a label.
            component["licenses"] = [
                {"license": {"id": PROJECT_LICENSE, "url": PROJECT_LICENSE_URL}}
            ]
            component["properties"] = [
                {"name": "aero:commercial-license", "value": "paid license required above USD 10M revenue or for service offerings; see LICENSE"}
            ]
            application = component
        else:
            if package.get("license"):
                component["licenses"] = [{"license": {"name": package["license"]}}]
            if package.get("repository"):
                component["externalReferences"] = [{"type": "vcs", "url": package["repository"]}]
            components.append(component)

    pin = json.loads((ROOT / "docs" / "engine-pin.json").read_text(encoding="utf-8"))
    engine_version = pin["version"].lstrip("v")
    engine_ref = "pkg:generic/lexmount/moli@%s" % engine_version
    components.append(
        {
            "type": "application",
            "bom-ref": engine_ref,
            "name": "Moli",
            "version": engine_version,
            "purl": engine_ref,
            "hashes": [{"alg": "SHA-256", "content": pin["executable_sha256"]}],
            "externalReferences": [{"type": "distribution", "url": pin["url"]}],
            "properties": [
                {
                    "name": "aero:inventory-scope",
                    "value": "Pinned engine executable only; see bundled upstream licenses for its dependencies.",
                }
            ],
        }
    )

    dependencies = [
        {
            "ref": refs[node["id"]],
            "dependsOn": [refs[d] for d in node["dependencies"]]
            + ([engine_ref] if node["id"] == root_id else []),
        }
        for node in metadata["resolve"]["nodes"]
    ]
    dependencies.append({"ref": engine_ref, "dependsOn": []})

    return {
        "bomFormat": "CycloneDX",
        "specVersion": "1.6",
        "version": 1,
        "metadata": {
            "timestamp": datetime.datetime.now(datetime.timezone.utc)
            .replace(microsecond=0)
            .isoformat()
            .replace("+00:00", "Z"),
            "component": application,
            "properties": [{"name": "aero:scope", "value": SCOPE_PROPERTY}],
        },
        "components": components,
        "dependencies": dependencies,
        # Explicitly empty rather than absent: an absent array reads as
        # "never scanned", an empty one reads as "scanned, nothing found".
        "vulnerabilities": [],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when the committed SBOM is missing, stale, or has no vulnerabilities array",
    )
    args = parser.parse_args()

    bom = build()
    rendered = json.dumps(bom, indent=2) + "\n"

    if args.check:
        if not OUTPUT.exists():
            print("docs/SBOM.cdx.json is missing; run scripts/generate-sbom.py")
            return 1
        committed = json.loads(OUTPUT.read_text(encoding="utf-8"))
        problems = []
        if committed.get("metadata", {}).get("component", {}).get("version") != bom["metadata"]["component"]["version"]:
            problems.append(
                "committed SBOM describes version %s but the workspace is %s"
                % (
                    committed.get("metadata", {}).get("component", {}).get("version"),
                    bom["metadata"]["component"]["version"],
                )
            )
        if "vulnerabilities" not in committed:
            problems.append(
                "committed SBOM has no vulnerabilities array; add an explicit empty one so "
                "readers can tell 'scanned, none found' from 'never scanned'"
            )
        if committed.get("metadata", {}).get("component", {}).get("licenses", [{}])[0].get("license", {}).get("id") != PROJECT_LICENSE:
            problems.append("committed SBOM does not carry the SPDX license id %s" % PROJECT_LICENSE)
        for problem in problems:
            print("sbom check failed: %s" % problem)
        if problems:
            return 1
        print("SBOM consistent with the workspace (%d components)" % len(bom["components"]))
        return 0

    OUTPUT.write_text(rendered, encoding="utf-8")
    print("wrote %s (%d components)" % (OUTPUT.relative_to(ROOT), len(bom["components"])))
    return 0


if __name__ == "__main__":
    sys.exit(main())