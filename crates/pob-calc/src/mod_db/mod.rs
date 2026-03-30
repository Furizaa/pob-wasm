pub mod eval_mod;
pub mod types;

use std::collections::HashMap;
use std::sync::Arc;
use types::{KeywordFlags, Mod, ModFlags, ModTag, ModType, ModValue};

/// Per-mod display row returned by tabulate().
/// Mirrors the rows POB's Tabulate() method returns.
#[derive(Debug, Clone)]
pub struct TabulatedMod {
    pub value: ModValue,
    pub mod_type: ModType,
    pub source_category: String,
    pub source_name: String,
    pub flags: ModFlags,
}

/// The central modifier database.
/// Mirrors POB's ModDB class (ModDB.lua + ModStore.lua).
#[derive(Debug)]
pub struct ModDb {
    mods: HashMap<String, Vec<Mod>>,
    pub conditions: HashMap<String, bool>,
    pub multipliers: HashMap<String, f64>,
    parent: Option<Arc<ModDb>>,
}

impl ModDb {
    pub fn new() -> Self {
        Self {
            mods: HashMap::new(),
            conditions: HashMap::new(),
            multipliers: HashMap::new(),
            parent: None,
        }
    }

    pub fn with_parent(parent: Arc<ModDb>) -> Self {
        Self {
            mods: HashMap::new(),
            conditions: HashMap::new(),
            multipliers: HashMap::new(),
            parent: Some(parent),
        }
    }

    /// Add a modifier to the database.
    pub fn add(&mut self, m: Mod) {
        self.mods.entry(m.name.clone()).or_default().push(m);
    }

    /// Set a condition flag (mirrors modDB.conditions[var] = true/false in POB).
    pub fn set_condition(&mut self, var: &str, value: bool) {
        self.conditions.insert(var.to_string(), value);
    }

    /// Set a multiplier value (mirrors modDB.multipliers[var] = n in POB).
    pub fn set_multiplier(&mut self, var: &str, value: f64) {
        self.multipliers.insert(var.to_string(), value);
    }

    /// Temporary bridge: evaluate tags that are pure gates (Condition, MultiplierThreshold).
    /// This will be replaced by eval_mod() in Task 5 when we refactor query methods.
    fn eval_tags_as_gates(&self, tags: &[ModTag]) -> bool {
        for tag in tags {
            match tag {
                ModTag::Condition { var, neg } => {
                    let set = self.conditions.get(var).copied().unwrap_or(false);
                    if *neg && set {
                        return false;
                    }
                    if !*neg && !set {
                        return false;
                    }
                }
                ModTag::MultiplierThreshold {
                    var,
                    threshold,
                    upper,
                } => {
                    let val = self.multipliers.get(var).copied().unwrap_or(0.0);
                    if *upper {
                        // upper=true means "less than threshold" gates the mod
                        if val >= *threshold {
                            return false;
                        }
                    } else {
                        if val < *threshold {
                            return false;
                        }
                    }
                }
                // Other tag types are handled by eval_mod() — for now, skip them
                _ => {}
            }
        }
        true
    }

    fn mod_matches_query(
        &self,
        m: &Mod,
        mod_type: &ModType,
        flags: ModFlags,
        keyword_flags: KeywordFlags,
    ) -> bool {
        &m.mod_type == mod_type
            && flags.contains(m.flags)
            && keyword_flags.contains(m.keyword_flags)
            && self.eval_tags_as_gates(&m.tags)
    }

    /// Sum all BASE or INC mods for `name` that pass the flag/keyword/condition filters.
    /// Mirrors POB's modDB:Sum(modType, cfg, statName).
    pub fn sum(
        &self,
        mod_type: ModType,
        name: &str,
        flags: ModFlags,
        keyword_flags: KeywordFlags,
    ) -> f64 {
        let mut total = 0.0;
        if let Some(list) = self.mods.get(name) {
            for m in list {
                if self.mod_matches_query(m, &mod_type, flags, keyword_flags) {
                    total += m.value.as_f64();
                }
            }
        }
        if let Some(parent) = &self.parent {
            total += parent.sum(mod_type, name, flags, keyword_flags);
        }
        total
    }

    /// Multiply all MORE mods for `name`.
    /// Mirrors POB's modDB:More(cfg, statName).
    /// Each MORE mod value N means ×(1 + N/100).
    pub fn more(&self, name: &str, flags: ModFlags, keyword_flags: KeywordFlags) -> f64 {
        let mut result = 1.0_f64;
        if let Some(list) = self.mods.get(name) {
            for m in list {
                if self.mod_matches_query(m, &ModType::More, flags, keyword_flags) {
                    result *= 1.0 + m.value.as_f64() / 100.0;
                }
            }
        }
        // Round to 2 decimal places per POB's precision rules
        result = (result * 100.0).round() / 100.0;
        if let Some(parent) = &self.parent {
            result *= parent.more(name, flags, keyword_flags);
        }
        result
    }

    /// Return true if any FLAG mod with `name` is set and passes filters.
    /// Mirrors POB's modDB:Flag(cfg, statName).
    pub fn flag(&self, name: &str, flags: ModFlags, keyword_flags: KeywordFlags) -> bool {
        if let Some(list) = self.mods.get(name) {
            for m in list {
                if self.mod_matches_query(m, &ModType::Flag, flags, keyword_flags)
                    && m.value.as_bool()
                {
                    return true;
                }
            }
        }
        if let Some(parent) = &self.parent {
            return parent.flag(name, flags, keyword_flags);
        }
        false
    }

    /// Return all mods matching `name` (and optionally `mod_type`) for source-attribution UI.
    /// Mirrors POB's modDB:Tabulate(modType, cfg, statName).
    pub fn tabulate(
        &self,
        name: &str,
        mod_type: Option<ModType>,
        flags: ModFlags,
        keyword_flags: KeywordFlags,
    ) -> Vec<TabulatedMod> {
        let mut rows = Vec::new();
        if let Some(list) = self.mods.get(name) {
            for m in list {
                let type_matches = mod_type.as_ref().map_or(true, |t| t == &m.mod_type);
                if type_matches
                    && flags.contains(m.flags)
                    && keyword_flags.contains(m.keyword_flags)
                {
                    rows.push(TabulatedMod {
                        value: m.value.clone(),
                        mod_type: m.mod_type.clone(),
                        source_category: m.source.category.clone(),
                        source_name: m.source.name.clone(),
                        flags: m.flags,
                    });
                }
            }
        }
        if let Some(parent) = &self.parent {
            rows.extend(parent.tabulate(name, mod_type, flags, keyword_flags));
        }
        rows
    }
}

impl Default for ModDb {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{KeywordFlags, Mod, ModFlags, ModSource, ModTag, ModType, ModValue};

    fn src() -> ModSource {
        ModSource::new("Test", "test")
    }

    #[test]
    fn sum_base_mods() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("Life", 100.0, src()));
        db.add(Mod::new_base("Life", 50.0, src()));
        assert_eq!(
            db.sum(ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE),
            150.0
        );
    }

    #[test]
    fn more_multiplies() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::More,
            value: ModValue::Number(20.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::More,
            value: ModValue::Number(10.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        let result = db.more("Life", ModFlags::NONE, KeywordFlags::NONE);
        assert!((result - 1.32).abs() < 0.001, "expected 1.32, got {result}");
    }

    #[test]
    fn flag_returns_true_when_set() {
        let mut db = ModDb::new();
        db.add(Mod::new_flag("ChaosInoculation", src()));
        assert!(db.flag("ChaosInoculation", ModFlags::NONE, KeywordFlags::NONE));
        assert!(!db.flag("FullLife", ModFlags::NONE, KeywordFlags::NONE));
    }

    #[test]
    fn flags_filter_mods() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(50.0),
            flags: ModFlags::SPELL,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        assert_eq!(
            db.sum(ModType::Inc, "Damage", ModFlags::ATTACK, KeywordFlags::NONE),
            0.0
        );
        assert_eq!(
            db.sum(ModType::Inc, "Damage", ModFlags::SPELL, KeywordFlags::NONE),
            50.0
        );
    }

    #[test]
    fn tabulate_returns_matching_mods() {
        let mut db = ModDb::new();
        db.add(Mod::new_base(
            "Life",
            100.0,
            ModSource::new("Passive", "Thick Skin"),
        ));
        db.add(Mod::new_base(
            "Life",
            40.0,
            ModSource::new("Item", "Kaom's Heart"),
        ));
        let rows = db.tabulate("Life", None, ModFlags::NONE, KeywordFlags::NONE);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn condition_gates_mod() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::Base,
            value: ModValue::Number(500.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::Condition {
                var: "FullLife".into(),
                neg: false,
            }],
            source: src(),
        });
        assert_eq!(
            db.sum(ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE),
            0.0
        );
        db.set_condition("FullLife", true);
        assert_eq!(
            db.sum(ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE),
            500.0
        );
    }
}
