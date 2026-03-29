//! Parse POB-format item text and passive node stat strings into Mods.
//!
//! POB's full ModParser.lua handles hundreds of patterns. This implementation
//! covers the most common patterns sufficient for passive tree nodes.
//! Add patterns as needed when oracle tests reveal discrepancies.

use crate::mod_db::types::{KeywordFlags, Mod, ModFlags, ModSource, ModType, ModValue};

/// Parse a single stat description string (as found on passive nodes or items)
/// into zero or more Mods. Returns an empty vec if the pattern is not recognised.
///
/// Examples of handled patterns:
///   "+40 to maximum Life"       → Mod { name: "Life", type: Base, value: 40 }
///   "10% increased maximum Life" → Mod { name: "Life", type: Inc, value: 10 }
///   "20% more Life"             → Mod { name: "Life", type: More, value: 20 }
///   "+15% to Fire Resistance"   → Mod { name: "FireResist", type: Base, value: 15 }
pub fn parse_stat_text(text: &str, source: ModSource) -> Vec<Mod> {
    let mut mods = Vec::new();

    // Normalise: strip colour codes (^8, ^x...), trim
    let clean: String = text
        .split('^')
        .enumerate()
        .filter_map(|(i, part)| {
            if i == 0 {
                Some(part.to_string())
            } else if part.starts_with('x') || part.starts_with(|c: char| c.is_ascii_digit()) {
                Some(part.chars().skip(1).collect())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("");
    let text = clean.trim();

    // Pattern: +N to maximum Life
    if let Some(n) = extract_prefix_num(text, "+", " to maximum Life") {
        mods.push(Mod::new_base("Life", n, source.clone()));
    }
    // +N to maximum Mana
    else if let Some(n) = extract_prefix_num(text, "+", " to maximum Mana") {
        mods.push(Mod::new_base("Mana", n, source.clone()));
    }
    // +N to maximum Energy Shield
    else if let Some(n) = extract_prefix_num(text, "+", " to maximum Energy Shield") {
        mods.push(Mod::new_base("EnergyShield", n, source.clone()));
    }
    // +N to Strength
    else if let Some(n) = extract_prefix_num(text, "+", " to Strength") {
        mods.push(Mod::new_base("Str", n, source.clone()));
    }
    // +N to Dexterity
    else if let Some(n) = extract_prefix_num(text, "+", " to Dexterity") {
        mods.push(Mod::new_base("Dex", n, source.clone()));
    }
    // +N to Intelligence
    else if let Some(n) = extract_prefix_num(text, "+", " to Intelligence") {
        mods.push(Mod::new_base("Int", n, source.clone()));
    }
    // N% increased maximum Life
    else if let Some(n) = extract_inc_pattern(text, "maximum Life") {
        mods.push(Mod {
            name: "Life".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // +N% to Fire Resistance
    else if let Some(n) = extract_prefix_num(text, "+", "% to Fire Resistance") {
        mods.push(Mod::new_base("FireResist", n, source));
    } else if let Some(n) = extract_prefix_num(text, "+", "% to Cold Resistance") {
        mods.push(Mod::new_base("ColdResist", n, source));
    } else if let Some(n) = extract_prefix_num(text, "+", "% to Lightning Resistance") {
        mods.push(Mod::new_base("LightningResist", n, source));
    } else if let Some(n) = extract_prefix_num(text, "+", "% to Chaos Resistance") {
        mods.push(Mod::new_base("ChaosResist", n, source));
    }
    // N% increased Armour
    else if let Some(n) = extract_inc_pattern(text, "Armour") {
        mods.push(Mod {
            name: "Armour".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // +N to all Attributes
    else if let Some(n) = extract_prefix_num(text, "+", " to all Attributes") {
        mods.push(Mod::new_base("Str", n, source.clone()));
        mods.push(Mod::new_base("Dex", n, source.clone()));
        mods.push(Mod::new_base("Int", n, source));
    }
    // N% increased Evasion Rating
    else if let Some(n) = extract_inc_pattern(text, "Evasion Rating") {
        mods.push(Mod {
            name: "Evasion".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // N% increased Energy Shield
    else if let Some(n) = extract_inc_pattern(text, "Energy Shield") {
        mods.push(Mod {
            name: "EnergyShield".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // N% increased Mana
    else if let Some(n) = extract_inc_pattern(text, "Mana") {
        mods.push(Mod {
            name: "Mana".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // N% increased Strength / Dexterity / Intelligence
    else if let Some(n) = extract_inc_pattern(text, "Strength") {
        mods.push(Mod {
            name: "Str".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    } else if let Some(n) = extract_inc_pattern(text, "Dexterity") {
        mods.push(Mod {
            name: "Dex".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    } else if let Some(n) = extract_inc_pattern(text, "Intelligence") {
        mods.push(Mod {
            name: "Int".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // N% increased Attack Speed
    else if let Some(n) = extract_inc_pattern(text, "Attack Speed") {
        mods.push(Mod {
            name: "Speed".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags(ModFlags::ATTACK.0),
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // N% increased Cast Speed
    else if let Some(n) = extract_inc_pattern(text, "Cast Speed") {
        mods.push(Mod {
            name: "Speed".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags(ModFlags::SPELL.0),
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // N% increased Physical Damage  (must come before generic "Damage")
    else if let Some(n) = extract_inc_pattern(text, "Physical Damage") {
        mods.push(Mod {
            name: "PhysicalDamage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // N% increased Area of Effect
    else if let Some(n) = extract_inc_pattern(text, "Area of Effect") {
        mods.push(Mod {
            name: "AreaOfEffect".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // N% increased Projectile Speed
    else if let Some(n) = extract_inc_pattern(text, "Projectile Speed") {
        mods.push(Mod {
            name: "ProjectileSpeed".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // +N to Accuracy Rating  (before generic "Accuracy Rating" inc)
    else if let Some(n) = extract_prefix_num(text, "+", " to Accuracy Rating") {
        mods.push(Mod::new_base("Accuracy", n, source));
    }
    // N% increased Accuracy Rating
    else if let Some(n) = extract_inc_pattern(text, "Accuracy Rating") {
        mods.push(Mod {
            name: "Accuracy".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // N% increased Damage  (generic — must come after all specific damage types)
    else if let Some(n) = extract_inc_pattern(text, "Damage") {
        mods.push(Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(n),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: vec![],
            source,
        });
    }
    // +N% to all Elemental Resistances
    else if let Some(n) = extract_prefix_num(text, "+", "% to all Elemental Resistances") {
        mods.push(Mod::new_base("FireResist", n, source.clone()));
        mods.push(Mod::new_base("ColdResist", n, source.clone()));
        mods.push(Mod::new_base("LightningResist", n, source));
    }

    mods
}

fn extract_prefix_num(text: &str, prefix: &str, suffix: &str) -> Option<f64> {
    let t = text.trim();
    if t.starts_with(prefix) && t.ends_with(suffix) {
        let inner = &t[prefix.len()..t.len() - suffix.len()];
        inner.parse::<f64>().ok()
    } else {
        None
    }
}

fn extract_inc_pattern(text: &str, stat: &str) -> Option<f64> {
    // Matches "N% increased STAT" or "N% reduced STAT"
    let t = text.trim();
    let (n_str, sign) = if let Some(rest) = t.strip_suffix(&format!("% increased {stat}")) {
        (rest.trim(), 1.0_f64)
    } else if let Some(rest) = t.strip_suffix(&format!("% reduced {stat}")) {
        (rest.trim(), -1.0_f64)
    } else {
        return None;
    };
    n_str.parse::<f64>().ok().map(|n| n * sign)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src() -> ModSource {
        ModSource::new("Test", "test")
    }

    #[test]
    fn parses_base_life() {
        let mods = parse_stat_text("+40 to maximum Life", src());
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "Life");
        assert!(matches!(
            mods[0].mod_type,
            crate::mod_db::types::ModType::Base
        ));
        assert_eq!(mods[0].value.as_f64(), 40.0);
    }

    #[test]
    fn parses_inc_life() {
        let mods = parse_stat_text("8% increased maximum Life", src());
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "Life");
        assert!(matches!(
            mods[0].mod_type,
            crate::mod_db::types::ModType::Inc
        ));
        assert_eq!(mods[0].value.as_f64(), 8.0);
    }

    #[test]
    fn parses_fire_resist() {
        let mods = parse_stat_text("+30% to Fire Resistance", src());
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "FireResist");
    }

    #[test]
    fn unknown_stat_returns_empty() {
        let mods = parse_stat_text("Socketed Gems are Supported by Level 1 Trap", src());
        assert!(mods.is_empty());
    }

    #[test]
    fn parses_all_attributes() {
        let mods = parse_stat_text("+10 to all Attributes", src());
        assert_eq!(mods.len(), 3);
        assert!(mods.iter().any(|m| m.name == "Str"));
        assert!(mods.iter().any(|m| m.name == "Dex"));
        assert!(mods.iter().any(|m| m.name == "Int"));
    }

    #[test]
    fn parses_all_elemental_resists() {
        let mods = parse_stat_text("+15% to all Elemental Resistances", src());
        assert_eq!(mods.len(), 3);
        assert!(mods.iter().all(|m| m.value.as_f64() == 15.0));
    }

    #[test]
    fn parses_inc_evasion() {
        let mods = parse_stat_text("12% increased Evasion Rating", src());
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "Evasion");
        assert!(matches!(
            mods[0].mod_type,
            crate::mod_db::types::ModType::Inc
        ));
    }

    #[test]
    fn parses_inc_physical_damage() {
        let mods = parse_stat_text("20% increased Physical Damage", src());
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "PhysicalDamage");
    }

    #[test]
    fn physical_damage_not_matched_as_generic_damage() {
        let mods = parse_stat_text("20% increased Physical Damage", src());
        assert!(
            mods.iter().all(|m| m.name != "Damage"),
            "Should match PhysicalDamage, not Damage"
        );
    }
}
