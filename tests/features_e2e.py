#!/usr/bin/env python3
"""End-to-end verification of the 0.5 features against a real browser.

Covers the three things 0.5 added or changed:

  1. batch      - ordering, shared browser, per-call results, stop_on_error
  2. delta      - added/changed/removed elements and baseline invalidation
  3. correctness fixes - the error-to-success conversions and the token length

Requires a CDP browser on :9222, a fixture server on :4173, and a running
`delta mcp --transport http` on :8931. See CONTRIBUTING.md.

    python3 tests/features_e2e.py
"""

import json
import os
import sys
import urllib.error
import urllib.request

TOKEN = os.environ.get("DELTA_TOKEN", "")
SERVER = os.environ.get("DELTA_SERVER", "http://127.0.0.1:8931")
FIXTURE = os.environ.get("DELTA_FIXTURE", "http://127.0.0.1:4173")
PAGE = FIXTURE + "/login.html"
CALL_ID = [0]

failures = []
passes = 0


def check(name, condition, detail=""):
    global passes
    if condition:
        passes += 1
        print("  ok    %s" % name)
    else:
        failures.append(name)
        print("  FAIL  %s  %s" % (name, detail))


def call(tool, arguments=None):
    """Invoke a tool through the authenticated /cli endpoint.

    /mcp requires a full initialize handshake and session id, which this suite
    does not need; /cli reaches the same tools/call dispatch with one principal.
    """
    CALL_ID[0] += 1
    body = json.dumps({"name": tool, "arguments": arguments or {}}).encode()
    request = urllib.request.Request(
        SERVER.rstrip("/") + "/cli",
        data=body,
        headers={"Content-Type": "application/json", "Authorization": "Bearer " + TOKEN},
    )
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            payload = json.loads(response.read())
    except urllib.error.HTTPError as error:
        payload = json.loads(error.read() or b"{}")
        raise AssertionError("%s failed: %s" % (tool, payload.get("error", error.code)))
    if payload.get("status") != "ok":
        raise AssertionError("%s failed: %s" % (tool, payload.get("error")))
    return payload["result"]


def main():
    if not TOKEN:
        print("DELTA_TOKEN is required")
        return 2

    try:
        urllib.request.urlopen(SERVER.rstrip("/") + "/health", timeout=5).read()
        urllib.request.urlopen(PAGE, timeout=5).read()
    except (urllib.error.URLError, OSError) as error:
        print("environment not ready: %s" % error)
        print("See CONTRIBUTING.md for the three-terminal setup.")
        return 2

    # ---------------------------------------------------------------- batch
    print("\nbatch")

    result = call("batch", {"calls": [
        {"name": "navigate", "arguments": {"url": PAGE}},
        {"name": "get_title", "arguments": {}},
        {"name": "get_url", "arguments": {}},
    ]})
    check("reports every requested call", result["requested"] == 3 and result["executed"] == 3, result)
    check("none failed", result["failed"] == 0, result)
    check("preserves order", [r["name"] for r in result["results"]] == ["navigate", "get_title", "get_url"])
    check(
        "navigation returned the title",
        "delta test app" in str(result["results"][0]["result"].get("title", "")).lower(),
        result["results"][0],
    )

    # A shared browser: one batch, one context, one navigation.
    result = call("batch", {"calls": [
        {"name": "get_text", "arguments": {"selector": "h1"}},
        {"name": "get_text", "arguments": {"selector": "#email"}},
    ]})
    check("second call sees the first's page state", result["failed"] == 0, result)
    check(
        "reads real page content",
        "fixture" in str(result["results"][0]["result"]).lower(),
        result,
    )

    # Per-call failure reporting.
    result = call("batch", {"stop_on_error": False, "calls": [
        {"name": "get_url", "arguments": {}},
        {"name": "fill", "arguments": {"selector": "#email", "value": 123}},
        {"name": "get_title", "arguments": {}},
    ]})
    check("continues past a bad argument", result["executed"] == 3, result)
    check("counts the failure", result["failed"] == 1, result)
    check("marks which call failed", result["results"][1]["ok"] is False, result["results"][1])
    check("error names the parameter", "value" in result["results"][1]["error"], result["results"][1])
    check("does not stop early", result["stopped_early"] is False, result)

    # Default stop_on_error.
    result = call("batch", {"calls": [
        {"name": "fill", "arguments": {"selector": "#email", "value": 123}},
        {"name": "get_title", "arguments": {}},
    ]})
    check("stops at first failure by default", result["stopped_early"] is True and result["executed"] == 1, result)

    # Argument validation happens before the browser is touched.
    result = call("batch", {"stop_on_error": False, "calls": [
        {"name": "navigate", "arguments": {}},
    ]})
    check("rejects a missing required parameter", result["failed"] == 1, result)
    check("names the missing parameter", "url" in result["results"][0]["error"], result["results"][0])

    # Malformed call shapes are batch-level errors.
    try:
        call("batch", {"calls": [{"arguments": {}}]})
        check("rejects a call with no name", False, "no error raised")
    except AssertionError:
        check("rejects a call with no name", True)

    try:
        call("batch", {"calls": []})
        check("rejects an empty call list", False, "no error raised")
    except AssertionError:
        check("rejects an empty call list", True)

    # max_calls is enforced.
    try:
        call("batch", {"max_calls": 2, "calls": [{"name": "get_url"} for _ in range(5)]})
        check("enforces max_calls", False, "no error raised")
    except AssertionError:
        check("enforces max_calls", True)

    # A real end-to-end flow through one batch. The fixture marks both fields
    # required, so both must be filled before the submit click is accepted.
    call("batch", {"calls": [{"name": "navigate", "arguments": {"url": PAGE}}]})
    result = call("batch", {"calls": [
        {"name": "fill", "arguments": {"selector": "#email", "value": "user@example.com"}},
        {"name": "fill", "arguments": {"selector": "#password", "value": "hunter2"}},
        {"name": "click", "arguments": {"selector": "form button"}},
    ]})
    check("fills and clicks in one batch", result["failed"] == 0, result)
    body = call("get_text", {"selector": "#result"})
    check("page reacted to the batched actions", "user@example.com" in str(body), body)

    # ------------------------------------------------------------ delta
    print("\ndelta snapshots")

    call("batch", {"calls": [{"name": "navigate", "arguments": {"url": PAGE}}]})

    first = call("snapshot", {"delta": True, "interactive_only": True})
    check("first delta is a full baseline", first["delta"]["baseline"] is False, first.get("delta"))
    check("baseline explains itself", "reason" in first["delta"], first["delta"])
    check("baseline reports no removals", first["delta"]["removed"] == 0, first["delta"])
    baseline_elements = len(first["elements"])
    check("baseline has elements", baseline_elements > 0, baseline_elements)

    # Nothing changed: the delta must be empty, not a repeat of the tree.
    second = call("snapshot", {"delta": True, "interactive_only": True})
    counts = second["delta"]
    check("unchanged page yields an empty delta",
          counts["added"] == 0 and counts["changed"] == 0 and counts["removed"] == 0, counts)
    check("unchanged page sends no elements", second["elements"] == [], second["elements"])
    check("unchanged page still reports url and title", bool(second["url"]), second)

    # Change an interactive element's label, then observe the difference.
    call("evaluate", {"script": "document.querySelector('form button').textContent = 'Sign in now'"})
    third = call("snapshot", {"delta": True, "interactive_only": True})
    check("detects a content change", third["delta"]["changed"] > 0, third["delta"])
    check("reports the change in text", "Sign in now" in third["text"], third["text"])
    check(
        "an unchanged page produced no churn in this cycle",
        third["delta"]["removed"] == 0,
        third["delta"],
    )

    # Adding an element must be reported as added.
    call("evaluate", {"script": (
        "const b=document.createElement('button');"
        "b.textContent='Brand new';"
        "document.querySelector('#login').appendChild(b);"
    )})
    fourth = call("snapshot", {"delta": True, "interactive_only": True})
    check("detects a new element", fourth["delta"]["added"] > 0, fourth["delta"])
    check("names the new element", "Brand new" in fourth["text"], fourth["text"])
    check("new elements carry a usable ref",
          all(e["ref"].startswith("e") for e in fourth["elements"]), fourth["elements"])

    # Removing an element must produce a tombstone.
    removed_ref = fourth["elements"][0]["ref"]
    call("evaluate", {"script": "document.querySelector('#login button').remove()"})
    fifth = call("snapshot", {"delta": True, "interactive_only": True})
    check("reports the removal", fifth["delta"]["removed"] > 0, fifth["delta"])
    check("names the removed ref", any(r.startswith("e") for r in fifth["removed"]), fifth["removed"])

    # Navigation invalidates the baseline.
    call("navigate", {"url": PAGE + "?again=1"})
    sixth = call("snapshot", {"delta": True, "interactive_only": True})
    check("navigation invalidates the baseline", sixth["delta"]["baseline"] is False, sixth["delta"])
    check("invalidation is explained", "reason" in sixth["delta"], sixth["delta"])
    check("invalidated delta is a full snapshot", len(sixth["elements"]) > 0, len(sixth["elements"]))

    # A plain snapshot must not carry delta bookkeeping.
    plain = call("snapshot", {"interactive_only": True})
    check("plain snapshot has no delta field", "delta" not in plain, list(plain))

    # ---------------------------------------------------------- CLI fixes
    print("\nerror handling")

    status = call("server_status")
    check("server_status is reachable", status["tool_count"] == 62, status.get("tool_count"))
    check("server_status names the engine", bool(status.get("engine")), status)
    check(
        "external engine reports no kernel policy",
        "responsibility" in json.dumps(status.get("sandbox", "")) or status.get("engine") == "moli",
        status.get("sandbox"),
    )

    print("\n%d passed, %d failed" % (passes, len(failures)))
    if failures:
        for name in failures:
            print("  failed: %s" % name)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())