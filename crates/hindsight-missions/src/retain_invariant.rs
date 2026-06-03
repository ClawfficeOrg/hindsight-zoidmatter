use crate::error::MissionError;
use crate::traits::{FactStore, RetainMission};
use crate::MemoryItem;
use hindsight_core::MemoryType;

/// A retain mission that enforces `ArchitecturalInvariant` rules.
///
/// # Disposition
///
/// Maximum skepticism toward contradiction, maximum literalism, never softened
/// by recency or frequency. An invariant once set can only be overridden by
/// explicit human-tagged input, not by agent inference.
///
/// # Contradiction detection
///
/// Currently uses exact name-match: two items contradict if they share the same
/// `name` field (subject key) but have different `content`. This is a simple
/// initial implementation — semantic contradiction detection would require an
/// LLM call and is out of scope for v0.2.1.
pub struct ArchitecturalInvariantRetainMission;

impl RetainMission for ArchitecturalInvariantRetainMission {
    fn name(&self) -> &str {
        "architectural_invariant"
    }

    fn process(
        &self,
        items: Vec<MemoryItem>,
        store: &dyn FactStore,
    ) -> Result<Vec<MemoryItem>, MissionError> {
        let existing_invariants = store.get_invariants()?;
        let mut accepted = Vec::with_capacity(items.len());

        for item in items {
            match self.evaluate(&item, &existing_invariants) {
                Ok(()) => accepted.push(item),
                Err(e) => {
                    tracing::warn!(
                        mission = self.name(),
                        item_name = %item.name,
                        item_source = %item.source,
                        error = %e,
                        "fact rejected by invariant retain mission"
                    );
                    return Err(e);
                }
            }
        }

        Ok(accepted)
    }
}

impl ArchitecturalInvariantRetainMission {
    fn evaluate(&self, item: &MemoryItem, invariants: &[MemoryItem]) -> Result<(), MissionError> {
        for invariant in invariants {
            if item.name == invariant.name {
                if item.content == invariant.content {
                    continue;
                }

                if item.memory_type == MemoryType::ArchitecturalInvariant && item.source == "human"
                {
                    continue;
                }

                return Err(MissionError::Rejected(format!(
                    "item '{}' contradicts existing invariant '{}': invariant content '{}' vs incoming '{}'",
                    item.name,
                    invariant.id,
                    invariant.content,
                    item.content
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fact_store::InMemoryFactStore;

    fn mk_item(name: &str, content: &str, memory_type: MemoryType, source: &str) -> MemoryItem {
        MemoryItem {
            id: format!("item-{}", name),
            name: name.to_string(),
            content: content.to_string(),
            memory_type,
            source: source.to_string(),
            tags: vec![],
            invariant_scope: None,
            created_at: None,
            base_priority_multiplier: None,
            version: None,
        }
    }

    #[test]
    fn test_agent_contradiction_rejected() {
        let store = InMemoryFactStore::new(vec![mk_item(
            "sky",
            "The sky is blue",
            MemoryType::ArchitecturalInvariant,
            "human",
        )]);

        let mission = ArchitecturalInvariantRetainMission;
        let result = mission.process(
            vec![mk_item(
                "sky",
                "The sky is green",
                MemoryType::EmpiricalObservation,
                "agent",
            )],
            &store,
        );

        match result {
            Err(MissionError::Rejected(msg)) => {
                assert!(
                    msg.contains("sky"),
                    "rejection message should reference the item name"
                );
            }
            other => panic!("expected MissionError::Rejected, got {:?}", other),
        }
    }

    #[test]
    fn test_human_override_accepted() {
        let store = InMemoryFactStore::new(vec![mk_item(
            "port",
            "Port 443 is default",
            MemoryType::ArchitecturalInvariant,
            "human",
        )]);

        let mission = ArchitecturalInvariantRetainMission;
        let result = mission.process(
            vec![mk_item(
                "port",
                "Port 8443 is default",
                MemoryType::ArchitecturalInvariant,
                "human",
            )],
            &store,
        );

        let accepted = result.expect("human-tagged invariant override should be accepted");
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].content, "Port 8443 is default");
    }

    #[test]
    fn test_non_contradicting_fact_passes() {
        let store = InMemoryFactStore::new(vec![mk_item(
            "retries",
            "Max retries = 3",
            MemoryType::ArchitecturalInvariant,
            "human",
        )]);

        let mission = ArchitecturalInvariantRetainMission;
        let result = mission.process(
            vec![mk_item(
                "timeout",
                "Timeout is 30s",
                MemoryType::EmpiricalObservation,
                "agent",
            )],
            &store,
        );

        let accepted = result.expect("non-contradicting fact on different name should pass");
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].name, "timeout");
    }

    #[test]
    fn test_invariant_matches_itself() {
        let store = InMemoryFactStore::new(vec![mk_item(
            "retries",
            "Max retries = 3",
            MemoryType::ArchitecturalInvariant,
            "human",
        )]);

        let mission = ArchitecturalInvariantRetainMission;
        let result = mission.process(
            vec![mk_item(
                "retries",
                "Max retries = 3",
                MemoryType::ArchitecturalInvariant,
                "human",
            )],
            &store,
        );

        let accepted = result.expect("restating the same invariant content should pass");
        assert_eq!(accepted.len(), 1);
    }

    #[test]
    fn test_agent_invariant_rejected_when_contradicting() {
        let store = InMemoryFactStore::new(vec![mk_item(
            "runtime",
            "WASMEdge must be the execution layer",
            MemoryType::ArchitecturalInvariant,
            "human",
        )]);

        let mission = ArchitecturalInvariantRetainMission;
        let result = mission.process(
            vec![mk_item(
                "runtime",
                "llama.cpp CLI is acceptable",
                MemoryType::ArchitecturalInvariant,
                "agent",
            )],
            &store,
        );

        match result {
            Err(MissionError::Rejected(_)) => {}
            other => panic!(
                "agent-authored invariant contradicting existing should be rejected, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_empty_invariants_accepts_all() {
        let store = InMemoryFactStore::default();
        let mission = ArchitecturalInvariantRetainMission;

        let result = mission.process(
            vec![
                mk_item(
                    "sky",
                    "The sky is green",
                    MemoryType::EmpiricalObservation,
                    "agent",
                ),
                mk_item(
                    "port",
                    "Port 8443 is default",
                    MemoryType::ArchitecturalInvariant,
                    "agent",
                ),
            ],
            &store,
        );

        let accepted = result.expect("all items should pass when no invariants exist");
        assert_eq!(accepted.len(), 2);
    }
}
