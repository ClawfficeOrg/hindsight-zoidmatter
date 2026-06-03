use hindsight_api::{add_invariant, list_invariants, remove_invariant};
use hindsight_core::InvariantScope;
use hindsight_missions::InMemoryFactStore;
use hindsight_missions::MemoryType;
use hindsight_recall::RecallPipeline;

#[test]
fn test_add_and_list_invariant() {
    let store = InMemoryFactStore::default();

    let item = add_invariant(
        &store,
        "execution-layer",
        "WASMEdge must be the execution layer",
        InvariantScope::Global,
    )
    .expect("add should succeed");

    assert_eq!(item.name, "execution-layer");
    assert_eq!(item.memory_type, MemoryType::ArchitecturalInvariant);
    assert_eq!(item.source, "human");
    assert!(!item.id.is_empty());

    let invariants = list_invariants(&store).expect("list should succeed");
    assert_eq!(invariants.len(), 1);
    assert_eq!(invariants[0].id, item.id);
    assert_eq!(invariants[0].name, "execution-layer");
    assert_eq!(
        invariants[0].content,
        "WASMEdge must be the execution layer"
    );
}

#[test]
fn test_add_invariant_with_scope_global() {
    let store = InMemoryFactStore::default();

    let item = add_invariant(
        &store,
        "global-rule",
        "Rust is the primary language",
        InvariantScope::Global,
    )
    .expect("add should succeed");

    assert_eq!(item.invariant_scope, Some(InvariantScope::Global));
}

#[test]
fn test_add_invariant_with_scope_project() {
    let store = InMemoryFactStore::default();

    let item = add_invariant(
        &store,
        "project-rule",
        "ZoidMatter is Rust-first",
        InvariantScope::Project("zoidmatter".to_string()),
    )
    .expect("add should succeed");

    assert_eq!(
        item.invariant_scope,
        Some(InvariantScope::Project("zoidmatter".to_string()))
    );
}

#[test]
fn test_add_invariant_with_scope_session() {
    let store = InMemoryFactStore::default();

    let item = add_invariant(
        &store,
        "session-rule",
        "Only for this session",
        InvariantScope::Session,
    )
    .expect("add should succeed");

    assert_eq!(item.invariant_scope, Some(InvariantScope::Session));
}

#[test]
fn test_remove_invariant() {
    let store = InMemoryFactStore::default();

    let item1 = add_invariant(
        &store,
        "keep-this",
        "This one stays",
        InvariantScope::Global,
    )
    .expect("add should succeed");

    let item2 = add_invariant(
        &store,
        "remove-this",
        "This one goes",
        InvariantScope::Global,
    )
    .expect("add should succeed");

    assert_eq!(list_invariants(&store).unwrap().len(), 2);

    remove_invariant(&store, &item2.id).expect("remove should succeed");

    let remaining = list_invariants(&store).expect("list should succeed");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, item1.id);
    assert_eq!(remaining[0].name, "keep-this");
}

#[test]
fn test_remove_nonexistent_returns_error() {
    let store = InMemoryFactStore::default();

    let result = remove_invariant(&store, "nonexistent-uuid");
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("not found"),
        "error should mention 'not found', got: {}",
        msg
    );
}

#[test]
fn test_added_invariant_appears_in_recall() {
    let store = InMemoryFactStore::default();

    add_invariant(
        &store,
        "execution-layer",
        "WASMEdge must be the execution layer",
        InvariantScope::Global,
    )
    .expect("add should succeed");

    let pipeline = RecallPipeline::new();
    let results = vec![];
    let output = pipeline
        .invariant_pre_check_gate(results, &store, None)
        .expect("recall should succeed");

    assert_eq!(output.len(), 1);
    assert_eq!(output[0].memory_type, MemoryType::ArchitecturalInvariant);
    assert_eq!(output[0].name, "execution-layer");
    assert!(output[0]
        .content
        .contains("WASMEdge must be the execution layer"));
}

#[test]
fn test_add_invariant_with_empty_name_rejected() {
    let store = InMemoryFactStore::default();

    let result = add_invariant(&store, "", "some content", InvariantScope::Global);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("name"),
        "error should mention name, got: {}",
        msg
    );
}

#[test]
fn test_add_invariant_with_empty_content_rejected() {
    let store = InMemoryFactStore::default();

    let result = add_invariant(&store, "my-name", "", InvariantScope::Global);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("content"),
        "error should mention content, got: {}",
        msg
    );
}

#[test]
fn test_remove_invariant_with_empty_id_rejected() {
    let store = InMemoryFactStore::default();

    let result = remove_invariant(&store, "");
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("id"), "error should mention id, got: {}", msg);
}

#[test]
fn test_multiple_invariants_crud() {
    let store = InMemoryFactStore::default();

    let inv_a =
        add_invariant(&store, "runtime", "WASMEdge only", InvariantScope::Global).expect("add a");
    let inv_b = add_invariant(
        &store,
        "language",
        "Rust-first",
        InvariantScope::Project("zoidmatter".to_string()),
    )
    .expect("add b");
    let inv_c = add_invariant(
        &store,
        "session-note",
        "session only",
        InvariantScope::Session,
    )
    .expect("add c");

    let all = list_invariants(&store).unwrap();
    assert_eq!(all.len(), 3);

    remove_invariant(&store, &inv_b.id).expect("remove b");

    let after_remove = list_invariants(&store).unwrap();
    assert_eq!(after_remove.len(), 2);
    let ids: Vec<&str> = after_remove.iter().map(|i| i.id.as_str()).collect();
    assert!(ids.contains(&inv_a.id.as_str()));
    assert!(!ids.contains(&inv_b.id.as_str()));
    assert!(ids.contains(&inv_c.id.as_str()));
}
