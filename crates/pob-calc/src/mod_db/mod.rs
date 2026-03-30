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

    // ── New query methods ────────────────────────────────────────────────

    /// Return the value of the last OVERRIDE mod for `name` that passes filters.
    /// Mirrors PoB's modDB:Override(cfg, statName).
    pub fn override_value(
        &self,
        name: &str,
        cfg: Option<&SkillCfg>,
        output: &OutputTable,
    ) -> Option<f64> {
        let mut result = None;
        if let Some(list) = self.mods.get(name) {
            for m in list {
                if self.mod_matches_cfg(m, &ModType::Override, cfg) {
                    if m.tags.is_empty() {
                        result = Some(m.value.as_f64());
                    } else if let Some(val) = eval_mod::eval_mod(m, cfg, self, output) {
                        result = Some(val);
                    }
                }
            }
        }
        if result.is_none() {
            if let Some(parent) = &self.parent {
                return parent.override_value(name, cfg, output);
            }
        }
        result
    }

    /// Return all LIST mods for `name` that pass filters.
    /// Returns the full Mod references for downstream processing.
    /// Mirrors PoB's modDB:List(cfg, statName).
    pub fn list(&self, name: &str, cfg: Option<&SkillCfg>, output: &OutputTable) -> Vec<&Mod> {
        let mut result = Vec::new();
        if let Some(mods) = self.mods.get(name) {
            for m in mods {
                if self.mod_matches_cfg(m, &ModType::List, cfg) {
                    if m.tags.is_empty() {
                        result.push(m);
                    } else if eval_mod::eval_mod(m, cfg, self, output).is_some() {
                        result.push(m);
                    }
                }
            }
        }
        if let Some(parent) = &self.parent {
            result.extend(parent.list(name, cfg, output));
        }
        result
    }

    /// Return the maximum value among all mods for `name` (any ModType) that pass filters.
    /// Mirrors PoB's modDB:Max(cfg, statName).
    pub fn max_value(
        &self,
        name: &str,
        cfg: Option<&SkillCfg>,
        output: &OutputTable,
    ) -> Option<f64> {
        let mut result: Option<f64> = None;
        if let Some(list) = self.mods.get(name) {
            for m in list {
                let cfg_flags = cfg.map_or(ModFlags::NONE, |c| c.flags);
                let cfg_kw = cfg.map_or(KeywordFlags::NONE, |c| c.keyword_flags);
                if !cfg_flags.contains(m.flags) || !cfg_kw.match_keyword_flags(m.keyword_flags) {
                    continue;
                }
                let val = if m.tags.is_empty() {
                    m.value.as_f64()
                } else {
                    match eval_mod::eval_mod(m, cfg, self, output) {
                        Some(v) => v,
                        None => continue,
                    }
                };
                result = Some(result.map_or(val, |prev: f64| prev.max(val)));
            }
        }
        if let Some(parent) = &self.parent {
            if let Some(parent_max) = parent.max_value(name, cfg, output) {
                result = Some(result.map_or(parent_max, |prev| prev.max(parent_max)));
            }
        }
        result
    }

    /// Return the minimum value among all mods for `name` (any ModType) that pass filters.
    /// Mirrors PoB's modDB:Min(cfg, statName).
    pub fn min_value(
        &self,
        name: &str,
        cfg: Option<&SkillCfg>,
        output: &OutputTable,
    ) -> Option<f64> {
        let mut result: Option<f64> = None;
        if let Some(list) = self.mods.get(name) {
            for m in list {
                let cfg_flags = cfg.map_or(ModFlags::NONE, |c| c.flags);
                let cfg_kw = cfg.map_or(KeywordFlags::NONE, |c| c.keyword_flags);
                if !cfg_flags.contains(m.flags) || !cfg_kw.match_keyword_flags(m.keyword_flags) {
                    continue;
                }
                let val = if m.tags.is_empty() {
                    m.value.as_f64()
                } else {
                    match eval_mod::eval_mod(m, cfg, self, output) {
                        Some(v) => v,
                        None => continue,
                    }
                };
                result = Some(result.map_or(val, |prev: f64| prev.min(val)));
            }
        }
        if let Some(parent) = &self.parent {
            if let Some(parent_min) = parent.min_value(name, cfg, output) {
                result = Some(result.map_or(parent_min, |prev| prev.min(parent_min)));
            }
        }
        result
    }

    /// Return true if any mod of `mod_type` for `name` passes filters.
    /// Does not evaluate the value — just checks existence.
    pub fn has_mod(
        &self,
        mod_type: ModType,
        name: &str,
        cfg: Option<&SkillCfg>,
        output: &OutputTable,
    ) -> bool {
        if let Some(list) = self.mods.get(name) {
            for m in list {
                if self.mod_matches_cfg(m, &mod_type, cfg) {
                    if m.tags.is_empty() {
                        return true;
                    } else if eval_mod::eval_mod(m, cfg, self, output).is_some() {
                        return true;
                    }
                }
            }
        }
        if let Some(parent) = &self.parent {
            return parent.has_mod(mod_type, name, cfg, output);
        }
        false
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

    // ── Task 6: override_value, list, max_value, min_value, has_mod ────

    #[test]
    fn override_value_returns_last_override() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::Override,
            value: ModValue::Number(1.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::Override,
            value: ModValue::Number(500.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        // Returns the last (most recent) Override mod's value
        assert_eq!(
            db.override_value("Life", None, &empty_output()),
            Some(500.0)
        );
    }

    #[test]
    fn override_value_returns_none_when_absent() {
        let db = ModDb::new();
        assert_eq!(db.override_value("Life", None, &empty_output()), None);
    }

    #[test]
    fn override_value_respects_tags() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::Override,
            value: ModValue::Number(999.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::Condition {
                var: "NeverTrue".into(),
                neg: false,
            }],
            source: src(),
        });
        // Condition fails → override not applied
        assert_eq!(db.override_value("Life", None, &empty_output()), None);
    }

    #[test]
    fn list_returns_all_matching_list_mods() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "ExtraAura".into(),
            mod_type: ModType::List,
            value: ModValue::String("Hatred".into()),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        db.add(Mod {
            name: "ExtraAura".into(),
            mod_type: ModType::List,
            value: ModValue::String("Wrath".into()),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        let result = db.list("ExtraAura", None, &empty_output());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn max_value_returns_largest() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "CritChance".into(),
            mod_type: ModType::Max,
            value: ModValue::Number(50.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        db.add(Mod {
            name: "CritChance".into(),
            mod_type: ModType::Max,
            value: ModValue::Number(75.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        assert_eq!(
            db.max_value("CritChance", None, &empty_output()),
            Some(75.0)
        );
    }

    #[test]
    fn max_value_returns_none_when_empty() {
        let db = ModDb::new();
        assert_eq!(db.max_value("CritChance", None, &empty_output()), None);
    }

    #[test]
    fn min_value_returns_smallest() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Speed".into(),
            mod_type: ModType::Override,
            value: ModValue::Number(100.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        db.add(Mod {
            name: "Speed".into(),
            mod_type: ModType::Override,
            value: ModValue::Number(50.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        assert_eq!(db.min_value("Speed", None, &empty_output()), Some(50.0));
    }

    #[test]
    fn has_mod_returns_true_when_present() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(50.0),
            flags: ModFlags::ATTACK,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        let cfg = SkillCfg {
            flags: ModFlags::ATTACK,
            ..Default::default()
        };
        assert!(db.has_mod(ModType::Inc, "Damage", Some(&cfg), &empty_output()));
    }

    #[test]
    fn has_mod_returns_false_when_filtered() {
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(50.0),
            flags: ModFlags::ATTACK,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        let cfg = SkillCfg {
            flags: ModFlags::SPELL,
            ..Default::default()
        };
        assert!(!db.has_mod(ModType::Inc, "Damage", Some(&cfg), &empty_output()));
    }

    // ── Task 7: Integration tests ───────────────────────────────────────

    #[test]
    fn integration_realistic_skill_query() {
        // Simulate a realistic scenario: a Fire Attack skill querying damage mods.
        let mut db = ModDb::new();

        // 1. Generic +20% increased Damage
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(20.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: ModSource::new("Passive", "Generic Node"),
        });

        // 2. +30% increased Attack Damage
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(30.0),
            flags: ModFlags::ATTACK,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: ModSource::new("Passive", "Attack Node"),
        });

        // 3. +40% increased Spell Damage (should NOT match)
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(40.0),
            flags: ModFlags::SPELL,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: ModSource::new("Passive", "Spell Node"),
        });

        // 4. +25% increased Fire Damage
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(25.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::FIRE,
            tags: vec![],
            source: ModSource::new("Passive", "Fire Node"),
        });

        // 5. +15% increased Cold Damage (should NOT match)
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(15.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::COLD,
            tags: vec![],
            source: ModSource::new("Passive", "Cold Node"),
        });

        // 6. +10% increased Damage per Power Charge
        db.set_multiplier("PowerCharge", 4.0);
        db.add(Mod {
            name: "Damage".into(),
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
            source: ModSource::new("Passive", "Charge Node"),
        });

        // 7. +50% increased Damage while at Full Life
        db.set_condition("FullLife", true);
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(50.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::Condition {
                var: "FullLife".into(),
                neg: false,
            }],
            source: ModSource::new("Passive", "Full Life Node"),
        });

        // 8. 20% more Melee Attack Damage
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::More,
            value: ModValue::Number(20.0),
            flags: ModFlags(ModFlags::ATTACK.0 | ModFlags::MELEE.0),
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: ModSource::new("Passive", "Melee More Node"),
        });

        // 9. 30% more Spell Damage (should NOT match)
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::More,
            value: ModValue::Number(30.0),
            flags: ModFlags::SPELL,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: ModSource::new("Passive", "Spell More Node"),
        });

        // Query: Fire Melee Attack skill
        let cfg = SkillCfg {
            flags: ModFlags(ModFlags::ATTACK.0 | ModFlags::HIT.0 | ModFlags::MELEE.0),
            keyword_flags: KeywordFlags(KeywordFlags::FIRE.0 | KeywordFlags::ATTACK.0),
            ..Default::default()
        };

        // Expected Inc sum:
        // 1. Generic +20 ✓
        // 2. Attack +30 ✓ (cfg has ATTACK)
        // 3. Spell +40 ✗ (cfg doesn't have SPELL)
        // 4. Fire +25 ✓ (keyword OR: FIRE overlaps)
        // 5. Cold +15 ✗ (keyword OR: COLD doesn't overlap)
        // 6. Per charge: 10 * 4 = 40 ✓
        // 7. Full Life: 50 ✓ (condition true)
        // Total Inc = 20 + 30 + 25 + 40 + 50 = 165
        let inc = db.sum_cfg(ModType::Inc, "Damage", Some(&cfg), &empty_output());
        assert_eq!(inc, 165.0);

        // Expected More:
        // 8. Melee Attack +20% ✓ (cfg has ATTACK|MELEE)
        // 9. Spell +30% ✗ (cfg doesn't have SPELL)
        // Result = 1.0 * (1 + 20/100) = 1.20
        let more = db.more_cfg("Damage", Some(&cfg), &empty_output());
        assert!((more - 1.20).abs() < 0.001, "expected 1.20, got {more}");

        // Final damage multiplier: base * (1 + inc/100) * more
        // = 100 * (1 + 165/100) * 1.20 = 100 * 2.65 * 1.20 = 318.0
        let base = 100.0_f64;
        let final_damage = base * (1.0 + inc / 100.0) * more;
        assert!(
            (final_damage - 318.0).abs() < 0.1,
            "expected ~318.0, got {final_damage}"
        );
    }

    #[test]
    fn integration_parent_db_with_cfg() {
        // Child ModDb overrides parent. Both use SkillCfg filtering.
        let mut parent = ModDb::new();
        parent.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(10.0),
            flags: ModFlags::ATTACK,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: ModSource::new("Base", "parent"),
        });
        let parent = Arc::new(parent);

        let mut child = ModDb::with_parent(parent);
        child.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(20.0),
            flags: ModFlags::ATTACK,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: ModSource::new("Skill", "child"),
        });

        let cfg = SkillCfg {
            flags: ModFlags::ATTACK,
            ..Default::default()
        };
        // 20 (child) + 10 (parent) = 30
        assert_eq!(
            child.sum_cfg(ModType::Inc, "Damage", Some(&cfg), &empty_output()),
            30.0
        );
    }

    #[test]
    fn integration_per_stat_with_output() {
        use crate::calc::env::OutputValue;
        // Test PerStat tag that reads from the output table
        let mut db = ModDb::new();
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::Base,
            value: ModValue::Number(1.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::PerStat {
                stat: "Str".into(),
                div: 2.0,
                limit: None,
                base: 0.0,
            }],
            source: ModSource::new("Base", "Str bonus"),
        });

        let mut output: OutputTable = HashMap::new();
        output.insert("Str".into(), OutputValue::Number(200.0));

        // 1 * floor(200 / 2) = 1 * 100 = 100
        assert_eq!(db.sum_cfg(ModType::Base, "Life", None, &output), 100.0);
    }

    #[test]
    fn integration_multi_mod_types_with_conditions() {
        use std::collections::HashMap as StdHashMap;
        // Task 7: Full pipeline integration test
        // Create a ModDb with multiple mod types:
        // - Base Life +100 (no tags, always active)
        // - Inc Life +50% with Condition tag ("UsingShield")
        // - More Life 10%
        // - Base Damage +50 with ATTACK flags
        // - Base Damage +30 with SPELL flags
        let mut db = ModDb::new();

        // Base Life +100 (always active)
        db.add(Mod::new_base("Life", 100.0, src()));

        // Inc Life +50% with Condition: UsingShield
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(50.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::Condition {
                var: "UsingShield".into(),
                neg: false,
            }],
            source: src(),
        });

        // More Life 10%
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::More,
            value: ModValue::Number(10.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });

        // Base Damage +50 with ATTACK flags
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Base,
            value: ModValue::Number(50.0),
            flags: ModFlags::ATTACK,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });

        // Base Damage +30 with SPELL flags
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Base,
            value: ModValue::Number(30.0),
            flags: ModFlags::SPELL,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });

        // Query 1: sum_cfg for "Life" without cfg → gets base 100
        // (condition mod excluded because UsingShield not set)
        assert_eq!(
            db.sum_cfg(ModType::Base, "Life", None, &empty_output()),
            100.0
        );
        // Inc should be 0 since UsingShield is not set
        assert_eq!(db.sum_cfg(ModType::Inc, "Life", None, &empty_output()), 0.0);

        // Query 2: sum_cfg for "Life" with cfg that has "UsingShield" in skill_cond
        let cfg_with_shield = SkillCfg {
            skill_cond: StdHashMap::from([("UsingShield".to_string(), true)]),
            ..Default::default()
        };
        // Base still 100
        assert_eq!(
            db.sum_cfg(
                ModType::Base,
                "Life",
                Some(&cfg_with_shield),
                &empty_output()
            ),
            100.0
        );
        // Inc should now be 50 since UsingShield is in skill_cond
        assert_eq!(
            db.sum_cfg(
                ModType::Inc,
                "Life",
                Some(&cfg_with_shield),
                &empty_output()
            ),
            50.0
        );

        // Query 3: sum_cfg for "Damage" with ATTACK flags → gets 50
        let cfg_attack = SkillCfg {
            flags: ModFlags::ATTACK,
            ..Default::default()
        };
        assert_eq!(
            db.sum_cfg(ModType::Base, "Damage", Some(&cfg_attack), &empty_output()),
            50.0
        );

        // Query 4: sum_cfg for "Damage" with SPELL flags → gets 30
        let cfg_spell = SkillCfg {
            flags: ModFlags::SPELL,
            ..Default::default()
        };
        assert_eq!(
            db.sum_cfg(ModType::Base, "Damage", Some(&cfg_spell), &empty_output()),
            30.0
        );

        // Query 5: sum_cfg for "Damage" with no flags → gets 80 (both)
        // NONE flags means both mods pass (NONE contains ATTACK and SPELL via
        // the AND check: (NONE & ATTACK) == ATTACK is false)
        // Wait — ModFlags::NONE.contains(ModFlags::ATTACK) is (0 & 0x01) == 0x01 → false
        // So with cfg=None (flags=NONE), neither mod passes.
        // With cfg having ATTACK|SPELL, both pass.
        let cfg_both = SkillCfg {
            flags: ModFlags(ModFlags::ATTACK.0 | ModFlags::SPELL.0),
            ..Default::default()
        };
        assert_eq!(
            db.sum_cfg(ModType::Base, "Damage", Some(&cfg_both), &empty_output()),
            80.0
        );

        // More Life should be 1.10 (10% more)
        assert_eq!(db.more_cfg("Life", None, &empty_output()), 1.10);

        // Also test has_mod
        assert!(db.has_mod(ModType::Base, "Life", None, &empty_output()));
        assert!(!db.has_mod(ModType::Base, "Nonexistent", None, &empty_output()));
        assert!(db.has_mod(ModType::Base, "Damage", Some(&cfg_attack), &empty_output()));
        assert!(!db.has_mod(
            ModType::Base,
            "Damage",
            Some(&SkillCfg {
                flags: ModFlags::DOT,
                ..Default::default()
            }),
            &empty_output()
        ));
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
