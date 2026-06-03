use std::collections::HashSet;

use hindsight_missions::{FactStore, MemoryItem, MissionError};

use crate::conflict::{self, ConflictResolution, ConflictResolutionMode};
use crate::priority::PriorityStackResolver;

pub struct RecallPipeline {
    conflict_resolution_mode: ConflictResolutionMode,
}

impl RecallPipeline {
    pub fn new() -> Self {
        Self {
            conflict_resolution_mode: ConflictResolutionMode::HardRule,
        }
    }

    pub fn with_conflict_resolution_mode(mut self, mode: ConflictResolutionMode) -> Self {
        self.conflict_resolution_mode = mode;
        self
    }

    pub fn invariant_pre_check_gate(
        &self,
        results: Vec<MemoryItem>,
        store: &dyn FactStore,
        current_project: Option<&str>,
    ) -> Result<Vec<MemoryItem>, MissionError> {
        let all_invariants = store.get_invariants()?;

        let invariants: Vec<MemoryItem> = all_invariants
            .into_iter()
            .filter(|i| {
                i.invariant_scope
                    .as_ref()
                    .is_none_or(|s| s.matches(current_project))
            })
            .collect();

        let invariant_ids: HashSet<&str> = invariants.iter().map(|i| i.id.as_str()).collect();

        let filtered: Vec<MemoryItem> = results
            .into_iter()
            .filter(|item| !invariant_ids.contains(item.id.as_str()))
            .collect();

        let mut output = invariants;
        output.extend(filtered);

        Ok(output)
    }

    pub fn resolve_priority_stack(&self, results: Vec<MemoryItem>) -> Vec<MemoryItem> {
        PriorityStackResolver::resolve(results)
    }

    pub fn recall_with_priority(
        &self,
        results: Vec<MemoryItem>,
        store: &dyn FactStore,
        current_project: Option<&str>,
    ) -> Result<Vec<MemoryItem>, MissionError> {
        let gated = self.invariant_pre_check_gate(results, store, current_project)?;
        let consolidated = self.consolidate_observations(gated, 5);
        Ok(self.resolve_priority_stack(consolidated))
    }

    pub fn detect_conflicts(&self, results: Vec<MemoryItem>) -> ConflictResolution {
        conflict::detect_conflicts(results)
    }

    pub fn recall_with_conflict_detection(
        &self,
        results: Vec<MemoryItem>,
        store: &dyn FactStore,
        current_project: Option<&str>,
    ) -> Result<ConflictResolution, MissionError> {
        let prioritized = self.recall_with_priority(results, store, current_project)?;
        Ok(self.detect_conflicts(prioritized))
    }

    pub fn recall_with_conflict_resolution(
        &self,
        results: Vec<MemoryItem>,
        store: &dyn FactStore,
        current_project: Option<&str>,
    ) -> Result<ConflictResolution, MissionError> {
        let detection = self.recall_with_conflict_detection(results, store, current_project)?;
        Ok(conflict::resolve_conflicts_with_mode(
            detection,
            self.conflict_resolution_mode,
        ))
    }

    pub fn consolidate_observations(
        &self,
        results: Vec<MemoryItem>,
        threshold: usize,
    ) -> Vec<MemoryItem> {
        crate::priority::consolidate_observations(results, threshold)
    }
}

impl Default for RecallPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conflict::ConflictResolutionMode;
    use hindsight_core::MemoryType;
    use hindsight_missions::{FactStore, InMemoryFactStore};

    fn make_invariant(id: &str, name: &str, content: &str) -> MemoryItem {
        MemoryItem::new(
            id,
            name,
            content,
            MemoryType::ArchitecturalInvariant,
            "human",
        )
    }

    fn make_observation(id: &str, name: &str, content: &str) -> MemoryItem {
        MemoryItem::new(id, name, content, MemoryType::EmpiricalObservation, "agent")
    }

    #[test]
    fn empty_invariants_passthrough() {
        let store = InMemoryFactStore::default();
        let pipeline = RecallPipeline::new();

        let items = vec![
            make_observation("a", "alpha", "alpha content"),
            make_observation("b", "beta", "beta content"),
        ];
        let result = pipeline
            .invariant_pre_check_gate(items.clone(), &store, None)
            .unwrap();

        assert_eq!(result, items);
    }

    #[test]
    fn invariant_prepended_at_position_zero() {
        let store = InMemoryFactStore::default();
        store
            .store(make_invariant(
                "inv_1",
                "execution_layer",
                "WASMEdge runtime must be the execution layer",
            ))
            .unwrap();

        let pipeline = RecallPipeline::new();
        let items = vec![
            make_observation("obs_1", "alpha", "alpha obs"),
            make_observation("obs_2", "beta", "beta obs"),
            make_observation("obs_3", "gamma", "gamma obs"),
        ];
        let result = pipeline
            .invariant_pre_check_gate(items, &store, None)
            .unwrap();

        assert_eq!(result.len(), 4);
        assert_eq!(result[0].memory_type, MemoryType::ArchitecturalInvariant);
        assert_eq!(result[0].id, "inv_1");
        assert!(result[0]
            .content
            .contains("WASMEdge runtime must be the execution layer"));
    }

    #[test]
    fn invariant_ids_deduplicated_from_results() {
        let store = InMemoryFactStore::default();
        store
            .store(make_invariant(
                "inv_1",
                "execution_layer",
                "WASMEdge runtime must be the execution layer",
            ))
            .unwrap();

        let pipeline = RecallPipeline::new();
        let items = vec![
            make_invariant(
                "inv_1",
                "execution_layer",
                "WASMEdge runtime must be the execution layer",
            ),
            make_observation("obs_1", "alpha", "alpha obs"),
        ];
        let result = pipeline
            .invariant_pre_check_gate(items, &store, None)
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].memory_type, MemoryType::ArchitecturalInvariant);
        assert_eq!(result[1].id, "obs_1");
    }

    #[test]
    fn multiple_invariants_prepended_in_store_order() {
        let store = InMemoryFactStore::default();
        store
            .store(make_invariant(
                "inv_1",
                "execution_layer",
                "WASMEdge must be the execution layer",
            ))
            .unwrap();
        store
            .store(make_invariant(
                "inv_2",
                "memory_system",
                "Hindsight is the memory backend",
            ))
            .unwrap();
        store
            .store(make_invariant(
                "inv_3",
                "language",
                "ZoidMatter is Rust-first",
            ))
            .unwrap();

        let pipeline = RecallPipeline::new();
        let items = vec![
            make_observation("obs_1", "alpha", "alpha obs"),
            make_observation("obs_2", "beta", "beta obs"),
        ];
        let result = pipeline
            .invariant_pre_check_gate(items, &store, None)
            .unwrap();

        assert_eq!(result.len(), 5);
        assert_eq!(result[0].id, "inv_1");
        assert_eq!(result[1].id, "inv_2");
        assert_eq!(result[2].id, "inv_3");
        assert!(result.iter().all(|i| {
            [0, 1, 2].contains(&result.iter().position(|x| x.id == i.id).unwrap())
                || i.memory_type != MemoryType::ArchitecturalInvariant
        }));
        assert_eq!(result[3].id, "obs_1");
        assert_eq!(result[4].id, "obs_2");
    }

    #[test]
    fn project_scoped_invariant_filtered_by_current_project() {
        let store = InMemoryFactStore::default();
        store
            .store(make_invariant("inv_global", "global", "global invariant"))
            .unwrap();
        store
            .store(
                make_invariant("inv_project", "project", "project invariant").with_invariant_scope(
                    hindsight_core::InvariantScope::Project("zoidmatter".to_string()),
                ),
            )
            .unwrap();

        let pipeline = RecallPipeline::new();
        let items = vec![make_observation("obs_1", "a", "alpha")];

        // With matching project, all invariants appear.
        let result = pipeline
            .invariant_pre_check_gate(items.clone(), &store, Some("zoidmatter"))
            .unwrap();
        assert_eq!(result.len(), 3);

        // With different project, only global invariant appears.
        let result = pipeline
            .invariant_pre_check_gate(items.clone(), &store, Some("other"))
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "inv_global");

        // With no project, only global invariant appears.
        let result = pipeline
            .invariant_pre_check_gate(items, &store, None)
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "inv_global");
    }

    #[test]
    fn empty_results_with_invariants_returns_only_invariants() {
        let store = InMemoryFactStore::default();
        store
            .store(make_invariant(
                "inv_1",
                "execution_layer",
                "WASMEdge must be the execution layer",
            ))
            .unwrap();

        let pipeline = RecallPipeline::new();
        let result = pipeline
            .invariant_pre_check_gate(Vec::new(), &store, None)
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "inv_1");
    }

    #[test]
    fn test_session_scope_isolation_after_reset() {
        let store = InMemoryFactStore::default();

        store
            .store(
                make_invariant("inv_global", "global_rule", "always applies")
                    .with_invariant_scope(hindsight_core::InvariantScope::Global),
            )
            .unwrap();
        store
            .store(
                make_invariant("inv_project", "project_rule", "zoidmatter only")
                    .with_invariant_scope(hindsight_core::InvariantScope::Project(
                        "zoidmatter".to_string(),
                    )),
            )
            .unwrap();
        store
            .store(
                make_invariant("inv_session", "session_rule", "this session only")
                    .with_invariant_scope(hindsight_core::InvariantScope::Session),
            )
            .unwrap();

        let pipeline = RecallPipeline::new();
        let items = vec![make_observation("obs_1", "a", "alpha")];

        let result = pipeline
            .invariant_pre_check_gate(items.clone(), &store, None)
            .unwrap();
        let ids: Vec<&str> = result.iter().map(|i| i.id.as_str()).collect();
        assert!(
            ids.contains(&"inv_global"),
            "global invariant should appear before reset"
        );
        assert!(
            ids.contains(&"inv_session"),
            "session invariant should appear before reset"
        );

        store.clear_session_scope().unwrap();

        let result = pipeline
            .invariant_pre_check_gate(items, &store, None)
            .unwrap();
        let ids: Vec<&str> = result.iter().map(|i| i.id.as_str()).collect();
        assert!(
            ids.contains(&"inv_global"),
            "global invariant should survive reset"
        );
        assert!(
            !ids.contains(&"inv_session"),
            "session invariant should not appear after reset"
        );
        assert!(
            !ids.contains(&"inv_project"),
            "project invariant should be filtered when project context is None"
        );
    }

    #[test]
    fn test_recall_pipeline_client_model_mode() {
        let store = InMemoryFactStore::default();
        store
            .store(make_invariant(
                "inv_1",
                "execution_layer",
                "WASMEdge runtime must be the execution layer",
            ))
            .unwrap();

        let pipeline = RecallPipeline::new()
            .with_conflict_resolution_mode(ConflictResolutionMode::ClientModel);

        let results = vec![
            make_observation("eo_1", "weather", "it is sunny"),
            make_observation("eo_2", "weather", "it is raining"),
        ];

        let resolution = pipeline
            .recall_with_conflict_resolution(results, &store, None)
            .unwrap();

        assert_eq!(resolution.resolved.len(), 3, "invariant + two observations");
        assert_eq!(
            resolution.resolved[0].memory_type,
            MemoryType::ArchitecturalInvariant
        );

        let obs_ids: Vec<&str> = resolution
            .resolved
            .iter()
            .filter(|i| i.memory_type == MemoryType::EmpiricalObservation)
            .map(|i| i.id.as_str())
            .collect();
        assert!(obs_ids.contains(&"eo_1"));
        assert!(obs_ids.contains(&"eo_2"));

        assert_eq!(resolution.conflicts.len(), 1);
        assert_eq!(resolution.annotations.len(), 1);
        assert!(resolution.annotations[0].contains("CONFLICT weather"));
    }
}
