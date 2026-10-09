> Current 0.4 updates: see [RELEASE-0.4.md](RELEASE-0.4.md) and [DEPLOYMENT.md](DEPLOYMENT.md). Older state-restoration and cleanup limitations below are historical.

> **Historical 0.2 baseline below.** For current 0.3 changes and remaining gaps, read [RELEASE-0.3.md](RELEASE-0.3.md) and [DEPLOYMENT.md](DEPLOYMENT.md). Statements below that cancellation, tenant session ownership, hard RAM limits or measured HAR are absent were superseded in 0.3. Other limitations still apply.

# Runtime architecture — 0.2

```text
                 stdio / Streamable HTTP / legacy SSE / CLI
                                   |
                         schema + session router
                         /                    \
                 managed mode             external mode
                      |                         |
              private worker process      external CDP browser
              Landlock + seccomp          no retroactive sandbox
                      |                         |
                   Moli                  isolated browser context
                      |
           one owned context, up to 10 directly created tabs
```

The release is a single Rust package split into focused modules, not the PRD's proposed seven-crate workspace. That packaging choice does not pretend to change runtime isolation.

## Bundled distribution

The `bundled-moli` Cargo feature embeds the pinned upstream `.tar.gz`. The published archive digest and recorded executable digest are compiled into the binary. First use verifies/extracts a small allowlist of engine and license files into a cache; writes use temporary files and rename. Later managed launches rehash the cached engine before executing it. This provides a single downloadable executable, **not** an in-process browser or zero extracted files.

MCP initialization is lazy with respect to engine startup. A tool that needs a browser launches/attaches one. `engine_details.startup_ms` measures the launch/readiness interval only, excluding prior extraction/hash verification. Whole workflow timings are reported in the bundled test evidence.

## Managed workers

Every managed MCP session owns a child process group. The broker gives the child a private working directory, a cleared environment, runtime read grants and private write grants. It invokes the same Aero executable in an internal worker mode, applies restrictions, then `exec`s the engine. The engine's arguments and policy are constructed by the trusted broker, not page JS.

The child has its own engine and storage context. An integration test kills one Moli process and checks that another session remains usable. This solves crash separation by **not sharing one engine process**. It does not satisfy the original one-process target. The unbundled external-CDP mode instead shares a browser and cannot promise engine crash separation.

## CDP event actor

Each browser connection has a continuously running WebSocket actor. A bounded channel carries outgoing commands. Unique IDs correlate replies with oneshot receivers; timeouts remove abandoned callers on later dispatch. The actor responds to Fetch pauses immediately while a page evaluation is awaiting a mocked request, avoiding a request/response deadlock. It records bounded network/console events and tracks inflight requests for network-idle waits.

An HTTP session mutex provides sequential commands per session. Other sessions proceed concurrently on Tokio's runtime. The CLI endpoint intentionally shares one session among trusted CLI callers. Browser events and command outputs remain untrusted data.

## Snapshot lifecycle

Snapshots use the real engine accessibility tree, optionally scoped to a CSS-matched root, depth-limited and compacted. Cursor-target scanning is bounded to 5,000 DOM candidates / 200 pointer targets. Element refs contain snapshot generations and map to backend DOM node IDs. A new snapshot, tab switch or explicit navigation invalidates refs. Document identity is checked again on ref resolution.

Moli v1.1.12 can retain stale JS geometry after a `hidden` mutation until painting. The Moli adapter requests a genuine 1-pixel screenshot before geometry-dependent operations to refresh layout; it does not fabricate rectangles. This compatibility workaround adds latency.

## State and artifacts

Private artifact directories use restrictive Unix directory/file modes. Agent-supplied artifact names cannot contain directories. Page screenshots, PDFs and reports are written only there. CLI screenshot/report destinations are intentionally local caller filesystem writes, not remote tools.

State export includes versioned cookies, current tab URLs, localStorage, sessionStorage and scroll. Restore validates basic shape and URLs before applying it, checks redirect origins before injecting storage, and explicitly reports unsupported state. Storage is applied after initial page load; applications that hydrate from storage during startup may need reload. A failure can leave a partially restored session.

## Transport and lifecycle

Stdio uses bounded newline-delimited JSON-RPC. HTTP supports JSON responses, session IDs and DELETE. Legacy SSE has endpoint/message events and bounded delivery queues. The administrative shutdown endpoint is distinct from the session-scoped agent shutdown tool. A server idle-session sweeper releases abandoned sessions; SSE disconnect cleanup can take up to its interval.

Cancellation, progress/resumption, complete protocol conformance, tenant-bound OAuth and automatic worker-crash restart are not implemented. After a crash, close the failed session browser and retry only operations known to be safe to repeat.
