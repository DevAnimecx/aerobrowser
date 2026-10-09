# AeroBrowser — The Agent's Personal Browser

[![CI](https://github.com/DevAnimecx/aerobrowser/actions/workflows/ci.yml/badge.svg)](https://github.com/DevAnimecx/aerobrowser/actions/workflows/ci.yml)
[![Release](https://github.com/DevAnimecx/aerobrowser/actions/workflows/release.yml/badge.svg)](https://github.com/DevAnimecx/aerobrowser/releases)
[![License: BSL-1.1](https://img.shields.io/badge/License-BSL--1.1-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-Stdio%20%7C%20HTTP%20%7C%20SSE-green)](https://modelcontextprotocol.io)

**62 MCP tools. A real sandboxed browser. One command to verify your install.**

AeroBrowser connects an AI agent to a real browser through the [Model Context Protocol](https://modelcontextprotocol.io). It ships a sandboxed managed engine, batched tool calls, delta snapshots, persistent sessions, and one-command agent setup.

Originally by [DevAnimecx/aerobrowser](https://github.com/DevAnimecx/aerobrowser) · Maintained under Blackvault Technology · Created by Adarsh Kushwah

---

## Languages

- [English](README.md) · [中文](README.zh.md) · [Español](README.es.md)

---

## Install

Pre-built binaries are published as GitHub Releases, with a `SHA256SUMS` file that the installers verify.

```sh
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/DevAnimecx/aerobrowser/master/scripts/install.sh | sh
```

```powershell
# Windows PowerShell (5.1 or newer)
irm https://raw.githubusercontent.com/DevAnimecx/aerobrowser/master/scripts/install.ps1 | iex; Install-AeroBrowser -Agent opencode
```

Both installers resolve the latest release from the GitHub API, verify the SHA-256 digest, confirm the payload is a native executable before touching your `PATH`, and remove the binary if it fails to run afterwards.

Prefer to build from source?

```sh
git clone https://github.com/DevAnimecx/aerobrowser.git
cd aerobrowser && cargo build --release --locked
```

---

## Verify your install

```sh
aero doctor
```

`aero doctor` checks the engine, sandbox support, browser and daemon ports, the auth token, and every known agent config path. Each failing check prints the specific fix. Add `--json` for machine-readable output.

```
  AeroBrowser 0.5.0 — environment report: issues

  [ok  ] build             aero 0.5.0 on linux
  [ok  ] home_directory    /home/you
  [ok  ] engine_detected   moli via managed
  [ok  ] sandbox           Landlock ruleset applied
  [FAIL] cdp_port          nothing listening on 127.0.0.1:9222
                  -> Launch your browser with --remote-debugging-port=9222, or use a managed engine
  [ok  ] daemon_port       listening on 127.0.0.1:8931
  [ok  ] auth_token        stored token at /home/you/.aero/token.json (44 chars)

  Agent configs found: opencode, claude-code
  Tools available: 62
```

---

## Connect your agent

```sh
aero connect opencode          # OpenCode
aero connect claude-code       # Claude Code
aero connect cursor            # Cursor
aero connect codex             # Codex
aero connect claude-desktop    # Claude Desktop
```

Adapters exist for OpenCode, Claude Code, Claude Desktop, Cursor, VS Code, Codex, Windsurf, Cline, Roo Code, Hermes, and a generic `aero-mcp.json`. **Config generation is implemented and tested for all of them; no client has been run against a live server in the validation environment.** Treat "it wrote a config file" as distinct from "it has been verified end to end."

---

## What AeroBrowser actually does

### Fewer round-trips

`batch` runs several tools in one MCP message against a shared browser, reporting results per call so a partial failure stays inspectable:

```json
{"calls": [
  {"name": "navigate", "arguments": {"url": "https://example.com/login"}},
  {"name": "snapshot", "arguments": {"interactive_only": true}},
  {"name": "fill", "arguments": {"ref": "@e1_4", "value": "user@example.com"}},
  {"name": "click", "arguments": {"ref": "@e1_5"}}
]}
```

Set `stop_on_error: false` to continue past failures.

### Only changed elements

`snapshot { "delta": true }` returns added and changed elements plus tombstones for refs that disappeared, instead of the whole accessibility tree:

```json
{"text": "+ button Sign in [ref=e4_2]\n~ changed textbox Email [ref=e4_1]\n- removed [ref=e2_7]",
 "elements": [...], "changed": [...], "removed": ["e2_7"],
 "delta": {"baseline": false, "added": 1, "changed": 1, "removed": 1}}
```

The first call, a document change, or a URL change returns a full snapshot with `"baseline": false` and a reason — a partial answer would be least useful exactly when the agent has the least context.

### Persistent sessions

`save_state` / `restore_state` export cookies, tabs, storage, and scroll position, and stage them into a new context before the browser boots. This is **not** a process checkpoint, and state is not encrypted at rest. Use it to avoid re-authenticating, not to protect secrets.

### A sandbox that fails closed

On Linux x86_64, the managed Moli engine runs in a dedicated worker process with a Landlock filesystem policy and seccomp-BPF syscall filtering applied before the browser starts. cgroup v2 can enforce hard memory and PID ceilings, with OOM confined to the worker.

Where a policy is unimplemented, AeroBrowser refuses rather than degrading to something weaker. `base`, `web-dev`, and `test` profiles are declared but **not implemented** and error out. There is no kernel-level network egress boundary — the CDP allowlist in `cdp_raw` is target-scoped, not a firewall. macOS Seatbelt enforcement is unvalidated. Windows has no AppContainer or Job Object enforcement; use an external browser there and treat containment as the browser's responsibility.

See [docs/SECURITY.md](docs/SECURITY.md) for the full boundary, and [docs/ROADMAP.md](docs/ROADMAP.md) for what is explicitly missing.

---

## Tools (62)

| Group | Tools |
|---|---|
| Navigation | `navigate` `snapshot` `reload` `go_back` `go_forward` `wait_for` `scroll` |
| Interaction | `click` `double_click` `right_click` `fill` `type` `press_key` `hover` `drag` `upload_file` |
| Elements | `get_text` `get_attribute` `get_bounding_box` `get_computed_style` `get_dom` `get_title` `get_url` `evaluate` |
| Forms | `select_option` `check` `uncheck` `set_viewport` |
| Media | `screenshot` `annotate_screenshot` `save_pdf` |
| Tabs | `list_tabs` `new_tab` `switch_tab` `close_tab` `close` |
| State | `save_state` `restore_state` `get_cookies` `set_cookie` `clear_cookies` |
| Testing | `test_run` `test_assert_text` `test_assert_attr` `test_assert_url` `test_assert_visible` `test_report` |
| Network | `intercept_request` `get_network_log` `save_har` `get_console_logs` |
| Performance | `get_performance` `get_memory_usage` `set_user_agent` `set_geolocation` |
| Diagnostics | `server_status` `server_logs` `server_profile_list` `server_profile_set` `server_shutdown` |
| Protocol | `batch` `cdp_raw` |

`aero tools` prints the machine-readable inventory. `docs/tools.json` is generated from the same source as the server's schema, so the two cannot drift.

---

## Engines

| Engine | Platform | Isolation | Use |
|---|---|---|---|
| **Moli** (managed) | Linux x86_64 | Landlock + seccomp + cgroup v2 | Untrusted pages, production |
| **Chrome / Edge / Chromium** (external) | Any | The browser's own | Development, trusted pages |
| **Lightpanda** | Any | Unvalidated | Experimental; requires `--engine-path` |

```sh
aero --engine moli mcp                                    # managed, sandboxed
aero --engine external --cdp http://127.0.0.1:9222 mcp    # your own browser
```

---

## Test recipes

AeroBrowser includes a small DSL for browser tests that run against a real engine.

```
open https://example.com/login
wait_for #email
fill #email "user@example.com"
click #submit
assert_text .welcome "Welcome back"
assert_url "https://example.com/dashboard"
```

```sh
aero test examples/login.aero --report results.html
```

`results.html` is a self-contained report with a light/dark theme, per-step durations relative to the slowest step, expandable error detail, and no arguments logged — safe to attach to a review.

---

## Documentation

| Document | Contents |
|---|---|
| [Quick Start](docs/QUICKSTART.md) | First working setup |
| [Install](docs/INSTALL.md) | All platforms and agents |
| [API Reference](docs/API.md) | All 62 tools, schemas, DSL grammar |
| [Agent Skill](docs/AGENT-SKILL.md) | How an agent should drive AeroBrowser |
| [Architecture](docs/ARCHITECTURE.md) | System design |
| [Security](docs/SECURITY.md) | Trust boundaries and known limits |
| [Deployment](docs/DEPLOYMENT.md) | Daemon, cgroups, transports |
| [Roadmap](docs/ROADMAP.md) | Implemented vs. remaining, honestly |
| [Release notes 0.5](docs/RELEASE-0.5.md) | What changed |

---

## Contributing

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked --all-targets
python3 scripts/check-encoding.py     # reject mojibake
python3 scripts/check-docs.py         # verify links resolve
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

---

## License

Business Source License 1.1 with an Additional Use Grant. Free for personal, research, educational, and internal business use below USD 10M annual revenue. Production use above that threshold, and offering browser automation as a service, require a commercial license.

See [LICENSE](LICENSE) for the full terms.