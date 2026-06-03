use hindsight_core::InvariantScope;
use hindsight_missions::{FactStore, MemoryItem, MemoryType, MissionError};

pub fn add_invariant(
    store: &dyn FactStore,
    name: &str,
    content: &str,
    scope: InvariantScope,
) -> Result<MemoryItem, MissionError> {
    if name.trim().is_empty() {
        return Err(MissionError::Rejected("name must not be empty".into()));
    }
    if content.trim().is_empty() {
        return Err(MissionError::Rejected("content must not be empty".into()));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let item = MemoryItem::new(
        &id,
        name,
        content,
        MemoryType::ArchitecturalInvariant,
        "human",
    )
    .with_invariant_scope(scope);

    store.store(item.clone())?;

    Ok(item)
}

pub fn list_invariants(store: &dyn FactStore) -> Result<Vec<MemoryItem>, MissionError> {
    store.get_invariants()
}

pub fn remove_invariant(store: &dyn FactStore, id: &str) -> Result<(), MissionError> {
    if id.trim().is_empty() {
        return Err(MissionError::Rejected("id must not be empty".into()));
    }
    store.remove(id)
}
