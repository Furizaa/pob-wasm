use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct GemData {
    pub id: String,
    pub display_name: String,
    pub is_support: bool,
    pub skill_types: Vec<u32>,
}

pub type GemsMap = HashMap<String, GemData>;

/// Mirrors POB's SkillType constants (Common.lua).
/// Used to determine how a skill interacts with the mod system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillTypeFlags(pub u64);

impl SkillTypeFlags {
    pub const NONE: Self = SkillTypeFlags(0);
    pub const ATTACK: Self = SkillTypeFlags(1 << 0);
    pub const SPELL: Self = SkillTypeFlags(1 << 1);
    pub const PROJECTILE: Self = SkillTypeFlags(1 << 2);
    pub const AREA: Self = SkillTypeFlags(1 << 3);
    pub const DURATION: Self = SkillTypeFlags(1 << 4);
    pub const MELEE: Self = SkillTypeFlags(1 << 5);
    pub const DAMAGE: Self = SkillTypeFlags(1 << 6);

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}
