# AeroBrowser 0.5 — batched calls, delta snapshots, and an install you can verify

0.5 makes three changes to how an agent drives AeroBrowser, replaces the install
story, and removes a set of claims that the code did not support.

---

## What an agent can do differently now

### `batch` — several tools in one message

```json
{"calls": [
  {"name": "navigate", "arguments": {"url": "https://example.com/login"}},
  {"name": "snapshot", "arguments": {"interactive_only": true}},
  {"name": "fill", "arguments": {"ref": "@e1_4", "value": "user@example.com"}},
  {"name": "click", "arguments": {"ref": "@e1_5"}}
]}
```

Calls run in order against one browser context. Each call is validated against the
same schema as a standalone `tools/call`, so a bad argument fails before it
reaches the browser. Results are reported per call with `ok`, `result` or `error`,
and `duration_ms`. `stop_on_error` defaults to `true`; set it to `false` to
continue past a failure. Capped at 50 calls per batch.

The point is round-trips. Four actions in one message is one latency boundary
instead of four.

### `snapshot { delta: true }` — only what changed

Returns added elements, changed elements, and tombstones for refs that no longer
exist:

```json
{"text": "+ button Sign in [ref=e4_2]\n~ changed textbox Email [ref=e4_1]\n- removed [ref=e2_7]",
 "elements": [...], "changed": [...], "removed": ["e2_7"],
 "delta": {"baseline": false, "added": 1, "changed": 1, "removed": 1}}
```

Each element is fingerprinted on the fields an agent actually acts on — role,
accessible name, depth, disabled state — so a re-render with identical semantics
produces no output, while a rename or a role change does.

Three cases deliberately return a **full** snapshot with `"baseline": false` and a
`reason`: no previous snapshot on this tab, the document changed, or the URL
changed. A partial answer is least useful exactly when the agent has the least
context, so those cases do not guess.

### 62 tools

`batch` and the `delta` parameter on `snapshot` are new. `set_viewport`,
`upload_file`, and `close` were already implemented but were missing from the
README index.

---

## Install story replaced

The previous one-liner pointed at `get.aerobrowser.dev`, which does not resolve.
Installers now resolve the latest GitHub Release and verify it.

**Both installers now:**

- fetch with `--fail` semantics, so a 404 page can never be written to disk and
  announced as a successful install
- verify the published SHA-256 before installing
- confirm the payload is an ELF, Mach-O, or PE binary before touching `PATH`
- stage inside the destination directory and move atomically, so an interrupted
  download never leaves a truncated binary on your `PATH`
- run `aero --version` afterwards and remove the binary if it fails
- exit non-zero on failure instead of printing a success message

`install.ps1` additionally forces TLS 1.2 and silences progress output, both of
which are required for stock Windows PowerShell 5.1. It uses a random per-run temp
directory rather than a predictable shared path, and now defines the
`Install-AeroBrowser` function that the documentation had been promising.

The Linux and macOS installer no longer uses `grep -P`, which BSD grep on macOS
does not support, and no longer runs the engine download twice.

---

## `aero doctor`

```sh
aero doctor          # human-readable
aero doctor --json   # machine-readable
```

Checks the engine, sandbox support, browser and daemon ports, the auth token, and
all ten agent config paths. Every failing check prints the specific fix.

---

## Fixed

These were real defects, not polish:

- **`aero connect` could panic.** `Handle::block_on` was called from inside the
  tokio runtime, which panics unconditionally. It is now a plain `async fn`.
- **Generated tokens could never authenticate.** The generator produced 22
  characters; the auth layer requires at least 24. Tokens are now 44 characters,
  and existing short tokens are no longer skipped.
- **`aero connect --install` always failed on Windows and macOS.** Managed Moli
  downloads are Linux x86_64 only. It now adopts a Chromium-family browser found
  locally and says so in `platform_note`.
- **A dead session actor returned HTTP 200 with a null result.** It now returns
  503.
- **`aero test` could exit 0 on a failed suite.** A missing `failed` field was
  read as zero. It now refuses to report success.
- **Path resolution assumed `HOME`.** `USERPROFILE` and
  `HOMEDRIVE`+`HOMEPATH` are consulted as fallbacks, and the Claude Desktop config
  path is now platform-specific rather than always macOS.

---

## Claims removed

The previous README advertised an "80% less tokens" headline and seven tools that
did not exist: `batch`, `plan`, `ask`, `bookmark_save`, `bookmark_list`,
`session_save`, `session_restore`. Five of the mechanisms behind the headline
(batch, delta snapshots, persistent sessions, auto-refresh refs, AI element
descriptions) were absent. An unverifiable testimonial cited batch tool calls as
its cause. All of that has been removed.

`batch` and delta snapshots now exist and can be measured. No token-efficiency
percentage is claimed here, because publishing one requires a reproducible
benchmark that has not been run.

Also corrected: the agent table said eight clients were "Tested" when the only
automated coverage validated that a config file was well-formed, and the sandbox
row implied completeness that three unimplemented profiles contradict.

---

## Verification

- CI now runs on `main` and `master` (the repository was on `master` while the
  workflows triggered on `main`, so CI had never run).
- `cargo clippy --all-targets -- -D warnings` is enforced, as
  [CONTRIBUTING.md](../CONTRIBUTING.md) already required.
- Windows is in the test matrix, matching the release artifacts.
- The seven integration suites that were never wired to CI now run, alongside
  `v03_stdio_cancel.py`. Moli, cgroup, and Landlock suites moved to a dedicated
  sandbox job that skips with a notice when the kernel lacks the controllers
  instead of failing opaquely.
- `scripts/check-encoding.py` fails the build on mojibake. Nine files were
  affected, including both translated READMEs and the `--help` text.
- `scripts/check-docs.py` fails the build on broken internal links.
- `scripts/check-metadata.py` fails the build when the version or license story
  disagrees across files.
- The SBOM is regenerated in CI and now carries an SPDX license id and an explicit
  empty `vulnerabilities` array.

Test count went from 21 to 35, adding coverage for the previously untested CLI
surface, report rendering, and the batch tool.

---

## Licensing

Now genuine Business Source License 1.1 with the required `Licensor`,
`Licensed Work`, `Change Date`, `Change License`, and `Additional Use Grant`
fields. Free for personal, research, educational, and internal business use below
USD 10M annual revenue. Production use above that threshold, and offering browser
automation as a service, require a commercial license.

[THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md) previously stated the source was
MIT, which contradicted every other file. It now matches.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the verification commands referenced above.

---

## Still missing

Unchanged from [ROADMAP.md](ROADMAP.md), and worth repeating here:

- No kernel-level network egress boundary. The CDP allowlist is target-scoped,
  not a firewall. Hostname and IP mediation at connection time is the top
  outstanding item.
- `base`, `web-dev`, and `test` sandbox profiles are declared but not implemented.
  They fail closed.
- macOS Seatbelt is unvalidated. Windows has no AppContainer or Job Object
  enforcement.
- State is not encrypted at rest, and the auth token is stored in plaintext.
- No client has been run against a live server in the validation environment.
- Not published to Homebrew, apt, rpm, or crates.io.
- No signed provenance for releases.