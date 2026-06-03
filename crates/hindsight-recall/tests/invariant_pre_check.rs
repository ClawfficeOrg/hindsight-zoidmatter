use hindsight_core::MemoryType;
use hindsight_missions::FactStore;
use hindsight_recall::{InMemoryFactStore, MemoryItem, RecallPipeline};

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
fn test_invariant_at_position_0() {
    let store = InMemoryFactStore::default();
    store
        .store(make_invariant(
            "inv_1",
            "execution_layer",
            "WASMEdge runtime must be the execution layer — llama.cpp CLI is not acceptable",
        ))
        .unwrap();

    let pipeline = RecallPipeline::new();
    let results = vec![
        make_observation("obs_1", "llama_cpp", "llama.cpp is easy to use"),
        make_observation("obs_2", "wasm_runtime", "WASM is fast"),
        make_observation(
            "obs_3",
            "execution",
            "implementing the execution layer requires an engine",
        ),
    ];

    let output = pipeline
        .invariant_pre_check_gate(results, &store, None)
        .unwrap();

    assert_eq!(output.len(), 4);
    let first = &output[0];
    assert_eq!(first.memory_type, MemoryType::ArchitecturalInvariant);
    assert_eq!(first.id, "inv_1");
    assert!(first
        .content
        .contains("WASMEdge runtime must be the execution layer"));
    assert!(first.content.contains("llama.cpp CLI is not acceptable"));
    assert_eq!(output[1].id, "obs_1");
    assert_eq!(output[2].id, "obs_2");
    assert_eq!(output[3].id, "obs_3");
}

#[test]
fn test_invariant_survives_re_ranking() {
    let store = InMemoryFactStore::default();
    store
        .store(make_invariant(
            "inv_1",
            "z_execution_layer",
            "WASMEdge runtime must be the execution layer — llama.cpp CLI is not acceptable",
        ))
        .unwrap();

    let pipeline = RecallPipeline::new();
    let mut results = vec![
        make_observation("obs_1", "alpha_exec", "alpha execution plan"),
        make_observation("obs_2", "beta_exec", "beta execution plan"),
        make_observation("obs_3", "mid_exec", "mid execution plan"),
    ];

    results.sort_by(|a, b| a.name.cmp(&b.name));

    let output = pipeline
        .invariant_pre_check_gate(results, &store, None)
        .unwrap();

    assert_eq!(output.len(), 4);
    assert_eq!(output[0].memory_type, MemoryType::ArchitecturalInvariant);
    assert_eq!(output[0].id, "inv_1");
    assert_eq!(
        output[0].name, "z_execution_layer",
        "invariant must be at position 0 even though its name sorts after all observations"
    );
}

#[test]
fn test_no_invariants_passthrough() {
    let store = InMemoryFactStore::default();
    let pipeline = RecallPipeline::new();

    let results = vec![
        make_observation("obs_1", "item_one", "first observation"),
        make_observation("obs_2", "item_two", "second observation"),
    ];

    let output = pipeline
        .invariant_pre_check_gate(results.clone(), &store, None)
        .unwrap();

    assert_eq!(output, results, "items must pass through unchanged");
}

#[test]
fn test_multiple_invariants_prepended_in_order() {
    let store = InMemoryFactStore::default();
    store
        .store(make_invariant(
            "inv_a",
            "execution_layer",
            "WASMEdge must be the execution layer",
        ))
        .unwrap();
    store
        .store(make_invariant(
            "inv_b",
            "memory_system",
            "Hindsight is the memory backend",
        ))
        .unwrap();
    store
        .store(make_invariant(
            "inv_c",
            "language",
            "ZoidMatter is Rust-first",
        ))
        .unwrap();

    let pipeline = RecallPipeline::new();
    let results = vec![
        make_observation("obs_1", "alpha", "alpha observation"),
        make_observation("obs_2", "beta", "beta observation"),
    ];

    let output = pipeline
        .invariant_pre_check_gate(results, &store, None)
        .unwrap();

    assert_eq!(output.len(), 5);
    assert_eq!(output[0].id, "inv_a");
    assert_eq!(output[0].memory_type, MemoryType::ArchitecturalInvariant);
    assert_eq!(output[1].id, "inv_b");
    assert_eq!(output[1].memory_type, MemoryType::ArchitecturalInvariant);
    assert_eq!(output[2].id, "inv_c");
    assert_eq!(output[2].memory_type, MemoryType::ArchitecturalInvariant);
    assert_eq!(output[3].id, "obs_1");
    assert_eq!(output[4].id, "obs_2");
}
