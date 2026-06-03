pub mod conflict;
pub mod priority;
pub mod recall;

#[cfg(feature = "mitm_resolver")]
pub mod mitm;

pub use conflict::{
    detect_conflicts, resolve_conflicts, resolve_conflicts_client_model,
    resolve_conflicts_hard_rule, resolve_conflicts_with_mode, ConflictPair, ConflictResolution,
    ConflictResolutionMode,
};
pub use priority::{consolidate_observations, PriorityStackResolver};
pub use recall::RecallPipeline;

#[cfg(feature = "mitm_resolver")]
pub use mitm::{
    resolve_conflicts_with_model, MitmConflictInput, MitmDecision, MitmItemView, MitmModelProvider,
    StubMitmModelProvider,
};

pub use hindsight_core::MemoryType;
pub use hindsight_missions::{FactStore, InMemoryFactStore, MemoryItem, MissionError};
