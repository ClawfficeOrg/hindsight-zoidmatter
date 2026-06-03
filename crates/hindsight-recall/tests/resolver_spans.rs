use hindsight_core::MemoryType;
use hindsight_missions::MemoryItem;
use hindsight_recall::{
    resolve_conflicts_client_model, resolve_conflicts_hard_rule, ConflictPair, ConflictResolution,
};
use tracing_test::traced_test;

fn make_item(id: &str, name: &str, content: &str, mt: MemoryType) -> MemoryItem {
    MemoryItem::new(id, name, content, mt, "test")
}

#[traced_test]
#[test]
fn test_hard_rule_span_fields() {
    let hf = make_item("hf_1", "weather", "hard fact wins", MemoryType::HardFact);
    let eo_a = make_item("eo_a", "weather", "obs a", MemoryType::EmpiricalObservation);
    let eo_b = make_item("eo_b", "weather", "obs b", MemoryType::EmpiricalObservation);

    let result = ConflictResolution {
        resolved: vec![hf, eo_a, eo_b],
        conflicts: vec![
            ConflictPair {
                item_a_id: "hf_1".into(),
                item_b_id: "eo_a".into(),
                subject: "weather".into(),
            },
            ConflictPair {
                item_a_id: "hf_1".into(),
                item_b_id: "eo_b".into(),
                subject: "weather".into(),
            },
        ],
        annotations: Vec::new(),
    };

    let _output = resolve_conflicts_hard_rule(result);

    assert!(logs_contain("resolver_type"));
    assert!(logs_contain("hard_rule"));
    assert!(logs_contain("conflicts_resolved"));
    assert!(logs_contain("latency_us"));
}

#[traced_test]
#[test]
fn test_client_model_span_fields() {
    let eo_a = make_item(
        "eo_1",
        "weather",
        "the sky is blue",
        MemoryType::EmpiricalObservation,
    );
    let eo_b = make_item(
        "eo_2",
        "weather",
        "the sky is green",
        MemoryType::EmpiricalObservation,
    );

    let pair = ConflictPair {
        item_a_id: "eo_1".into(),
        item_b_id: "eo_2".into(),
        subject: "weather".into(),
    };

    let result = ConflictResolution {
        resolved: vec![eo_a, eo_b],
        conflicts: vec![pair],
        annotations: Vec::new(),
    };

    let _output = resolve_conflicts_client_model(result);

    assert!(logs_contain("resolver_type"));
    assert!(logs_contain("client_model"));
    assert!(logs_contain("conflicts_resolved"));
    assert!(logs_contain("latency_us"));
}

#[cfg(feature = "mitm_resolver")]
mod mitm_tests {
    use super::*;
    use hindsight_recall::{resolve_conflicts_with_model, StubMitmModelProvider};

    #[traced_test]
    #[test]
    fn test_mitm_span_fields() {
        let a = make_item("a", "subj", "content a", MemoryType::EmpiricalObservation);
        let b = make_item("b", "subj", "content b", MemoryType::EmpiricalObservation);

        let input_result = ConflictResolution {
            resolved: vec![a, b],
            conflicts: vec![ConflictPair {
                item_a_id: "a".into(),
                item_b_id: "b".into(),
                subject: "subj".into(),
            }],
            annotations: Vec::new(),
        };

        let _output = resolve_conflicts_with_model(input_result, &StubMitmModelProvider);

        assert!(logs_contain("resolver_type"));
        assert!(logs_contain("mitm"));
        assert!(logs_contain("conflicts_resolved"));
        assert!(logs_contain("latency_us"));
    }
}
