# Self-Improving Agent Skills: Research Findings

Research log from building and testing procedural memory for OpenClaw agents (April 2026). Goal: understand where local file-based memory breaks and where an external system like Hindsight is genuinely needed.

Related work: [Memento-Skills (arXiv:2603.18743)](https://arxiv.org/abs/2603.18743) — "Let Agents Design Agents". Similar premise (skills as memory, read-write reflective learning), converges on the same insight: the agent needs a persistent, evolving knowledge store outside its context window.

---

## Key Discovery #1: Capture and synthesis must be separated

We built an `agent-memory` skill where the agent maintains its own wiki of markdown files — one per topic, with evidence sections, git-tracked, indexed. The agent reads before acting and writes after responding.

**What worked:** The agent reads memory files reliably and applies them well (dedup, preferences, procedures). The evidence trail provides basic provenance. Git gives full history.

**What broke:** The agent forgets to write. The LLM's natural stopping point is after the visible response — post-response writes (update files, append activity log, git commit) get dropped ~30% of the time. We tried a mandatory checklist (`📝 Memory: [wrote: X | logged: Y | committed: Z]`) which helped but didn't eliminate the problem.

**The insight:** Asking the agent to both produce output AND maintain its memory in the same turn is unreliable. Capture must be infrastructure-level and deterministic. Synthesis can be LLM-driven but should happen asynchronously, off the critical path.

This is exactly what Hindsight's architecture does: auto-retain (deterministic hook on every turn) + consolidation (async LLM synthesis in the background).

## Key Discovery #2: The agent is an excellent reader but an unreliable writer

Reading memory and applying it to tasks works well. The failure is consistently on writes — the agent understands the rules, agrees with them, then doesn't execute the post-response steps. Each session's agent is stateless except for what it explicitly reads; writes are an afterthought that competes with the primary task for attention.

**Why this matters for architecture:** The agent should be read-only on memory. All writes should come from infrastructure (hooks, pipelines) or from explicit agent actions that are part of the task itself (not post-task cleanup).

## Key Discovery #3: Auto-creating knowledge pages from raw observations doesn't work

We built a `knowledge_base_update` pipeline that runs after consolidation: it reads the KB mission + recent observations + existing pages, asks an LLM whether new pages should be created, and creates them.

**What broke:** The observations include everything retained from conversations — agent tool usage, delivered news content, identity setup, user names — not just user preferences. No matter how strict the prompt, the LLM creates pages for "Open Source AI Models" (from a news delivery) or "Agent Identity" (from setup chatter). We tried:
- Strict prompt rules ("NEVER create pages for delivered content") — LLM ignores them
- Code-level observation filters (pattern matching) — fragile, wrong approach
- Requiring 3+ observations per topic — still creates junk from clustered noise

**The insight:** A cheap LLM reading decontextualized observations cannot distinguish signal from noise. The agent can, because it has the full conversation context and understands what matters. Auto-creation works in Karpathy's LLM Wiki model because the input is curated documents, not raw conversation transcripts full of noise.

## Key Discovery #4: Karpathy's LLM Wiki pattern maps cleanly but the orchestrator should be the agent, not a pipeline

Karpathy's pattern: raw sources → LLM-maintained wiki → schema. Three operations: ingest, query, lint. The LLM does all the writing.

Our mapping: session transcripts → Hindsight KB (mental models) → agent skill. The agent reads; the system writes.

**Where Karpathy's model breaks for us:** His model assumes curated document inputs where the LLM can identify topic boundaries. Our inputs are raw conversation transcripts where 80% of the content is noise (tool calls, formatting, agent self-talk). A pipeline LLM can't curate this well enough.

**The fix:** Let the agent decide what pages to create (it has context), let the system keep them updated (it has reliability). This is the split we converged on.

---

## Current Architecture

```
[Agent conversation]
    ↓
[auto-retain plugin hook] → captures every turn, deterministically
    ↓
[Hindsight bank] → raw retained content
    ↓
[consolidation] → extracts observations from raw content
    ↓
[refresh_after_consolidation] → each MM re-runs its source_query
    ↓
[updated mental model content]
    ↓
[Agent reads via CLI] ← mental-model list/get
```

### Who does what

| Role | Agent | System (Hindsight) |
|---|---|---|
| **Capture** | Nothing — auto-retain handles it | Hook fires on every `agent_end`, deterministic |
| **Create pages** | Decides what topics need a page, calls `mental-model create` with a `source_query` | Stores the page, schedules initial content generation |
| **Read pages** | `mental-model list --kb <kb>` + `mental-model get <id>` | Returns current content |
| **Update page content** | Nothing — acknowledges user feedback in one sentence so retain captures it | Consolidation extracts observations → MM refresh re-synthesizes page content |
| **Update page scope** | `mental-model update <id> --source-query "..."` when the scope needs changing | Applies the new query on next refresh |
| **Delete pages** | `mental-model delete <id>` when a page is redundant | Removes it |
| **Organize pages** | Groups pages into a Knowledge Base (KB) via `--kb` flag | KB is a named collection with a mission, used for grouping |

### Why each choice

**Agent creates pages, not a pipeline.** Because the agent has conversation context and knows what's a recurring concern vs noise. The pipeline only sees decontextualized observations and creates junk pages (Discovery #3).

**System refreshes pages, not the agent.** Because the agent forgets to write (Discovery #2). `refresh_after_consolidation` runs automatically — no post-response step to forget.

**`source_query` is the key abstraction.** It's a question the system re-asks on every consolidation. The agent writes it once when creating the page; the system runs it forever. The agent controls *what* gets synthesized; the system handles *when* and *how*.

**Direct CLI reads, not mounted files.** We tried `hindsight-mount` (dump MMs to disk as markdown files). Dropped it because: files go stale the moment they're written, need re-mounting after consolidation, add a sync problem. Direct `mental-model get` is always live.

**KB groups pages but doesn't auto-create.** `auto_create: false`. The KB is a namespace (news-feed vs discord-watch), not an orchestrator. The agent decides the structure.

**Auto-retain with `retainToolCalls: false`.** Captures every conversation turn including user feedback. Tool calls excluded to reduce noise in retained content — tool results (file reads, web searches) pollute the observation space and cause the consolidation + MM refresh to synthesize irrelevant content.

### What's implemented

| Component | Status |
|---|---|
| Knowledge Base entity (CRUD, migration, API, CLI) | ✅ |
| `--kb` flag on `mental-model list` and `mental-model create` | ✅ |
| `knowledge_base_update` pipeline step (auto-create disabled) | ✅ |
| `agent-knowledge` skill (read + create + update + delete pages via CLI) | ✅ |
| Auto-retain via openclaw plugin | ✅ |
| Consolidation → MM refresh pipeline | ✅ (existing Hindsight feature) |
| 16 tests (KB CRUD + relationships + pipeline) | ✅ |

### What's NOT implemented (known gaps)

- **Tag-scoped observation routing** — all observations in the bank are visible to all MMs. Tag filtering would let MMs scope to relevant observations only.
- **`mental_model_ids` on recall results** — agent can't yet discover which MM covers a topic via recall. Has to scan the list.
- **Cross-agent KB sharing** — shared topics across agents require a cross-bank mechanism.
- **Activity log extraction** — mechanical parsing of "what was delivered" from session transcripts. Currently the agent has to notice and remember this itself.
- **Per-statement provenance** — each line in a MM tracing back to the observation(s) that produced it. The delta-mode MM work is heading here.

---

## Comparison: File-Based vs Hindsight-Backed

| Concern | Agent writes files | Hindsight KB + CLI reads |
|---|---|---|
| Capture reliability | Agent forgets ~30% of writes | Auto-retain hook, 100% reliable |
| Synthesis timing | Synchronous, blocks user's turn | Async (consolidation), off critical path |
| Read pattern | `cat ~/.agent-memory/topic.md` | `hindsight mental-model get <bank> <id>` |
| Staleness | Always current (agent just wrote it) | Minutes latency (consolidation cycle) |
| Scale | Index breaks at ~100 files | Semantic recall across any bank size |
| Agent complexity | Read + write + git + checklist | Read + create (one-time) |
| Infrastructure | Zero | Hindsight server + worker |
| Page creation quality | Agent decides (good) | Pipeline decides (bad) → switched to agent decides |

The async latency is the one trade-off. Within a session, the agent applies feedback from conversation context. Cross-session, the KB catches up via consolidation.

---

## Open Questions

1. **Can per-statement provenance work in practice?** Each line in a MM tracing to the observation(s) that produced it — "this rule came from turn 7 in session X". Requires the MM refresh to output cited fragments, not free text. The delta-mode work is the foundation.

2. **Should the agent also read via auto-recall injection?** Currently `autoRecall: false` — the agent reads pages via CLI. An alternative: the plugin injects relevant MM content into the system prompt at `before_prompt_build`, like it does with recalled memories. Zero agent effort, but burns context tokens.

3. **Will the source_query abstraction hold at scale?** With 3-5 pages it works well. At 50 pages, each MM refresh does a full reflect call — that's 50 LLM calls per consolidation. May need batching or incremental refresh.

4. **Cross-agent knowledge sharing.** User voice preferences, timezone, known tools — these apply to all agents. Need either a shared KB or cross-bank MM references.

5. **How does this compare to Memento-Skills' approach?** They let the agent rewrite skill files directly (with a judge + unit tests + rollback). We let the agent create pages but not edit content. Their approach is more autonomous but needs heavier infrastructure (judge, test gate, rollback). Ours is simpler but depends on the consolidation pipeline quality.
