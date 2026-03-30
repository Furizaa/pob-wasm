pub mod eval_mod;
pub mod types;

use std::collections::HashMap;
use std::sync::Arc;
use types::{KeywordFlags, Mod, ModFlags, ModType, ModValue, SkillCfg};

use crate::calc::env::OutputTable;

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

    /// Check if a mod matches the query's mod type, flags, and keyword flags.
    /// Uses cfg if provided, otherwise uses NONE defaults.
    fn mod_matches_cfg(&self, m: &Mod, mod_type: &ModType, cfg: Option<&SkillCfg>) -> bool {
        if &m.mod_type != mod_type {
            return false;
        }
        let cfg_flags = cfg.map_or(ModFlags::NONE, |c| c.flags);
        let cfg_kw = cfg.map_or(KeywordFlags::NONE, |c| c.keyword_flags);
        // ModFlags: AND matching — all mod bits must be present in cfg
        if !cfg_flags.contains(m.flags) {
            return false;
        }
        // KeywordFlags: use match_keyword_flags (OR or AND depending on MatchAll)
        if !cfg_kw.match_keyword_flags(m.keyword_flags) {
            return false;
        }
        true
    }

    // ── Primary query methods (accept SkillCfg + OutputTable) ────────────

    /// Sum all mods of `mod_type` for `name`, filtered by SkillCfg and evaluated by eval_mod.
    /// This is the primary query method. Mirrors PoB's modDB:Sum(modType, cfg, statName).
    pub fn sum_cfg(
        &self,
        mod_type: ModType,
        name: &str,
        cfg: Option<&SkillCfg>,
        output: &OutputTable,
    ) -> f64 {
        let mut total = 0.0;
        if let Some(list) = self.mods.get(name) {
            for m in list {
                if self.mod_matches_cfg(m, &mod_type, cfg) {
                    if m.tags.is_empty() {
                        total += m.value.as_f64();
                    } else if let Some(val) = eval_mod::eval_mod(m, cfg, self, output) {
                        total += val;
                    }
                }
            }
        }
        if let Some(parent) = &self.parent {
            total += parent.sum_cfg(mod_type, name, cfg, output);
        }
        total
    }

    /// Multiply all MORE mods for `name`, filtered by SkillCfg and evaluated by eval_mod.
    /// Each MORE mod value N means ×(1 + N/100).
    /// Mirrors PoB's modDB:More(cfg, statName).
    pub fn more_cfg(&self, name: &str, cfg: Option<&SkillCfg>, output: &OutputTable) -> f64 {
        let mut result = 1.0_f64;
        if let Some(list) = self.mods.get(name) {
            for m in list {
                if self.mod_matches_cfg(m, &ModType::More, cfg) {
                    let val = if m.tags.is_empty() {
                        m.value.as_f64()
                    } else {
                        match eval_mod::eval_mod(m, cfg, self, output) {
                            Some(v) => v,
                            None => continue,
                        }
                    };
                    result *= 1.0 + val / 100.0;
                }
            }
        }
        result = (result * 100.0).round() / 100.0;
        if let Some(parent) = &self.parent {
            result *= parent.more_cfg(name, cfg, output);
        }
        result
    }

    /// Return true if any FLAG mod with `name` passes filters and eval_mod.
    /// Mirrors PoB's modDB:Flag(cfg, statName).
    pub fn flag_cfg(&self, name: &str, cfg: Option<&SkillCfg>, output: &OutputTable) -> bool {
        if let Some(list) = self.mods.get(name) {
            for m in list {
                if self.mod_matches_cfg(m, &ModType::Flag, cfg) {
                    if m.tags.is_empty() {
                        if m.value.as_bool() {
                            return true;
                        }
                    } else if let Some(val) = eval_mod::eval_mod(m, cfg, self, output) {
                        if val != 0.0 {
                            return true;
                        }
                    }
                }
            }
        }
        if let Some(parent) = &self.parent {
            return parent.flag_cfg(name, cfg, output);
        }
        false
    }

    /// Return all mods matching `name` for source-attribution UI, filtered by SkillCfg.
    /// Mirrors PoB's modDB:Tabulate(modType, cfg, statName).
    pub fn tabulate_cfg(
        &self,
        name: &str,
        mod_type: Option<ModType>,
        cfg: Option<&SkillCfg>,
        output: &OutputTable,
    ) -> Vec<TabulatedMod> {
        let mut rows = Vec::new();
        if let Some(list) = self.mods.get(name) {
            for m in list {
                let type_matches = mod_type.as_ref().map_or(true, |t| t == &m.mod_type);
                if !type_matches {
                    continue;
                }
                let cfg_flags = cfg.map_or(ModFlags::NONE, |c| c.flags);
                let cfg_kw = cfg.map_or(KeywordFlags::NONE, |c| c.keyword_flags);
                if !cfg_flags.contains(m.flags) {
                    continue;
                }
                if !cfg_kw.match_keyword_flags(m.keyword_flags) {
                    continue;
                }
                // For tabulate, we still check tags but don't scale the value
                if !m.tags.is_empty() {
                    if eval_mod::eval_mod(m, cfg, self, output).is_none() {
                        continue;
                    }
                }
                rows.push(TabulatedMod {
                    value: m.value.clone(),
                    mod_type: m.mod_type.clone(),
                    source_category: m.source.category.clone(),
                    source_name: m.source.name.clone(),
                    flags: m.flags,
                });
            }
        }
        if let Some(parent) = &self.parent {
            rows.extend(parent.tabulate_cfg(name, mod_type, cfg, output));
        }
        rows
    }

    // ── Legacy methods (backward-compatible wrappers) ────────────────────

    /// Legacy sum: delegates to sum_cfg with a minimal SkillCfg built from raw flags.
    /// Kept for backward compatibility with existing calc modules.
    pub fn sum(
        &self,
        mod_type: ModType,
        name: &str,
        flags: ModFlags,
        keyword_flags: KeywordFlags,
    ) -> f64 {
        let cfg = SkillCfg {
            flags,
            keyword_flags,
            ..Default::default()
        };
        let empty: OutputTable = HashMap::new();
        self.sum_cfg(mod_type, name, Some(&cfg), &empty)
    }

    /// Legacy more: delegates to more_cfg.
    pub fn more(&self, name: &str, flags: ModFlags, keyword_flags: KeywordFlags) -> f64 {
        let cfg = SkillCfg {
            flags,
            keyword_flags,
            ..Default::default()
        };
        let empty: OutputTable = HashMap::new();
        self.more_cfg(name, Some(&cfg), &empty)
    }

    /// Legacy flag: delegates to flag_cfg.
    pub fn flag(&self, name: &str, flags: ModFlags, keyword_flags: KeywordFlags) -> bool {
        let cfg = SkillCfg {
            flags,
            keyword_flags,
            ..Default::default()
        };
        let empty: OutputTable = HashMap::new();
        self.flag_cfg(name, Some(&cfg), &empty)
    }

    /// Legacy tabulate: delegates to tabulate_cfg.
    pub fn tabulate(
        &self,
        name: &str,
        mod_type: Option<ModType>,
        flags: ModFlags,
        keyword_flags: KeywordFlags,
    ) -> Vec<TabulatedMod> {
        let cfg = SkillCfg {
            flags,
            keyword_flags,
            ..Default::default()
        };
        let empty: OutputTable = HashMap::new();
        self.tabulate_cfg(name, mod_type, Some(&cfg), &empty)
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
    use crate::calc::env::OutputTable;
    use std::collections::HashMap;
    use types::{KeywordFlags, Mod, ModFlags, ModSource, ModTag, ModType, ModValue, SkillCfg};

    fn src() -> ModSource {
        ModSource::new("Test", "test")
    }

    fn empty_output() -> OutputTable {
        HashMap::new()
    }

    // ── Legacy API tests ─────────────────────────────────────────────────

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

    // ── New _cfg API tests ───────────────────────────────────────────────

    #[test]
    fn sum_cfg_filters_by_mod_flags() {
        let mut db = ModDb::new();
        // Mod with ATTACK|HIT flags
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(50.0),
            flags: ModFlags(ModFlags::ATTACK.0 | ModFlags::HIT.0),
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        let cfg = SkillCfg {
            flags: ModFlags(ModFlags::ATTACK.0 | ModFlags::HIT.0 | ModFlags::MELEE.0),
            ..Default::default()
        };
        // cfg has ATTACK|HIT|MELEE which contains ATTACK|HIT → matches
        assert_eq!(
            db.sum_cfg(ModType::Inc, "Damage", Some(&cfg), &empty_output()),
            50.0
        );
        // cfg with only SPELL → doesn't contain ATTACK|HIT
        let cfg2 = SkillCfg {
            flags: ModFlags::SPELL,
            ..Default::default()
        };
        assert_eq!(
            db.sum_cfg(ModType::Inc, "Damage", Some(&cfg2), &empty_output()),
            0.0
        );
    }

    #[test]
    fn sum_cfg_filters_by_keyword_flags_or() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(30.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::FIRE,
            tags: vec![],
            source: src(),
        });
        let cfg = SkillCfg {
            keyword_flags: KeywordFlags(KeywordFlags::FIRE.0 | KeywordFlags::COLD.0),
            ..Default::default()
        };
        // OR matching: FIRE overlaps with FIRE|COLD → matches
        assert_eq!(
            db.sum_cfg(ModType::Inc, "Damage", Some(&cfg), &empty_output()),
            30.0
        );
        let cfg2 = SkillCfg {
            keyword_flags: KeywordFlags::COLD,
            ..Default::default()
        };
        // FIRE vs COLD → no overlap → excluded
        assert_eq!(
            db.sum_cfg(ModType::Inc, "Damage", Some(&cfg2), &empty_output()),
            0.0
        );
    }

    #[test]
    fn sum_cfg_keyword_flags_match_all() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(40.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags(
                KeywordFlags::FIRE.0 | KeywordFlags::SPELL.0 | KeywordFlags::MATCH_ALL.0,
            ),
            tags: vec![],
            source: src(),
        });
        let cfg = SkillCfg {
            keyword_flags: KeywordFlags(KeywordFlags::FIRE.0 | KeywordFlags::SPELL.0),
            ..Default::default()
        };
        // MatchAll AND: cfg has both FIRE and SPELL → matches
        assert_eq!(
            db.sum_cfg(ModType::Inc, "Damage", Some(&cfg), &empty_output()),
            40.0
        );
        let cfg2 = SkillCfg {
            keyword_flags: KeywordFlags::FIRE,
            ..Default::default()
        };
        // MatchAll AND: cfg missing SPELL → excluded
        assert_eq!(
            db.sum_cfg(ModType::Inc, "Damage", Some(&cfg2), &empty_output()),
            0.0
        );
    }

    #[test]
    fn sum_cfg_calls_eval_mod_for_tags() {
        let mut db = ModDb::new();
        db.set_condition("FullLife", true);
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(20.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::Condition {
                var: "FullLife".into(),
                neg: false,
            }],
            source: src(),
        });
        // FullLife is true → condition passes → includes 20
        assert_eq!(
            db.sum_cfg(ModType::Inc, "Life", None, &empty_output()),
            20.0
        );
    }

    #[test]
    fn sum_cfg_eval_mod_scales_value() {
        let mut db = ModDb::new();
        db.set_multiplier("PowerCharge", 3.0);
        db.add(Mod {
            name: "CritChance".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(10.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::Multiplier {
                var: "PowerCharge".into(),
                div: 1.0,
                limit: None,
                base: 0.0,
            }],
            source: src(),
        });
        // 10 * 3 charges = 30
        assert_eq!(
            db.sum_cfg(ModType::Inc, "CritChance", None, &empty_output()),
            30.0
        );
    }

    #[test]
    fn sum_cfg_none_is_backward_compatible() {
        // When cfg=None, behaves like the old sum() method
        let mut db = ModDb::new();
        db.add(Mod::new_base("Life", 100.0, src()));
        db.add(Mod::new_base("Life", 50.0, src()));
        assert_eq!(
            db.sum_cfg(ModType::Base, "Life", None, &empty_output()),
            150.0
        );
    }

    #[test]
    fn more_cfg_with_eval_mod() {
        let mut db = ModDb::new();
        db.set_condition("FullLife", true);
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::More,
            value: ModValue::Number(20.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::Condition {
                var: "FullLife".into(),
                neg: false,
            }],
            source: src(),
        });
        // FullLife true → mod applies → 1.0 * (1 + 20/100) = 1.20
        assert_eq!(db.more_cfg("Damage", None, &empty_output()), 1.20);
    }

    #[test]
    fn flag_cfg_with_eval_mod() {
        let mut db = ModDb::new();
        // Flag without condition → always applies
        db.add(Mod::new_flag("ChaosInoculation", src()));
        assert!(db.flag_cfg("ChaosInoculation", None, &empty_output()));

        // Flag with failing condition → excluded
        db.add(Mod {
            name: "SomeFlag".into(),
            mod_type: ModType::Flag,
            value: ModValue::Bool(true),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::Condition {
                var: "NeverTrue".into(),
                neg: false,
            }],
            source: src(),
        });
        assert!(!db.flag_cfg("SomeFlag", None, &empty_output()));
    }
}
