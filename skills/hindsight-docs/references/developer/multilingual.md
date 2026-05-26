---
sidebar_position: 5
---

# Multilingual Support

Hindsight automatically detects the language of your input and responds in the same language. This means facts, entities, and reflect responses are preserved in their original language without translation to English.

## How It Works

```mermaid
graph LR
    A[Chinese Input] --> B[Language Detection]
    B --> C[Extract Facts in Chinese]
    C --> D[Chinese Entities]
    D --> E[Chinese Response]
```

When you retain content or reflect on a query, Hindsight:

1. **Detects the input language** automatically from the content
2. **Extracts facts in the original language** - preserving nuance and meaning
3. **Stores entities in their native script** - 张伟 stays 张伟, not "Zhang Wei"
4. **Responds in the same language** - queries in Chinese get Chinese answers

---

## Retain with Non-English Content

When you retain content in any language, Hindsight extracts and stores facts in that same language.

### Example: Chinese Content

```python
from hindsight import Hindsight

hindsight = Hindsight()

# Retain Chinese content
hindsight.retain(
    bank_id="user-123",
    content="""
    张伟是一位资深软件工程师，在腾讯工作了五年。
    他专门研究分布式系统，并领导了公司微服务架构的开发。
    """,
    context="团队概述"
)

# Query in Chinese - get Chinese results
results = hindsight.recall(
    bank_id="user-123",
    query="告诉我关于张伟的信息"
)

# Facts are returned in Chinese:
# - 张伟是一位资深软件工程师，在腾讯工作了五年
# - 张伟专门研究分布式系统，并领导了公司微服务架构的开发
```

### Example: Japanese Content

```python
hindsight.retain(
    bank_id="user-123",
    content="""
    田中さんはソフトウェアエンジニアで、東京のスタートアップで働いています。
    彼女はPythonとTypeScriptが得意で、毎日コードレビューをしています。
    """,
    context="チームプロフィール"
)

# Query in Japanese
results = hindsight.recall(
    bank_id="user-123",
    query="田中さんについて教えてください"
)
```

---

## Reflect with Non-English Queries

The `reflect` operation also respects the input language, generating thoughtful responses in the same language as the query.

### Example: Chinese Reflection

```python
# Store facts about team members (in Chinese)
hindsight.retain(
    bank_id="team-eval",
    content="张伟是一位优秀的软件工程师，完成了五个重大项目。他总是按时交付，代码整洁有良好的文档。",
    context="绩效评估"
)

hindsight.retain(
    bank_id="team-eval",
    content="李明最近加入团队。他错过了第一个截止日期，代码有很多bug。",
    context="绩效评估"
)

# Reflect in Chinese
result = hindsight.reflect(
    bank_id="team-eval",
    query="谁是更可靠的工程师？"
)

# Response is in Chinese:
# "我认为张伟更可靠。张伟完成了五个重大项目，按时交付，代码质量高..."
```

---

## Mixed Language Content

Hindsight handles mixed-language content gracefully, preserving both languages where appropriate.

### Example: Chinese Text with English Company Names

```python
hindsight.retain(
    bank_id="user-123",
    content="""
    王芳在Google北京办公室工作，她是一名高级产品经理。
    之前她在Microsoft和Amazon工作过。
    她负责管理YouTube在中国市场的推广策略。
    """,
    context="员工资料"
)

# Facts preserve both languages:
# - 王芳在Google北京办公室工作，担任高级产品经理
# - 王芳曾在Microsoft和Amazon工作过
# - 王芳负责管理YouTube在中国市场的推广策略
```

---

## Supported Languages

**Hindsight's multilingual support depends entirely on your LLM's language capabilities.** Hindsight instructs the LLM to detect the input language and respond in that same language. If your LLM supports a language, Hindsight will work with it.

Most modern LLMs (GPT-4, Claude, Gemini, Llama 3, etc.) support dozens of languages including:

- **East Asian**: Chinese (Simplified/Traditional), Japanese, Korean
- **European**: Spanish, French, German, Italian, Portuguese, Dutch, Polish, Russian
- **Middle Eastern**: Arabic, Hebrew, Turkish
- **South Asian**: Hindi, Bengali, Tamil
- **Southeast Asian**: Thai, Vietnamese, Indonesian

**To verify support for your target language**, test your LLM directly with content in that language. If the LLM can understand and generate text in the language, Hindsight will preserve it correctly.

---

## Configuring for Multilingual Use

For optimal multilingual performance, configure all four components of the pipeline:

### 1. LLM (Required)
Your LLM must support the target languages. Most modern LLMs do, but verify with your specific model.

### 2. Embedding Model (Recommended)
The default embedding model (`BAAI/bge-small-en-v1.5`) is **English-only**. For multilingual content, use a multilingual embedding model:

```bash
# In your .env file
HINDSIGHT_API_EMBEDDINGS_LOCAL_MODEL=BAAI/bge-m3
```

**Recommended multilingual embedding models:**
| Model | Languages | Notes |
|-------|-----------|-------|
| `BAAI/bge-m3` | 100+ | Best overall multilingual performance |
| `intfloat/multilingual-e5-large` | 100+ | Good alternative |
| `sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2` | 50+ | Lighter weight |

### 3. Reranker Model (Recommended)
The default reranker (`cross-encoder/ms-marco-MiniLM-L-6-v2`) is **English-only**. For multilingual content, use a multilingual reranker:

```bash
# In your .env file
HINDSIGHT_API_RERANKER_LOCAL_MODEL=BAAI/bge-reranker-v2-m3
```

**Recommended multilingual reranker models:**
| Model | Languages | Notes |
|-------|-----------|-------|
| `BAAI/bge-reranker-v2-m3` | 100+ | Best multilingual reranking |
| `cross-encoder/mmarco-mMiniLMv2-L12-H384-v1` | 14 | Lighter alternative |

### 4. BM25 / Full-Text Search Backend

The semantic (embedding) arm covers cross-lingual matches by meaning. Hindsight runs a BM25 keyword arm in parallel, and **BM25 is inherently within-language** — it's character/token matching against a tokenizer's lexemes. The default `native` backend uses PostgreSQL's English dictionary, which produces poor results for non-English content (and no useful tokenization at all for Chinese / Japanese / Korean, which lack whitespace word boundaries).

There are two knobs that interact:

- `HINDSIGHT_API_TEXT_SEARCH_EXTENSION` — selects the backend (`native`, `vchord`, `pg_textsearch`, or `pgroonga`).
- `HINDSIGHT_API_TEXT_SEARCH_EXTENSION_NATIVE_LANGUAGE` — selects the PostgreSQL dictionary used by the `native` backend (default: `english`).

Pick the backend based on the languages your bank stores:

| Backend | Multilingual / CJK | Notes |
|---------|--------------------|-------|
| `native` | European languages only (English, French, German, Spanish, Italian, Portuguese, Russian, Dutch, Swedish, Norwegian, Danish, Finnish, Hungarian, Turkish, Arabic, plus `simple`). CJK requires a third-party dictionary like `zhparser`. | Stock PostgreSQL — no extra extensions. Configure the language via `HINDSIGHT_API_TEXT_SEARCH_EXTENSION_NATIVE_LANGUAGE`. |
| `vchord` | Multilingual via `llmlingua2` tokenizer. | Best when you're already using vchord for vector search. |
| `pg_textsearch` | English only (hardcoded). | Industry-standard BM25 ranking + Block-Max WAND. |
| `pgroonga` | **Yes — out of the box.** Single index handles English, CJK, and mixed-script content via the `TokenBigram` polyglot tokenizer + `NormalizerNFKC150` Unicode normalization. | Recommended for non-English / mixed-language banks. Requires the `pgroonga` extension. See `docker/docker-compose/pgroonga/`. |

**Choosing for a single-language bank** (e.g. all Spanish content):
```bash
HINDSIGHT_API_TEXT_SEARCH_EXTENSION=native
HINDSIGHT_API_TEXT_SEARCH_EXTENSION_NATIVE_LANGUAGE=spanish
```

**Choosing for a CJK or mixed-language bank**:
```bash
HINDSIGHT_API_TEXT_SEARCH_EXTENSION=pgroonga
```

The `native` and `pgroonga` knobs do not apply to each other — `pgroonga`'s tokenizer is set at index creation and ignores `HINDSIGHT_API_TEXT_SEARCH_EXTENSION_NATIVE_LANGUAGE`.

#### Forcing the LLM Output Language

Independent from the BM25 backend, `HINDSIGHT_API_LLM_OUTPUT_LANGUAGE` forces every LLM-generated artifact into a single language regardless of the source content. This applies uniformly to:

- **Retain** — fact text, context, and entity names extracted from source documents.
- **Consolidation** — observations / mental models synthesized from those facts.
- **Reflect** — the final natural-language response returned by the reflect API.

```bash
# Every LLM call (retain, consolidation, reflect) emits Spanish regardless of source language.
HINDSIGHT_API_LLM_OUTPUT_LANGUAGE=Spanish
```

Common patterns:
- **Aligned, single-language bank**: `HINDSIGHT_API_TEXT_SEARCH_EXTENSION_NATIVE_LANGUAGE=spanish` + `HINDSIGHT_API_LLM_OUTPUT_LANGUAGE=Spanish` — store, index, and respond in Spanish even when sources are mixed.
- **Mixed-language bank with multilingual indexing**: `HINDSIGHT_API_TEXT_SEARCH_EXTENSION=pgroonga` + leave `HINDSIGHT_API_LLM_OUTPUT_LANGUAGE` unset — preserve source-language facts; pgroonga handles all of them in one index; reflect responds in the query's language.
- **Cross-lingual unification**: `HINDSIGHT_API_LLM_OUTPUT_LANGUAGE=English` — every fact, observation, and reflect response in English regardless of source. Useful when the consumer (an English-only LLM, dashboard, or downstream pipeline) needs uniform output.

Leave `HINDSIGHT_API_LLM_OUTPUT_LANGUAGE` unset to preserve the source/query language across the pipeline (the default).

#### Backfilling After a Language Change

Both `HINDSIGHT_API_TEXT_SEARCH_EXTENSION_NATIVE_LANGUAGE` and the bank's BM25 backend only affect newly-written rows — existing rows keep whatever lexemes were computed at their insert time. To re-index existing `native`-backend data in a new language:

```sql
UPDATE memory_units
SET search_vector = to_tsvector('<new_language>'::regconfig,
    COALESCE(text, '') || ' ' || COALESCE(context, '') || ' ' || COALESCE(text_signals, ''));
```

For backend switches (e.g. `native` → `pgroonga`), the safest path is an empty database; otherwise Hindsight will refuse to convert and ask you to clear `memory_units` and `reflections` first.

---

## Best Practices

### 1. Use Multilingual Models for Non-English Content
If you primarily work with non-English content, configure multilingual embedding and reranker models. English-only models will still store your content correctly, but semantic search quality will be degraded.

### 2. Keep Content in One Language Per Retain Call
While mixed content works, keeping each `retain` call in a single language produces more consistent results.

### 3. Query in the Same Language as Your Content
For best results, query using the same language as your stored content. Cross-language queries (e.g., English query for Chinese content) may work but results can vary depending on your embedding model.

---

## Technical Details

Multilingual support is implemented through LLM prompt instructions rather than external language detection libraries. This approach:

- **Requires no additional dependencies**
- **Works with any LLM** that supports multiple languages
- **Handles edge cases** like mixed-language content naturally
- **Preserves semantic meaning** better than rule-based translation

The LLM is instructed to:
1. Detect the input language
2. Extract all facts, entities, and descriptions in that same language
3. Never translate to English unless the input is in English
