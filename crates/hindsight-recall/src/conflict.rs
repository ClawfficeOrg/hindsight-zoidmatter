use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use hindsight_core::MemoryType;
use hindsight_missions::MemoryItem;
use tracing::{info, info_span};

/// A detected semantic conflict between two facts on the same subject.
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictPair {
    pub item_a_id: String,
    pub item_b_id: String,
    pub subject: String,
}

/// How the recall pipeline resolves detected conflicts.
///
/// Three modes are available:
/// - `HardRule` — deterministic 4-rule cascade, zero model calls (default).
/// - `ClientModel` — all conflicts pass through with annotations for the
///   querying agent to resolve inline. Lowest-latency, no model call.
/// - `Mitm` — delegates to a model-in-the-middle resolver (requires the
///   `mitm_resolver` feature flag). Invariants never reach the MITM model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolutionMode {
    HardRule,
    ClientModel,
    #[cfg(feature = "mitm_resolver")]
    Mitm,
}

/// Result of conflict detection on a recall result set.
///
/// The `annotations` field holds human-readable conflict descriptions in
/// the documented annotation format (one `String` per conflict pair).
/// This field is populated by the `ClientModel` resolver and is empty for
/// `HardRule` and `Mitm` resolvers (which handle conflicts via other means).
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictResolution {
    pub resolved: Vec<MemoryItem>,
    pub conflicts: Vec<ConflictPair>,
    pub annotations: Vec<String>,
}

/// Detects semantic conflicts in the recall result set.
///
/// Items are grouped by `name` (the subject key). Two items on the same subject
/// with different `content` form a conflict pair. `ArchitecturalInvariant` items
/// are never flagged as conflicting — when an invariant shares a subject with a
/// non-invariant item, the invariant wins silently and the non-invariant is
/// dropped from the result set.
pub fn detect_conflicts(results: Vec<MemoryItem>) -> ConflictResolution {
    let (invariants, non_invariants): (Vec<MemoryItem>, Vec<MemoryItem>) = results
        .into_iter()
        .partition(|item| item.memory_type == MemoryType::ArchitecturalInvariant);

    let invariant_names: std::collections::HashSet<&str> =
        invariants.iter().map(|i| i.name.as_str()).collect();

    let non_invariants: Vec<MemoryItem> = non_invariants
        .into_iter()
        .filter(|item| !invariant_names.contains(item.name.as_str()))
        .collect();

    let mut name_to_indices: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, item) in non_invariants.iter().enumerate() {
        name_to_indices
            .entry(item.name.clone())
            .or_default()
            .push(idx);
    }

    let mut conflicts: Vec<ConflictPair> = Vec::new();

    for (subject, indices) in &name_to_indices {
        if indices.len() < 2 {
            continue;
        }
        for i in 0..indices.len() {
            for j in (i + 1)..indices.len() {
                let a = &non_invariants[indices[i]];
                let b = &non_invariants[indices[j]];
                if a.content != b.content {
                    conflicts.push(ConflictPair {
                        item_a_id: a.id.clone(),
                        item_b_id: b.id.clone(),
                        subject: subject.clone(),
                    });
                }
            }
        }
    }

    let mut resolved = invariants;
    resolved.extend(non_invariants);

    ConflictResolution {
        resolved,
        conflicts,
        annotations: Vec::new(),
    }
}

impl ConflictPair {
    /// Produces a structured, parseable annotation string for this conflict.
    ///
    /// Format:
    /// ```text
    /// CONFLICT <subject> | <item_a_id>: "<content_a>" vs <item_b_id>: "<content_b>"
    /// ```
    ///
    /// The `items` slice must contain exactly two `MemoryItem`s corresponding
    /// to `item_a` (index 0) and `item_b` (index 1). Pipe characters (`|`) in
    /// content strings are escaped as `\|` to keep the format parseable.
    /// Newlines in content are replaced with `\\n`.
    ///
    /// # Panics
    ///
    /// Panics if `items.len() != 2`.
    pub fn to_annotation(&self, items: &[MemoryItem]) -> String {
        assert_eq!(
            items.len(),
            2,
            "to_annotation requires exactly 2 items, got {}",
            items.len()
        );

        let sanitize =
            |s: &str| -> String { s.replace('|', "\\|").replace('\n', "\\n").replace('\r', "") };

        format!(
            "CONFLICT {} | {}: \"{}\" vs {}: \"{}\"",
            self.subject,
            items[0].id,
            sanitize(&items[0].content),
            items[1].id,
            sanitize(&items[1].content),
        )
    }
}

/// Compares two [`MemoryItem`]s under the hard-rule cascade and returns the
/// winner ordering, or `None` if no rule breaks the tie.
///
/// Rules (in order):
/// 1. Higher [`MemoryType`] wins (per the priority stack).
/// 2. Within the same type, more recent `created_at` wins. An item with a
///    timestamp always beats one without.
/// 3. Within the same type and recency window, higher
///    `base_priority_multiplier` wins. `None` is treated as 1.0.
/// 4. Tie — returns `None` so both items survive with a conflict annotation.
fn pick_winner(a: &MemoryItem, b: &MemoryItem) -> Option<Ordering> {
    let type_cmp = a.memory_type.cmp(&b.memory_type);
    if type_cmp != Ordering::Equal {
        return Some(type_cmp);
    }

    match (a.created_at, b.created_at) {
        (Some(ta), Some(tb)) if ta != tb => return Some(ta.cmp(&tb)),
        (Some(_), None) => return Some(Ordering::Greater),
        (None, Some(_)) => return Some(Ordering::Less),
        _ => {}
    }

    let ma = a.base_priority_multiplier.unwrap_or(1.0);
    let mb = b.base_priority_multiplier.unwrap_or(1.0);
    let cmp = ma
        .partial_cmp(&mb)
        .expect("base_priority_multiplier must be a finite f64");
    if cmp != Ordering::Equal {
        return Some(cmp);
    }

    None
}

/// Resolves conflict pairs deterministically using the four-rule hard-rule
/// cascade without any model calls.
///
/// For each [`ConflictPair`], the two items are compared via [`pick_winner`].
/// The loser is removed from `resolved` and the pair is dropped from
/// `conflicts`. When no rule breaks the tie (rule 4), both items survive and
/// the pair is retained as a conflict annotation for the querying agent.
pub fn resolve_conflicts_hard_rule(result: ConflictResolution) -> ConflictResolution {
    let span = info_span!("hard_rule_resolver", resolver_type = "hard_rule");
    let _enter = span.enter();
    let start = Instant::now();
    let initial_conflicts = result.conflicts.len();

    let mut losers: HashSet<String> = HashSet::new();
    let mut conflicts: Vec<ConflictPair> = Vec::new();

    {
        let item_map: HashMap<&str, &MemoryItem> = result
            .resolved
            .iter()
            .map(|item| (item.id.as_str(), item))
            .collect();

        for pair in &result.conflicts {
            if losers.contains(&pair.item_a_id) || losers.contains(&pair.item_b_id) {
                continue;
            }

            if let (Some(a), Some(b)) = (
                item_map.get(pair.item_a_id.as_str()),
                item_map.get(pair.item_b_id.as_str()),
            ) {
                match pick_winner(a, b) {
                    Some(Ordering::Greater) => {
                        losers.insert(pair.item_b_id.clone());
                    }
                    Some(Ordering::Less) => {
                        losers.insert(pair.item_a_id.clone());
                    }
                    _ => {
                        conflicts.push(pair.clone());
                    }
                }
            }
        }
    }

    let mut resolved = result.resolved;
    resolved.retain(|item| !losers.contains(&item.id));

    let out = ConflictResolution {
        resolved,
        conflicts,
        annotations: Vec::new(),
    };
    let latency_us = start.elapsed().as_micros();
    let conflicts_resolved = initial_conflicts - out.conflicts.len();
    info!(
        resolver_type = "hard_rule",
        conflicts_resolved, latency_us, "Hard-rule conflict resolution completed"
    );
    out
}

/// Resolves conflict pairs by passing all items through with annotations.
///
/// This is the `ClientModel` resolver — it does **not** drop any items or
/// resolve any conflicts. Every `ConflictPair` is annotated into a parseable
/// format string stored in `ConflictResolution::annotations`. All `resolved`
/// items and all `conflicts` pairs survive unchanged. The querying agent's
/// model is expected to read the annotations and resolve conflicts inline.
///
/// This is the lowest-latency option — zero model calls, zero decisions made.
pub fn resolve_conflicts_client_model(result: ConflictResolution) -> ConflictResolution {
    let span = info_span!("client_model_resolver", resolver_type = "client_model");
    let _enter = span.enter();
    let start = Instant::now();
    let initial_conflicts = result.conflicts.len();

    let item_map: std::collections::HashMap<&str, &MemoryItem> = result
        .resolved
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();

    let mut annotations: Vec<String> = Vec::with_capacity(result.conflicts.len());
    for pair in &result.conflicts {
        if let (Some(a), Some(b)) = (
            item_map.get(pair.item_a_id.as_str()),
            item_map.get(pair.item_b_id.as_str()),
        ) {
            annotations.push(pair.to_annotation(&[(*a).clone(), (*b).clone()]));
        }
    }

    let out = ConflictResolution {
        resolved: result.resolved,
        conflicts: result.conflicts,
        annotations,
    };
    let latency_us = start.elapsed().as_micros();
    let conflicts_resolved = initial_conflicts - out.conflicts.len();
    info!(
        resolver_type = "client_model",
        conflicts_resolved, latency_us, "Client-model conflict annotation completed"
    );
    out
}

/// Resolves conflict pairs using the resolver selected by `mode`.
///
/// This is the config-aware dispatch entry point. The legacy zero-argument
/// [`resolve_conflicts`] function preserves the feature-flag-based dispatch
/// for backward compatibility.
pub fn resolve_conflicts_with_mode(
    result: ConflictResolution,
    mode: ConflictResolutionMode,
) -> ConflictResolution {
    match mode {
        ConflictResolutionMode::ClientModel => resolve_conflicts_client_model(result),
        ConflictResolutionMode::HardRule => resolve_conflicts_hard_rule(result),
        #[cfg(feature = "mitm_resolver")]
        ConflictResolutionMode::Mitm => {
            use crate::mitm::{resolve_conflicts_with_model, StubMitmModelProvider};
            resolve_conflicts_with_model(result, &StubMitmModelProvider)
        }
    }
}

/// Resolves conflict pairs using the appropriate resolver based on feature flags.
///
/// When the `mitm_resolver` feature is enabled, delegates to the MITM model
/// resolver (using `StubMitmModelProvider` by default). When the feature is
/// disabled, delegates to the deterministic hard-rule resolver.
///
/// Invariants are never passed to any resolver — they are filtered out by
/// [`detect_conflicts`] before conflict pairs are created.
pub fn resolve_conflicts(result: ConflictResolution) -> ConflictResolution {
    #[cfg(feature = "mitm_resolver")]
    {
        use crate::mitm::{resolve_conflicts_with_model, StubMitmModelProvider};
        resolve_conflicts_with_model(result, &StubMitmModelProvider)
    }

    #[cfg(not(feature = "mitm_resolver"))]
    {
        resolve_conflicts_hard_rule(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hindsight_core::MemoryType;

    fn make_item(id: &str, name: &str, content: &str, mt: MemoryType) -> MemoryItem {
        MemoryItem::new(id, name, content, mt, "test")
    }

    fn ids(conflicts: &[ConflictPair]) -> Vec<&str> {
        conflicts
            .iter()
            .flat_map(|c| [c.item_a_id.as_str(), c.item_b_id.as_str()])
            .collect()
    }

    #[test]
    fn test_two_contradicting_eos_flagged() {
        let items = vec![
            make_item(
                "eo_1",
                "weather",
                "the sky is blue",
                MemoryType::EmpiricalObservation,
            ),
            make_item(
                "eo_2",
                "weather",
                "the sky is green",
                MemoryType::EmpiricalObservation,
            ),
        ];
        let result = detect_conflicts(items);
        assert_eq!(result.resolved.len(), 2);
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].subject, "weather");
        let conflict_ids = ids(&result.conflicts);
        assert!(conflict_ids.contains(&"eo_1"));
        assert!(conflict_ids.contains(&"eo_2"));
    }

    #[test]
    fn test_invariant_wins_silently_eo_dropped() {
        let items = vec![
            make_item(
                "ai_1",
                "execution_layer",
                "WASMEdge must be the execution layer",
                MemoryType::ArchitecturalInvariant,
            ),
            make_item(
                "eo_1",
                "execution_layer",
                "llama.cpp is easier",
                MemoryType::EmpiricalObservation,
            ),
        ];
        let result = detect_conflicts(items);
        assert_eq!(result.resolved.len(), 1);
        assert_eq!(result.resolved[0].id, "ai_1");
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_same_content_no_conflict() {
        let items = vec![
            make_item(
                "eo_1",
                "weather",
                "same content",
                MemoryType::EmpiricalObservation,
            ),
            make_item(
                "eo_2",
                "weather",
                "same content",
                MemoryType::EmpiricalObservation,
            ),
        ];
        let result = detect_conflicts(items);
        assert_eq!(result.resolved.len(), 2);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_different_names_no_conflict() {
        let items = vec![
            make_item(
                "eo_1",
                "weather",
                "some content",
                MemoryType::EmpiricalObservation,
            ),
            make_item(
                "eo_2",
                "traffic",
                "some content",
                MemoryType::EmpiricalObservation,
            ),
        ];
        let result = detect_conflicts(items);
        assert_eq!(result.resolved.len(), 2);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_empty_input() {
        let result = detect_conflicts(Vec::new());
        assert!(result.resolved.is_empty());
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_single_item_no_conflict() {
        let items = vec![make_item(
            "eo_1",
            "weather",
            "content",
            MemoryType::EmpiricalObservation,
        )];
        let result = detect_conflicts(items);
        assert_eq!(result.resolved.len(), 1);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_mixed_invariant_wins_and_non_invariant_conflicts() {
        let items = vec![
            make_item(
                "ai_1",
                "execution",
                "WASMEdge must be used",
                MemoryType::ArchitecturalInvariant,
            ),
            make_item(
                "eo_exec",
                "execution",
                "llama.cpp is fine",
                MemoryType::EmpiricalObservation,
            ),
            make_item(
                "eo_a",
                "weather",
                "it is sunny",
                MemoryType::EmpiricalObservation,
            ),
            make_item(
                "eo_b",
                "weather",
                "it is raining",
                MemoryType::EmpiricalObservation,
            ),
        ];
        let result = detect_conflicts(items);

        assert_eq!(result.resolved.len(), 3);
        let resolved_ids: Vec<&str> = result.resolved.iter().map(|i| i.id.as_str()).collect();
        assert!(resolved_ids.contains(&"ai_1"));
        assert!(resolved_ids.contains(&"eo_a"));
        assert!(resolved_ids.contains(&"eo_b"));
        assert!(!resolved_ids.contains(&"eo_exec"));

        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].subject, "weather");
    }

    #[test]
    fn test_multiple_invariants_all_win_silently() {
        let items = vec![
            make_item(
                "ai_x",
                "subject_x",
                "invariant for X",
                MemoryType::ArchitecturalInvariant,
            ),
            make_item(
                "ai_y",
                "subject_y",
                "invariant for Y",
                MemoryType::ArchitecturalInvariant,
            ),
            make_item(
                "eo_x",
                "subject_x",
                "contradicts X",
                MemoryType::EmpiricalObservation,
            ),
            make_item(
                "eo_y",
                "subject_y",
                "contradicts Y",
                MemoryType::EmpiricalObservation,
            ),
            make_item(
                "eo_z",
                "subject_z",
                "no invariant for Z",
                MemoryType::EmpiricalObservation,
            ),
        ];
        let result = detect_conflicts(items);

        assert_eq!(result.resolved.len(), 3);
        let resolved_ids: Vec<&str> = result.resolved.iter().map(|i| i.id.as_str()).collect();
        assert!(resolved_ids.contains(&"ai_x"));
        assert!(resolved_ids.contains(&"ai_y"));
        assert!(resolved_ids.contains(&"eo_z"));
        assert!(!resolved_ids.contains(&"eo_x"));
        assert!(!resolved_ids.contains(&"eo_y"));
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_cross_type_non_invariant_conflict_flagged() {
        let items = vec![
            make_item(
                "hf_1",
                "weather",
                "hard fact about weather",
                MemoryType::HardFact,
            ),
            make_item(
                "eo_1",
                "weather",
                "observation disagrees",
                MemoryType::EmpiricalObservation,
            ),
        ];
        let result = detect_conflicts(items);
        assert_eq!(result.resolved.len(), 2);
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].subject, "weather");
        let conflict_ids = ids(&result.conflicts);
        assert!(conflict_ids.contains(&"hf_1"));
        assert!(conflict_ids.contains(&"eo_1"));
    }

    #[test]
    fn test_invariant_vs_invariant_never_flagged() {
        let items = vec![
            make_item(
                "ai_1",
                "execution",
                "WASMEdge is the execution layer",
                MemoryType::ArchitecturalInvariant,
            ),
            make_item(
                "ai_2",
                "execution",
                "make llama.cpp the execution layer",
                MemoryType::ArchitecturalInvariant,
            ),
        ];
        let result = detect_conflicts(items);
        assert_eq!(result.resolved.len(), 2);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_three_items_two_conflict_pairs() {
        let items = vec![
            make_item(
                "eo_a",
                "weather",
                "it is sunny",
                MemoryType::EmpiricalObservation,
            ),
            make_item(
                "eo_b",
                "weather",
                "it is raining",
                MemoryType::EmpiricalObservation,
            ),
            make_item(
                "eo_c",
                "weather",
                "it is snowing",
                MemoryType::EmpiricalObservation,
            ),
        ];
        let result = detect_conflicts(items);
        assert_eq!(result.resolved.len(), 3);
        assert_eq!(result.conflicts.len(), 3);
        let subjects: Vec<&str> = result
            .conflicts
            .iter()
            .map(|c| c.subject.as_str())
            .collect();
        assert!(subjects.iter().all(|s| *s == "weather"));
    }

    // ---------- resolve_conflicts_hard_rule unit tests ----------

    #[test]
    fn test_rule1_higher_memory_type_wins() {
        let hf = make_item("hf_1", "weather", "hard fact wins", MemoryType::HardFact);
        let eo = make_item(
            "eo_1",
            "weather",
            "observation loses",
            MemoryType::EmpiricalObservation,
        );

        let result = ConflictResolution {
            resolved: vec![hf.clone(), eo.clone()],
            annotations: Vec::new(),
            conflicts: vec![ConflictPair {
                item_a_id: "hf_1".into(),
                item_b_id: "eo_1".into(),
                subject: "weather".into(),
            }],
        };

        let resolved = resolve_conflicts_hard_rule(result);
        assert_eq!(resolved.resolved.len(), 1);
        assert_eq!(resolved.resolved[0].id, "hf_1");
        assert!(resolved.conflicts.is_empty());
    }

    #[test]
    fn test_rule2_within_same_type_more_recent_wins() {
        let older = make_item(
            "eo_old",
            "weather",
            "old content",
            MemoryType::EmpiricalObservation,
        )
        .with_created_at(100);
        let newer = make_item(
            "eo_new",
            "weather",
            "new content",
            MemoryType::EmpiricalObservation,
        )
        .with_created_at(200);

        let result = ConflictResolution {
            resolved: vec![older.clone(), newer.clone()],
            annotations: Vec::new(),
            conflicts: vec![ConflictPair {
                item_a_id: "eo_old".into(),
                item_b_id: "eo_new".into(),
                subject: "weather".into(),
            }],
        };

        let resolved = resolve_conflicts_hard_rule(result);
        assert_eq!(resolved.resolved.len(), 1);
        assert_eq!(resolved.resolved[0].id, "eo_new");
        assert!(resolved.conflicts.is_empty());
    }

    #[test]
    fn test_rule3_higher_multiplier_wins_on_recency_tie() {
        let low = make_item("hf_low", "weather", "low multiplier", MemoryType::HardFact)
            .with_priority_multiplier(1.0);
        let high = make_item(
            "hf_high",
            "weather",
            "high multiplier",
            MemoryType::HardFact,
        )
        .with_priority_multiplier(2.0);

        let result = ConflictResolution {
            resolved: vec![low.clone(), high.clone()],
            annotations: Vec::new(),
            conflicts: vec![ConflictPair {
                item_a_id: "hf_low".into(),
                item_b_id: "hf_high".into(),
                subject: "weather".into(),
            }],
        };

        let resolved = resolve_conflicts_hard_rule(result);
        assert_eq!(resolved.resolved.len(), 1);
        assert_eq!(resolved.resolved[0].id, "hf_high");
        assert!(resolved.conflicts.is_empty());
    }

    #[test]
    fn test_rule4_tie_passed_through_with_annotation() {
        let cc_a = make_item(
            "cc_a",
            "weather",
            "context A",
            MemoryType::ConversationalContext,
        );
        let cc_b = make_item(
            "cc_b",
            "weather",
            "context B",
            MemoryType::ConversationalContext,
        );

        let pair = ConflictPair {
            item_a_id: "cc_a".into(),
            item_b_id: "cc_b".into(),
            subject: "weather".into(),
        };

        let result = ConflictResolution {
            resolved: vec![cc_a.clone(), cc_b.clone()],
            annotations: Vec::new(),
            conflicts: vec![pair.clone()],
        };

        let resolved = resolve_conflicts_hard_rule(result);
        assert_eq!(resolved.resolved.len(), 2);
        assert_eq!(resolved.conflicts.len(), 1);
        assert_eq!(resolved.conflicts[0], pair);
    }

    #[test]
    fn test_rule3_same_timestamp_higher_multiplier_wins() {
        let low = make_item(
            "eo_low",
            "weather",
            "low m",
            MemoryType::EmpiricalObservation,
        )
        .with_created_at(100)
        .with_priority_multiplier(1.0);
        let high = make_item(
            "eo_high",
            "weather",
            "high m",
            MemoryType::EmpiricalObservation,
        )
        .with_created_at(100)
        .with_priority_multiplier(3.0);

        let result = ConflictResolution {
            resolved: vec![low.clone(), high.clone()],
            annotations: Vec::new(),
            conflicts: vec![ConflictPair {
                item_a_id: "eo_low".into(),
                item_b_id: "eo_high".into(),
                subject: "weather".into(),
            }],
        };

        let resolved = resolve_conflicts_hard_rule(result);
        assert_eq!(resolved.resolved.len(), 1);
        assert_eq!(resolved.resolved[0].id, "eo_high");
        assert!(resolved.conflicts.is_empty());
    }

    #[test]
    fn test_resolve_empty_conflicts() {
        let items: Vec<MemoryItem> = vec![
            make_item("a", "subj", "c", MemoryType::HardFact),
            make_item("b", "subj", "c", MemoryType::EmpiricalObservation),
        ];

        let result = ConflictResolution {
            resolved: items.clone(),
            annotations: Vec::new(),
            conflicts: vec![],
        };

        let resolved = resolve_conflicts_hard_rule(result);
        assert_eq!(resolved.resolved, items);
        assert!(resolved.conflicts.is_empty());
    }

    #[test]
    fn test_resolve_no_conflicting_items() {
        let items: Vec<MemoryItem> = vec![make_item("a", "subj", "content", MemoryType::HardFact)];

        let result = ConflictResolution {
            resolved: items.clone(),
            annotations: Vec::new(),
            conflicts: vec![],
        };

        let resolved = resolve_conflicts_hard_rule(result);
        assert_eq!(resolved.resolved, items);
        assert!(resolved.conflicts.is_empty());
    }

    #[test]
    fn test_resolve_three_items_one_loser() {
        let hf = make_item("hf", "weather", "hard fact", MemoryType::HardFact);
        let eo_a = make_item("eo_a", "weather", "obs a", MemoryType::EmpiricalObservation);
        let eo_b = make_item("eo_b", "weather", "obs b", MemoryType::EmpiricalObservation);

        let result = ConflictResolution {
            resolved: vec![hf.clone(), eo_a.clone(), eo_b.clone()],
            annotations: Vec::new(),
            conflicts: vec![
                ConflictPair {
                    item_a_id: "hf".into(),
                    item_b_id: "eo_a".into(),
                    subject: "weather".into(),
                },
                ConflictPair {
                    item_a_id: "hf".into(),
                    item_b_id: "eo_b".into(),
                    subject: "weather".into(),
                },
            ],
        };

        let resolved = resolve_conflicts_hard_rule(result);
        assert_eq!(resolved.resolved.len(), 1);
        assert_eq!(resolved.resolved[0].id, "hf");
        assert!(resolved.conflicts.is_empty());
    }

    #[test]
    fn test_resolve_three_items_all_tied() {
        let cc_a = make_item(
            "cc_a",
            "weather",
            "ctx A",
            MemoryType::ConversationalContext,
        );
        let cc_b = make_item(
            "cc_b",
            "weather",
            "ctx B",
            MemoryType::ConversationalContext,
        );
        let cc_c = make_item(
            "cc_c",
            "weather",
            "ctx C",
            MemoryType::ConversationalContext,
        );

        let pair_ab = ConflictPair {
            item_a_id: "cc_a".into(),
            item_b_id: "cc_b".into(),
            subject: "weather".into(),
        };
        let pair_ac = ConflictPair {
            item_a_id: "cc_a".into(),
            item_b_id: "cc_c".into(),
            subject: "weather".into(),
        };
        let pair_bc = ConflictPair {
            item_a_id: "cc_b".into(),
            item_b_id: "cc_c".into(),
            subject: "weather".into(),
        };

        let result = ConflictResolution {
            resolved: vec![cc_a.clone(), cc_b.clone(), cc_c.clone()],
            annotations: Vec::new(),
            conflicts: vec![pair_ab.clone(), pair_ac.clone(), pair_bc.clone()],
        };

        let resolved = resolve_conflicts_hard_rule(result);
        assert_eq!(resolved.resolved.len(), 3);
        assert_eq!(resolved.conflicts.len(), 3);
    }

    #[test]
    fn test_resolve_bulk_100_pairs_under_1ms() {
        use std::time::Instant;

        let mut resolved = Vec::with_capacity(200);
        let mut pairs = Vec::with_capacity(100);

        for i in 0..50 {
            let cc_a = make_item(
                &format!("cc_a_{i}"),
                &format!("subject_cc_{i}"),
                "content a",
                MemoryType::ConversationalContext,
            );
            let cc_b = make_item(
                &format!("cc_b_{i}"),
                &format!("subject_cc_{i}"),
                "content b",
                MemoryType::ConversationalContext,
            );
            pairs.push(ConflictPair {
                item_a_id: cc_a.id.clone(),
                item_b_id: cc_b.id.clone(),
                subject: format!("subject_cc_{i}"),
            });
            resolved.push(cc_a);
            resolved.push(cc_b);
        }

        for i in 0..50 {
            let hf = make_item(
                &format!("hf_{i}"),
                &format!("subject_hf_{i}"),
                "hard fact",
                MemoryType::HardFact,
            );
            let eo = make_item(
                &format!("eo_{i}"),
                &format!("subject_hf_{i}"),
                "observation",
                MemoryType::EmpiricalObservation,
            );
            pairs.push(ConflictPair {
                item_a_id: hf.id.clone(),
                item_b_id: eo.id.clone(),
                subject: format!("subject_hf_{i}"),
            });
            resolved.push(hf);
            resolved.push(eo);
        }

        let input = ConflictResolution {
            resolved,
            annotations: Vec::new(),
            conflicts: pairs,
        };

        let start = Instant::now();
        let _output = resolve_conflicts_hard_rule(input);
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_micros() < 1000,
            "expected <1ms, got {}µs",
            elapsed.as_micros()
        );
    }

    // ---------- feature-flag dispatch tests ----------

    #[cfg(not(feature = "mitm_resolver"))]
    #[test]
    fn test_feature_gate_dispatches_to_hard_rule() {
        let hf = make_item("hf_1", "weather", "hard fact wins", MemoryType::HardFact);
        let eo = make_item(
            "eo_1",
            "weather",
            "observation loses",
            MemoryType::EmpiricalObservation,
        );

        let result = ConflictResolution {
            resolved: vec![hf.clone(), eo.clone()],
            annotations: Vec::new(),
            conflicts: vec![ConflictPair {
                item_a_id: "hf_1".into(),
                item_b_id: "eo_1".into(),
                subject: "weather".into(),
            }],
        };

        let via_dispatch = resolve_conflicts(result.clone());
        let via_hard_rule = resolve_conflicts_hard_rule(result);
        assert_eq!(via_dispatch, via_hard_rule);
    }

    #[cfg(feature = "mitm_resolver")]
    #[test]
    fn test_feature_gate_dispatches_to_mitm() {
        let hf = make_item("hf_1", "weather", "hard fact wins", MemoryType::HardFact);
        let eo = make_item(
            "eo_1",
            "weather",
            "observation loses",
            MemoryType::EmpiricalObservation,
        );

        let result = ConflictResolution {
            resolved: vec![hf.clone(), eo.clone()],
            annotations: Vec::new(),
            conflicts: vec![ConflictPair {
                item_a_id: "hf_1".into(),
                item_b_id: "eo_1".into(),
                subject: "weather".into(),
            }],
        };

        let via_hard_rule = resolve_conflicts_hard_rule(result.clone());
        assert_eq!(via_hard_rule.resolved.len(), 1);
        assert_eq!(via_hard_rule.resolved[0].id, "hf_1");

        let via_mitm = resolve_conflicts(result);
        assert_eq!(
            via_mitm.resolved.len(),
            2,
            "Stub MITM keeps both items on tied conflict"
        );
        let ids: Vec<&str> = via_mitm.resolved.iter().map(|i| i.id.as_str()).collect();
        assert!(ids.contains(&"hf_1"));
        assert!(ids.contains(&"eo_1"));
        assert_eq!(via_mitm.conflicts.len(), 1);
    }

    #[test]
    fn test_invariant_exclusion_before_resolver() {
        let ai = make_item(
            "ai_1",
            "execution_layer",
            "WASMEdge must be the execution layer",
            MemoryType::ArchitecturalInvariant,
        );
        let eo = make_item(
            "eo_1",
            "execution_layer",
            "llama.cpp is easier",
            MemoryType::EmpiricalObservation,
        );

        let detection_result = detect_conflicts(vec![ai, eo]);
        assert!(detection_result.conflicts.is_empty());
        assert_eq!(detection_result.resolved.len(), 1);
        assert_eq!(detection_result.resolved[0].id, "ai_1");

        let resolved = resolve_conflicts_hard_rule(detection_result);
        assert_eq!(resolved.resolved.len(), 1);
        assert!(resolved.conflicts.is_empty());
    }

    // ---------- client_model resolver tests ----------

    #[test]
    fn test_client_model_resolver_passes_all_conflicts_through() {
        let eo_a = make_item(
            "eo_1",
            "weather",
            "the sky is blue",
            MemoryType::EmpiricalObservation,
        );
        let eo_b = make_item(
            "eo_2",
            "weather",
            "the sky is green",
            MemoryType::EmpiricalObservation,
        );

        let pair = ConflictPair {
            item_a_id: "eo_1".into(),
            item_b_id: "eo_2".into(),
            subject: "weather".into(),
        };

        let result = ConflictResolution {
            resolved: vec![eo_a.clone(), eo_b.clone()],
            conflicts: vec![pair.clone()],
            annotations: Vec::new(),
        };

        let resolved = resolve_conflicts_client_model(result);
        assert_eq!(resolved.resolved.len(), 2, "both items should survive");
        let ids: Vec<&str> = resolved.resolved.iter().map(|i| i.id.as_str()).collect();
        assert!(ids.contains(&"eo_1"));
        assert!(ids.contains(&"eo_2"));
        assert_eq!(resolved.conflicts.len(), 1);
        assert_eq!(resolved.annotations.len(), 1);
        assert!(resolved.annotations[0].contains("CONFLICT weather"));
    }

    #[test]
    fn test_client_model_resolver_empty_conflicts() {
        let item = make_item("a", "subj", "content", MemoryType::HardFact);

        let result = ConflictResolution {
            resolved: vec![item.clone()],
            conflicts: vec![],
            annotations: Vec::new(),
        };

        let resolved = resolve_conflicts_client_model(result);
        assert_eq!(resolved.resolved.len(), 1);
        assert_eq!(resolved.resolved[0].id, "a");
        assert!(resolved.conflicts.is_empty());
        assert!(resolved.annotations.is_empty());
    }

    #[test]
    fn test_client_model_resolver_three_items_all_conflicts_annotated() {
        let cc_a = make_item(
            "cc_a",
            "weather",
            "ctx A",
            MemoryType::ConversationalContext,
        );
        let cc_b = make_item(
            "cc_b",
            "weather",
            "ctx B",
            MemoryType::ConversationalContext,
        );
        let cc_c = make_item(
            "cc_c",
            "weather",
            "ctx C",
            MemoryType::ConversationalContext,
        );

        let pair_ab = ConflictPair {
            item_a_id: "cc_a".into(),
            item_b_id: "cc_b".into(),
            subject: "weather".into(),
        };
        let pair_ac = ConflictPair {
            item_a_id: "cc_a".into(),
            item_b_id: "cc_c".into(),
            subject: "weather".into(),
        };
        let pair_bc = ConflictPair {
            item_a_id: "cc_b".into(),
            item_b_id: "cc_c".into(),
            subject: "weather".into(),
        };

        let result = ConflictResolution {
            resolved: vec![cc_a.clone(), cc_b.clone(), cc_c.clone()],
            conflicts: vec![pair_ab, pair_ac, pair_bc],
            annotations: Vec::new(),
        };

        let resolved = resolve_conflicts_client_model(result);
        assert_eq!(resolved.resolved.len(), 3, "all three items should survive");
        assert_eq!(resolved.conflicts.len(), 3);
        assert_eq!(resolved.annotations.len(), 3);
        for annotation in &resolved.annotations {
            assert!(annotation.contains("CONFLICT weather"));
        }
    }

    #[test]
    fn test_annotation_format_is_parseable() {
        let pair = ConflictPair {
            item_a_id: "eo_1".into(),
            item_b_id: "eo_2".into(),
            subject: "weather".into(),
        };

        let item_a = make_item(
            "eo_1",
            "weather",
            "the sky is blue",
            MemoryType::EmpiricalObservation,
        );
        let item_b = make_item(
            "eo_2",
            "weather",
            "the sky is green",
            MemoryType::EmpiricalObservation,
        );

        let annotation = pair.to_annotation(&[item_a, item_b]);

        assert!(annotation.starts_with("CONFLICT weather |"));
        assert!(annotation.contains("eo_1: \"the sky is blue\""));
        assert!(annotation.contains("eo_2: \"the sky is green\""));
        assert!(annotation.contains(" vs "));

        let parts: Vec<&str> = annotation.splitn(2, " | ").collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "CONFLICT weather");

        let vs_parts: Vec<&str> = parts[1].splitn(2, " vs ").collect();
        assert_eq!(vs_parts.len(), 2);
        assert!(vs_parts[0].starts_with("eo_1: \""));
        assert!(vs_parts[1].starts_with("eo_2: \""));
        assert!(vs_parts[0].ends_with('"'));
        assert!(vs_parts[1].ends_with('"'));
    }

    #[test]
    fn test_annotation_format_escapes_pipe_and_newline() {
        let pair = ConflictPair {
            item_a_id: "a".into(),
            item_b_id: "b".into(),
            subject: "s".into(),
        };

        let item_a = make_item("a", "s", "content|with|pipes", MemoryType::HardFact);
        let item_b = make_item(
            "b",
            "s",
            "content\nwith\nnewlines",
            MemoryType::EmpiricalObservation,
        );

        let annotation = pair.to_annotation(&[item_a, item_b]);
        assert!(!annotation.contains("content|with|pipes"));
        assert!(annotation.contains("content\\|with\\|pipes"));
        assert!(!annotation.contains('\n'));
        assert!(annotation.contains("content\\nwith\\nnewlines"));
    }

    #[test]
    fn test_config_selection_hard_rule() {
        let hf = make_item("hf_1", "weather", "hard fact wins", MemoryType::HardFact);
        let eo = make_item(
            "eo_1",
            "weather",
            "observation loses",
            MemoryType::EmpiricalObservation,
        );

        let result = ConflictResolution {
            resolved: vec![hf.clone(), eo.clone()],
            conflicts: vec![ConflictPair {
                item_a_id: "hf_1".into(),
                item_b_id: "eo_1".into(),
                subject: "weather".into(),
            }],
            annotations: Vec::new(),
        };

        let resolved = resolve_conflicts_with_mode(result, ConflictResolutionMode::HardRule);
        assert_eq!(resolved.resolved.len(), 1);
        assert_eq!(resolved.resolved[0].id, "hf_1");
        assert!(resolved.conflicts.is_empty());
        assert!(resolved.annotations.is_empty());
    }

    #[test]
    fn test_config_selection_client_model() {
        let hf = make_item("hf_1", "weather", "hard fact", MemoryType::HardFact);
        let eo = make_item(
            "eo_1",
            "weather",
            "observation",
            MemoryType::EmpiricalObservation,
        );

        let result = ConflictResolution {
            resolved: vec![hf.clone(), eo.clone()],
            conflicts: vec![ConflictPair {
                item_a_id: "hf_1".into(),
                item_b_id: "eo_1".into(),
                subject: "weather".into(),
            }],
            annotations: Vec::new(),
        };

        let resolved = resolve_conflicts_with_mode(result, ConflictResolutionMode::ClientModel);
        assert_eq!(resolved.resolved.len(), 2, "both items survive");
        assert_eq!(resolved.conflicts.len(), 1);
        assert_eq!(resolved.annotations.len(), 1, "one annotation");
        assert!(resolved.annotations[0].contains("CONFLICT weather"));
    }
}
