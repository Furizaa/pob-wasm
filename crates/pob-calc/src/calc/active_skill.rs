// Resolve the main active skill from build.skill_sets and set skill flags.
//
// Reference: third-party/PathOfBuilding/src/Modules/CalcActiveSkill.lua
// Full implementation resolves the main active skill from build.skill_sets,
// builds skillCfg (flags, keyword flags), and sets conditions like UsingAttack,
// UsingSpell, IsMainSkill, etc.

use std::sync::LazyLock;

use super::env::CalcEnv;

// Heuristic spell list — will be replaced by gem data in Task 4.
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

pub fn run(env: &mut CalcEnv, build: &crate::build::Build) {
    use crate::build::types::ActiveSkill;
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

    // Find the active (non-support) gems
    let active_gems: Vec<_> = skill_group
        .gems
        .iter()
        .filter(|g| g.enabled && !g.is_support)
        .collect();
    let active_gem_idx = skill_group.main_active_skill;
    let Some(active_gem) = active_gems
        .get(active_gem_idx)
        .or_else(|| active_gems.first())
    else {
        return;
    };

    let skill_id = active_gem.skill_id.clone();

    // Classify skill type by name (heuristic until gem data drives this)
    let is_spell = KNOWN_SPELLS.contains(skill_id.as_str());
    let is_attack = !is_spell;
    let is_melee = is_attack && !KNOWN_RANGED_ATTACKS.contains(skill_id.as_str());

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

    // Set conditions on the player mod db
    env.player.mod_db.set_condition("IsMainSkill", true);
    if is_attack {
        env.player.mod_db.set_condition("UsingAttack", true);
    }
    if is_spell {
        env.player.mod_db.set_condition("UsingSpell", true);
    }
    if is_melee {
        env.player.mod_db.set_condition("UsingMelee", true);
    }

    // Default timing — overridden by gem level data below
    let mut cast_time = if is_spell { 0.7 } else { 0.0 };
    let mut attack_speed_base = if is_attack { 1.5 } else { 0.0 };
    let mut base_crit_chance = if is_spell { 0.06 } else { 0.05 };
    let mut base_damage: std::collections::HashMap<String, (f64, f64)> =
        std::collections::HashMap::new();

    // Populate base_damage and timing from gem level data.
    // gems.json keys are lowercase; skillId in the build XML may be mixed-case.
    let gem_key = skill_id.to_lowercase();
    // Also try replacing spaces with underscores (e.g. "Heavy Strike" -> "heavy_strike")
    let gem_key_underscored = gem_key.replace(' ', "_");
    let gem_data = env
        .data
        .gems
        .get(&gem_key)
        .or_else(|| env.data.gems.get(&gem_key_underscored));

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

    env.player.main_skill = Some(ActiveSkill {
        skill_id,
        level: active_gem.level,
        skill_mod_db: ModDb::new(),
        is_attack,
        is_spell,
        is_melee,
        can_crit: true,
        base_crit_chance,
        base_damage,
        attack_speed_base,
        cast_time,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build::parse_xml, calc::setup::init_env, data::GameData};
    use std::sync::Arc;

    fn make_data() -> Arc<GameData> {
        Arc::new(GameData::from_json(crate::tests::stub_game_data_json()).unwrap())
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
        use std::path::PathBuf;

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
}
