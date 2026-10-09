# AeroBrowser 0.4 — restore, retention and lifecycle release

This release extends the actual Rust/browser implementation. It does not claim that the entire original PRD is complete.

## Implemented and verified

### State available before application startup
Restoration now stages a fresh browser context while keeping the previous context intact. Cookies are installed before navigation. Origin-scoped Web Storage initialization runs before the page's startup scripts, and a per-tab execution marker confirms it ran. The initialization hook is removed before committing. Multiple tabs retain distinct sessionStorage; conflicting same-origin localStorage is rejected rather than resolved arbitrarily.

URL/origin mismatches, invalid storage, oversized state, excessive tabs/cookies, and malformed scroll positions fail validation. A failed staged navigation or cross-origin redirect restores the original context selection and disposes the staged context. Failed cleanup is explicitly reported and retained for retry on close.

This is a browser-context transaction, not a transaction across remote websites. Requests, submissions, and other external effects during staging cannot be undone. Cancellation/deadlines deliberately destroy the affected session worker instead of preserving a possibly half-mutated browser. IndexedDB, history, service workers and caches remain excluded.

### Bounded telemetry, not fake or silently truncated results
Network and console buffers each retain at most 4 MiB of serialized event data and 1,000 events. An individual event over 256 KiB is omitted in full. Drop/oversize counters and measured retained bytes are exposed in `server_status.telemetry`. No replacement rows or invented values stand in for omitted data. This does not impose a hard cap on supervisor RSS or transient parsing allocations.

### Awaited normal cleanup
Normal close, cancellation and graceful shutdown now await worker reaping and kernel cgroup removal. Emergency Drop remains a fallback, not a guarantee following abrupt supervisor termination. A real test checks both `/proc/<pid>` and the worker cgroup directory are absent before close returns.

### Dependency inventory
`docs/SBOM.cdx.json` inventories 227 components from actual Cargo resolution plus the pinned Moli executable. It passes the official CycloneDX 1.6 JSON Schema. `scripts/generate-sbom.py` regenerates it. This is declared-dependency metadata across platforms, not a complete engine/OS SBOM, vulnerability audit, license opinion, signature or build attestation.

## Tests on this release

| Suite | Result |
|---|---|
| Rust unit tests | 18 passed |
| New state/lifecycle/telemetry workflow | 22 checks passed |
| Real execution/security/resource regression | 28 checks passed |
| Transport and client-config regression | 54 checks passed |
| Managed Moli regression | 14 checks passed |
| Native sandbox profiles | Both passed |
| Bundled extraction and repeated workflow | Both passed |
| Stdio in-flight cancellation | Passed |
| Process and cgroup cleanup before close response | Passed |
| CycloneDX schema validation | Passed |

The state test serves a real HTTP application whose first inline script reads cookies, localStorage and sessionStorage. It verifies those startup observations independently, a rejected cross-origin restore, old-tab preservation, hook removal after reload, cancellation during a real delayed HTTP request, and observed console-event pressure. No request interception supplies the results. The public HTTPS smoke test still fetches example.com through the actual browser.

Evidence, screenshots and machine-readable reports are under `artifacts/v04/`. Older release evidence is not current validation. The Chromium-specific 74-check suite was not rerun; ten installed agent clients were not launched.

## Distribution measurements

- Bundled Linux x86_64 executable: **48,520,496 bytes**.
- External-engine executable: **5,977,560 bytes**.
- Fresh-cache bundled test workflow: **1,158.28 ms**; repeat: **386.70 ms**.
- Both runs completed 10 DSL steps and produced a real 36,354-byte PNG.
- Worker readiness was 22 ms in both runs; that excludes extraction and is not a cold-start guarantee.

The distribution still extracts and launches a separate pinned Moli engine and needs compatible Linux system libraries. It is not an in-process browser or a universal 40 MB/40 ms implementation.

## Still open

Domain/IP egress mediation, OAuth/TLS integration, formal MCP conformance, progress/resumption, sensitive-safe replay, A2A, complete state checkpointing, broad browser/site compatibility, validated cross-platform releases, signed provenance, public package publication and sustained production testing remain open. See [ROADMAP.md](ROADMAP.md) for the current coverage map.

Mocks remain disabled by default. Opt-in network mocking is retained only as a clearly labeled testing capability; test fixtures are separate from production runtime.

## Reproduce the new state test

From the extracted release directory, install the verified engine and run:

```bash
./package-v04/aero-bundled-linux-x86_64 engine install moli
AERO_BIN=package-v04/aero-linux-x86_64 \
MOLI_PATH="$HOME/.aero/engines/moli-v1.1.12/moli" \
python3 tests/v04_state_e2e.py
```

This test starts and stops its own local HTTP service. It needs public HTTPS access for the final smoke test. The cgroup/security regression additionally requires a delegated controller subtree and the loopback validation server described in DEPLOYMENT.md; it must not be represented as passed if those prerequisites are missing. `tests/v03_real_e2e.py` uses explicitly test-only alice/bob/admin tokens, not production credentials. All test evidence defaults to `artifacts/v04/`; legacy test files accept `AERO_EVIDENCE_DIR` for another output directory.
