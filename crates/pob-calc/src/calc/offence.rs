//! Offence calculation — damage, crit, speed, DPS, leech.
//! Mirrors CalcOffence.lua from Path of Building.

use super::env::{get_output_f64, CalcEnv};
use crate::build::Build;
use crate::calc::offence_utils::{
    build_conversion_table, ConversionTable, DMG_TYPE_NAMES, NUM_DMG_TYPES,
};
use crate::mod_db::types::{KeywordFlags, ModFlags, ModType, SkillCfg};

// ── Helper: build SkillCfg from active skill ────────────────────────────────

/// Build a SkillCfg from the active skill's flags and metadata.
fn build_skill_cfg(env: &CalcEnv) -> SkillCfg {
    let skill = env.player.main_skill.as_ref().unwrap();

    let mut flags = ModFlags::NONE;
    if skill.is_attack {
        flags = flags | ModFlags::ATTACK;
    }
    if skill.is_spell {
        flags = flags | ModFlags::SPELL;
    }
    // HIT flag: attacks and spells that deal damage are hits
    flags = flags | ModFlags::HIT;
    if skill.is_melee {
        flags = flags | ModFlags::MELEE;
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

// ── Main entry point ────────────────────────────────────────────────────────

pub fn run(env: &mut CalcEnv, build: &Build) {
    let Some(_skill_ref) = env.player.main_skill.as_ref() else {
        return;
    };

    // Summoner skills: set placeholder outputs and return
    // Full minion actor calculation is deferred (TODO: Phase 9)
    let is_summoner = env
        .player
        .mod_db
        .flag("Summoner", ModFlags::NONE, KeywordFlags::NONE);
    if is_summoner {
        let minion_count = get_output_f64(&env.player.output, "MinionCount");
        env.player.set_output("TotalDPS", 0.0);
        env.player.set_output("CombinedDPS", 0.0);
        env.player.set_output("MinionDPS", 0.0);
        env.player.set_output("MinionCount", minion_count);
        return;
    }

    // Extract skill values before mutably borrowing env for set_output
    let (is_attack, is_spell, base_crit_chance, attack_speed_base, cast_time, can_crit) = {
        let skill = env.player.main_skill.as_ref().unwrap();
        (
            skill.is_attack,
            skill.is_spell,
            skill.base_crit_chance,
            skill.attack_speed_base,
            skill.cast_time,
            skill.can_crit,
        )
    };
    let base_damage = {
        let skill = env.player.main_skill.as_ref().unwrap();
        skill.base_damage.clone()
    };
    let damage_effectiveness = {
        let skill = env.player.main_skill.as_ref().unwrap();
        skill.damage_effectiveness
    };

    // Build SkillCfg for all queries
    let cfg = build_skill_cfg(env);
    let output_snap = env.player.output.clone();

    // ── Hit chance ───────────────────────────────────────────────────────

    let resolute_technique =
        env.player
            .mod_db
            .flag_cfg("ResoluteTechnique", Some(&cfg), &output_snap);

    let hit_chance_pct = if is_spell || resolute_technique {
        // Spells always hit; Resolute Technique always hits
        100.0
    } else if is_attack {
        let accuracy = get_output_f64(&output_snap, "Accuracy");
        let enemy_evasion = get_output_f64(&env.enemy.output, "Evasion");
        let evasion = if enemy_evasion > 0.0 {
            enemy_evasion
        } else {
            // Fallback: estimate enemy evasion from level
            let lv = build.level as f64;
            15.0 + 8.0 * lv * lv / (lv + 5.0)
        };
        crate::calc::defence::hit_chance(evasion, accuracy)
    } else {
        100.0
    };
    env.player.set_output("HitChance", hit_chance_pct);
    let hit_chance = hit_chance_pct / 100.0;

    // ── Crit ─────────────────────────────────────────────────────────────

    let (crit_chance, crit_multi) = if !can_crit || resolute_technique {
        // Resolute Technique: no crits
        env.player.set_output("CritChance", 0.0);
        env.player.set_output("CritMultiplier", 1.5);
        (0.0, 1.5)
    } else {
        let base_crit_pct = base_crit_chance * 100.0;
        let inc_crit =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, "CritChance", Some(&cfg), &output_snap);
        let more_crit = env
            .player
            .mod_db
            .more_cfg("CritChance", Some(&cfg), &output_snap);
        let cc = (base_crit_pct * (1.0 + inc_crit / 100.0) * more_crit).clamp(0.0, 100.0);
        env.player.set_output("CritChance", cc);

        // Crit multiplier: base 150% + base mods
        let base_crit_multi =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, "CritMultiplier", Some(&cfg), &output_snap);
        let cm = (150.0 + base_crit_multi) / 100.0;
        env.player.set_output("CritMultiplier", cm);
        (cc, cm)
    };

    // ── Speed ────────────────────────────────────────────────────────────

    let action_speed = env.player.action_speed_mod;

    let uses_per_sec = if is_attack {
        let inc_speed = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "Speed", Some(&cfg), &output_snap)
            + env
                .player
                .mod_db
                .sum_cfg(ModType::Inc, "AttackSpeed", Some(&cfg), &output_snap);
        let more_speed = env
            .player
            .mod_db
            .more_cfg("Speed", Some(&cfg), &output_snap)
            * env
                .player
                .mod_db
                .more_cfg("AttackSpeed", Some(&cfg), &output_snap);
        attack_speed_base * (1.0 + inc_speed / 100.0) * more_speed * action_speed
    } else if is_spell {
        if cast_time <= 0.0 {
            0.0
        } else {
            let inc_speed =
                env.player
                    .mod_db
                    .sum_cfg(ModType::Inc, "Speed", Some(&cfg), &output_snap)
                    + env.player.mod_db.sum_cfg(
                        ModType::Inc,
                        "CastSpeed",
                        Some(&cfg),
                        &output_snap,
                    );
            let more_speed = env
                .player
                .mod_db
                .more_cfg("Speed", Some(&cfg), &output_snap)
                * env
                    .player
                    .mod_db
                    .more_cfg("CastSpeed", Some(&cfg), &output_snap);
            (1.0 / cast_time) * (1.0 + inc_speed / 100.0) * more_speed * action_speed
        }
    } else {
        0.0
    };
    env.player.set_output("Speed", uses_per_sec);

    // ── Double / Triple damage ───────────────────────────────────────────

    let double_dmg_chance = env
        .player
        .mod_db
        .sum_cfg(
            ModType::Base,
            "DoubleDamageChance",
            Some(&cfg),
            &output_snap,
        )
        .clamp(0.0, 100.0);
    env.player
        .set_output("DoubleDamageChance", double_dmg_chance);

    let triple_dmg_chance = env
        .player
        .mod_db
        .sum_cfg(
            ModType::Base,
            "TripleDamageChance",
            Some(&cfg),
            &output_snap,
        )
        .clamp(0.0, 100.0);
    env.player
        .set_output("TripleDamageChance", triple_dmg_chance);

    // ── Conversion table ─────────────────────────────────────────────────

    let conv_table = build_conversion_table(&env.player.mod_db, &output_snap, Some(&cfg));

    // ── Per-type base damage (from skill + added damage from modDB) ──────

    let mut base_min_arr = [0.0_f64; NUM_DMG_TYPES];
    let mut base_max_arr = [0.0_f64; NUM_DMG_TYPES];

    for (i, dtype) in DMG_TYPE_NAMES.iter().enumerate() {
        let (skill_min, skill_max) = base_damage.get(*dtype).copied().unwrap_or((0.0, 0.0));

        // Added damage from modDB (flat added, e.g. "PhysicalMin", "PhysicalMax")
        let added_min = env.player.mod_db.sum_cfg(
            ModType::Base,
            &format!("{}Min", dtype),
            Some(&cfg),
            &output_snap,
        ) * damage_effectiveness;
        let added_max = env.player.mod_db.sum_cfg(
            ModType::Base,
            &format!("{}Max", dtype),
            Some(&cfg),
            &output_snap,
        ) * damage_effectiveness;

        base_min_arr[i] = skill_min + added_min;
        base_max_arr[i] = skill_max + added_max;

        env.player
            .set_output(&format!("{}MinBase", dtype), base_min_arr[i]);
        env.player
            .set_output(&format!("{}MaxBase", dtype), base_max_arr[i]);
    }

    // ── Apply conversion chain ──────────────────────────────────────────

    let (converted_min, converted_max) =
        apply_conversion(&conv_table, &base_min_arr, &base_max_arr);

    // ── Per-type inc/more and enemy resistance ──────────────────────────

    let enemy_output = env.enemy.output.clone();
    let mut total_min = 0.0_f64;
    let mut total_max = 0.0_f64;

    for (i, dtype) in DMG_TYPE_NAMES.iter().enumerate() {
        let c_min = converted_min[i];
        let c_max = converted_max[i];

        if c_min == 0.0 && c_max == 0.0 {
            env.player.set_output(&format!("{}Min", dtype), 0.0);
            env.player.set_output(&format!("{}Max", dtype), 0.0);
            continue;
        }

        // Inc/more for generic "Damage" + type-specific "{Type}Damage"
        let inc = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "Damage", Some(&cfg), &output_snap)
            + env.player.mod_db.sum_cfg(
                ModType::Inc,
                &format!("{}Damage", dtype),
                Some(&cfg),
                &output_snap,
            );
        let more = env
            .player
            .mod_db
            .more_cfg("Damage", Some(&cfg), &output_snap)
            * env
                .player
                .mod_db
                .more_cfg(&format!("{}Damage", dtype), Some(&cfg), &output_snap);

        let min_val = c_min * (1.0 + inc / 100.0) * more;
        let max_val = c_max * (1.0 + inc / 100.0) * more;

        // Apply enemy resistance
        let resist_key = format!("{}Resist", dtype);
        let enemy_resist = get_output_f64(&enemy_output, &resist_key);

        // Penetration
        let pen = env.player.mod_db.sum_cfg(
            ModType::Base,
            &format!("{}Penetration", dtype),
            Some(&cfg),
            &output_snap,
        );
        // For elemental types, also check generic ElementalPenetration
        let elem_pen = if *dtype == "Fire" || *dtype == "Cold" || *dtype == "Lightning" {
            env.player.mod_db.sum_cfg(
                ModType::Base,
                "ElementalPenetration",
                Some(&cfg),
                &output_snap,
            )
        } else {
            0.0
        };
        let effective_resist = enemy_resist - pen - elem_pen;
        let resist_mult = 1.0 - effective_resist / 100.0;

        let final_min = (min_val * resist_mult).round();
        let final_max = (max_val * resist_mult).round();

        env.player.set_output(&format!("{}Min", dtype), final_min);
        env.player.set_output(&format!("{}Max", dtype), final_max);
        total_min += final_min;
        total_max += final_max;
    }

    // ── Crit-weighted average hit ────────────────────────────────────────

    let avg_non_crit = (total_min + total_max) / 2.0;
    let crit_rate = crit_chance / 100.0;
    let average_hit = avg_non_crit * (1.0 - crit_rate) + (avg_non_crit * crit_multi) * crit_rate;

    // Apply double/triple damage
    let double_rate = double_dmg_chance / 100.0;
    let triple_rate = triple_dmg_chance / 100.0;
    let extra_multi = 1.0 + double_rate * 1.0 + triple_rate * 2.0;
    env.player.set_output("ScaledDamageEffect", extra_multi);
    let average_hit_final = average_hit * extra_multi;
    env.player.set_output("AverageHit", average_hit_final);

    // ── DPS assembly ────────────────────────────────────────────────────

    let average_damage = average_hit_final * hit_chance;
    env.player.set_output("AverageDamage", average_damage);

    let total_dps = average_damage * uses_per_sec;
    env.player.set_output("TotalDPS", total_dps);

    // ── Per-type hit averages (for ailments) ────────────────────────────
    // Store crit-weighted average hit per damage type for use by ailment modules.

    for dtype in DMG_TYPE_NAMES {
        let type_avg = calc_type_avg_hit(env, dtype, crit_rate, crit_multi, hit_chance);
        env.player
            .set_output(&format!("{}HitAverage", dtype), type_avg);
    }

    // ── Skill type stats ─────────────────────────────────────────────────

    calc_skill_type_stats(env, &cfg);

    // ── Duration and cost ────────────────────────────────────────────────

    calc_duration_and_cost(env, &cfg);

    // ── Ailment DPS ─────────────────────────────────────────────────────
    {
        let ailment_ctx = super::offence_ailments::AilmentContext {
            crit_chance,
            crit_multiplier: crit_multi,
            hit_chance: hit_chance_pct,
            speed: uses_per_sec,
            is_attack,
        };
        super::offence_ailments::calc_ignite(env, &cfg, &ailment_ctx);
        super::offence_ailments::calc_bleed(env, &cfg, &ailment_ctx);
        super::offence_ailments::calc_poison(env, &cfg, &ailment_ctx);
    }

    // ── Impale and non-ailment DoT ───────────────────────────────────────

    super::offence_dot::calc_impale(env, &cfg);
    super::offence_dot::calc_skill_dot(env, &cfg);

    // ── Combined DPS ────────────────────────────────────────────────────

    super::offence_dot::calc_combined_dps(env);

    // ── Leech ────────────────────────────────────────────────────────────

    calc_leech(env, &cfg, &output_snap, average_damage, uses_per_sec);

    // ── Breakdown ────────────────────────────────────────────────────────

    if total_min > 0.0 || total_max > 0.0 {
        env.player.set_breakdown_lines(
            "Damage",
            vec![
                format!("{:.0}–{:.0} (base)", total_min, total_max),
                format!("Average hit: {:.1}", average_hit_final),
                format!("Speed: {:.2}/s → TotalDPS: {:.1}", uses_per_sec, total_dps),
            ],
        );
    }
}

// ── Conversion application ──────────────────────────────────────────────────

/// Apply the conversion table to per-type base min/max arrays.
/// Returns (converted_min[5], converted_max[5]).
fn apply_conversion(
    table: &ConversionTable,
    base_min: &[f64; NUM_DMG_TYPES],
    base_max: &[f64; NUM_DMG_TYPES],
) -> ([f64; NUM_DMG_TYPES], [f64; NUM_DMG_TYPES]) {
    let mut out_min = [0.0_f64; NUM_DMG_TYPES];
    let mut out_max = [0.0_f64; NUM_DMG_TYPES];

    for src in 0..NUM_DMG_TYPES {
        if base_min[src] == 0.0 && base_max[src] == 0.0 {
            continue;
        }
        for dst in 0..NUM_DMG_TYPES {
            let conv_frac = table.base[src][dst];
            let extra_frac = table.extra[src][dst];
            let total_frac = conv_frac + extra_frac;
            if total_frac > 0.0 {
                out_min[dst] += base_min[src] * total_frac;
                out_max[dst] += base_max[src] * total_frac;
            }
        }
    }

    (out_min, out_max)
}

// ── Per-type average hit (for ailment calculations) ─────────────────────────

fn calc_type_avg_hit(
    env: &CalcEnv,
    dtype: &str,
    crit_rate: f64,
    crit_multi: f64,
    hit_chance: f64,
) -> f64 {
    let min = get_output_f64(&env.player.output, &format!("{}Min", dtype));
    let max = get_output_f64(&env.player.output, &format!("{}Max", dtype));
    let avg = (min + max) / 2.0;
    let crit_weighted = avg * (1.0 - crit_rate) + avg * crit_multi * crit_rate;
    crit_weighted * hit_chance
}

// ── Skill type stats (Task 8) ───────────────────────────────────────────────

fn calc_skill_type_stats(env: &mut CalcEnv, cfg: &SkillCfg) {
    let output_snap = env.player.output.clone();

    // Projectile count
    let proj_count =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ProjectileCount", Some(cfg), &output_snap);
    if proj_count > 0.0 {
        env.player.set_output("ProjectileCount", proj_count);
    }

    // Pierce count
    let pierce = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "PierceCount", Some(cfg), &output_snap);
    env.player.set_output("PierceCount", pierce);

    // Chain max
    let chain = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "ChainCountMax", Some(cfg), &output_snap);
    env.player.set_output("ChainMax", chain);

    // Fork count
    let fork = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "ForkCountMax", Some(cfg), &output_snap);
    env.player.set_output("ForkCount", fork);

    // Melee weapon range
    let weapon_range =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MeleeWeaponRange", Some(cfg), &output_snap);
    env.player.set_output("WeaponRange", weapon_range);

    // Trap throwing speed
    let trap_inc =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "TrapThrowingSpeed", Some(cfg), &output_snap);
    let trap_more = env
        .player
        .mod_db
        .more_cfg("TrapThrowingSpeed", Some(cfg), &output_snap);
    let trap_speed = (1.0 + trap_inc / 100.0) * trap_more;
    env.player.set_output("TrapThrowingSpeed", trap_speed);

    // Mine laying speed
    let mine_inc =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "MineLayingSpeed", Some(cfg), &output_snap);
    let mine_more = env
        .player
        .mod_db
        .more_cfg("MineLayingSpeed", Some(cfg), &output_snap);
    let mine_speed = (1.0 + mine_inc / 100.0) * mine_more;
    env.player.set_output("MineLayingSpeed", mine_speed);

    // Active totem limit
    let totem_limit =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ActiveTotemLimit", Some(cfg), &output_snap);
    let totem_limit = if totem_limit > 0.0 { totem_limit } else { 1.0 };
    env.player.set_output("ActiveTotemLimit", totem_limit);

    // Totem placement speed
    let totem_inc =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "TotemPlacementSpeed", Some(cfg), &output_snap);
    let totem_more = env
        .player
        .mod_db
        .more_cfg("TotemPlacementSpeed", Some(cfg), &output_snap);
    let totem_speed = (1.0 + totem_inc / 100.0) * totem_more;
    env.player.set_output("TotemPlacementSpeed", totem_speed);
}

// ── Duration and cost (Task 9) ──────────────────────────────────────────────

fn calc_duration_and_cost(env: &mut CalcEnv, cfg: &SkillCfg) {
    let output_snap = env.player.output.clone();

    // Duration
    let base_dur = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "Duration", Some(cfg), &output_snap);
    let dur_inc = env
        .player
        .mod_db
        .sum_cfg(ModType::Inc, "Duration", Some(cfg), &output_snap);
    let dur_more = env
        .player
        .mod_db
        .more_cfg("Duration", Some(cfg), &output_snap);
    let duration = crate::calc::offence_utils::calc_skill_duration(base_dur, dur_inc, dur_more);
    env.player.set_output("Duration", duration);

    // Cooldown
    let base_cd = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "Cooldown", Some(cfg), &output_snap);
    let cd_inc =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "CooldownRecovery", Some(cfg), &output_snap);
    let cd_more = env
        .player
        .mod_db
        .more_cfg("CooldownRecovery", Some(cfg), &output_snap);
    let cooldown = crate::calc::offence_utils::calc_skill_cooldown(base_cd, cd_inc, cd_more);
    env.player.set_output("Cooldown", cooldown);

    // Mana cost
    let mana_base = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "ManaCost", Some(cfg), &output_snap);
    let mana_inc = env
        .player
        .mod_db
        .sum_cfg(ModType::Inc, "ManaCost", Some(cfg), &output_snap);
    let mana_more = env
        .player
        .mod_db
        .more_cfg("ManaCost", Some(cfg), &output_snap);
    let mana_cost = mana_base * (1.0 + mana_inc / 100.0) * mana_more;
    env.player.set_output("ManaCost", mana_cost.max(0.0));

    // Mana per second
    let speed = get_output_f64(&output_snap, "Speed");
    let mana_per_sec = if speed > 0.0 { mana_cost * speed } else { 0.0 };
    env.player.set_output("ManaPerSecondCost", mana_per_sec);
}

// ── Leech calculation ───────────────────────────────────────────────────────

fn calc_leech(
    env: &mut CalcEnv,
    cfg: &SkillCfg,
    output: &super::env::OutputTable,
    _average_damage: f64,
    uses_per_sec: f64,
) {
    // Per-type life/ES/mana leech
    for (resource, stat_prefix) in &[
        ("Life", "DamageLifeLeech"),
        ("EnergyShield", "DamageEnergyShieldLeech"),
        ("Mana", "DamageManaLeech"),
    ] {
        let mut total_leech_rate = 0.0_f64;

        // Generic leech: "{resource}LeechRate" or "DamageLifeLeech" etc.
        let generic_leech =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, stat_prefix, Some(cfg), output);

        // Per-type leech: "{Type}DamageLifeLeech" etc.
        for dtype in &DMG_TYPE_NAMES {
            let type_leech_stat = format!("{}Damage{}Leech", dtype, resource);
            let type_leech =
                env.player
                    .mod_db
                    .sum_cfg(ModType::Base, &type_leech_stat, Some(cfg), output);

            if type_leech > 0.0 || generic_leech > 0.0 {
                let type_min = get_output_f64(&env.player.output, &format!("{}Min", dtype));
                let type_max = get_output_f64(&env.player.output, &format!("{}Max", dtype));
                let type_avg = (type_min + type_max) / 2.0;
                if type_avg > 0.0 {
                    total_leech_rate += type_avg * (type_leech + generic_leech) / 100.0;
                }
            }
        }

        let leech_per_sec = total_leech_rate * uses_per_sec;
        env.player
            .set_output(&format!("{}LeechPerSecond", resource), leech_per_sec);
    }

    // On-hit recovery
    for (resource, stat_name) in &[
        ("Life", "LifeOnHit"),
        ("EnergyShield", "EnergyShieldOnHit"),
        ("Mana", "ManaOnHit"),
    ] {
        let on_hit = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, stat_name, Some(cfg), output);
        let hit_chance_pct = get_output_f64(&env.player.output, "HitChance");
        let on_hit_per_sec = on_hit * (hit_chance_pct / 100.0) * uses_per_sec;
        env.player
            .set_output(&format!("{}OnHitPerSecond", resource), on_hit_per_sec);
        env.player.set_output(stat_name, on_hit);
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        build::parse_xml,
        calc::{
            active_skill, defence,
            env::{CalcEnv, OutputValue},
            perform,
            setup::init_env,
        },
        data::GameData,
        mod_db::{
            types::{Mod, ModSource},
            ModDb,
        },
    };
    use std::collections::HashMap;
    use std::sync::Arc;

    fn make_data() -> Arc<GameData> {
        Arc::new(GameData::from_json(crate::tests::stub_game_data_json()).unwrap())
    }

    fn full_run(xml: &str) -> CalcEnv {
        let build = parse_xml(xml).unwrap();
        let data = make_data();
        let mut env = init_env(&build, data).unwrap();
        perform::run(&mut env);
        defence::run(&mut env);
        active_skill::run(&mut env, &build);
        run(&mut env, &build);
        env
    }

    fn src() -> ModSource {
        ModSource::new("Test", "test")
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

    // ── Existing tests (backward compatible) ─────────────────────────────

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

    // ── New Task tests ──────────────────────────────────────────────────

    // Task 1: ConversionTable tests are in offence_utils::tests

    // Task 2: Crit base 150%

    #[test]
    fn crit_multiplier_base_is_150() {
        // Create a minimal env with an active skill that has crit
        let mut db = ModDb::new();
        // Need some base stats for perform to work
        db.add(Mod::new_base("Life", 1000.0, src()));
        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(db, ModDb::new(), Arc::new(game_data));

        // Set up a minimal active skill with crit
        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestSkill".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: true,
            is_spell: false,
            is_melee: true,
            can_crit: true,
            base_crit_chance: 0.05, // 5%
            base_damage: HashMap::from([("Physical".to_string(), (100.0, 200.0))]),
            attack_speed_base: 1.5,
            cast_time: 0.0,
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.set_output("Accuracy", 1000.0);
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Marauder".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        let cm = get_output_f64(&env.player.output, "CritMultiplier");
        assert!(
            (cm - 1.5).abs() < 0.01,
            "Base crit multiplier should be 1.5 (150%), got {}",
            cm
        );
    }

    #[test]
    fn crit_multiplier_with_base_mods() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("Life", 1000.0, src()));
        // +50% base crit multiplier
        db.add(Mod::new_base("CritMultiplier", 50.0, src()));
        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(db, ModDb::new(), Arc::new(game_data));

        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestSkill".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: true,
            is_spell: false,
            is_melee: true,
            can_crit: true,
            base_crit_chance: 0.05,
            base_damage: HashMap::from([("Physical".to_string(), (100.0, 200.0))]),
            attack_speed_base: 1.5,
            cast_time: 0.0,
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.set_output("Accuracy", 1000.0);
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Marauder".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        let cm = get_output_f64(&env.player.output, "CritMultiplier");
        assert!(
            (cm - 2.0).abs() < 0.01,
            "Crit multiplier should be 2.0 ((150+50)/100), got {}",
            cm
        );
    }

    // Task 3: Enemy resistance reduces damage

    #[test]
    fn enemy_resistance_reduces_damage() {
        let mut player_db = ModDb::new();
        player_db.add(Mod::new_base("Life", 1000.0, src()));

        // enemy has no direct resist mods but we set output directly
        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(player_db, ModDb::new(), Arc::new(game_data));

        // Set enemy fire resist to 50%
        env.enemy.set_output("FireResist", 50.0);

        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestSpell".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: false,
            is_spell: true,
            is_melee: false,
            can_crit: false,
            base_crit_chance: 0.0,
            base_damage: HashMap::from([("Fire".to_string(), (100.0, 100.0))]),
            attack_speed_base: 0.0,
            cast_time: 1.0,
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        // Fire damage should be reduced by 50% resistance
        let fire_min = get_output_f64(&env.player.output, "FireMin");
        let fire_max = get_output_f64(&env.player.output, "FireMax");
        assert!(
            (fire_min - 50.0).abs() < 1.0,
            "FireMin should be ~50 (100 * 0.5), got {}",
            fire_min
        );
        assert!(
            (fire_max - 50.0).abs() < 1.0,
            "FireMax should be ~50 (100 * 0.5), got {}",
            fire_max
        );
    }

    #[test]
    fn negative_enemy_resistance_amplifies_damage() {
        let player_db = ModDb::new();
        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(player_db, ModDb::new(), Arc::new(game_data));

        // Enemy has -50% cold resist
        env.enemy.set_output("ColdResist", -50.0);

        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestSpell".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: false,
            is_spell: true,
            is_melee: false,
            can_crit: false,
            base_crit_chance: 0.0,
            base_damage: HashMap::from([("Cold".to_string(), (100.0, 100.0))]),
            attack_speed_base: 0.0,
            cast_time: 1.0,
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        // Cold damage should be amplified: 100 * (1 - (-50/100)) = 100 * 1.5 = 150
        let cold_min = get_output_f64(&env.player.output, "ColdMin");
        assert!(
            (cold_min - 150.0).abs() < 1.0,
            "ColdMin should be ~150 (100 * 1.5), got {}",
            cold_min
        );
    }

    #[test]
    fn resolute_technique_always_hits_no_crit() {
        let mut player_db = ModDb::new();
        player_db.add(Mod::new_base("Life", 1000.0, src()));
        player_db.add(Mod::new_flag("ResoluteTechnique", src()));

        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(player_db, ModDb::new(), Arc::new(game_data));

        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestAttack".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: true,
            is_spell: false,
            is_melee: true,
            can_crit: true,
            base_crit_chance: 0.10,
            base_damage: HashMap::from([("Physical".to_string(), (100.0, 200.0))]),
            attack_speed_base: 1.5,
            cast_time: 0.0,
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.set_output("Accuracy", 100.0); // low accuracy
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Marauder".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        // RT: hit chance = 100%, crit = 0%
        let hc = get_output_f64(&env.player.output, "HitChance");
        assert!(
            (hc - 100.0).abs() < 0.01,
            "RT should give 100% hit chance, got {}",
            hc
        );
        let cc = get_output_f64(&env.player.output, "CritChance");
        assert!(cc.abs() < 0.01, "RT should give 0% crit chance, got {}", cc);
    }

    #[test]
    fn conversion_table_applied_to_damage() {
        let mut player_db = ModDb::new();
        // 50% phys converted to lightning
        player_db.add(Mod::new_base(
            "PhysicalDamageConvertToLightning",
            50.0,
            src(),
        ));

        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(player_db, ModDb::new(), Arc::new(game_data));

        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestAttack".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: true,
            is_spell: false,
            is_melee: true,
            can_crit: false,
            base_crit_chance: 0.0,
            base_damage: HashMap::from([("Physical".to_string(), (100.0, 100.0))]),
            attack_speed_base: 1.0,
            cast_time: 0.0,
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.set_output("Accuracy", 10000.0);
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Marauder".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        // 100 phys, 50% converted to lightning → 50 phys, 50 lightning
        // (no enemy resist, so values should be as-is, rounded)
        let phys_min = get_output_f64(&env.player.output, "PhysicalMin");
        let lightning_min = get_output_f64(&env.player.output, "LightningMin");
        assert!(
            (phys_min - 50.0).abs() < 1.0,
            "PhysMin should be ~50, got {}",
            phys_min
        );
        assert!(
            (lightning_min - 50.0).abs() < 1.0,
            "LightningMin should be ~50, got {}",
            lightning_min
        );
    }

    #[test]
    fn double_damage_increases_average_hit() {
        let mut player_db = ModDb::new();
        // 100% double damage chance
        player_db.add(Mod::new_base("DoubleDamageChance", 100.0, src()));

        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(player_db, ModDb::new(), Arc::new(game_data));

        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestSpell".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: false,
            is_spell: true,
            is_melee: false,
            can_crit: false,
            base_crit_chance: 0.0,
            base_damage: HashMap::from([("Fire".to_string(), (100.0, 100.0))]),
            attack_speed_base: 0.0,
            cast_time: 1.0,
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        // 100% double damage: average_hit = base_avg * 2.0
        let avg_hit = get_output_f64(&env.player.output, "AverageHit");
        // Base avg = (100 + 100) / 2 = 100. With double damage: 100 * 2 = 200
        assert!(
            (avg_hit - 200.0).abs() < 1.0,
            "AverageHit should be ~200 with 100% double damage, got {}",
            avg_hit
        );
    }

    #[test]
    fn speed_computed_for_attack() {
        let player_db = ModDb::new();
        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(player_db, ModDb::new(), Arc::new(game_data));

        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestAttack".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: true,
            is_spell: false,
            is_melee: true,
            can_crit: false,
            base_crit_chance: 0.0,
            base_damage: HashMap::from([("Physical".to_string(), (10.0, 20.0))]),
            attack_speed_base: 1.5,
            cast_time: 0.0,
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.set_output("Accuracy", 10000.0);
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Marauder".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        let speed = get_output_f64(&env.player.output, "Speed");
        assert!(
            (speed - 1.5).abs() < 0.01,
            "Attack speed should be 1.5/s, got {}",
            speed
        );
    }

    #[test]
    fn speed_computed_for_spell() {
        let player_db = ModDb::new();
        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(player_db, ModDb::new(), Arc::new(game_data));

        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestSpell".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: false,
            is_spell: true,
            is_melee: false,
            can_crit: false,
            base_crit_chance: 0.0,
            base_damage: HashMap::from([("Fire".to_string(), (10.0, 20.0))]),
            attack_speed_base: 0.0,
            cast_time: 0.5, // 0.5s cast time = 2 casts/sec
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        let speed = get_output_f64(&env.player.output, "Speed");
        assert!(
            (speed - 2.0).abs() < 0.01,
            "Spell speed should be 2.0/s (1/0.5), got {}",
            speed
        );
    }

    #[test]
    fn leech_outputs_set() {
        let player_db = ModDb::new();
        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(player_db, ModDb::new(), Arc::new(game_data));

        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestSpell".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: false,
            is_spell: true,
            is_melee: false,
            can_crit: false,
            base_crit_chance: 0.0,
            base_damage: HashMap::from([("Fire".to_string(), (100.0, 100.0))]),
            attack_speed_base: 0.0,
            cast_time: 1.0,
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        // Leech outputs should exist (even if 0)
        assert!(
            env.player.output.contains_key("LifeLeechPerSecond"),
            "LifeLeechPerSecond should be set"
        );
        assert!(
            env.player.output.contains_key("ManaLeechPerSecond"),
            "ManaLeechPerSecond should be set"
        );
        assert!(
            env.player.output.contains_key("EnergyShieldLeechPerSecond"),
            "EnergyShieldLeechPerSecond should be set"
        );
    }

    #[test]
    fn on_hit_recovery_outputs_set() {
        let mut player_db = ModDb::new();
        player_db.add(Mod::new_base("LifeOnHit", 20.0, src()));

        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(player_db, ModDb::new(), Arc::new(game_data));

        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestSpell".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: false,
            is_spell: true,
            is_melee: false,
            can_crit: false,
            base_crit_chance: 0.0,
            base_damage: HashMap::from([("Fire".to_string(), (100.0, 100.0))]),
            attack_speed_base: 0.0,
            cast_time: 1.0,
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        // LifeOnHit should be 20, and LifeOnHitPerSecond = 20 * 1.0 (hit_chance) * 1.0 (speed)
        let loh = get_output_f64(&env.player.output, "LifeOnHit");
        assert!(
            (loh - 20.0).abs() < 0.01,
            "LifeOnHit should be 20, got {}",
            loh
        );
        let loh_ps = get_output_f64(&env.player.output, "LifeOnHitPerSecond");
        assert!(
            (loh_ps - 20.0).abs() < 0.01,
            "LifeOnHitPerSecond should be 20 (20 * 1.0 * 1.0), got {}",
            loh_ps
        );
    }

    #[test]
    fn penetration_reduces_effective_resist() {
        let mut player_db = ModDb::new();
        player_db.add(Mod::new_base("FirePenetration", 20.0, src()));

        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(player_db, ModDb::new(), Arc::new(game_data));

        // Enemy has 50% fire resist
        env.enemy.set_output("FireResist", 50.0);

        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestSpell".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: false,
            is_spell: true,
            is_melee: false,
            can_crit: false,
            base_crit_chance: 0.0,
            base_damage: HashMap::from([("Fire".to_string(), (100.0, 100.0))]),
            attack_speed_base: 0.0,
            cast_time: 1.0,
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        // Effective resist = 50 - 20 = 30%, so damage = 100 * 0.7 = 70
        let fire_min = get_output_f64(&env.player.output, "FireMin");
        assert!(
            (fire_min - 70.0).abs() < 1.0,
            "FireMin should be ~70 (100 * (1 - 0.30)), got {}",
            fire_min
        );
    }

    #[test]
    fn dps_assembly_total_equals_avg_dmg_times_speed() {
        let player_db = ModDb::new();
        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        let mut env = CalcEnv::new(player_db, ModDb::new(), Arc::new(game_data));

        env.player.main_skill = Some(crate::build::types::ActiveSkill {
            skill_id: "TestSpell".into(),
            level: 20,
            quality: 0,
            skill_mod_db: ModDb::new(),
            is_attack: false,
            is_spell: true,
            is_melee: false,
            can_crit: false,
            base_crit_chance: 0.0,
            base_damage: HashMap::from([("Fire".to_string(), (100.0, 100.0))]),
            attack_speed_base: 0.0,
            cast_time: 0.5,
            damage_effectiveness: 1.0,
            skill_types: vec![],
            skill_flags: HashMap::new(),
            skill_cfg: None,
            slot_name: None,
            support_list: vec![],
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            target_version: "3_29".into(),
            passive_spec: Default::default(),
            skill_sets: vec![],
            active_skill_set: 0,
            main_socket_group: 0,
            item_sets: vec![],
            active_item_set: 0,
            config: Default::default(),
            items: HashMap::new(),
        };

        run(&mut env, &build);

        let avg_dmg = get_output_f64(&env.player.output, "AverageDamage");
        let speed = get_output_f64(&env.player.output, "Speed");
        let total_dps = get_output_f64(&env.player.output, "TotalDPS");

        assert!(
            (total_dps - avg_dmg * speed).abs() < 0.01,
            "TotalDPS ({}) should equal AverageDamage ({}) * Speed ({})",
            total_dps,
            avg_dmg,
            speed
        );
    }
}
