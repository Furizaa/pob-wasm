use serde::{Deserialize, Serialize};

fn is_false(v: &bool) -> bool {
    !*v
}

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

// ---------------------------------------------------------------------------
// Skill Gems
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillGemData {
    pub id: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "is_false")]
    pub is_support: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skill_types: Vec<String>,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub cast_time: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub base_effectiveness: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub incremental_effectiveness: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub base_flags: Vec<String>,
    pub levels: Vec<SkillLevelData>,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub mana_multiplier_at_20: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub require_skill_types: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add_skill_types: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exclude_skill_types: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constant_stats: Vec<StatEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub quality_stats: Vec<StatEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatEntry {
    pub stat_id: String,
    pub value: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillLevelData {
    pub level: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub level_requirement: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stat_values: Vec<f64>,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub crit_chance: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub damage_effectiveness: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub attack_speed_mult: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub mana_cost: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub life_cost: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub mana_multiplier: f64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub stored_uses: u32,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub cooldown: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub duration: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub mana_reservation_flat: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub mana_reservation_percent: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub life_reservation_flat: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub life_reservation_percent: f64,
}

// ---------------------------------------------------------------------------
// Base Items
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BaseItemData {
    pub name: String,
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_type: Option<String>,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub socket_limit: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub implicit: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon: Option<WeaponData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub armour: Option<ArmourData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flask: Option<FlaskData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req: Option<BaseRequirements>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WeaponData {
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub physical_min: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub physical_max: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub crit_chance_base: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub attack_rate_base: f64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub range: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ArmourData {
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub armour_min: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub armour_max: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub evasion_min: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub evasion_max: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub energy_shield_min: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub energy_shield_max: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub ward_min: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub ward_max: f64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub block_chance: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub movement_penalty: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FlaskData {
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub life: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub mana: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub duration: f64,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub charges_used: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub charges_max: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BaseRequirements {
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub level: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub str_req: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub dex_req: u32,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub int_req: u32,
}

// ---------------------------------------------------------------------------
// Unique Items
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UniqueItemData {
    pub name: String,
    pub base_type: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub implicits: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub explicits: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<String>,
}

// ---------------------------------------------------------------------------
// Item Mods
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemModData {
    pub id: String,
    pub mod_type: String,
    pub domain: String,
    pub generation_type: String,
    pub stats: Vec<ItemModStat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "is_zero_u32")]
    pub level_requirement: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemModStat {
    pub stat_id: String,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub min: i64,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub max: i64,
}
