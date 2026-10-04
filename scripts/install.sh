#!/usr/bin/env bash
set -euo pipefail

# AeroBrowser universal installer
# Usage: curl -sSf https://get.aerobrowser.dev | sh
#   Or:  sh scripts/install.sh

REPO="DevAnimecx/aerobrowser"
INSTALL_DIR="${AERO_INSTALL_DIR:-$HOME/.local/bin}"
BIN_NAME="aero"
FORCE=0
NO_PROFILE=0
AGENT=""
AUTO_INSTALL_ENGINE=0

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info()  { printf "${BLUE}â„¹${NC} %s\n" "$*"; }
ok()    { printf "${GREEN}âœ“${NC} %s\n" "$*"; }
warn()  { printf "${YELLOW}âš ${NC} %s\n" "$*"; }
fail()  { printf "${RED}âœ—${NC} %s\n" "$*" >&2; exit 1; }

usage() {
    cat <<EOF
AeroBrowser installer

Usage: curl -sSf https://get.aerobrowser.dev | sh [OPTIONS]

Options:
  --dir DIR           Install directory (default: \$HOME/.local/bin)
  --force             Overwrite existing installation
  --agent AGENT       Auto-configure for agent (claude-code, opencode, cursor, vscode, codex, hermes)
  --no-profile        Skip adding ~/.local/bin to PATH
  --install-engine    Auto-install Moli engine after binary install
  --help              Show this help

Examples:
  curl -sSf https://get.aerobrowser.dev | sh
  curl -sSf https://get.aerobrowser.dev | sh -s -- --agent opencode
  curl -sSf https://get.aerobrowser.dev | sh -s -- --agent opencode --install-engine
EOF
    exit "${1:-0}"
}

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dir)      INSTALL_DIR="$2"; shift 2 ;;
        --force)    FORCE=1; shift ;;
        --agent)    AGENT="$2"; shift 2 ;;
        --no-profile) NO_PROFILE=1; shift ;;
        --install-engine) AUTO_INSTALL_ENGINE=1; shift ;;
        --help|-h)  usage 0 ;;
        *) fail "Unknown option: $1" ;;
    esac
done

# Detect platform
detect_platform() {
    local os arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m | tr '[:upper:]' '[:lower]')"

    case "$os" in
        linux*)  os="linux" ;;
        darwin*) os="macos" ;;
        msys*|cygwin*|mingw*) os="windows" ;;
        *) fail "Unsupported OS: $(uname -s)" ;;
    esac

    case "$arch" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) fail "Unsupported architecture: $arch" ;;
    esac

    PLATFORM="${os}-${arch}"
    info "Detected platform: $PLATFORM"
}

# Get latest release version
get_latest_version() {
    if command -v curl >/dev/null 2>&1; then
        VERSION=$(curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" | \
            grep -oP '"tag_name"\s*:\s*"[^"]*"' | head -1 | grep -oP 'v[^"]*' || true)
    elif command -v wget >/dev/null 2>&1; then
        VERSION=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | \
            grep -oP '"tag_name"\s*:\s*"[^"]*"' | head -1 | grep -oP 'v[^"]*' || true)
    else
        fail "Need curl or wget to download"
    fi

    if [[ -z "$VERSION" ]]; then
        VERSION="v0.5.0"
        warn "Could not fetch latest version, using fallback: $VERSION"
    fi
    info "Latest release: $VERSION"
}

# Determine asset name for platform
get_asset_name() {
    case "$PLATFORM" in
        linux-x86_64)  ASSET="aero-linux-x86_64" ;;
        linux-aarch64) ASSET="aero-linux-aarch64" ;;
        macos-x86_64)  ASSET="aero-macos-x86_64" ;;
        macos-aarch64) ASSET="aero-macos-aarch64" ;;
        windows-x86_64) ASSET="aero-windows-x86_64.zip" ;;
        windows-aarch64) ASSET="aero-windows-aarch64.zip" ;;
        *) fail "Platform not supported: $PLATFORM (Moli engine requires Linux x86_64)" ;;
    esac
}

# Download and install
install_binary() {
    mkdir -p "$INSTALL_DIR"

    if [[ "$FORCE" -eq 0 && -x "$INSTALL_DIR/$BIN_NAME" ]]; then
        local existing_version
        existing_version=$("$INSTALL_DIR/$BIN_NAME" --version 2>/dev/null || echo "unknown")
        info "Existing installation: $existing_version"
        info "Use --force to overwrite"
        return 0
    fi

    local tmp_dir
    tmp_dir=$(mktemp -d)
    trap "rm -rf $tmp_dir" EXIT

    local download_url="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}"
    local tmp_file="$tmp_dir/$ASSET"

    info "Downloading $ASSET from $download_url"

    if [[ "$ASSET" == *.zip ]]; then
        if command -v curl >/dev/null 2>&1; then
            curl -sSL -o "$tmp_file" "$download_url"
        else
            wget -qO "$tmp_file" "$download_url"
        fi
        (cd "$tmp_dir" && unzip -o "$ASSET" >/dev/null 2>&1 || 7z x "$ASSET" >/dev/null 2>&1 || fail "Could not extract zip")
        local binary_name
        binary_name=$(find "$tmp_dir" -name "aero*" -type f | head -1)
        cp "$binary_name" "$INSTALL_DIR/$BIN_NAME"
    else
        if command -v curl >/dev/null 2>&1; then
            curl -sSL -o "$tmp_file" "$download_url"
        else
            wget -qO "$tmp_file" "$download_url"
        fi
        cp "$tmp_file" "$INSTALL_DIR/$BIN_NAME"
    fi

    chmod +x "$INSTALL_DIR/$BIN_NAME"
    ok "Installed: $INSTALL_DIR/$BIN_NAME"
}

# Update PATH profile
update_profile() {
    if [[ "$NO_PROFILE" -eq 1 ]]; then
        return 0
    fi

    local profile=""
    if [[ "$SHELL" == */zsh ]]; then
        profile="$HOME/.zshrc"
    elif [[ "$SHELL" == */fish ]]; then
        profile="$HOME/.config/fish/config.fish"
    else
        profile="$HOME/.bashrc"
        [[ -f "$profile" ]] || profile="$HOME/.bash_profile"
    fi

    if [[ -n "$profile" && -f "$profile" ]]; then
        if ! grep -qF "$INSTALL_DIR" "$profile" 2>/dev/null; then
            echo "" >> "$profile"
            echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$profile"
            warn "Added $INSTALL_DIR to PATH in $profile"
            warn "Run: source $profile"
        else
            ok "PATH already configured in $profile"
        fi
    fi
}

# Configure agent
configure_agent() {
    if [[ -z "$AGENT" ]]; then
        return 0
    fi

    info "Configuring for agent: $AGENT"

    # Ensure binary is available
    local aero="$INSTALL_DIR/$BIN_NAME"

    if [[ "$AUTO_INSTALL_ENGINE" -eq 1 && "$AGENT" != "generic" ]]; then
        info "Auto-installing Moli engine..."
        "$aero" engine install moli 2>&1 || warn "Moli install failed, will use external CDP"
    fi

    "$aero" connect "$AGENT" --install 2>&1 || {
        info "Falling back to manual profile install..."
        "$aero" profile install "$AGENT" --engine auto
    }

    ok "Agent '$AGENT' configured!"
    info "Restart your agent to pick up the MCP server"
}

# Auto-install engine
install_engine() {
    if [[ "$AUTO_INSTALL_ENGINE" -eq 0 ]]; then
        return 0
    fi

    local aero="$INSTALL_DIR/$BIN_NAME"
    info "Installing Moli engine..."

    if [[ "$PLATFORM" == linux-x86_64 ]]; then
        "$aero" engine install moli 2>&1
        ok "Moli engine installed."
    else
        warn "Moli engine requires Linux x86_64. Install an external browser with --remote-debugging-port=9222"
    fi
}

# Main
main() {
    info "AeroBrowser installer starting..."
    detect_platform
    get_latest_version
    get_asset_name
    install_binary
    update_profile
    install_engine
    configure_agent

    echo ""
    ok "AeroBrowser installed successfully!"
    echo ""
    cat <<EOF
Next steps:
  1. Ensure binary is in PATH: export PATH="$INSTALL_DIR:\$PATH"
  2. Start a browser with CDP: chrome --remote-debugging-port=9222 --no-sandbox &
  3. Run: aero mcp   (or 'aero start' for HTTP server)
  4. Or:  aero connect --agent <agent-name> --install
EOF
}

main

