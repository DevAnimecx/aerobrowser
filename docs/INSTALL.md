# Installation Guide

AeroBrowser is available as pre-built binaries for Linux, macOS, and Windows, or from source.

## Prerequisites

### All platforms
- A browser supporting Chrome DevTools Protocol (CDP): Chrome, Edge, Brave, or Chromium
- Port 9222 free on localhost (for browser remote debugging)

### Linux (recommended for managed Moli engine)
- Linux kernel with Landlock (ABI >= 2, kernel 5.13+)
- glibc 2.31+ (Ubuntu 20.04+, Debian 11+, etc.)
- OpenSSL 3.x
- Fontconfig

### macOS
- macOS 12.0+ (Monterey)
- No kernel sandbox available; managed Moli not supported. Use external Chrome.

### Windows
- Windows 10/11 (x86_64 or ARM64)
- No kernel sandbox available; managed Moli not supported. Use external Chrome.

## Quick Install

### Linux / macOS (one-liner)
```sh
curl -sSf https://get.aerobrowser.dev | sh
```

### Windows (PowerShell)
```powershell
irm https://get.aerobrowser.dev | iex
```

### With agent auto-config
```sh
# Linux: install + configure opencode + auto-install Moli engine
curl -sSf https://get.aerobrowser.dev | sh -s -- --agent opencode --install-engine

# Windows PowerShell
iwr https://get.aerobrowser.dev | iex; Install-AeroBrowser -Agent opencode
```

## Install Options

| Flag | Description |
|------|-------------|
| `--dir DIR` | Install directory (default: `$HOME/.local/bin`) |
| `--force` | Overwrite existing installation |
| `--agent AGENT` | Auto-configure for agent (claude-code, opencode, cursor, vscode, codex, hermes) |
| `--no-profile` | Skip adding to PATH profile |
| `--install-engine` | Auto-install Moli engine (Linux x86_64 only) |
| `--help` | Show all options |

## Platform-Specific Installation

### Linux

#### Option 1: One-liner (recommended)
```sh
curl -sSf https://get.aerobrowser.dev | sh -- --install-engine
```

#### Option 2: From GitHub releases
```sh
# Download latest binary
curl -L https://github.com/DevAnimecx/aerobrowser/releases/latest/download/aero-linux-x86_64 -o aero
chmod +x aero
sudo mv aero /usr/local/bin/aero
```

#### Option 3: Build from source
```sh
git clone https://github.com/DevAnimecx/aerobrowser.git
cd aerobrowser
cargo build --release --locked
sudo cp target/release/aero /usr/local/bin/aero
```

### macOS

#### Option 1: One-liner
```sh
curl -sSf https://get.aerobrowser.dev | sh
```

#### Option 2: Homebrew (when available)
```sh
brew install DevAnimecx/aerobrowser/aero
```

#### Option 3: Build from source
```sh
git clone https://github.com/DevAnimecx/aerobrowser.git
cd aerobrowser
cargo build --release --locked
sudo cp target/release/aero /usr/local/bin/aero
```

### Windows

#### Option 1: PowerShell one-liner
```powershell
irm https://get.aerobrowser.dev | iex
```

#### Option 2: Manual download
1. Download `aero-windows-x86_64.zip` from [GitHub releases](https://github.com/DevAnimecx/aerobrowser/releases/latest)
2. Extract `aero.exe`
3. Add to PATH:
   ```powershell
   [Environment]::SetEnvironmentVariable("PATH", $env:PATH + ";C:\path\to\aero", "User")
   ```

#### Option 3: Build from source
```powershell
git clone https://github.com/DevAnimecx/aerobrowser.git
cd aerobrowser
cargo build --release --locked
# Binary: target\release\aero.exe
```

## Verify Installation

```sh
aero --version
aero --help
```

Expected output:
```
aero 0.4.0
Agent browser CLI and MCP server with managed engines and explicit sandbox profiles.
```

## Next Steps

After installation:

1. **Start a browser with CDP debugging port**:
   ```sh
   google-chrome --remote-debugging-port=9222 --no-sandbox
   ```

2. **Test the connection**:
   ```sh
   aero status
   ```

3. **Connect to an agent**:
   ```sh
   aero connect opencode --install
   ```

See [Quickstart](QUICKSTART.md) for detailed next steps.

