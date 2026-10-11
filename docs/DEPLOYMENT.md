# Deploying Delta Browser 0.4

## Single-file Linux distribution

```bash
chmod +x package-v04/delta-bundled-linux-x86_64
./package-v04/delta-bundled-linux-x86_64 mcp  # stdio, no bearer token needed
```

The bundled file contains a pinned compressed Moli engine. It verifies and extracts the engine into the user's cache before launching a separate sandboxed worker. This is not an in-process browser, a statically linked system image, or a 40 ms cold launch. No Docker or Node runtime is required. Linux shared libraries and fonts are still needed.

## HTTP authentication

For one operator, generate a random token (at least 24 characters):

```bash
export DELTA_TOKEN="$(openssl rand -hex 32)"
./package-v04/delta-bundled-linux-x86_64 mcp --transport http
```

For multiple principals, set `DELTA_AUTH_FILE` to a protected JSON file:

```json
{"principals":[
  {"id":"alice","token_sha256":"REPLACE_WITH_SHA256_OF_A_RANDOM_TOKEN","admin":false,"max_sessions":4},
  {"id":"operator","token_sha256":"REPLACE_WITH_A_DIFFERENT_TOKEN_DIGEST","admin":true,"max_sessions":8}
]}
```

Generate each independent high-entropy bearer token yourself; hash its exact bytes without a newline. Digests must be 64 lowercase hexadecimal characters. Keep the bearer tokens secret. Example placeholders above are deliberately invalid; they cannot authenticate. There are no default production credentials. If `DELTA_TOKEN` is also present, it adds an admin principal named `operator`; avoid conflicting IDs. Configuration is loaded on startup; restart to rotate/revoke credentials.

All HTTP browser endpoints require `Authorization: Bearer TOKEN`. MCP, SSE, CLI, cancellation, and DELETE operations are owner-bound. Even an admin cannot borrow another principal's browser session. Only admins can call `POST /admin/shutdown`; MCP `server_shutdown` still closes just the caller's browser. `GET /whoami` returns the authenticated identity and actual session counts. `/health` is process liveness, not a browser readiness claim. Browser Origin headers are denied.

Limits: 64 configured principals, 32 total MCP/SSE/CLI sessions, per-principal quota 1–32. Each principal has a distinct cached CLI session, counted in its quota. MCP/SSE sessions idle for an hour are expired; an active request is not evicted. CLI sessions persist until process shutdown, although `close` releases their browser workers.

Bind to loopback by default. Remote deployments need your TLS reverse proxy and a trusted network policy. Static bearer authentication is **not OAuth**, per-principal policies do not yet differ, and this has not been certified for hostile multitenant hosting. All principals share the operator-configured upload allowlist and sandbox profile.

## Request lifecycle

`DELTA_REQUEST_TIMEOUT_MS` defaults to 120000; permitted range 100–900000. One actor serializes operations per session. Eight requests may wait; overflow is rejected, never silently reordered. Other sessions execute independently. MCP cancellation notifications are processed outside the operation queue.

Cancellation/deadline destroys the managed browser worker and invalidates its refs/state. The next browser tool launches a fresh worker. Already completed clicks, submissions, or external writes are **not rolled back**, and there is no automatic retry of non-idempotent work. External CDP contexts get a bounded disposal attempt; a dead external engine can prevent cleanup. JSON-RPC responses must be correlated by ID, not arrival position.

## Actual memory and PID limits

```bash
./package-v04/delta-bundled-linux-x86_64 \
  --cgroup-root "$DELEGATED_CGROUP" \
  --memory-limit-mb 256 --pids-limit 128 mcp
```

Linux cgroup v2 is required. The configured parent must already delegate and enable the `memory` and `pids` controllers, and grant permission for process migration from the supervisor's cgroup. The launcher creates a private subgroup and enrolls the worker **before exec**. `memory.max`, `memory.oom.group`, `memory.swap.max` when available, and `pids.max` are kernel controls, not advisory counters. Missing delegation or enrollment errors fail closed. No limits are claimed when none were requested. External browsers cannot receive these limits retroactively.

`server_status.resources` reads live cgroup memory, peak, PID, CPU and OOM counters. The limit includes worker descendants, not the supervisor. A PID limit counts threads as well as processes. Unlimited/unavailable numeric values are null. The supervisor itself needs its own deployment-level memory budget. Normal close, cancellation and graceful shutdown await worker reaping and cgroup removal. Emergency Drop cleanup remains best effort after unexpected supervisor termination; operators may need to remove empty owned subgroups.

`scripts/run-delegated.sh` prepares supervisor/worker child groups inside an already delegated service/scope, then execs the supplied command. It refuses the global root and groups containing other processes. It does not use sudo or grant itself privileges. A typical systemd service uses `User=delta`, `Delegate=yes`, and an ExecStart such as:

```text
/opt/delta/scripts/run-delegated.sh /opt/delta/package-v04/delta-bundled-linux-x86_64 --memory-limit-mb 256 --pids-limit 128 mcp --transport http
```

Configure the auth-file environment, service resource ceiling, TLS proxy, filesystem permissions, restart policy and logs for your environment. The delegation helper and kernel limits were tested directly; a complete systemd deployment was not exercised in this sandbox. Do not make global cgroup controls writable to untrusted users.

## No default mocks

Normal execution uses the real browser and network. `intercept_request` with `action=mock` fails unless the server operator explicitly starts with `DELTA_ALLOW_NETWORK_MOCKS=1`. Enabling it is for tests, never a claim of real upstream data. Interception attempts are marked in captured HAR. Block/log rules remain available without mock permission.

Test fixtures live in `tests/` and `examples/`; they are not runtime data. HAR omits bodies and unfinished requests, preserves redirects, and records unknown sizes/timings as -1 instead of estimates. Authorization/cookie/API-key header values are redacted in HAR; URLs, query parameters, and raw network logs can still contain secrets. Protect exported artifacts accordingly.

## Rebuilding

```bash
cargo test --locked
cargo build --release --locked
# Download the exact archive from docs/engine-pin.json and verify its digest first.
DELTA_BUNDLED_MOLI_ARCHIVE=/absolute/path/moli.tar.gz \
  cargo build --release --locked --features bundled-moli
```

The shipped executable pin, notices and licenses remain included. Do not substitute an unverified engine archive. The compiled Rust target and dependency caches are intentionally not packaged.

## State restoration and telemetry in 0.4
Restore validates origins, storage types, tab counts and conflicting same-origin localStorage before changing the active context. It stages a fresh context, installs cookies, initializes Web Storage before page scripts, verifies execution, removes initialization hooks, then commits and disposes the old context. Ordinary preparation failure restores the old context selection; cancellation/deadline still invalidates the entire browser. If old-context disposal fails, the response carries a warning and cleanup is retried at close. External website effects are not rolled back. IndexedDB/history/workers/cache are still not checkpointed. Engines without the necessary pre-document script APIs fail rather than pretending restoration succeeded.

Network and console retained buffers each have 4 MiB serialized-byte, 1,000-event, and 256 KiB per-event ceilings. Oversized events are omitted in full, with counters; older events are evicted under pressure. `server_status.telemetry` exposes measured retention/drop counters. These are retention budgets, not hard supervisor RSS bounds. In-flight parsing, browser responses and other allocations still require deployment-level resource controls.
