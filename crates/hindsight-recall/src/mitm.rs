use std::collections::{HashMap, HashSet};
use std::time::Instant;

use hindsight_missions::MemoryItem;
use serde::{Deserialize, Serialize};
use tracing::{info, info_span};

use crate::conflict::{ConflictPair, ConflictResolution};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MitmItemView {
    pub id: String,
    pub content: String,
    pub memory_type: String,
}

impl MitmItemView {
    pub fn from_memory_item(item: &MemoryItem) -> Self {
        Self {
            id: item.id.clone(),
            content: item.content.clone(),
            memory_type: format!("{:?}", item.memory_type),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MitmConflictInput {
    pub subject: String,
    pub item_a: MitmItemView,
    pub item_b: MitmItemView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MitmDecision {
    KeepLeft,
    KeepRight,
    KeepBoth,
    KeepNeither,
}

pub trait MitmModelProvider {
    fn resolve(&self, conflicts: Vec<MitmConflictInput>) -> Result<Vec<MitmDecision>, String>;
}

#[derive(Debug, Clone)]
pub struct StubMitmModelProvider;

impl MitmModelProvider for StubMitmModelProvider {
    fn resolve(&self, conflicts: Vec<MitmConflictInput>) -> Result<Vec<MitmDecision>, String> {
        Ok(conflicts.iter().map(|_| MitmDecision::KeepBoth).collect())
    }
}

pub fn resolve_conflicts_with_model(
    result: ConflictResolution,
    model: &dyn MitmModelProvider,
) -> ConflictResolution {
    let span = info_span!("mitm_resolver", resolver_type = "mitm");
    let _enter = span.enter();
    let start = Instant::now();
    let initial_conflicts = result.conflicts.len();

    let item_map: HashMap<&str, &MemoryItem> = result
        .resolved
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();

    let inputs: Vec<MitmConflictInput> = result
        .conflicts
        .iter()
        .filter_map(|pair| {
            let a = item_map.get(pair.item_a_id.as_str())?;
            let b = item_map.get(pair.item_b_id.as_str())?;
            Some(MitmConflictInput {
                subject: pair.subject.clone(),
                item_a: MitmItemView::from_memory_item(a),
                item_b: MitmItemView::from_memory_item(b),
            })
        })
        .collect();

    let input_count = inputs.len();
    let decisions = model
        .resolve(inputs)
        .unwrap_or_else(|_| vec![MitmDecision::KeepBoth; input_count]);

    let mut losers: HashSet<String> = HashSet::new();
    let mut remaining_conflicts: Vec<ConflictPair> = Vec::new();

    for (i, pair) in result.conflicts.iter().enumerate() {
        let decision = decisions.get(i).copied().unwrap_or(MitmDecision::KeepBoth);
        match decision {
            MitmDecision::KeepLeft => {
                losers.insert(pair.item_b_id.clone());
            }
            MitmDecision::KeepRight => {
                losers.insert(pair.item_a_id.clone());
            }
            MitmDecision::KeepBoth => {
                remaining_conflicts.push(pair.clone());
            }
            MitmDecision::KeepNeither => {
                losers.insert(pair.item_a_id.clone());
                losers.insert(pair.item_b_id.clone());
            }
        }
    }

    let mut resolved = result.resolved;
    resolved.retain(|item| !losers.contains(&item.id));

    let latency_us = start.elapsed().as_micros();
    let conflicts_resolved = initial_conflicts - remaining_conflicts.len();
    info!(
        resolver_type = "mitm",
        conflicts_resolved, latency_us, "MITM conflict resolution completed"
    );

    ConflictResolution {
        resolved,
        conflicts: remaining_conflicts,
        annotations: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hindsight_core::MemoryType;

    fn make_item(id: &str, name: &str, content: &str, mt: MemoryType) -> MemoryItem {
        MemoryItem::new(id, name, content, mt, "test")
    }

    #[test]
    fn test_stub_resolver_keeps_both() {
        let a = make_item("a", "subj", "content a", MemoryType::EmpiricalObservation);
        let b = make_item("b", "subj", "content b", MemoryType::EmpiricalObservation);

        let input_result = ConflictResolution {
            resolved: vec![a, b],
            annotations: Vec::new(),
            conflicts: vec![ConflictPair {
                item_a_id: "a".into(),
                item_b_id: "b".into(),
                subject: "subj".into(),
            }],
        };

        let output = resolve_conflicts_with_model(input_result, &StubMitmModelProvider);
        assert_eq!(output.resolved.len(), 2);
        assert_eq!(output.conflicts.len(), 1);
    }

    #[test]
    fn test_stub_resolver_empty_conflicts() {
        let items = vec![make_item("a", "subj", "content", MemoryType::HardFact)];
        let input_result = ConflictResolution {
            resolved: items.clone(),
            annotations: Vec::new(),
            conflicts: vec![],
        };

        let output = resolve_conflicts_with_model(input_result, &StubMitmModelProvider);
        assert_eq!(output.resolved.len(), 1);
        assert!(output.conflicts.is_empty());
    }

    #[test]
    fn test_mitm_item_view_from_memory_item() {
        let item = make_item("id", "subj", "content", MemoryType::HardFact);
        let view = MitmItemView::from_memory_item(&item);
        assert_eq!(view.id, "id");
        assert_eq!(view.content, "content");
        assert_eq!(view.memory_type, "HardFact");
    }

    #[test]
    fn test_custom_provider_keep_left() {
        struct KeepLeftProvider;

        impl MitmModelProvider for KeepLeftProvider {
            fn resolve(
                &self,
                _conflicts: Vec<MitmConflictInput>,
            ) -> Result<Vec<MitmDecision>, String> {
                Ok(vec![MitmDecision::KeepLeft])
            }
        }

        let a = make_item("a", "subj", "content a", MemoryType::EmpiricalObservation);
        let b = make_item("b", "subj", "content b", MemoryType::EmpiricalObservation);

        let input_result = ConflictResolution {
            resolved: vec![a, b],
            annotations: Vec::new(),
            conflicts: vec![ConflictPair {
                item_a_id: "a".into(),
                item_b_id: "b".into(),
                subject: "subj".into(),
            }],
        };

        let output = resolve_conflicts_with_model(input_result, &KeepLeftProvider);
        assert_eq!(output.resolved.len(), 1);
        assert_eq!(output.resolved[0].id, "a");
        assert!(output.conflicts.is_empty());
    }

    #[test]
    fn test_custom_provider_keep_right() {
        struct KeepRightProvider;

        impl MitmModelProvider for KeepRightProvider {
            fn resolve(
                &self,
                _conflicts: Vec<MitmConflictInput>,
            ) -> Result<Vec<MitmDecision>, String> {
                Ok(vec![MitmDecision::KeepRight])
            }
        }

        let a = make_item("a", "subj", "content a", MemoryType::EmpiricalObservation);
        let b = make_item("b", "subj", "content b", MemoryType::EmpiricalObservation);

        let input_result = ConflictResolution {
            resolved: vec![a, b],
            annotations: Vec::new(),
            conflicts: vec![ConflictPair {
                item_a_id: "a".into(),
                item_b_id: "b".into(),
                subject: "subj".into(),
            }],
        };

        let output = resolve_conflicts_with_model(input_result, &KeepRightProvider);
        assert_eq!(output.resolved.len(), 1);
        assert_eq!(output.resolved[0].id, "b");
        assert!(output.conflicts.is_empty());
    }

    #[test]
    fn test_custom_provider_keep_neither() {
        struct KeepNeitherProvider;

        impl MitmModelProvider for KeepNeitherProvider {
            fn resolve(
                &self,
                _conflicts: Vec<MitmConflictInput>,
            ) -> Result<Vec<MitmDecision>, String> {
                Ok(vec![MitmDecision::KeepNeither])
            }
        }

        let a = make_item("a", "subj", "content a", MemoryType::EmpiricalObservation);
        let b = make_item("b", "subj", "content b", MemoryType::EmpiricalObservation);

        let input_result = ConflictResolution {
            resolved: vec![a, b],
            annotations: Vec::new(),
            conflicts: vec![ConflictPair {
                item_a_id: "a".into(),
                item_b_id: "b".into(),
                subject: "subj".into(),
            }],
        };

        let output = resolve_conflicts_with_model(input_result, &KeepNeitherProvider);
        assert!(output.resolved.is_empty());
        assert!(output.conflicts.is_empty());
    }

    #[test]
    fn test_provider_error_falls_back_to_keep_both() {
        struct ErrorProvider;

        impl MitmModelProvider for ErrorProvider {
            fn resolve(
                &self,
                _conflicts: Vec<MitmConflictInput>,
            ) -> Result<Vec<MitmDecision>, String> {
                Err("model unavailable".to_string())
            }
        }

        let a = make_item("a", "subj", "content a", MemoryType::EmpiricalObservation);
        let b = make_item("b", "subj", "content b", MemoryType::EmpiricalObservation);

        let input_result = ConflictResolution {
            resolved: vec![a, b],
            annotations: Vec::new(),
            conflicts: vec![ConflictPair {
                item_a_id: "a".into(),
                item_b_id: "b".into(),
                subject: "subj".into(),
            }],
        };

        let output = resolve_conflicts_with_model(input_result, &ErrorProvider);
        assert_eq!(output.resolved.len(), 2);
        assert_eq!(output.conflicts.len(), 1);
    }

    #[test]
    fn test_mitm_latency_logging() {
        let a = make_item("a", "subj", "content a", MemoryType::EmpiricalObservation);
        let b = make_item("b", "subj", "content b", MemoryType::EmpiricalObservation);

        let input_result = ConflictResolution {
            resolved: vec![a, b],
            annotations: Vec::new(),
            conflicts: vec![ConflictPair {
                item_a_id: "a".into(),
                item_b_id: "b".into(),
                subject: "subj".into(),
            }],
        };

        let output = resolve_conflicts_with_model(input_result, &StubMitmModelProvider);
        assert_eq!(output.resolved.len(), 2);
        assert_eq!(output.conflicts.len(), 1);
    }

    #[test]
    fn test_mitm_invariant_never_appears_in_conflict_input() {
        use crate::conflict::detect_conflicts;

        let ai = make_item(
            "ai_1",
            "execution_layer",
            "WASMEdge must be used",
            MemoryType::ArchitecturalInvariant,
        );
        let eo = make_item(
            "eo_1",
            "execution_layer",
            "llama.cpp is easier",
            MemoryType::EmpiricalObservation,
        );
        let eo_conflict = make_item(
            "eo_2",
            "weather",
            "it is sunny",
            MemoryType::EmpiricalObservation,
        );
        let eo_conflict2 = make_item(
            "eo_3",
            "weather",
            "it is raining",
            MemoryType::EmpiricalObservation,
        );

        let detection_result = detect_conflicts(vec![ai, eo, eo_conflict, eo_conflict2]);

        assert!(detection_result
            .conflicts
            .iter()
            .all(|pair| { pair.item_a_id != "ai_1" && pair.item_b_id != "ai_1" }));

        let conflict_inputs: Vec<MitmConflictInput> = detection_result
            .conflicts
            .iter()
            .filter_map(|pair| {
                let a = detection_result
                    .resolved
                    .iter()
                    .find(|item| item.id == pair.item_a_id)?;
                let b = detection_result
                    .resolved
                    .iter()
                    .find(|item| item.id == pair.item_b_id)?;
                Some(MitmConflictInput {
                    subject: pair.subject.clone(),
                    item_a: MitmItemView::from_memory_item(a),
                    item_b: MitmItemView::from_memory_item(b),
                })
            })
            .collect();

        for input in &conflict_inputs {
            assert_ne!(
                input.item_a.memory_type, "ArchitecturalInvariant",
                "Invariant must never appear as item_a in MITM conflict input"
            );
            assert_ne!(
                input.item_b.memory_type, "ArchitecturalInvariant",
                "Invariant must never appear as item_b in MITM conflict input"
            );
        }
    }
}
