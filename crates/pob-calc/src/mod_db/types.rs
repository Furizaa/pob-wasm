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
    pub const AURA: Self = KeywordFlags(0x01);
    pub const CURSE: Self = KeywordFlags(0x02);
    pub const WARCRY: Self = KeywordFlags(0x04);
    pub const MOVEMENT: Self = KeywordFlags(0x08);
    pub const PHYSICAL: Self = KeywordFlags(0x10);
    pub const FIRE: Self = KeywordFlags(0x20);
    pub const COLD: Self = KeywordFlags(0x40);
    pub const LIGHTNING: Self = KeywordFlags(0x80);
    pub const CHAOS: Self = KeywordFlags(0x100);
    pub const VAAL: Self = KeywordFlags(0x200);
    pub const BOW: Self = KeywordFlags(0x400);
    pub const ARROW: Self = KeywordFlags(0x800);
    pub const TRAP: Self = KeywordFlags(0x1000);
    pub const MINE: Self = KeywordFlags(0x2000);
    pub const TOTEM: Self = KeywordFlags(0x4000);
    pub const MINION: Self = KeywordFlags(0x8000);
    pub const ATTACK: Self = KeywordFlags(0x10000);
    pub const SPELL: Self = KeywordFlags(0x20000);
    pub const HIT: Self = KeywordFlags(0x40000);
    pub const AILMENT: Self = KeywordFlags(0x80000);
    pub const BRAND: Self = KeywordFlags(0x100000);
    pub const POISON: Self = KeywordFlags(0x200000);
    pub const BLEED: Self = KeywordFlags(0x400000);
    pub const IGNITE: Self = KeywordFlags(0x800000);
    pub const PHYSICAL_DOT: Self = KeywordFlags(0x1000000);
    pub const LIGHTNING_DOT: Self = KeywordFlags(0x2000000);
    pub const COLD_DOT: Self = KeywordFlags(0x4000000);
    pub const FIRE_DOT: Self = KeywordFlags(0x8000000);
    pub const CHAOS_DOT: Self = KeywordFlags(0x10000000);
    pub const MATCH_ALL: Self = KeywordFlags(0x40000000);

    /// Mask that strips the MatchAll control bit, leaving only keyword bits.
    const KEYWORD_MASK: u32 = !0x40000000;

    /// PoB's MatchKeywordFlags logic.
    /// - If mod has MatchAll: AND — all mod keyword bits must be in cfg.
    /// - Else: OR — any overlap passes, or mod has no keywords (always matches).
    pub fn match_keyword_flags(self, mod_flags: Self) -> bool {
        let mod_masked = mod_flags.0 & Self::KEYWORD_MASK;
        if mod_flags.0 & Self::MATCH_ALL.0 != 0 {
            // AND: all mod bits must be present in cfg
            (self.0 & mod_masked) == mod_masked
        } else {
            // OR: no keywords = always match, else any overlap
            mod_masked == 0 || (self.0 & mod_masked) != 0
        }
    }

    /// Legacy method — OR matching without MatchAll awareness.
    /// Kept for backward compatibility with existing ModDb code.
    pub fn contains(self, other: Self) -> bool {
        other.0 == 0 || (self.0 & other.0) != 0
    }
}

impl std::ops::BitOr for KeywordFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        KeywordFlags(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for KeywordFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        KeywordFlags(self.0 & rhs.0)
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

/// A tag that gates or scales a modifier's value.
/// Mirrors PoB's mod tag system from EvalMod in ModStore.lua.
/// Each tag type corresponds to a `type = "..."` tag in PoB's mod tables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModTag {
    /// Scales value by a multiplier variable from modDB.multipliers.
    /// value = value * floor((multiplier + base) / div), capped by limit.
    Multiplier {
        var: String,
        div: f64,
        limit: Option<f64>,
        base: f64,
    },

    /// Gates mod on multiplier >= threshold (or < threshold if upper=true).
    MultiplierThreshold {
        var: String,
        threshold: f64,
        upper: bool,
    },

    /// Scales value by an output stat.
    /// value = value * floor((stat_value + base) / div), capped by limit.
    PerStat {
        stat: String,
        div: f64,
        limit: Option<f64>,
        base: f64,
    },

    /// Gates mod on output stat >= threshold (or < threshold if upper=true).
    StatThreshold {
        stat: String,
        threshold: f64,
        upper: bool,
    },

    /// Gates mod on a condition flag being true (or false if neg=true).
    Condition {
        var: String,
        neg: bool,
    },

    /// Gates mod on another actor's condition flag.
    ActorCondition {
        actor: String,
        var: String,
        neg: bool,
    },

    /// Caps the cumulative value of this mod (applied after scaling).
    Limit {
        limit: f64,
    },

    /// Gates mod on the active skill having a specific skill type flag.
    SkillType {
        skill_type: u32,
    },

    /// Gates mod on the active skill's equipment slot.
    SlotName {
        slot_name: String,
        neg: bool,
    },

    /// OR-based flag check (instead of the default AND matching).
    /// Passes if (cfg_flags & mod_flags) != 0.
    ModFlagOr {
        mod_flags: ModFlags,
    },

    /// AND-based keyword check (instead of the default OR matching).
    /// Passes if (cfg_keywords & keyword_flags) == keyword_flags.
    KeywordFlagAnd {
        keyword_flags: KeywordFlags,
    },

    /// Marks this mod as a buff/debuff for the GlobalEffect system.
    GlobalEffect {
        effect_type: String,
        unscalable: bool,
    },

    // --- Phase 3: needed by generated mod parser ---
    SkillName {
        name: String,
    },
    SkillId {
        id: String,
    },
    SkillPart {
        part: u32,
    },
    SocketedIn {
        slot_name: String,
    },
    ItemCondition {
        var: String,
        neg: bool,
    },
}

/// Configuration for the active skill being evaluated.
/// Passed to ModDb query methods to filter mods by skill context.
/// Mirrors PoB's `cfg` table passed to Sum/More/Flag.
#[derive(Debug, Clone, Default)]
pub struct SkillCfg {
    /// ModFlag bits for the skill (e.g. ATTACK|HIT|MELEE).
    pub flags: ModFlags,
    /// KeywordFlag bits for the skill (e.g. FIRE|SPELL).
    pub keyword_flags: KeywordFlags,
    /// Equipment slot this skill is socketed in (e.g. "Weapon 1").
    pub slot_name: Option<String>,
    /// Skill name for SkillName tag matching.
    pub skill_name: Option<String>,
    /// Skill ID for SkillId tag matching.
    pub skill_id: Option<String>,
    /// Skill part index for SkillPart tag matching.
    pub skill_part: Option<u32>,
    /// Set of SkillType flags the active skill has.
    pub skill_types: std::collections::HashSet<u32>,
    /// Per-skill conditions (e.g. "usedByMirage" = true).
    pub skill_cond: std::collections::HashMap<String, bool>,
    /// Source attribution string.
    pub source: Option<String>,
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
    /// Tags that gate or scale this mod's value (replaces old `conditions` field).
    pub tags: Vec<ModTag>,
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
            tags: Vec::new(),
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
            tags: Vec::new(),
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

    #[test]
    fn keyword_flags_all_constants_defined() {
        assert_eq!(KeywordFlags::AURA.0, 0x01);
        assert_eq!(KeywordFlags::CURSE.0, 0x02);
        assert_eq!(KeywordFlags::WARCRY.0, 0x04);
        assert_eq!(KeywordFlags::MOVEMENT.0, 0x08);
        assert_eq!(KeywordFlags::PHYSICAL.0, 0x10);
        assert_eq!(KeywordFlags::FIRE.0, 0x20);
        assert_eq!(KeywordFlags::COLD.0, 0x40);
        assert_eq!(KeywordFlags::LIGHTNING.0, 0x80);
        assert_eq!(KeywordFlags::CHAOS.0, 0x100);
        assert_eq!(KeywordFlags::VAAL.0, 0x200);
        assert_eq!(KeywordFlags::BOW.0, 0x400);
        assert_eq!(KeywordFlags::ARROW.0, 0x800);
        assert_eq!(KeywordFlags::TRAP.0, 0x1000);
        assert_eq!(KeywordFlags::MINE.0, 0x2000);
        assert_eq!(KeywordFlags::TOTEM.0, 0x4000);
        assert_eq!(KeywordFlags::MINION.0, 0x8000);
        assert_eq!(KeywordFlags::ATTACK.0, 0x10000);
        assert_eq!(KeywordFlags::SPELL.0, 0x20000);
        assert_eq!(KeywordFlags::HIT.0, 0x40000);
        assert_eq!(KeywordFlags::AILMENT.0, 0x80000);
        assert_eq!(KeywordFlags::BRAND.0, 0x100000);
        assert_eq!(KeywordFlags::POISON.0, 0x200000);
        assert_eq!(KeywordFlags::BLEED.0, 0x400000);
        assert_eq!(KeywordFlags::IGNITE.0, 0x800000);
        assert_eq!(KeywordFlags::PHYSICAL_DOT.0, 0x1000000);
        assert_eq!(KeywordFlags::LIGHTNING_DOT.0, 0x2000000);
        assert_eq!(KeywordFlags::COLD_DOT.0, 0x4000000);
        assert_eq!(KeywordFlags::FIRE_DOT.0, 0x8000000);
        assert_eq!(KeywordFlags::CHAOS_DOT.0, 0x10000000);
        assert_eq!(KeywordFlags::MATCH_ALL.0, 0x40000000);
    }

    #[test]
    fn keyword_flags_or_matching() {
        // Default (no MatchAll): OR logic — any overlap passes
        let cfg = KeywordFlags(KeywordFlags::FIRE.0 | KeywordFlags::COLD.0);
        let mod_kw = KeywordFlags(KeywordFlags::FIRE.0);
        assert!(cfg.match_keyword_flags(mod_kw));

        // No overlap → fail
        let mod_kw2 = KeywordFlags(KeywordFlags::LIGHTNING.0);
        assert!(!cfg.match_keyword_flags(mod_kw2));
    }

    #[test]
    fn keyword_flags_none_mod_always_matches() {
        // A mod with NONE keywords always matches (no keyword restriction)
        let cfg = KeywordFlags(KeywordFlags::FIRE.0);
        assert!(cfg.match_keyword_flags(KeywordFlags::NONE));
        assert!(KeywordFlags::NONE.match_keyword_flags(KeywordFlags::NONE));
    }

    #[test]
    fn keyword_flags_match_all_and_logic() {
        // With MatchAll bit set: AND logic — all mod bits must be present in cfg
        let cfg = KeywordFlags(KeywordFlags::FIRE.0 | KeywordFlags::COLD.0);
        let mod_kw =
            KeywordFlags(KeywordFlags::FIRE.0 | KeywordFlags::COLD.0 | KeywordFlags::MATCH_ALL.0);
        assert!(cfg.match_keyword_flags(mod_kw));

        // Missing COLD from cfg → fail with MatchAll
        let cfg2 = KeywordFlags(KeywordFlags::FIRE.0);
        assert!(!cfg2.match_keyword_flags(mod_kw));
    }

    #[test]
    fn keyword_flags_bitwise_or() {
        let combined = KeywordFlags::FIRE | KeywordFlags::COLD;
        assert_eq!(combined.0, 0x60);
    }

    #[test]
    fn mod_tag_condition_creates_correctly() {
        let tag = ModTag::Condition {
            var: "FullLife".into(),
            neg: false,
        };
        match &tag {
            ModTag::Condition { var, neg } => {
                assert_eq!(var, "FullLife");
                assert!(!neg);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn mod_tag_multiplier_creates_correctly() {
        let tag = ModTag::Multiplier {
            var: "PowerCharge".into(),
            div: 1.0,
            limit: None,
            base: 0.0,
        };
        match &tag {
            ModTag::Multiplier {
                var,
                div,
                limit,
                base,
            } => {
                assert_eq!(var, "PowerCharge");
                assert_eq!(*div, 1.0);
                assert!(limit.is_none());
                assert_eq!(*base, 0.0);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn skill_cfg_default_has_no_flags() {
        let cfg = SkillCfg::default();
        assert_eq!(cfg.flags, ModFlags::NONE);
        assert_eq!(cfg.keyword_flags, KeywordFlags::NONE);
        assert!(cfg.slot_name.is_none());
        assert!(cfg.skill_types.is_empty());
        assert!(cfg.skill_cond.is_empty());
    }

    #[test]
    fn mod_struct_uses_tags_field() {
        let m = Mod {
            name: "Life".into(),
            mod_type: ModType::Base,
            value: ModValue::Number(100.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::Condition {
                var: "FullLife".into(),
                neg: false,
            }],
            source: ModSource::new("Test", "test"),
        };
        assert_eq!(m.tags.len(), 1);
    }
}
