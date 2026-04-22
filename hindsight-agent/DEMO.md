# Marketing SEO Agent — Self-Learning Demo

An SEO marketing agent that starts with industry best practices, learns your preferences, and adapts to real performance data — all through Hindsight's memory system.

## Prerequisites

- Hindsight API running (`http://localhost:8888`)
- Hindsight Control Plane running (`http://localhost:8889`)
- OpenClaw gateway installed
- `hindsight-agent` CLI installed: `cd hindsight-agent && uv tool install -e .`

## Setup

```bash
hindsight-agent setup marketing-seo \
  --bank-id demo-marketing-seo \
  --harness openclaw \
  --template ~/dev/nicolo-agents/marketing-seo-blog-posts/template.json \
  --content ~/dev/nicolo-agents/marketing-seo-blog-posts/content
```

Then restart the OpenClaw gateway:
```bash
openclaw gateway restart
```

This creates everything in one shot:
- Bank with reflect/retain missions tuned for SEO content
- 3 knowledge pages: SEO Best Practices, Content Performance, Editorial Preferences
- SEO specialist reference doc ingested (async)
- OpenClaw agent with the skill auto-loaded at session startup
- Retain plugin configured

## Step 1: Consolidate and show best practices in UI

The SEO reference doc was ingested during setup. Trigger consolidation so observations are extracted and pages populate:

```bash
curl -X POST http://localhost:8888/v1/default/banks/demo-marketing-seo/consolidate
```

Wait ~30-60s for consolidation + page refresh.

**Show in UI:**
1. Open Control Plane → `http://localhost:8889`
2. Select bank `demo-marketing-seo`
3. Go to Mental Models tab
4. Select Knowledge Base: `knowledge`
5. Click "SEO Best Practices" → page shows synthesized rules from the reference doc (meta descriptions, title tags, schema markup, content structure, etc.)
6. "Content Performance" and "Editorial Preferences" are empty — no user data yet

**Talking point:** "The agent starts with industry best practices. These pages were pre-configured by the template and auto-populated from a reference document. But they're not static — they'll evolve."

## Step 2: Chat and show user preferences captured

Open OpenClaw chat with the `marketing-seo` agent:

```
You: Write me a blog post outline about vector databases for RAG
```

Agent produces an outline following SEO best practices from its knowledge.

```
You: Keep posts to 800 words max. Our audience is senior engineers — skip intro fluff, 
go straight to the technical meat. Always use comparison format when possible, our 
readers love side-by-side analysis. And never use the word "leverage" — I hate it.
```

Agent acknowledges and adjusts.

Now trigger consolidation to process this conversation:

```bash
curl -X POST http://localhost:8888/v1/default/banks/demo-marketing-seo/consolidate
```

Wait ~30s.

**Show in UI:**
1. Go to Mental Models → "Editorial Preferences"
2. Page now shows: "800 words max", "senior engineer audience", "comparison format preferred", "no intro fluff", "never use 'leverage'"
3. Click "Based on" to show the observations that fed this page

**Talking point:** "The user's preferences were captured from a natural conversation — no forms, no config files. The system extracted observations and the page synthesized them automatically."

## Step 3: New blog post follows preferences

In the same or new session:

```
You: Write me a blog post about reranking strategies in RAG pipelines
```

Agent should produce:
- ~800 words (learned)
- Comparison format (learned)
- Technical, no fluff (learned)
- SEO rules applied: proper H1/H2, keyword in first 100 words, meta description, etc. (from best practices page)
- No "leverage" anywhere

**Talking point:** "The agent combined static best practices with learned preferences. It didn't need to be told again."

## Step 4: Feed performance stats

Now simulate analytics data:

```
You: Here are the content performance stats from last month:

- "BM25 vs Semantic Search" (comparison post): 12,400 views, 3.2% CTR, 4:30 avg time on page
- "What is RAG" (explainer post): 4,100 views, 1.1% CTR, 1:45 avg time on page  
- "Building a RAG Pipeline" (tutorial): 8,200 views, 2.8% CTR, 5:10 avg time on page
- Posts with FAQ schema markup: +40% search impressions vs without
- Short titles (under 50 chars): 2.1x CTR vs long titles
- Posts published Tuesday/Wednesday: 30% more organic traffic than other days

Key takeaway: comparison posts are crushing it. Tutorials do well too. Pure explainers underperform.
```

Agent acknowledges and analyzes.

Trigger consolidation:

```bash
curl -X POST http://localhost:8888/v1/default/banks/demo-marketing-seo/consolidate
```

Wait ~30s.

**Show in UI:**
1. Go to Mental Models → "Content Performance"
2. Page now shows specific numbers: "comparison posts 3x views vs explainers", "FAQ schema +40% impressions", "short titles 2.1x CTR", "Tuesday/Wednesday best publish days"
3. Go to "SEO Best Practices" → if updated, it now notes: "short titles (<50 chars) perform better for us, adjusting from standard 50-60 char recommendation"

**Talking point:** "The system doesn't just store preferences — it integrates real performance data. The mental model now reflects what actually works, not just industry dogma. And it noted the deviation from standard advice."

## Step 5: New blog post follows stat-driven suggestions

New session or continue:

```
You: Write me a blog post about embedding models for RAG — which ones to use and when
```

Agent should produce:
- Comparison format (data says 3x better)
- ~800 words (preference)
- Short title under 50 chars (data says 2.1x CTR)
- FAQ schema included (data says +40% impressions)
- Technical tone for senior engineers (preference)
- Proper SEO structure (best practices)
- Will likely recommend publishing on Tuesday or Wednesday

**Talking point:** "This post wasn't written from a static template. Every decision — format, length, title style, schema — is backed by either the user's stated preferences or actual performance data. And this keeps evolving with every conversation and every new data point."

## Resetting the demo

```bash
# Delete everything
curl -X DELETE http://localhost:8888/v1/default/banks/demo-marketing-seo
openclaw agents delete marketing-seo --force

# Remove from config
python3 -c "
import json; p='$HOME/.hindsight-agent/config.json'
d=json.load(open(p)); d['agents'].pop('marketing-seo',None)
json.dump(d,open(p,'w'),indent=2)
"

# Re-run setup
hindsight-agent setup marketing-seo \
  --bank-id demo-marketing-seo \
  --harness openclaw \
  --template ~/dev/nicolo-agents/marketing-seo-blog-posts/template.json \
  --content ~/dev/nicolo-agents/marketing-seo-blog-posts/content
```

## What to highlight in the UI

| Moment | What to show |
|--------|-------------|
| After setup + consolidation | SEO Best Practices page populated from reference doc |
| After user feedback | Editorial Preferences page with extracted preferences |
| After stats | Content Performance page with specific numbers and patterns |
| On any page | "Based on" section → observations that fed the synthesis |
| KB selector | Filter to "knowledge" KB to see only this agent's pages |
| Source query | Visible on each page — the question driving synthesis |
| Auto-refresh badge | Pages marked for automatic update after consolidation |
