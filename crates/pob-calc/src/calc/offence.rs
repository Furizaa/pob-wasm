use super::env::{get_output_f64, CalcEnv, OutputValue};
use crate::build::Build;
use crate::mod_db::types::{KeywordFlags, ModFlags, ModType};

pub fn run(env: &mut CalcEnv, build: &Build) {
    let Some(_skill_ref) = env.player.main_skill.as_ref() else {
        return;
    };

    // Summoner skills: set placeholder outputs and return
    // Full minion actor calculation is deferred (TODO: Phase 9)
    let is_summoner = env.player.mod_db.flag(
        "Summoner",
        crate::mod_db::types::ModFlags::NONE,
        crate::mod_db::types::KeywordFlags::NONE,
    );
    if is_summoner {
        let minion_count = crate::calc::env::get_output_f64(&env.player.output, "MinionCount");
        env.player.set_output("TotalDPS", 0.0);
        env.player.set_output("CombinedDPS", 0.0);
        env.player.set_output("MinionDPS", 0.0);
        env.player.set_output("MinionCount", minion_count); // preserve it
        return;
    }

    // Extract skill values before mutably borrowing env.player for set_output
    let (is_attack, is_spell, base_crit_chance, attack_speed_base, cast_time) = {
        let skill = env.player.main_skill.as_ref().unwrap();
        (
            skill.is_attack,
            skill.is_spell,
            skill.base_crit_chance,
            skill.attack_speed_base,
            skill.cast_time,
        )
    };
    let base_damage = {
        let skill = env.player.main_skill.as_ref().unwrap();
        skill.base_damage.clone()
    };

    let combined_flags = ModFlags(
        if is_attack { ModFlags::ATTACK.0 } else { 0 }
            | if is_spell { ModFlags::SPELL.0 } else { 0 },
    );

    // --- Base damage per type ---
    let dmg_types = ["Physical", "Lightning", "Cold", "Fire", "Chaos"];
    for dtype in &dmg_types {
        let (base_min, base_max) = base_damage.get(*dtype).copied().unwrap_or((0.0, 0.0));
        env.player
            .set_output(&format!("{}MinBase", dtype), base_min);
        env.player
            .set_output(&format!("{}MaxBase", dtype), base_max);
    }

    // --- Per-type inc/more modifiers ---
    let mut total_min = 0.0_f64;
    let mut total_max = 0.0_f64;

    for dtype in &dmg_types {
        let base_min_key = format!("{}MinBase", dtype);
        let base_max_key = format!("{}MaxBase", dtype);
        let base_min = get_output_f64(&env.player.output, &base_min_key);
        let base_max = get_output_f64(&env.player.output, &base_max_key);

        if base_min == 0.0 && base_max == 0.0 {
            continue;
        }

        let inc = env
            .player
            .mod_db
            .sum(ModType::Inc, "Damage", combined_flags, KeywordFlags::NONE)
            + env.player.mod_db.sum(
                ModType::Inc,
                &format!("{}Damage", dtype),
                combined_flags,
                KeywordFlags::NONE,
            );
        let more = env
            .player
            .mod_db
            .more("Damage", combined_flags, KeywordFlags::NONE)
            * env.player.mod_db.more(
                &format!("{}Damage", dtype),
                combined_flags,
                KeywordFlags::NONE,
            );

        let min_val = (base_min * (1.0 + inc / 100.0) * more).round();
        let max_val = (base_max * (1.0 + inc / 100.0) * more).round();
        env.player.set_output(&format!("{}Min", dtype), min_val);
        env.player.set_output(&format!("{}Max", dtype), max_val);
        total_min += min_val;
        total_max += max_val;
    }

    // --- Crit ---
    let base_crit_pct = base_crit_chance * 100.0;
    let inc_crit = env.player.mod_db.sum(
        ModType::Inc,
        "CritChance",
        combined_flags,
        KeywordFlags::NONE,
    );
    let more_crit = env
        .player
        .mod_db
        .more("CritChance", combined_flags, KeywordFlags::NONE);
    let crit_chance = (base_crit_pct * (1.0 + inc_crit / 100.0) * more_crit).clamp(0.0, 100.0);
    env.player.set_output("CritChance", crit_chance);

    let inc_crit_multi = env.player.mod_db.sum(
        ModType::Inc,
        "CritMultiplier",
        combined_flags,
        KeywordFlags::NONE,
    );
    let crit_multi = (150.0 + inc_crit_multi) / 100.0;
    env.player.set_output("CritMultiplier", crit_multi);

    // --- Hit chance ---
    let hit_chance = if is_spell {
        1.0
    } else {
        let accuracy = env.player.mod_db.sum(
            ModType::Base,
            "Accuracy",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        let lv = build.level as f64;
        let enemy_evasion = 15.0 + 8.0 * lv * lv / (lv + 5.0);
        if accuracy <= 0.0 {
            0.05
        } else {
            (accuracy / (accuracy + enemy_evasion)).clamp(0.05, 1.0)
        }
    };
    env.player.set_output("HitChance", hit_chance * 100.0);

    // --- Average hit and damage ---
    let avg_non_crit = (total_min + total_max) / 2.0;
    let crit_rate = crit_chance / 100.0;
    let average_hit = avg_non_crit * (1.0 - crit_rate) + (avg_non_crit * crit_multi) * crit_rate;
    env.player.set_output("AverageHit", average_hit);
    let average_damage = average_hit * hit_chance;
    env.player.set_output("AverageDamage", average_damage);

    // --- Speed ---
    let speed_mod = if is_spell {
        get_output_f64(&env.player.output, "CastSpeedMod").max(1.0)
    } else {
        get_output_f64(&env.player.output, "AttackSpeedMod").max(1.0)
    };
    let uses_per_sec = if is_spell {
        if cast_time > 0.0 {
            speed_mod / cast_time
        } else {
            0.0
        }
    } else {
        attack_speed_base * speed_mod
    };
    env.player.set_output("Speed", uses_per_sec);

    // --- TotalDPS ---
    let total_dps = average_damage * uses_per_sec;
    env.player.set_output("TotalDPS", total_dps);

    // --- Ailment DPS ---
    // Reference: CalcOffence.lua — IgniteDPS, BleedDPS, PoisonDPS sections
    // Uses POST-crit average hit per damage type.

    let fire_hit = {
        let min = get_output_f64(&env.player.output, "FireMin");
        let max = get_output_f64(&env.player.output, "FireMax");
        let avg = (min + max) / 2.0;
        let crit_rate = crit_chance / 100.0;
        let crit_weighted = avg * (1.0 - crit_rate) + avg * crit_multi * crit_rate;
        crit_weighted * hit_chance
    };
    let phys_hit = {
        let min = get_output_f64(&env.player.output, "PhysicalMin");
        let max = get_output_f64(&env.player.output, "PhysicalMax");
        let avg = (min + max) / 2.0;
        let crit_rate = crit_chance / 100.0;
        let crit_weighted = avg * (1.0 - crit_rate) + avg * crit_multi * crit_rate;
        crit_weighted * hit_chance
    };
    let chaos_hit = {
        let min = get_output_f64(&env.player.output, "ChaosMin");
        let max = get_output_f64(&env.player.output, "ChaosMax");
        let avg = (min + max) / 2.0;
        let crit_rate = crit_chance / 100.0;
        let crit_weighted = avg * (1.0 - crit_rate) + avg * crit_multi * crit_rate;
        crit_weighted * hit_chance
    };

    // Ignite DPS: fire_hit * 0.5 per second
    let inc_burn = env.player.mod_db.sum(
        ModType::Inc,
        "BurningDamage",
        combined_flags,
        KeywordFlags::NONE,
    ) + env.player.mod_db.sum(
        ModType::Inc,
        "AilmentDamage",
        combined_flags,
        KeywordFlags::NONE,
    ) + env.player.mod_db.sum(
        ModType::Inc,
        "FireDamage",
        combined_flags,
        KeywordFlags::NONE,
    );
    let more_burn = env
        .player
        .mod_db
        .more("BurningDamage", combined_flags, KeywordFlags::NONE)
        * env
            .player
            .mod_db
            .more("AilmentDamage", combined_flags, KeywordFlags::NONE);
    let ignite_dps = fire_hit * 0.5 * (1.0 + inc_burn / 100.0) * more_burn;
    env.player.set_output("IgniteDPS", ignite_dps);

    // Bleed DPS: phys_hit * 0.7 / 5.0 (bleed ticks for 5 seconds at 70% of hit damage)
    let inc_bleed = env.player.mod_db.sum(
        ModType::Inc,
        "BleedDamage",
        combined_flags,
        KeywordFlags::NONE,
    ) + env.player.mod_db.sum(
        ModType::Inc,
        "AilmentDamage",
        combined_flags,
        KeywordFlags::NONE,
    ) + env.player.mod_db.sum(
        ModType::Inc,
        "PhysicalDamage",
        combined_flags,
        KeywordFlags::NONE,
    );
    let more_bleed = env
        .player
        .mod_db
        .more("BleedDamage", combined_flags, KeywordFlags::NONE)
        * env
            .player
            .mod_db
            .more("AilmentDamage", combined_flags, KeywordFlags::NONE);
    let bleed_dps = phys_hit * 0.7 / 5.0 * (1.0 + inc_bleed / 100.0) * more_bleed;
    env.player.set_output("BleedDPS", bleed_dps);

    // Poison DPS: (phys_hit + chaos_hit) * 0.3 / 2.0 (poison ticks for 2 seconds at 30%)
    let inc_poison = env.player.mod_db.sum(
        ModType::Inc,
        "PoisonDamage",
        combined_flags,
        KeywordFlags::NONE,
    ) + env.player.mod_db.sum(
        ModType::Inc,
        "AilmentDamage",
        combined_flags,
        KeywordFlags::NONE,
    ) + env.player.mod_db.sum(
        ModType::Inc,
        "ChaosDamage",
        combined_flags,
        KeywordFlags::NONE,
    );
    let more_poison = env
        .player
        .mod_db
        .more("PoisonDamage", combined_flags, KeywordFlags::NONE)
        * env
            .player
            .mod_db
            .more("AilmentDamage", combined_flags, KeywordFlags::NONE);
    let poison_dps = (phys_hit + chaos_hit) * 0.3 / 2.0 * (1.0 + inc_poison / 100.0) * more_poison;
    env.player.set_output("PoisonDPS", poison_dps);

    // TotalDotDPS
    let total_dot_dps = ignite_dps + bleed_dps + poison_dps;
    env.player.set_output("TotalDotDPS", total_dot_dps);

    // CombinedDPS: hit DPS + dominant DoT DPS
    let dominant_dot = [ignite_dps, bleed_dps, poison_dps]
        .into_iter()
        .fold(0.0_f64, f64::max);
    let combined_dps = total_dps + dominant_dot;
    env.player.set_output("CombinedDPS", combined_dps);

    // Breakdown
    if total_min > 0.0 || total_max > 0.0 {
        env.player.set_breakdown_lines(
            "Damage",
            vec![
                format!("{:.0}–{:.0} (base)", total_min, total_max),
                format!("Average hit: {:.1}", average_hit),
                format!("Speed: {:.2}/s → TotalDPS: {:.1}", uses_per_sec, total_dps),
            ],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        build::parse_xml,
        calc::{active_skill, defence, perform, setup::init_env},
        data::GameData,
    };
    use std::sync::Arc;

    fn make_data() -> Arc<GameData> {
        Arc::new(GameData::from_json(crate::tests::stub_game_data_json()).unwrap())
    }

    fn full_run(xml: &str) -> crate::calc::env::CalcEnv {
        let build = parse_xml(xml).unwrap();
        let data = make_data();
        let mut env = init_env(&build, data).unwrap();
        perform::run(&mut env);
        defence::run(&mut env);
        active_skill::run(&mut env, &build);
        run(&mut env, &build);
        env
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
    fn offence_sets_crit_chance_output() {
        let env = full_run(CLEAVE_BUILD);
        let crit = env.player.output.get("CritChance");
        assert!(crit.is_some(), "CritChance should be set by offence::run");
    }

    #[test]
    fn offence_sets_total_dps_output() {
        let env = full_run(CLEAVE_BUILD);
        let dps = env.player.output.get("TotalDPS");
        assert!(dps.is_some(), "TotalDPS should be set by offence::run");
        // With stub data (no gem levels), TotalDPS should be 0 (no base damage).
        // With real gem data (Cleave L20), TotalDPS > 0 would be expected.
    }

    #[test]
    fn offence_sets_hit_chance_output() {
        let env = full_run(CLEAVE_BUILD);
        let hit_chance = env.player.output.get("HitChance");
        assert!(
            hit_chance.is_some(),
            "HitChance should be set by offence::run"
        );
    }

    #[test]
    fn spell_has_full_hit_chance() {
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
        let env = full_run(xml);
        if let Some(OutputValue::Number(hit_chance)) = env.player.output.get("HitChance") {
            assert!(
                (*hit_chance - 100.0).abs() < 0.01,
                "Spell should have 100% hit chance, got {}",
                hit_chance
            );
        } else {
            panic!("HitChance not set");
        }
    }
}
