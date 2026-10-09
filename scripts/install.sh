#!/usr/bin/env bash
# AeroBrowser installer for Linux and macOS.
#
#   curl -fsSL https://raw.githubusercontent.com/DevAnimecx/aerobrowser/master/scripts/install.sh | sh
#
# Options:
#   --dir DIR           Install directory (default: $HOME/.local/bin)
#   --version VERSION   Install a specific tag (default: latest release)
#   --force             Overwrite an existing installation
#   --agent AGENT       Also configure an agent (opencode, claude-code, cursor, ...)
#   --install-engine    Download the managed Moli engine (Linux x86_64 only)
#   --no-profile        Do not edit your shell profile
#   --no-verify         Skip SHA-256 verification (not recommended)
set -euo pipefail

REPO="DevAnimecx/aerobrowser"
RAW_BASE="https://raw.githubusercontent.com/${REPO}"
INSTALL_DIR="${AERO_INSTALL_DIR:-${HOME}/.local/bin}"
BIN_NAME="aero"
FORCE=0
NO_PROFILE=0
NO_VERIFY=0
AGENT=""
VERSION=""
AUTO_INSTALL_ENGINE=0
ENGINE_INSTALLED=0

RED=''
GREEN=''
YELLOW=''
BLUE=''
DIM=''
BOLD=''
NC=''
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    RED=$'\033[0;31m'
    GREEN=$'\033[0;32m'
    YELLOW=$'\033[1;33m'
    BLUE=$'\033[0;34m'
    DIM=$'\033[2m'
    BOLD=$'\033[1m'
    NC=$'\033[0m'
fi

step() { printf "${BLUE}==>%s${NC} %s\n" "$*"; }
info()  { printf "${BLUE}i${NC} %s\n" "$*"; }
ok()    { printf "${GREEN}v${NC} %s\n" "$*"; }
warn()  { printf "${YELLOW}!${NC} %s\n" "$*"; }
fail()  { printf "${RED}x${NC} %s\n" "$*" >&2; exit 1; }
dim()   { printf "${DIM}%s${NC}\n" "$*"; }

usage() {
    cat <<EOF
AeroBrowser installer

  --dir DIR           Install directory (default: \$HOME/.local/bin)
  --version VERSION   Install a specific tag, e.g. v0.5.0 (default: latest release)
  --force             Overwrite an existing installation
  --agent AGENT       Also configure an agent
  --install-engine    Download the managed Moli engine (Linux x86_64 only)
  --no-profile        Do not edit your shell profile
  --no-verify         Skip SHA-256 verification (not recommended)
  --help              Show this help

Agents: opencode, claude-code, claude-desktop, cursor, vscode, codex,
        windsurf, cline, roo-code, hermes, generic
EOF
    exit "${1:-0}"
}

while [ $# -gt 0 ]; do
    case "$1" in
        --dir)           INSTALL_DIR="$2"; shift 2 ;;
        --version)       VERSION="$2"; shift 2 ;;
        --force)         FORCE=1; shift ;;
        --agent)         AGENT="$2"; shift 2 ;;
        --install-engine) AUTO_INSTALL_ENGINE=1; shift ;;
        --no-profile)    NO_PROFILE=1; shift ;;
        --no-verify)     NO_VERIFY=1; shift ;;
        --help|-h)       usage 0 ;;
        *) fail "Unknown option: $1 (try --help)" ;;
    esac
done

TMP_DIR=""
cleanup() {
    if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
        rm -rf "$TMP_DIR"
    fi
}
trap cleanup EXIT INT TERM

# Downloads to stdout, failing loudly on any HTTP error so a 404 page can never
# be mistaken for a payload.
fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 --retry-delay 1 --proto '=https' --tlsv1.2 "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "$1"
    else
        fail "Need curl or wget to download"
    fi
}

fetch_to() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 --retry-delay 1 --proto '=https' --tlsv1.2 -o "$2" "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$2" "$1"
    else
        fail "Need curl or wget to download"
    fi
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        return 1
    fi
}

detect_platform() {
    local os arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m | tr '[:upper:]' '[:lower:]')"

    case "$os" in
        linux*)  os="linux" ;;
        darwin*) os="macos" ;;
        *) fail "Unsupported OS: $(uname -s). Use the PowerShell installer on Windows." ;;
    esac

    case "$arch" in
        x86_64|amd64) arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *) fail "Unsupported architecture: $arch" ;;
    esac

    PLATFORM="${os}-${arch}"
    info "Platform: $PLATFORM"
}

resolve_version() {
    if [ -n "$VERSION" ]; then
        info "Version: $VERSION (requested)"
        return 0
    fi
    # sed -E rather than grep -P: BSD grep on macOS has no -P support.
    local tag
    tag=$(fetch "https://api.github.com/repos/${REPO}/releases/latest" \
          | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    if [ -z "$tag" ]; then
        fail "Could not determine the latest release. Pass --version to pin one explicitly."
    fi
    VERSION="$tag"
    info "Latest release: $VERSION"
}

asset_name() {
    case "$PLATFORM" in
        linux-x86_64)   ASSET="aero-linux-x86_64" ;;
        linux-aarch64)  ASSET="aero-linux-aarch64" ;;
        macos-x86_64)   ASSET="aero-macos-x86_64" ;;
        macos-aarch64)  ASSET="aero-macos-aarch64" ;;
        *) fail "No published binary for $PLATFORM. Build from source: cargo build --release --locked" ;;
    esac
}

# Confirms the payload is a native executable before it ever reaches PATH. A
# 404 HTML body saved as `aero` would otherwise install and then fail confusingly.
looks_like_binary() {
    local file="$1"
    local magic
    magic=$(dd if="$file" bs=4 count=1 2>/dev/null | od -An -tx1 | tr -d ' \n')
    case "$magic" in
        7f454c46*) return 0 ;;                        # ELF
        cffaedfe*|cefaedfe*|feedface*) return 0 ;;     # Mach-O
        4d5a*) return 0 ;;                            # PE
        *) return 1 ;;
    esac
}

verify_checksum() {
    local file="$1" expected="$2" name="$3"
    if [ "$NO_VERIFY" -eq 1 ]; then
        warn "Checksum verification skipped (--no-verify)"
        return 0
    fi
    local sums=""
    if ! sums=$(fetch "https://github.com/${REPO}/releases/download/${VERSION}/SHA256SUMS" 2>/dev/null); then
        warn "SHA256SUMS not published for ${VERSION}; cannot verify ${name}"
        return 0
    fi
    local expected_sum actual_sum
    expected_sum=$(printf '%s\n' "$sums" \
        | sed -n "s/^\([0-9a-fA-F]\{64\}\)[ *]\{1,2\}${name}\$/\1/p" | head -1)
    if [ -z "$expected_sum" ]; then
        warn "${name} is not listed in SHA256SUMS; cannot verify"
        return 0
    fi
    actual_sum=$(sha256_of "$file") || { warn "No sha256 tool available; cannot verify"; return 0; }
    if [ "$expected_sum" != "$actual_sum" ]; then
        fail "Checksum mismatch for ${name}.
  expected: ${expected_sum}
  actual:   ${actual_sum}
The download was not installed."
    fi
    ok "Checksum verified"
    dim "  $actual_sum  $name"
}

install_engine() {
    [ "$AUTO_INSTALL_ENGINE" -eq 1 ] || return 0
    [ "$ENGINE_INSTALLED" -eq 1 ] && return 0

    local aero="$INSTALL_DIR/$BIN_NAME"
    if [ "$PLATFORM" != "linux-x86_64" ]; then
        warn "Managed Moli engine downloads are Linux x86_64 only."
        info "On $PLATFORM, start a local browser instead:"
        info "  chrome --remote-debugging-port=9222 &"
        return 0
    fi
    info "Installing the managed Moli engine..."
    if "$aero" engine install moli; then
        ENGINE_INSTALLED=1
        ok "Managed engine installed"
    else
        warn "Engine install failed; AeroBrowser will fall back to an external browser."
    fi
}

install_binary() {
    local aero="$INSTALL_DIR/$BIN_NAME"

    if [ "$FORCE" -eq 0 ] && [ -x "$aero" ]; then
        local existing
        existing=$("$aero" --version 2>/dev/null || echo "unknown")
        ok "Already installed: $existing"
        info "Location: $aero"
        info "Re-run with --force to upgrade."
        return 0
    fi

    TMP_DIR=$(mktemp -d)
    chmod 700 "$TMP_DIR"
    mkdir -p "$INSTALL_DIR"

    local url="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}"
    local payload="$TMP_DIR/$ASSET"

    info "Downloading ${ASSET}"
    fetch_to "$url" "$payload" || fail "Download failed: $url"

    verify_checksum "$payload" "" "$ASSET"

    if ! looks_like_binary "$payload"; then
        fail "$ASSET is not a native executable. The download was not installed."
    fi

    # Stage inside the destination directory so the final move is atomic and an
    # interrupted copy never leaves a truncated binary on PATH.
    local staged="$INSTALL_DIR/.${BIN_NAME}.new.$$"
    cp "$payload" "$staged"
    chmod 755 "$staged"
    mv -f "$staged" "$aero"

    if ! "$aero" --version >/dev/null 2>&1; then
        rm -f "$aero"
        fail "Installed binary did not run. Removing it."
    fi

    ok "Installed $("$aero" --version)"
    dim "  $aero"
}

update_profile() {
    [ "$NO_PROFILE" -eq 1 ] && return 0
    [ -n "${SHELL:-}" ] || return 0

    local profile
    case "$SHELL" in
        */zsh)  profile="$HOME/.zshrc" ;;
        */fish) return 0 ;;  # fish uses a different variable; leave it alone
        *)      profile="$HOME/.bashrc"; [ -f "$profile" ] || profile="$HOME/.bash_profile" ;;
    esac

    [ -f "$profile" ] || return 0
    if grep -qF "$INSTALL_DIR" "$profile" 2>/dev/null; then
        dim "PATH already includes $INSTALL_DIR in $profile"
        return 0
    fi
    {
        printf '\n'
        printf '# AeroBrowser\n'
        printf 'export PATH="%s:$PATH"\n' "$INSTALL_DIR"
    } >> "$profile"
    warn "Added $INSTALL_DIR to PATH in $profile"
    info "Run: source $profile"
}

configure_agent() {
    [ -n "$AGENT" ] || return 0
    local aero="$INSTALL_DIR/$BIN_NAME"
    step "Configuring agent: $AGENT"

    if ! "$aero" connect "$AGENT" --install; then
        fail "Could not configure agent '$AGENT'.
Fix the problems reported above, or run:
  $aero doctor"
    fi
    ok "Agent '$AGENT' configured"
}

summary() {
    local aero="$INSTALL_DIR/$BIN_NAME"
    echo
    ok "AeroBrowser is ready."
    echo
    printf "  %sCheck the install:  %s doctor\n" "${BOLD}" "$aero"
    if [ "$PLATFORM" = "linux-x86_64" ] && [ "$ENGINE_INSTALLED" -eq 1 ]; then
        printf "  %sConnect an agent:   %s connect opencode\n" "${BOLD}" "$aero"
    elif [ "$AGENT" != "" ]; then
        printf "  %sRestart your agent to load the MCP server.\n" "${BOLD}"
    else
        printf "  %sManaged browsing:   %s connect opencode --install\n" "${BOLD}" "$aero"
        printf "  %s  or start a browser: chrome --remote-debugging-port=9222 &\n" "${DIM}"
    fi
    echo
}

main() {
    printf "${BOLD}AeroBrowser installer${NC}\n"
    detect_platform
    resolve_version
    asset_name
    install_binary
    update_profile
    install_engine
    configure_agent
    summary
}

main