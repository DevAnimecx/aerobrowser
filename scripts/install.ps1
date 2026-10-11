<#
.SYNOPSIS
    DeltaBrowser installer for Windows.

.DESCRIPTION
    Downloads the published release binary, verifies its SHA-256 digest, installs
    it to a user-writable directory, and optionally configures an agent.

.PARAMETER InstallDir
    Installation directory. Default: $env:USERPROFILE\.local\bin

.PARAMETER Version
    Install a specific tag, e.g. v0.0.1. Default: latest release.

.PARAMETER Force
    Overwrite an existing installation.

.PARAMETER Agent
    Configure an agent: opencode, claude-code, claude-desktop, cursor, vscode,
    codex, windsurf, cline, roo-code, hermes, generic.

.PARAMETER NoProfile
    Do not modify the user PATH.

.PARAMETER InstallEngine
    Request the managed engine. Managed Moli downloads are Linux x86_64 only, so
    on Windows the installer adopts a locally installed browser instead.

.PARAMETER NoVerify
    Skip SHA-256 verification. Not recommended.

.EXAMPLE
    PS> .\scripts\install.ps1 -Agent opencode

.EXAMPLE
    PS> irm https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.ps1 | iex; Install-DeltaBrowser -Agent opencode
#>
param(
    [string]$InstallDir,
    [string]$Version,
    [switch]$Force,
    [string]$Agent,
    [switch]$NoProfile,
    [switch]$InstallEngine,
    [switch]$NoVerify,
    [switch]$Help
)

$ErrorActionPreference = "Stop"
# Windows PowerShell 5.1 defaults to a protocol set that can exclude TLS 1.2,
# which makes every request below fail with "Could not create SSL/TLS secure
# channel" on stock systems.
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
# Invoke-WebRequest's progress record can overflow the 5.1 pipeline and abort
# large downloads.
$ProgressPreference = 'SilentlyContinue'

$REPO = "DevAnimecx/deltabrowser"
$BIN_NAME = "delta.exe"

function Write-Step($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }
function Write-Info($msg) { Write-Host "  $msg" -ForegroundColor Blue }
function Write-Ok($msg)   { Write-Host "  $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "  $msg" -ForegroundColor Yellow }
function Write-Fail($msg) { Write-Host "  $msg" -ForegroundColor Red; exit 1 }

if ($Help) {
    Get-Help $MyInvocation.MyCommand -Detailed
    exit 0
}

if (-not $InstallDir) {
    $profile = if ($env:USERPROFILE) { $env:USERPROFILE } else { $HOME }
    $InstallDir = Join-Path $profile ".local\bin"
}

$script:TempDir = $null

function Initialize-DeltaBrowser {
    <#
    .SYNOPSIS
        Installs DeltaBrowser. This is the entry point used by the pipe-to-shell
        form, where the script body runs once with default parameters.
    #>
    param(
        [string]$InstallDir,
        [string]$Version,
        [switch]$Force,
        [string]$Agent,
        [switch]$NoProfile,
        [switch]$InstallEngine,
        [switch]$NoVerify
    )

    $ErrorActionPreference = "Stop"
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $ProgressPreference = 'SilentlyContinue'

    $REPO = "DevAnimecx/deltabrowser"
    $BIN_NAME = "delta.exe"

    if (-not $InstallDir) {
        $profile = if ($env:USERPROFILE) { $env:USERPROFILE } else { $HOME }
        $InstallDir = Join-Path $profile ".local\bin"
    }

    $DELTA = Join-Path $InstallDir $BIN_NAME

    Write-Host ""
    Write-Host "DeltaBrowser installer" -ForegroundColor White
    Write-Host ""

    # --- Platform -----------------------------------------------------------
    $arch = $env:PROCESSOR_ARCHITECTURE
    if (-not $arch) {
        try {
            $arch = [System.Runtime.InteropServices.RuntimeInformation]::ProcessorArchitecture.ToString()
        } catch {
            $arch = "X64"
        }
    }
    if ($arch -match "ARM") {
        $PLATFORM = "windows-aarch64"
        $ASSET = "delta-windows-aarch64.zip"
    } else {
        $PLATFORM = "windows-x86_64"
        $ASSET = "delta-windows-x86_64.zip"
    }
    Write-Info "Platform: $PLATFORM"

    # --- Version ------------------------------------------------------------
    if (-not $Version) {
        try {
            $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$REPO/releases/latest" `
                -Headers @{ "Accept" = "application/vnd.github+json"; "User-Agent" = "deltabrowser-installer" }
            $Version = $release.tag_name
        } catch {
            Write-Fail "Could not determine the latest release ($_). Pass -Version to pin one explicitly."
        }
    }
    Write-Info "Version: $Version"

    # --- Existing install ---------------------------------------------------
    if (-not $Force -and (Test-Path $DELTA)) {
        $existing = try { & $DELTA --version 2>$null } catch { "unknown" }
        Write-Ok "Already installed: $existing"
        Write-Info "Location: $DELTA"
        Write-Info "Re-run with -Force to upgrade."
        Invoke-DeltaSummary -DeltaPath $DELTA -Agent $Agent -InstallEngine:$InstallEngine
        return
    }

    # --- Work in a private, random temp directory ----------------------------
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $work = Join-Path ([System.IO.Path]::GetTempPath()) ("delta-" + [System.IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $work | Out-Null
    $script:TempDir = $work

    try {
        $zip = Join-Path $work $ASSET
        $url = "https://github.com/$REPO/releases/download/$Version/$ASSET"
        Write-Info "Downloading $ASSET"
        try {
            Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
        } catch {
            Write-Fail "Download failed: $_"
        }

        # --- Integrity --------------------------------------------------------
        $ok = $false
        if (-not $NoVerify) {
            try {
                $sumsText = (Invoke-WebRequest -Uri "https://github.com/$REPO/releases/download/$Version/SHA256SUMS" -UseBasicParsing).Content
                $line = ($sumsText -split "`n" | Where-Object { $_ -match "\s\*?$([regex]::Escape($ASSET))\s*$" } | Select-Object -First 1)
                if ($line) {
                    $expected = ($line -split "\s+")[0].ToUpperInvariant()
                    $actual = (Get-FileHash -Path $zip -Algorithm SHA256).Hash.ToUpperInvariant()
                    if ($expected -ne $actual) {
                        Write-Fail "Checksum mismatch for $ASSET.`n  expected: $expected`n  actual:   $actual`nThe download was not installed."
                    }
                    Write-Ok "Checksum verified"
                    Write-Host "  $actual  $ASSET" -ForegroundColor DarkGray
                    $ok = $true
                } else {
                    Write-Warn "$ASSET is not listed in SHA256SUMS; cannot verify"
                }
            } catch {
                Write-Warn "Could not retrieve SHA256SUMS; cannot verify"
            }
        } else {
            Write-Warn "Checksum verification skipped (-NoVerify)"
        }
        if (-not $ok -and $NoVerify) { $ok = $true }
        if (-not $ok) { Write-Warn "Proceeding without verification" }

        # --- Extract and install ---------------------------------------------
        $extract = Join-Path $work "extracted"
        Expand-Archive -Path $zip -DestinationPath $extract -Force

        $binary = Get-ChildItem $extract -Filter "delta*.exe" -Recurse -File | Select-Object -First 1
        if (-not $binary) { Write-Fail "No delta.exe found in $ASSET" }
        if ($binary.Length -lt 100KB) { Write-Fail "Downloaded delta.exe is implausibly small; refusing to install." }

        # Stage inside the destination so an interrupted copy cannot leave a
        # broken binary on PATH.
        $staged = Join-Path $InstallDir ".delta.new.exe"
        Copy-Item $binary.FullName $staged -Force
        Move-Item -Path $staged -Destination $DELTA -Force

        try {
            $versionLine = & $DELTA --version 2>$null
        } catch {
            Remove-Item $DELTA -Force -ErrorAction SilentlyContinue
            Write-Fail "Installed binary did not run. Removing it."
        }
        Write-Ok "Installed $versionLine"
        Write-Host "  $DELTA" -ForegroundColor DarkGray
    }
    finally {
        if ($script:TempDir -and (Test-Path $script:TempDir)) {
            Remove-Item $script:TempDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }

    # --- PATH ---------------------------------------------------------------
    if (-not $NoProfile) {
        $current = [Environment]::GetEnvironmentVariable("PATH", "User")
        if ($current -and $current.Contains($InstallDir)) {
            Write-Host "  PATH already contains $InstallDir" -ForegroundColor DarkGray
        } else {
            [Environment]::SetEnvironmentVariable("PATH", "$current;$InstallDir", "User")
            Write-Warn "Added $InstallDir to your user PATH. Restart your terminal to apply."
        }
    }

    # --- Managed engine -----------------------------------------------------
    if ($InstallEngine) {
        Write-Warn "Managed Moli engine downloads are Linux x86_64 only."
        Write-Info "On Windows, DeltaBrowser will adopt a locally installed browser instead."
        Write-Info "  Start Chrome with: chrome.exe --remote-debugging-port=9222"
    }

    # --- Agent --------------------------------------------------------------
    if ($Agent) {
        Write-Step "Configuring agent: $Agent"
        & $DELTA connect $Agent --install
        if ($LASTEXITCODE -ne 0) {
            Write-Fail "Could not configure agent '$Agent'. Run '$DELTA doctor' to diagnose."
        }
        Write-Ok "Agent '$Agent' configured"
    }

    Invoke-DeltaSummary -DeltaPath $DELTA -Agent $Agent -InstallEngine:$InstallEngine
}

function Invoke-DeltaSummary {
    param(
        [Parameter(Mandatory = $true)][string]$DeltaPath,
        [string]$Agent,
        [switch]$InstallEngine
    )

    Write-Host ""
    Write-Host "  DeltaBrowser is ready." -ForegroundColor Green
    Write-Host ""
    Write-Host "  Check the install:  $DeltaPath doctor" -ForegroundColor White
    if ($Agent) {
        Write-Host "  Restart your agent to load the MCP server." -ForegroundColor White
    } else {
        Write-Host "  Connect an agent:   $DeltaPath connect opencode" -ForegroundColor White
        Write-Host "  Or start Chrome:    chrome.exe --remote-debugging-port=9222" -ForegroundColor DarkGray
    }
    Write-Host ""
}

# Always run the install, forwarding whatever parameters were supplied. Direct
# invocation passes them through; the pipe-to-shell form arrives with none, so
# the defaults apply and the function does the work.
Install-DeltaBrowser -InstallDir $InstallDir -Version $Version -Force:$Force `
    -Agent $Agent -NoProfile:$NoProfile -InstallEngine:$InstallEngine -NoVerify:$NoVerify