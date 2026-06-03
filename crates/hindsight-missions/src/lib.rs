pub mod error;
pub mod fact_store;
pub mod registry;
pub mod retain_invariant;
pub mod traits;

pub use error::MissionError;
pub use fact_store::InMemoryFactStore;
pub use hindsight_core::{InvariantScope, MemoryType};
pub use registry::MissionRegistry;
pub use retain_invariant::ArchitecturalInvariantRetainMission;
pub use traits::{FactStore, RetainMission};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub name: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub source: String,
    pub tags: Vec<String>,
    pub invariant_scope: Option<InvariantScope>,
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub base_priority_multiplier: Option<f64>,
    #[serde(default)]
    pub version: Option<String>,
}

impl PartialEq for MemoryItem {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.content == other.content
            && self.memory_type == other.memory_type
            && self.source == other.source
            && self.tags == other.tags
            && self.invariant_scope == other.invariant_scope
    }
}

impl Eq for MemoryItem {}

impl MemoryItem {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
        memory_type: MemoryType,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            content: content.into(),
            memory_type,
            source: source.into(),
            tags: Vec::new(),
            invariant_scope: None,
            created_at: None,
            base_priority_multiplier: None,
            version: None,
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_invariant_scope(mut self, scope: InvariantScope) -> Self {
        self.invariant_scope = Some(scope);
        self
    }

    pub fn with_created_at(mut self, ts: i64) -> Self {
        self.created_at = Some(ts);
        self
    }

    pub fn with_priority_multiplier(mut self, m: f64) -> Self {
        self.base_priority_multiplier = Some(m);
        self
    }

    pub fn with_version(mut self, v: impl Into<String>) -> Self {
        self.version = Some(v.into());
        self
    }
}
