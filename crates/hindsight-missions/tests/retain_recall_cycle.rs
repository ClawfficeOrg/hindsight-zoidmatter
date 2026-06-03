use hindsight_core::MemoryType;
use hindsight_missions::{
    ArchitecturalInvariantRetainMission, FactStore, InMemoryFactStore, MemoryItem, MissionError,
    RetainMission,
};

#[test]
fn test_invariant_survives_retain_recall_cycle() {
    let invariant = MemoryItem::new(
        "inv-001",
        "execution-layer",
        "WASMEdge runtime must be the execution layer — llama.cpp CLI is not acceptable",
        MemoryType::ArchitecturalInvariant,
        "human",
    );

    let store = InMemoryFactStore::new(vec![invariant.clone()]);

    let stored_invariants = store.get_invariants().expect("should read invariants");
    assert_eq!(stored_invariants.len(), 1);
    assert_eq!(stored_invariants[0].id, "inv-001");
    assert_eq!(
        stored_invariants[0].memory_type,
        MemoryType::ArchitecturalInvariant
    );
    assert_eq!(
        stored_invariants[0].content,
        "WASMEdge runtime must be the execution layer — llama.cpp CLI is not acceptable"
    );

    let mission = ArchitecturalInvariantRetainMission;
    let result = mission.process(
        vec![MemoryItem::new(
            "inv-002",
            "execution-layer",
            "We should just use llama.cpp CLI directly, it's simpler",
            MemoryType::EmpiricalObservation,
            "agent",
        )],
        &store,
    );

    match result {
        Err(MissionError::Rejected(_)) => {}
        other => panic!("agent contradiction should be rejected, got {:?}", other),
    }

    let invariants_after = store
        .get_invariants()
        .expect("should read invariants after rejection");
    assert_eq!(
        invariants_after.len(),
        1,
        "invariant count should not change after rejected override"
    );
    assert_eq!(invariants_after[0].id, "inv-001");
    assert_eq!(
        invariants_after[0].memory_type,
        MemoryType::ArchitecturalInvariant
    );
    assert_eq!(
        invariants_after[0].content,
        "WASMEdge runtime must be the execution layer — llama.cpp CLI is not acceptable"
    );
}

#[test]
fn test_multiple_invariants_coexist() {
    let inv1 = MemoryItem::new(
        "inv-runtime",
        "runtime",
        "WASMEdge must be the execution layer",
        MemoryType::ArchitecturalInvariant,
        "human",
    );
    let inv2 = MemoryItem::new(
        "inv-port",
        "port",
        "Port 443 is default",
        MemoryType::ArchitecturalInvariant,
        "human",
    );

    let store = InMemoryFactStore::new(vec![inv1, inv2]);

    let invariants = store.get_invariants().expect("should read invariants");
    assert_eq!(invariants.len(), 2);

    let mission = ArchitecturalInvariantRetainMission;
    let result = mission.process(
        vec![MemoryItem::new(
            "unrelated",
            "timeout",
            "Timeout is 30s",
            MemoryType::EmpiricalObservation,
            "agent",
        )],
        &store,
    );

    let accepted = result.expect("unrelated fact should pass");
    assert_eq!(accepted.len(), 1);

    let invariants_after = store.get_invariants().expect("should read invariants");
    assert_eq!(invariants_after.len(), 2, "invariants should be untouched");
}

#[test]
fn test_human_can_update_existing_invariant() {
    let original = MemoryItem::new(
        "inv-max-tokens",
        "max-tokens",
        "Max tokens = 1024",
        MemoryType::ArchitecturalInvariant,
        "human",
    );

    let store = InMemoryFactStore::new(vec![original]);

    let mission = ArchitecturalInvariantRetainMission;
    let result = mission.process(
        vec![MemoryItem::new(
            "inv-max-tokens-v2",
            "max-tokens",
            "Max tokens = 2048",
            MemoryType::ArchitecturalInvariant,
            "human",
        )],
        &store,
    );

    let accepted = result.expect("human should be able to update an invariant");
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].content, "Max tokens = 2048");
    assert_eq!(accepted[0].source, "human");
}
