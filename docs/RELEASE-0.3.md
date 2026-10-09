# AeroBrowser 0.3 — real execution and lifecycle upgrade

This is implemented Rust software with a real bundled Moli engine, not a dashboard populated with demo results. It is a substantial upgrade, **not a claim that every original PRD requirement is complete**.

## What changed

- **Real kernel resource limits:** optional cgroup-v2 memory/PID ceilings, enrollment before engine exec, group-wide OOM handling, live resource counters and subtree cleanup. Requested enforcement fails closed if unsupported or undelegated.
- **Independent HTTP principals:** SHA-256-digest credential file, owner-bound MCP/SSE/CLI sessions, quotas, identity endpoint and admin-only process shutdown. No default credentials.
- **Cancellable operations:** bounded FIFO actors for stdio, HTTP and SSE. Cancellation bypasses the operation queue. Deadlines invalidate the affected browser context rather than leaving a possibly half-mutated session reusable.
- **Measured HAR:** redirect hops preserved; timing values come from CDP, not fabricated send/wait/receive estimates. Unknown sizes/times use -1; incomplete requests are counted and omitted. Transfer bytes are not mislabeled as body bytes. Wall-time milliseconds are retained.
- **Actual drag input:** CDP mouse movements, press and release replace synthetic DOM DragEvents. Application drop outcomes must be independently asserted.
- **No default network mocks:** operator opt-in is required. Mock/block interception attempts are explicitly marked in captured data. Local fixtures remain separate from production runtime.
- **Recovery and data:** managed worker close remains usable after a crash; MCP tool results include structured data alongside presentation content. JSON-RPC consumers must match IDs rather than assume reply order.

## Validation of the shipped 0.3 binaries

Evidence is under `artifacts/v03/`:

| Validation | Observed result |
|---|---|
| Rust unit tests | 13 passed |
| Real end-to-end security/resource/network workflow | 28 checks passed |
| Transport and client-configuration regression | 54 checks passed |
| Managed Moli regression | 14 checks passed; mock capability intentionally disabled |
| Native Linux sandbox probes | Both profiles passed |
| Bundled engine extraction and repeated workflow | Both runs passed; 10 DSL steps and a rendered PNG each |
| Stdio cancellation | In-flight request cancelled, queued ping completed, clean process exit |
| Delegated launcher helper | Actual supervisor/worker cgroup split and controller enablement verified |

The new end-to-end test runs a real SQLite-backed HTTP service with an initially empty database. Browser input and a form POST insert a fresh UUID-bearing record; the test independently queries SQLite and verifies the record survives browser reload. It captures actual PNG/PDF artifacts and network traffic. The local service is an integration fixture, **not evidence of compatibility with arbitrary production applications**.

Separately, the managed browser fetched **https://example.com/** over live HTTPS; page text/title and an observed HTTP 200 were recorded in `public-https.har`. No interception rules supplied that content. This is a live-public smoke test, not broad-site certification.

The cgroup test allocates and touches a 512 MiB typed array in a worker capped at 256 MiB. The test checks real `memory.events` OOM-kill counters, then proves a peer browser still works. It does not merely compare a configured limit with a label. Cross-principal RPC/deletion and non-admin process shutdown were denied. HTTP and stdio cancellation were exercised; legacy SSE transport passed regression, but SSE cancellation was not separately stress-tested.

The bundled executable is **48,473,192 bytes**; the external-engine executable is **5,930,272 bytes**. The observed fresh-cache bundled login workflow took **999.51 ms**, versus **363.20 ms** on the repeat. Worker readiness measurements were 20/22 ms; these exclude extraction and do **not** establish a 40 ms cold start. Exact hashes accompany the package.

## Still incomplete / not certified

- Domain/loopback egress allowlist mediation and a comprehensive hostile multitenant security review.
- OAuth, TLS termination, full MCP conformance certification, progress/resumable streams.
- Replay, A2A, registry publishing and automatic safe recovery/retry semantics.
- Windows support and validated macOS/Lightpanda implementations.
- Full transactional browser-state restore: IndexedDB, service workers, caches and history remain omitted.
- Universal native HTML5 drag/drop behavior and comprehensive browser/SPA compatibility.
- Full performance/memory metrics where an engine lacks the underlying capability; unavailable values remain unknown.
- A universal 40 MB worker memory / 40 ms end-to-end cold-start guarantee.

The 74-check Chromium suite from 0.2 was **not re-run for this release**. Its historical results must not be treated as 0.3 validation. The 54 configuration checks validate generated client files, not live operation of ten installed agent clients. The included systemd deployment recipe is guidance; the delegation helper and kernel controls were tested directly, not a complete service installation.

For operating instructions and security boundaries, use [DEPLOYMENT.md](DEPLOYMENT.md). Historical 0.2 design documents are retained with supersession notices for traceability.
