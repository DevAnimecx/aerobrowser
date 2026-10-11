> **This document is a 0.2 baseline.** For the current trust model and the full list of
> what is *not* enforced, read [../SECURITY.md](../SECURITY.md) — it supersedes
> anything below. Statements that cancellation, tenant session ownership, hard RAM
> limits, or measured HAR are absent were superseded in 0.3; restore behaviour
> changed in 0.4. Other limitations still apply.

> Current release notes: [RELEASE-0.0.md](RELEASE-0.0.md). Deployment boundaries:
> [DEPLOYMENT.md](DEPLOYMENT.md). Outstanding gaps: [ROADMAP.md](ROADMAP.md).

# Security boundaries — 0.2 (historical baseline)

## Intended trust model

This is a tested engineering build, not an audited hostile multi-tenant service. Page content is untrusted. The server operator, local configuration files and bearer-token holders are trusted control-plane users. Never use a personal browser profile containing credentials for external CDP tests.

## Managed Linux mode: real enforcement

- Landlock filesystem policy is installed before exec. Requires ABI >=2; unsupported kernels fail closed.
- Runtime directories are read-granted. The engine working directory is write-granted. Explicit upload roots and custom profile paths are additional operator grants.
- `no_new_privs` plus seccomp blocks selected dangerous syscalls: tracing, namespace changes, mount operations, BPF/key operations, process-memory access, pidfd operations and others.
- ABI2 lacks truncate mediation. The compatibility filter denies `truncate`/`ftruncate`, `open(O_RDONLY|O_TRUNC)`, and `openat2` (ENOSYS fallback to inspectable open/openat). ABI3+ uses Landlock's truncate right. Regression tests cover the read-only-open truncation case.
- Default Moli workers cannot fork/vfork or create non-thread clones. Thread creation is allowed; clone3 returns ENOSYS for a filterable fallback. Peer signalling/limit/affinity operations are restricted in this no-child-process mode.
- CPU time and open-FD limits are applied. These are **not RAM/cgroup limits**.
- Worker launch clears inherited environment variables, including HTTP bearer tokens. Private worker config is removed before exec.
- Process-group cleanup terminates owned workers. Dedicated managed workers provide tested crash separation, at higher process/memory cost than a shared engine.

## Network policies

**`scrape` / `full`: unrestricted network egress.** These names do not imply a hostname allowlist. Filesystem/syscall restrictions still apply.

**`offline` / `untrusted` (`network: none`):** outbound `connect` is denied; new datagram/raw sockets are denied; destination-address sendto and TCP Fast Open paths are restricted. Accepted stream-socket replies remain permitted so the loopback CDP server can function. This is an outbound-connection restriction with an intentional control-channel exception, **not a total network namespace or zero-I/O guarantee**. An engine listener is configured on loopback. The filter does not itself implement an IP-address bind allowlist.

**`base`, `web-dev`, `test`, arbitrary hostname allowlists and loopback-only egress:** not implemented. Selecting them fails instead of falling back to unrestricted networking. Implementing these safely requires an egress boundary covering DNS, redirects, alternate protocols and private/metadata addresses. Landlock TCP port grants alone cannot enforce hostnames or destination IPs.

## Other modes

- External CDP browsers are not sandboxed by Delta. Their existing browser/process security is the operator's responsibility.
- Managed Chrome needs child processes and uses a broader policy; this managed path has not been validated. Do not infer Moli's no-child-process guarantees for it.
- `--no-sandbox` explicitly disables Delta restrictions and is reported in status. It is a development opt-out, not a fallback.
- A macOS Seatbelt launcher exists, but its policy and runtime behavior were **not validated here**. Windows AppContainer/Job Object enforcement is not implemented.

## Access and artifacts

- HTTP requires a bearer token of at least 24 characters; generate 32 random bytes. Browser Origin requests are rejected. Loopback is the default bind.
- Token holders share one trust domain. A session ID is not an independently authenticated tenant identity. The `/cli` session is shared intentionally.
- Raw CDP is limited to selected current-target operations. Browser/Target/Storage/IO escape-hatch methods are not exposed.
- Server shutdown/profile tools are session-scoped. `delta stop` uses a separate authenticated admin endpoint.
- Remote artifact paths are plain filenames in a private session directory. Uploads are disabled unless an explicit authorized root is configured. Upload paths are canonicalized and constrained to that root.
- State, screenshots, console/network output and cookies can contain secrets. Metadata operation logs omit arguments. HAR redacts selected common credential headers, **not all secrets**: URLs, other headers and network events can still reveal sensitive data.
- Named dev-server argv/cwd/port entries are operator configuration. Agent recipes cannot supply arbitrary commands or override their args. These servers run outside the engine sandbox with the operator's privileges; environment is cleared and owned processes are cleaned up. Do not configure an unsafe command.

## Residual risks and missing hardening

- No independent security review, seccomp/Landlock escape audit, syscall fuzzing campaign or dependency audit.
- Landlock is filesystem-access control, not a VM. Not all metadata operations are mediated by the selected filesystem rights. Runtime-path read grants and native code/JIT execution must be included in a threat review. Do not interpret the policy as preventing every possible action on user-owned metadata.
- Host-kernel compromise remains outside these protections. Native engine vulnerabilities still require patching and defense in depth.
- No hard per-session RSS limit, cgroup CPU accounting, whole-process-tree accounting or comprehensive popup/worker quotas.
- Requests waiting on a session mutex are not globally admission-controlled. Script/input/event counts are bounded, but all outputs and page allocations are not.
- Event queues hold 1,000 entries; very large individual events or resource graphs need stronger byte budgets.
- Timeouts do not roll back browser actions. Never blindly retry non-idempotent actions such as payments.
- No full MCP cancellation, OAuth, TLS termination, tenant identity, encrypted state-at-rest or replay-safe credentials.
- Local CLI destination writes and local config generation use the caller's privileges. Existing files can be overwritten only through the documented operations/flags; these writes are not engine-sandbox operations.
- Skill/prompt text tells agents to treat pages as untrusted, but cannot enforce an agent's reasoning behavior.

## Required production gate

Audit the actual engine/policy pair on each supported kernel/architecture, tighten metadata and device access, add cgroup-backed resource controls, implement a robust egress broker/network namespace design, authenticate tenant identities, fuzz transports and schema boundaries, and publish reproducible signed artifacts with an SBOM. Do not publish a “zero sandbox escape” or “all clients/platforms supported” guarantee based on this test suite.
