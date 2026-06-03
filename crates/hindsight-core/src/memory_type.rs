use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InvariantScope {
    Global,
    Project(String),
    Session,
}

impl InvariantScope {
    pub fn matches(&self, current_project: Option<&str>) -> bool {
        match self {
            InvariantScope::Global => true,
            InvariantScope::Project(p) => current_project == Some(p.as_str()),
            InvariantScope::Session => true,
        }
    }

    pub fn is_session(&self) -> bool {
        matches!(self, InvariantScope::Session)
    }

    pub fn project_name(&self) -> Option<&str> {
        match self {
            InvariantScope::Project(p) => Some(p.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryType {
    ArchitecturalInvariant,
    ExplicitMentalModel,
    HardFact,
    EmpiricalObservation,
    ConversationalContext,
}

impl MemoryType {
    pub fn default_weight(&self) -> f64 {
        match self {
            MemoryType::ArchitecturalInvariant => 100.0,
            MemoryType::ExplicitMentalModel => 80.0,
            MemoryType::HardFact => 60.0,
            MemoryType::EmpiricalObservation => 40.0,
            MemoryType::ConversationalContext => 20.0,
        }
    }

    pub fn effective_weight(&self, multiplier: Option<f64>) -> f64 {
        self.default_weight() * multiplier.unwrap_or(1.0)
    }
}

impl Ord for MemoryType {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.default_weight()
            .partial_cmp(&other.default_weight())
            .expect("default_weight must produce finite, comparable values")
    }
}

impl PartialOrd for MemoryType {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_type_serde_round_trip() {
        let variants = vec![
            MemoryType::ArchitecturalInvariant,
            MemoryType::ExplicitMentalModel,
            MemoryType::HardFact,
            MemoryType::EmpiricalObservation,
            MemoryType::ConversationalContext,
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).expect("serialization failed");
            let parsed: MemoryType = serde_json::from_str(&json).expect("deserialization failed");
            assert_eq!(variant, parsed, "round-trip failed for {:?}", variant);
        }
    }

    #[test]
    fn test_memory_type_priority_ordering() {
        use std::cmp::Ordering;

        assert_eq!(
            MemoryType::ArchitecturalInvariant.cmp(&MemoryType::ExplicitMentalModel),
            Ordering::Greater
        );
        assert_eq!(
            MemoryType::ExplicitMentalModel.cmp(&MemoryType::HardFact),
            Ordering::Greater
        );
        assert_eq!(
            MemoryType::HardFact.cmp(&MemoryType::EmpiricalObservation),
            Ordering::Greater
        );
        assert_eq!(
            MemoryType::EmpiricalObservation.cmp(&MemoryType::ConversationalContext),
            Ordering::Greater
        );

        let all = [
            MemoryType::ArchitecturalInvariant,
            MemoryType::ExplicitMentalModel,
            MemoryType::HardFact,
            MemoryType::EmpiricalObservation,
            MemoryType::ConversationalContext,
        ];

        for i in 0..all.len() {
            for j in 0..all.len() {
                if i == j {
                    assert_eq!(all[i].cmp(&all[j]), Ordering::Equal);
                } else if i < j {
                    assert_eq!(all[i].cmp(&all[j]), Ordering::Greater);
                } else {
                    assert_eq!(all[i].cmp(&all[j]), Ordering::Less);
                }
            }
        }
    }

    #[test]
    fn test_default_weights_are_distinct() {
        let weights = [
            MemoryType::ArchitecturalInvariant.default_weight(),
            MemoryType::ExplicitMentalModel.default_weight(),
            MemoryType::HardFact.default_weight(),
            MemoryType::EmpiricalObservation.default_weight(),
            MemoryType::ConversationalContext.default_weight(),
        ];

        for pair in weights.windows(2) {
            assert!(
                pair[0] > pair[1],
                "weights must be strictly descending: {} vs {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn test_effective_weight_no_multiplier() {
        let variants = [
            MemoryType::ArchitecturalInvariant,
            MemoryType::ExplicitMentalModel,
            MemoryType::HardFact,
            MemoryType::EmpiricalObservation,
            MemoryType::ConversationalContext,
        ];

        for variant in &variants {
            assert_eq!(
                variant.effective_weight(None),
                variant.default_weight(),
                "effective_weight(None) must equal default_weight() for {:?}",
                variant
            );
        }
    }

    #[test]
    fn test_effective_weight_with_multiplier() {
        let multiplier = 1.5;
        let epsilon = 1e-10;

        let variants = [
            MemoryType::ArchitecturalInvariant,
            MemoryType::ExplicitMentalModel,
            MemoryType::HardFact,
            MemoryType::EmpiricalObservation,
            MemoryType::ConversationalContext,
        ];

        for variant in &variants {
            let expected = variant.default_weight() * multiplier;
            let actual = variant.effective_weight(Some(multiplier));
            assert!(
                (actual - expected).abs() < epsilon,
                "effective_weight({:?}) mismatch: expected {}, got {}",
                variant,
                expected,
                actual
            );
        }
    }

    #[test]
    fn test_effective_weight_zero_multiplier() {
        let variants = [
            MemoryType::ArchitecturalInvariant,
            MemoryType::ExplicitMentalModel,
            MemoryType::HardFact,
            MemoryType::EmpiricalObservation,
            MemoryType::ConversationalContext,
        ];

        for variant in &variants {
            assert_eq!(
                variant.effective_weight(Some(0.0)),
                0.0,
                "effective_weight(Some(0.0)) must be 0.0 for {:?}",
                variant
            );
        }
    }
}

#[cfg(test)]
mod invariant_scope_tests {
    use super::*;

    #[test]
    fn test_invariant_scope_serde_round_trip() {
        let variants = vec![
            InvariantScope::Global,
            InvariantScope::Project("my-project".to_string()),
            InvariantScope::Session,
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).expect("serialization failed");
            let parsed: InvariantScope =
                serde_json::from_str(&json).expect("deserialization failed");
            assert_eq!(variant, parsed, "round-trip failed for {:?}", variant);
        }
    }

    #[test]
    fn test_invariant_scope_project_payload_preserved() {
        let scope = InvariantScope::Project("zoidmatter".to_string());
        let json = serde_json::to_string(&scope).unwrap();
        let parsed: InvariantScope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, InvariantScope::Project("zoidmatter".to_string()));
    }

    #[test]
    fn test_invariant_scope_equality() {
        assert_eq!(InvariantScope::Global, InvariantScope::Global);
        assert_eq!(
            InvariantScope::Project("a".to_string()),
            InvariantScope::Project("a".to_string())
        );
        assert_ne!(
            InvariantScope::Project("a".to_string()),
            InvariantScope::Project("b".to_string())
        );
        assert_eq!(InvariantScope::Session, InvariantScope::Session);
        assert_ne!(InvariantScope::Global, InvariantScope::Session);
    }

    #[test]
    fn test_scope_matches_global() {
        let scope = InvariantScope::Global;
        assert!(scope.matches(None));
        assert!(scope.matches(Some("foo")));
        assert!(scope.matches(Some("bar")));
        assert!(scope.matches(Some("")));
    }

    #[test]
    fn test_scope_matches_project() {
        let scope = InvariantScope::Project("zoidmatter".to_string());
        assert!(scope.matches(Some("zoidmatter")));
        assert!(!scope.matches(None));
        assert!(!scope.matches(Some("other")));
        assert!(!scope.matches(Some("")));
    }

    #[test]
    fn test_scope_matches_session() {
        let scope = InvariantScope::Session;
        assert!(scope.matches(None));
        assert!(scope.matches(Some("foo")));
        assert!(scope.matches(Some("bar")));
    }

    #[test]
    fn test_is_session() {
        assert!(InvariantScope::Session.is_session());
        assert!(!InvariantScope::Global.is_session());
        assert!(!InvariantScope::Project("x".to_string()).is_session());
    }

    #[test]
    fn test_project_name() {
        assert_eq!(
            InvariantScope::Project("zoidmatter".to_string()).project_name(),
            Some("zoidmatter")
        );
        assert_eq!(InvariantScope::Global.project_name(), None);
        assert_eq!(InvariantScope::Session.project_name(), None);
    }
}
