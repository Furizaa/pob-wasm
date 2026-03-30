use super::env::{CalcEnv, OutputValue};
use crate::build::Build;
use crate::mod_db::types::{KeywordFlags, ModFlags, ModType};
use std::sync::LazyLock;

static KNOWN_TOTEMS: LazyLock<std::collections::HashSet<&'static str>> = LazyLock::new(|| {
    [
        "Shockwave Totem",
        "Searing Bond",
        "Rejuvenation Totem",
        "Decoy Totem",
        "Ancestral Warchief",
        "Ancestral Protector",
        "Ballista Totem",
        "Holy Flame Totem",
        "Siege Ballista",
        "Toxic Rain Totem",
    ]
    .iter()
    .copied()
    .collect()
});

static KNOWN_TRAPS: LazyLock<std::collections::HashSet<&'static str>> = LazyLock::new(|| {
    [
        "Fire Trap",
        "Lightning Trap",
        "Bear Trap",
        "Explosive Trap",
        "Cluster Trap",
        "Conversion Trap",
        "Seismic Trap",
        "Charged Trap",
        "Ice Trap",
        "Chaos Trap",
    ]
    .iter()
    .copied()
    .collect()
});

static KNOWN_MINES: LazyLock<std::collections::HashSet<&'static str>> = LazyLock::new(|| {
    [
        "Blastchain Mine",
        "Remote Mine",
        "High-Impact Mine",
        "Stormblast Mine",
        "Pyroclast Mine",
        "Galvanic Field",
    ]
    .iter()
    .copied()
    .collect()
});

pub fn run(env: &mut CalcEnv, _build: &Build) {
    let Some(skill) = env.player.main_skill.as_ref() else {
        return;
    };
    let skill_id = skill.skill_id.clone();

    if KNOWN_TOTEMS.contains(skill_id.as_str()) {
        calc_totem_dps(env);
    } else if KNOWN_TRAPS.contains(skill_id.as_str()) {
        calc_trap_dps(env, &skill_id);
    } else if KNOWN_MINES.contains(skill_id.as_str()) {
        calc_mine_dps(env);
    }
}

fn get_f64(output: &crate::calc::env::OutputTable, key: &str) -> f64 {
    output
        .get(key)
        .and_then(|v| {
            if let OutputValue::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .unwrap_or(0.0)
}

fn calc_totem_dps(env: &mut CalcEnv) {
    // Active totem limit — default 1 from initModDB, modified by "ActiveTotemLimit" BASE mods
    let active_totems = env
        .player
        .mod_db
        .sum(
            ModType::Base,
            "ActiveTotemLimit",
            ModFlags::NONE,
            KeywordFlags::NONE,
        )
        .max(1.0);
    env.player.set_output("ActiveTotemLimit", active_totems);

    // Totem life: base 100, scaled by inc/more
    let inc_life = env.player.mod_db.sum(
        ModType::Inc,
        "TotemLife",
        ModFlags::NONE,
        KeywordFlags::NONE,
    );
    let more_life = env
        .player
        .mod_db
        .more("TotemLife", ModFlags::NONE, KeywordFlags::NONE);
    let totem_life = (100.0 * (1.0 + inc_life / 100.0) * more_life).round();
    env.player.set_output("TotemLife", totem_life);
    env.player
        .set_output("TotemLifeTotal", totem_life * active_totems);

    // Totem placement time (default 0.6s from initModDB)
    let base_time = env.player.mod_db.sum(
        ModType::Base,
        "TotemPlacementTime",
        ModFlags::NONE,
        KeywordFlags::NONE,
    );
    let place_time = if base_time > 0.0 { base_time } else { 0.6 };
    env.player.set_output("TotemPlacementTime", place_time);

    // Totem DPS = skill TotalDPS * number of active totems
    let skill_dps = get_f64(&env.player.output, "TotalDPS");
    let totem_dps = skill_dps * active_totems;
    env.player.set_output("TotemDPS", totem_dps);
    env.player.set_output("CombinedDPS", totem_dps);
}

fn calc_trap_dps(env: &mut CalcEnv, skill_id: &str) {
    // Trap throw time (default 0.6s from initModDB, modified by TrapThrowingSpeed)
    let base_throw = 0.6_f64;
    let inc_speed = env.player.mod_db.sum(
        ModType::Inc,
        "TrapThrowingSpeed",
        ModFlags::NONE,
        KeywordFlags::NONE,
    );
    let more_speed =
        env.player
            .mod_db
            .more("TrapThrowingSpeed", ModFlags::NONE, KeywordFlags::NONE);
    let throw_time = base_throw / ((1.0 + inc_speed / 100.0) * more_speed).max(0.001);
    env.player.set_output("TrapThrowingTime", throw_time);

    // Trap cooldown — varies by skill. Fire Trap: 4s, Bear Trap: 3s, others: 4s
    let trap_cooldown = match skill_id {
        "Bear Trap" => 3.0,
        _ => 4.0,
    };
    env.player.set_output("TrapCooldown", trap_cooldown);

    // Traps per throw (default 1)
    let traps_per_throw = 1.0_f64;

    // Effective trap usage rate = traps / cooldown
    let skill_dps = get_f64(&env.player.output, "TotalDPS");
    let trap_dps = skill_dps * traps_per_throw / trap_cooldown;
    env.player.set_output("TrapDPS", trap_dps);
    env.player.set_output("CombinedDPS", trap_dps);
}

fn calc_mine_dps(env: &mut CalcEnv) {
    // Mine laying time (default 0.3s from initModDB)
    let base_lay = env.player.mod_db.sum(
        ModType::Base,
        "MineLayingTime",
        ModFlags::NONE,
        KeywordFlags::NONE,
    );
    let lay_time = if base_lay > 0.0 { base_lay } else { 0.3 };
    env.player.set_output("MineLayingTime", lay_time);

    // Detonation time = lay time + 0.25s detonation delay
    let detonation_time = lay_time + 0.25;
    env.player.set_output("MineDetonationTime", detonation_time);

    // Mine DPS = skill_DPS / detonation_time
    let skill_dps = get_f64(&env.player.output, "TotalDPS");
    let mine_dps = skill_dps / detonation_time;
    env.player.set_output("MineDPS", mine_dps);
    env.player.set_output("CombinedDPS", mine_dps);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        build::parse_xml,
        calc::{active_skill, defence, offence, perform, setup::init_env},
        data::GameData,
    };
    use std::sync::Arc;

    fn make_data() -> Arc<GameData> {
        Arc::new(GameData::from_json(crate::tests::stub_game_data_json()).unwrap())
    }

    #[test]
    fn totem_skill_sets_totem_life_total() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Templar" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1">
    <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
      <Gem skillId="Shockwave Totem" level="20" quality="0" enabled="true"/>
    </Skill>
  </SkillSet></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="5" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let data = make_data();
        let mut env = init_env(&build, data).unwrap();
        perform::run(&mut env);
        defence::run(&mut env);
        active_skill::run(&mut env, &build);
        offence::run(&mut env, &build);
        run(&mut env, &build);
        // With stub data "Shockwave Totem" has no gem levels, but triggers::run still
        // detects it as a totem and sets TotemLifeTotal
        let life_total = env.player.output.get("TotemLifeTotal");
        assert!(
            life_total.is_some(),
            "TotemLifeTotal should be set for a totem skill"
        );
    }

    #[test]
    fn trap_skill_sets_trap_cooldown() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Shadow" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1">
    <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
      <Gem skillId="Fire Trap" level="20" quality="0" enabled="true"/>
    </Skill>
  </SkillSet></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="2" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let data = make_data();
        let mut env = init_env(&build, data).unwrap();
        perform::run(&mut env);
        defence::run(&mut env);
        active_skill::run(&mut env, &build);
        offence::run(&mut env, &build);
        run(&mut env, &build);
        let cooldown = env.player.output.get("TrapCooldown");
        assert!(
            cooldown.is_some(),
            "TrapCooldown should be set for a trap skill"
        );
    }

    #[test]
    fn mine_skill_sets_detonation_time() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Shadow" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1">
    <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
      <Gem skillId="Blastchain Mine" level="20" quality="0" enabled="true"/>
    </Skill>
  </SkillSet></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="2" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let data = make_data();
        let mut env = init_env(&build, data).unwrap();
        perform::run(&mut env);
        defence::run(&mut env);
        active_skill::run(&mut env, &build);
        offence::run(&mut env, &build);
        run(&mut env, &build);
        let det_time = env.player.output.get("MineDetonationTime");
        assert!(
            det_time.is_some(),
            "MineDetonationTime should be set for a mine skill"
        );
    }
}
