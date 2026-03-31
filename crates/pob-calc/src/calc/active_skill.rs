// Resolve the main active skill from build.skill_sets and set skill flags.
//
// Reference: third-party/PathOfBuilding/src/Modules/CalcActiveSkill.lua
// Full implementation resolves the main active skill from build.skill_sets,
// builds skillCfg (flags, keyword flags), and sets conditions like UsingAttack,
// UsingSpell, IsMainSkill, etc.

use std::sync::LazyLock;

use super::env::CalcEnv;
use crate::build::types::ActiveSkill;
use crate::data::gems::{GemData, GemsMap};
use crate::mod_db::types::{KeywordFlags, ModFlags, SkillCfg};

// ── Heuristic fallback lists (used when gem data is unavailable) ────────────

// Kinetic Blast is a ranged ATTACK, not a spell — excluded from this list.
static KNOWN_SPELLS: LazyLock<std::collections::HashSet<&'static str>> = LazyLock::new(|| {
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

// Ranged attacks that must not set UsingMelee.
static KNOWN_RANGED_ATTACKS: LazyLock<std::collections::HashSet<&'static str>> =
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
/// 2. Lowercase match
/// 3. Lowercase with spaces replaced by underscores
fn lookup_gem<'a>(gems: &'a GemsMap, skill_id: &str) -> Option<&'a GemData> {
    gems.get(skill_id).or_else(|| {
        let lower = skill_id.to_lowercase();
        gems.get(&lower)
            .or_else(|| gems.get(&lower.replace(' ', "_")))
    })
}

// ── Support gem matching ────────────────────────────────────────────────────

/// Determine if a support gem can support an active skill based on skill type
/// requirements and exclusions.
fn can_support(support_data: &GemData, active_skill_types: &[String]) -> bool {
    // Check require_skill_types: if non-empty, the active skill must have at
    // least one matching type
    if !support_data.require_skill_types.is_empty() {
        let matches = support_data.require_skill_types.iter().any(|req| {
            active_skill_types
                .iter()
                .any(|ast| ast.eq_ignore_ascii_case(req))
        });
        if !matches {
            return false;
        }
    }
    // Check exclude_skill_types: if non-empty, the active skill must have NONE
    // of those types
    if !support_data.exclude_skill_types.is_empty() {
        let excluded = support_data.exclude_skill_types.iter().any(|exc| {
            active_skill_types
                .iter()
                .any(|ast| ast.eq_ignore_ascii_case(exc))
        });
        if excluded {
            return false;
        }
    }
    true
}

/// Build a SkillCfg from the active skill's flags and metadata.
/// This is the canonical way to construct a SkillCfg for ModDb queries.
pub fn build_skill_cfg(skill: &ActiveSkill) -> SkillCfg {
    let mut flags = ModFlags::NONE;
    if skill.is_attack {
        flags = flags | ModFlags::ATTACK;
    }
    if skill.is_spell {
        flags = flags | ModFlags::SPELL;
    }
    flags = flags | ModFlags::HIT;
    if skill.is_melee {
        flags = flags | ModFlags::MELEE;
    }
    // Check skill_flags for additional flag types
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
    // (These are set based on what weapon the character is using, which is
    // relevant for mods like "while using a Sword")
    if let Some(wd) = env.player.weapon_data1.as_ref() {
        // Infer weapon type from attack rate and base data heuristics
        // In a full implementation this would come from item type data;
        // for now we use the presence of weapon data to set basic conditions
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

pub fn run(env: &mut CalcEnv, build: &crate::build::Build) {
    use crate::build::types::SupportEffect;
    use crate::mod_db::ModDb;

    // Resolve the active skill set and socket group
    let skill_set_idx = build.active_skill_set;
    let socket_group_idx = build.main_socket_group;

    let Some(skill_set) = build.skill_sets.get(skill_set_idx) else {
        return;
    };
    let Some(skill_group) = skill_set.skills.get(socket_group_idx) else {
        return;
    };

    // ── Resolve is_support from gem data ────────────────────────────────────
    // The XML parser doesn't know which gems are supports; resolve from gem data.
    // Build a vec of (gem, resolved_is_support) for all enabled gems.
    let resolved_gems: Vec<_> = skill_group
        .gems
        .iter()
        .filter(|g| g.enabled)
        .map(|g| {
            let is_support = if let Some(gd) = lookup_gem(&env.data.gems, &g.skill_id) {
                gd.is_support
            } else {
                g.is_support
            };
            (g, is_support)
        })
        .collect();

    // Find the active (non-support) gems
    let active_gems: Vec<_> = resolved_gems
        .iter()
        .filter(|(_, is_support)| !is_support)
        .map(|(g, _)| *g)
        .collect();
    let active_gem_idx = skill_group.main_active_skill;
    let Some(active_gem) = active_gems
        .get(active_gem_idx)
        .or_else(|| active_gems.first())
    else {
        return;
    };

    let skill_id = active_gem.skill_id.clone();

    // ── Gem data lookup ─────────────────────────────────────────────────────
    let gem_data = lookup_gem(&env.data.gems, &skill_id);

    // ── Data-driven skill classification ────────────────────────────────────
    // Use gem data's base_flags and skill_types when available, falling back
    // to the heuristic lists for unknown gems.
    let (is_attack, is_spell, is_melee) = if let Some(gd) = gem_data {
        let has_flag = |f: &str| {
            gd.base_flags.iter().any(|b| b.eq_ignore_ascii_case(f))
                || gd.skill_types.iter().any(|t| t.eq_ignore_ascii_case(f))
        };
        let is_attack = has_flag("attack");
        let is_spell = has_flag("spell");
        let is_melee =
            has_flag("melee") || (is_attack && !has_flag("projectile") && !has_flag("bow"));
        (is_attack, is_spell, is_melee)
    } else {
        // Fallback to heuristic for unknown gems
        let is_spell = KNOWN_SPELLS.contains(skill_id.as_str());
        let is_attack = !is_spell;
        let is_melee = is_attack && !KNOWN_RANGED_ATTACKS.contains(skill_id.as_str());
        (is_attack, is_spell, is_melee)
    };

    // ── Populate skill_types and skill_flags from gem data ──────────────────
    let (skill_types, skill_flags) = if let Some(gd) = gem_data {
        let types = gd.skill_types.clone();
        let flags: std::collections::HashMap<String, bool> =
            gd.base_flags.iter().map(|f| (f.clone(), true)).collect();
        (types, flags)
    } else {
        (Vec::new(), std::collections::HashMap::new())
    };

    let is_summoner = KNOWN_SUMMONER_SKILLS.contains(skill_id.as_str());
    if is_summoner {
        env.player.mod_db.set_condition("Summoner", true);
        // Set MinionCount based on gem ID
        let count = match skill_id.as_str() {
            "Raise Zombie" => 6.0,
            "Raise Spectre" => 1.0,
            "Summon Skeleton" | "Summon Raging Spirit" => 5.0,
            _ => 1.0,
        };
        env.player.set_output("MinionCount", count);
    }

    // Default timing — overridden by gem level data below
    let mut cast_time = if is_spell { 0.7 } else { 0.0 };
    let mut attack_speed_base = if is_attack { 1.5 } else { 0.0 };
    let mut base_crit_chance = if is_spell { 0.06 } else { 0.05 };
    let mut base_damage: std::collections::HashMap<String, (f64, f64)> =
        std::collections::HashMap::new();

    // Populate base_damage and timing from gem level data.
    if let Some(gem_data) = gem_data {
        // Find by level field instead of positional index
        if let Some(level_data) = gem_data.levels.iter().find(|l| l.level == active_gem.level) {
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
        }
    }

    // ── Build support list ──────────────────────────────────────────────────
    // Iterate other gems in the socket group, find supports that can support
    // this active skill based on skill type requirements.
    let support_list: Vec<SupportEffect> = resolved_gems
        .iter()
        .filter(|(g, is_support)| *is_support && g.skill_id != active_gem.skill_id)
        .filter_map(|(g, _)| {
            let support_gd = lookup_gem(&env.data.gems, &g.skill_id)?;
            if can_support(support_gd, &skill_types) {
                Some(SupportEffect {
                    skill_id: g.skill_id.clone(),
                    level: g.level,
                    quality: g.quality,
                    gem_data: Some(support_gd.id.clone()),
                })
            } else {
                None
            }
        })
        .collect();

    env.player.main_skill = Some(ActiveSkill {
        skill_id,
        level: active_gem.level,
        quality: active_gem.quality,
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
        slot_name: None,
        support_list,
        triggered_by: None,
    });

    // Build skill_cfg from the resolved active skill
    if let Some(skill) = env.player.main_skill.as_ref() {
        let cfg = build_skill_cfg(skill);
        // Store the cfg back on the skill
        if let Some(skill_mut) = env.player.main_skill.as_mut() {
            skill_mut.skill_cfg = Some(cfg);
        }
    }

    // Set conditions on the player mod_db based on the active skill and weapon data
    set_skill_conditions(env);
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
        // This test requires that data/gems.json contains a "fireball" entry with level 20 data.
        // It verifies the gem level lookup actually works (not just the struct parsing).
        // Uses the real data directory if available.
        let data_dir = std::env::var("DATA_DIR").unwrap_or_default();
        if data_dir.is_empty() {
            // With stub data, no gem levels exist — just verify no panic
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
            // With stub data, skill resolves but base_damage is empty
            let skill = env.player.main_skill.unwrap();
            assert!(skill.base_damage.is_empty(), "stub data has no gem levels");
            return;
        }

        // With real data: verify the level 20 fire damage loads
        // (build a minimal GameData from real files)
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
        // Create game data with a gem that has base_flags: ["spell"]
        // "MagicMissile" is NOT in the KNOWN_SPELLS list, so the only way it can
        // be classified as a spell is via the gem data.
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
        // Cleave with gem data should be classified as attack + melee
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
        // Cleave (attack, melee) + SupportMeleeSplash (requires Attack, Melee)
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

        // Build XML with both gems in the same socket group.
        // Note: is_support="false" in XML — the code resolves it from gem data.
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
        // Cleave (attack, melee) + SupportSpellEcho (requires Spell)
        // SupportSpellEcho should NOT be matched because Cleave is not a spell.
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
        // A support that excludes "Melee" should not match Cleave
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
        // A support with empty require/exclude should match any active skill
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

        // Helper to build minimal GemData for testing can_support
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

        // No requirements -> matches anything
        let s = make_support(vec![], vec![]);
        assert!(can_support(&s, &attack_melee));
        assert!(can_support(&s, &spell));

        // Requires Attack -> matches attack, not spell
        let s = make_support(vec!["Attack"], vec![]);
        assert!(can_support(&s, &attack_melee));
        assert!(!can_support(&s, &spell));

        // Requires Spell -> matches spell, not attack
        let s = make_support(vec!["Spell"], vec![]);
        assert!(!can_support(&s, &attack_melee));
        assert!(can_support(&s, &spell));

        // Requires Attack, excludes Melee -> not attack+melee
        let s = make_support(vec!["Attack"], vec!["Melee"]);
        assert!(!can_support(&s, &attack_melee));

        // Case insensitive
        let s = make_support(vec!["attack"], vec![]);
        assert!(can_support(&s, &attack_melee));
    }
}
