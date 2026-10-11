# Contributing to Delta Browser

Thanks for taking the time to contribute.

## Development setup

### Prerequisites

- Rust stable (`rustup toolchain install stable`)
- Python 3.11+ for the check scripts and integration suites
- A Chromium-family browser for the integration suites
- For sandbox work: a Linux kernel with Landlock (5.13+) and cgroup v2

### Build

```sh
git clone https://github.com/DevAnimecx/deltabrowser.git
cd deltabrowser
cargo build --release --locked
```

`--locked` matters: every workflow uses it, so a stale or missing `Cargo.lock`
fails the build everywhere.

### Verify

All of these must pass before you open a PR. CI runs the same set.

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
python3 scripts/check-encoding.py    # rejects mojibake
python3 scripts/check-docs.py        # verifies internal links resolve
python3 scripts/check-metadata.py    # version and license agreement
```

`scripts/check-encoding.py` exists because nine files were once corrupted by a
bad encoding round trip, including both translated READMEs and the `--help` text.
`scripts/check-metadata.py` exists because the SBOM, LICENSE, and
THIRD-PARTY-NOTICES had contradicted each other. Both are cheap to run locally.

### Integration suites

The Python suites need a browser with a debugging port, a fixture server, and a
running daemon. There is no single command; run them in this order:

```sh
# 1. Terminal 1 — fixture server
python3 -m http.server 4173 --bind 127.0.0.1 --directory examples

# 2. Terminal 2 — browser
google-chrome --headless --no-sandbox --remote-debugging-port=9222 about:blank &

# 3. Terminal 3 — daemon and tests
export DELTA_TOKEN=$(openssl rand -hex 32)
export DELTA_UPLOAD_ROOT="$PWD/examples"
export DELTA_EXPECT_VERSION="0.0.1"
target/release/delta mcp --transport http &

# Wait for readiness
until curl --fail http://127.0.0.1:8931/health >/dev/null; do sleep 1; done

python3 tests/e2e_extended.py
python3 tests/transports_profiles_e2e.py
python3 tests/supervisor_e2e.py
python3 tests/stdio_cancel_e2e.py
```

The Moli, cgroup, and Landlock suites (`moli_e2e.py`, `sandbox_delegation_e2e.py`,
`sandbox_e2e.py`, `bundled_e2e.py`, `state_e2e.py`) require Linux with the
relevant kernel facilities and are run by a separate CI job that skips with a
notice when the kernel lacks them.

---

## Adding an MCP tool

1. Add the schema to `src/schema.rs`. The description is read by the agent, so
   write it for that audience, not for a human reading docs.
2. Implement dispatch in `src/browser.rs` or `src/extended.rs`.
3. Update `docs/tools.json`.
4. Update the count assertion in `src/mcp.rs` — the build fails if you forget.
5. Add unit tests covering validation and at least one error path.
6. Document it in `README.md` and `docs/API.md`.

---

## Standards

- `rustfmt` defaults, `clippy` with `-D warnings`.
- New features need unit tests. Several large modules still lack coverage; adding
  tests there is welcome.
- Public items get doc comments explaining what is guaranteed, not just what the
  code does.
- No mocks in tests, except the explicitly opt-in network mocks guarded by
  `DELTA_ALLOW_NETWORK_MOCKS=1`.
- Security defaults fail closed. An unimplemented policy errors rather than
  degrading.

---

## Honesty requirements

This project distinguishes between what is implemented and what is planned, and
that distinction has to survive review.

- Do not add a capability claim to the README that the code does not implement.
- Do not quote a test count, benchmark, or performance figure unless a script in
  this repository produces it.
- Document new limitations in `docs/ROADMAP.md` rather than omitting them.
- Changes to sandbox, auth, network policy, or artifact paths need a security
  review; see `.github/CODEOWNERS`.

Contributions are licensed under the Business Source License 1.1. See
[LICENSE](LICENSE).

## Code of conduct

Participation is governed by the [Code of Conduct](CODE_OF_CONDUCT.md).

Questions go to [Discussions](https://github.com/DevAnimecx/deltabrowser/discussions);
bugs to [Issues](https://github.com/DevAnimecx/deltabrowser/issues).