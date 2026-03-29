use serde::{Deserialize, Serialize};

/// Mirrors POB's mod type enum: BASE, INC, MORE, FLAG, LIST, OVERRIDE, MAX
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModType {
    Base,
    Inc,
    More,
    Flag,
    List,
    Override,
    Max,
}

/// Skill-type flags (bitfield). Mirrors POB's ModFlag.
/// Values match POB's ModFlag constants in Common.lua.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ModFlags(pub u32);

impl ModFlags {
    pub const NONE: Self = ModFlags(0);
    pub const ATTACK: Self = ModFlags(0x1);
    pub const SPELL: Self = ModFlags(0x2);
    pub const HIT: Self = ModFlags(0x4);
    pub const AILMENT: Self = ModFlags(0x8);
    pub const DOT: Self = ModFlags(0x10);
    pub const BOW: Self = ModFlags(0x80);
    pub const MELEE: Self = ModFlags(0x100);

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Keyword flags (bitfield). Mirrors POB's KeywordFlag constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct KeywordFlags(pub u32);

impl KeywordFlags {
    pub const NONE: Self = KeywordFlags(0);

    pub fn contains(self, other: Self) -> bool {
        other.0 == 0 || (self.0 & other.0) != 0
    }
}

/// The value a modifier carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModValue {
    Number(f64),
    Bool(bool),
    String(String),
}

impl ModValue {
    pub fn as_f64(&self) -> f64 {
        match self {
            Self::Number(n) => *n,
            Self::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Number(n) => *n != 0.0,
            _ => false,
        }
    }
}

/// A condition that gates whether a mod applies.
/// Mirrors POB's tag system: { type = "Condition", var = "FullLife" }, etc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// Mod applies only when this condition flag is true in modDB.conditions
    Flag { var: String, negated: bool },
    /// Mod applies only when a multiplier meets a threshold
    MultiplierThreshold {
        var: String,
        threshold: f64,
        negated: bool,
    },
    /// Mod always applies (no condition)
    None,
}

/// Where a mod came from. Used for source attribution in the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModSource {
    /// Category: "Passive", "Item", "Skill", "Base", "Config", "Buff", etc.
    pub category: String,
    /// Human-readable identifier within category: node name, item slot, skill name
    pub name: String,
}

impl ModSource {
    pub fn new(category: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            name: name.into(),
        }
    }
}

/// A single modifier — the atomic unit of the POB calculation system.
/// Mirrors the mod table created by modLib.createMod() in POB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mod {
    /// Stat name, e.g. "Life", "FireResist", "PhysicalDamage"
    pub name: String,
    pub mod_type: ModType,
    pub value: ModValue,
    pub flags: ModFlags,
    pub keyword_flags: KeywordFlags,
    /// All conditions must be satisfied for this mod to apply
    pub conditions: Vec<Condition>,
    pub source: ModSource,
}

impl Mod {
    pub fn new_base(name: impl Into<String>, value: f64, source: ModSource) -> Self {
        Self {
            name: name.into(),
            mod_type: ModType::Base,
            value: ModValue::Number(value),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: Vec::new(),
            source,
        }
    }

    pub fn new_flag(name: impl Into<String>, source: ModSource) -> Self {
        Self {
            name: name.into(),
            mod_type: ModType::Flag,
            value: ModValue::Bool(true),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            conditions: Vec::new(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_flags_contains() {
        let flags = ModFlags(ModFlags::ATTACK.0 | ModFlags::HIT.0);
        assert!(flags.contains(ModFlags::ATTACK));
        assert!(flags.contains(ModFlags::HIT));
        assert!(!flags.contains(ModFlags::SPELL));
    }

    #[test]
    fn keyword_flags_none_always_matches() {
        // KeywordFlags::NONE means "no keyword restriction" — always passes
        assert!(KeywordFlags(0xFF).contains(KeywordFlags::NONE));
        assert!(KeywordFlags::NONE.contains(KeywordFlags::NONE));
    }

    #[test]
    fn mod_value_conversions() {
        assert_eq!(ModValue::Number(3.0).as_f64(), 3.0);
        assert_eq!(ModValue::Bool(true).as_f64(), 1.0);
        assert!(ModValue::Bool(true).as_bool());
        assert!(!ModValue::Number(0.0).as_bool());
    }
}
