# Quickstart

## 1. Install AeroBrowser

### Linux / macOS
```sh
curl -sSf https://get.aerobrowser.dev | sh
```

### Windows (PowerShell)
```powershell
irm https://get.aerobrowser.dev | iex
```

Verify:
```sh
aero --version
```

## 2. Start a browser with CDP

### Linux
```sh
google-chrome --remote-debugging-port=9222 --no-sandbox --user-data-dir=/tmp/aero-chrome &
```

### macOS
```sh
/Applications/Google\ Chrome.app/Contents/MacOS/Google\ Chrome --remote-debugging-port=9222 --no-sandbox &
```

### Windows
```powershell
chrome.exe --remote-debugging-port=9222 --no-sandbox
```

## 3. Connect to your agent

### One-step connect (recommended)
```sh
aero connect opencode --install
```

This auto-detects or installs the engine, starts the daemon, and writes your agent config:
- Claude Code: `aero connect claude-code --install`
- Cursor: `aero connect cursor --install`
- VS Code: `aero connect vscode --install`
- Codex: `aero connect codex --install`
- Hermes: `aero connect hermes --install`

## 4. Start using MCP tools

Once your agent is configured and restarted, run:

```sh
aero mcp --transport stdio
```

Your agent will now have access to 61 MCP tools:

### Navigate
```
aero_navigate(url="https://example.com")
aero_snapshot()
aero_status()
```

### Interact
```
aero_click(selector="button.submit")
aero_fill(selector="#email", value="test@example.com")
aero_type(selector="#search", text="hello world")
```

### Extract
```
aero_get_text(selector="h1")
aero_get_attribute(selector="a", attr="href")
aero_extract({"title": "h1", "links": ["a"]})
```

### Media
```
aero_screenshot(path="page.png")
aero_screenshot(path="full.png", full_page=true)
aero_pdf(path="page.pdf")
```

### Testing
```
aero_test(path="tests/example.aero")
aero_validate(url="https://example.com", checks=[{"selector": "h1", "text_contains": "Example"}])
```

## 5. HTTP daemon mode (for scripting)

```sh
# Terminal 1: start the daemon
export AERO_TOKEN="$(openssl rand -hex 32)"
aero start --port 8931

# Terminal 2: use the CLI
aero open https://example.com
aero get text h1
aero screenshot page.png
```

## Quick Reference

### CLI Commands
| Command | Description |
|---------|-------------|
| `aero mcp` | Start MCP server (stdio/http/sse) |
| `aero start` | Start HTTP daemon with MCP endpoints |
| `aero open URL` | Navigate to URL |
| `aero click SELECTOR` | Click element |
| `aero fill SELECTOR VALUE` | Fill input |
| `aero get text SELECTOR` | Get element text |
| `aero screenshot PATH` | Take screenshot |
| `aero status` | Show browser/session status |
| `aero connect AGENT` | Configure agent |
| `aero profile install AGENT` | Generate config file |
| `aero test PATH` | Run test recipe |
| `aero validate URL` | Validate page |
| `aero engine install moli` | Install Moli engine |

### Environment Variables
| Variable | Default | Description |
|----------|---------|-------------|
| `AERO_TOKEN` | auto-generated | Authentication token for HTTP API |
| `AERO_CDP` | `http://127.0.0.1:9222` | CDP endpoint |
| `AERO_ENGINE` | `external` | Engine: external, moli, chrome, lightpanda |
| `AERO_SERVER` | `http://127.0.0.1:8931` | HTTP server address |
| `AERO_PROFILE` | `scrape` | Sandbox profile |

### Sandbox Profiles
| Profile | Network | Filesystem | Use case |
|---------|---------|------------|----------|
| `scrape` | Unrestricted | Read-only workspace | General browsing |
| `offline` | Blocked | Read-only workspace | Local content only |
| `full` | Unrestricted | Read-only workspace | Same as scrape |

## Troubleshooting

**MCP server doesn't start:**
- Check `aero --version` works
- Verify browser is running on port 9222: `curl http://127.0.0.1:9222/json/version`

**Agent can't connect:**
- Restart your agent after running `aero connect`
- Check `~/.aero/token.json` exists

**Sandbox errors on Linux:**
- Ensure kernel supports Landlock (5.13+)
- Check `aero sandbox probe`

**Moli engine not found:**
- Run `aero connect --install` on Linux x86_64
- Or use an external Chrome browser
