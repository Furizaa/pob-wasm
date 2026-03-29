// Resolve the main active skill from build.skill_sets and set skill flags.
//
// Reference: third-party/PathOfBuilding/src/Modules/CalcActiveSkill.lua
// Full implementation resolves the main active skill from build.skill_sets,
// builds skillCfg (flags, keyword flags), and sets conditions like UsingAttack,
// UsingSpell, IsMainSkill, etc.

use super::env::CalcEnv;

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
    let known_spells: std::collections::HashSet<&str> = [
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
        "Kinetic Blast",
    ]
    .iter()
    .copied()
    .collect();

    let is_spell = known_spells.contains(skill_id.as_str());
    let is_attack = !is_spell;
    let is_melee = is_attack; // simplified: all non-spell attacks treated as melee

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

    // Default timing — will be overridden by gem level data in Task 4
    let cast_time = if is_spell { 0.7 } else { 0.0 };
    let attack_speed_base = if is_attack { 1.5 } else { 0.0 };
    let base_crit_chance = if is_spell { 0.06 } else { 0.05 };

    env.player.main_skill = Some(ActiveSkill {
        skill_id,
        level: active_gem.level,
        skill_mod_db: ModDb::new(),
        is_attack,
        is_spell,
        is_melee,
        can_crit: true,
        base_crit_chance,
        base_damage: std::collections::HashMap::new(), // populated in Task 4
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
