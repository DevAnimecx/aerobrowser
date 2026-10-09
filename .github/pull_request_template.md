## What this changes

<!-- One or two sentences. Link the issue it closes. -->

Closes #

## Type

- [ ] Bug fix
- [ ] New tool
- [ ] New engine or platform support
- [ ] Documentation
- [ ] Build, CI, or installer
- [ ] Security hardening

## For a new MCP tool

- [ ] Added to `src/schema.rs` with a description an agent can act on
- [ ] Added to `docs/tools.json`
- [ ] Has a unit test for validation and error paths
- [ ] Documented in `../README.md` and `../docs/API.md`
- [ ] `schema::tools()` count assertion updated

## For a sandbox or network change

- [ ] Fails closed on unsupported platforms
- [ ] `../docs/SECURITY.md` updated, including any new limitation
- [ ] `../SECURITY.md` reviewed for trust-boundary impact

## Verification

All of these must pass:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
python3 scripts/check-encoding.py
python3 scripts/check-docs.py
python3 scripts/check-metadata.py
```

- [ ] Ran the full suite locally
- [ ] Added a test that fails without this change

## Honesty check

- [ ] No capability claim in the README that the code does not implement
- [ ] No test counts or numbers quoted unless produced by a script in this repo
- [ ] New limitations documented rather than omitted