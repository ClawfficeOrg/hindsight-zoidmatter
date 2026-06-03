use serde::{Deserialize, Serialize};

/// Top-level bank template manifest, matching the Hindsight import/export JSON
/// format.  The four Zoidmatter metadata extensions live inside [`BankConfig`].
///
/// # Legacy compatibility
///
/// Fields `bank`, `directives`, and `mental_models` all use
/// `#[serde(default)]` so a minimal `{"version":"1"}` document deserializes
/// without error.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BankJson {
    /// Manifest schema version (currently "1").
    pub version: String,

    /// Bank-level configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bank: Option<BankConfig>,

    /// Directives to create or update (matched by name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directives: Option<Vec<Directive>>,

    /// Mental models to create or update (matched by id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mental_models: Option<Vec<MentalModel>>,
}

/// Bank-level configuration within a template manifest.
///
/// Contains a representative subset of Hindsight's existing configuration
/// fields plus four Zoidmatter metadata extensions (`memory_type`,
/// `base_priority_multiplier`, `invariant_scope`, `lod_level`).  Every field
/// is optional — legacy JSON that omits the new fields (or any existing
/// field) will deserialize with the omitted fields set to [`None`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BankConfig {
    // ── Existing Hindsight fields ──
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflect_mission: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_mission: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_extraction_mode: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_custom_instructions: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_chunk_size: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_observations: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observations_mission: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition_skepticism: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition_literalism: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition_empathy: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_labels: Option<Vec<serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entities_allow_free_form: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_default_strategy: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_strategies: Option<serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_chunk_batch_size: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_enabled_tools: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consolidation_llm_batch_size: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consolidation_source_facts_max_tokens: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consolidation_source_facts_max_tokens_per_observation: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_observations_per_scope: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflect_source_facts_max_tokens: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_gemini_safety_settings: Option<Vec<serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_budget_function: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_budget_fixed_low: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_budget_fixed_mid: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_budget_fixed_high: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_budget_adaptive_low: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_budget_adaptive_mid: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_budget_adaptive_high: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_budget_min: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_budget_max: Option<i64>,

    // ── Zoidmatter metadata extensions ──
    /// Zoidmatter memory type classification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<String>,

    /// Multiplier applied to the memory type's default priority weight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_priority_multiplier: Option<f64>,

    /// Scope for architectural invariants: `project`, `session`, or `global`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invariant_scope: Option<String>,

    /// Level of Detail for code understanding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lod_level: Option<i64>,
}

/// A directive definition within a bank template manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Directive {
    pub name: String,

    pub content: String,

    #[serde(default)]
    pub priority: i64,

    #[serde(default = "default_true")]
    pub is_active: bool,

    #[serde(default)]
    pub tags: Vec<String>,
}

/// A mental model definition within a bank template manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MentalModel {
    pub id: String,

    pub name: String,

    pub source_query: String,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default = "default_max_tokens")]
    pub max_tokens: i64,

    #[serde(default)]
    pub trigger: MentalModelTrigger,
}

/// Trigger settings for a mental model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MentalModelTrigger {
    #[serde(default = "default_mode")]
    pub mode: String,

    #[serde(default)]
    pub refresh_after_consolidation: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact_types: Option<Vec<String>>,

    #[serde(default)]
    pub exclude_mental_models: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_mental_model_ids: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags_match: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_groups: Option<Vec<serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_chunks: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_max_tokens: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recall_chunks_max_tokens: Option<i64>,
}

// ── Default helpers ──

fn default_true() -> bool {
    true
}

fn default_max_tokens() -> i64 {
    2048
}

fn default_mode() -> String {
    "full".into()
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal legacy JSON — none of the four Zoidmatter fields are present.
    #[test]
    fn test_legacy_bank_json_deserializes() {
        let json = r#"{
            "version": "1",
            "bank": {
                "retain_mission": "test mission",
                "enable_observations": true
            }
        }"#;

        let manifest: BankJson = serde_json::from_str(json).expect("legacy JSON should parse");
        assert_eq!(manifest.version, "1");

        let bank = manifest.bank.expect("bank config should exist");
        assert_eq!(bank.retain_mission.as_deref(), Some("test mission"));
        assert_eq!(bank.enable_observations, Some(true));

        // All four Zoidmatter fields must be None
        assert!(
            bank.memory_type.is_none(),
            "memory_type should be None in legacy JSON"
        );
        assert!(
            bank.base_priority_multiplier.is_none(),
            "base_priority_multiplier should be None in legacy JSON"
        );
        assert!(
            bank.invariant_scope.is_none(),
            "invariant_scope should be None in legacy JSON"
        );
        assert!(
            bank.lod_level.is_none(),
            "lod_level should be None in legacy JSON"
        );
    }

    /// Fully extended JSON with all four Zoidmatter fields populated.
    #[test]
    fn test_extended_bank_json_round_trip() {
        let manifest = BankJson {
            version: "1".into(),
            bank: Some(BankConfig {
                retain_mission: Some("extract preferences".into()),
                enable_observations: Some(true),
                memory_type: Some("ExplicitMentalModel".into()),
                base_priority_multiplier: Some(1.5),
                invariant_scope: Some("project".into()),
                lod_level: Some(2),
                ..Default::default()
            }),
            directives: None,
            mental_models: None,
        };

        let serialized = serde_json::to_string(&manifest).expect("serialize extended");
        let round_tripped: BankJson =
            serde_json::from_str(&serialized).expect("deserialize extended");

        let bank = round_tripped.bank.expect("bank config should exist");
        assert_eq!(bank.memory_type.as_deref(), Some("ExplicitMentalModel"));
        assert_eq!(bank.base_priority_multiplier, Some(1.5));
        assert_eq!(bank.invariant_scope.as_deref(), Some("project"));
        assert_eq!(bank.lod_level, Some(2));
    }

    /// JSON with only a subset of new fields — omitted fields must be None.
    #[test]
    fn test_partial_new_fields_deserialize() {
        let json = r#"{
            "version": "1",
            "bank": {
                "retain_mission": "mixed",
                "memory_type": "HardFact",
                "lod_level": 1
            }
        }"#;

        let manifest: BankJson = serde_json::from_str(json).expect("partial fields JSON");
        let bank = manifest.bank.expect("bank config should exist");

        assert_eq!(bank.memory_type.as_deref(), Some("HardFact"));
        assert_eq!(bank.lod_level, Some(1));
        assert!(
            bank.base_priority_multiplier.is_none(),
            "missing field should be None"
        );
        assert!(
            bank.invariant_scope.is_none(),
            "missing field should be None"
        );
    }

    /// Minimal manifest with `"bank": null` — bank field is absent.
    #[test]
    fn test_empty_bank_config() {
        let json = r#"{"version": "1", "bank": null}"#;

        let manifest: BankJson = serde_json::from_str(json).expect("null bank should parse");
        assert_eq!(manifest.version, "1");
        assert!(manifest.bank.is_none(), "bank should be None when null");
    }

    /// Manifest with bank field completely omitted.
    #[test]
    fn test_missing_bank_config() {
        let json = r#"{"version": "1"}"#;

        let manifest: BankJson = serde_json::from_str(json).expect("missing bank should parse");
        assert_eq!(manifest.version, "1");
        assert!(manifest.bank.is_none(), "bank should be None when missing");
    }

    /// BankJson round-trip: serialize then deserialize an empty manifest.
    #[test]
    fn test_empty_manifest_round_trip() {
        let manifest = BankJson {
            version: "1".into(),
            bank: None,
            directives: None,
            mental_models: None,
        };

        let json = serde_json::to_string(&manifest).expect("serialize empty");
        let back: BankJson = serde_json::from_str(&json).expect("deserialize empty");
        assert_eq!(back.version, "1");
        assert!(back.bank.is_none());
    }

    /// All four Zoidmatter metadata fields round-trip correctly through JSON.
    #[test]
    fn test_zoidmatter_fields_serialize_correctly() {
        let bank = BankConfig {
            memory_type: Some("ArchitecturalInvariant".into()),
            base_priority_multiplier: Some(0.8),
            invariant_scope: Some("global".into()),
            lod_level: Some(3),
            ..Default::default()
        };

        let json = serde_json::to_string(&bank).expect("serialize");
        assert!(
            json.contains("memory_type"),
            "json must contain memory_type"
        );
        assert!(
            json.contains("ArchitecturalInvariant"),
            "json must contain the value"
        );
        assert!(
            json.contains("base_priority_multiplier"),
            "json must contain base_priority_multiplier"
        );
        assert!(
            json.contains("invariant_scope"),
            "json must contain invariant_scope"
        );
        assert!(json.contains("lod_level"), "json must contain lod_level");

        let parsed: BankConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            parsed.memory_type.as_deref(),
            Some("ArchitecturalInvariant")
        );
        assert_eq!(parsed.base_priority_multiplier, Some(0.8));
        assert_eq!(parsed.invariant_scope.as_deref(), Some("global"));
        assert_eq!(parsed.lod_level, Some(3));
    }
}
