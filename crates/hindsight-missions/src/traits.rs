use crate::error::MissionError;
use crate::MemoryItem;

pub trait RetainMission: Send + Sync {
    fn name(&self) -> &str;

    fn process(
        &self,
        items: Vec<MemoryItem>,
        store: &dyn FactStore,
    ) -> Result<Vec<MemoryItem>, MissionError>;
}

pub trait FactStore: Send + Sync {
    fn store(&self, item: MemoryItem) -> Result<(), MissionError>;

    fn get_invariants(&self) -> Result<Vec<MemoryItem>, MissionError>;

    fn clear_session_scope(&self) -> Result<(), MissionError>;

    fn remove(&self, id: &str) -> Result<(), MissionError>;
}
