# Third-party components

The AeroBrowser wrapper source is licensed under the Business Source License 1.1
with an Additional Use Grant — see `LICENSE` for the full terms, including the
revenue threshold and the conditions under which commercial use requires a paid
license. Dependencies retain their own licenses.

The optional bundled Linux executable contains the unmodified compressed upstream **Moli v1.1.12** release archive, pinned in `docs/engine-pin.json` (release revision and two SHA-256 digests). Source: https://github.com/lexmount/moli/tree/v1.1.12 . The upstream release includes MIT/Apache-2.0 notices plus Chromium and Selenium notices. Those files remain inside the embedded archive and are extracted alongside the cached engine. Copies are also included in `licenses/moli/` in this project.

The wrapper has not relicensed the embedded browser, V8 or their dependencies. Before publishing a redistribution, perform a full dependency-license and SBOM review. Lightpanda is not bundled in this release. A locally supplied Chrome/Chromium/Lightpanda executable remains separately licensed.

Configuration-reference sources:
- Moli project: [1](https://github.com/lexmount/moli)
- Hermes MCP configuration: [2](https://hermes-agent.nousresearch.com/docs/reference/mcp-config-reference)
- Codex TOML configuration reference used for the adapter: [1](https://www.gptmap.org/en/posts/codex-cli-mcp)

These references inform the adapters; they are not evidence that those clients were run in the validation environment.
