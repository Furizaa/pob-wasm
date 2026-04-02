//! Shared calculation utility functions.
//! Mirrors helpers from CalcTools.lua in Path of Building.

use crate::calc::env::OutputTable;
use crate::mod_db::types::{ModType, SkillCfg};
use crate::mod_db::ModDb;
use std::collections::HashSet;

/// Calculate `base * (1 + inc/100) * more` for a given stat.
/// Mirrors PoB's calcLib.mod(modDB, cfg, stat).
pub fn calc_mod(mod_db: &ModDb, cfg: Option<&SkillCfg>, output: &OutputTable, stat: &str) -> f64 {
    let base = mod_db.sum_cfg(ModType::Base, stat, cfg, output);
    let inc = mod_db.sum_cfg(ModType::Inc, stat, cfg, output);
    let more = mod_db.more_cfg(stat, cfg, output);
    base * (1.0 + inc / 100.0) * more
}

/// Calculate the combined INC+MORE multiplier for multiple stat names (no base).
/// Mirrors PoB's calcLib.mod(modDB, cfg, stat1, stat2, ...) — returns (1 + sum_of_INC/100) * product_of_MORE.
/// Used for defence calculations where a base value is multiplied by INC and MORE from multiple stat names.
pub fn calc_def_mod(
    mod_db: &ModDb,
    cfg: Option<&SkillCfg>,
    output: &OutputTable,
    stats: &[&str],
) -> f64 {
    let mut total_inc = 0.0_f64;
    let mut total_more = 1.0_f64;
    for &stat in stats {
        total_inc += mod_db.sum_cfg(ModType::Inc, stat, cfg, output);
        total_more *= mod_db.more_cfg(stat, cfg, output);
    }
    (1.0 + total_inc / 100.0) * total_more
}

/// Sum only BASE mods for a given stat.
/// Mirrors PoB's calcLib.val(modDB, cfg, stat).
pub fn calc_val(mod_db: &ModDb, cfg: Option<&SkillCfg>, output: &OutputTable, stat: &str) -> f64 {
    mod_db.sum_cfg(ModType::Base, stat, cfg, output)
}

/// Check if a skill's types match a comma-separated type expression.
/// Each token is a type name; prefix `!` means negation. All tokens must match (AND logic).
///
/// Example: `"Attack,!Melee"` matches skills that have "Attack" AND do NOT have "Melee".
pub fn does_type_expression_match(skill_types: &HashSet<String>, expression: &str) -> bool {
    for token in expression.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if let Some(negated) = token.strip_prefix('!') {
            let negated = negated.trim();
            // Negation: skill must NOT have this type
            if skill_types.iter().any(|t| t.eq_ignore_ascii_case(negated)) {
                return false;
            }
        } else {
            // Positive: skill must have this type
            if !skill_types.iter().any(|t| t.eq_ignore_ascii_case(token)) {
                return false;
            }
        }
    }
    true
}

/// Check if a support gem can support an active skill based on type matching.
/// `support_types`: types required by the support (at least one must match active_types).
/// `exclude_types`: types that exclude the support (none must match active_types).
/// `active_types`: the active skill's type tags.
///
/// If `support_types` is empty, the support matches any active skill (as long as no exclusion).
pub fn can_support_active_skill(
    support_types: &[String],
    exclude_types: &[String],
    active_types: &HashSet<String>,
) -> bool {
    // Check exclusions first
    for exc in exclude_types {
        if active_types.iter().any(|t| t.eq_ignore_ascii_case(exc)) {
            return false;
        }
    }
    // If no required types, it matches anything
    if support_types.is_empty() {
        return true;
    }
    // At least one required type must match
    support_types
        .iter()
        .any(|req| active_types.iter().any(|t| t.eq_ignore_ascii_case(req)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mod_db::types::{KeywordFlags, Mod, ModFlags, ModSource, ModType, ModValue};
    use crate::mod_db::ModDb;
    use std::collections::HashMap;

    fn src() -> ModSource {
        ModSource::new("Test", "test")
    }

    fn empty_output() -> OutputTable {
        HashMap::new()
    }

    #[test]
    fn calc_mod_base_inc_more() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("Damage", 100.0, src()));
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(50.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        db.add(Mod {
            name: "Damage".into(),
            mod_type: ModType::More,
            value: ModValue::Number(20.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        let result = calc_mod(&db, None, &empty_output(), "Damage");
        // 100 * (1 + 50/100) * (1 + 20/100) = 100 * 1.5 * 1.2 = 180
        assert!((result - 180.0).abs() < 0.001, "got {result}");
    }

    #[test]
    fn calc_val_sums_base_only() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("Life", 50.0, src()));
        db.add(Mod::new_base("Life", 30.0, src()));
        db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(100.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        });
        let result = calc_val(&db, None, &empty_output(), "Life");
        assert!((result - 80.0).abs() < 0.001, "got {result}");
    }

    #[test]
    fn type_expression_positive_match() {
        let types: HashSet<String> = ["Attack", "Melee", "Area"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(does_type_expression_match(&types, "Attack"));
        assert!(does_type_expression_match(&types, "Attack,Melee"));
        assert!(does_type_expression_match(&types, "Attack, Area"));
    }

    #[test]
    fn type_expression_negation() {
        let types: HashSet<String> = ["Attack", "Melee", "Area"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(does_type_expression_match(&types, "Attack,!Projectile"));
        assert!(!does_type_expression_match(&types, "Attack,!Melee"));
    }

    #[test]
    fn type_expression_no_match() {
        let types: HashSet<String> = ["Spell", "Projectile"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(!does_type_expression_match(&types, "Attack"));
    }

    #[test]
    fn type_expression_empty_always_matches() {
        let types: HashSet<String> = ["Attack"].iter().map(|s| s.to_string()).collect();
        assert!(does_type_expression_match(&types, ""));
    }

    #[test]
    fn type_expression_case_insensitive() {
        let types: HashSet<String> = ["attack"].iter().map(|s| s.to_string()).collect();
        assert!(does_type_expression_match(&types, "Attack"));
    }

    #[test]
    fn can_support_no_requirements_matches_all() {
        let active: HashSet<String> = ["Attack", "Melee"].iter().map(|s| s.to_string()).collect();
        assert!(can_support_active_skill(&[], &[], &active));
    }

    #[test]
    fn can_support_requires_attack_matches_attack() {
        let active: HashSet<String> = ["Attack", "Melee"].iter().map(|s| s.to_string()).collect();
        let req = vec!["Attack".to_string()];
        assert!(can_support_active_skill(&req, &[], &active));
    }

    #[test]
    fn can_support_requires_spell_rejects_attack() {
        let active: HashSet<String> = ["Attack", "Melee"].iter().map(|s| s.to_string()).collect();
        let req = vec!["Spell".to_string()];
        assert!(!can_support_active_skill(&req, &[], &active));
    }

    #[test]
    fn can_support_excludes_melee() {
        let active: HashSet<String> = ["Attack", "Melee"].iter().map(|s| s.to_string()).collect();
        let req = vec!["Attack".to_string()];
        let exc = vec!["Melee".to_string()];
        assert!(!can_support_active_skill(&req, &exc, &active));
    }
}
