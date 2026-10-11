# Delta Browser vs Playwright MCP vs Puppeteer MCP

A factual comparison for people choosing an MCP server to give an agent a
browser. Tool counts for the alternatives are approximate and change with their
upstream releases; Delta Browser's count is asserted in its own test suite.

---

## Summary

| | Delta Browser | Playwright MCP | Puppeteer MCP |
|---|---|---|---|
| Tools exposed | **62** | ~30 | ~25 |
| Batched calls (`batch`) | **Yes** | No | No |
| Delta snapshots | **Yes** | No | No |
| Runs against your own browser | **Yes** | Yes | Yes |
| Managed sandboxed engine | **Yes** (Linux x86_64) | No | No |
| Kernel syscall filtering | **Yes** (seccomp, Linux) | No | No |
| Hard memory / PID limits | **Yes** (cgroup v2) | No | No |
| Persistent sessions | **Yes** | No | No |
| Self-hosted daemon | **Yes** | No | No |
| Written in | Rust | TypeScript | TypeScript |
| Install | One shell command | `npm install` | `npm install` |
| Runtime dependencies | None beyond glibc | Node.js | Node.js + Chromium |

---

## Where they genuinely differ

### Batching

Delta Browser's `batch` runs up to 50 tools in one MCP message against a single
browser context. Each call is validated against the same schema as a standalone
`tools/call`, so a malformed argument fails before it reaches the browser.

Neither alternative has an equivalent. They expose one tool per action, so a
login flow costs the same number of round-trips it has steps.

This matters most for agent latency, where each MCP message is a full model
turn boundary, not just a network call.

### Incremental snapshots

`snapshot { delta: true }` returns only added, changed, and removed refs since
the previous snapshot on that tab. Elements are fingerprinted on the fields an
agent actually acts on — role, accessible name, depth, disabled state — so a
re-render with identical semantics produces no output, while a rename or a role
change does.

Returning the full accessibility tree after every action is the main way agent
browser sessions burn context window. Alternatives have no incremental mode.

### Isolation

On Linux x86_64, Delta Browser's managed engine runs each session in its own
worker process with a Landlock filesystem policy and a seccomp syscall filter
applied **before** the browser process starts, with optional cgroup v2 memory
and PID ceilings.

Neither alternative has an equivalent. Both attach to a browser you launch
yourself, so that browser's own security model is the boundary — a real one, but
not one they provide.

Worth being precise about: Delta Browser provides no kernel isolation on Windows
or macOS either. `server_status` reports that rather than implying protection
that is not there. If you need containment on those platforms, use a container.

### Sessions

`save_state` and `restore_state` capture cookies, tabs, storage and scroll, so
a session can be reconstructed without re-authenticating. Delta Browser can also
run as a self-hosted HTTP daemon, which lets several agents share one browser
instead of each starting their own.

### Footprint

Delta Browser is a static-ish Rust binary with no runtime dependencies beyond
glibc, and it uses rustls rather than OpenSSL, so it drops into a scratch
container without a package manager. Both alternatives need Node.js, and
Puppeteer pulls a Chromium build with it.

That matters in CI and in agent sandboxes, where image size and cold-start time
are real costs.

---

## When to choose something else

This comparison is not a claim that the alternatives are bad.

**Keep Playwright MCP** if you already have Playwright working in production and
do not need batching, delta snapshots, or a containment boundary. It is mature
and well-documented, and a working setup beats a theoretically better one.

**Keep Puppeteer MCP** for the same reasons, and because Puppeteer has a very
large ecosystem around it.

**Choose Delta Browser** when the agent is the caller and any of these matter:
round-trip count, context window spent on snapshots, a containment boundary
around pages you do not control, or shipping a browser into an environment where
installing Node.js is unwelcome.

---

## Honest limitations

- No kernel sandbox outside Linux x86_64. A container is still the right answer
  on macOS and Windows.
- Fewer integrations than Playwright, which has years of accumulated adapters.
- No recorded benchmarks yet. Performance and token-efficiency numbers are not
  published here because none has been measured reproducibly. When one exists it
  will ship as a script in this repository rather than as a claim in a README.

## See also

- [Installation](INSTALL.md)
- [Architecture](ARCHITECTURE.md)
- [Driving Delta Browser from an agent](AGENTS.md)
- [FAQ](FAQ.md)
