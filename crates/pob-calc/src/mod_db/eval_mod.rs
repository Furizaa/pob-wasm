//! Modifier evaluation — processes ModTag tags to gate/scale mod values.
//! Mirrors PoB's ModStore.lua EvalMod function.

use super::types::{KeywordFlags, Mod, ModFlags, ModTag, ModValue};
use super::ModDb;
use crate::calc::env::OutputTable;

/// Configuration for the active skill being evaluated.
/// Re-exported from types for convenience.
pub use super::types::SkillCfg;

/// Evaluate a modifier's tags against the query context.
/// Returns Some(scaled_value) if the mod passes all gates, None if excluded.
pub fn eval_mod(
    mod_entry: &Mod,
    cfg: Option<&SkillCfg>,
    mod_db: &ModDb,
    output: &OutputTable,
) -> Option<f64> {
    let mut value = match &mod_entry.value {
        ModValue::Number(n) => *n,
        ModValue::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        _ => return None,
    };

    for tag in &mod_entry.tags {
        match tag {
            ModTag::Condition { var, neg } => {
                let met = mod_db.conditions.get(var).copied().unwrap_or(false)
                    || cfg.map_or(false, |c| {
                        c.skill_cond.get(var.as_str()).copied().unwrap_or(false)
                    });
                if met == *neg {
                    return None;
                }
            }
            ModTag::ActorCondition { actor: _, var, neg } => {
                // For now, check local modDB conditions (full actor lookup deferred)
                let met = mod_db.conditions.get(var).copied().unwrap_or(false);
                if met == *neg {
                    return None;
                }
            }
            ModTag::Multiplier {
                var,
                div,
                limit,
                base,
            } => {
                let raw = mod_db.multipliers.get(var).copied().unwrap_or(0.0);
                let mut mult = (raw / div + 0.0001).floor();
                if let Some(lim) = limit {
                    mult = mult.min(*lim);
                }
                value = value * mult + base;
            }
            ModTag::MultiplierThreshold {
                var,
                threshold,
                upper,
            } => {
                let mult = mod_db.multipliers.get(var).copied().unwrap_or(0.0);
                if *upper {
                    if mult > *threshold {
                        return None;
                    }
                } else {
                    if mult < *threshold {
                        return None;
                    }
                }
            }
            ModTag::PerStat {
                stat,
                div,
                limit,
                base,
            } => {
                let stat_val = get_output_f64(output, stat);
                let mut mult = (stat_val / div + 0.0001).floor();
                if let Some(lim) = limit {
                    mult = mult.min(*lim);
                }
                value = value * mult + base;
            }
            ModTag::StatThreshold {
                stat,
                threshold,
                upper,
            } => {
                let stat_val = get_output_f64(output, stat);
                if *upper {
                    if stat_val > *threshold {
                        return None;
                    }
                } else {
                    if stat_val < *threshold {
                        return None;
                    }
                }
            }
            ModTag::Limit { limit } => {
                value = value.min(*limit);
            }
            ModTag::SkillType { skill_type } => {
                // SkillType has no `neg` field — it's a simple inclusion check
                let met = cfg.map_or(false, |c| c.skill_types.contains(skill_type));
                if !met {
                    return None;
                }
            }
            ModTag::SlotName { slot_name, neg } => {
                let met = cfg.map_or(false, |c| {
                    c.slot_name.as_ref().map_or(false, |s| s == slot_name)
                });
                if met == *neg {
                    return None;
                }
            }
            ModTag::ModFlagOr { mod_flags } => {
                let cfg_flags = cfg.map_or(ModFlags::NONE, |c| c.flags);
                if (cfg_flags & *mod_flags).0 == 0 {
                    return None;
                }
            }
            ModTag::KeywordFlagAnd { keyword_flags } => {
                let cfg_kw = cfg.map_or(KeywordFlags::NONE, |c| c.keyword_flags);
                // AND check: all bits in keyword_flags must be present in cfg_kw
                if (cfg_kw.0 & keyword_flags.0) != keyword_flags.0 {
                    return None;
                }
            }
            ModTag::GlobalEffect { .. } => {
                // Metadata only — no gating or scaling
            }
            // Phase 3 stubs — tags stored correctly, evaluation deferred to Phase 4+
            ModTag::SkillName { .. } => {
                // TODO(phase4): check against cfg.skill_name
            }
            ModTag::SkillId { .. } => {
                // TODO(phase4): check against cfg.skill_id
            }
            ModTag::SkillPart { .. } => {
                // TODO(phase4): check against cfg.skill_part
            }
            ModTag::SocketedIn { .. } => {
                // TODO(phase4): check against item socket context
            }
            ModTag::ItemCondition { .. } => {
                // TODO(phase4): check against item condition context
            }
        }
    }

    Some(value)
}

/// Helper: get a numeric value from OutputTable, defaulting to 0.0.
fn get_output_f64(output: &OutputTable, key: &str) -> f64 {
    use crate::calc::env::OutputValue;
    match output.get(key) {
        Some(OutputValue::Number(n)) => *n,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mod_db::types::{ModSource, ModType};
    use std::collections::{HashMap, HashSet};

    fn make_mod(value: f64, tags: Vec<ModTag>) -> Mod {
        Mod {
            name: "TestStat".to_string(),
            mod_type: ModType::Base,
            value: ModValue::Number(value),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags,
            source: ModSource {
                category: "test".into(),
                name: "test".into(),
            },
        }
    }

    fn empty_db() -> ModDb {
        ModDb::new()
    }

    fn empty_output() -> OutputTable {
        HashMap::new()
    }

    #[test]
    fn no_tags_returns_raw_value() {
        let m = make_mod(42.0, vec![]);
        let db = empty_db();
        let out = empty_output();
        assert_eq!(eval_mod(&m, None, &db, &out), Some(42.0));
    }

    #[test]
    fn condition_gates_true() {
        let m = make_mod(
            10.0,
            vec![ModTag::Condition {
                var: "Onslaught".into(),
                neg: false,
            }],
        );
        let mut db = empty_db();
        db.set_condition("Onslaught", true);
        assert_eq!(eval_mod(&m, None, &db, &empty_output()), Some(10.0));
    }

    #[test]
    fn condition_gates_false() {
        let m = make_mod(
            10.0,
            vec![ModTag::Condition {
                var: "Onslaught".into(),
                neg: false,
            }],
        );
        let db = empty_db();
        assert_eq!(eval_mod(&m, None, &db, &empty_output()), None);
    }

    #[test]
    fn condition_negated() {
        let m = make_mod(
            10.0,
            vec![ModTag::Condition {
                var: "Onslaught".into(),
                neg: true,
            }],
        );
        // condition not set = false, neg = true → met==neg is false==true → false → NOT excluded
        let db = empty_db();
        assert_eq!(eval_mod(&m, None, &db, &empty_output()), Some(10.0));
    }

    #[test]
    fn condition_from_skill_cond() {
        let m = make_mod(
            10.0,
            vec![ModTag::Condition {
                var: "UsingClaw".into(),
                neg: false,
            }],
        );
        let db = empty_db();
        let cfg = SkillCfg {
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            slot_name: None,
            skill_name: None,
            skill_id: None,
            skill_part: None,
            skill_types: HashSet::new(),
            skill_cond: HashMap::from([("UsingClaw".to_string(), true)]),
            source: None,
        };
        assert_eq!(eval_mod(&m, Some(&cfg), &db, &empty_output()), Some(10.0));
    }

    #[test]
    fn multiplier_scales_value() {
        let m = make_mod(
            5.0,
            vec![ModTag::Multiplier {
                var: "PowerCharges".into(),
                div: 1.0,
                limit: None,
                base: 0.0,
            }],
        );
        let mut db = empty_db();
        db.set_multiplier("PowerCharges", 3.0);
        assert_eq!(eval_mod(&m, None, &db, &empty_output()), Some(15.0));
    }

    #[test]
    fn multiplier_with_div() {
        let m = make_mod(
            1.0,
            vec![ModTag::Multiplier {
                var: "Str".into(),
                div: 10.0,
                limit: None,
                base: 0.0,
            }],
        );
        let mut db = empty_db();
        db.set_multiplier("Str", 155.0);
        // floor(155/10 + 0.0001) = floor(15.5001) = 15
        assert_eq!(eval_mod(&m, None, &db, &empty_output()), Some(15.0));
    }

    #[test]
    fn multiplier_with_limit() {
        let m = make_mod(
            5.0,
            vec![ModTag::Multiplier {
                var: "PowerCharges".into(),
                div: 1.0,
                limit: Some(5.0),
                base: 0.0,
            }],
        );
        let mut db = empty_db();
        db.set_multiplier("PowerCharges", 10.0);
        // mult capped at 5, so 5.0 * 5 = 25.0
        assert_eq!(eval_mod(&m, None, &db, &empty_output()), Some(25.0));
    }

    #[test]
    fn multiplier_threshold_below() {
        let m = make_mod(
            10.0,
            vec![ModTag::MultiplierThreshold {
                var: "FrenzyCharges".into(),
                threshold: 3.0,
                upper: false,
            }],
        );
        let mut db = empty_db();
        db.set_multiplier("FrenzyCharges", 2.0);
        assert_eq!(eval_mod(&m, None, &db, &empty_output()), None); // 2 < 3
    }

    #[test]
    fn multiplier_threshold_met() {
        let m = make_mod(
            10.0,
            vec![ModTag::MultiplierThreshold {
                var: "FrenzyCharges".into(),
                threshold: 3.0,
                upper: false,
            }],
        );
        let mut db = empty_db();
        db.set_multiplier("FrenzyCharges", 3.0);
        assert_eq!(eval_mod(&m, None, &db, &empty_output()), Some(10.0));
    }

    #[test]
    fn per_stat_scales_by_output() {
        let m = make_mod(
            2.0,
            vec![ModTag::PerStat {
                stat: "Life".into(),
                div: 100.0,
                limit: None,
                base: 0.0,
            }],
        );
        let db = empty_db();
        let mut out = empty_output();
        use crate::calc::env::OutputValue;
        out.insert("Life".into(), OutputValue::Number(500.0));
        // floor(500/100 + 0.0001) = 5, value = 2*5 + 0 = 10
        assert_eq!(eval_mod(&m, None, &db, &out), Some(10.0));
    }

    #[test]
    fn stat_threshold_gates() {
        let m = make_mod(
            10.0,
            vec![ModTag::StatThreshold {
                stat: "Life".into(),
                threshold: 1000.0,
                upper: false,
            }],
        );
        let db = empty_db();
        let mut out = empty_output();
        use crate::calc::env::OutputValue;
        out.insert("Life".into(), OutputValue::Number(500.0));
        assert_eq!(eval_mod(&m, None, &db, &out), None); // 500 < 1000
    }

    #[test]
    fn limit_caps_value() {
        let m = make_mod(100.0, vec![ModTag::Limit { limit: 50.0 }]);
        assert_eq!(eval_mod(&m, None, &empty_db(), &empty_output()), Some(50.0));
    }

    #[test]
    fn skill_type_gates() {
        let m = make_mod(10.0, vec![ModTag::SkillType { skill_type: 1 }]);
        let cfg = SkillCfg {
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            slot_name: None,
            skill_name: None,
            skill_id: None,
            skill_part: None,
            skill_types: HashSet::from([1, 2]),
            skill_cond: HashMap::new(),
            source: None,
        };
        assert_eq!(
            eval_mod(&m, Some(&cfg), &empty_db(), &empty_output()),
            Some(10.0)
        );
    }

    #[test]
    fn skill_type_excludes() {
        let m = make_mod(10.0, vec![ModTag::SkillType { skill_type: 99 }]);
        let cfg = SkillCfg {
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            slot_name: None,
            skill_name: None,
            skill_id: None,
            skill_part: None,
            skill_types: HashSet::from([1]),
            skill_cond: HashMap::new(),
            source: None,
        };
        assert_eq!(eval_mod(&m, Some(&cfg), &empty_db(), &empty_output()), None);
    }

    #[test]
    fn multiple_tags_compose() {
        // Condition gates + Multiplier scales
        let m = make_mod(
            5.0,
            vec![
                ModTag::Condition {
                    var: "Active".into(),
                    neg: false,
                },
                ModTag::Multiplier {
                    var: "Stacks".into(),
                    div: 1.0,
                    limit: None,
                    base: 0.0,
                },
            ],
        );
        let mut db = empty_db();
        db.set_condition("Active", true);
        db.set_multiplier("Stacks", 4.0);
        assert_eq!(eval_mod(&m, None, &db, &empty_output()), Some(20.0));
    }

    #[test]
    fn mod_flag_or_matches() {
        let m = make_mod(
            10.0,
            vec![ModTag::ModFlagOr {
                mod_flags: ModFlags::ATTACK | ModFlags::SPELL,
            }],
        );
        let cfg = SkillCfg {
            flags: ModFlags::SPELL,
            keyword_flags: KeywordFlags::NONE,
            slot_name: None,
            skill_name: None,
            skill_id: None,
            skill_part: None,
            skill_types: HashSet::new(),
            skill_cond: HashMap::new(),
            source: None,
        };
        assert_eq!(
            eval_mod(&m, Some(&cfg), &empty_db(), &empty_output()),
            Some(10.0)
        );
    }

    #[test]
    fn mod_flag_or_excludes() {
        let m = make_mod(
            10.0,
            vec![ModTag::ModFlagOr {
                mod_flags: ModFlags::ATTACK | ModFlags::SPELL,
            }],
        );
        let cfg = SkillCfg {
            flags: ModFlags::DOT,
            keyword_flags: KeywordFlags::NONE,
            slot_name: None,
            skill_name: None,
            skill_id: None,
            skill_part: None,
            skill_types: HashSet::new(),
            skill_cond: HashMap::new(),
            source: None,
        };
        assert_eq!(eval_mod(&m, Some(&cfg), &empty_db(), &empty_output()), None);
    }

    #[test]
    fn keyword_flag_and_matches() {
        let m = make_mod(
            10.0,
            vec![ModTag::KeywordFlagAnd {
                keyword_flags: KeywordFlags::TRAP | KeywordFlags::PHYSICAL,
            }],
        );
        let cfg = SkillCfg {
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::TRAP | KeywordFlags::PHYSICAL | KeywordFlags::HIT,
            slot_name: None,
            skill_name: None,
            skill_id: None,
            skill_part: None,
            skill_types: HashSet::new(),
            skill_cond: HashMap::new(),
            source: None,
        };
        assert_eq!(
            eval_mod(&m, Some(&cfg), &empty_db(), &empty_output()),
            Some(10.0)
        );
    }

    #[test]
    fn keyword_flag_and_excludes() {
        let m = make_mod(
            10.0,
            vec![ModTag::KeywordFlagAnd {
                keyword_flags: KeywordFlags::TRAP | KeywordFlags::PHYSICAL,
            }],
        );
        let cfg = SkillCfg {
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::TRAP, // missing PHYSICAL
            slot_name: None,
            skill_name: None,
            skill_id: None,
            skill_part: None,
            skill_types: HashSet::new(),
            skill_cond: HashMap::new(),
            source: None,
        };
        assert_eq!(eval_mod(&m, Some(&cfg), &empty_db(), &empty_output()), None);
    }

    #[test]
    fn eval_skill_name_tag_stubbed_passes() {
        let m = Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(10.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::SkillName {
                name: "Fireball".into(),
            }],
            source: ModSource::new("Test", "test"),
        };
        let db = ModDb::new();
        let output = OutputTable::new();
        assert_eq!(eval_mod(&m, None, &db, &output), Some(10.0));
    }

    #[test]
    fn eval_skill_id_tag_stubbed_passes() {
        let m = Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(5.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::SkillId {
                id: "Fireball".into(),
            }],
            source: ModSource::new("Test", "test"),
        };
        let db = ModDb::new();
        let output = OutputTable::new();
        assert_eq!(eval_mod(&m, None, &db, &output), Some(5.0));
    }

    #[test]
    fn eval_socketed_in_tag_stubbed_passes() {
        let m = Mod {
            name: "Level".into(),
            mod_type: ModType::Base,
            value: ModValue::Number(1.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::SocketedIn {
                slot_name: "Body Armour".into(),
            }],
            source: ModSource::new("Test", "test"),
        };
        let db = ModDb::new();
        let output = OutputTable::new();
        assert_eq!(eval_mod(&m, None, &db, &output), Some(1.0));
    }

    #[test]
    fn eval_item_condition_tag_stubbed_passes() {
        let m = Mod {
            name: "Armour".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(20.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::ItemCondition {
                var: "UsingShield".into(),
                neg: false,
            }],
            source: ModSource::new("Test", "test"),
        };
        let db = ModDb::new();
        let output = OutputTable::new();
        assert_eq!(eval_mod(&m, None, &db, &output), Some(20.0));
    }

    #[test]
    fn eval_skill_part_tag_stubbed_passes() {
        let m = Mod {
            name: "Damage".into(),
            mod_type: ModType::More,
            value: ModValue::Number(30.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::SkillPart { part: 1 }],
            source: ModSource::new("Test", "test"),
        };
        let db = ModDb::new();
        let output = OutputTable::new();
        assert_eq!(eval_mod(&m, None, &db, &output), Some(30.0));
    }
}
