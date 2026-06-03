use std::collections::HashMap;

use crate::retain_invariant::ArchitecturalInvariantRetainMission;
use crate::traits::RetainMission;

pub struct MissionRegistry {
    missions: HashMap<String, Box<dyn RetainMission>>,
}

impl MissionRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            missions: HashMap::new(),
        };
        registry.register(Box::new(ArchitecturalInvariantRetainMission));
        registry
    }

    pub fn register(&mut self, mission: Box<dyn RetainMission>) {
        self.missions.insert(mission.name().to_string(), mission);
    }

    pub fn get(&self, name: &str) -> Option<&dyn RetainMission> {
        self.missions.get(name).map(|b| b.as_ref())
    }

    pub fn has(&self, name: &str) -> bool {
        self.missions.contains_key(name)
    }

    pub fn names(&self) -> Vec<&String> {
        self.missions.keys().collect()
    }
}

impl Default for MissionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_pre_registers_invariant_mission() {
        let registry = MissionRegistry::new();
        assert!(registry.has("architectural_invariant"));
        let mission = registry
            .get("architectural_invariant")
            .expect("should exist");
        assert_eq!(mission.name(), "architectural_invariant");
    }

    #[test]
    fn test_registry_get_missing() {
        let registry = MissionRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_register_custom() {
        let mut registry = MissionRegistry::new();
        registry.register(Box::new(ArchitecturalInvariantRetainMission));
        assert!(registry.has("architectural_invariant"));
    }
}
