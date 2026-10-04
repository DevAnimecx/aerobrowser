<#
.SYNOPSIS
    AeroBrowser Windows installer

.DESCRIPTION
    Universal installer for AeroBrowser on Windows.
    Downloads the latest release binary and optionally configures an agent.

.PARAMETER InstallDir
    Installation directory (default: $HOME\.local\bin)

.PARAMETER Force
    Overwrite existing installation

.PARAMETER Agent
    Auto-configure for agent (claude-code, opencode, cursor, vscode, codex, hermes)

.PARAMETER NoProfile
    Skip PATH update

.PARAMETER InstallEngine
    Attempt to install Moli engine

.PARAMETER Help
    Show help

.EXAMPLE
    PS> curl -sSf https://get.aerobrowser.dev | powershell -Command -
.EXAMPLE
    PS> .\scripts\install.ps1 -Agent opencode -InstallEngine
#>
param(
    [string]$InstallDir = "$env:USERPROFILE\.local\bin",
    [switch]$Force,
    [string]$Agent,
    [switch]$NoProfile,
    [switch]$InstallEngine,
    [switch]$Help
)

$ErrorActionPreference = "Stop"
$REPO = "DevAnimecx/aerobrowser"
$BIN_NAME = "aero.exe"

function Write-Info($msg) { Write-Host "â„¹ $msg" -ForegroundColor Blue }
function Write-Ok($msg)   { Write-Host "âœ“ $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "âš  $msg" -ForegroundColor Yellow }
function Write-Fail($msg) { Write-Host "âœ— $msg" -ForegroundColor Red; exit 1 }

if ($Help) {
    Get-Help $MyInvocation.MyCommand -Detailed
    exit 0
}

# Detect platform
$IsArm = ([System.Runtime.InteropServices.RuntimeInformation]::ProcessorArchitecture -eq "Arm64")
if ($IsArm) {
    $PLATFORM = "windows-aarch64"
} else {
    $PLATFORM = "windows-x86_64"
}
Write-Info "Detected platform: $PLATFORM"

# Get latest version
$RELEASE_URL = "https://api.github.com/repos/$REPO/releases/latest"
try {
    $RELEASE = Invoke-RestMethod -Uri $RELEASE_URL -Headers @{"Accept" = "application/vnd.github.v3+json"}
    $VERSION = $RELEASE.tag_name
} catch {
    $VERSION = "v0.4.0"
    Write-Warn "Could not fetch latest version, using fallback: $VERSION"
}
Write-Info "Latest release: $VERSION"

# Determine asset
if ($PLATFORM -eq "windows-aarch64") {
    $ASSET = "aero-windows-aarch64.zip"
} else {
    $ASSET = "aero-windows-x86_64.zip"
}

# Check existing install
if (-not $Force -and (Test-Path "$InstallDir\$BIN_NAME")) {
    $EXISTING = & "$InstallDir\$BIN_NAME" --version 2>$null
    Write-Info "Existing installation: $EXISTING"
    Write-Info "Use -Force to overwrite"
    exit 0
}

# Create install dir
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

# Download
$DOWNLOAD_URL = "https://github.com/$REPO/releases/download/$VERSION/$ASSET"
$TEMP_ZIP = "$env:TEMP\aero-$VERSION.zip"
Write-Info "Downloading $ASSET..."

try {
    Invoke-WebRequest -Uri $DOWNLOAD_URL -OutFile $TEMP_ZIP -UseBasicParsing
    Expand-Archive -Path $TEMP_ZIP -DestinationPath $env:TEMP\aero-extract -Force
} catch {
    Write-Fail "Download failed: $_"
}

# Find and copy binary
$BINARY = Get-ChildItem "$env:TEMP\aero-extract" -Filter "aero*.exe" -Recurse | Select-Object -First 1
if ($BINARY) {
    Copy-Item $BINARY.FullName "$InstallDir\$BIN_NAME" -Force
    Remove-Item $TEMP_ZIP -Force -ErrorAction SilentlyContinue
    Remove-Item "$env:TEMP\aero-extract" -Recurse -Force -ErrorAction SilentlyContinue
    Write-Ok "Installed: $InstallDir\$BIN_NAME"
} else {
    Write-Fail "Could not find aero.exe in extracted archive"
}

# Update PATH (user scope)
if (-not $NoProfile) {
    $CURRENT_PATH = [Environment]::GetEnvironmentVariable("PATH", "User")
    if (-not $CURRENT_PATH.Contains($InstallDir)) {
        [Environment]::SetEnvironmentVariable("PATH", "$CURRENT_PATH;$InstallDir", "User")
        Write-Warn "Added $InstallDir to user PATH. Restart your terminal to apply."
    } else {
        Write-Ok "PATH already contains $InstallDir"
    }
}

# Install engine
if ($InstallEngine -and $PLATFORM -eq "windows-x86_64") {
    Write-Info "Moli engine requires Linux. Use external CDP browser on Windows."
    Write-Warn "Install Chrome with --remote-debugging-port=9222"
}

# Configure agent
if ($Agent) {
    Write-Info "Configuring for agent: $Agent"
    $AERO = "$InstallDir\AERO.exe"
    if (Test-Path $AERO) {
        & $AERO connect $Agent --install 2>&1
        Write-Ok "Agent '$Agent' configured!"
        Write-Info "Restart your agent to pick up the MCP server"
    }
}

Write-Host ""
Write-Ok "AeroBrowser installed successfully!"
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Start Chrome: chrome.exe --remote-debugging-port=9222 --no-sandbox"
Write-Host "  2. Run: aero mcp   (or 'aero start' for HTTP server)"
Write-Host "  3. Or:  aero connect --agent <name> --install"

