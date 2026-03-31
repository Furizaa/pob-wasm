//! Trigger mechanics — totem, trap, mine, CoC, CWC, CWDT, and generic triggers.
//! Mirrors CalcTriggers.lua from Path of Building.

use super::env::{get_output_f64, CalcEnv};
use crate::build::Build;
use crate::mod_db::types::{KeywordFlags, ModFlags, ModType};

// ── Pure utility functions ──────────────────────────────────────────────────

/// Align a cooldown (in seconds) to the server tick rate (33ms).
/// `ceil(cd * 1000 / 33) * 33 / 1000`
pub fn align_to_server_tick(cd_seconds: f64) -> f64 {
    (cd_seconds * 1000.0 / 33.0).ceil() * 33.0 / 1000.0
}

/// Apply increased cooldown recovery rate (ICDR) to a base cooldown.
/// Returns `base_cd / (1 + icdr_percent/100)`.
pub fn apply_icdr(base_cd: f64, icdr_percent: f64) -> f64 {
    base_cd / (1.0 + icdr_percent / 100.0)
}

/// Calculate the effective trigger rate, limited by cooldown.
/// `min(source_rate * trigger_chance, 1/cooldown)`
pub fn calc_trigger_rate(source_rate: f64, trigger_chance: f64, cooldown: f64) -> f64 {
    let rate = source_rate * trigger_chance;
    if cooldown > 0.0 {
        rate.min(1.0 / cooldown)
    } else {
        rate
    }
}

// ── Skill type/name detection helpers ───────────────────────────────────────

fn skill_has_type(env: &CalcEnv, type_name: &str) -> bool {
    env.player
        .main_skill
        .as_ref()
        .map(|s| {
            s.skill_types
                .iter()
                .any(|t| t.eq_ignore_ascii_case(type_name))
        })
        .unwrap_or(false)
}

/// Detect if the active skill is a totem/trap/mine by type tags or name heuristic.
fn detect_trigger_category(env: &CalcEnv) -> Option<&'static str> {
    if skill_has_type(env, "Totem") || skill_has_type(env, "Ballista") {
        return Some("Totem");
    }
    if skill_has_type(env, "Trap") {
        return Some("Trap");
    }
    if skill_has_type(env, "Mine") {
        return Some("Mine");
    }
    // Fallback: heuristic by skill name for when gem data has no type tags
    let skill_id = env
        .player
        .main_skill
        .as_ref()
        .map(|s| s.skill_id.as_str())
        .unwrap_or("");
    if skill_name_is_totem(skill_id) {
        return Some("Totem");
    }
    if skill_name_is_trap(skill_id) {
        return Some("Trap");
    }
    if skill_name_is_mine(skill_id) {
        return Some("Mine");
    }
    None
}

fn skill_name_is_totem(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("totem")
        || lower.contains("ballista")
        || lower == "ancestral warchief"
        || lower == "ancestral protector"
        || lower == "searing bond"
}

fn skill_name_is_trap(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("trap")
}

fn skill_name_is_mine(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("mine")
}

// ── Orchestrator ────────────────────────────────────────────────────────────

pub fn run(env: &mut CalcEnv, _build: &Build) {
    let Some(skill) = env.player.main_skill.as_ref() else {
        return;
    };

    // First, check triggered_by for explicit trigger dispatch
    if let Some(trigger_type) = skill.triggered_by.clone() {
        dispatch_trigger(env, &trigger_type);
        return;
    }

    // Fallback: detect trigger type from skill types, flags, or name heuristic
    match detect_trigger_category(env) {
        Some("Totem") => calc_totem_dps(env),
        Some("Trap") => calc_trap_dps(env),
        Some("Mine") => calc_mine_dps(env),
        _ => {}
    }
}

fn dispatch_trigger(env: &mut CalcEnv, trigger_type: &str) {
    let output = env.player.output.clone();

    // ICDR from mods
    let icdr = env.player.mod_db.sum(
        ModType::Inc,
        "CooldownRecovery",
        ModFlags::NONE,
        KeywordFlags::NONE,
    );

    match trigger_type {
        "CastOnCrit" | "CoC" => {
            let base_cd = 0.15;
            let cd = align_to_server_tick(apply_icdr(base_cd, icdr));
            let speed = get_output_f64(&output, "Speed");
            let hit = get_output_f64(&output, "HitChance") / 100.0;
            let crit = get_output_f64(&output, "CritChance") / 100.0;
            let source_rate = speed * hit * crit;
            let trigger_rate = calc_trigger_rate(source_rate, 1.0, cd);
            env.player.set_output("TriggerRate", trigger_rate);
            env.player.set_output("TriggerCooldown", cd);
            env.player.set_output("Speed", trigger_rate);
        }
        "CastWhileChannelling" | "CWC" => {
            let base_interval = 0.35;
            let cd = align_to_server_tick(apply_icdr(base_interval, icdr));
            let trigger_rate = if cd > 0.0 { 1.0 / cd } else { 0.0 };
            env.player.set_output("TriggerRate", trigger_rate);
            env.player.set_output("TriggerCooldown", cd);
            env.player.set_output("Speed", trigger_rate);
        }
        "CastWhenDamageTaken" | "CWDT" => {
            let base_cd = 0.25;
            let cd = align_to_server_tick(apply_icdr(base_cd, icdr));
            let trigger_rate = if cd > 0.0 { 1.0 / cd } else { 0.0 };
            env.player.set_output("TriggerRate", trigger_rate);
            env.player.set_output("TriggerCooldown", cd);
            env.player.set_output("Speed", trigger_rate);
        }
        "Trap" => {
            calc_trap_dps(env);
        }
        "Mine" => {
            calc_mine_dps(env);
        }
        "Totem" | "Ballista" => {
            calc_totem_dps(env);
        }
        _ => {
            // Generic trigger: use TriggerCooldown base mod
            let base_cd = env.player.mod_db.sum(
                ModType::Base,
                "TriggerCooldown",
                ModFlags::NONE,
                KeywordFlags::NONE,
            );
            if base_cd > 0.0 {
                let cd = align_to_server_tick(apply_icdr(base_cd, icdr));
                let trigger_rate = if cd > 0.0 { 1.0 / cd } else { 0.0 };
                env.player.set_output("TriggerRate", trigger_rate);
                env.player.set_output("TriggerCooldown", cd);
                env.player.set_output("Speed", trigger_rate);
            }
        }
    }

    // Recalculate TotalDPS with new speed if speed was overridden
    let new_speed = get_output_f64(&env.player.output, "Speed");
    if new_speed > 0.0 {
        let avg_damage = get_output_f64(&env.player.output, "AverageDamage");
        if avg_damage > 0.0 {
            let total_dps = avg_damage * new_speed;
            env.player.set_output("TotalDPS", total_dps);
            let combined = get_output_f64(&env.player.output, "CombinedDPS");
            if combined == 0.0 {
                env.player.set_output("CombinedDPS", total_dps);
            }
        }
    }
}

// ── Totem calculations ──────────────────────────────────────────────────────

fn calc_totem_dps(env: &mut CalcEnv) {
    // Active totem limit — default 1, modified by "ActiveTotemLimit" BASE mods
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

    // Totem placement time (default 0.6s)
    let base_time = env.player.mod_db.sum(
        ModType::Base,
        "TotemPlacementTime",
        ModFlags::NONE,
        KeywordFlags::NONE,
    );
    let place_time = if base_time > 0.0 { base_time } else { 0.6 };
    env.player.set_output("TotemPlacementTime", place_time);

    // Totem DPS = skill TotalDPS * number of active totems
    let skill_dps = get_output_f64(&env.player.output, "TotalDPS");
    let totem_dps = skill_dps * active_totems;
    env.player.set_output("TotemDPS", totem_dps);
    env.player.set_output("CombinedDPS", totem_dps);
}

// ── Trap calculations ───────────────────────────────────────────────────────

fn calc_trap_dps(env: &mut CalcEnv) {
    // Trap throw time (default 0.6s, modified by TrapThrowingSpeed)
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
    env.player.set_output("TrapThrowingSpeed", 1.0 / throw_time);

    // Trap cooldown from mods, or default 4s
    let cooldown_mod = env.player.mod_db.sum(
        ModType::Base,
        "TrapCooldown",
        ModFlags::NONE,
        KeywordFlags::NONE,
    );
    let trap_cooldown = if cooldown_mod > 0.0 {
        cooldown_mod
    } else {
        4.0
    };
    env.player.set_output("TrapCooldown", trap_cooldown);

    // Trap DPS
    let skill_dps = get_output_f64(&env.player.output, "TotalDPS");
    let trap_dps = skill_dps / trap_cooldown;
    env.player.set_output("TrapDPS", trap_dps);
    env.player.set_output("CombinedDPS", trap_dps);
}

// ── Mine calculations ───────────────────────────────────────────────────────

fn calc_mine_dps(env: &mut CalcEnv) {
    // Mine laying time (default 0.3s)
    let base_lay = env.player.mod_db.sum(
        ModType::Base,
        "MineLayingTime",
        ModFlags::NONE,
        KeywordFlags::NONE,
    );
    let lay_time = if base_lay > 0.0 { base_lay } else { 0.3 };
    env.player.set_output("MineLayingTime", lay_time);

    let inc_speed = env.player.mod_db.sum(
        ModType::Inc,
        "MineLayingSpeed",
        ModFlags::NONE,
        KeywordFlags::NONE,
    );
    let more_speed = env
        .player
        .mod_db
        .more("MineLayingSpeed", ModFlags::NONE, KeywordFlags::NONE);
    let effective_lay_time = lay_time / ((1.0 + inc_speed / 100.0) * more_speed).max(0.001);
    env.player
        .set_output("MineLayingSpeed", 1.0 / effective_lay_time);

    // Detonation time = lay time + 0.25s detonation delay
    let detonation_time = effective_lay_time + 0.25;
    env.player.set_output("MineDetonationTime", detonation_time);

    // Mine DPS = skill_DPS / detonation_time
    let skill_dps = get_output_f64(&env.player.output, "TotalDPS");
    let mine_dps = skill_dps / detonation_time;
    env.player.set_output("MineDPS", mine_dps);
    env.player.set_output("CombinedDPS", mine_dps);
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        build::parse_xml,
        calc::{active_skill, defence, env::CalcEnv, offence, perform, setup::init_env},
        data::GameData,
        mod_db::ModDb,
    };
    use std::sync::Arc;

    fn make_data() -> Arc<GameData> {
        Arc::new(GameData::from_json(crate::tests::stub_game_data_json()).unwrap())
    }

    fn make_env() -> CalcEnv {
        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        CalcEnv::new(ModDb::new(), ModDb::new(), Arc::new(game_data))
    }

    // ── Pure function tests ─────────────────────────────────────────────

    #[test]
    fn server_tick_alignment() {
        // 0.15s → ceil(150/33)*33/1000 = ceil(4.545)*33/1000 = 5*33/1000 = 0.165
        let aligned = align_to_server_tick(0.15);
        assert!(
            (aligned - 0.165).abs() < 0.001,
            "expected ~0.165, got {aligned}"
        );

        // 0.33s → ceil(330/33)*33/1000 = 10*33/1000 = 0.33 (exact tick boundary)
        let aligned2 = align_to_server_tick(0.33);
        assert!(
            (aligned2 - 0.33).abs() < 0.001,
            "expected ~0.33, got {aligned2}"
        );

        // 0.01s → ceil(10/33)*33/1000 = 1*33/1000 = 0.033
        let aligned3 = align_to_server_tick(0.01);
        assert!(
            (aligned3 - 0.033).abs() < 0.001,
            "expected ~0.033, got {aligned3}"
        );
    }

    #[test]
    fn icdr_reduces_cooldown() {
        // 0.15s base, 52% ICDR → 0.15 / 1.52 ≈ 0.0987
        let cd = apply_icdr(0.15, 52.0);
        assert!((cd - 0.0987).abs() < 0.001, "expected ~0.0987, got {cd}");

        // 0% ICDR → unchanged
        let cd2 = apply_icdr(0.15, 0.0);
        assert!((cd2 - 0.15).abs() < 0.0001, "expected 0.15, got {cd2}");
    }

    #[test]
    fn trigger_rate_limited_by_cooldown() {
        // source_rate=10, chance=1.0, cooldown=0.2 → 1/0.2=5 → min(10, 5) = 5
        let rate = calc_trigger_rate(10.0, 1.0, 0.2);
        assert!((rate - 5.0).abs() < 0.001, "expected 5.0, got {rate}");

        // source_rate=2, chance=0.5, cooldown=0.5 → rate=1.0, 1/0.5=2.0 → min(1.0, 2.0) = 1.0
        let rate2 = calc_trigger_rate(2.0, 0.5, 0.5);
        assert!((rate2 - 1.0).abs() < 0.001, "expected 1.0, got {rate2}");

        // No cooldown → uncapped
        let rate3 = calc_trigger_rate(10.0, 1.0, 0.0);
        assert!((rate3 - 10.0).abs() < 0.001, "expected 10.0, got {rate3}");
    }

    // ── Integration tests (preserved from original) ─────────────────────

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
