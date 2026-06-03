use std::sync::Mutex;

use crate::error::MissionError;
use crate::traits::FactStore;
use crate::MemoryItem;

pub struct InMemoryFactStore {
    items: Mutex<Vec<MemoryItem>>,
}

impl InMemoryFactStore {
    pub fn new(items: Vec<MemoryItem>) -> Self {
        Self {
            items: Mutex::new(items),
        }
    }

    pub fn all_items(&self) -> Result<Vec<MemoryItem>, MissionError> {
        let items = self
            .items
            .lock()
            .map_err(|e| MissionError::Query(format!("lock poisoned: {}", e)))?;
        Ok(items.clone())
    }

    pub fn clear(&self) -> Result<(), MissionError> {
        let mut items = self
            .items
            .lock()
            .map_err(|e| MissionError::Storage(format!("lock poisoned: {}", e)))?;
        items.clear();
        Ok(())
    }
}

impl Default for InMemoryFactStore {
    fn default() -> Self {
        Self {
            items: Mutex::new(Vec::new()),
        }
    }
}

impl FactStore for InMemoryFactStore {
    fn store(&self, item: MemoryItem) -> Result<(), MissionError> {
        let mut items = self
            .items
            .lock()
            .map_err(|e| MissionError::Storage(format!("lock poisoned: {}", e)))?;
        items.push(item);
        Ok(())
    }

    fn get_invariants(&self) -> Result<Vec<MemoryItem>, MissionError> {
        let items = self
            .items
            .lock()
            .map_err(|e| MissionError::Query(format!("lock poisoned: {}", e)))?;
        Ok(items
            .iter()
            .filter(|i| i.memory_type == crate::MemoryType::ArchitecturalInvariant)
            .cloned()
            .collect())
    }

    fn clear_session_scope(&self) -> Result<(), MissionError> {
        let mut items = self
            .items
            .lock()
            .map_err(|e| MissionError::Storage(format!("lock poisoned: {}", e)))?;
        items.retain(|i| i.invariant_scope.as_ref().is_none_or(|s| !s.is_session()));
        Ok(())
    }

    fn remove(&self, id: &str) -> Result<(), MissionError> {
        let mut items = self
            .items
            .lock()
            .map_err(|e| MissionError::Storage(format!("lock poisoned: {}", e)))?;
        let len_before = items.len();
        items.retain(|i| i.id != id);
        if items.len() == len_before {
            return Err(MissionError::NotFound(format!("no item with id: {}", id)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InvariantScope;
    use crate::MemoryType;

    #[test]
    fn test_clear_session_scope_removes_only_session_items() {
        let store = InMemoryFactStore::default();

        let global = MemoryItem::new(
            "g1",
            "global_rule",
            "always applies",
            MemoryType::ArchitecturalInvariant,
            "human",
        )
        .with_invariant_scope(InvariantScope::Global);

        let project = MemoryItem::new(
            "p1",
            "project_rule",
            "applies to zoidmatter",
            MemoryType::ArchitecturalInvariant,
            "human",
        )
        .with_invariant_scope(InvariantScope::Project("zoidmatter".to_string()));

        let session = MemoryItem::new(
            "s1",
            "session_rule",
            "only this session",
            MemoryType::ArchitecturalInvariant,
            "agent",
        )
        .with_invariant_scope(InvariantScope::Session);

        let ordinary = MemoryItem::new(
            "o1",
            "ordinary",
            "just a fact",
            MemoryType::EmpiricalObservation,
            "agent",
        );

        store.store(global).unwrap();
        store.store(project).unwrap();
        store.store(session).unwrap();
        store.store(ordinary).unwrap();

        store.clear_session_scope().unwrap();

        let invariants = store.get_invariants().unwrap();
        let ids: Vec<&str> = invariants.iter().map(|i| i.id.as_str()).collect();

        assert!(ids.contains(&"g1"), "global invariant should survive");
        assert!(ids.contains(&"p1"), "project invariant should survive");
        assert!(!ids.contains(&"s1"), "session invariant should be removed");
    }
}
