# Frequently asked questions

Direct answers. For the long form, see the [documentation index](#documentation).

---

## What is Delta Browser?

Delta Browser is a [Model Context Protocol](https://modelcontextprotocol.io)
server that gives an AI agent a real web browser to drive. It exposes 62 tools
over MCP — navigate, click, type, read rendered text, screenshot, assert — and
connects to a browser you already have (Chrome, Edge, Chromium) or to a managed
sandboxed engine it launches itself.

It is not a hosted automation API. There is no key, no per-run billing, and no
service holding your credentials.

## Do I need a browser extension or an API key?

No. Delta Browser talks to Chrome, Edge, or Chromium over the Chrome DevTools
Protocol, or runs its own managed engine. There is no hosted service and no key.

## Do I need Node.js, npm, or Python?

No. Delta Browser is a single Rust binary. The installers put a `delta`
executable on your `PATH` and nothing else.

## Which platforms does it support?

Linux (x86_64 and aarch64), macOS (Intel and Apple Silicon), Windows (x64 and
ARM64), and WSL. Prebuilt binaries are published for all six.

The kernel-enforced sandbox is **Linux x86_64 only**. See
[Sandbox](#what-is-the-sandbox) below.

## Can I use my existing browser profile and logins?

Yes, when you run an external engine. Delta Browser creates an isolated context
inside that browser rather than driving your default profile directly, so your
existing tabs are not disturbed.

Logins do **not** carry over automatically. Use `save_state` and `restore_state`
to persist cookies and storage across sessions without re-authenticating.

## What is the sandbox, exactly?

On Linux x86_64, the managed engine runs each session in its own worker process
with a Landlock filesystem policy and a seccomp syscall filter applied **before**
the browser process starts, plus optional cgroup v2 memory and PID ceilings.

On Windows and macOS there is no equivalent. `server_status` reports
`external browser responsibility` rather than implying protection that does not
exist. If you need containment there, use a container.

When you point Delta Browser at a browser you launched yourself, it applies no
kernel policy to it — that browser's own security model is the boundary. This is
true on every platform.

## Why does `server_status` say `sandbox: external browser responsibility`?

Because you asked for an external engine. Delta Browser does not apply a kernel
policy to a browser it did not launch. Switch to a managed engine on Linux to
get one.

## Is Delta Browser open source?

It is **source-available** under the Business Source License 1.1, not OSI open
source. You get the complete source and may use, modify and redistribute it
freely for non-production and internal work.

It converts to Apache-2.0 on **2030-01-01**. Production use above USD 10M
annual revenue, or offering browser automation as a service, requires a
commercial licence. This is stated up front so nobody discovers it after
building on top of it. See [LICENSE](../LICENSE).

## How is it different from Selenium, Puppeteer, or Playwright?

Those are browser-automation *libraries*: your code drives them imperatively,
one call at a time. Delta Browser is an *MCP server*: the agent chooses tools
from a schema and the tool layer decides the sequence. It also adds batching,
delta snapshots, and a containment boundary, which are agent-shaped problems a
raw library does not try to solve.

The full comparison is in [COMPARISON.md](COMPARISON.md).

## Does it replace Playwright MCP?

Not universally. If Playwright MCP already works for you and you do not need
batching, delta snapshots, or sandboxing, either is reasonable. Delta Browser is
for the case where the agent is the caller and you want fewer round-trips and a
containment boundary.

## Can I use it with LangChain, CrewAI, AutoGen, or any MCP client?

Yes. Delta Browser speaks the standard Model Context Protocol, so any
MCP-capable client works — not only the ones with a built-in adapter.

Run `delta agents --config` to print a ready-to-paste server block for a client
Delta Browser does not recognise.

## Which agents have built-in support?

`opencode`, `claude-code`, `claude-desktop`, `cursor`, `vscode`, `codex`,
`windsurf`, `cline`, `roo-code`, `hermes`, and `generic`.

```sh
delta connect <agent> --install
```

Run `delta agents` to see which are actually installed on this machine.

## How do I install it?

One command:

```sh
curl -fsSL https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.ps1 | iex; Install-DeltaBrowser
```

No admin rights required — everything installs to your user directory. See
[INSTALL.md](INSTALL.md).

## How do I know the install worked?

```sh
delta doctor
```

It reports the engine, sandbox support, ports, auth token, and the config paths
of every agent it can detect, and prints the fix for anything that fails.
`delta doctor --json` gives machine-readable output. Include it in any bug
report.

## Does it work in Docker?

Yes. It is a static-ish binary with no runtime dependencies beyond glibc, and
it uses rustls rather than OpenSSL, so it drops into a scratch image without a
package manager. The container runtime provides the isolation.

## Why is it called Delta Browser?

Two things. The product is a browser for agents; "delta" is also the name of
its incremental snapshot feature, which returns only what changed since the
previous snapshot. The product was previously published as AeroBrowser, and was
renamed to Delta Browser in v0.0.1. See [CHANGELOG.md](../CHANGELOG.md).

## How do I report a vulnerability?

Through GitHub's private advisory form on the repository, not as a public issue.
See [SECURITY.md](../SECURITY.md).

## Are the numbers in the README verified?

The tool count is asserted in the test suite. Performance and token-efficiency
figures are **not** published, because no reproducible benchmark has been run.
When one exists it will ship as a script in this repository.

## Documentation

| Page | What it covers |
|---|---|
| [QUICKSTART.md](QUICKSTART.md) | Install, verify, connect an agent, first tool calls |
| [INSTALL.md](INSTALL.md) | Installers, options, environment variables, package managers |
| [AGENTS.md](AGENTS.md) | How to drive it from an agent |
| [API.md](API.md) | Every tool and its schema |
| [ARCHITECTURE.md](ARCHITECTURE.md) | How it works internally |
| [COMPARISON.md](COMPARISON.md) | Against Playwright MCP and Puppeteer MCP |
| [MIGRATION.md](MIGRATION.md) | From AeroBrowser |
| [DEPLOYMENT.md](DEPLOYMENT.md) | Daemon, systemd, containers |
| [SECURITY.md](SECURITY.md) | Trust model and what is not enforced |
| [ROADMAP.md](ROADMAP.md) | What is and is not implemented |
| [CHANGELOG.md](../CHANGELOG.md) | Release timeline and provenance |
