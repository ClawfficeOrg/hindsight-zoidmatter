use hindsight_core::MemoryType;
use hindsight_missions::{
    ArchitecturalInvariantRetainMission, FactStore, InMemoryFactStore, MemoryItem, MissionError,
    RetainMission,
};

fn mk_item(name: &str, content: &str, memory_type: MemoryType, source: &str) -> MemoryItem {
    MemoryItem::new(format!("test-{}", name), name, content, memory_type, source)
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
                "rejection message should reference the item name: {}",
                msg
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
fn test_agent_cannot_masquerade_as_human() {
    let store = InMemoryFactStore::new(vec![]);

    let mission = ArchitecturalInvariantRetainMission;
    let result = mission.process(
        vec![mk_item(
            "secret",
            "agent pretends to be human",
            MemoryType::ArchitecturalInvariant,
            "human",
        )],
        &store,
    );

    let accepted = result.expect("first invariant from any source should be accepted");
    assert_eq!(accepted.len(), 1);
    store
        .store(accepted[0].clone())
        .expect("store should succeed");

    let result2 = mission.process(
        vec![mk_item(
            "secret",
            "this would override",
            MemoryType::ArchitecturalInvariant,
            "agent",
        )],
        &store,
    );

    match result2 {
        Err(MissionError::Rejected(_)) => {}
        other => panic!(
            "agent cannot override an existing invariant, got {:?}",
            other
        ),
    }
}
