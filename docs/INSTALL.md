# Installation

Pre-built binaries are published as GitHub Releases for six targets. There is no
hosted installer domain; both installers read directly from the release API.

## Requirements

| Platform | Requirement | Managed engine | Kernel isolation |
|---|---|---|---|
| Linux x86_64 | glibc 2.31+, OpenSSL 3.x, fontconfig, kernel 5.13+ | Yes | Landlock + seccomp + cgroup v2 |
| Linux aarch64 | same | No | Linux only, but no managed engine |
| macOS 12+ | — | No | Seatbelt path exists, unvalidated |
| Windows 10/11 | — | No | **None.** No AppContainer or Job Object |

Landlock requires kernel 5.13 or newer. Run `delta sandbox probe` to confirm what
your kernel provides; `delta doctor` includes it.

On any platform you can use an external Chromium-family browser instead. That
browser's own security model is the boundary — Delta Browser applies no kernel
policy to it.

---

## One-line install

```sh
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.sh | sh
```

```powershell
# Windows PowerShell 5.1 or newer
irm https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.ps1 | iex; Install-DeltaBrowser
```

With agent configuration in one step:

```sh
curl -fsSL https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.sh \
  | sh -s -- --agent opencode --install-engine
```

```powershell
irm https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.ps1 | iex
Install-DeltaBrowser -Agent opencode -InstallEngine
```

### What the installers verify

1. Resolves the latest release from the GitHub API, or honours `--version`/`-Version`.
2. Downloads the asset for your platform.
3. Fetches the release's `SHA256SUMS` and verifies the digest. A mismatch aborts
   the install.
4. Confirms the payload starts with an ELF, Mach-O, or PE magic number, so a 404
   page can never reach your `PATH`.
5. Writes to a staging file in the destination directory and moves it into place
   atomically.
6. Runs `delta --version`. If it fails, the binary is removed.
7. Exits non-zero on any failure.

### Installer options

| Shell | PowerShell | Description |
|---|---|---|
| `--dir DIR` | `-InstallDir DIR` | Install location (default: `~/.local/bin`) |
| `--version VER` | `-Version VER` | Pin a tag instead of latest |
| `--force` | `-Force` | Replace an existing install |
| `--agent NAME` | `-Agent NAME` | Configure an agent |
| `--install-engine` | `-InstallEngine` | Fetch the managed engine (Linux x86_64) |
| `--no-profile` | `-NoProfile` | Do not modify `PATH` |
| `--no-verify` | `-NoVerify` | Skip checksum verification. Not recommended. |

On Windows, if your policy blocks script execution:

```powershell
powershell -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.ps1 | iex; Install-DeltaBrowser"
```

---

## Manual download

Every release includes `SHA256SUMS`. Verify before installing.

```sh
VERSION=v0.0.1
curl -fsSLO "https://github.com/DevAnimecx/deltabrowser/releases/download/${VERSION}/delta-linux-x86_64"
curl -fsSLO "https://github.com/DevAnimecx/deltabrowser/releases/download/${VERSION}/SHA256SUMS"
sha256sum -c SHA256SUMS --ignore-missing
chmod +x delta-linux-x86_64
mv delta-linux-x86_64 ~/.local/bin/delta
```

Windows archives ship the `LICENSE` alongside the binary, as BSL-1.1 redistribution
requires.

---

## Build from source

```sh
git clone https://github.com/DevAnimecx/deltabrowser.git
cd deltabrowser
cargo build --release --locked
```

The binary is at `target/release/delta` (`delta.exe` on Windows). Use `--locked` so
the build uses the committed `Cargo.lock`.

To build with the engine bundled in:

```sh
cargo build --release --locked --features bundled-moli
```

This requires the engine archive produced by the release pipeline; see
[DEPLOYMENT.md](DEPLOYMENT.md).

---

## Verify the install

```sh
delta --version   # delta 0.0.1
delta doctor      # engine, sandbox, ports, token, agent configs
```

`delta doctor` is the check to run when anything misbehaves. Each failing check
includes the specific remedy. Include its output in any bug report.

---

## Then

```sh
delta connect opencode --install
```

Restart your agent. See [QUICKSTART.md](QUICKSTART.md) for the full walkthrough.

---

## Uninstall

```sh
rm ~/.local/bin/delta
rm -rf ~/.delta                      # engine cache and generated token
```

Agent configs written by `delta connect` are left in place; remove the `delta` entry
from your agent's MCP config yourself. Backups of pre-existing config files are
written next to the original with a `.backup-<uuid>.json` suffix.