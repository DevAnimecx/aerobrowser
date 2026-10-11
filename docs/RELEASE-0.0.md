# Delta Browser 0.0.1 — the first release under the Delta Browser name

Delta Browser is a Model Context Protocol browser for AI agents. This is
version 0.0.1: the first release published under the Delta Browser name, and
the first tagged release of the repository at all.

Everything technical here was already built and tested. What 0.0.1 changes is
the **name**, and the cost of that change is the breaking configuration rename
described below.

---

## Install

One command. No admin rights, no API keys, no browser driver.

```sh
# Linux, macOS, WSL
curl -fsSL https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.sh | sh
```

```powershell
# Windows PowerShell
irm https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.ps1 | iex; Install-DeltaBrowser
```

```powershell
# Windows CMD
powershell -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.ps1 | iex; Install-DeltaBrowser"
```

Then prove it works:

```sh
delta doctor
```

## Breaking changes

If you are coming from AeroBrowser 0.5, everything below changed. There are no
compatibility shims, on purpose: a Delta install should never read AeroBrowser
state by accident.

| Before (AeroBrowser) | Now (Delta Browser) |
| --- | --- |
| `aero <command>` | `delta <command>` |
| `aerobrowser` (crate) | `deltabrowser` |
| `AERO_*` environment variables | `DELTA_*` |
| `~/.aero/` runtime state | `~/.delta/` |
| `.aero-token.json` | `.delta-token.json` |
| `Install-AeroBrowser` (PowerShell) | `Install-DeltaBrowser` |
| `mcpServers.aero` config key | `mcpServers.delta` |
| Release assets `aero-linux-x86_64` | `delta-linux-x86_64` |
| `examples/login.aero` | `examples/login.delta` |

The full environment variable list is in [INSTALL.md](INSTALL.md).

Renaming the host environment also renamed two features' *internal* identifiers
(`DeltaEntry`, `SnapshotDelta`, `delta_payload`). Those name the delta-snapshot
**feature**, not the brand, and their behaviour is unchanged.

## What is in the box

**62 MCP tools.** Navigation (`navigate`, `go_back`, `go_forward`, `reload`),
interaction (`click`, `fill`, `type`, `select_option`, `hover`, `drag`,
`press_key`, `check`, `uncheck`), extraction (`snapshot`, `get_text`,
`get_attribute`, `get_dom`, `evaluate`, `get_computed_style`), observation
(`screenshot`, `save_pdf`, `annotate_screenshot`, `get_console_logs`,
`get_network_log`, `save_har`, `get_performance`, `get_memory_usage`), state
(`save_state`, `restore_state`), assertions (`test_run`, `test_assert_text`,
`test_assert_url`, `test_assert_visible`, `test_report`), and control
(`server_status`, `list_tabs`, `new_tab`, `switch_tab`, `close_tab`, `batch`).

**Two tools exist purely to cut round-trips.**

```json
{"calls": [
  {"name": "navigate", "arguments": {"url": "https://example.com/login"}},
  {"name": "snapshot",  "arguments": {"interactive_only": true}},
  {"name": "fill",      "arguments": {"ref": "@e1_4", "value": "user@example.com"}},
  {"name": "click",     "arguments": {"ref": "@e1_5"}}
]}
```

Four actions in one MCP message is one latency boundary instead of four.

```json
{"delta": true}
```

`snapshot { delta: true }` returns only what changed since the previous
snapshot on that tab — added, changed, and removed refs — instead of the whole
accessibility tree.

**Engines.** Managed (sandboxed, Linux x86_64), bundled Moli, external Chrome,
external Edge, Lightpanda. `delta connect <agent> --install` detects what is on
the machine and writes the MCP block for it.

**Honest isolation.** On Linux x86_64 the managed engine runs each session in
its own worker with a Landlock filesystem policy and a seccomp syscall filter
applied *before* the browser process starts, with optional cgroup v2 memory and
PID ceilings. On Windows and macOS there is no equivalent, and `server_status`
says so rather than implying protection that does not exist.

## Verify the install

```sh
delta doctor
```

`doctor` reports the engine, the sandbox capability, the CDP and daemon ports,
the token, and the config paths of every agent it can detect. It exits non-zero
when something is actually wrong, so it works as a CI gate.

## Supported platforms

| Platform | Engine | Kernel sandbox |
| --- | --- | --- |
| Linux x86_64 | managed / bundled Moli | Landlock + seccomp + cgroup v2 |
| Linux aarch64 | external browser | none |
| macOS (Intel, Apple Silicon) | external browser | none |
| Windows (x64, ARM64) | external browser | none |

## Security

Report a vulnerability through GitHub's private advisory form on the
repository, not as a public issue. See [SECURITY.md](../SECURITY.md).

## Provenance

Delta Browser is the renamed and continued development of AeroBrowser, which
the same author published as versions 0.1 through 0.5 under the same Business
Source License 1.1. The copyright is unchanged and the earlier releases remain
available in the repository history. See [CHANGELOG.md](../CHANGELOG.md).
