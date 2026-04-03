use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct Build {
    pub class_name: String,
    pub ascend_class_name: String,
    pub level: u8,
    pub bandit: String,
    /// Pantheon major god key (e.g. "Arakaali", "Solaris", "None").
    /// Parsed from `pantheonMajorGod` attribute on `<Build>` element.
    pub pantheon_major_god: String,
    /// Pantheon minor god key (e.g. "Shakari", "Gruthkul", "None").
    /// Parsed from `pantheonMinorGod` attribute on `<Build>` element.
    pub pantheon_minor_god: String,
    pub target_version: String,
    pub passive_spec: PassiveSpec,
    pub skill_sets: Vec<SkillSet>,
    pub active_skill_set: usize,  // 0-based index
    pub main_socket_group: usize, // 0-based index
    pub item_sets: Vec<ItemSet>,
    pub active_item_set: usize,
    pub config: BuildConfig,
    pub items: HashMap<u32, Item>,
}

#[derive(Debug, Clone, Default)]
pub struct PassiveSpec {
    pub tree_version: String,
    pub allocated_nodes: HashSet<u32>,
    pub class_id: u32,
    pub ascend_class_id: u32,
    /// Maps tree socket node ID → item ID for cluster jewels.
    /// Populated from `<Sockets><Socket nodeId="..." itemId="..."/></Sockets>`
    /// inside `<Spec>` in the PoB XML.
    pub jewels: HashMap<u32, u32>,
    /// Maps mastery node ID → selected effect ID.
    /// Populated from `masteryEffects="{nodeId,effectId},..."` attribute on `<Spec>`.
    /// Mirrors `PassiveSpec.masterySelections` in Lua.
    pub mastery_selections: HashMap<u32, u32>,
    /// Maps tree node ID → tattoo override data.
    /// Populated from `<Overrides><Override nodeId="..." dn="..."/></Overrides>`
    /// inside `<Spec>` in the PoB XML.
    /// Mirrors `PassiveSpec.hashOverrides` in Lua (PassiveSpec.lua line 83).
    pub hash_overrides: HashMap<u32, TattooOverrideNode>,
}

/// Tattoo replacement data for a single passive tree node.
///
/// Mirrors what `ReplaceNode` in PassiveSpec.lua copies from `tree.tattoo.nodes`
/// onto the original tree node. Specifically captures the fields needed for
/// multiplier counting in CalcSetup.lua lines 582–677 and stat replacement in
/// `build_mod_list_for_node`.
///
/// Fields map to TattooPassives.lua entry fields:
/// - `dn`                  → `["dn"]`                (display name / key into `tree.tattoo.nodes`)
/// - `is_tattoo`           → `["isTattoo"]`          (always `true` for tattoo nodes)
/// - `override_type`       → `["overrideType"]`      (e.g. `"KeystoneTattoo"`, `"StrTattoo"`)
/// - `is_keystone`         → `["ks"]`                (keystone tattoo flag)
/// - `is_notable`          → `["not"]`               (notable tattoo flag; `["not"]` avoids keyword)
/// - `is_mastery`          → `["m"]`                 (mastery tattoo flag)
/// - `stats`               → `["sd"]`                (stat description lines)
/// - `active_effect_image` → `["activeEffectImage"]` (artwork path for fallback lookup)
/// - `icon`                → `["icon"]`              (icon path for fallback lookup)
#[derive(Debug, Clone, Default)]
pub struct TattooOverrideNode {
    /// The tree node ID this tattoo replaces (the slot's hash ID).
    pub node_id: u32,
    /// Tattoo display name (e.g. "Acrobatics", "Ancestral Bond").
    pub dn: String,
    /// Always `true` for tattoo nodes (mirrors `isTattoo = true` in TattooPassives.lua).
    pub is_tattoo: bool,
    /// The tattoo's type string, used as the Multiplier key.
    /// E.g. `"KeystoneTattoo"`, `"StrTattoo"`, `"DexTattoo"`, `"IntTattoo"`,
    /// `"JourneyTattooBody"`, `"JourneyTattooSoul"`, `"JourneyTattooMind"`.
    /// Maps to `env.modDB.multipliers[override_type]` in CalcSetup.lua line 309.
    pub override_type: String,
    /// `true` if this is a keystone tattoo (node.ks in TattooPassives.lua).
    pub is_keystone: bool,
    /// `true` if this is a notable tattoo (node["not"] in TattooPassives.lua).
    pub is_notable: bool,
    /// `true` if this is a mastery tattoo (node.m in TattooPassives.lua).
    pub is_mastery: bool,
    /// Stat description lines (the `sd` field from TattooPassives.lua).
    /// Used to replace the original node's stats in `build_mod_list_for_node`.
    /// When non-empty, these override the tree node's `stats` array.
    pub stats: Vec<String>,
    /// Active effect image path (the `["activeEffectImage"]` field).
    /// Stored from XML `<Override activeEffectImage="...">` for the renamed-tattoo
    /// fallback lookup (PassiveSpec.lua lines 153–160).
    pub active_effect_image: String,
    /// Icon path (the `["icon"]` field).
    /// Stored from XML `<Override icon="...">` for the renamed-tattoo fallback.
    pub icon: String,
}

#[derive(Debug, Clone)]
pub struct SkillSet {
    pub id: u32,
    pub skills: Vec<Skill>,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub slot: String,
    pub enabled: bool,
    pub main_active_skill: usize, // 0-based index into gems
    pub gems: Vec<Gem>,

    // ── SETUP-02 fields ─────────────────────────────────────────────────────
    /// For item-granted skill groups, the source identifier (e.g. "Item:Heretic's Veil").
    /// None for normal socketed gem groups.
    pub source: Option<String>,

    /// True if this socket group should not receive support gem effects.
    /// Matches Lua's group.noSupports. Set for item-granted skill groups.
    pub no_supports: bool,

    /// True if this group's weapon set is the currently active one.
    /// Matches Lua's group.slotEnabled.
    pub slot_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct Gem {
    pub skill_id: String,
    pub level: u8,
    pub quality: u8,
    pub enabled: bool,
    pub is_support: bool,
    /// For Vaal gems, controls the Vaal active effect (index 1).
    /// For non-Vaal gems, controls the single granted effect.
    pub enable_global1: bool,
    /// For Vaal gems, controls the base (non-Vaal) active effect (index 2).
    /// When true, the base version of the Vaal gem is active (e.g., base Discipline
    /// from Vaal Discipline reserves mana).
    pub enable_global2: bool,
}

/// A fully resolved active skill, ready for offence calculations.
/// Mirrors POB's activeSkill table (env.player.mainSkill).
#[derive(Debug)]
pub struct ActiveSkill {
    /// The active gem's skill ID (e.g. "Fireball", "Cleave")
    pub skill_id: String,
    /// Gem level (1–20)
    pub level: u8,
    /// Gem quality (0–20+)
    pub quality: u8,
    /// Combined modifier database for this skill's context
    pub skill_mod_db: crate::mod_db::ModDb,
    /// True if this is an attack (uses attack speed, weapon damage)
    pub is_attack: bool,
    /// True if this is a spell
    pub is_spell: bool,
    /// True if this skill uses melee mechanics
    pub is_melee: bool,
    /// True if the skill can crit
    pub can_crit: bool,
    /// Base critical strike chance (0.0–1.0). From gem data.
    pub base_crit_chance: f64,
    /// Base damage (min, max) per damage type, from gem level scaling.
    /// Key: "Physical" | "Lightning" | "Cold" | "Fire" | "Chaos"
    pub base_damage: HashMap<String, (f64, f64)>,
    /// Attack speed (uses per second, for attacks)
    pub attack_speed_base: f64,
    /// Cast time in seconds (for spells)
    pub cast_time: f64,
    /// Damage effectiveness multiplier (default 1.0)
    pub damage_effectiveness: f64,
    /// Skill type tags (e.g. "Attack", "Melee", "Duration")
    pub skill_types: Vec<String>,
    /// Skill flags (e.g. "attack" => true, "spell" => false)
    pub skill_flags: HashMap<String, bool>,
    /// Skill-specific modifier configuration for SkillCfg-aware queries
    pub skill_cfg: Option<crate::mod_db::types::SkillCfg>,
    /// The equipment slot this skill is socketed in (e.g. "Weapon 1")
    pub slot_name: Option<String>,
    /// List of support gems linked to this active skill
    pub support_list: Vec<SupportEffect>,
    /// If set, this skill is triggered by some mechanism (e.g. "CastOnCrit", "CWC", "CWDT", "Trap", "Mine", "Totem")
    pub triggered_by: Option<String>,

    // ── SETUP-02 fields ─────────────────────────────────────────────────────
    /// Key-value bag of skill-specific runtime data.
    /// Mirrors activeSkill.skillData in Lua (e.g. CritChance, attackTime, manaReservationPercent).
    pub skill_data: HashMap<String, f64>,

    /// Index of the active skill part (for multi-part skills like Glacial Cascade).
    /// 1-based (Lua convention). None = skill has no parts or only one part.
    pub skill_part: Option<u32>,

    /// True if this skill group should not receive support gem effects.
    /// Set for item-granted skills (e.g., Heretic's Veil curse auras).
    pub no_supports: bool,

    /// Human-readable reason this skill is disabled (e.g. "This skill requires a Shield").
    /// None if skill is not disabled.
    pub disable_reason: Option<String>,

    /// Weapon ModFlags bit-OR for the main-hand attack.
    /// Set when skill uses weapon1 (weapon1Attack flag).
    pub weapon1_flags: u32,

    /// Weapon ModFlags bit-OR for the off-hand attack.
    /// Set when skill uses weapon2 (weapon2Attack flag).
    pub weapon2_flags: u32,

    /// Active mine count for mine skills.
    /// Mirrors activeSkill.activeMineCount in Lua (CalcActiveSkill.lua:596).
    /// Used by reservation calculation to scale reservation per active mine.
    pub active_mine_count: Option<f64>,

    /// Active stage count for channelled/staged skills (e.g. Blood Sacrament).
    /// Mirrors activeSkill.activeStageCount in Lua (CalcActiveSkill.lua:620).
    /// Used by reservation calculation to scale reservation per stage.
    pub active_stage_count: Option<f64>,

    /// Human-readable display name from gem data (e.g. "Discipline", "Grace").
    /// Mirrors activeSkill.activeEffect.grantedEffect.name in Lua.
    pub display_name: String,
}

impl Default for ActiveSkill {
    fn default() -> Self {
        Self {
            skill_id: String::new(),
            level: 1,
            quality: 0,
            skill_mod_db: crate::mod_db::ModDb::new(),
            is_attack: false,
            is_spell: false,
            is_melee: false,
            can_crit: true,
            base_crit_chance: 0.05,
            base_damage: HashMap::new(),
            attack_speed_base: 0.0,
            cast_time: 0.0,
            damage_effectiveness: 1.0,
            skill_types: Vec::new(),
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: Vec::new(),
            triggered_by: None,
            skill_data: HashMap::new(),
            skill_part: None,
            no_supports: false,
            disable_reason: None,
            weapon1_flags: 0,
            weapon2_flags: 0,
            active_mine_count: None,
            active_stage_count: None,
            display_name: String::new(),
        }
    }
}

// ── Item types ──────────────────────────────────────────────────────────────

/// A parsed equipped item from the build XML.
#[derive(Debug, Clone)]
pub struct Item {
    /// Item ID from XML
    pub id: u32,
    /// Normal / Magic / Rare / Unique
    pub rarity: ItemRarity,
    /// Item name (unique name for uniques, random name for rares)
    pub name: String,
    /// Base item name (e.g. "Rusted Sword")
    pub base_type: String,
    /// Item category (e.g. "One Handed Sword"), resolved later from base data
    pub item_type: String,
    /// Item quality 0–30
    pub quality: u32,
    /// Socket groups
    pub sockets: Vec<SocketGroup>,
    /// Implicit mod text lines
    pub implicits: Vec<String>,
    /// Explicit mod text lines
    pub explicits: Vec<String>,
    /// Crafted mod lines
    pub crafted_mods: Vec<String>,
    /// Enchant mod lines
    pub enchant_mods: Vec<String>,
    /// Whether the item is corrupted
    pub corrupted: bool,
    /// True if the item name/title contains "Foulborn" (PoE2 unique tag)
    pub foulborn: bool,
    /// Class restriction (e.g. "Scion"), set from "Requires Class XXX" line in item text
    pub class_restriction: Option<String>,
    /// Influence flags (shaper, elder, etc.)
    pub influence: ItemInfluence,
    /// Weapon-specific data, resolved from base data
    pub weapon_data: Option<ItemWeaponData>,
    /// Armour-specific data, resolved from base data
    pub armour_data: Option<ItemArmourData>,
    /// Flask-specific data, resolved from base data
    pub flask_data: Option<ItemFlaskData>,
    /// Level/attribute requirements
    pub requirements: ItemRequirements,
    /// Jewel radius label (e.g. "Small", "Medium", "Large", "Very Large", "Massive", "Variable").
    /// Parsed from "Radius: X" property line in the item text.
    /// None for non-jewel items.
    pub radius: Option<String>,
    /// Maximum number of this jewel that can be active simultaneously.
    /// Parsed from "Limited to: N" property line in the item text.
    /// None if not specified (no limit).
    /// Mirrors PoB's `item.limit` field set from base item data.
    pub limit: Option<u32>,
}

/// Item rarity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemRarity {
    #[default]
    Normal,
    Magic,
    Rare,
    Unique,
    /// Relic items: treated same as Unique for counting purposes.
    Relic,
}

impl ItemRarity {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "NORMAL" => Some(Self::Normal),
            "MAGIC" => Some(Self::Magic),
            "RARE" => Some(Self::Rare),
            "UNIQUE" => Some(Self::Unique),
            "RELIC" => Some(Self::Relic),
            _ => None,
        }
    }
}

/// A group of sockets in an item.
#[derive(Debug, Clone)]
pub struct SocketGroup {
    /// Socket colours: R (red), G (green), B (blue), W (white), A (abyss)
    pub colors: Vec<char>,
    /// Whether this group is fully linked
    pub linked: bool,
}

/// Influence flags on an item.
#[derive(Debug, Clone, Default)]
pub struct ItemInfluence {
    pub shaper: bool,
    pub elder: bool,
    pub crusader: bool,
    pub redeemer: bool,
    pub hunter: bool,
    pub warlord: bool,
    pub fractured: bool,
    pub synthesised: bool,
}

/// Weapon-specific base data.
#[derive(Debug, Clone)]
pub struct ItemWeaponData {
    pub phys_min: f64,
    pub phys_max: f64,
    pub attack_rate: f64,
    pub crit_chance: f64,
    pub range: u32,
}

/// Armour-specific base data.
#[derive(Debug, Clone)]
pub struct ItemArmourData {
    pub armour: f64,
    pub evasion: f64,
    pub energy_shield: f64,
    pub ward: f64,
    pub block: f64,
}

/// Flask-specific base data.
#[derive(Debug, Clone, Default)]
pub struct ItemFlaskData {
    pub life: f64,
    pub mana: f64,
    pub duration: f64,
    pub charges_used: u32,
    pub charges_max: u32,
}

/// Item level/attribute requirements.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemRequirements {
    pub level: u32,
    pub str_req: u32,
    pub dex_req: u32,
    pub int_req: u32,
}

/// Equipment slot identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemSlot {
    Weapon1,
    Weapon2,
    Helmet,
    BodyArmour,
    Gloves,
    Boots,
    Amulet,
    Ring1,
    Ring2,
    Belt,
    Flask1,
    Flask2,
    Flask3,
    Flask4,
    Flask5,
    Jewel1,
    Jewel2,
    Jewel3,
    Jewel4,
    Jewel5,
    Jewel6,
    Jewel7,
    Jewel8,
    Jewel9,
    Jewel10,
    Jewel11,
    Jewel12,
    Jewel13,
    Jewel14,
    Jewel15,
    Jewel16,
    Jewel17,
    Jewel18,
    Jewel19,
    Jewel20,
    Jewel21,
}

impl ItemSlot {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Weapon 1" => Some(Self::Weapon1),
            "Weapon 2" => Some(Self::Weapon2),
            "Helmet" => Some(Self::Helmet),
            "Body Armour" => Some(Self::BodyArmour),
            "Gloves" => Some(Self::Gloves),
            "Boots" => Some(Self::Boots),
            "Amulet" => Some(Self::Amulet),
            "Ring 1" => Some(Self::Ring1),
            "Ring 2" => Some(Self::Ring2),
            "Belt" => Some(Self::Belt),
            "Flask 1" => Some(Self::Flask1),
            "Flask 2" => Some(Self::Flask2),
            "Flask 3" => Some(Self::Flask3),
            "Flask 4" => Some(Self::Flask4),
            "Flask 5" => Some(Self::Flask5),
            _ => {
                // Try "Jewel N" pattern
                if let Some(rest) = s.strip_prefix("Jewel ") {
                    if let Ok(n) = rest.parse::<u32>() {
                        return Self::jewel_n(n);
                    }
                }
                None
            }
        }
    }

    fn jewel_n(n: u32) -> Option<Self> {
        match n {
            1 => Some(Self::Jewel1),
            2 => Some(Self::Jewel2),
            3 => Some(Self::Jewel3),
            4 => Some(Self::Jewel4),
            5 => Some(Self::Jewel5),
            6 => Some(Self::Jewel6),
            7 => Some(Self::Jewel7),
            8 => Some(Self::Jewel8),
            9 => Some(Self::Jewel9),
            10 => Some(Self::Jewel10),
            11 => Some(Self::Jewel11),
            12 => Some(Self::Jewel12),
            13 => Some(Self::Jewel13),
            14 => Some(Self::Jewel14),
            15 => Some(Self::Jewel15),
            16 => Some(Self::Jewel16),
            17 => Some(Self::Jewel17),
            18 => Some(Self::Jewel18),
            19 => Some(Self::Jewel19),
            20 => Some(Self::Jewel20),
            21 => Some(Self::Jewel21),
            _ => None,
        }
    }

    pub fn is_weapon(&self) -> bool {
        matches!(self, Self::Weapon1 | Self::Weapon2)
    }

    pub fn is_flask(&self) -> bool {
        matches!(
            self,
            Self::Flask1 | Self::Flask2 | Self::Flask3 | Self::Flask4 | Self::Flask5
        )
    }

    pub fn is_jewel(&self) -> bool {
        matches!(
            self,
            Self::Jewel1
                | Self::Jewel2
                | Self::Jewel3
                | Self::Jewel4
                | Self::Jewel5
                | Self::Jewel6
                | Self::Jewel7
                | Self::Jewel8
                | Self::Jewel9
                | Self::Jewel10
                | Self::Jewel11
                | Self::Jewel12
                | Self::Jewel13
                | Self::Jewel14
                | Self::Jewel15
                | Self::Jewel16
                | Self::Jewel17
                | Self::Jewel18
                | Self::Jewel19
                | Self::Jewel20
                | Self::Jewel21
        )
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Weapon1 => "Weapon 1",
            Self::Weapon2 => "Weapon 2",
            Self::Helmet => "Helmet",
            Self::BodyArmour => "Body Armour",
            Self::Gloves => "Gloves",
            Self::Boots => "Boots",
            Self::Amulet => "Amulet",
            Self::Ring1 => "Ring 1",
            Self::Ring2 => "Ring 2",
            Self::Belt => "Belt",
            Self::Flask1 => "Flask 1",
            Self::Flask2 => "Flask 2",
            Self::Flask3 => "Flask 3",
            Self::Flask4 => "Flask 4",
            Self::Flask5 => "Flask 5",
            Self::Jewel1 => "Jewel 1",
            Self::Jewel2 => "Jewel 2",
            Self::Jewel3 => "Jewel 3",
            Self::Jewel4 => "Jewel 4",
            Self::Jewel5 => "Jewel 5",
            Self::Jewel6 => "Jewel 6",
            Self::Jewel7 => "Jewel 7",
            Self::Jewel8 => "Jewel 8",
            Self::Jewel9 => "Jewel 9",
            Self::Jewel10 => "Jewel 10",
            Self::Jewel11 => "Jewel 11",
            Self::Jewel12 => "Jewel 12",
            Self::Jewel13 => "Jewel 13",
            Self::Jewel14 => "Jewel 14",
            Self::Jewel15 => "Jewel 15",
            Self::Jewel16 => "Jewel 16",
            Self::Jewel17 => "Jewel 17",
            Self::Jewel18 => "Jewel 18",
            Self::Jewel19 => "Jewel 19",
            Self::Jewel20 => "Jewel 20",
            Self::Jewel21 => "Jewel 21",
        }
    }
}

/// A support gem effect linked to an active skill.
#[derive(Debug, Clone)]
pub struct SupportEffect {
    pub skill_id: String,
    pub level: u8,
    pub quality: u8,
    pub gem_data: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ItemSet {
    pub id: u32,
    /// Map of slot name → item id
    pub slots: HashMap<String, u32>,
}

#[derive(Debug, Clone, Default)]
pub struct BuildConfig {
    pub numbers: HashMap<String, f64>,
    pub booleans: HashMap<String, bool>,
    pub strings: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_stores_mods_and_base_stats() {
        let item = Item {
            id: 1,
            rarity: ItemRarity::Rare,
            name: "Doom Edge".into(),
            base_type: "Rusted Sword".into(),
            item_type: "One Handed Sword".into(),
            quality: 20,
            sockets: vec![SocketGroup {
                colors: vec!['R', 'G', 'B'],
                linked: true,
            }],
            implicits: vec!["40% increased Global Accuracy Rating".into()],
            explicits: vec![
                "Adds 10 to 20 Physical Damage".into(),
                "+50 to maximum Life".into(),
            ],
            crafted_mods: vec!["10% increased Attack Speed".into()],
            enchant_mods: vec![],
            corrupted: false,
            foulborn: false,
            class_restriction: None,
            influence: ItemInfluence::default(),
            weapon_data: Some(ItemWeaponData {
                phys_min: 10.0,
                phys_max: 25.0,
                attack_rate: 1.4,
                crit_chance: 5.0,
                range: 11,
            }),
            armour_data: None,
            flask_data: None,
            requirements: ItemRequirements {
                level: 60,
                str_req: 100,
                dex_req: 50,
                int_req: 0,
            },
            radius: None,
            limit: None,
        };

        assert_eq!(item.id, 1);
        assert_eq!(item.rarity, ItemRarity::Rare);
        assert_eq!(item.name, "Doom Edge");
        assert_eq!(item.base_type, "Rusted Sword");
        assert_eq!(item.quality, 20);
        assert_eq!(item.implicits.len(), 1);
        assert_eq!(item.explicits.len(), 2);
        assert_eq!(item.crafted_mods.len(), 1);
        assert!(!item.corrupted);

        let wd = item.weapon_data.as_ref().unwrap();
        assert_eq!(wd.phys_min, 10.0);
        assert_eq!(wd.attack_rate, 1.4);
        assert_eq!(wd.range, 11);

        assert_eq!(item.requirements.level, 60);
        assert_eq!(item.requirements.str_req, 100);

        assert_eq!(item.sockets.len(), 1);
        assert_eq!(item.sockets[0].colors, vec!['R', 'G', 'B']);
        assert!(item.sockets[0].linked);
    }

    #[test]
    fn item_slot_from_str() {
        assert_eq!(ItemSlot::from_str("Weapon 1"), Some(ItemSlot::Weapon1));
        assert_eq!(ItemSlot::from_str("Weapon 2"), Some(ItemSlot::Weapon2));
        assert_eq!(
            ItemSlot::from_str("Body Armour"),
            Some(ItemSlot::BodyArmour)
        );
        assert_eq!(ItemSlot::from_str("Helmet"), Some(ItemSlot::Helmet));
        assert_eq!(ItemSlot::from_str("Gloves"), Some(ItemSlot::Gloves));
        assert_eq!(ItemSlot::from_str("Boots"), Some(ItemSlot::Boots));
        assert_eq!(ItemSlot::from_str("Amulet"), Some(ItemSlot::Amulet));
        assert_eq!(ItemSlot::from_str("Ring 1"), Some(ItemSlot::Ring1));
        assert_eq!(ItemSlot::from_str("Ring 2"), Some(ItemSlot::Ring2));
        assert_eq!(ItemSlot::from_str("Belt"), Some(ItemSlot::Belt));
        assert_eq!(ItemSlot::from_str("Flask 1"), Some(ItemSlot::Flask1));
        assert_eq!(ItemSlot::from_str("Flask 3"), Some(ItemSlot::Flask3));
        assert_eq!(ItemSlot::from_str("Flask 5"), Some(ItemSlot::Flask5));
        assert_eq!(ItemSlot::from_str("Jewel 1"), Some(ItemSlot::Jewel1));
        assert_eq!(ItemSlot::from_str("Jewel 15"), Some(ItemSlot::Jewel15));
        assert_eq!(ItemSlot::from_str("Jewel 21"), Some(ItemSlot::Jewel21));
        assert_eq!(ItemSlot::from_str("Jewel 22"), None);
        assert_eq!(ItemSlot::from_str("Nonsense"), None);
        assert_eq!(ItemSlot::from_str(""), None);

        // Round-trip test
        assert_eq!(ItemSlot::Weapon1.as_str(), "Weapon 1");
        assert_eq!(ItemSlot::BodyArmour.as_str(), "Body Armour");
        assert_eq!(ItemSlot::Jewel15.as_str(), "Jewel 15");

        // Category checks
        assert!(ItemSlot::Weapon1.is_weapon());
        assert!(ItemSlot::Weapon2.is_weapon());
        assert!(!ItemSlot::Helmet.is_weapon());

        assert!(ItemSlot::Flask1.is_flask());
        assert!(ItemSlot::Flask5.is_flask());
        assert!(!ItemSlot::Belt.is_flask());

        assert!(ItemSlot::Jewel1.is_jewel());
        assert!(ItemSlot::Jewel21.is_jewel());
        assert!(!ItemSlot::Amulet.is_jewel());
    }

    #[test]
    fn item_rarity_from_str() {
        assert_eq!(ItemRarity::from_str("NORMAL"), Some(ItemRarity::Normal));
        assert_eq!(ItemRarity::from_str("Normal"), Some(ItemRarity::Normal));
        assert_eq!(ItemRarity::from_str("normal"), Some(ItemRarity::Normal));
        assert_eq!(ItemRarity::from_str("MAGIC"), Some(ItemRarity::Magic));
        assert_eq!(ItemRarity::from_str("Magic"), Some(ItemRarity::Magic));
        assert_eq!(ItemRarity::from_str("RARE"), Some(ItemRarity::Rare));
        assert_eq!(ItemRarity::from_str("Rare"), Some(ItemRarity::Rare));
        assert_eq!(ItemRarity::from_str("UNIQUE"), Some(ItemRarity::Unique));
        assert_eq!(ItemRarity::from_str("Unique"), Some(ItemRarity::Unique));
        assert_eq!(ItemRarity::from_str(""), None);
        assert_eq!(ItemRarity::from_str("Legendary"), None);

        // Default is Normal
        assert_eq!(ItemRarity::default(), ItemRarity::Normal);
    }
}
