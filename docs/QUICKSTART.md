# Quickstart

## 1. Install

```sh
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/DevAnimecx/aerobrowser/master/scripts/install.sh | sh
```

```powershell
# Windows PowerShell
irm https://raw.githubusercontent.com/DevAnimecx/aerobrowser/master/scripts/install.ps1 | iex; Install-AeroBrowser
```

Both installers verify the published SHA-256 before installing, confirm the
download is a native executable, and remove it if it fails to run afterwards.

## 2. Verify the install

```sh
aero doctor
```

This checks the engine, sandbox support, ports, auth token, and agent config
paths, and prints the fix for anything that fails. Run `aero doctor --json` for
machine-readable output. If you are about to file a bug, include this output.

## 3. Choose an engine

### Managed, sandboxed (Linux x86_64 only)

```sh
aero connect opencode --install
```

Downloads the Moli engine and runs each session in a dedicated worker with
Landlock and seccomp applied. Nothing to start by hand.

### External browser (any platform)

Start a Chromium-family browser with a debugging port:

```sh
# Linux
google-chrome --remote-debugging-port=9222 --no-sandbox --user-data-dir=/tmp/aero-chrome &

# macOS
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --remote-debugging-port=9222 &

# Windows
chrome.exe --remote-debugging-port=9222
```

```sh
aero connect opencode --install
```

On Windows and macOS this adopts the browser it finds locally, since managed
engine downloads are Linux-only. The result reports which engine was chosen in
`engine` and explains the choice in `platform_note`.

## 4. Connect your agent

```sh
aero connect opencode        # or claude-code, cursor, vscode, codex, claude-desktop, ...
```

Then restart your agent. Run `aero profile list` to see every adapter.

## 5. Use the tools

Restart your agent and it will have 62 MCP tools available. Tool names are the
bare names, with no prefix:

```json
{"name": "navigate", "arguments": {"url": "https://example.com"}}
{"name": "snapshot", "arguments": {"interactive_only": true}}
{"name": "get_text", "arguments": {"selector": "h1"}}
{"name": "get_attribute", "arguments": {"selector": "a", "attribute": "href"}}
{"name": "screenshot", "arguments": {"path": "page.png"}}
{"name": "save_pdf", "arguments": {"path": "page.pdf"}}
```

To reduce round-trips, send several in one message:

```json
{"name": "batch", "arguments": {"calls": [
  {"name": "navigate", "arguments": {"url": "https://example.com/login"}},
  {"name": "snapshot", "arguments": {"interactive_only": true}},
  {"name": "fill", "arguments": {"ref": "@e1_4", "value": "user@example.com"}},
  {"name": "click", "arguments": {"ref": "@e1_5"}}
]}}
```

To send only what changed since the last snapshot:

```json
{"name": "snapshot", "arguments": {"delta": true}}
```

`aero tools` prints the full inventory. [API.md](API.md) documents every schema.

## 6. Verify a recipe

```sh
python3 -m http.server 4173 --bind 127.0.0.1 --directory examples   # fixture server
aero validate examples/login.aero
aero test examples/login.aero --report results.html
```

`aero validate` takes a `.aero` recipe file, not a URL. It parses the recipe and
reports the step count without touching a browser.

---

## Reference

### CLI

| Command | Description |
|---|---|
| `aero doctor` | Diagnose the install; `--json` for machine output |
| `aero tools` | Print the tool inventory as JSON |
| `aero mcp` | MCP server over stdio, HTTP, or SSE |
| `aero start` | HTTP daemon with MCP, SSE, and CLI endpoints |
| `aero stop` | Shut down the daemon |
| `aero connect AGENT` | Configure an agent (`--install` to fetch an engine) |
| `aero profile list` | List sandbox profiles and whether each is implemented |
| `aero open URL` | Navigate |
| `aero snapshot` | Accessibility snapshot (`-i` for interactive only) |
| `aero click TARGET` | Click; `TARGET` is `@ref` or a CSS selector |
| `aero fill TARGET VALUE` | Fill a field |
| `aero eval SCRIPT` | Run JavaScript in the page |
| `aero screenshot PATH` | Capture (`--full-page` for the full document) |
| `aero test PATH` | Run a `.aero` recipe (`--report` for HTML or JSON) |
| `aero validate PATH` | Parse a recipe and report its steps |
| `aero engine install moli` | Download the managed engine (Linux x86_64) |
| `aero sandbox probe` | Report sandbox support on this host |
| `aero config` | Print an MCP server config snippet |

Most commands also accept shorthand verbs: `get text`, `get url`, `back`,
`forward`, `reload`, `tabs`, `new-tab`, `press`, `wait`, `scroll`, `cookies`,
`cdp`, `save-state`.

### Environment

| Variable | Default | Purpose |
|---|---|---|
| `AERO_TOKEN` | auto-generated | Bearer token for the HTTP API. Minimum 24 characters. |
| `AERO_TOKEN_FILE` | `~/.aero/token.json` | Where the generated token is stored |
| `AERO_CDP` | `http://127.0.0.1:9222` | External CDP endpoint |
| `AERO_ENGINE` | `external` | `external`, `moli`, `chrome`, or `lightpanda` |
| `AERO_ENGINE_PATH` | — | Explicit browser or engine executable |
| `AERO_SERVER` | `http://127.0.0.1:8931` | Daemon address |
| `AERO_PROFILE` | `scrape` | Sandbox profile |
| `AERO_MEMORY_LIMIT_MB` | unlimited | Worker memory ceiling (cgroup v2, Linux) |
| `AERO_PIDS_LIMIT` | unlimited | Worker process ceiling (cgroup v2, Linux) |
| `AERO_REQUEST_TIMEOUT_MS` | `120000` | Per-request deadline, 100–900000 |
| `AERO_AUTH_FILE` | — | Principals file for multiple identities |
| `AERO_UPLOAD_ROOT` | — | Directory `upload_file` is restricted to |

`AERO_ENGINE` defaults to `moli` only in builds with the `bundled-moli` feature;
published binaries default to `external`.

### Sandbox profiles

| Profile | Implemented | Network | Filesystem |
|---|---|---|---|
| `scrape` | Yes | Unrestricted | Runtime read, private worker write |
| `offline` | Yes | Outbound connect and datagram send denied | Runtime read, private worker write |
| `untrusted` | Yes | Same as `offline` | Same as `offline` |
| `full` | Yes | Unrestricted | Same as `scrape`; no host write |
| `base` | **No** | — | Refuses to run |
| `web-dev` | **No** | — | Refuses to run |
| `test` | **No** | — | Refuses to run |

`aero profile list` is the authoritative source. There is no hostname or IP
allowlist: `offline` is all-or-nothing.

---

## Troubleshooting

**`aero doctor` shows engine_detected failing.** On Linux x86_64, run
`aero engine install moli`. Elsewhere, start a browser with
`--remote-debugging-port=9222` and re-run `aero connect <agent> --install`.

**The agent does not list AeroBrowser's tools.** Restart it after
`aero connect`. Check that the config file it reads is the one `aero connect`
wrote — `aero doctor` lists the paths it checks.

**`AERO_TOKEN must contain at least 24 characters`.** The generated token is 44
characters, so this means a token was set by hand. Use at least 24, or delete the
token file to have one generated.

**Sandbox errors on Linux.** Landlock requires kernel 5.13+. Run
`aero sandbox probe`; it reports what this kernel supports.

**`sandbox: {"supported": false}`.** Expected on Windows and unvalidated on
macOS. Use an external browser and treat containment as its responsibility.

**The daemon returns 503.** The session actor stopped. Run `aero start`.

**A ref is rejected as stale.** Refs are invalidated by navigation and tab
switches. Take a fresh `snapshot`.