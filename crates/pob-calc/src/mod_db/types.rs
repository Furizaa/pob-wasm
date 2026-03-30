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
    pub const ATTACK: Self = ModFlags(0x01);
    pub const SPELL: Self = ModFlags(0x02);
    pub const HIT: Self = ModFlags(0x04);
    pub const DOT: Self = ModFlags(0x08);
    pub const CAST: Self = ModFlags(0x10);
    pub const MELEE: Self = ModFlags(0x100);
    pub const AREA: Self = ModFlags(0x200);
    pub const PROJECTILE: Self = ModFlags(0x400);
    pub const AILMENT: Self = ModFlags(0x800);
    pub const MELEE_HIT: Self = ModFlags(0x1000);
    pub const WEAPON: Self = ModFlags(0x2000);
    pub const AXE: Self = ModFlags(0x10000);
    pub const BOW: Self = ModFlags(0x20000);
    pub const CLAW: Self = ModFlags(0x40000);
    pub const DAGGER: Self = ModFlags(0x80000);
    pub const MACE: Self = ModFlags(0x100000);
    pub const STAFF: Self = ModFlags(0x200000);
    pub const SWORD: Self = ModFlags(0x400000);
    pub const WAND: Self = ModFlags(0x800000);
    pub const UNARMED: Self = ModFlags(0x1000000);
    pub const WEAPON_MELEE: Self = ModFlags(0x4000000);
    pub const WEAPON_RANGED: Self = ModFlags(0x8000000);
    pub const WEAPON_1H: Self = ModFlags(0x10000000);
    pub const WEAPON_2H: Self = ModFlags(0x20000000);

    /// AND matching: all bits in `other` must be present in `self`.
    /// This is PoB's `(cfg_flags & mod_flags) == mod_flags`.
    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for ModFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        ModFlags(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for ModFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        ModFlags(self.0 & rhs.0)
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

    #[test]
    fn mod_flags_all_constants_defined() {
        // Verify every flag constant has the correct bit value from PoB's Global.lua
        assert_eq!(ModFlags::ATTACK.0, 0x01);
        assert_eq!(ModFlags::SPELL.0, 0x02);
        assert_eq!(ModFlags::HIT.0, 0x04);
        assert_eq!(ModFlags::DOT.0, 0x08);
        assert_eq!(ModFlags::CAST.0, 0x10);
        assert_eq!(ModFlags::MELEE.0, 0x100);
        assert_eq!(ModFlags::AREA.0, 0x200);
        assert_eq!(ModFlags::PROJECTILE.0, 0x400);
        assert_eq!(ModFlags::AILMENT.0, 0x800);
        assert_eq!(ModFlags::MELEE_HIT.0, 0x1000);
        assert_eq!(ModFlags::WEAPON.0, 0x2000);
        assert_eq!(ModFlags::AXE.0, 0x10000);
        assert_eq!(ModFlags::BOW.0, 0x20000);
        assert_eq!(ModFlags::CLAW.0, 0x40000);
        assert_eq!(ModFlags::DAGGER.0, 0x80000);
        assert_eq!(ModFlags::MACE.0, 0x100000);
        assert_eq!(ModFlags::STAFF.0, 0x200000);
        assert_eq!(ModFlags::SWORD.0, 0x400000);
        assert_eq!(ModFlags::WAND.0, 0x800000);
        assert_eq!(ModFlags::UNARMED.0, 0x1000000);
        assert_eq!(ModFlags::WEAPON_MELEE.0, 0x4000000);
        assert_eq!(ModFlags::WEAPON_RANGED.0, 0x8000000);
        assert_eq!(ModFlags::WEAPON_1H.0, 0x10000000);
        assert_eq!(ModFlags::WEAPON_2H.0, 0x20000000);
    }

    #[test]
    fn mod_flags_and_matching_multi_bit() {
        // AND matching: (cfg_flags & mod_flags) == mod_flags
        // A mod with ATTACK|HIT should match a cfg with ATTACK|HIT|MELEE
        let cfg = ModFlags(ModFlags::ATTACK.0 | ModFlags::HIT.0 | ModFlags::MELEE.0);
        let mod_flags = ModFlags(ModFlags::ATTACK.0 | ModFlags::HIT.0);
        assert!(cfg.contains(mod_flags));

        // A mod with ATTACK|SPELL should NOT match cfg with only ATTACK
        let cfg2 = ModFlags(ModFlags::ATTACK.0);
        let mod_flags2 = ModFlags(ModFlags::ATTACK.0 | ModFlags::SPELL.0);
        assert!(!cfg2.contains(mod_flags2));
    }

    #[test]
    fn mod_flags_none_always_matches() {
        // A mod with NONE flags matches any cfg
        let cfg = ModFlags(ModFlags::ATTACK.0 | ModFlags::MELEE.0);
        assert!(cfg.contains(ModFlags::NONE));
        assert!(ModFlags::NONE.contains(ModFlags::NONE));
    }

    #[test]
    fn mod_flags_bitwise_or() {
        let combined = ModFlags::ATTACK | ModFlags::SPELL;
        assert_eq!(combined.0, 0x03);
        assert!(combined.contains(ModFlags::ATTACK));
        assert!(combined.contains(ModFlags::SPELL));
        assert!(!combined.contains(ModFlags::HIT));
    }
}
