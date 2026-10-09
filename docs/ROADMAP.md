# Implementation status — 0.5

This is a current map of what works, not a claim of completeness. The right-hand
column is as important as the left.

| Area | Implemented | Remaining |
|---|---|---|
| CLI | All subcommands, aliases, `aero doctor`, HTML and JSON reports | Shell completions; interactive prompts |
| MCP tools | 62 tools with schemas, `batch`, delta snapshots, target-scoped CDP allowlist | Formal MCP conformance testing; resumable streams |
| Bundled engine | Pinned, digest-verified Moli; managed workers on Linux x86_64 | In-process embedding; broad SPA compatibility; cold-start targets |
| Worker isolation | Dedicated worker per session, Landlock + seccomp, private artifacts | Independent adversarial security review |
| Resource limits | cgroup v2 memory and PID ceilings, OOM confined to the worker, live counters | Windows Job Objects; macOS equivalents; supervisor budgets |
| HTTP identities | Owner-bound MCP/SSE/CLI sessions, quotas, admin-only shutdown | OAuth; TLS; per-principal sandbox policies |
| Lifecycle | FIFO queue, cancellation, deadlines, awaited cleanup | Automatic recovery and reconnect guarantees; soak tests |
| State | Cookies, tabs, storage and scroll staged before boot; rollback on prepare failure | IndexedDB, history, cache, service workers; **encryption at rest** |
| Browser actions | Real pointer and keyboard input, native drag, uploads, screenshots, PDF, styles | Universal HTML5 drag and drop; occlusion semantics |
| Network evidence | Real CDP capture, redirects, measured HAR, unknown values marked `-1` | **Connection-time egress mediation**; HAR bodies; replay |
| Performance | Measured page timings; unavailable values stay `null` | Metrics the engine does not expose |
| Testing | DSL, assertions, self-contained HTML reports, allowlisted services | Broad-site certification; engine matrix |
| Clients | Ten config adapters with preservation and backup tests | **No client has been run against a live server** |
| Transports | Stdio, HTTP, legacy SSE, session ownership | Full conformance; progress reporting |
| Replay | Metadata logs and executable recipes | Secret-safe replay; non-idempotent action controls |
| Platforms | Linux x86_64 verified; Windows and macOS build and run | Windows has **no** isolation; macOS Seatbelt unvalidated |
| Distribution | GitHub Releases with `SHA256SUMS` and build provenance | Homebrew, apt, rpm, crates.io |
| Licensing | Real BSL-1.1 with an Additional Use Grant | Commercial licensing operations |

---

## Not implemented

Stated explicitly because each of these was previously implied or claimed:

- **No network egress boundary by hostname or IP.** The `offline` profile denies
  outbound connections outright via seccomp, which is a real kernel boundary. There
  is no allowlist, no per-destination decision, and no mediation at connection
  time. A CDP request filter is not a firewall and must not be presented as one.
- **`base`, `web-dev`, and `test` sandbox profiles do not exist.** They are declared
  in the profile list and refuse to run.
- **No Windows isolation.** No AppContainer, no Job Object, no memory or PID ceiling.
- **macOS Seatbelt is unvalidated.** The code path exists; it has not been exercised
  in the validation environment.
- **State is not encrypted at rest.** The auth token is stored in plaintext.
- **`evaluate` is not privilege-gated.** Its schema says "trusted clients only"; the
  code does not enforce it.
- **No client has been run against a live server.** Config serialization is tested;
  runtime integration is not.
- **No signed release artifacts.** GitHub build provenance is published; there is no
  GPG, minisign, or cosign signature.
- **No performance claims.** No token-efficiency figure is published because no
  reproducible benchmark has been run.

---

## Priorities

1. **Connection-time network mediation** enforcing domain and IP decisions at the
   moment of connection, including redirects, DNS rebinding, IPv6, and alternate
   protocols. This is the largest gap between what a reader might assume from
   "sandboxed" and what is actually enforced.
2. **Encryption at rest** for `save_state` output and the auth token.
3. **MCP conformance and authentication** against maintained clients and a real
   identity provider.
4. **Windows Job Objects** and validation of the macOS Seatbelt path, so the
   platform matrix matches the claims.
5. **A reproducible token-efficiency benchmark** covering `batch` and delta
   snapshots. Until this exists, no percentage should be claimed.
6. **Live client verification** for the ten adapters.
7. **Registry publishing** — requires an owner-controlled registry and signing
   identity.

No capability is added to reach a feature count. Opt-in testing mocks remain
disabled in normal execution.