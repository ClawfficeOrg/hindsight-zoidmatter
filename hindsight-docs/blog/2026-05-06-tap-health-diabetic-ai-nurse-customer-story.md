---
title: "How Tap Health Built an AI Nurse That Remembers"
authors: [afifahmed, chrislatimer]
date: 2026-05-06
tags: [hindsight, self-hosted, healthcare, customer-story, memory, agents, mental-models]
description: "Tap Health uses self-hosted Hindsight to power an AI nurse for Type-2 diabetic patients. Early data shows a downward glucose trend in roughly 60% of users."
image: /img/blog/tap-health-customer-story.png
hide_table_of_contents: true
---

![How Tap Health Built an AI Nurse That Remembers](/img/blog/tap-health-customer-story.png)

When Tap Health's engineering team interviewed users of their AI nurse, the same complaint kept surfacing.

> "I just told you yesterday I don't like mushrooms. Why are you recommending them today? Do I have to tell you every time?"

<!-- truncate -->

The product was an AI assistant for Type-2 diabetic patients. Most of the people using it were on a long, careful journey: tracking meals, logging glucose, building habits. The agent could answer well in any single conversation. It could not carry anything between them.

That gap turns out to be fatal in healthcare. "It's a critical AI agent," Tap Health engineering lead Afif Ahmed says. "We have medical stuff here. It has to remember symptoms, what someone has eaten in the last week, how their glucose has trended. Otherwise the personalization isn't real."

This is the story of how Tap Health rebuilt their agent around a memory layer, what they learned along the way, and what changed for their patients.

## The Before: A Multi-Agent System Without Memory

Tap Health's first version was a single LangChain agent. As the product grew, the team moved to a multi-agent system: an orchestrator with five sub-agents, one each for meal logging, glucose logging, consolidation, summarization, and review.

It was slow. More importantly, it had no shared memory.

The sub-agents weren't a memory fix; they were a parallelism fix. Each one ran fresh on every interaction. Whatever the meal-logging agent learned about a patient's preferences vanished as soon as it returned a result. The orchestrator had no continuity to pass to the glucose agent. Yesterday's spike from a high-carb meal had no way to inform today's recommendation.

User research told the team what the architecture already implied: patients felt like they were starting over every conversation.

## Why Tap Health Chose Hindsight

When the team began evaluating memory systems, most of them looked similar from the outside. Retain a fact, recall it later. The differentiator that mattered for medical use was synthesis — the ability to consolidate many observations into a working understanding of a patient over weeks and months.

> "Mental models was different. Others were having remember and recall, but Hindsight had mental models that consolidated from all the observations."

That mattered because diabetes management is not a question of any single fact. It is a long pattern. A 10-point drop in two weeks. A 20-point drop in four. Energy levels that correlate with adherence. Spikes that follow specific foods. The agent needs to recognize the trend, not just remember the data points.

Self-hosting was a hard requirement — patient data could not sit on infrastructure Tap Health did not control — and Hindsight's open-source path supported that out of the box.

## The New Architecture

The team collapsed the multi-agent setup into a single agent with tools and skills. Around it, they built a sophisticated nudge system: a 10-gate pipeline that decides whether it's the right time to send a message, whether the right skill is being invoked (glucose logging? meal logging? an emotional check-in?), and what to ask.

During each conversation, an asynchronous observation tool watches what the patient says and does, then retains the relevant pieces to Hindsight. Consolidation runs on hooks (when specific patient events fire) and on a weekly cadence to keep mental models current.

The deployment is straightforward by design. Hindsight runs self-hosted, backed by Cloud SQL Postgres. After looking at Hindsight's published benchmarks, Tap Health initially used a 120-billion-parameter model for retain and a 20-billion-parameter model for recall. Cost-conscious, they tested moving both to the 20B model and kept it.

> "The 20B model captures most of the observations. For a critical medical agent, that's what matters."

## What Tap Health Learned

Two things were not obvious from the documentation.

The first was the right structure for mental models. "How efficiently can we recall? Do we need one mental model? Five? Seven?" The team iterated on a few configurations before landing on what worked for their domain.

The second was a memory anti-pattern they walked into early. They had been pre-summarizing each conversation in their own pipeline, then sending the summary to Hindsight to retain.

> "That's the anti-pattern. You need to let the LLM decide. Let it summarize. Let it extract the memories."

Once they stopped pre-processing and let Hindsight's retain pipeline do the extraction, the quality of what landed in the bank — and what came back during recall — improved noticeably.

A smaller lesson, but one worth flagging: tag discipline matters. Tags must match exactly during consolidation, or the observations don't get pulled together into mental models. The team stripped non-deterministic tags from their pipeline once they understood how strict the matching is.

## Early Results

These are early numbers, captured a few months after the new architecture went live.

About 60% of users have shown a downward trend in their glucose readings. Engagement is up — patients log meals and glucose more consistently because the nudges feel personalized to their actual lives, not generic. Habits are forming, which is the underlying outcome diabetes management depends on.

> *[PM-supplied metrics — adherence rates, time-to-stable-glucose, retention deltas — to be added during review.]*

For an AI nurse, none of those numbers are guaranteed by the model alone. They come from the agent being able to remember.

## What's Next

Tap Health is continuing to iterate on mental model structure as they accumulate more patient data and surface new patterns. The longer the agent runs, the more there is for it to learn from.

More broadly, the team sees memory as the layer that lets healthcare AI move from impressive demos to durable patient relationships. A nurse — human or otherwise — is only useful if they remember you between visits.

---

*Tap Health is rolling out memory-driven AI nursing to Type-2 diabetic patients, self-hosted on their own infrastructure. To explore Hindsight for your own product, see [the quickstart](https://hindsight.vectorize.io/developer/api/quickstart) for self-hosted deployment, or [Hindsight Cloud](https://ui.hindsight.vectorize.io/signup) for the fastest path.*
