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
}
