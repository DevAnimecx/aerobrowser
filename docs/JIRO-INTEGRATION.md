# Search with Jiro, verify with Delta Browser

Jiro Search and Delta Browser solve two different halves of the same job.

- **Jiro** searches and returns **cited claims** instead of a pile of links.
  It is open-source, self-hosted, and has 22 MCP tools, 9 search engines and
  12 platforms. See `github.com/DevAnimecx/jiro` — `pip install jirosearch`,
  then `jiro serve`.
- **Delta Browser** opens a real browser, snapshots the page, and extracts
  the exact text, so an agent can check a claim against its source before
  acting on it. It has 62 MCP tools, batched calls, and delta snapshots.

Neither tool replaces the other. Jiro finds and cites. Delta opens and proves.

## The workflow

1. **Search in Jiro.** Ask for claims, not links, and keep the source URL
   attached to every claim.
2. **Open the source in Delta.** Use Delta's `navigate` tool with the
   source URL from step 1, not a URL the model guessed.
3. **Snapshot, don't screenshot-read.** Use Delta's `snapshot` with
   `interactive_only: true` first, then `get_text` for the exact passage.
   Screenshots are for a human to eyeball; text is what an agent should
   quote back.
4. **Quote or reject.** If the page text supports the claim, quote it with
   its source. If it does not, say so and do not act on the claim.
   A claim that fails step 3 is not a fact, even if Jiro returned it.
5. **Batch the boring parts.** Where Delta supports it, use its `batch`
   tool to run navigate + snapshot in one MCP message instead of one
   round-trip per tool call.

## Example flow (illustrative, not a live run)

```
User: "What is Groq's free-tier request limit, with a source?"

Agent (Jiro):  returns a claim — "Groq: 1,000 requests/day" —
               cited to Groq's pricing/docs page.
Agent (Delta): navigate -> that source URL
               snapshot { interactive_only: true }
               get_text -> the passage stating the limit
Agent (user):  "Groq: 1,000 requests/day — source: <that page>,
               passage quoted above. Verified in the browser, not
               just from a search snippet."
```

If Delta cannot load the page, or the passage is missing, the agent must
report that failure instead of repeating the Jiro claim as verified.

## MCP setup (both servers, same agent)

Run each server the way its own README documents it — Delta via
`delta mcp` (see `docs/QUICKSTART.md`), Jiro via its own MCP server
(see the Jiro README). Do not merge their configs into one server:
they are separate processes with separate tool namespaces.

In your agent's MCP config, add both servers, then restart the agent.
Verify with Delta's `delta doctor` and with Jiro's own health check
before giving the agent a real research task.

## Safety limits — read these before you automate

- **Separate profile for untrusted pages.** Delta inherits the browser
  profile and login state of the browser it drives. Do not point it at
  your main logged-in profile to scrape untrusted sites.
- **Sandbox is Linux-only.** Landlock + seccomp + cgroup v2 isolation
  applies on Linux x86_64 with the managed engine. On Windows and macOS
  there is no kernel isolation, and Delta's `server_status` says so.
  Do not promise sandbox protection on those platforms.
- **No shared auth.** Jiro and Delta do not share credentials. A Jiro
  API key, if any, is not a Delta credential, and vice versa.
- **Do not auto-act on unverified claims.** Filling a form, sending a
  message, or spending money on the basis of a search snippet alone is
  exactly the failure this pairing is meant to prevent. Verify first,
  then act, and log the source you verified against.
- **Rate limits still apply.** Opening many source pages in a loop can
  hit a site's limits or look like scraping. Batch sensibly, respect
  robots/terms for the target site, and stop on blocks instead of
  retrying aggressively.
