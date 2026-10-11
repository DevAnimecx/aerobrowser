# Migrating from AeroBrowser to Delta Browser

Delta Browser is the renamed and continued development of AeroBrowser. Both are
published by the same author under the same Business Source License 1.1, so
the licence terms did not change — only the names did.

There are **no compatibility shims**. A Delta Browser install reads only
`DELTA_*` variables and only `~/.delta/` state, so it can never be pointed at
an AeroBrowser-configured environment by accident. That is deliberate: a
silent fallback between two differently-behaved binaries is worse than a
clear error.

---

## What changed

| AeroBrowser | Delta Browser |
| --- | --- |
| `aero <command>` | `delta <command>` |
| crate `aerobrowser` | crate `deltabrowser` |
| `AERO_CDP` | `DELTA_CDP` |
| `AERO_SERVER` | `DELTA_SERVER` |
| `AERO_TOKEN` | `DELTA_TOKEN` |
| `AERO_TOKEN_FILE` | `DELTA_TOKEN_FILE` |
| `AERO_ENGINE` | `DELTA_ENGINE` |
| `AERO_ENGINE_PATH` | `DELTA_ENGINE_PATH` |
| `AERO_PROFILE` | `DELTA_PROFILE` |
| `AERO_CONFIG_PATH` | `DELTA_CONFIG_PATH` |
| `AERO_MEMORY_LIMIT_MB` | `DELTA_MEMORY_LIMIT_MB` |
| `AERO_PIDS_LIMIT` | `DELTA_PIDS_LIMIT` |
| `AERO_CGROUP_ROOT` | `DELTA_CGROUP_ROOT` |
| `AERO_REQUEST_TIMEOUT_MS` | `DELTA_REQUEST_TIMEOUT_MS` |
| `AERO_AUTH_FILE` | `DELTA_AUTH_FILE` |
| `AERO_SERVE_CONFIG` | `DELTA_SERVE_CONFIG` |
| `AERO_ARTIFACT_ROOT` | `DELTA_ARTIFACT_ROOT` |
| `AERO_UPLOAD_ROOT` | `DELTA_UPLOAD_ROOT` |
| `AERO_EVIDENCE_DIR` | `DELTA_EVIDENCE_DIR` |
| `AERO_ALLOW_NETWORK_MOCKS` | `DELTA_ALLOW_NETWORK_MOCKS` |
| `AERO_EXPECT_VERSION` | `DELTA_EXPECT_VERSION` |
| `AERO_BIN` | `DELTA_BIN` |
| `AERO_INSTALL_DIR` | `DELTA_INSTALL_DIR` |
| `AERO_BUNDLED_MOLI_ARCHIVE` | `DELTA_BUNDLED_MOLI_ARCHIVE` |
| `~/.aero/` | `~/.delta/` |
| `.aero-token.json` | `.delta-token.json` |
| `*.aero-skill.md` | `*.delta-skill.md` |
| `Install-AeroBrowser` | `Install-DeltaBrowser` |
| `mcpServers.aero` | `mcpServers.delta` |
| `aero-linux-x86_64` (asset) | `delta-linux-x86_64` |
| `examples/login.aero` | `examples/login.delta` |
| GitHub `DevAnimecx/aerobrowser` | GitHub `DevAnimecx/deltabrowser` |

### What did *not* change

The delta-snapshot **feature** is unrelated to the brand. These identifiers
are unchanged and mean exactly what they did before:

- the `snapshot` tool's `delta: true` argument
- `DeltaEntry`, `SnapshotDelta`, `delta_payload` in the source
- the `.aero` → `.delta` runtime rename does not affect any of them

---

## Migrate in four steps

### 1. Install Delta Browser alongside, not over

```sh
curl -fsSL https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/DevAnimecx/deltabrowser/main/scripts/install.ps1 | iex; Install-DeltaBrowser
```

The two binaries have different names, so both can be on `PATH` while you
check that nothing depends on the old one.

### 2. Re-point your agent config

Run:

```sh
delta connect <agent> --install
```

or, for any MCP client without an adapter:

```sh
delta agents --config
```

and paste the block it prints. The JSON key changes from `aero` to `delta`.

Before:

```json
{
  "mcpServers": {
    "aero": { "command": "aero", "args": ["mcp"] }
  }
}
```

After:

```json
{
  "mcpServers": {
    "delta": { "command": "delta", "args": ["mcp"] }
  }
}
```

### 3. Rename the environment variables

If you set any `AERO_*` variables in a shell profile, a `.env`, a systemd unit,
a Dockerfile, or a CI workflow, rename them. Anything still exported under the
old names is simply ignored — it will not error, it will just stop having any
effect, which is the quiet failure mode to watch for.

```sh
# find anything still setting the old names
grep -rn 'AERO_' . --include='*.yml' --include='*.yaml' --include='*.sh' \
  --include='*.ps1' --include='*.env' --include='Dockerfile*'
```

### 4. Verify

```sh
delta doctor
```

`doctor` reports the engine, the sandbox capability, the ports, the token, and
every agent config path it can find. Compare it against your old
`aero doctor` output to confirm nothing changed except the names.

---

## Rolling back

Uninstall is the reverse: remove `delta` from `PATH`, delete the MCP entry from
your agent config, and delete `~/.delta/`. Nothing outside those three places
is touched, and your AeroBrowser installation keeps working untouched.

```sh
rm -f ~/.local/bin/delta
rm -rf ~/.delta
```

---

## Why the version was reset to 0.0.1

AeroBrowser ended at 0.5.0. Delta Browser starts at 0.0.1 rather than 0.6.0
so that no Delta Browser release can be mistaken for, or compared against, an
AeroBrowser release number. The older releases stay in the git history and
remain available under the terms on which they were published.

See [CHANGELOG.md](../CHANGELOG.md) for the full timeline.
