# API and test language — 0.5

`aero tools` and `docs/tools.json` are generated from the same source as the server's schema, so neither can drift from the implementation. There are **62 tools**.

## Tool groups

- Navigation: `navigate`, `snapshot`, `reload`, `go_back`, `go_forward`, `wait_for`, `scroll`.
- Interaction: `click`, `double_click`, `right_click`, `fill`, `type`, `press_key`, `hover`, `drag`, `upload_file`.
- Elements: `get_text`, `get_attribute`, `get_bounding_box`, `get_computed_style`, `get_dom`, `get_title`, `get_url`, `evaluate`.
- Forms: `select_option`, `check`, `uncheck`, `set_viewport`.
- Media: `screenshot`, `annotate_screenshot`, `save_pdf`.
- Tabs: `list_tabs`, `new_tab`, `switch_tab`, `close_tab`, `close`.
- State: `save_state`, `restore_state`, `get_cookies`, `set_cookie`, `clear_cookies`.
- Test: `test_run`, `test_assert_text`, `test_assert_attr`, `test_assert_url`, `test_assert_visible`, `test_report`.
- Network and diagnostics: `intercept_request`, `get_network_log`, `save_har`, `get_console_logs`, `get_performance`, `get_memory_usage`, `set_user_agent`, `set_geolocation`.
- Management: `server_status`, `server_shutdown`, `server_profile_list`, `server_profile_set`, `server_logs`.
- Protocol: `batch`, `cdp_raw`.

## Important contract details

**Batching:** `batch` takes `calls` (an array of `{name, arguments}`), `stop_on_error` (default `true`), and `max_calls` (default and ceiling 50). Calls run in order against one browser context. Every call is validated against the same schema as a standalone `tools/call`, so a bad argument fails before reaching the browser. Results are per call with `ok`, `result` or `error`, and `duration_ms`; the envelope reports `requested`, `executed`, `failed`, `stopped_early`, and `duration_ms`. A malformed call shape — a missing `name`, or non-object `arguments` — is a batch-level error rather than a per-call result. `batch` cannot contain itself.

**Delta snapshots:** `snapshot` accepts `delta`. Each element is fingerprinted on role, accessible name, depth, and disabled state, so a re-render with identical semantics produces no output while a rename or role change does. The response carries `elements` (added), `changed`, `removed` (refs no longer present), and a `delta` count object. Three cases deliberately return a **full** snapshot with `delta.baseline: false` and a `reason`: no previous snapshot on this tab, the document changed, or the URL changed. Deltas are tracked per tab and reset on navigation.

**Locators:** a ref or a CSS selector, not both. No full frame/shadow-root locator traversal. Ref actions validate document identity. Snapshot generation, active tab and session define a ref's scope. Snapshot filters include interactive_only, compact, max_nodes, max_depth, selector, bounded cursor_interactive scanning, and delta. Snapshot names can be truncated, and output reports truncation.

**Actions:** click uses real CDP mouse events at element center, not full Playwright-style actionability/occlusion checks. Fill uses the native value setter plus input/change. Type uses CDP insertText. Drag dispatches native CDP mouse input across 20 movement steps; it never constructs DOM DragEvents. Both element centers must be visible. The tool reports dispatched input, not successful application drop; HTML5 drop support is engine-dependent. Keyboard chords support common Control/Alt/Meta/Shift combinations and navigation keys; not every platform shortcut is emulated.

**Waits:** navigate supports domcontentloaded, load and networkidle. Network idle uses the current CDP session's inflight network events and a 500 ms quiet interval; long-lived requests can cause timeout. Visible checks geometry and CSS visibility, not visual occlusion.

**Timeout layering:** three independent deadlines apply. A per-request deadline (`AERO_REQUEST_TIMEOUT_MS`, default 120 s, range 100–900000) covers the whole MCP request; a 30 s deadline covers each CDP call; and `navigate` waits up to 15 s for readiness. `navigate.timeout_ms` and `wait_for.timeout_ms` are additionally capped at 30000 by their schemas. On request-deadline expiry the session **disposes its browser context** to guarantee cleanup, so any in-flight browser action is abandoned with it. Individual tool timeouts alone do not cancel browser work.

**Images/PDF:** screenshot returns image data; selectors/JPEG quality/full-page capture are supported in the ordinary screenshot path. Annotation is a separate temporary DOM overlay path, with PNG output and cleanup. Annotation combined with other screenshot options is not a fully supported visual contract. PDF supports A4/letter and landscape. Optional remote output paths are plain artifact filenames.

**State:** save_state returns a path and portable state object. restore_state accepts a state object or a filename within this session's artifact directory. Only supported cookies, tabs/URLs, localStorage, sessionStorage and scroll are restored. Storage is applied after navigation; an app may need reload. History, IndexedDB, CacheStorage, service workers and in-flight JS/connections are omitted. Restore can fail partially; it is not transactional.

**Network:** interception rules apply to the current tab, newest matching wildcard rule wins. Actions are block/mock/log/clear. Mock body is a string with optional status/headers; operator AERO_ALLOW_NETWORK_MOCKS=1 is required. Event queues are bounded. HAR bodies are omitted, redirect hops are preserved, and timing subfields use observed CDP values or -1 for unknown. Incomplete requests are omitted and counted. Encoded transfer bytes are distinct from unknown body size. URLs/query parameters are not redacted. Raw network logs retain untrusted CDP event data and may contain secrets.

**Metrics:** FCP/navigation timings and available LCP are measured from page APIs. TTI is null, not guessed. Memory uses engine CDP counters when supported; fallback DOM traversal is labeled as current-document-only. Unsupported heap/listener metrics are null. Server RSS and worker RSS on Linux are individual process readings, not Chrome whole-tree accounting.

**Restricted raw CDP:** only the listed target-scoped methods are allowed. Browser.close, cross-target commands, storage-global commands and arbitrary IO are rejected.

**Management:** server_shutdown closes only the caller's browser/worker, optionally exporting state. It cannot terminate peers. server_profile_set can tighten to offline/untrusted only before a worker starts; it does not relax a running sandbox. server_logs returns metadata for this session, not other clients' logs. The operator's `aero stop` is a separate admin action.

**Assertions:** a returned `passed:false` is a test failure, even when the tool invocation itself completed successfully. `test_run` returns passed/failed/skipped counts; CLI test exits nonzero for failed runs.

## MCP transports

Stdio: bounded newline-delimited JSON-RPC 2.0; stdout contains protocol output only, stderr diagnostics. Input limit 256 KiB. Initialize, notifications/initialized, ping, tools/list, tools/call, resources/list/read and prompts/list/get are implemented. Negotiated versions: 2025-03-26, 2025-06-18, 2025-11-25.

Streamable HTTP: POST `/mcp` returns JSON or 202 for notifications. Initialize returns `Mcp-Session-Id`; later requests require it. Supplied protocol-version headers must match. DELETE disposes the session. GET `/mcp` returns 405; no standalone server-event stream is offered on this endpoint. Legacy SSE is separately supported through `/sse` and `/messages`.

Authenticated HTTP requests require `Authorization: Bearer TOKEN`; Origin-bearing requests are denied. Tokens must be at least 24 characters; the generated token is 44. `/health` is the one unauthenticated route, because clients poll it for liveness. A stopped or unavailable session actor returns HTTP 503 rather than 200 with an empty result. The server is not an OAuth authorization server. Per-request cancellation and resumable streams are absent; `batch` provides call-level batching within one request, which is not the same thing.

Resources: current snapshot, screenshot, DOM, performance, and server status. Prompts: test-script, debug-page and summarize-page. Page-derived content is always untrusted input.

## DSL grammar

One statement per line. Blank lines/full-line `#` comments ignored. Shell-style quoting is parsed as text, **never passed to a shell**. Up to 200 steps / 128 KiB, maximum eight nested conditionals. Syntax is validated before executing any step. No variables, loops or arbitrary program execution; extraction keys are output fields, not substitution variables.

```text
open URL [--wait CSS_SELECTOR [TIMEOUT_MS]]
back
forward
reload
snapshot
click TARGET
double_click TARGET
right_click TARGET
fill TARGET "text"
type TARGET "text" [DELAY_MS]
select TARGET "value"
check TARGET
uncheck TARGET
press KEY_OR_CHORD
hover TARGET
scroll up|down|left|right [PIXELS]
drag FROM_TARGET TO_TARGET
upload TARGET RELATIVE_UPLOAD_PATH
wait CSS_SELECTOR [TIMEOUT_MS]
wait_for load|domcontentloaded|networkidle [TIMEOUT_MS]
sleep MILLISECONDS
assert text TARGET "expected"
assert text TARGET contains "substring"
assert attr TARGET ATTRIBUTE "expected"
assert url "expected"
assert url contains "substring"
assert title "expected"
assert title contains "substring"
assert visible TARGET
assert hidden TARGET
assert count CSS_SELECTOR NUMBER
assert js "expression" "expected-string-form"
screenshot [FILENAME] [--full] [--annotate]
pdf FILENAME
log "message"
get text TARGET -> OUTPUT_KEY
get attr TARGET ATTRIBUTE -> OUTPUT_KEY
get url -> OUTPUT_KEY
get title -> OUTPUT_KEY
if visible TARGET
  STEPS
end
serve OPERATOR_CONFIGURED_NAME PORT
stop OWNED_PORT
```

A target is `@eGENERATION_INDEX` or a CSS selector. Missing conditional targets mean false; engine errors fail the run. First failing executed step stops execution; later steps are counted as skipped. Conditional control markers count as steps. A sleep is limited to 30 seconds.

`serve` selects a fixed argv/cwd/port entry from AERO_SERVE_CONFIG. It does not accept arbitrary shell strings or dynamically chosen args. All processes owned by a run are cleaned up on completion/failure. Existing listeners are not adopted or stopped.

## CLI

Common aliases map to the same tools: open, snapshot, click, fill, type, select, check/uncheck, press, hover, scroll, drag, upload, eval, get text/attr/url/title, screenshot, pdf, wait, tabs/tab/new-tab/close-tab, back/forward/reload, save-state/restore-state, cookies, cdp, status, logs, close. `aero call TOOL JSON` exposes the exact schema without CLI alias ambiguity.

Use `aero test FILE --report PATH` for a test run; `aero validate FILE` parses only. The CLI command tree is not byte-for-byte identical to the illustrative PRD examples. `aero start` is a foreground HTTP service; use an external supervisor for daemonization. `aero stop` requests authenticated administrative shutdown.

## 0.3 protocol additions
See [deployment](DEPLOYMENT.md) for principals and request budgets. `notifications/cancelled` works while a request is running on stdio, HTTP and legacy SSE. Cancellation invalidates the browser context; effects already performed are not rolled back. Requests execute FIFO with at most eight waiting requests. `-32800` is cancellation, `-32002` deadline, and `-32001` queue full. IDs need only be unique while in flight. Tool results include `structuredContent.result` in addition to MCP presentation content. Replies may arrive out of order: match their IDs.

**0.4 restore override:** restoration now stages a new browser context and initializes storage before application boot. Preparation failure keeps the old context; this supersedes older descriptions of after-load/partial state injection. See DEPLOYMENT.md for the exact boundary and exclusions.

## 0.5 additions

- **`batch`** and **`snapshot.delta`** as described above. `batch` is call-level batching inside one MCP request; it is not request cancellation or a streaming/resumable protocol, and neither of those exists.
- Tool count is 62. `aero doctor` is a CLI subcommand and is not an MCP tool.
- The `tools/list` assertion in `src/mcp.rs` pins the count, so adding a tool without updating it fails the build.

## Diagnostics

`aero doctor` runs entirely locally and reports: build and platform, home directory resolution, engine detection, local browser discovery, sandbox probe support, CDP and daemon port reachability, auth token validity and location, daemon liveness, and the resolved path plus configured state for all ten agent config locations.

Every failing check includes a `fix` string. `--json` emits the same data for tooling. Use it as the first step in any troubleshooting, and include it in bug reports.
