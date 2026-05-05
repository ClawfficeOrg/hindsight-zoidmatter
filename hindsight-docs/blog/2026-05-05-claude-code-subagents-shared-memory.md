---
title: "Your Claude Code Subagents Don't Share What They Learn"
description: "Claude Code subagents (Plan, Explore, general-purpose, custom) each spawn fresh and discard everything they discover. Here's how to give them shared memory."
authors: [benfrank241]
date: 2026-05-05
tags: [memory, agents, hindsight, claude-code, subagents]
image: /img/blog/claude-code-subagents-memory.png
---

![Your Claude Code Subagents Don't Share What They Learn](/img/blog/claude-code-subagents-memory.png)

[Claude Code's subagent system](https://docs.claude.com/en/docs/claude-code/sub-agents) is one of the best things to ship in the harness layer this year. You can delegate work to specialized agents — `Plan` to think through a strategy, `Explore` to crawl the codebase, `general-purpose` to handle a multi-step task, or any custom subagent you define under `.claude/agents/`. Each one runs in its own context, with its own system prompt and tools, and reports back when it's done.

It's a clean delegation model. It is also completely amnesiac.

Every subagent invocation starts from zero. Whatever the subagent figures out — the file it found, the architectural pattern it noticed, the dead end it hit, the decision the user made mid-task — vanishes the moment it returns. The orchestrator gets back a final message. Everything else evaporates.

If you have ever launched the same `Explore` agent twice in a row to find a thing it already found, you have hit this. If you have ever watched two parallel subagents independently discover the same constraint, you have hit this. If you have ever wondered why your custom code-review subagent never seems to learn what your team actually cares about, you have hit this.

<!-- truncate -->

## TL;DR

- Claude Code subagents (Plan, Explore, general-purpose, and custom subagents under `.claude/agents/`) are powerful but stateless
- Each subagent spawn starts fresh — no memory of prior runs, no awareness of what sibling subagents have already discovered
- Only the final message returns to the orchestrator; intermediate exploration, decisions, and learnings disappear
- A shared memory layer (Hindsight on a single project bank) lets every subagent and the orchestrator read and write to the same memory
- What one subagent discovers, every subsequent subagent can recall — no more re-exploring, no more re-deciding
- [Hindsight](https://github.com/vectorize-io/hindsight) is the memory layer; the [hindsight-memory plugin](https://hindsight.vectorize.io/integrations) wires it into Claude Code's hook system automatically

---

## What Subagents Already Do Well

Subagents are not the problem. They are an answer to a real one.

A single Claude Code session has limits. The context window fills up. Long exploration tasks generate noise that crowds out the work. Specialized work — planning, searching, code review, security analysis — benefits from a focused system prompt that doesn't have to coexist with the orchestrator's general instructions.

Subagents solve all of that. They give you:

- **Fresh context per task.** Big exploration jobs don't poison the orchestrator's working memory.
- **Specialized prompts and tools.** A code-reviewer subagent can have stricter tool permissions and a tighter system prompt than the parent.
- **Parallelism.** Multiple subagents can run independently, returning their summaries when done.
- **A clean return protocol.** The orchestrator gets a final message it can act on. No babysitting.

That model works. It is also exactly the model that makes the memory gap unavoidable.

---

## What Disappears When A Subagent Returns

The subagent runs in its own loop. It reads files. It greps. It calls tools. It forms hypotheses. It rules things out. It makes intermediate decisions about what is worth pursuing. It writes a final message and exits.

The orchestrator receives the final message. Everything else is gone.

That includes:

- **The exploration trail.** Which files the subagent opened, which it ruled out, what it grepped for and didn't find.
- **The intermediate decisions.** "I tried approach X, it didn't work because of Y, so I switched to Z."
- **The implicit conventions discovered.** "All HTTP handlers in this repo use the `withAuth` wrapper, even though it's not in the README."
- **The dead ends.** The five things the subagent considered and rejected, which a future subagent might walk straight back into.

For a single one-shot task, none of that matters — the final message captures the relevant outcome. For any work that involves more than one subagent invocation over time, all of that is information you generated and then threw away.

---

## Where The Pain Shows Up

This is not theoretical. The patterns repeat:

### Two Explore agents, one codebase

You launch an `Explore` subagent on Monday to find every place the auth middleware is used. It returns a clean summary. You launch a different `Explore` subagent on Tuesday to find every place the user model is constructed. The second agent re-greps half the same files, re-discovers the same auth wrapper, and reports back as if for the first time.

The first agent's exploration could have shortcut the second agent's. Nothing carries over.

### Parallel subagents that collide

You spawn three subagents in parallel to investigate three related questions. Each one independently rediscovers the same architectural quirk in your codebase. You get three slightly different framings of the same observation in three returned messages, and the orchestrator has to reconcile them.

If they shared memory, the second and third agents would have started from "the first agent already noted this; what else is true?"

### The custom subagent that never learns your team's preferences

You wrote a `code-reviewer` subagent in `.claude/agents/code-reviewer.md`. It does a fine job on the first PR. On the second PR, it flags the same lint pattern your team explicitly decided not to enforce. On the third PR, it argues for a refactor pattern you rejected last sprint.

You can update the system prompt to capture those decisions. But you have to remember to do it, and you have to do it manually. The subagent itself learned nothing from the first two PRs.

### The orchestrator that has to re-instruct every time

The orchestrator knows things from the conversation. The subagent it just spawned knows nothing about that conversation. So the orchestrator has to repeat context in the subagent prompt — again, every time. That eats orchestrator tokens and still doesn't carry over to the *next* subagent.

---

## What Changes With Shared Memory

A shared memory layer — one bank, accessible to the orchestrator and every subagent it spawns — flips the model.

Now the picture looks like this:

- The orchestrator and all subagents read from the same memory before they start their work
- They write back what they learn — facts, decisions, preferences, patterns, dead ends
- The next subagent (or the next session entirely) inherits everything that was retained

The first `Explore` agent's findings are available to the second one. The custom `code-reviewer` subagent inherits the preferences your team accumulated over previous reviews. Parallel subagents stop colliding because each one starts from "here is the current shared understanding of the project," not "here is the prompt and the codebase, figure it out."

The orchestrator stops spending tokens re-explaining context to subagents — the subagent just recalls it from the bank.

This is not a small ergonomic win. It is the difference between subagents being a delegation primitive and subagents being a *team*.

---

## Setting It Up

The integration is intentionally low-effort. The [hindsight-memory plugin](https://hindsight.vectorize.io/integrations) for Claude Code uses the standard [hook architecture](https://docs.claude.com/en/docs/claude-code/hooks):

- `SessionStart` — health check on the memory bank
- `UserPromptSubmit` — auto-recall relevant memories before the model is called
- `Stop` — auto-retain anything worth keeping when the turn ends

Subagents inherit the same hooks. So when a subagent starts, it pulls relevant context from the same shared bank. When it stops, what it learned gets retained back to the same bank. No separate setup per subagent. No special wiring.

A typical project setup:

```yaml
# .claude/hindsight.yaml
bank: my-project
autoRecall: true
autoRetain: true
```

The `bank` is the shared identity — every subagent that runs in this project writes to and reads from `my-project`. If you want each user to have their own slice while still sharing project knowledge, the [memory bank reference](https://hindsight.vectorize.io/developer/api/memory-banks) covers per-user scoping patterns.

That is the entire setup. After a few sessions of normal work, you can run a subagent and watch it recall preferences and decisions it never personally encountered.

---

## A Concrete Before/After

Before:

> **Orchestrator:** Use the Explore agent to find every place we call the Stripe webhook handler.
>
> *(Explore agent grep, opens 12 files, returns: "Found 7 call sites. They're all in `src/billing/`. The handler signature is `(req, res) => Promise<void>`, and 5 of the 7 wrap calls in our `withSpan` tracing helper.")*
>
> **Orchestrator:** Now use the Explore agent to find every place we call the Slack webhook handler.
>
> *(Explore agent grep, opens 11 files, returns: "Found 4 call sites. They're all in `src/notifications/`. The handler signature is `(req, res) => Promise<void>`, and 3 of the 4 wrap calls in our `withSpan` tracing helper.")*

The second exploration learns nothing from the first. The shared `withSpan` pattern is rediscovered from scratch.

After:

> **Orchestrator:** Use the Explore agent to find every place we call the Slack webhook handler.
>
> *(Explore agent recalls from shared bank: "We have a project-wide convention of wrapping webhook handlers in `withSpan`, last observed in the Stripe webhook exploration. Confirm this holds.")*
>
> *(Explore agent grep, opens 4 files, returns: "Found 4 call sites in `src/notifications/`. Convention holds — 3 of 4 wrap in `withSpan`. The 4th is a known exception in the Slack retry path; flagging for review.")*

Same task, half the exploration, more useful answer. The second agent built on the first.

---

## What This Looks Like Across A Week

Memory effects compound. After a week of normal subagent use against a shared bank, you start to notice:

- Repeated explorations stop being repetitive
- The orchestrator stops needing to re-paste context into subagent prompts
- Custom subagents (code-reviewer, security-checker, doc-writer) get measurably better at matching your team's preferences without you editing their system prompts
- New sessions don't feel like cold starts — the project's accumulated understanding is already there

For solo work, it feels like the harness finally remembers. For team setups where multiple people use Claude Code on the same project bank, it feels like the agents are working off a shared brain instead of independent re-derivations.

---

## A Quick Note on What's Coming

The current model is: Hindsight retains and recalls memory; you (or your subagents) read and write it through the bank.

The next step is closer to genuinely self-improving agents. Soon, Hindsight will be able to **write directly back to the markdown files that shape agent behavior** — [`CLAUDE.md`](https://docs.claude.com/en/docs/claude-code/memory), custom subagent prompts under `.claude/agents/`, even [skills](https://docs.claude.com/en/docs/claude-code/skills). As the agent learns your team's conventions and decisions, those learnings get reflected into the static files the next session loads. You stop maintaining `CLAUDE.md` by hand. The agent maintains it for you, based on what it has actually observed.

That is one piece of a broader self-driving-agents push we will say more about soon. For now, the shared-memory layer is the part that's available today and the part that gives you the immediate compounding effect.

---

## Tradeoffs and Limits

Shared memory is not always the right choice. A few honest tradeoffs:

- **Bank scope matters.** A single bank shared across unrelated projects creates noise. Use one bank per project, or one bank per team where appropriate. The [memory bank reference](https://hindsight.vectorize.io/developer/api/memory-banks) covers the patterns.
- **Sensitive context.** If a subagent is reading customer data, think carefully about what gets retained. The plugin's retention is configurable; you can keep specific data out of the bank.
- **Single-shot work.** If you almost never use subagents and your sessions are isolated, the value is lower. Memory pays off when work is iterative.
- **The first few days.** A new bank is empty. The compounding effect kicks in once a few sessions have built it up — usually within a week of normal use.

These are not deal-breakers, just things to size for. For most Claude Code users running subagents regularly, the right answer is one project bank with auto-retain and auto-recall on.

---

## Recap

- Claude Code subagents are powerful but stateless — every spawn starts fresh
- Only the final message returns; everything the subagent learned in the loop disappears
- Without shared memory, sibling subagents collide and sequential subagents re-derive what their predecessors found
- One shared bank — Hindsight on the project — gives every subagent and the orchestrator a common, growing understanding
- The hindsight-memory plugin uses Claude Code's hook system, so subagents inherit memory access with no per-subagent wiring
- Self-improving behavior — including agents that update their own `CLAUDE.md` — is the next layer; shared memory is the foundation it sits on

Subagents are how Claude Code scales beyond one context window. Shared memory is how subagents stop being strangers to each other.

---

## Further Reading

- [The Missing Layer in Every Agent Harness](https://hindsight.vectorize.io/blog/2026/05/04/agent-harness-needs-memory) — the broader case for why harnesses need memory
- [Your Agent Is Not Forgetful. It Was Never Given a Memory.](https://hindsight.vectorize.io/blog/2026/04/23/your-agent-is-not-forgetful) — the foundational argument
- [Claude Code on Telegram: Pair-Programming from Anywhere](https://hindsight.vectorize.io/blog/2026/03/23/claude-code-telegram) — the cross-surface Claude Code setup; same shared-memory pattern at the surface level
- [Adding Persistent Memory to OpenClaw with Hindsight](https://hindsight.vectorize.io/blog/2026/03/06/adding-memory-to-openclaw-with-hindsight) — the companion harness integration; the same hook pattern applies directly to Claude Code
- [Memory banks reference](https://hindsight.vectorize.io/developer/api/memory-banks) — scoping patterns for projects, teams, and per-user banks

---

## Next Steps

- [Sign up for Hindsight Cloud](https://ui.hindsight.vectorize.io/signup) and add memory to Claude Code in minutes
- Read the [quickstart](https://hindsight.vectorize.io/developer/api/quickstart) for self-hosted deployment
- Browse the [integration guides](https://hindsight.vectorize.io/integrations) for Claude Code and other harnesses
- Configure [memory banks](https://hindsight.vectorize.io/developer/api/memory-banks) to match how your team works — one project bank, per-user banks, or shared team banks
