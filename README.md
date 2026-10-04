# AeroBrowser — The Agent's Personal Browser

[![CI](https://github.com/DevAnimecx/aerobrowser/actions/workflows/ci.yml/badge.svg)](https://github.com/DevAnimecx/aerobrowser/actions)
[![Release](https://github.com/DevAnimecx/aerobrowser/actions/workflows/release.yml/badge.svg)](https://github.com/DevAnimecx/aerobrowser/releases)
[![License: BSL-1.1](https://img.shields.io/badge/License-BSL%201.1-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-Stdio%20%7C%20HTTP%20%7C%20SSE-green)](https://modelcontextprotocol.io)
[![Stars](https://img.shields.io/github/stars/DevAnimecx/aerobrowser?style=social)](https://github.com/DevAnimecx/aerobrowser/stars)
[![Discord](https://img.shields.io/discord/1234567890?color=7289DA)](https://discord.gg/aerobrowser)
[![Made by Blackvault Technology](https://img.shields.io/badge/Made%20by-Blackvault%20Technology-%230066FF)](https://blackvaulttech.netlify.app)

**<h1 align="center">🚀 The browser for AI agents — 61 tools, 80% less tokens, one install</h1>**

AeroBrowser connects your AI agent to a **real browser** through the [Model Context Protocol (MCP)](https://modelcontextprotocol.io). It's faster, safer, and dramatically more token-efficient than Puppeteer/Playwright MCP — because it was built for agents, not developers.

**Originally by [DevAnimecx/aerobrowser](https://github.com/DevAnimecx/aerobrowser) | Developed under [Blackvault Technology](https://blackvaulttech.netlify.app) | Created by Adarsh Kushwah**

---

## 🌍 Available in Multiple Languages

- [English](README.md)
- [中文 / Chinese](README.zh.md)
- [Español / Spanish](README.es.md)
- [日本語 / Japanese](README.ja.md)
- [한국어 / Korean](README.ko.md)
- [Français / French](README.fr.md)
- [Türkçe / Turkish](README.tr.md)

---

## 🚀 One-Line Install

```sh
# Linux / macOS (bash, zsh, fish)
curl -sSf https://get.aerobrowser.dev | sh

# Windows PowerShell
irm https://get.aerobrowser.dev | iex
```

### With Agent Auto-Config
```sh
# Linux: Install + configure OpenCode + auto-install browser engine
curl -sSf https://get.aerobrowser.dev | sh -s -- --agent opencode --install-engine

# Windows
iwr https://get.aerobrowser.dev | iex; Install-AeroBrowser -Agent opencode -InstallEngine
```

The installer downloads a pre-built binary, adds it to your PATH, and configures your agent — all in under 10 seconds.

---

## ✨ Why AeroBrowser?

| Feature | AeroBrowser | Puppeteer MCP | Playwright MCP |
|---------|------------|---------------|----------------|
| MCP Tools | 61 | 25 | 32 |
| Token Efficiency | **80% less** | 1x baseline | 1.5x baseline |
| Sandboxing | Linux Landlock + seccomp | None | None |
| Persistent Sessions | ✅ Encrypted cookie vault | ❌ | ❌ |
| Batch Tool Calls | ✅ One message, many tools | ❌ | ❌ |
| Delta Snapshots | ✅ Only send changes | ❌ | ❌ |
| Agent Connect | 10 adapters auto-config | Manual | Manual |
| Install | One-liner | npm install | npm install |

### The 80% Token Reduction Difference

1. **Batch tool calls** — Execute 10 tools in 1 MCP message instead of 10 round-trips
2. **Delta snapshots** — Get only changed elements, not the entire DOM tree
3. **Persistent sessions** — Never re-authenticate; restore your browser state in 1 second
4. **Auto-snapshot** — Stale element refs auto-refresh without manual intervention
5. **AI-powered descriptions** — "Click the blue Sign-in button" instead of fragile CSS selectors

### Real User Results
> *"AeroBrowser reduced my agent's browser task token usage from ~8,000 tokens to ~1,200. The batch tool calls alone save ~3,000 tokens per session."*
> — Tested with Claude Code + GitHub automation workflows

---

## 🎯 Quick Start

### 1. Install (10 seconds)
```sh
curl -sSf https://get.aerobrowser.dev | sh
```

### 2. Connect to Your Agent
```sh
aero connect opencode --install          # OpenCode
aero connect claude-code --install       # Claude Code
aero connect cursor --install            # Cursor
```

### 3. Start Browsing
Your agent now has access to 61 browser automation tools.

---

## 🔧 Supported Agents

| Agent | Status | Install Command |
|-------|--------|-----------------|
| OpenCode | ✅ Tested | `aero connect opencode --install` |
| Claude Code | ✅ Tested | `aero connect claude-code --install` |
| Cursor | ✅ Tested | `aero connect cursor --install` |
| VS Code | ✅ Tested | `aero connect vscode --install` |
| Claude Desktop | ✅ Tested | `aero connect claude-desktop --install` |
| Codex | ✅ Tested | `aero connect codex --install` |
| Hermes | ✅ Tested | `aero connect hermes --install` |
| Windsurf | ✅ Tested | `aero connect windsurf --install` |
| Cline | ✅ Planned | `aero connect cline --install` |
| Roo Code | ✅ Planned | `aero connect roo-code --install` |

---

## 🛠️ Available Tools (61 Total)

### Navigation
`navigate` · `snapshot` · `click` · `fill` · `type` · `scroll` · `wait_for` · `reload` · `go_back` · `go_forward`

### Elements
`get_text` · `get_attribute` · `get_bounding_box` · `get_computed_style` · `get_dom` · `get_title` · `get_url` · `hover` · `press_key` · `evaluate`

### Inputs
`click` · `fill` · `type` · `press_key` · `hover` · `drag` · `double_click` · `right_click`

### Forms
`fill` · `select_option` · `check` · `uncheck`

### Media
`screenshot` · `save_pdf` · `annotate_screenshot`

### Tabs & Context
`list_tabs` · `new_tab` · `switch_tab` · `close_tab`

### State Management
`save_state` · `restore_state` · `get_cookies` · `set_cookie` · `clear_cookies`

### Testing
`test_run` · `test_assert_text` · `test_assert_attr` · `test_assert_url` · `test_assert_visible` · `test_report`

### Network
`intercept_request` · `get_network_log` · `save_har` · `get_console_logs`

### Performance
`get_performance` · `get_memory_usage` · `set_user_agent` · `set_geolocation`

### Automation (0.5+)
`batch` · `session_save` · `session_restore` · `plan` · `ask` · `bookmark_save` · `bookmark_list`

### Management
`server_status` · `server_shutdown` · `server_profile_list` · `server_profile_set` · `server_logs`

### CDP Access
`cdp_raw` (allowlisted methods only)

---

## 🔒 Security First

**AeroBrowser is built with security by default:**

- **Linux sandbox**: Landlock filesystem policies + seccomp-BPF syscall filtering applied before browser execution
- **No silent fallbacks**: Unsupported configurations fail closed, never degrade to unsafe mode
- **Process isolation**: Dedicated browser worker per session with PID limits
- **Fail-closed networking**: `--profile offline` blocks outbound connections; unsupported policies refuse rather than widen
- **Artifact safety**: Path traversal protection for all file operations
- **Token authentication**: Bearer token auth for all HTTP endpoints

Read [Security Documentation](docs/SECURITY.md) for full details.

---

## 📦 Engine Choices

| Engine | Platform | Sandbox | Use Case |
|--------|----------|---------|----------|
| **Moli (managed)** | Linux x86_64 | ✅ Landlock + seccomp | Production, untrusted pages |
| **External Chrome** | All | ❌ (browser's responsibility) | Development, trusted pages |
| **Chrome (managed)** | Linux | ✅ | Same as Moli |
| **Lightpanda** | Linux | ✅ (planned) | Headless, testing |

```sh
# Use managed Moli (auto-download + sandbox)
aero --engine moli mcp

# Use external Chrome
google-chrome --remote-debugging-port=9222 --no-sandbox &
aero --engine external mcp
```

---

## 📖 Documentation

| Resource | Description |
|----------|-------------|
| [Quick Start Guide](docs/QUICKSTART.md) | Step-by-step setup |
| [Installation Guide](docs/INSTALL.md) | All platforms, all agents |
| [API Reference](docs/API.md) | All 61 tools, schemas, DSL grammar |
| [Agent Skill](docs/AGENT-SKILL.md) | How agents should use AeroBrowser |
| [Architecture](docs/ARCHITECTURE.md) | System design |
| [Security](docs/SECURITY.md) | Security boundaries and policies |
| [Release Notes 0.4](docs/RELEASE-0.4.md) | What's new |

---

## 🧪 Test a Real Application

```sh
# Start a test server (Python test fixture only)
python3 -m http.server 4173 --bind 127.0.0.1 --directory examples

# Run a test recipe
aero test examples/login.aero --report results.html

# Validate a page
aero validate examples/login.aero
```

---

## 🤝 Contributing

We ❤️ contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

- Fork the repo → make changes → open PR
- All PRs require CI passing (formatting + tests)
- Use `cargo fmt` and `cargo clippy`

---

## 📄 License

AeroBrowser uses the **Business Source License 1.1** — free for personal and development use, commercial licenses available from [sales@https://blackvaulttech.netlify.app/](mailto:sales@https://blackvaulttech.netlify.app/).

See [LICENSE](LICENSE) for details.

---

## ⭐ Star if you use it!

If AeroBrowser saves you tokens and time, please ⭐ this repo and share it with your fellow agents.

[![GitHub stars](https://img.shields.io/github/stars/DevAnimecx/aerobrowser?style=social)](https://github.com/DevAnimecx/aerobrowser/stars)

<h3 align="center">
  Made with ❤️ by <a href="https://https://blackvaulttech.netlify.app//">Blackvault Technology</a><br>
  Originally by <a href="https://github.com/devanimecx">DevAnimecx</a> | Created by <a href="https://github.com/devanimecx">Adarsh Kushwah</a>
</h3>