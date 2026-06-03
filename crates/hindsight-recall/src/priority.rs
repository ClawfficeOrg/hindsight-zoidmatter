use std::collections::HashMap;

use hindsight_core::MemoryType;
use hindsight_missions::MemoryItem;

fn is_recency_weighted(memory_type: &MemoryType, version: Option<&str>) -> bool {
    match memory_type {
        MemoryType::ArchitecturalInvariant | MemoryType::HardFact => false,
        MemoryType::ExplicitMentalModel => version.is_some(),
        MemoryType::EmpiricalObservation | MemoryType::ConversationalContext => true,
    }
}

pub struct PriorityStackResolver;

impl PriorityStackResolver {
    pub fn resolve(results: Vec<MemoryItem>) -> Vec<MemoryItem> {
        let mut results = results;
        results.sort_by(|a, b| {
            let type_cmp = b.memory_type.cmp(&a.memory_type);
            if type_cmp != std::cmp::Ordering::Equal {
                return type_cmp;
            }

            if !is_recency_weighted(&a.memory_type, a.version.as_deref())
                && !is_recency_weighted(&b.memory_type, b.version.as_deref())
            {
                return std::cmp::Ordering::Equal;
            }

            match (a.created_at, b.created_at) {
                (Some(ta), Some(tb)) => tb.cmp(&ta),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });
        results
    }
}

/// Consolidates identical EmpiricalObservations by content.
///
/// Groups EOs with the same `content` string. Groups with size ≤ `threshold`
/// pass through unchanged. Groups with size > `threshold` are consolidated
/// into a single item with a dampened `base_priority_multiplier` calculated as:
///
/// ```text
/// multiplier = 6.0 * (1 - threshold / count) / EmpiricalObservation.default_weight()
/// ```
///
/// The dampening curve ensures `effective_weight(multiplier)` is always ≤ 6.0,
/// approaching 6.0 as duplicate count increases. This prevents high-frequency
/// observations from accumulating unbounded priority.
///
/// The consolidated item keeps the `created_at` of the most recent item in the
/// group. Non-EmpiricalObservation items pass through unchanged.
///
/// `threshold` of 0 is treated as "consolidate all groups into 1 item each".
pub fn consolidate_observations(results: Vec<MemoryItem>, threshold: usize) -> Vec<MemoryItem> {
    let (eos, others): (Vec<MemoryItem>, Vec<MemoryItem>) = results
        .into_iter()
        .partition(|item| item.memory_type == MemoryType::EmpiricalObservation);

    let mut groups: HashMap<String, Vec<MemoryItem>> = HashMap::new();
    for eo in eos {
        groups.entry(eo.content.clone()).or_default().push(eo);
    }

    let default_weight = MemoryType::EmpiricalObservation.default_weight();
    let mut output = others;

    for (_, mut group) in groups {
        let count = group.len();
        if count <= threshold {
            output.extend(group);
        } else {
            let dampened_multiplier =
                6.0 * (1.0 - threshold as f64 / count as f64) / default_weight;
            group.sort_by_key(|b| std::cmp::Reverse(b.created_at));
            let mut consolidated = group.remove(0);
            consolidated.base_priority_multiplier = Some(dampened_multiplier);
            output.push(consolidated);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use hindsight_core::MemoryType;

    fn item_of_type(
        mt: MemoryType,
        id: &str,
        ts: Option<i64>,
        multiplier: Option<f64>,
    ) -> MemoryItem {
        let mut item = MemoryItem::new(id, id, "content", mt, "test");
        if let Some(t) = ts {
            item = item.with_created_at(t);
        }
        if let Some(m) = multiplier {
            item = item.with_priority_multiplier(m);
        }
        item
    }

    fn item_with_version(
        mt: MemoryType,
        id: &str,
        ts: Option<i64>,
        version: Option<&str>,
    ) -> MemoryItem {
        let mut item = MemoryItem::new(id, id, "content", mt, "test");
        if let Some(t) = ts {
            item = item.with_created_at(t);
        }
        if let Some(v) = version {
            item = item.with_version(v);
        }
        item
    }

    #[test]
    fn test_inter_tier_ordering_all_types() {
        let input = vec![
            item_of_type(MemoryType::ConversationalContext, "cc", None, None),
            item_of_type(MemoryType::EmpiricalObservation, "eo", None, None),
            item_of_type(MemoryType::HardFact, "hf", None, None),
            item_of_type(MemoryType::ExplicitMentalModel, "emm", None, None),
            item_of_type(MemoryType::ArchitecturalInvariant, "ai", None, None),
        ];

        let output = PriorityStackResolver::resolve(input);

        let names: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(names, vec!["ai", "emm", "hf", "eo", "cc"]);
    }

    #[test]
    fn test_inter_tier_ordering_reversed_input() {
        let input = vec![
            item_of_type(MemoryType::ArchitecturalInvariant, "ai", None, None),
            item_of_type(MemoryType::ExplicitMentalModel, "emm", None, None),
            item_of_type(MemoryType::HardFact, "hf", None, None),
            item_of_type(MemoryType::EmpiricalObservation, "eo", None, None),
            item_of_type(MemoryType::ConversationalContext, "cc", None, None),
        ];

        let output = PriorityStackResolver::resolve(input);

        let names: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(names, vec!["ai", "emm", "hf", "eo", "cc"]);
    }

    #[test]
    fn test_within_tier_recency_eo() {
        let input = vec![
            item_of_type(MemoryType::EmpiricalObservation, "old", Some(100), None),
            item_of_type(MemoryType::EmpiricalObservation, "mid", Some(200), None),
            item_of_type(MemoryType::EmpiricalObservation, "new", Some(300), None),
        ];

        let output = PriorityStackResolver::resolve(input);

        let names: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(names, vec!["new", "mid", "old"]);
    }

    #[test]
    fn test_within_tier_unknown_recency() {
        let input = vec![
            item_of_type(MemoryType::EmpiricalObservation, "no_ts_1", None, None),
            item_of_type(MemoryType::EmpiricalObservation, "known", Some(100), None),
            item_of_type(MemoryType::EmpiricalObservation, "no_ts_2", None, None),
        ];

        let output = PriorityStackResolver::resolve(input);

        let names: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(names[0], "known");
        assert!(names[1..].contains(&"no_ts_1"));
        assert!(names[1..].contains(&"no_ts_2"));
    }

    #[test]
    fn test_multiplier_does_not_violate_tier_boundary() {
        let input = vec![
            item_of_type(
                MemoryType::ArchitecturalInvariant,
                "ai_low_mult",
                None,
                Some(0.01),
            ),
            item_of_type(
                MemoryType::ExplicitMentalModel,
                "emm_high_mult",
                None,
                Some(10.0),
            ),
        ];

        let output = PriorityStackResolver::resolve(input);

        let names: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            names,
            vec!["ai_low_mult", "emm_high_mult"],
            "ArchitecturalInvariant must come before ExplicitMentalModel regardless of multiplier"
        );
    }

    #[test]
    fn test_multiplier_scales_within_tier_effective_weight() {
        let base = MemoryType::EmpiricalObservation.default_weight();

        let weight_none = MemoryType::EmpiricalObservation.effective_weight(None);
        assert!((weight_none - base).abs() < 1e-10);

        let weight_2x = MemoryType::EmpiricalObservation.effective_weight(Some(2.0));
        assert!((weight_2x - base * 2.0).abs() < 1e-10);

        let weight_half = MemoryType::EmpiricalObservation.effective_weight(Some(0.5));
        assert!((weight_half - base * 0.5).abs() < 1e-10);

        let weight_zero = MemoryType::EmpiricalObservation.effective_weight(Some(0.0));
        assert!((weight_zero - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_empty_results() {
        let output = PriorityStackResolver::resolve(Vec::new());
        assert!(output.is_empty());
    }

    #[test]
    fn test_single_item() {
        let input = vec![item_of_type(
            MemoryType::HardFact,
            "only",
            Some(42),
            Some(1.5),
        )];

        let output = PriorityStackResolver::resolve(input);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].id, "only");
    }

    #[test]
    fn test_mixed_with_unknown_and_known_timestamps() {
        let input = vec![
            item_of_type(MemoryType::ArchitecturalInvariant, "ai_none", None, None),
            item_of_type(
                MemoryType::ArchitecturalInvariant,
                "ai_old",
                Some(100),
                None,
            ),
            item_of_type(
                MemoryType::ArchitecturalInvariant,
                "ai_new",
                Some(300),
                None,
            ),
            item_of_type(MemoryType::HardFact, "hf_none", None, None),
            item_of_type(MemoryType::HardFact, "hf_mid", Some(200), None),
        ];

        let output = PriorityStackResolver::resolve(input);

        let ids: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();

        let ai_pos: Vec<usize> = ids
            .iter()
            .enumerate()
            .filter(|(_, id)| id.starts_with("ai_"))
            .map(|(i, _)| i)
            .collect();
        let hf_pos: Vec<usize> = ids
            .iter()
            .enumerate()
            .filter(|(_, id)| id.starts_with("hf_"))
            .map(|(i, _)| i)
            .collect();

        assert!(
            ai_pos.iter().all(|p| hf_pos.iter().all(|h| p < h)),
            "All ArchitecturalInvariants must precede all HardFacts"
        );
    }

    #[test]
    fn test_eo_recency_weighted() {
        let input = vec![
            item_of_type(MemoryType::EmpiricalObservation, "old", Some(10), None),
            item_of_type(MemoryType::EmpiricalObservation, "new", Some(10000), None),
        ];

        let output = PriorityStackResolver::resolve(input);

        let names: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(names, vec!["new", "old"]);
    }

    #[test]
    fn test_hf_recency_neutral() {
        let input = vec![
            item_of_type(MemoryType::HardFact, "hf_old", Some(10), None),
            item_of_type(MemoryType::HardFact, "hf_new", Some(10000), None),
        ];

        let output = PriorityStackResolver::resolve(input);

        let names: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(names, vec!["hf_old", "hf_new"]);
    }

    #[test]
    fn test_ai_recency_neutral() {
        let input = vec![
            item_of_type(MemoryType::ArchitecturalInvariant, "ai_old", Some(10), None),
            item_of_type(
                MemoryType::ArchitecturalInvariant,
                "ai_new",
                Some(10000),
                None,
            ),
        ];

        let output = PriorityStackResolver::resolve(input);

        let names: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(names, vec!["ai_old", "ai_new"]);
    }

    #[test]
    fn test_invariant_not_demoted_by_newer_eo() {
        let input = vec![
            item_of_type(
                MemoryType::EmpiricalObservation,
                "eo_new",
                Some(999999),
                None,
            ),
            item_of_type(
                MemoryType::ArchitecturalInvariant,
                "ai_old",
                Some(100),
                None,
            ),
        ];

        let output = PriorityStackResolver::resolve(input);

        let names: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(names, vec!["ai_old", "eo_new"]);
    }

    #[test]
    fn test_emm_recency_neutral_default() {
        let input = vec![
            item_with_version(MemoryType::ExplicitMentalModel, "emm_old", Some(10), None),
            item_with_version(
                MemoryType::ExplicitMentalModel,
                "emm_new",
                Some(10000),
                None,
            ),
        ];

        let output = PriorityStackResolver::resolve(input);

        let names: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(names, vec!["emm_old", "emm_new"]);
    }

    #[test]
    fn test_emm_versioned_recency_weighted() {
        let input = vec![
            item_with_version(
                MemoryType::ExplicitMentalModel,
                "emm_v1",
                Some(100),
                Some("v1"),
            ),
            item_with_version(
                MemoryType::ExplicitMentalModel,
                "emm_v2",
                Some(200),
                Some("v2"),
            ),
        ];

        let output = PriorityStackResolver::resolve(input);

        let names: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(names, vec!["emm_v2", "emm_v1"]);
    }

    #[test]
    fn test_type_pair_recency_correctness() {
        let input = vec![
            item_of_type(MemoryType::ConversationalContext, "cc_old", Some(50), None),
            item_of_type(MemoryType::ConversationalContext, "cc_new", Some(600), None),
            item_of_type(MemoryType::EmpiricalObservation, "eo_old", Some(100), None),
            item_of_type(MemoryType::EmpiricalObservation, "eo_new", Some(500), None),
            item_of_type(MemoryType::HardFact, "hf_old", Some(3), None),
            item_of_type(MemoryType::HardFact, "hf_new", Some(400), None),
            item_with_version(MemoryType::ExplicitMentalModel, "emm_old", Some(200), None),
            item_with_version(MemoryType::ExplicitMentalModel, "emm_new", Some(300), None),
            item_of_type(MemoryType::ArchitecturalInvariant, "ai_old", Some(1), None),
            item_of_type(
                MemoryType::ArchitecturalInvariant,
                "ai_new",
                Some(1000),
                None,
            ),
        ];

        let output = PriorityStackResolver::resolve(input);

        let names: Vec<&str> = output.iter().map(|i| i.id.as_str()).collect();

        let ai_pos: Vec<usize> = names
            .iter()
            .enumerate()
            .filter(|(_, n)| n.starts_with("ai_"))
            .map(|(i, _)| i)
            .collect();
        let emm_pos: Vec<usize> = names
            .iter()
            .enumerate()
            .filter(|(_, n)| n.starts_with("emm_"))
            .map(|(i, _)| i)
            .collect();
        let hf_pos: Vec<usize> = names
            .iter()
            .enumerate()
            .filter(|(_, n)| n.starts_with("hf_"))
            .map(|(i, _)| i)
            .collect();
        let eo_pos: Vec<usize> = names
            .iter()
            .enumerate()
            .filter(|(_, n)| n.starts_with("eo_"))
            .map(|(i, _)| i)
            .collect();
        let cc_pos: Vec<usize> = names
            .iter()
            .enumerate()
            .filter(|(_, n)| n.starts_with("cc_"))
            .map(|(i, _)| i)
            .collect();

        assert!(
            ai_pos.iter().all(|p| emm_pos.iter().all(|h| p < h)),
            "All AIs must precede all EMMs"
        );
        assert!(
            emm_pos.iter().all(|p| hf_pos.iter().all(|h| p < h)),
            "All EMMs must precede all HFs"
        );
        assert!(
            hf_pos.iter().all(|p| eo_pos.iter().all(|h| p < h)),
            "All HFs must precede all EOs"
        );
        assert!(
            eo_pos.iter().all(|p| cc_pos.iter().all(|h| p < h)),
            "All EOs must precede all CCs"
        );

        let eo_new_idx = names.iter().position(|n| *n == "eo_new").unwrap();
        let eo_old_idx = names.iter().position(|n| *n == "eo_old").unwrap();
        assert!(eo_new_idx < eo_old_idx, "New EO must precede old EO");

        let cc_new_idx = names.iter().position(|n| *n == "cc_new").unwrap();
        let cc_old_idx = names.iter().position(|n| *n == "cc_old").unwrap();
        assert!(cc_new_idx < cc_old_idx, "New CC must precede old CC");
    }

    fn make_eo(id: &str, content: &str, ts: Option<i64>) -> MemoryItem {
        let mut item = MemoryItem::new(id, id, content, MemoryType::EmpiricalObservation, "agent");
        if let Some(t) = ts {
            item = item.with_created_at(t);
        }
        item
    }

    #[test]
    fn test_frequency_dampening_curve_cap() {
        let input: Vec<MemoryItem> = (0..100)
            .map(|i| make_eo(&format!("eo_{i}"), "the sky is blue", Some(1000 - i as i64)))
            .collect();

        let output = consolidate_observations(input, 5);

        assert_eq!(output.len(), 1, "100 identical EOs should consolidate to 1");
        let item = &output[0];
        assert_eq!(item.memory_type, MemoryType::EmpiricalObservation);

        let weight =
            MemoryType::EmpiricalObservation.effective_weight(item.base_priority_multiplier);
        assert!(
            weight > 0.0 && weight <= 6.0,
            "dampened effective weight {weight} should be in (0, 6]"
        );
    }

    #[test]
    fn test_below_threshold_no_consolidation() {
        let input: Vec<MemoryItem> = (0..5)
            .map(|i| make_eo(&format!("eo_{i}"), "the sky is blue", None))
            .collect();

        let output = consolidate_observations(input, 5);

        assert_eq!(
            output.len(),
            5,
            "5 EOs with threshold 5 should not be consolidated"
        );
        for item in &output {
            assert!(
                item.base_priority_multiplier.is_none(),
                "multiplier should be unchanged when below threshold"
            );
        }
    }

    #[test]
    fn test_threshold_configurable() {
        let input: Vec<MemoryItem> = (0..200)
            .map(|i| make_eo(&format!("eo_{i}"), "the sky is blue", Some(1000 - i as i64)))
            .collect();

        let output = consolidate_observations(input, 10);

        assert_eq!(
            output.len(),
            1,
            "200 identical EOs with threshold 10 should consolidate to 1"
        );
        let weight =
            MemoryType::EmpiricalObservation.effective_weight(output[0].base_priority_multiplier);
        assert!(
            weight > 0.0 && weight <= 6.0,
            "dampened effective weight {weight} should be ≤ 6.0"
        );
    }

    #[test]
    fn test_different_content_not_consolidated() {
        let input: Vec<MemoryItem> = (0..10)
            .map(|i| make_eo(&format!("eo_{i}"), &format!("observation_{i}"), None))
            .collect();

        let output = consolidate_observations(input, 5);

        assert_eq!(
            output.len(),
            10,
            "all items have different content, none should be consolidated"
        );
        for item in &output {
            assert!(
                item.base_priority_multiplier.is_none(),
                "multiplier should be unchanged for distinct content"
            );
        }
    }

    #[test]
    fn test_cross_type_hardfact_outranks_consolidated_eo() {
        let mut items: Vec<MemoryItem> = (0..100)
            .map(|i| make_eo(&format!("eo_{i}"), "the sky is blue", Some(1000 - i as i64)))
            .collect();
        items.push(item_of_type(
            MemoryType::HardFact,
            "hf_truth",
            Some(500),
            None,
        ));

        let consolidated = consolidate_observations(items, 5);
        let resolved = PriorityStackResolver::resolve(consolidated);

        let hf_idx = resolved.iter().position(|i| i.id == "hf_truth").unwrap();
        let eo_idx = resolved
            .iter()
            .position(|i| i.memory_type == MemoryType::EmpiricalObservation)
            .unwrap();

        assert!(
            hf_idx < eo_idx,
            "HardFact must appear before consolidated EmpiricalObservation"
        );

        let eo_weight = MemoryType::EmpiricalObservation
            .effective_weight(resolved[eo_idx].base_priority_multiplier);
        assert!(
            eo_weight <= 6.0,
            "consolidated EO effective weight {eo_weight} should be ≤ 6.0"
        );
    }

    #[test]
    fn test_non_eo_items_passthrough() {
        let mut items: Vec<MemoryItem> = (0..100)
            .map(|i| {
                make_eo(
                    &format!("eo_{i}"),
                    "duplicate content",
                    Some(1000 - i as i64),
                )
            })
            .collect();
        items.push(item_of_type(
            MemoryType::ArchitecturalInvariant,
            "ai_1",
            Some(100),
            None,
        ));
        items.push(item_of_type(
            MemoryType::ExplicitMentalModel,
            "emm_1",
            Some(200),
            None,
        ));
        items.push(item_of_type(
            MemoryType::HardFact,
            "hf_1",
            Some(300),
            Some(1.5),
        ));
        items.push(item_of_type(
            MemoryType::ConversationalContext,
            "cc_1",
            Some(400),
            None,
        ));

        let output = consolidate_observations(items, 5);

        let ai = output.iter().find(|i| i.id == "ai_1").unwrap();
        assert_eq!(ai.memory_type, MemoryType::ArchitecturalInvariant);
        assert!(ai.base_priority_multiplier.is_none());

        let emm = output.iter().find(|i| i.id == "emm_1").unwrap();
        assert_eq!(emm.memory_type, MemoryType::ExplicitMentalModel);
        assert!(emm.base_priority_multiplier.is_none());

        let hf = output.iter().find(|i| i.id == "hf_1").unwrap();
        assert_eq!(hf.memory_type, MemoryType::HardFact);
        assert!((hf.base_priority_multiplier.unwrap() - 1.5).abs() < 1e-10);

        let cc = output.iter().find(|i| i.id == "cc_1").unwrap();
        assert_eq!(cc.memory_type, MemoryType::ConversationalContext);
        assert!(cc.base_priority_multiplier.is_none());

        assert_eq!(
            output
                .iter()
                .filter(|i| i.memory_type == MemoryType::EmpiricalObservation)
                .count(),
            1,
            "100 EOs should condense to 1"
        );
    }

    #[test]
    fn test_empty_input() {
        let output = consolidate_observations(Vec::new(), 5);
        assert!(output.is_empty());
    }
}
