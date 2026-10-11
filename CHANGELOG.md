# Changelog

All notable changes to Delta Browser are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

---

## Provenance: AeroBrowser → Delta Browser

Delta Browser is the renamed and continued development of **AeroBrowser**,
published by the same author (Blackvault Technology, Inc.) as versions 0.1
through 0.5 under the same Business Source License 1.1.

The rename is a rebranding of the author's own work. The copyright is
unchanged, the earlier releases remain available in the git history, and
nothing about the licensing terms was altered by the rename.

The version was reset from 0.5.0 to 0.0.1 to mark a clean start under the new
name rather than implying feature parity with a differently-numbered lineage.

### Migration table

| AeroBrowser | Delta Browser |
| --- | --- |
| `aero <command>` | `delta <command>` |
| crate `aerobrowser` | crate `deltabrowser` |
| `AERO_*` | `DELTA_*` |
| `~/.aero/` | `~/.delta/` |
| `.aero-token.json` | `.delta-token.json` |
| `Install-AeroBrowser` | `Install-DeltaBrowser` |
| `mcpServers.aero` | `mcpServers.delta` |
| `aero-<platform>` assets | `delta-<platform>` assets |
| `examples/login.aero` | `examples/login.delta` |
| `*.aero-skill.md` | `*.delta-skill.md` |

The delta-snapshot **feature** identifiers (`DeltaEntry`, `SnapshotDelta`,
`delta_payload`, `snapshot { delta: true }`) are unrelated to the brand and are
unchanged.

---

## [0.0.1] — 2026-10-11

First release under the Delta Browser name, and the first tagged release of
this repository. Full notes: [docs/RELEASE-0.0.md](docs/RELEASE-0.0.md).

### Changed

- Renamed the product to **Delta Browser**: crate `deltabrowser`, binary
  `delta`, brand strings, runtime directories, CDP binding names, release
  asset names and all documentation.
- Moved every configuration variable from `AERO_*` to `DELTA_*`.
- Moved runtime state from `~/.aero/` to `~/.delta/` and the token file to
  `.delta-token.json`, so a Delta install never reads AeroBrowser state.
- Standardised every install URL on the `main` branch, which is the actual
  default branch of the repository.

### Fixed

- `profiles::home_dir` tests mutated process-global environment variables and
  raced each other under `cargo test`, producing intermittent failures. They are
  now serialised behind a mutex so a plain `cargo test` is deterministic, not
  only a CI run with `--test-threads=1`.

---

## Earlier releases, published as AeroBrowser

These shipped as AeroBrowser and are preserved in the git history at the
commits that produced them. They are summarised here rather than reproduced, so
this file is a single timeline rather than two disconnected ones.

### 0.5.0 — batched calls, delta snapshots, verifiable install

- `batch`: run up to 50 tools in one MCP message against a single browser
  context, validated against the same schema as a standalone `tools/call`.
- `snapshot { delta: true }`: return only added, changed and removed refs.
- Replaced the install story with a verified one-liner that checks SHA-256 and
  refuses to install a payload that is not a native executable.
- Removed claims the code did not support.

### 0.4.0 — session state, cleanup, deployment

- `save_state` / `restore_state`: cookies, tabs, storage and scroll.
- Explicit session teardown and resource cleanup.
- Documented deployment boundaries.

### 0.3.0 — real cancellation, session ownership, HAR

- Working cancellation over the transport.
- Correct session ownership for concurrent clients.
- Measured HAR capture.

### 0.1.0–0.2.0 — first public shape

- The original agent browser CLI and MCP server, the managed Moli engine, and
  the first sandbox profiles.
- See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the design those
  releases settled on.

---

[0.0.1]: https://github.com/DevAnimecx/deltabrowser/releases/tag/v0.0.1
