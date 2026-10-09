# Security Policy

## Reporting a vulnerability

Please report security issues privately rather than opening a public issue.

- Email: licensing@blackvaulttech.netlify.app with "security" in the subject
- Include: affected version, platform, reproduction steps, and the impact you observed

We aim to acknowledge within 3 business days. Please give us a reasonable window to
ship a fix before disclosing publicly.

## Supported versions

| Version | Supported |
|---|---|
| 0.5.x | Yes |
| 0.4.x | Security fixes only |
| < 0.4 | No |

## Trust model

AeroBrowser runs **untrusted web content**. The design assumes any page it opens may
attempt to escape its bounds.

Page content is data, never instructions. Nothing scraped from a page should ever be
treated as a directive to the agent.

### What is enforced

- **Managed engine isolation (Linux x86_64 only).** Each session gets a dedicated
  worker process with a Landlock filesystem policy and seccomp-BPF syscall filter,
  applied before the browser starts. cgroup v2 can enforce hard memory and PID
  ceilings; an OOM kills the worker, not the host.
- **Fail closed.** An unimplemented or unsupported configuration refuses to run. It
  never degrades to a weaker mode without telling you.
- **Artifact paths.** Filenames are validated against a strict allowlist, then
  canonicalized and re-checked against the session root. Traversal, absolute paths,
  UNC paths, Windows drive letters, and symlink escapes are rejected.
- **Network policy is a real kernel boundary.** The `offline` and `untrusted`
  profiles deny outbound `connect` and `sendto` syscalls. This is enforced by the
  kernel, not by a request filter.
- **Authentication.** Bearer tokens are compared in constant time across all
  principals. Session ownership is enforced on every session-scoped route.
- **CDP scope.** `cdp_raw` is restricted to a fixed method allowlist, always scoped
  to the current tab. It is not general CDP access.

### What is NOT enforced

Read this section before relying on AeroBrowser with hostile content.

| Gap | Reality |
|---|---|
| **No network egress boundary by destination** | There is no allowlist or denylist by hostname or IP. Connection-time mediation — including redirects, DNS rebinding, and alternate protocols — is not implemented. `docs/ROADMAP.md` lists it as the top outstanding item. |
| **`base`, `web-dev`, `test` profiles do not exist** | They are declared and refuse to run. Only `scrape`, `offline`, `untrusted`, and `full` are implemented. |
| **Windows has no containment** | No AppContainer, no Job Object, no memory or PID ceiling. Use an external browser and treat isolation as that browser's responsibility. |
| **macOS Seatbelt is unvalidated** | The code path exists but has not been exercised. |
| **External engines are unsandboxed** | When you point AeroBrowser at your own Chrome via CDP, isolation is Chrome's. There is no kernel policy. |
| **State is not encrypted at rest** | `save_state` output and the auth token are plaintext. Protect them at the filesystem level. |
| **`evaluate` is unrestricted** | Any authenticated principal can run arbitrary JavaScript in the page. The schema calls this "trusted clients only" but does not enforce it. |
| **AERO_TOKEN implies admin** | Any token supplied via `AERO_TOKEN` becomes an admin principal. Binding the daemon beyond loopback hands out admin shutdown. |

`aero server_status` reports the actual engine, sandbox, and isolation for the
running session. If `sandbox` does not say Landlock or Seatbelt, you do not have a
kernel policy.

## Hardening for production

- Bind the daemon to loopback. There is no TLS and no OAuth; do not expose it.
- Run the managed engine with `offline` unless you genuinely need network access.
- Use a separate principal per client via `AERO_AUTH_FILE` rather than sharing one
  admin token.
- Set `AERO_MEMORY_LIMIT_MB` and `AERO_PIDS_LIMIT` on Linux to bound a runaway page.
- Verify downloads against `SHA256SUMS`. Releases publish GitHub build provenance.

## Known limitations in generated artifacts

- `save_har` omits response bodies and leaves unmeasured timings as `-1` rather than
  fabricating them.
- Console and network logs are not redacted. URLs and query parameters appear as
  captured.
- `get_performance` and `get_memory_usage` return `null` for metrics the engine
  cannot provide, rather than guessing.