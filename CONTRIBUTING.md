# Contributing to AeroBrowser

First off â€” **thank you** for taking the time to contribute! â¤ï¸

This document explains how to set up and start contributing to AeroBrowser. We follow a few simple guidelines, and this project exists because of the [community](https://github.com/DevAnimecx/aerobrowser/discussions).

---

## Development Setup

### Prerequisites
- Rust 1.75+ (stable)
- OpenSSL 3.x development headers
- pkg-config
- For Linux engine builds: Landlock-capable kernel (5.13+)

### Clone & Build
```sh
git clone https://github.com/DevAnimecx/aerobrowser.git
cd aerobrowser
cargo build --release --locked
```

### Run Tests
```sh
cargo fmt --check
cargo test --locked
cargo clippy -- -D warnings
```

### Run with Test Browser
```sh
# Start Chrome with CDP
python3 -m http.server 4173 --bind 127.0.0.1 --directory examples &
google-chrome --remote-debugging-port=9222 --no-sandbox &
target/release/aero mcp --transport http
```

---

## How to Contribute

### Reporting Bugs
1. Check existing [issues](https://github.com/DevAnimecx/aerobrowser/issues)
2. Include: OS, AeroBrowser version, reproduction steps, error output
3. Add evidence (artifacts, HAR, logs) if possible

### Feature Requests
1. Start a [discussion](https://github.com/DevAnimecx/aerobrowser/discussions)
2. Explain the use case
3. The team will respond within 48 hours

### Pull Requests
1. Fork the repo
2. Create a branch (`git checkout -b feat/my-feature`)
3. Make changes with tests (`cargo test`)
4. Ensure CI passes (`cargo fmt --check`, `cargo clippy`)
5. Open a PR with a clear description

### PR Checklist
- [ ] Tests added or updated
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy` passes
- [ ] Documentation updated
- [ ] Breaking changes documented

---

## Coding Standards

- **Rust**: Follow `rustfmt` defaults, use `clippy` with `-D warnings`
- **Tests**: All new features must have unit tests
- **Documentation**: Public APIs require doc comments
- **No mocks**: Tests must use real browser behavior (mocks only with explicit opt-in)

---

## Architecture Guidelines

- Single binary, modular modules
- Async-first with tokio
- Fail-closed security defaults
- Schema-validated tool parameters
- Bounded resource usage (memory, FDs, events)

---

## Code of Conduct

By participating in this project, you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md).

---

## License

By contributing, you agree that your contributions will be licensed under the Business Source License 1.1. See [LICENSE](LICENSE) for details.

**Join the community:**
- [Discord](https://discord.gg/aerobrowser)
- [Discussions](https://github.com/DevAnimecx/aerobrowser/discussions)
- [Issues](https://github.com/DevAnimecx/aerobrowser/issues)

