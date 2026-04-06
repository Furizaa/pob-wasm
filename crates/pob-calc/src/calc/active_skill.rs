// Resolve active skills from build.skill_sets and set skill flags.
//
// Reference: third-party/PathOfBuilding/src/Modules/CalcSetup.lua  (lines 1292–1789)
//            third-party/PathOfBuilding/src/Modules/CalcActiveSkill.lua (full file)
//
// Port strategy:
//   Phase 4 — Support collection (CalcSetup.lua:1441–1552):
//       Walk all enabled socket groups; for each enabled support gem call
//       add_best_support() into that group's support list.
//       Also build a per-slot index: slot_support_lists[slot_name] = list of
//       per-group support lists for every group socketed in that slot.
//       Mirrors supportLists[slotName][group] (CalcSetup.lua:1460–1462).
//
//   Phase 5 — Active skill creation (CalcSetup.lua:1554–1676):
//       Walk all enabled socket groups; for each enabled non-support gem call
//       create_active_skill(), then append to active_skill_list.
//       For the main socket group, set main_skill from mainActiveSkill index.
//       For item-granted skills (group.source.is_some()), merge ALL support
//       lists for the same slot into the applied_support_list (Lua:1611–1628).
//       Also handles crossLinkedSupportGroups (Lua:1631–1649) — see TODO below.
//
//   Fallback — if no main_skill, build the default Melee active skill.
//
//   Phase 7 — build_active_skill_mod_list (CalcSetup.lua:1756–1759):
//       For every active skill in active_skill_list call build_active_skill_mod_list().
//       This constructs skillCfg, skillModFlags, skillKeywordFlags, and populates
//       skill_data (CritChance, attackTime, manaReservationPercent, etc.).

use std::collections::HashMap;
use std::sync::LazyLock;

use super::env::CalcEnv;
use crate::build::types::{ActiveSkill, SupportEffect};
use crate::data::gems::{GemData, GemsMap};
use crate::mod_db::types::{KeywordFlags, ModFlags, ModValue, SkillCfg};
use crate::mod_db::ModDb;

// ── Heuristic fallback lists (used when gem data is unavailable) ────────────

static KNOWN_SUMMONER_SKILLS: LazyLock<std::collections::HashSet<&'static str>> =
    LazyLock::new(|| {
        [
            "Raise Zombie",
            "Raise Spectre",
            "Summon Skeleton",
            "Summon Raging Spirit",
            "Animate Weapon",
            "Animate Guardian",
            "Summon Golem",
            "Summon Chaos Golem",
            "Summon Flame Golem",
            "Summon Ice Golem",
            "Summon Lightning Golem",
            "Summon Stone Golem",
        ]
        .iter()
        .copied()
        .collect()
    });

// ── Gem data lookup helper ──────────────────────────────────────────────────

/// Look up a gem in the gems map by trying:
/// 1. Exact match on `skill_id`
/// 2. Normalized ID (legacy renames like "AngerAura" → "Anger")
/// 3. Lowercase match
/// 4. Lowercase with spaces replaced by underscores
fn lookup_gem<'a>(gems: &'a GemsMap, skill_id: &str) -> Option<&'a GemData> {
    gems.get(skill_id)
        .or_else(|| {
            let normalized = crate::calc::setup::normalize_gem_skill_id(skill_id);
            if normalized != skill_id {
                gems.get(normalized)
            } else {
                None
            }
        })
        .or_else(|| {
            let lower = skill_id.to_lowercase();
            gems.get(&lower)
                .or_else(|| gems.get(&lower.replace(' ', "_")))
        })
}

/// Returns the normalized skill ID for an active skill, applying legacy renames.
/// This is the ID stored in the gems data map.
fn normalize_skill_id(skill_id: &str) -> String {
    let normalized = crate::calc::setup::normalize_gem_skill_id(skill_id);
    normalized.to_string()
}

// ── Support gem matching ────────────────────────────────────────────────────

/// Port of `calcLib.canGrantedEffectSupportActiveSkill` (CalcTools.lua:85–144).
/// Evaluate a PoB skill-type expression using a stack machine.
///
/// The expression is a list of type names and logical operators (AND, OR, NOT).
/// This mirrors `calcLib.doesTypeExpressionMatch` in CalcTools.lua.
///
/// Return value: true if any element of the final stack is true.
fn eval_type_expression(expr: &[String], active_skill_types: &[String]) -> bool {
    let mut stack: Vec<bool> = Vec::new();
    for token in expr {
        if token.eq_ignore_ascii_case("OR") {
            if stack.len() >= 2 {
                let b = stack.pop().unwrap();
                let a = stack.last_mut().unwrap();
                *a = *a || b;
            }
        } else if token.eq_ignore_ascii_case("AND") {
            if stack.len() >= 2 {
                let b = stack.pop().unwrap();
                let a = stack.last_mut().unwrap();
                *a = *a && b;
            }
        } else if token.eq_ignore_ascii_case("NOT") {
            if let Some(a) = stack.last_mut() {
                *a = !*a;
            }
        } else {
            // Type name: check if active skill has this type
            let has_type = active_skill_types
                .iter()
                .any(|ast| ast.eq_ignore_ascii_case(token));
            stack.push(has_type);
        }
    }
    // Return true if any entry in the stack is true
    stack.iter().any(|&v| v)
}

/// Determines if a support gem can support an active skill based on skill type
/// requirements and exclusions.
///
/// Uses a stack machine to evaluate AND/OR/NOT type expressions, mirroring
/// `calcLib.doesTypeExpressionMatch` in CalcTools.lua.
fn can_support(support_data: &GemData, active_skill_types: &[String]) -> bool {
    // Check exclude_skill_types: excluded if the expression evaluates to true
    if !support_data.exclude_skill_types.is_empty() {
        if eval_type_expression(&support_data.exclude_skill_types, active_skill_types) {
            return false;
        }
    }
    // Check require_skill_types: must satisfy the expression (evaluates to true)
    if !support_data.require_skill_types.is_empty() {
        if !eval_type_expression(&support_data.require_skill_types, active_skill_types) {
            return false;
        }
    }
    true
}

// ── ExtraSupport helpers ──────────────────────────────────────────────────────

/// Parse an ExtraSupport mod value of the form "DisplayName:Level" (e.g. "Blasphemy:22").
/// Returns (display_name, level_as_u8).  If the value has no colon, returns
/// (whole_string, 1).
fn parse_extra_support_value(val: &str) -> (String, u8) {
    if let Some(colon) = val.rfind(':') {
        let name = val[..colon].trim().to_string();
        let level: u8 = val[colon + 1..].trim().parse().unwrap_or(1);
        (name, level)
    } else {
        (val.trim().to_string(), 1)
    }
}

/// Look up a gem's skill_id (map key) by its display_name.
/// Tries exact match first, then "<name> Support" (Lua does the same:
/// if grantedEffect and not grantedEffect.support then
///   grantedEffect = env.data.skills["Support"..value.skillId]
/// but since Rust stores the display_name, we try "<name> Support").
/// Only returns gems that are supports (is_support = true).
fn lookup_gem_id_by_display_name(gems: &crate::data::gems::GemsMap, name: &str) -> Option<String> {
    // Exact match
    for (id, gd) in gems {
        if gd.is_support && gd.display_name.eq_ignore_ascii_case(name) {
            return Some(id.clone());
        }
    }
    // Try "<name> Support" (Lua: "Support"..value.skillId)
    let with_support = format!("{} Support", name);
    for (id, gd) in gems {
        if gd.is_support && gd.display_name.eq_ignore_ascii_case(&with_support) {
            return Some(id.clone());
        }
    }
    None
}

// ── addBestSupport deduplication ─────────────────────────────────────────────

/// Port of `addBestSupport` (CalcSetup.lua:320–349).
///
/// Adds `effect` to `list` with deduplication:
/// - Same gem id: keep the higher level/quality one (mark lower as superseded).
/// - `effect.plus_version_of == existing.id`: awakened wins, replaces base.
/// - `existing.plus_version_of == effect.id`: base loses to already-present awakened.
/// - Otherwise: append.
///
/// `plus_version_of` is stored in GemData.  Since our GemData struct does not yet
/// have this field (it is rarely populated), we approximate by checking if the
/// new effect's gem_data id starts with "Awakened" and the existing does not.
fn add_best_support(effect: SupportEffect, list: &mut Vec<SupportEffect>) {
    // Check if an equivalent (same skill_id) is already present.
    for i in 0..list.len() {
        if list[i].skill_id == effect.skill_id {
            // Same gem — keep the higher level (or quality if same level).
            if effect.level > list[i].level
                || (effect.level == list[i].level && effect.quality > list[i].quality)
            {
                list[i] = effect;
            }
            // else: existing is better or equal, discard new one
            return;
        }

        // Awakened-supersedes-base heuristic:
        // If the new gem is the "Awakened" version of the existing gem,
        // replace the base with the awakened one.
        // Pattern: "Awakened X" supersedes "X" (same base name minus "Awakened ").
        let new_id = effect.skill_id.as_str();
        let existing_id = list[i].skill_id.as_str();
        if new_id.starts_with("Awakened") {
            let base_name = new_id.trim_start_matches("Awakened").trim();
            if existing_id.eq_ignore_ascii_case(base_name) {
                list[i] = effect;
                return;
            }
        } else if existing_id.starts_with("Awakened") {
            let base_name = existing_id.trim_start_matches("Awakened").trim();
            if new_id.eq_ignore_ascii_case(base_name) {
                // existing is awakened, new is base — keep existing
                return;
            }
        }
    }
    list.push(effect);
}

// ── createActiveSkill ──────────────────────────────────────────────────────

/// Port of `calcs.createActiveSkill` (CalcActiveSkill.lua:82–161).
///
/// Creates an ActiveSkill from an active gem plus its applicable support list.
/// Performs the two-pass support skill-type propagation (Pass 1: add skill types
/// from compatible supports; Pass 2: collect compatible supports into effectList
/// and apply addFlags to skillFlags).
///
/// Arguments:
///   - `skill_id`: the active gem's skill ID
///   - `level`, `quality`: gem level/quality
///   - `gem_data`: gem's data entry (for skill_types, base_flags, add_skill_types)
///   - `support_list`: the pre-collected list of SupportEffect for this skill
///   - `no_supports`: if true, no supports are applied (item-granted skill)
///   - `slot_name`: equipment slot this skill is socketed in
///   - `gems`: the full gems map (for looking up support gem data)
fn create_active_skill(
    skill_id: String,
    level: u8,
    quality: u8,
    gem_data: Option<&GemData>,
    support_list: Vec<SupportEffect>,
    no_supports: bool,
    slot_name: Option<String>,
    gems: &GemsMap,
) -> ActiveSkill {
    // ── Initialise skill_types from active gem data ──────────────────────────
    let mut skill_types: Vec<String> = gem_data
        .map(|gd| gd.skill_types.clone())
        .unwrap_or_default();

    // ── Initialise skill_flags from active gem base_flags ───────────────────
    let mut skill_flags: HashMap<String, bool> = gem_data
        .map(|gd| gd.base_flags.iter().map(|f| (f.clone(), true)).collect())
        .unwrap_or_default();

    // Lua: skillFlags.hit = hit OR Attack OR Damage OR Projectile
    let has_hit = skill_flags.get("hit").copied().unwrap_or(false)
        || skill_types.iter().any(|t| t.eq_ignore_ascii_case("Attack"))
        || skill_types.iter().any(|t| t.eq_ignore_ascii_case("Damage"))
        || skill_types
            .iter()
            .any(|t| t.eq_ignore_ascii_case("Projectile"));
    if has_hit {
        skill_flags.insert("hit".to_string(), true);
    }

    // ── Pass 1: Add skill types from compatible supports ─────────────────────
    // (CalcActiveSkill.lua:110–140)
    // Some supports add skill types to the active skill (e.g. Blasphemy adds
    // HasReservation, Aura, etc.).  After each addition, previously-rejected
    // supports may become compatible.  Repeat until stable (fixed-point).
    if !no_supports {
        let mut rejected_indices: Vec<usize> = Vec::new();

        // Initial pass: process each support, track incompatible ones
        for (idx, se) in support_list.iter().enumerate() {
            if let Some(sup_gd) = lookup_gem(gems, &se.skill_id) {
                if sup_gd.is_support && can_support(sup_gd, &skill_types) {
                    for add_type in &sup_gd.add_skill_types {
                        if !skill_types.iter().any(|t| t.eq_ignore_ascii_case(add_type)) {
                            skill_types.push(add_type.clone());
                        }
                    }
                } else if sup_gd.is_support {
                    rejected_indices.push(idx);
                }
            }
        }

        // Fixed-point loop: re-evaluate rejected supports
        loop {
            let mut added_new = false;
            let mut still_rejected: Vec<usize> = Vec::new();
            for &idx in &rejected_indices {
                let se = &support_list[idx];
                if let Some(sup_gd) = lookup_gem(gems, &se.skill_id) {
                    if sup_gd.is_support && can_support(sup_gd, &skill_types) {
                        added_new = true;
                        for add_type in &sup_gd.add_skill_types {
                            if !skill_types.iter().any(|t| t.eq_ignore_ascii_case(add_type)) {
                                skill_types.push(add_type.clone());
                            }
                        }
                    } else {
                        still_rejected.push(idx);
                    }
                }
            }
            rejected_indices = still_rejected;
            if !added_new {
                break;
            }
        }
    }

    // ── Pass 2: Collect compatible supports into effect_list, apply addFlags ─
    // (CalcActiveSkill.lua:142–158)
    // effectList in Lua starts with [activeEffect].  In Rust, we just store the
    // compatible support_list entries (active effect info lives on the skill itself).
    let compatible_supports: Vec<SupportEffect> = if no_supports {
        Vec::new()
    } else {
        support_list
            .into_iter()
            .filter(|se| {
                if let Some(sup_gd) = lookup_gem(gems, &se.skill_id) {
                    if sup_gd.is_support && can_support(sup_gd, &skill_types) {
                        // Apply addFlags to skillFlags (Lua: supportEffect.grantedEffect.addFlags)
                        // Currently no addFlags in GemData, so this is a no-op stub.
                        return true;
                    }
                }
                false
            })
            .collect()
    };

    // ── Derive is_attack, is_spell, is_melee from skill_types / skill_flags ──
    // When gem data is available, skill_types / skill_flags are authoritative.
    // When gem data is unavailable (stub data / unknown gem), apply heuristics
    // so that known gem names still get correct classification.
    let data_driven = gem_data.is_some();
    let is_spell = if data_driven {
        skill_types.iter().any(|t| t.eq_ignore_ascii_case("Spell"))
            || skill_flags.get("spell").copied().unwrap_or(false)
    } else {
        // Heuristic fallback — for tests with stub data
        static KNOWN_SPELLS_FB: LazyLock<std::collections::HashSet<&'static str>> =
            LazyLock::new(|| {
                [
                    "Fireball",
                    "Frostbolt",
                    "Arc",
                    "Lightning Bolt",
                    "Freezing Pulse",
                    "Ball Lightning",
                    "Storm Call",
                    "Ice Nova",
                    "Vaal Fireball",
                    "Spark",
                    "Incinerate",
                    "Flameblast",
                    "Scorching Ray",
                    "Firestorm",
                    "Glacial Cascade",
                    "Ice Spear",
                    "Arctic Breath",
                    "Discharge",
                    "Ethereal Knives",
                ]
                .iter()
                .copied()
                .collect()
            });
        KNOWN_SPELLS_FB.contains(skill_id.as_str())
    };
    let is_attack = if data_driven {
        skill_types.iter().any(|t| t.eq_ignore_ascii_case("Attack"))
            || skill_flags.get("attack").copied().unwrap_or(false)
    } else {
        !is_spell
    };
    let is_melee = if data_driven {
        skill_flags.get("melee").copied().unwrap_or(false)
            || skill_types.iter().any(|t| t.eq_ignore_ascii_case("Melee"))
            || (is_attack
                && !skill_flags.get("projectile").copied().unwrap_or(false)
                && !skill_types
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case("Projectile")))
    } else {
        static KNOWN_RANGED_FB: LazyLock<std::collections::HashSet<&'static str>> =
            LazyLock::new(|| {
                [
                    "Tornado Shot",
                    "Barrage",
                    "Split Arrow",
                    "Burning Arrow",
                    "Rain of Arrows",
                    "Lightning Arrow",
                    "Ice Shot",
                    "Shrapnel Shot",
                    "Puncture",
                    "Kinetic Blast",
                ]
                .iter()
                .copied()
                .collect()
            });
        is_attack && !KNOWN_RANGED_FB.contains(skill_id.as_str())
    };

    // ── Populate base damage and timing from gem level data ─────────────────
    let mut base_damage: HashMap<String, (f64, f64)> = HashMap::new();
    let mut cast_time = if is_spell { 0.7 } else { 0.0 };
    let mut attack_speed_base = if is_attack { 1.5 } else { 0.0 };
    let mut base_crit_chance = if is_spell { 0.06 } else { 0.05 };
    let mut skill_data: HashMap<String, f64> = HashMap::new();

    if let Some(gd) = gem_data {
        if let Some(level_data) = gd.levels.iter().find(|l| l.level == level) {
            macro_rules! ins {
                ($key:expr, $min:expr, $max:expr) => {
                    if $min > 0.0 || $max > 0.0 {
                        base_damage.insert($key.to_string(), ($min, $max));
                    }
                };
            }
            ins!("Physical", level_data.phys_min, level_data.phys_max);
            ins!("Fire", level_data.fire_min, level_data.fire_max);
            ins!("Cold", level_data.cold_min, level_data.cold_max);
            ins!(
                "Lightning",
                level_data.lightning_min,
                level_data.lightning_max
            );
            ins!("Chaos", level_data.chaos_min, level_data.chaos_max);

            if level_data.crit_chance > 0.0 {
                base_crit_chance = level_data.crit_chance;
            }
            if level_data.cast_time > 0.0 {
                cast_time = level_data.cast_time;
            }
            if level_data.attack_speed_mult > 0.0 {
                attack_speed_base = level_data.attack_speed_mult;
            }

            // ── Level data extraction (CalcActiveSkill.lua:554–577) ────────
            // skillData.CritChance
            skill_data.insert("CritChance".to_string(), level_data.crit_chance);

            // skillData.attackTime  (only if > 0)
            // In Lua: attackTime is an integer in milliseconds.
            // GemLevelData.cast_time is in seconds; the Lua level data may have
            // attackTime as a separate field.  For now we store cast_time as the
            // best proxy.
            if level_data.cast_time > 0.0 {
                skill_data.insert("attackTime".to_string(), level_data.cast_time * 1000.0);
            }

            if level_data.attack_speed_mult > 0.0 {
                skill_data.insert(
                    "attackSpeedMultiplier".to_string(),
                    level_data.attack_speed_mult,
                );
            }

            if level_data.cooldown > 0.0 {
                skill_data.insert("cooldown".to_string(), level_data.cooldown);
            }

            if level_data.stored_uses > 0 {
                skill_data.insert("storedUses".to_string(), level_data.stored_uses as f64);
            }

            if level_data.mana_reservation_percent > 0.0 {
                skill_data.insert(
                    "manaReservationPercent".to_string(),
                    level_data.mana_reservation_percent,
                );
            }

            if level_data.level_requirement > 0 {
                // totemLevel uses levelRequirement of the gem (Lua: CalcActiveSkill.lua:591)
                // Only set if skill is a totem; checked later in build_active_skill_mod_list
                // For now, store the level requirement for downstream use.
                skill_data.insert(
                    "totemLevel".to_string(),
                    level_data.level_requirement as f64,
                );
            }
        }
    }

    let display_name = gem_data
        .map(|gd| gd.display_name.clone())
        .unwrap_or_default();

    ActiveSkill {
        skill_id,
        level,
        quality,
        skill_mod_db: ModDb::new(),
        is_attack,
        is_spell,
        is_melee,
        can_crit: true,
        base_crit_chance,
        base_damage,
        attack_speed_base,
        cast_time,
        damage_effectiveness: 1.0,
        skill_types,
        skill_flags,
        skill_cfg: None,
        slot_name,
        support_list: compatible_supports,
        triggered_by: None,
        skill_data,
        skill_part: None,
        no_supports,
        disable_reason: None,
        weapon1_flags: 0,
        weapon2_flags: 0,
        active_mine_count: None,
        active_stage_count: None,
        display_name,
    }
}

// ── build_active_skill_mod_list ────────────────────────────────────────────

/// Port of `calcs.buildActiveSkillModList` (CalcActiveSkill.lua:227–843).
///
/// Builds the skill's `skill_cfg` (skillModFlags + skillKeywordFlags),
/// merges support gem mods (manaReservationPercent, triggeredBy),
/// and updates skill_data from level data.
pub fn build_active_skill_mod_list(
    skill: &mut ActiveSkill,
    env_mode_buffs: bool,
    env_mode_combat: bool,
    env_mode_effective: bool,
) {
    build_active_skill_mod_list_with_gems(
        skill,
        env_mode_buffs,
        env_mode_combat,
        env_mode_effective,
        None,
    );
}

/// Full version of build_active_skill_mod_list that accepts an optional gems map
/// for populating SupportManaMultiplier mods on skill_mod_db.
pub fn build_active_skill_mod_list_with_gems(
    skill: &mut ActiveSkill,
    env_mode_buffs: bool,
    env_mode_combat: bool,
    env_mode_effective: bool,
    gems_for_mod_list: Option<&GemsMap>,
) {
    // ── Mode flags (CalcActiveSkill.lua:235–243) ───────────────────────────
    if env_mode_buffs {
        skill.skill_flags.insert("buffs".to_string(), true);
    }
    if env_mode_combat {
        skill.skill_flags.insert("combat".to_string(), true);
    }
    if env_mode_effective {
        skill.skill_flags.insert("effective".to_string(), true);
    }

    // ── skillModFlags (CalcActiveSkill.lua:341–362) ──────────────────────
    // Snapshot all needed boolean values before any mutations.
    // This avoids Rust borrow conflicts when we call skill.skill_flags.insert() later.
    let f_hit = skill.skill_flags.get("hit").copied().unwrap_or(false);
    let f_attack = skill.skill_flags.get("attack").copied().unwrap_or(false);
    let f_spell = skill.skill_flags.get("spell").copied().unwrap_or(false);
    let f_melee = skill.skill_flags.get("melee").copied().unwrap_or(false);
    let f_projectile = skill
        .skill_flags
        .get("projectile")
        .copied()
        .unwrap_or(false);
    let f_area = skill.skill_flags.get("area").copied().unwrap_or(false);
    let f_weapon1 = skill
        .skill_flags
        .get("weapon1Attack")
        .copied()
        .unwrap_or(false);
    let f_brand = skill.skill_flags.get("brand").copied().unwrap_or(false);
    let f_arrow = skill.skill_flags.get("arrow").copied().unwrap_or(false);
    let f_totem = skill.skill_flags.get("totem").copied().unwrap_or(false);
    let f_trap = skill.skill_flags.get("trap").copied().unwrap_or(false);
    let f_mine = skill.skill_flags.get("mine").copied().unwrap_or(false);
    let f_cast = skill.skill_flags.get("cast").copied().unwrap_or(false);

    let t_attack = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("Attack"));
    let t_spell = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("Spell"));
    let t_aura = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("Aura"));
    let t_curse = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("AppliesCurse"));
    let t_warcry = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("Warcry"));
    let t_movement = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("Movement"));
    let t_vaal = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("Vaal"));
    let t_lightning = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("Lightning"));
    let t_cold = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("Cold"));
    let t_fire = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("Fire"));
    let t_chaos = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("Chaos"));
    let t_physical = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("Physical"));
    let t_triggered = skill
        .skill_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("Triggered"));

    let mut skill_mod_flags = ModFlags::NONE;

    if f_hit {
        skill_mod_flags = skill_mod_flags | ModFlags::HIT;
    }
    if f_attack {
        skill_mod_flags = skill_mod_flags | ModFlags::ATTACK;
    } else {
        // Non-attack skills always get CAST (spells and everything else)
        skill_mod_flags = skill_mod_flags | ModFlags::CAST;
        if f_spell {
            skill_mod_flags = skill_mod_flags | ModFlags::SPELL;
        }
    }
    if f_melee {
        skill_mod_flags = skill_mod_flags | ModFlags::MELEE;
    } else if f_projectile {
        skill_mod_flags = skill_mod_flags | ModFlags::PROJECTILE;
        // Lua: skillFlags.chaining = true for projectile skills
        // (not stored in current skill_flags but set as a side effect)
    }
    if f_area {
        skill_mod_flags = skill_mod_flags | ModFlags::AREA;
    }

    // Add weapon flags to cfg flags (weapon1Flags | weapon2Flags)
    let weapon_flags_u32 = skill.weapon1_flags | skill.weapon2_flags;
    let combined_flags = ModFlags(skill_mod_flags.0 | weapon_flags_u32);

    // ── skillKeywordFlags (CalcActiveSkill.lua:363–421) ─────────────────
    let mut skill_keyword_flags = KeywordFlags::NONE;

    if f_hit {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::HIT;
    }
    // SkillType-based keyword flags
    if t_aura {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::AURA;
    }
    if t_curse {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::CURSE;
    }
    if t_warcry {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::WARCRY;
    }
    if t_movement {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::MOVEMENT;
    }
    if t_vaal {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::VAAL;
    }
    if t_lightning {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::LIGHTNING;
    }
    if t_cold {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::COLD;
    }
    if t_fire {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::FIRE;
    }
    if t_chaos {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::CHAOS;
    }
    if t_physical {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::PHYSICAL;
    }
    // Bow keyword: weapon1Attack AND weapon1Flags has Bow bit
    if f_weapon1 && (skill.weapon1_flags & ModFlags::BOW.0) != 0 {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::BOW;
    }
    if f_brand {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::BRAND;
    }
    if f_arrow {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::ARROW;
    }
    let is_self_cast = if f_totem {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::TOTEM;
        false
    } else if f_trap {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::TRAP;
        false
    } else if f_mine {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::MINE;
        false
    } else {
        !t_triggered
    };
    if is_self_cast {
        // Not totem/trap/mine/triggered → selfcast
        skill.skill_flags.insert("selfCast".to_string(), true);
    }
    if t_attack {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::ATTACK;
    }
    // Spell keyword: SkillType.Spell AND not skillFlags.cast
    if t_spell && !f_cast {
        skill_keyword_flags = skill_keyword_flags | KeywordFlags::SPELL;
    }

    // ── Build skillCfg (CalcActiveSkill.lua:446–470) ─────────────────────
    // Strip "Vaal " prefix so Vaal skills also match their base skill name mods.
    let skill_name = skill.skill_id.trim_start_matches("Vaal ").to_string();

    skill.skill_cfg = Some(SkillCfg {
        flags: combined_flags,
        keyword_flags: skill_keyword_flags,
        slot_name: skill.slot_name.clone(),
        skill_name: Some(skill_name.clone()),
        skill_id: Some(skill.skill_id.clone()),
        ..Default::default()
    });

    // ── Add SupportManaMultiplier MORE mods from support gems ──────────────
    // Mirrors CalcActiveSkill.lua:500–506: for each support in effectList,
    // add SupportManaMultiplier MORE mod with the support's mana_multiplier.
    if let Some(gems) = gems_for_mod_list {
        use crate::mod_db::types::{Mod, ModSource, ModType, ModValue};
        for se in &skill.support_list {
            // Look up the support gem's level data
            if let Some(sup_gd) = lookup_gem(gems, &se.skill_id) {
                if sup_gd.is_support {
                    let sup_level = sup_gd
                        .levels
                        .iter()
                        .find(|l| l.level == se.level)
                        .or_else(|| sup_gd.levels.last());
                    if let Some(sl) = sup_level {
                        // Lua: level.manaMultiplier is the delta from 100 (e.g. -12 for Enlighten L4)
                        // Lua adds: NewMod("SupportManaMultiplier", "MORE", level.manaMultiplier, ...)
                        // MORE semantics: product of (1 + value/100) for each
                        if sl.mana_multiplier != 0.0 {
                            skill.skill_mod_db.add(Mod {
                                name: "SupportManaMultiplier".into(),
                                mod_type: ModType::More,
                                value: ModValue::Number(sl.mana_multiplier),
                                flags: ModFlags::NONE,
                                keyword_flags: KeywordFlags::NONE,
                                tags: vec![],
                                source: ModSource::new("Support", &se.skill_id),
                            });
                        }
                        // Lua: if level.manaReservationPercent then skillData.manaReservationPercent = level.manaReservationPercent
                        if sl.mana_reservation_percent > 0.0 {
                            skill.skill_data.insert(
                                "manaReservationPercent".into(),
                                sl.mana_reservation_percent,
                            );
                        }
                    }

                    // Process constant_stats from support gem.
                    // Mirrors calcLib.getActiveSkillStats() / SkillStatMap processing.
                    // Constant stats contribute fixed BASE mods to the skill's mod list
                    // regardless of gem level (e.g. Multiple Totems adds ActiveTotemLimit BASE 2,
                    // Cluster Traps adds ActiveTrapLimit BASE 5).
                    for cs in &sup_gd.constant_stats {
                        // Map known stat IDs to their mod names (mirrors SkillStatMap.lua).
                        // Some stats need specific ModFlags to match correctly in queries.
                        let mapping: Option<(&str, ModFlags)> = match cs.stat_id.as_str() {
                            // Base counts (from characterConstants stat map)
                            "base_number_of_totems_allowed" => Some(("ActiveTotemLimit", ModFlags::NONE)),
                            "base_number_of_ballistas_allowed" => Some(("ActiveBallistaLimit", ModFlags::NONE)),
                            "base_number_of_traps_allowed" => Some(("ActiveTrapLimit", ModFlags::NONE)),
                            "base_number_of_remote_mines_allowed" => Some(("ActiveMineLimit", ModFlags::NONE)),
                            "base_number_of_brands_allowed" => Some(("ActiveBrandLimit", ModFlags::NONE)),
                            // Additional counts (SkillStatMap.lua entries)
                            "number_of_additional_traps_allowed" => Some(("ActiveTrapLimit", ModFlags::NONE)),
                            "number_of_additional_remote_mines_allowed" => Some(("ActiveMineLimit", ModFlags::NONE)),
                            "number_of_additional_ballistas_allowed" => Some(("ActiveBallistaLimit", ModFlags::NONE)),
                            // Repeat counts (SkillStatMap.lua — Spell Echo, Multistrike, etc.)
                            // base_spell_repeat_count: ModFlag.Cast restricts to spells
                            "base_spell_repeat_count" => Some(("RepeatCount", ModFlags::CAST)),
                            // base_melee_attack_repeat_count: needs ModFlagOr(WeaponMelee|Unarmed)
                            // + SkillType(RequiresShield) tags — use MELEE as approximation
                            "base_melee_attack_repeat_count" => Some(("RepeatCount", ModFlags::MELEE)),
                            // skill_repeat_count: needs SkillType(Multicastable) tag — use CAST
                            // as approximation (Multicastable skills are cast skills)
                            "skill_repeat_count" => Some(("RepeatCount", ModFlags::CAST)),
                            _ => None,
                        };
                        if let Some((name, mod_flags)) = mapping {
                            skill.skill_mod_db.add(Mod {
                                name: name.into(),
                                mod_type: ModType::Base,
                                value: ModValue::Number(cs.value),
                                flags: mod_flags,
                                keyword_flags: KeywordFlags::NONE,
                                tags: Vec::new(),
                                source: ModSource::new("Support", &se.skill_id),
                            });
                        }
                    }
                }
            }
        }
    }
}

// ── set_skill_conditions ───────────────────────────────────────────────────

/// Set skill-related conditions on the player mod_db based on the active skill
/// and equipped weapon data.
pub fn set_skill_conditions(env: &mut CalcEnv) {
    let skill = match env.player.main_skill.as_ref() {
        Some(s) => s,
        None => return,
    };

    // Core skill type conditions
    env.player.mod_db.set_condition("IsMainSkill", true);
    if skill.is_attack {
        env.player.mod_db.set_condition("UsingAttack", true);
    }
    if skill.is_spell {
        env.player.mod_db.set_condition("UsingSpell", true);
    }
    if skill.is_melee {
        env.player.mod_db.set_condition("UsingMelee", true);
    }

    // Weapon-type conditions based on slot and weapon data
    if let Some(wd) = env.player.weapon_data1.as_ref() {
        if wd.phys_min > 0.0 || wd.phys_max > 0.0 {
            env.player.mod_db.set_condition("UsingWeapon", true);
        }
    }

    if env.player.dual_wield {
        env.player.mod_db.set_condition("DualWielding", true);
    }
    if env.player.has_shield {
        env.player.mod_db.set_condition("UsingShield", true);
    }
}

// ── build_skill_cfg (legacy helper kept for compatibility) ─────────────────

/// Build a SkillCfg from the active skill's flags and metadata.
/// This is now a thin wrapper — the real logic is in build_active_skill_mod_list.
pub fn build_skill_cfg(skill: &ActiveSkill) -> SkillCfg {
    let mut flags = ModFlags::NONE;
    if skill.is_attack {
        flags = flags | ModFlags::ATTACK;
    } else {
        flags = flags | ModFlags::CAST;
    }
    if skill.is_spell {
        flags = flags | ModFlags::SPELL;
    }
    flags = flags | ModFlags::HIT;
    if skill.is_melee {
        flags = flags | ModFlags::MELEE;
    }
    if skill
        .skill_flags
        .get("projectile")
        .copied()
        .unwrap_or(false)
    {
        flags = flags | ModFlags::PROJECTILE;
    }
    if skill.skill_flags.get("area").copied().unwrap_or(false) {
        flags = flags | ModFlags::AREA;
    }

    let mut keyword_flags = KeywordFlags::NONE;
    if skill.is_attack {
        keyword_flags = keyword_flags | KeywordFlags::ATTACK;
    }
    if skill.is_spell {
        keyword_flags = keyword_flags | KeywordFlags::SPELL;
    }
    keyword_flags = keyword_flags | KeywordFlags::HIT;

    SkillCfg {
        flags,
        keyword_flags,
        slot_name: skill.slot_name.clone(),
        skill_name: Some(skill.skill_id.clone()),
        skill_id: Some(skill.skill_id.clone()),
        ..Default::default()
    }
}

// ── run ────────────────────────────────────────────────────────────────────

pub fn run(env: &mut CalcEnv, build: &crate::build::Build) {
    // Resolve the active skill set
    let skill_set_idx = build.active_skill_set;
    let Some(skill_set) = build.skill_sets.get(skill_set_idx) else {
        build_fallback_main_skill(env);
        return;
    };

    // 1-based mainSocketGroup in build (Lua), 0-based in Rust.
    let main_socket_group_idx = build.main_socket_group;

    // ── Phase 4: Support collection ───────────────────────────────────────
    // For each enabled socket group, collect supports into per-group support lists.
    // support_lists[group_index] = Vec<SupportEffect>
    //
    // Mirrors CalcSetup.lua:1445–1552.
    let mut support_lists: Vec<Vec<SupportEffect>> = vec![Vec::new(); skill_set.skills.len()];

    // Per-slot support index: slot_support_lists[slot_name] collects the
    // support list for every group socketed in that slot.
    // Mirrors supportLists[slotName] (CalcSetup.lua:1460–1462).
    // Used in Phase 5 for item-granted skills (group.source.is_some()).
    let mut slot_support_lists: HashMap<String, Vec<usize>> = HashMap::new();

    for (group_idx, group) in skill_set.skills.iter().enumerate() {
        // slotEnabled: for now, all groups are slot-enabled (weapon set switching
        // is not implemented). Lua: group.slotEnabled = not slot or not slot.weaponSet or ...
        let slot_enabled = true; // TODO: implement weapon set check when needed

        let is_main = group_idx == main_socket_group_idx;
        let is_active = is_main || (group.enabled && slot_enabled);
        if !is_active {
            continue;
        }

        // Build the per-slot index: record every active group's index under its slot.
        // Mirrors CalcSetup.lua:1459–1462: if groupCfg.slotName then
        //   supportLists[slotName][group] = {}
        if !group.slot.is_empty() {
            slot_support_lists
                .entry(group.slot.clone())
                .or_default()
                .push(group_idx);
        }

        for gem in &group.gems {
            if !gem.enabled {
                continue;
            }
            // Resolve is_support from gem data
            let is_support = if let Some(gd) = lookup_gem(&env.data.gems, &gem.skill_id) {
                gd.is_support
            } else {
                gem.is_support
            };
            if !is_support {
                continue;
            }

            let support_effect = SupportEffect {
                skill_id: gem.skill_id.clone(),
                level: gem.level,
                quality: gem.quality,
                gem_data: None,
            };
            add_best_support(support_effect, &mut support_lists[group_idx]);
        }

        // Collect ExtraSupport mods from item affixes (e.g. Heretic's Veil's
        // "Socketed Gems are Supported by Level 22 Blasphemy").
        //
        // Mirrors CalcSetup.lua:1468–1491: for groups without group.source,
        // query env.modDB:List(groupCfg, "ExtraSupport") and add each to the
        // group's support list.
        //
        // The ExtraSupport mods in the mod_db carry a SocketedIn tag (added by
        // add_item_mods() in setup.rs), so querying with cfg.slot_name = group.slot
        // correctly filters to only the supports from the item in that slot.
        //
        // Lua value format: { skillId = "SupportBlasphemy", level = 22 }
        // Rust value format: ModValue::String("Blasphemy:22") (display name, not skill ID)
        if group.source.is_none() {
            let cfg = SkillCfg {
                slot_name: if group.slot.is_empty() {
                    None
                } else {
                    Some(group.slot.clone())
                },
                ..Default::default()
            };
            let extra_supports =
                env.player
                    .mod_db
                    .list("ExtraSupport", Some(&cfg), &env.player.output);
            for m in extra_supports {
                if let ModValue::String(val) = &m.value {
                    // Parse "DisplayName:Level" (e.g. "Blasphemy:22")
                    let (display_name, level) = parse_extra_support_value(val);
                    // Look up gem by display_name (exact) or "DisplayName Support"
                    let skill_id = lookup_gem_id_by_display_name(&env.data.gems, &display_name);
                    if let Some(skill_id) = skill_id {
                        let support_effect = SupportEffect {
                            skill_id,
                            level,
                            quality: 0,
                            gem_data: None,
                        };
                        add_best_support(support_effect, &mut support_lists[group_idx]);
                    }
                }
            }
        }
    }

    // TODO: crossLinkedSupportGroups (Lua:1631–1649)
    // In POB, env.crossLinkedSupportGroups is populated during item processing
    // (CalcSetup.lua items pass) when items share a "linked slot" mechanic (e.g.
    // some unique items that link two equipment slots so that supports in one slot
    // apply to skills in the other). The structure is:
    //   crossLinkedSupportGroups[supportSlot] = [supportedSlot1, supportedSlot2, ...]
    // meaning: supports from `supportSlot` apply to skills socketed in `supportedSlotN`.
    // When a skill's slot is listed as a crossLinkedSupportedSlot, all support lists
    // for `supportSlot` are merged into that skill's appliedSupportList.
    //
    // This is NOT yet populated in the Rust CalcEnv.  Cross-linked slot support
    // merging is therefore skipped here.  To implement, add a
    //   cross_linked_support_groups: HashMap<String, Vec<String>>
    // field to CalcEnv and populate it during add_item_mods() / setup, then
    // iterate it the same way we iterate slot_support_lists below.

    // ── Phase 5: Active skill creation ─────────────────────────────────────
    let mut socket_group_skill_lists: Vec<Vec<usize>> = vec![Vec::new(); skill_set.skills.len()];

    for (group_idx, group) in skill_set.skills.iter().enumerate() {
        let slot_enabled = true; // TODO: weapon set
        let is_main = group_idx == main_socket_group_idx;
        let is_active = is_main || (group.enabled && slot_enabled);
        if !is_active {
            continue;
        }

        for gem in &group.gems {
            if !gem.enabled {
                continue;
            }

            // Resolve is_support
            let is_support = if let Some(gd) = lookup_gem(&env.data.gems, &gem.skill_id) {
                gd.is_support
            } else {
                gem.is_support
            };
            if is_support {
                continue;
            }

            let gem_data = lookup_gem(&env.data.gems, &gem.skill_id);

            // Lua: grantedEffectList = gemInstance.gemData.grantedEffectList or { gemInstance.grantedEffect }
            // A Vaal gem has two effects: [1]=Vaal, [2]=base (non-Vaal).
            // For non-Vaal gems there is only [1]=sole effect.
            // In our data model, a Vaal gem just has a different skill_id (e.g. "Vaal Fireball"),
            // so there is only 1 granted effect per gem entry.
            // The hasGlobalEffect / enableGlobal1 / enableGlobal2 logic:
            //   index 1 uses gem.enable_global1 (default true)
            //   index 2 uses gem.enable_global2 (false by default for Vaal base)
            // We treat all non-support gems as a single granted effect here.
            // (Full Vaal gem multi-effect handling would require data changes.)
            let use_this_effect = gem.enable_global1;
            if !use_this_effect {
                continue;
            }

            // Collect the applied support list for this active skill.
            // Mirrors CalcSetup.lua:1605–1650.
            //
            // Lua:1606: if not group.noSupports then
            //   appliedSupportList = copyTable(supportLists[group] or supportLists[slotName][group])
            //   if group.source then
            //     for _, supportGroup in pairs(supportLists[slotName]) do
            //       for _, supportEffect in ipairs(supportGroup) do
            //         addBestSupport(supportEffect, appliedSupportList, env.mode)
            //
            // For item-granted skills (group.source.is_some()), merge supports
            // from ALL groups in the same slot into the applied support list.
            // This is how Blasphemy (socketed in Heretic's Veil) gets applied
            // to Temporal Chains and Enfeeble that are also granted by that item.
            let applied_support_list = if group.no_supports {
                Vec::new()
            } else if group.source.is_some() && !group.slot.is_empty() {
                // Item-granted skill: start with the group's own supports (usually empty
                // for item-granted groups), then merge ALL support lists for this slot.
                // Lua:1607: appliedSupportList = copyTable(supportLists[group] or supportLists[slotName][group])
                let mut merged = support_lists[group_idx].clone();

                // Lua:1612–1628: if supportLists[slotName] then
                //   for _, supportGroup in pairs(supportLists[slotName]) do
                //     for _, supportEffect in ipairs(supportGroup) do
                //       addBestSupport(supportEffect, appliedSupportList, env.mode)
                if let Some(slot_group_indices) = slot_support_lists.get(&group.slot) {
                    for &other_group_idx in slot_group_indices {
                        for support_effect in support_lists[other_group_idx].iter() {
                            add_best_support(support_effect.clone(), &mut merged);
                        }
                    }
                }
                merged
            } else {
                support_lists[group_idx].clone()
            };

            let slot_name = if !group.slot.is_empty() {
                Some(group.slot.clone())
            } else {
                None
            };

            // Normalize the skill ID to resolve legacy renames (e.g. "AngerAura" → "Anger").
            // This ensures the ActiveSkill's skill_id matches the gems data key.
            let normalized_id = normalize_skill_id(&gem.skill_id);
            let active_skill = create_active_skill(
                normalized_id,
                gem.level,
                gem.quality,
                gem_data,
                applied_support_list,
                group.no_supports,
                slot_name,
                &env.data.gems,
            );

            let skill_idx = env.player.active_skill_list.len();
            env.player.active_skill_list.push(active_skill);
            socket_group_skill_lists[group_idx].push(skill_idx);
        }
    }

    // ── Select main skill from the main socket group ────────────────────────
    // Lua: activeSkillIndex = m_min(#socketGroupSkillList, group.mainActiveSkill or 1)
    // In Rust: main_active_skill is already 0-based clamped.
    let main_group_skill_list = &socket_group_skill_lists
        [main_socket_group_idx.min(socket_group_skill_lists.len().saturating_sub(1))];

    if !main_group_skill_list.is_empty() {
        let main_active_skill_idx = build
            .skill_sets
            .get(skill_set_idx)
            .and_then(|ss| ss.skills.get(main_socket_group_idx))
            .map(|g| g.main_active_skill)
            .unwrap_or(0);

        let clamped_idx = main_active_skill_idx.min(main_group_skill_list.len() - 1);
        let skill_list_idx = main_group_skill_list[clamped_idx];
        // Move the skill out of active_skill_list into main_skill.
        // We need to clone because active_skill_list is Vec<ActiveSkill>.
        // To avoid cloning (ActiveSkill is not Clone due to ModDb), we swap:
        // take the skill out and put a placeholder back.
        // Actually, we can leave the skill in active_skill_list and just copy
        // a reference.  But main_skill is Option<ActiveSkill> (owned).
        // The Lua keeps mainSkill as a reference into activeSkillList.
        // We'll maintain parity by keeping the skill ONLY in active_skill_list
        // and having main_skill be an index, but that requires API changes.
        //
        // For now: clone the skill into main_skill (the skill stays in
        // active_skill_list too, as Lua does).
        // This works because we only need main_skill for downstream reads.
        if let Some(skill) = env.player.active_skill_list.get(skill_list_idx) {
            // We can't clone ActiveSkill (ModDb doesn't Clone), so we move and replace.
            // Use swap-remove and re-insert at end:
            let mut cloned = rebuild_active_skill_from_gem(&env.data.gems, skill);
            build_active_skill_mod_list_with_gems(
                &mut cloned,
                env.mode_buffs,
                env.mode_combat,
                env.mode_effective,
                Some(&env.data.gems),
            );
            env.player.main_skill = Some(cloned);
        }
    }

    // ── Build mod lists for all active skills ───────────────────────────────
    // (CalcSetup.lua:1756–1759) — build_active_skill_mod_list for every skill
    for skill in &mut env.player.active_skill_list {
        build_active_skill_mod_list_with_gems(
            skill,
            env.mode_buffs,
            env.mode_combat,
            env.mode_effective,
            Some(&env.data.gems),
        );
    }

    // ── Fallback: if still no main skill, create default Melee skill ────────
    if env.player.main_skill.is_none() {
        build_fallback_main_skill(env);
    }

    // ── Handle summoner conditions ─────────────────────────────────────────
    if let Some(skill) = env.player.main_skill.as_ref() {
        let skill_id = skill.skill_id.clone();
        if KNOWN_SUMMONER_SKILLS.contains(skill_id.as_str()) {
            env.player.mod_db.set_condition("Summoner", true);
            let count = match skill_id.as_str() {
                "Raise Zombie" => 6.0,
                "Raise Spectre" => 1.0,
                "Summon Skeleton" | "Summon Raging Spirit" => 5.0,
                _ => 1.0,
            };
            env.player.set_output("MinionCount", count);
        }
    }

    // Set conditions on the player mod_db based on the active skill and weapon data
    set_skill_conditions(env);
}

/// Rebuild (re-create) an ActiveSkill from gem data for the main_skill slot.
/// This is needed because we can't clone ActiveSkill (ModDb doesn't impl Clone).
/// We reconstruct the skill from scratch using the same parameters.
fn rebuild_active_skill_from_gem(gems: &GemsMap, skill: &ActiveSkill) -> ActiveSkill {
    let gem_data = lookup_gem(gems, &skill.skill_id);
    create_active_skill(
        skill.skill_id.clone(),
        skill.level,
        skill.quality,
        gem_data,
        skill.support_list.clone(),
        skill.no_supports,
        skill.slot_name.clone(),
        gems,
    )
}

/// Build the default "Melee" fallback skill when no active skill is found.
/// Mirrors CalcSetup.lua:1744–1754.
fn build_fallback_main_skill(env: &mut CalcEnv) {
    let skill_id = "Melee".to_string();
    let gem_data = lookup_gem(&env.data.gems, &skill_id);

    let mut skill = create_active_skill(
        skill_id,
        1,
        0,
        gem_data,
        Vec::new(),
        false,
        None,
        &env.data.gems,
    );
    build_active_skill_mod_list_with_gems(
        &mut skill,
        env.mode_buffs,
        env.mode_combat,
        env.mode_effective,
        Some(&env.data.gems),
    );

    // Fallback Melee: treat as physical melee attack
    skill.is_attack = true;
    skill.is_melee = true;
    skill.skill_flags.insert("attack".to_string(), true);
    skill.skill_flags.insert("melee".to_string(), true);
    skill.skill_flags.insert("hit".to_string(), true);

    env.player.active_skill_list.push(skill);
    let last_idx = env.player.active_skill_list.len() - 1;
    // Move into main_skill (can't clone, rebuild instead)
    let mut main =
        rebuild_active_skill_from_gem(&env.data.gems, &env.player.active_skill_list[last_idx]);
    main.is_attack = true;
    main.is_melee = true;
    main.skill_flags.insert("attack".to_string(), true);
    main.skill_flags.insert("melee".to_string(), true);
    main.skill_flags.insert("hit".to_string(), true);
    build_active_skill_mod_list_with_gems(
        &mut main,
        env.mode_buffs,
        env.mode_combat,
        env.mode_effective,
        Some(&env.data.gems),
    );
    env.player.main_skill = Some(main);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build::parse_xml, calc::setup::init_env, data::GameData};
    use std::sync::Arc;

    fn make_data() -> Arc<GameData> {
        Arc::new(GameData::from_json(crate::tests::stub_game_data_json()).unwrap())
    }

    /// Build GameData from a custom gems JSON string merged with the stub misc data.
    fn make_data_with_gems(gems_json: &str) -> Arc<GameData> {
        let json = format!(
            r#"{{
                "gems": {gems_json},
                "misc": {{
                    "game_constants": {{
                        "base_maximum_all_resistances_%": 75,
                        "maximum_block_%": 75,
                        "base_maximum_spell_block_%": 75,
                        "max_power_charges": 3,
                        "max_frenzy_charges": 3,
                        "max_endurance_charges": 3,
                        "maximum_life_leech_rate_%_per_minute": 20,
                        "maximum_mana_leech_rate_%_per_minute": 20,
                        "maximum_life_leech_amount_per_leech_%_max_life": 10,
                        "maximum_mana_leech_amount_per_leech_%_max_mana": 10,
                        "maximum_energy_shield_leech_amount_per_leech_%_max_energy_shield": 10,
                        "base_number_of_totems_allowed": 1,
                        "impaled_debuff_number_of_reflected_hits": 8,
                        "soul_eater_maximum_stacks": 40,
                        "maximum_righteous_charges": 10,
                        "maximum_blood_scythe_charges": 8
                    }},
                    "character_constants": {{"life_per_str": 0.5}},
                    "monster_life_table": [],
                    "monster_damage_table": [],
                    "monster_evasion_table": [],
                    "monster_accuracy_table": [],
                    "monster_ally_life_table": [],
                    "monster_ally_damage_table": [],
                    "monster_ailment_threshold_table": [],
                    "monster_phys_conversion_multi_table": []
                }}
            }}"#
        );
        Arc::new(GameData::from_json(&json).unwrap())
    }

    const CLEAVE_BUILD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Cleave" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;

    #[test]
    fn resolves_main_skill_from_build() {
        let build = parse_xml(CLEAVE_BUILD).unwrap();
        let mut env = init_env(&build, make_data()).unwrap();
        run(&mut env, &build);
        assert!(env.player.main_skill.is_some(), "main_skill should be set");
        assert_eq!(env.player.main_skill.as_ref().unwrap().skill_id, "Cleave");
    }

    #[test]
    fn attack_skill_sets_is_attack_true() {
        let build = parse_xml(CLEAVE_BUILD).unwrap();
        let mut env = init_env(&build, make_data()).unwrap();
        run(&mut env, &build);
        let skill = env.player.main_skill.as_ref().unwrap();
        assert!(skill.is_attack, "Cleave should be an attack");
        assert!(!skill.is_spell, "Cleave should not be a spell");
    }

    #[test]
    fn fireball_level_20_loads_fire_damage() {
        let data_dir = std::env::var("DATA_DIR").unwrap_or_default();
        if data_dir.is_empty() {
            let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Witch" ascendClassName="None"/>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Fireball" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="3" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
            let build = crate::build::parse_xml(xml).unwrap();
            let data = make_data();
            let mut env = crate::calc::setup::init_env(&build, data).unwrap();
            run(&mut env, &build);
            let skill = env.player.main_skill.unwrap();
            assert!(skill.base_damage.is_empty(), "stub data has no gem levels");
            return;
        }

        let gems_str = std::fs::read_to_string(format!("{data_dir}/gems.json")).unwrap();
        let misc_str = std::fs::read_to_string(format!("{data_dir}/misc.json")).unwrap();
        let combined = format!(r#"{{"gems": {gems_str}, "misc": {misc_str}}}"#);
        let data = std::sync::Arc::new(crate::data::GameData::from_json(&combined).unwrap());

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Witch" ascendClassName="None"/>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Fireball" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="3" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = crate::build::parse_xml(xml).unwrap();
        let mut env = crate::calc::setup::init_env(&build, data).unwrap();
        run(&mut env, &build);
        let skill = env.player.main_skill.unwrap();
        let fire = skill.base_damage.get("Fire").copied();
        assert!(fire.is_some(), "Fireball L20 should have Fire base damage");
        let (min, max) = fire.unwrap();
        assert!(
            min > 0.0 && max > min,
            "Fire damage should be min={min} < max={max}"
        );
    }

    #[test]
    fn spell_skill_sets_is_spell_true() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Witch" ascendClassName="None"/>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Fireball" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="3" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let mut env = init_env(&build, make_data()).unwrap();
        run(&mut env, &build);
        let skill = env.player.main_skill.as_ref().unwrap();
        assert!(skill.is_spell, "Fireball should be a spell");
        assert!(!skill.is_attack, "Fireball should not be an attack");
    }

    // ── Task 9: Data-driven classification tests ────────────────────────────

    #[test]
    fn data_driven_spell_classification() {
        let gems_json = r#"{
            "MagicMissile": {
                "id": "MagicMissile",
                "display_name": "Magic Missile",
                "is_support": false,
                "skill_types": ["Spell", "Projectile"],
                "base_flags": ["spell", "projectile"],
                "levels": [{"level": 20, "level_requirement": 70}]
            }
        }"#;
        let data = make_data_with_gems(gems_json);

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Witch" ascendClassName="None"/>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="MagicMissile" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="3" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;

        let build = parse_xml(xml).unwrap();
        let mut env = init_env(&build, data).unwrap();
        run(&mut env, &build);

        let skill = env.player.main_skill.as_ref().unwrap();
        assert!(
            skill.is_spell,
            "MagicMissile should be classified as spell from gem data"
        );
        assert!(!skill.is_attack, "MagicMissile should not be an attack");
        assert!(
            skill.skill_types.contains(&"Spell".to_string()),
            "skill_types should contain Spell"
        );
        assert!(
            skill.skill_flags.contains_key("spell"),
            "skill_flags should contain 'spell'"
        );
    }

    #[test]
    fn data_driven_attack_melee_classification() {
        let gems_json = r#"{
            "Cleave": {
                "id": "Cleave",
                "display_name": "Cleave",
                "is_support": false,
                "skill_types": ["Attack", "Melee", "Area"],
                "base_flags": ["attack", "melee", "area"],
                "levels": [{"level": 20, "level_requirement": 70}]
            }
        }"#;
        let data = make_data_with_gems(gems_json);
        let build = parse_xml(CLEAVE_BUILD).unwrap();
        let mut env = init_env(&build, data).unwrap();
        run(&mut env, &build);

        let skill = env.player.main_skill.as_ref().unwrap();
        assert!(skill.is_attack, "Cleave should be attack from gem data");
        assert!(skill.is_melee, "Cleave should be melee from gem data");
        assert!(!skill.is_spell, "Cleave should not be a spell");
    }

    // ── Task 10: Support gem matching tests ─────────────────────────────────

    #[test]
    fn support_gems_matched_to_active_skill() {
        let gems_json = r#"{
            "Cleave": {
                "id": "Cleave",
                "display_name": "Cleave",
                "is_support": false,
                "skill_types": ["Attack", "Melee", "Area"],
                "base_flags": ["attack", "melee", "area"],
                "levels": [{"level": 20, "level_requirement": 70}]
            },
            "SupportMeleeSplash": {
                "id": "SupportMeleeSplash",
                "display_name": "Melee Splash Support",
                "is_support": true,
                "skill_types": [],
                "base_flags": [],
                "require_skill_types": ["Attack", "Melee"],
                "exclude_skill_types": [],
                "levels": [{"level": 20, "level_requirement": 70}]
            }
        }"#;
        let data = make_data_with_gems(gems_json);

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Cleave" level="20" quality="0" enabled="true"/>
        <Gem skillId="SupportMeleeSplash" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;

        let build = parse_xml(xml).unwrap();
        let mut env = init_env(&build, data).unwrap();
        run(&mut env, &build);

        let skill = env.player.main_skill.as_ref().unwrap();
        assert_eq!(skill.skill_id, "Cleave");
        assert_eq!(skill.support_list.len(), 1, "should have 1 support");
        assert_eq!(skill.support_list[0].skill_id, "SupportMeleeSplash");
    }

    #[test]
    fn incompatible_support_not_matched() {
        let gems_json = r#"{
            "Cleave": {
                "id": "Cleave",
                "display_name": "Cleave",
                "is_support": false,
                "skill_types": ["Attack", "Melee", "Area"],
                "base_flags": ["attack", "melee", "area"],
                "levels": [{"level": 20, "level_requirement": 70}]
            },
            "SupportSpellEcho": {
                "id": "SupportSpellEcho",
                "display_name": "Spell Echo Support",
                "is_support": true,
                "skill_types": [],
                "base_flags": [],
                "require_skill_types": ["Spell"],
                "exclude_skill_types": [],
                "levels": [{"level": 20, "level_requirement": 70}]
            }
        }"#;
        let data = make_data_with_gems(gems_json);

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Cleave" level="20" quality="0" enabled="true"/>
        <Gem skillId="SupportSpellEcho" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;

        let build = parse_xml(xml).unwrap();
        let mut env = init_env(&build, data).unwrap();
        run(&mut env, &build);

        let skill = env.player.main_skill.as_ref().unwrap();
        assert_eq!(skill.skill_id, "Cleave");
        assert!(
            skill.support_list.is_empty(),
            "SupportSpellEcho should not support Cleave (attack/melee)"
        );
    }

    #[test]
    fn support_with_exclude_types_blocks_matching() {
        let gems_json = r#"{
            "Cleave": {
                "id": "Cleave",
                "display_name": "Cleave",
                "is_support": false,
                "skill_types": ["Attack", "Melee", "Area"],
                "base_flags": ["attack", "melee", "area"],
                "levels": [{"level": 20, "level_requirement": 70}]
            },
            "SupportRangedAttack": {
                "id": "SupportRangedAttack",
                "display_name": "Ranged Attack Support",
                "is_support": true,
                "skill_types": [],
                "base_flags": [],
                "require_skill_types": ["Attack"],
                "exclude_skill_types": ["Melee"],
                "levels": [{"level": 20, "level_requirement": 70}]
            }
        }"#;
        let data = make_data_with_gems(gems_json);

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Cleave" level="20" quality="0" enabled="true"/>
        <Gem skillId="SupportRangedAttack" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;

        let build = parse_xml(xml).unwrap();
        let mut env = init_env(&build, data).unwrap();
        run(&mut env, &build);

        let skill = env.player.main_skill.as_ref().unwrap();
        assert!(
            skill.support_list.is_empty(),
            "SupportRangedAttack should not support Cleave (excludes Melee)"
        );
    }

    #[test]
    fn support_with_no_requirements_matches_any_skill() {
        let gems_json = r#"{
            "Cleave": {
                "id": "Cleave",
                "display_name": "Cleave",
                "is_support": false,
                "skill_types": ["Attack", "Melee", "Area"],
                "base_flags": ["attack", "melee", "area"],
                "levels": [{"level": 20, "level_requirement": 70}]
            },
            "SupportAddedFire": {
                "id": "SupportAddedFire",
                "display_name": "Added Fire Damage Support",
                "is_support": true,
                "skill_types": [],
                "base_flags": [],
                "require_skill_types": [],
                "exclude_skill_types": [],
                "levels": [{"level": 20, "level_requirement": 70}]
            }
        }"#;
        let data = make_data_with_gems(gems_json);

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Cleave" level="20" quality="0" enabled="true"/>
        <Gem skillId="SupportAddedFire" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;

        let build = parse_xml(xml).unwrap();
        let mut env = init_env(&build, data).unwrap();
        run(&mut env, &build);

        let skill = env.player.main_skill.as_ref().unwrap();
        assert_eq!(
            skill.support_list.len(),
            1,
            "SupportAddedFire (no requirements) should support any skill"
        );
        assert_eq!(skill.support_list[0].skill_id, "SupportAddedFire");
    }

    #[test]
    fn can_support_fn_unit_tests() {
        use crate::data::gems::GemData;

        let make_support = |require: Vec<&str>, exclude: Vec<&str>| -> GemData {
            GemData {
                id: "test".to_string(),
                display_name: "test".to_string(),
                is_support: true,
                skill_types: vec![],
                levels: vec![],
                color: None,
                cast_time: 0.0,
                base_effectiveness: 0.0,
                incremental_effectiveness: 0.0,
                base_flags: vec![],
                mana_multiplier_at_20: 0.0,
                require_skill_types: require.into_iter().map(|s| s.to_string()).collect(),
                add_skill_types: vec![],
                exclude_skill_types: exclude.into_iter().map(|s| s.to_string()).collect(),
                constant_stats: vec![],
                quality_stats: vec![],
                stats: vec![],
            }
        };

        let attack_melee = vec!["Attack".to_string(), "Melee".to_string()];
        let spell = vec!["Spell".to_string()];

        let s = make_support(vec![], vec![]);
        assert!(can_support(&s, &attack_melee));
        assert!(can_support(&s, &spell));

        let s = make_support(vec!["Attack"], vec![]);
        assert!(can_support(&s, &attack_melee));
        assert!(!can_support(&s, &spell));

        let s = make_support(vec!["Spell"], vec![]);
        assert!(!can_support(&s, &attack_melee));
        assert!(can_support(&s, &spell));

        let s = make_support(vec!["Attack"], vec!["Melee"]);
        assert!(!can_support(&s, &attack_melee));

        let s = make_support(vec!["attack"], vec![]);
        assert!(can_support(&s, &attack_melee));
    }

    #[test]
    fn active_skill_list_is_populated() {
        let gems_json = r#"{
            "Cleave": {
                "id": "Cleave",
                "display_name": "Cleave",
                "is_support": false,
                "skill_types": ["Attack", "Melee", "Area"],
                "base_flags": ["attack", "melee", "area"],
                "levels": [{"level": 20, "level_requirement": 70}]
            }
        }"#;
        let data = make_data_with_gems(gems_json);
        let build = parse_xml(CLEAVE_BUILD).unwrap();
        let mut env = init_env(&build, data).unwrap();
        run(&mut env, &build);

        assert!(
            !env.player.active_skill_list.is_empty(),
            "active_skill_list should be populated"
        );
        assert_eq!(
            env.player.active_skill_list[0].skill_id, "Cleave",
            "first active skill should be Cleave"
        );
    }

    #[test]
    fn skill_cfg_is_built_for_main_skill() {
        let gems_json = r#"{
            "Cleave": {
                "id": "Cleave",
                "display_name": "Cleave",
                "is_support": false,
                "skill_types": ["Attack", "Melee", "Area"],
                "base_flags": ["attack", "melee", "area"],
                "levels": [{"level": 20, "level_requirement": 70}]
            }
        }"#;
        let data = make_data_with_gems(gems_json);
        let build = parse_xml(CLEAVE_BUILD).unwrap();
        let mut env = init_env(&build, data).unwrap();
        run(&mut env, &build);

        let skill = env.player.main_skill.as_ref().unwrap();
        assert!(
            skill.skill_cfg.is_some(),
            "skill_cfg should be built by build_active_skill_mod_list"
        );
        let cfg = skill.skill_cfg.as_ref().unwrap();
        assert!(
            cfg.flags.contains(ModFlags::ATTACK),
            "attack skill should have ATTACK flag"
        );
    }

    #[test]
    fn non_attack_skill_gets_cast_flag() {
        let gems_json = r#"{
            "Fireball": {
                "id": "Fireball",
                "display_name": "Fireball",
                "is_support": false,
                "skill_types": ["Spell", "Projectile", "Area"],
                "base_flags": ["spell", "projectile", "area"],
                "levels": [{"level": 20, "level_requirement": 70}]
            }
        }"#;
        let data = make_data_with_gems(gems_json);
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Witch" ascendClassName="None"/>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Fireball" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="3" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let mut env = init_env(&build, data).unwrap();
        run(&mut env, &build);
        let skill = env.player.main_skill.as_ref().unwrap();
        let cfg = skill.skill_cfg.as_ref().unwrap();
        assert!(
            cfg.flags.contains(ModFlags::CAST),
            "non-attack skill (Fireball) should have CAST flag"
        );
    }
}
