use hindsight_core::MemoryType;
use hindsight_missions::FactStore;
use hindsight_recall::{InMemoryFactStore, MemoryItem, RecallPipeline};

fn item_of_type(mt: MemoryType, id: &str, ts: Option<i64>, multiplier: Option<f64>) -> MemoryItem {
    let mut item = MemoryItem::new(id, id, "content", mt, "test");
    if let Some(t) = ts {
        item = item.with_created_at(t);
    }
    if let Some(m) = multiplier {
        item = item.with_priority_multiplier(m);
    }
    item
}

fn make_invariant(id: &str, name: &str, content: &str) -> MemoryItem {
    MemoryItem::new(
        id,
        name,
        content,
        MemoryType::ArchitecturalInvariant,
        "human",
    )
}

#[test]
fn test_recall_pipeline_with_priority() {
    let store = InMemoryFactStore::default();
    store
        .store(
            make_invariant(
                "inv_1",
                "execution_layer",
                "WASMEdge must be the execution layer",
            )
            .with_created_at(100),
        )
        .unwrap();
    store
        .store(
            make_invariant("inv_2", "memory_system", "Hindsight is the memory backend")
                .with_created_at(200),
        )
        .unwrap();

    let pipeline = RecallPipeline::new();
    let results = vec![
        item_of_type(MemoryType::HardFact, "hf_1", Some(500), None),
        item_of_type(MemoryType::ConversationalContext, "cc_1", Some(300), None),
        item_of_type(MemoryType::EmpiricalObservation, "eo_1", Some(100), None),
        item_of_type(MemoryType::ExplicitMentalModel, "emm_1", Some(400), None),
    ];

    let output = pipeline
        .recall_with_priority(results, &store, None)
        .unwrap();

    let ids: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();

    let ai_pos = ids.iter().position(|&id| id == "inv_1" || id == "inv_2");
    let emm_pos = ids.iter().position(|&id| id == "emm_1");
    let hf_pos = ids.iter().position(|&id| id == "hf_1");
    let eo_pos = ids.iter().position(|&id| id == "eo_1");
    let cc_pos = ids.iter().position(|&id| id == "cc_1");

    assert!(ai_pos.is_some(), "invariants must appear");
    assert!(ai_pos < emm_pos, "invariants before ExplicitMentalModel");
    assert!(emm_pos < hf_pos, "ExplicitMentalModel before HardFact");
    assert!(hf_pos < eo_pos, "HardFact before EmpiricalObservation");
    assert!(
        eo_pos < cc_pos,
        "EmpiricalObservation before ConversationalContext"
    );
}

#[test]
fn test_invariants_prepended_then_sorted_by_recency() {
    let store = InMemoryFactStore::default();
    store
        .store(make_invariant("inv_a", "alpha", "alpha invariant").with_created_at(300))
        .unwrap();
    store
        .store(make_invariant("inv_b", "beta", "beta invariant").with_created_at(100))
        .unwrap();
    store
        .store(make_invariant("inv_c", "gamma", "gamma invariant").with_created_at(200))
        .unwrap();

    let pipeline = RecallPipeline::new();
    let results = vec![
        item_of_type(MemoryType::EmpiricalObservation, "obs_1", Some(50), None),
        item_of_type(MemoryType::EmpiricalObservation, "obs_2", Some(60), None),
    ];

    let output = pipeline
        .recall_with_priority(results, &store, None)
        .unwrap();

    assert_eq!(output.len(), 5);

    assert_eq!(output[0].memory_type, MemoryType::ArchitecturalInvariant);
    assert_eq!(output[1].memory_type, MemoryType::ArchitecturalInvariant);
    assert_eq!(output[2].memory_type, MemoryType::ArchitecturalInvariant);

    assert!(output[0].id.starts_with("inv_"));
    assert!(output[1].id.starts_with("inv_"));
    assert!(output[2].id.starts_with("inv_"));
    let mut inv_ids: Vec<&str> = output[0..3].iter().map(|i| i.id.as_str()).collect();
    inv_ids.sort();
    assert_eq!(inv_ids, vec!["inv_a", "inv_b", "inv_c"]);

    assert_eq!(output[3].memory_type, MemoryType::EmpiricalObservation);
    assert_eq!(output[4].memory_type, MemoryType::EmpiricalObservation);
    assert_eq!(output[3].id, "obs_2");
    assert_eq!(output[4].id, "obs_1");
}

#[test]
fn test_property_randomized_results() {
    use rand::seq::SliceRandom;
    use rand::thread_rng;
    use rand::Rng;

    let pipeline = RecallPipeline::new();
    let mut rng = thread_rng();
    let all_types = [
        MemoryType::ArchitecturalInvariant,
        MemoryType::ExplicitMentalModel,
        MemoryType::HardFact,
        MemoryType::EmpiricalObservation,
        MemoryType::ConversationalContext,
    ];

    for _ in 0..100 {
        let count = (rng.gen::<usize>() % 20) + 5;
        let mut items: Vec<MemoryItem> = Vec::with_capacity(count);

        for idx in 0..count {
            let mt = all_types[rng.gen::<usize>() % all_types.len()].clone();
            let ts = if rng.gen::<bool>() {
                Some(rng.gen::<i64>() % 10000)
            } else {
                None
            };
            let multiplier = if rng.gen::<bool>() {
                Some(rng.gen::<f64>() * 5.0)
            } else {
                None
            };

            let mut item = MemoryItem::new(
                format!("item_{}", idx),
                format!("name_{}", idx),
                format!("content_{}", idx),
                mt,
                "test",
            );
            if let Some(t) = ts {
                item = item.with_created_at(t);
            }
            if let Some(m) = multiplier {
                item = item.with_priority_multiplier(m);
            }
            items.push(item);
        }

        items.shuffle(&mut rng);

        let output = pipeline.resolve_priority_stack(items);
        check_ordering_properties(&output);
    }
}

fn is_recency_weighted(memory_type: &MemoryType, version: Option<&str>) -> bool {
    match memory_type {
        MemoryType::ArchitecturalInvariant | MemoryType::HardFact => false,
        MemoryType::ExplicitMentalModel => version.is_some(),
        MemoryType::EmpiricalObservation | MemoryType::ConversationalContext => true,
    }
}

fn check_ordering_properties(output: &[MemoryItem]) {
    let tier_order = [
        MemoryType::ArchitecturalInvariant,
        MemoryType::ExplicitMentalModel,
        MemoryType::HardFact,
        MemoryType::EmpiricalObservation,
        MemoryType::ConversationalContext,
    ];

    for i in 0..output.len() {
        for j in (i + 1)..output.len() {
            let type_i = &output[i].memory_type;
            let type_j = &output[j].memory_type;

            let rank_i = tier_order.iter().position(|t| t == type_i).unwrap();
            let rank_j = tier_order.iter().position(|t| t == type_j).unwrap();

            assert!(
                rank_i <= rank_j,
                "item {} (type {:?}, rank {}) must not appear after item {} (type {:?}, rank {})",
                output[i].id,
                type_i,
                rank_i,
                output[j].id,
                type_j,
                rank_j,
            );

            if rank_i == rank_j && is_recency_weighted(type_i, output[i].version.as_deref()) {
                match (output[i].created_at, output[j].created_at) {
                    (Some(ta), Some(tb)) => {
                        assert!(
                            ta >= tb,
                            "same-type items with timestamps must be descending: {} (ts={}) before {} (ts={})",
                            output[i].id, ta, output[j].id, tb
                        );
                    }
                    (Some(_), None) => {}
                    (None, Some(_)) => {
                        panic!(
                            "item {} with no timestamp must not appear before item {} with timestamp",
                            output[i].id, output[j].id
                        );
                    }
                    (None, None) => {}
                }
            }
        }
    }
}
