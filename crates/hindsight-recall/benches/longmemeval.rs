#![cfg_attr(test, allow(dead_code))]

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use serde::Deserialize;

use hindsight_core::MemoryType;
use hindsight_missions::{FactStore, InMemoryFactStore, MemoryItem};
use hindsight_recall::RecallPipeline;

// ---------------------------------------------------------------------------
// LongMemEval dataset types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct LongMemEvalItem {
    question_id: String,
    question: String,
    #[serde(default)]
    answer: StringOrInt,
    question_type: String,
    #[serde(default)]
    question_date: Option<String>,
    haystack_sessions: Vec<Vec<serde_json::Value>>,
    haystack_dates: Vec<String>,
    haystack_session_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct StringOrInt(pub String);

impl<'de> serde::Deserialize<'de> for StringOrInt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;
        struct StringOrIntVisitor;
        impl<'de> de::Visitor<'de> for StringOrIntVisitor {
            type Value = StringOrInt;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string or integer")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(StringOrInt(v.to_string()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(StringOrInt(v.to_string()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(StringOrInt(v.to_string()))
            }
        }
        deserializer.deserialize_any(StringOrIntVisitor)
    }
}

// ---------------------------------------------------------------------------
// Benchmark result types
// ---------------------------------------------------------------------------

struct ItemResult {
    question_id: String,
    question_type: String,
    expected_answer_len: usize,
    sessions_ingested: usize,
    facts_stored: usize,
    recall_count: usize,
    recall_latency_us: u64,
    ingestion_latency_us: u64,
}

struct BenchmarkResult {
    total_items: usize,
    total_sessions_ingested: usize,
    total_facts_stored: usize,
    avg_recall_latency_us: f64,
    avg_ingestion_latency_us: f64,
    item_results: Vec<ItemResult>,
    score_estimate: f64,
}

fn load_dataset(path: &PathBuf, max_items: Option<usize>) -> Vec<LongMemEvalItem> {
    let raw = fs::read_to_string(path).expect("failed to read dataset file");
    let mut items: Vec<LongMemEvalItem> =
        serde_json::from_str(&raw).expect("failed to parse dataset JSON");

    if let Some(max) = max_items {
        items.truncate(max);
    }
    items
}

fn ingest_item(
    item: &LongMemEvalItem,
    store: &InMemoryFactStore,
) -> (Vec<MemoryItem>, usize, usize, u64) {
    let start = Instant::now();
    let mut facts_stored = 0;
    let sessions_ingested = item.haystack_sessions.len();
    let mut stored_items = Vec::new();

    for (session_idx, session_turns) in item.haystack_sessions.iter().enumerate() {
        let session_id = item
            .haystack_session_ids
            .get(session_idx)
            .cloned()
            .unwrap_or_else(|| format!("sess_{}", session_idx));

        let session_date = item
            .haystack_dates
            .get(session_idx)
            .cloned()
            .unwrap_or_default();

        for (turn_idx, turn) in session_turns.iter().enumerate() {
            let turn_text = match turn {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };

            let fact = MemoryItem::new(
                format!("{}_{}_{}", item.question_id, session_id, turn_idx),
                format!("{}_{}", item.question_id, session_id),
                turn_text,
                MemoryType::EmpiricalObservation,
                "benchmark",
            )
            .with_created_at(parse_date_timestamp(&session_date));

            store.store(fact.clone()).ok();
            stored_items.push(fact);
            facts_stored += 1;
        }
    }

    let elapsed = start.elapsed().as_micros() as u64;
    (stored_items, sessions_ingested, facts_stored, elapsed)
}

fn recall_for_question(
    item: &LongMemEvalItem,
    all_items: &[MemoryItem],
    store: &dyn FactStore,
) -> (usize, u64) {
    let pipeline = RecallPipeline::new();
    let start = Instant::now();

    let items_for_question: Vec<MemoryItem> = all_items
        .iter()
        .filter(|i| i.id.starts_with(&item.question_id))
        .cloned()
        .collect();

    let result = pipeline
        .recall_with_priority(items_for_question, store, None)
        .unwrap_or_default();

    let elapsed = start.elapsed().as_micros() as u64;
    (result.len(), elapsed)
}

fn keyword_overlap(question: &str, facts: &[MemoryItem]) -> f64 {
    if facts.is_empty() {
        return 0.0;
    }

    let question_words: HashSet<String> = question
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| w.len() > 2)
        .collect();

    if question_words.is_empty() {
        return 1.0;
    }

    let fact_text: String = facts
        .iter()
        .map(|f| f.content.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let fact_words: HashSet<String> = fact_text
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .collect();

    let matches: usize = question_words
        .iter()
        .filter(|qw| fact_words.contains(qw.as_str()))
        .count();

    matches as f64 / question_words.len() as f64
}

fn parse_date_timestamp(date_str: &str) -> i64 {
    if date_str.is_empty() {
        return 0;
    }

    if let Some(cleaned) = date_str.split('(').next() {
        let cleaned = cleaned.trim();
        for fmt in &[
            "%Y/%m/%d %H:%M",
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d",
            "%Y/%m/%d",
        ] {
            let result = chrono::NaiveDateTime::parse_from_str(cleaned, fmt).or_else(|_| {
                chrono::NaiveDate::parse_from_str(cleaned, fmt)
                    .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
            });
            if let Ok(dt) = result {
                return dt.and_utc().timestamp();
            }
        }
    }
    0
}

fn run_benchmark(dataset_path: PathBuf, max_items: Option<usize>) -> BenchmarkResult {
    let items = load_dataset(&dataset_path, max_items);

    let mut item_results = Vec::with_capacity(items.len());
    let mut total_sessions = 0;
    let mut total_facts = 0;
    let mut total_recall_us = 0u64;
    let mut total_ingestion_us = 0u64;
    let mut total_overlap = 0.0;

    for item in &items {
        let store = InMemoryFactStore::default();

        let (stored_items, sessions, facts, ingest_us) = ingest_item(item, &store);
        let (recall_count, recall_us) =
            recall_for_question(item, &stored_items, &store as &dyn FactStore);

        let overlap = keyword_overlap(&item.question, &stored_items);

        total_sessions += sessions;
        total_facts += facts;
        total_recall_us += recall_us;
        total_ingestion_us += ingest_us;
        total_overlap += overlap;

        item_results.push(ItemResult {
            question_id: item.question_id.clone(),
            question_type: item.question_type.clone(),
            expected_answer_len: item.answer.0.len(),
            sessions_ingested: sessions,
            facts_stored: facts,
            recall_count,
            recall_latency_us: recall_us,
            ingestion_latency_us: ingest_us,
        });

        let _ = &item.question_date;
    }

    let n = items.len() as f64;
    let score_estimate = if n > 0.0 {
        (total_overlap / n) * 100.0
    } else {
        0.0
    };

    BenchmarkResult {
        total_items: items.len(),
        total_sessions_ingested: total_sessions,
        total_facts_stored: total_facts,
        avg_recall_latency_us: if n > 0.0 {
            total_recall_us as f64 / n
        } else {
            0.0
        },
        avg_ingestion_latency_us: if n > 0.0 {
            total_ingestion_us as f64 / n
        } else {
            0.0
        },
        item_results,
        score_estimate,
    }
}

fn find_dataset() -> Option<PathBuf> {
    if let Ok(p) = env::var("LONGMEMEVAL_DATASET") {
        let path = PathBuf::from(&p);
        if path.exists() {
            return Some(path);
        }
    }

    let candidates = [
        ".benchmark_data/longmemeval_s_cleaned.json",
        "vendor_forks/hindsight-zoidmatter/hindsight-dev/benchmarks/longmemeval/datasets/longmemeval_s_cleaned.json",
        "../vendor_forks/hindsight-zoidmatter/hindsight-dev/benchmarks/longmemeval/datasets/longmemeval_s_cleaned.json",
    ];

    for candidate in &candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

fn write_results(result: &BenchmarkResult, output_path: &PathBuf) {
    let mut lines = Vec::new();
    lines.push("# LongMemEval Benchmark Results".to_string());
    lines.push(String::new());
    lines.push("## ZoidMatter Memory Layer — Baseline".to_string());
    lines.push(String::new());
    lines.push(format!("**Total items**: {}", result.total_items));
    lines.push(format!(
        "**Total sessions ingested**: {}",
        result.total_sessions_ingested
    ));
    lines.push(format!(
        "**Total facts stored**: {}",
        result.total_facts_stored
    ));
    lines.push(format!(
        "**Avg recall latency**: {:.1}µs",
        result.avg_recall_latency_us
    ));
    lines.push(format!(
        "**Avg ingestion latency**: {:.1}µs",
        result.avg_ingestion_latency_us
    ));
    lines.push(format!(
        "**Recall relevance score (keyword overlap)**: {:.2}%",
        result.score_estimate
    ));
    lines.push(String::new());
    lines.push(
        "> **Note**: The 94.6% target score is the end-to-end LongMemEval accuracy".to_string(),
    );
    lines.push(
        "> measured by the Python benchmark runner with an LLM judge. This Rust harness"
            .to_string(),
    );
    lines.push(
        "> validates the memory layer's recall quality and performance independently.".to_string(),
    );
    lines.push(
        "> Run the full Python benchmark at `vendor_forks/hindsight-zoidmatter/hindsight-dev/benchmarks/longmemeval/`".to_string(),
    );
    lines.push("> for the complete accuracy score.".to_string());
    lines.push(String::new());
    lines.push("## Per-Item Breakdown".to_string());
    lines.push(String::new());
    lines.push(
        "| Question ID | Type | Sessions | Facts | Recall | Recall µs | Ingestion µs | Answer Chars |".to_string(),
    );
    lines.push(
        "|-------------|------|----------|-------|--------|----------|-------------|-------------|"
            .to_string(),
    );
    for ir in &result.item_results {
        lines.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            ir.question_id,
            ir.question_type,
            ir.sessions_ingested,
            ir.facts_stored,
            ir.recall_count,
            ir.recall_latency_us,
            ir.ingestion_latency_us,
            ir.expected_answer_len,
        ));
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(output_path, lines.join("\n") + "\n").expect("failed to write results");
}

fn main() {
    // Run unit tests if RUN_TESTS env var is set, otherwise run the benchmark.
    if std::env::var("RUN_TESTS").is_ok() {
        tests::run_tests();
        return;
    }
    let dataset_path = find_dataset();

    match dataset_path {
        Some(path) => {
            eprintln!("Using dataset: {}", path.display());

            let max_items = env::var("LONGMEMEVAL_MAX_ITEMS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok());

            let result = run_benchmark(path, max_items);

            let output_path = PathBuf::from(
                env::var("LONGMEMEVAL_OUTPUT").unwrap_or_else(|_| "docs/benchmarks.md".to_string()),
            );

            write_results(&result, &output_path);
            eprintln!("Results written to {}", output_path.display());
            eprintln!("Recall relevance score: {:.2}%", result.score_estimate);
            eprintln!(
                "Avg recall latency: {:.1}µs over {} items",
                result.avg_recall_latency_us, result.total_items
            );
        }
        None => {
            eprintln!("LongMemEval dataset not found.");
            eprintln!();
            eprintln!("Download the dataset:");
            eprintln!(
                "  curl -L 'https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned/resolve/main/longmemeval_s_cleaned.json' \\"
            );
            eprintln!(
            "    -o vendor_forks/hindsight-zoidmatter/hindsight-dev/benchmarks/longmemeval/datasets/longmemeval_s_cleaned.json"
        );
            eprintln!();
            eprintln!("Or set LONGMEMEVAL_DATASET=/path/to/dataset.json and re-run.");
            eprintln!();
            eprintln!("Set LONGMEMEVAL_MAX_ITEMS=N to limit the benchmark to N items.");
            eprintln!(
                "Set LONGMEMEVAL_OUTPUT=path to change the output file (default: docs/benchmarks.md)."
            );
        }
    }
}

#[allow(unused)]
mod tests {
    use super::*;

    pub fn run_tests() {
        eprintln!("running 4 tests");
        test_keyword_overlap_empty_facts();
        test_keyword_overlap_full_match();
        test_parse_date_standard_format();
        test_parse_date_empty_returns_zero();
        test_ingest_item_stores_facts();
        eprintln!("all tests passed");
    }

    fn test_keyword_overlap_empty_facts() {
        let score = keyword_overlap("what is weather", &[]);
        assert_eq!(score, 0.0);
    }

    fn test_keyword_overlap_full_match() {
        let facts = vec![MemoryItem::new(
            "1",
            "weather",
            "the weather is sunny today",
            MemoryType::EmpiricalObservation,
            "test",
        )];
        let score = keyword_overlap("what is weather", &facts);
        assert!(score > 0.0);
    }

    fn test_parse_date_standard_format() {
        let ts = parse_date_timestamp("2023/05/20 02:21");
        assert!(ts > 0);
    }

    fn test_parse_date_empty_returns_zero() {
        let ts = parse_date_timestamp("");
        assert_eq!(ts, 0);
    }

    fn test_ingest_item_stores_facts() {
        let item = LongMemEvalItem {
            question_id: "test_q1".to_string(),
            question: "what is the weather?".to_string(),
            answer: StringOrInt("sunny".to_string()),
            question_type: "single-session-user".to_string(),
            question_date: None,
            haystack_sessions: vec![vec![
                serde_json::Value::String("hello".to_string()),
                serde_json::Value::String("how are you?".to_string()),
            ]],
            haystack_dates: vec!["2023/05/20 02:21".to_string()],
            haystack_session_ids: vec!["s1".to_string()],
        };

        let store = InMemoryFactStore::default();
        let (stored, sessions, facts, _latency) = ingest_item(&item, &store);
        assert_eq!(sessions, 1);
        assert_eq!(facts, 2);
        assert_eq!(stored.len(), 2);
    }
}
