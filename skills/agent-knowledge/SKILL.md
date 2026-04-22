---
name: agent-knowledge
description: Manage your long-term knowledge. Read existing topic pages before acting. Create new topic pages when you discover a recurring concern worth tracking across sessions. The system automatically keeps pages up to date from your conversations. Use when starting a task, when the user asks "what do you know about X", or when you realize a topic deserves its own persistent page.
---

# Agent Knowledge

Your knowledge is stored in Hindsight as topic pages (mental models) inside a Knowledge Base (KB). The system keeps pages updated automatically from your conversations. You **read** pages and **create** new ones when needed. You never edit page content directly — the system handles that.

**CLI:** `hindsight` (must be on PATH)
**API:** Wherever your Hindsight instance is running (check your environment or plugin config)

## Resolving your Bank ID and KB

Before you can use this skill, you need to know two things:
- **Bank ID** — the Hindsight bank where your memories are stored
- **KB ID** — the Knowledge Base within that bank (you create this yourself)

How you get your bank ID depends on your harness (OpenClaw, Claude Code, etc.) and its configuration. Figure it out from your environment — check plugin config, environment variables, or workspace files. If you truly can't determine it, ask the user once and remember the answer.

Once you know your bank ID, pick a KB ID that makes sense for your purpose (e.g., `knowledge`, `preferences`, `procedures`). Store both values somewhere persistent in your workspace so you don't re-derive them every session.

For the rest of this document, `$BANK` and `$KB` refer to these values.

## Mandatory startup sequence

Run these silently at the start of every session:

```bash
# 1. Resolve your bank ID and KB (see above) — skip if already stored from a prior session

# 2. Ensure the KB exists (no-op if already created)
hindsight kb list $BANK --output json | grep -q '"$KB"' || \
  hindsight kb create $BANK "$KB" --mission "Your KB mission description"

# 3. List your topic pages
hindsight mental-model list $BANK --kb $KB --output json
```

Read the pages relevant to the current task. If the list is empty, that's fine — create pages as you learn things (see below).

## Reading pages

```bash
# List all pages (names + content)
hindsight mental-model list $BANK --kb $KB --output json

# Read one specific page
hindsight mental-model get $BANK <page_id> --output json

# Search across all knowledge
hindsight memory recall $BANK "<query>" --output json
```

## Creating pages

When you discover a recurring topic worth tracking across sessions — user preferences, a procedure that works, a source list — create a page for it. Use your judgment, same as you would with a local file.

```bash
hindsight mental-model create $BANK \
  "<Page Name>" \
  "<source_query: a question that produces the page content from observations>" \
  --id <page-id> \
  --kb $KB \
  --trigger-refresh-after-consolidation
```

**The `source_query` is the key field.** It's a question the system will re-ask on every consolidation to rebuild the page content from your accumulated observations. Write it as a comprehensive question about what the user wants.

Example:
```bash
hindsight mental-model create $BANK \
  "Feed Source Preferences" \
  "What RSS feeds, websites, and sources does the user want included or excluded from their AI news feed, and in what priority order?" \
  --id feed-sources \
  --kb $KB \
  --trigger-refresh-after-consolidation
```

**When to create a page:**
- You've seen the same topic come up 2-3 times across turns
- The user stated a durable preference or rule you'll need next session
- You discovered a procedure that works and want to remember it

**When NOT to create a page:**
- One-off facts (just acknowledge and move on — the system retains the conversation)
- Things that are already covered by an existing page
- Agent internals, tool usage, or delivered content

## Updating a page's source query

If a page's scope needs to change — broader, narrower, or refocused — update its source_query. The system will re-synthesize the content on next consolidation.

```bash
hindsight mental-model update $BANK <page_id> \
  --source-query "Updated question about what the user wants..."
```

You can also rename a page:
```bash
hindsight mental-model update $BANK <page_id> \
  --name "Better Name"
```

## Deleting a page

If a page is redundant, outdated, or was a mistake — delete it:

```bash
hindsight mental-model delete $BANK <page_id>
```

Do this silently. Don't ask the user for permission to clean up your own knowledge.

## How pages stay current

1. Every conversation turn is automatically retained by the Hindsight plugin
2. The system extracts observations from your conversations
3. After consolidation, pages with `refresh_after_consolidation` re-run their source_query against the latest observations
4. Next time you read the page, the content reflects the latest user feedback

You don't need to update pages manually. Just acknowledge user feedback in one clear sentence so the retain pipeline captures it cleanly, and the page will update itself.

## Rules

- **Never edit page content directly** — the system synthesizes it from observations
- **Never ask the user about knowledge structure** — which pages exist, naming, organization. That's your decision, invisible to the user.
- **Create pages silently** — don't announce "I'm creating a page for X". Just do it.
- **Prefer fewer broader pages** — one "preferences" page is better than three narrow ones
