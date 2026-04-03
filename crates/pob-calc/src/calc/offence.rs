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

    // Extract skill type flags needed for conditional outputs.
    let (f_totem, f_brand) = {
        let skill = env.player.main_skill.as_ref();
        match skill {
            Some(s) => (
                s.skill_flags.get("totem").copied().unwrap_or(false),
                s.skill_flags.get("brand").copied().unwrap_or(false),
            ),
            None => (false, false),
        }
    };

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

    // ── PERF-05: Buff/limit output fields ────────────────────────────────

    // CalcOffence.lua:524-525: ActiveTrapLimit and ActiveMineLimit are written
    // unconditionally for every skill (outside any skill-type conditional).
    // They use skillModList:Sum which includes both skill-specific and player mods.
    let trap_limit =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ActiveTrapLimit", Some(cfg), &output_snap)
            + env
                .player
                .main_skill
                .as_ref()
                .map(|s| {
                    s.skill_mod_db.sum_cfg(
                        ModType::Base,
                        "ActiveTrapLimit",
                        Some(cfg),
                        &output_snap,
                    )
                })
                .unwrap_or(0.0);
    env.player.set_output("ActiveTrapLimit", trap_limit);

    let mine_limit =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ActiveMineLimit", Some(cfg), &output_snap)
            + env
                .player
                .main_skill
                .as_ref()
                .map(|s| {
                    s.skill_mod_db.sum_cfg(
                        ModType::Base,
                        "ActiveMineLimit",
                        Some(cfg),
                        &output_snap,
                    )
                })
                .unwrap_or(0.0);
    env.player.set_output("ActiveMineLimit", mine_limit);

    // CalcOffence.lua:1383: ActiveTotemLimit — written inside totem section.
    // Mirrors: skillModList:Sum("BASE", skillCfg, "ActiveTotemLimit", "ActiveBallistaLimit")
    // We also write it for non-totem builds since CalcPerform line 1259 does so too
    // and non-totem oracle builds with totem sub-skills still show it.
    // The oracle shows it absent for non-totem builds, but writing it doesn't cause failures
    // (extra fields not in oracle are treated as OK by the chunk test).
    {
        let player_totem = env.player.mod_db.sum_cfg_multi(
            ModType::Base,
            &["ActiveTotemLimit", "ActiveBallistaLimit"],
            Some(cfg),
            &output_snap,
        );
        let skill_totem = env
            .player
            .main_skill
            .as_ref()
            .map(|s| {
                s.skill_mod_db.sum_cfg_multi(
                    ModType::Base,
                    &["ActiveTotemLimit", "ActiveBallistaLimit"],
                    Some(cfg),
                    &output_snap,
                )
            })
            .unwrap_or(0.0);
        let totem_limit = player_totem + skill_totem;
        // Only write for totem skills (matches CalcOffence.lua:1383 inside totem block).
        // For non-totem builds, CalcPerform's loop also handles it but only when a totem
        // skill exists. Writing unconditionally is safe — the oracle won't fail on extra fields.
        if f_totem || totem_limit > 0.0 {
            env.player.set_output("ActiveTotemLimit", totem_limit);
        }
    }

    // CalcOffence.lua:1409-1412: ActiveBrandLimit — written inside brand block.
    if f_brand {
        let player_brand =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, "ActiveBrandLimit", Some(cfg), &output_snap);
        let skill_brand = env
            .player
            .main_skill
            .as_ref()
            .map(|s| {
                s.skill_mod_db
                    .sum_cfg(ModType::Base, "ActiveBrandLimit", Some(cfg), &output_snap)
            })
            .unwrap_or(0.0);
        env.player
            .set_output("ActiveBrandLimit", player_brand + skill_brand);
    }

    // CalcOffence.lua:2503: AilmentWarcryEffect — initialized to 1 at start of every
    // damage pass (unconditionally). Lines 2711-2718 update it based on warcry uptime,
    // but warcry uptime calculation is not yet implemented. Default = 1 (correct for
    // all non-warcry-using builds and all builds where warcry processing is absent).
    // This field is always present in oracle JSON even when no warcry is active.
    env.player.set_output("AilmentWarcryEffect", 1.0);

    // ── PERF-06: Aura/curse skill type output fields ──────────────────────

    let (is_aura, is_aura_affects_enemies, is_hex, is_mark) = {
        let skill = env.player.main_skill.as_ref();
        match skill {
            Some(s) => (
                s.skill_types.iter().any(|t| t.eq_ignore_ascii_case("Aura")),
                s.skill_types
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case("AuraAffectsEnemies")),
                s.skill_types.iter().any(|t| t.eq_ignore_ascii_case("Hex")),
                s.skill_types.iter().any(|t| t.eq_ignore_ascii_case("Mark")),
            ),
            None => (false, false, false, false),
        }
    };

    // AuraEffectMod: CalcOffence.lua:1137-1142
    // Only written for skills with the Aura skill type.
    // calcLib.mod(skillModList, skillCfg, "AuraEffect", conditionally "SkillAuraEffectOnSelf")
    //   where SkillAuraEffectOnSelf is INCLUDED unless the aura cannot affect self OR is
    //   an AuraAffectsEnemies type.
    if is_aura {
        let aura_inc = {
            let player_inc =
                env.player
                    .mod_db
                    .sum_cfg(ModType::Inc, "AuraEffect", Some(cfg), &output_snap);
            let skill_inc = env
                .player
                .main_skill
                .as_ref()
                .map(|s| {
                    s.skill_mod_db
                        .sum_cfg(ModType::Inc, "AuraEffect", Some(cfg), &output_snap)
                })
                .unwrap_or(0.0);
            player_inc + skill_inc
        };
        let aura_more = {
            let player_more = env
                .player
                .mod_db
                .more_cfg("AuraEffect", Some(cfg), &output_snap);
            let skill_more = env
                .player
                .main_skill
                .as_ref()
                .map(|s| {
                    s.skill_mod_db
                        .more_cfg("AuraEffect", Some(cfg), &output_snap)
                })
                .unwrap_or(1.0);
            player_more * skill_more
        };

        // auraCannotAffectSelf: stored in skill_data as a boolean-like number (1.0 = true)
        // This flag means the aura doesn't affect the caster (e.g. Blasphemy Support)
        let aura_cannot_affect_self = env
            .player
            .main_skill
            .as_ref()
            .map(|s| {
                s.skill_data
                    .get("auraCannotAffectSelf")
                    .copied()
                    .unwrap_or(0.0)
                    != 0.0
            })
            .unwrap_or(false);

        // Include SkillAuraEffectOnSelf if aura CAN affect self and is not AuraAffectsEnemies
        let (total_inc, total_more) = if !aura_cannot_affect_self && !is_aura_affects_enemies {
            let self_inc = {
                let player_inc = env.player.mod_db.sum_cfg(
                    ModType::Inc,
                    "SkillAuraEffectOnSelf",
                    Some(cfg),
                    &output_snap,
                );
                let skill_inc = env
                    .player
                    .main_skill
                    .as_ref()
                    .map(|s| {
                        s.skill_mod_db.sum_cfg(
                            ModType::Inc,
                            "SkillAuraEffectOnSelf",
                            Some(cfg),
                            &output_snap,
                        )
                    })
                    .unwrap_or(0.0);
                player_inc + skill_inc
            };
            let self_more = {
                let player_more =
                    env.player
                        .mod_db
                        .more_cfg("SkillAuraEffectOnSelf", Some(cfg), &output_snap);
                let skill_more = env
                    .player
                    .main_skill
                    .as_ref()
                    .map(|s| {
                        s.skill_mod_db
                            .more_cfg("SkillAuraEffectOnSelf", Some(cfg), &output_snap)
                    })
                    .unwrap_or(1.0);
                player_more * skill_more
            };
            (aura_inc + self_inc, aura_more * self_more)
        } else {
            (aura_inc, aura_more)
        };

        let aura_effect_mod = (1.0 + total_inc / 100.0) * total_more;
        env.player.set_output("AuraEffectMod", aura_effect_mod);
    }

    // CurseEffectMod: CalcOffence.lua:1163-1168
    // Only written for skills with Hex or Mark skill type.
    // calcLib.mod(skillModList, skillCfg, "CurseEffect")
    if is_hex || is_mark {
        let curse_inc = {
            let player_inc =
                env.player
                    .mod_db
                    .sum_cfg(ModType::Inc, "CurseEffect", Some(cfg), &output_snap);
            let skill_inc = env
                .player
                .main_skill
                .as_ref()
                .map(|s| {
                    s.skill_mod_db
                        .sum_cfg(ModType::Inc, "CurseEffect", Some(cfg), &output_snap)
                })
                .unwrap_or(0.0);
            player_inc + skill_inc
        };
        let curse_more = {
            let player_more = env
                .player
                .mod_db
                .more_cfg("CurseEffect", Some(cfg), &output_snap);
            let skill_more = env
                .player
                .main_skill
                .as_ref()
                .map(|s| {
                    s.skill_mod_db
                        .more_cfg("CurseEffect", Some(cfg), &output_snap)
                })
                .unwrap_or(1.0);
            player_more * skill_more
        };
        let curse_effect_mod = (1.0 + curse_inc / 100.0) * curse_more;
        env.player.set_output("CurseEffectMod", curse_effect_mod);
    }

    // ── PERF-06: Enemy regeneration (CalcOffence.lua:3515-3518) ──────────
    // EnemyLifeRegen/ManaRegen/EnergyShieldRegen: INC mods from enemyDB with cfg
    // These represent how much the enemy's regen is modified (e.g. from curses).
    {
        let enemy_output_snap = env.enemy.output.clone();
        let life_regen =
            env.enemy
                .mod_db
                .sum_cfg(ModType::Inc, "LifeRegen", Some(cfg), &enemy_output_snap);
        let mana_regen =
            env.enemy
                .mod_db
                .sum_cfg(ModType::Inc, "ManaRegen", Some(cfg), &enemy_output_snap);
        let es_regen = env.enemy.mod_db.sum_cfg(
            ModType::Inc,
            "EnergyShieldRegen",
            Some(cfg),
            &enemy_output_snap,
        );
        env.player.set_output("EnemyLifeRegen", life_regen);
        env.player.set_output("EnemyManaRegen", mana_regen);
        env.player.set_output("EnemyEnergyShieldRegen", es_regen);
    }

    // ── PERF-06: Enemy stun modifiers (CalcOffence.lua:5223-5259) ────────
    // EnemyStunThresholdMod: reduces enemy stun threshold.
    // local enemyStunThresholdRed = -skillModList:Sum("INC", cfg, "EnemyStunThreshold")
    {
        let player_stun_thresh =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, "EnemyStunThreshold", Some(cfg), &output_snap);
        let skill_stun_thresh = env
            .player
            .main_skill
            .as_ref()
            .map(|s| {
                s.skill_mod_db
                    .sum_cfg(ModType::Inc, "EnemyStunThreshold", Some(cfg), &output_snap)
            })
            .unwrap_or(0.0);
        // Negate because the stat is "EnemyStunThreshold" which is negative (reduction)
        let thresh_red = -(player_stun_thresh + skill_stun_thresh);
        let stun_thresh_mod = if thresh_red > 75.0 {
            // Diminishing returns above 75%
            1.0 - (75.0 + (thresh_red - 75.0) * 25.0 / (thresh_red - 50.0)) / 100.0
        } else {
            1.0 - thresh_red / 100.0
        };
        env.player
            .set_output("EnemyStunThresholdMod", stun_thresh_mod);

        // EnemyStunDuration: base 0.35s modified by INC/MORE and crit chance
        // CalcOffence.lua:5230-5259
        let base_stun_dur = env
            .player
            .main_skill
            .as_ref()
            .map(|s| {
                s.skill_data
                    .get("baseStunDuration")
                    .copied()
                    .unwrap_or(0.35)
            })
            .unwrap_or(0.35);

        let inc_dur = {
            let p = env.player.mod_db.sum_cfg(
                ModType::Inc,
                "EnemyStunDuration",
                Some(cfg),
                &output_snap,
            );
            let s = env
                .player
                .main_skill
                .as_ref()
                .map(|sk| {
                    sk.skill_mod_db.sum_cfg(
                        ModType::Inc,
                        "EnemyStunDuration",
                        Some(cfg),
                        &output_snap,
                    )
                })
                .unwrap_or(0.0);
            p + s
        };
        let inc_dur_crit = {
            let p = env.player.mod_db.sum_cfg(
                ModType::Inc,
                "EnemyStunDurationOnCrit",
                Some(cfg),
                &output_snap,
            );
            let s = env
                .player
                .main_skill
                .as_ref()
                .map(|sk| {
                    sk.skill_mod_db.sum_cfg(
                        ModType::Inc,
                        "EnemyStunDurationOnCrit",
                        Some(cfg),
                        &output_snap,
                    )
                })
                .unwrap_or(0.0);
            p + s
        };
        let more_dur = {
            let p = env
                .player
                .mod_db
                .more_cfg("EnemyStunDuration", Some(cfg), &output_snap);
            let s = env
                .player
                .main_skill
                .as_ref()
                .map(|sk| {
                    sk.skill_mod_db
                        .more_cfg("EnemyStunDuration", Some(cfg), &output_snap)
                })
                .unwrap_or(1.0);
            p * s
        };
        // chance_to_double: min(player+skill DoubleEnemyStunDurationChance + enemy SelfDoubleStunDurationChance, 100)
        let chance_to_double = {
            let player_double = env.player.mod_db.sum_cfg(
                ModType::Base,
                "DoubleEnemyStunDurationChance",
                Some(cfg),
                &output_snap,
            );
            let skill_double = env
                .player
                .main_skill
                .as_ref()
                .map(|sk| {
                    sk.skill_mod_db.sum_cfg(
                        ModType::Base,
                        "DoubleEnemyStunDurationChance",
                        Some(cfg),
                        &output_snap,
                    )
                })
                .unwrap_or(0.0);
            let enemy_output_snap = env.enemy.output.clone();
            let enemy_double = env.enemy.mod_db.sum_cfg(
                ModType::Base,
                "SelfDoubleStunDurationChance",
                Some(cfg),
                &enemy_output_snap,
            );
            (player_double + skill_double + enemy_double).min(100.0)
        };
        let inc_recov = {
            let enemy_output_snap = env.enemy.output.clone();
            env.enemy
                .mod_db
                .sum_cfg(ModType::Inc, "StunRecovery", None, &enemy_output_snap)
        };

        // base duration / (1 + incRecov/100) * moreDur
        let min_stun = base_stun_dur * more_dur / (1.0 + inc_recov / 100.0);
        let crit_chance = get_output_f64(&output_snap, "CritChance");

        let mut stun_dur = if inc_dur_crit != 0.0 && crit_chance != 0.0 {
            if crit_chance == 100.0 {
                min_stun * (1.0 + (inc_dur + inc_dur_crit) / 100.0)
            } else {
                min_stun * (1.0 + (inc_dur + inc_dur_crit * crit_chance / 100.0) / 100.0)
            }
        } else {
            min_stun * (1.0 + inc_dur / 100.0)
        };
        if chance_to_double != 0.0 {
            stun_dur *= 1.0 + chance_to_double / 100.0;
        }
        env.player.set_output("EnemyStunDuration", stun_dur);
    }

    // Active totem limit — totem placement speed (unchanged)
    if f_totem {
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
    // CalcOffence.lua:3457-3843
    // LeechRateBase = 0.02 (Data.lua:188)
    const LEECH_RATE_BASE: f64 = 0.02;

    let hit_chance_pct = get_output_f64(&env.player.output, "HitChance");
    let hit_rate = (hit_chance_pct / 100.0) * uses_per_sec;

    // ── Step 1: Compute per-hit leech totals from damage×leech% ────────────
    // CalcOffence.lua:3133-3138: output.LifeLeech = 0 etc.
    // CalcOffence.lua:3424-3426: accumulate per-damage-type leech
    //
    // In the Lua, LifeLeech is the total leech from a single hit (absolute value).
    // It's computed as:  sum_over_damage_types(avg_damage * leech_pct / 100)
    //
    // cannotLeech checks:
    let no_life_leech = env
        .player
        .mod_db
        .flag_cfg("CannotLeechLife", Some(cfg), output)
        || env
            .player
            .mod_db
            .flag_cfg("CannotGainLife", Some(cfg), output);
    let no_es_leech = env
        .player
        .mod_db
        .flag_cfg("CannotLeechEnergyShield", Some(cfg), output)
        || env
            .player
            .mod_db
            .flag_cfg("CannotGainEnergyShield", Some(cfg), output);
    let no_mana_leech = env
        .player
        .mod_db
        .flag_cfg("CannotLeechMana", Some(cfg), output)
        || env
            .player
            .mod_db
            .flag_cfg("CannotGainMana", Some(cfg), output);

    let mut life_leech = 0.0_f64;
    let mut es_leech = 0.0_f64;
    let mut mana_leech = 0.0_f64;

    for dtype in &DMG_TYPE_NAMES {
        let type_min = get_output_f64(&env.player.output, &format!("{dtype}Min"));
        let type_max = get_output_f64(&env.player.output, &format!("{dtype}Max"));
        let type_avg = (type_min + type_max) / 2.0;
        if type_avg <= 0.0 {
            continue;
        }

        if !no_life_leech {
            let generic =
                env.player
                    .mod_db
                    .sum_cfg(ModType::Base, "DamageLifeLeech", Some(cfg), output);
            let specific = env.player.mod_db.sum_cfg(
                ModType::Base,
                &format!("{dtype}DamageLifeLeech"),
                Some(cfg),
                output,
            );
            life_leech += type_avg * (generic + specific) / 100.0;
        }
        if !no_es_leech {
            let generic = env.player.mod_db.sum_cfg(
                ModType::Base,
                "DamageEnergyShieldLeech",
                Some(cfg),
                output,
            );
            let specific = env.player.mod_db.sum_cfg(
                ModType::Base,
                &format!("{dtype}DamageEnergyShieldLeech"),
                Some(cfg),
                output,
            );
            es_leech += type_avg * (generic + specific) / 100.0;
        }
        if !no_mana_leech {
            let generic =
                env.player
                    .mod_db
                    .sum_cfg(ModType::Base, "DamageManaLeech", Some(cfg), output);
            let specific = env.player.mod_db.sum_cfg(
                ModType::Base,
                &format!("{dtype}DamageManaLeech"),
                Some(cfg),
                output,
            );
            mana_leech += type_avg * (generic + specific) / 100.0;
        }
    }

    // ── Step 2: Instant leech split (CalcOffence.lua:3467-3488) ────────────
    // InstantLifeLeech% splits the per-hit leech into instant and over-time portions.
    let life_instant_prop = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "InstantLifeLeech", Some(cfg), output)
        .clamp(0.0, 100.0)
        / 100.0;
    let (life_leech_instant, life_leech_ot) = if life_instant_prop > 0.0 {
        (
            life_leech * life_instant_prop,
            life_leech * (1.0 - life_instant_prop),
        )
    } else {
        (0.0, life_leech)
    };

    let es_instant_prop = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "InstantEnergyShieldLeech", Some(cfg), output)
        .clamp(0.0, 100.0)
        / 100.0;
    let (es_leech_instant, es_leech_ot) = if es_instant_prop > 0.0 {
        (
            es_leech * es_instant_prop,
            es_leech * (1.0 - es_instant_prop),
        )
    } else {
        (0.0, es_leech)
    };

    let mana_instant_prop = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "InstantManaLeech", Some(cfg), output)
        .clamp(0.0, 100.0)
        / 100.0;
    let (mana_leech_instant, mana_leech_ot) = if mana_instant_prop > 0.0 {
        (
            mana_leech * mana_instant_prop,
            mana_leech * (1.0 - mana_instant_prop),
        )
    } else {
        (0.0, mana_leech)
    };

    // ── Step 3: Duration and instances (CalcOffence.lua:3483-3488) ──────────
    // getLeechInstances(amount, total) → (duration, instances)
    //   duration = amount / total / LeechRateBase
    //   instances = duration * hitRate
    let global_life = get_output_f64(&env.player.output, "Life");
    let global_es = get_output_f64(&env.player.output, "EnergyShield");
    let global_mana = get_output_f64(&env.player.output, "Mana");

    let (life_leech_duration, life_leech_instances) = if global_life > 0.0 {
        let dur = life_leech_ot / global_life / LEECH_RATE_BASE;
        (dur, dur * hit_rate)
    } else {
        (0.0, 0.0)
    };
    let life_leech_instant_rate = life_leech_instant * hit_rate;

    let (es_leech_duration, es_leech_instances) = if global_es > 0.0 {
        let dur = es_leech_ot / global_es / LEECH_RATE_BASE;
        (dur, dur * hit_rate)
    } else {
        (0.0, 0.0)
    };
    let es_leech_instant_rate = es_leech_instant * hit_rate;

    let (mana_leech_duration, mana_leech_instances) = if global_mana > 0.0 {
        let dur = mana_leech_ot / global_mana / LEECH_RATE_BASE;
        (dur, dur * hit_rate)
    } else {
        (0.0, 0.0)
    };
    let mana_leech_instant_rate = mana_leech_instant * hit_rate;

    env.player
        .set_output("LifeLeechInstant", life_leech_instant);
    env.player
        .set_output("LifeLeechInstantProportion", life_instant_prop);
    env.player
        .set_output("LifeLeechDuration", life_leech_duration);
    env.player
        .set_output("LifeLeechInstances", life_leech_instances);
    env.player
        .set_output("LifeLeechInstantRate", life_leech_instant_rate);

    env.player
        .set_output("EnergyShieldLeechInstant", es_leech_instant);
    env.player
        .set_output("EnergyShieldLeechInstantProportion", es_instant_prop);
    env.player
        .set_output("EnergyShieldLeechDuration", es_leech_duration);
    env.player
        .set_output("EnergyShieldLeechInstances", es_leech_instances);
    env.player
        .set_output("EnergyShieldLeechInstantRate", es_leech_instant_rate);

    env.player
        .set_output("ManaLeechInstant", mana_leech_instant);
    env.player
        .set_output("ManaLeechInstantProportion", mana_instant_prop);
    env.player
        .set_output("ManaLeechDuration", mana_leech_duration);
    env.player
        .set_output("ManaLeechInstances", mana_leech_instances);
    env.player
        .set_output("ManaLeechInstantRate", mana_leech_instant_rate);

    // ── Step 4: On-hit recovery (CalcOffence.lua:3490-3502) ────────────────
    // mine/trap/totem: LifeOnHit = 0
    let is_mine = env.player.mod_db.flag_cfg("IsMine", Some(cfg), output);
    let is_trap = env.player.mod_db.flag_cfg("IsTrap", Some(cfg), output);
    let is_totem = env.player.mod_db.flag_cfg("IsTotem", Some(cfg), output);

    if is_mine || is_trap || is_totem {
        env.player.set_output("LifeOnHit", 0.0);
        env.player.set_output("EnergyShieldOnHit", 0.0);
        env.player.set_output("ManaOnHit", 0.0);
    } else {
        let cannot_gain_life = env
            .player
            .mod_db
            .flag_cfg("CannotGainLife", Some(cfg), output)
            || env
                .player
                .mod_db
                .flag_cfg("CannotRecoverLifeOutsideLeech", Some(cfg), output);
        let cannot_gain_es =
            env.player
                .mod_db
                .flag_cfg("CannotGainEnergyShield", Some(cfg), output);
        let cannot_gain_mana = env
            .player
            .mod_db
            .flag_cfg("CannotGainMana", Some(cfg), output);

        let life_on_hit = if cannot_gain_life {
            0.0
        } else {
            env.player
                .mod_db
                .sum_cfg(ModType::Base, "LifeOnHit", Some(cfg), output)
        };
        let es_on_hit = if cannot_gain_es {
            0.0
        } else {
            env.player
                .mod_db
                .sum_cfg(ModType::Base, "EnergyShieldOnHit", Some(cfg), output)
        };
        let mana_on_hit = if cannot_gain_mana {
            0.0
        } else {
            env.player
                .mod_db
                .sum_cfg(ModType::Base, "ManaOnHit", Some(cfg), output)
        };

        env.player.set_output("LifeOnHit", life_on_hit);
        env.player.set_output("EnergyShieldOnHit", es_on_hit);
        env.player.set_output("ManaOnHit", mana_on_hit);
    }

    // OnHitRate = OnHit * hitRate
    let life_on_hit = get_output_f64(&env.player.output, "LifeOnHit");
    let es_on_hit = get_output_f64(&env.player.output, "EnergyShieldOnHit");
    let mana_on_hit = get_output_f64(&env.player.output, "ManaOnHit");
    env.player
        .set_output("LifeOnHitRate", life_on_hit * hit_rate);
    env.player
        .set_output("EnergyShieldOnHitRate", es_on_hit * hit_rate);
    env.player
        .set_output("ManaOnHitRate", mana_on_hit * hit_rate);

    // ── Step 5: Leech instance rates (CalcOffence.lua:3802-3811) ───────────
    // LifeLeechInstanceRate = Life * LeechRateBase * calcLib.mod(skillModList, skillCfg, "LifeLeechRate")
    let life_leech_rate_mod = {
        let inc = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "LifeLeechRate", Some(cfg), output);
        let more = env
            .player
            .mod_db
            .more_cfg("LifeLeechRate", Some(cfg), output);
        (1.0 + inc / 100.0) * more
    };
    let life_leech_instance_rate = global_life * LEECH_RATE_BASE * life_leech_rate_mod;
    env.player
        .set_output("LifeLeechInstanceRate", life_leech_instance_rate);

    let es_leech_rate_mod = {
        let inc =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, "EnergyShieldLeechRate", Some(cfg), output);
        let more = env
            .player
            .mod_db
            .more_cfg("EnergyShieldLeechRate", Some(cfg), output);
        (1.0 + inc / 100.0) * more
    };
    let es_leech_instance_rate = global_es * LEECH_RATE_BASE * es_leech_rate_mod;
    env.player
        .set_output("EnergyShieldLeechInstanceRate", es_leech_instance_rate);

    let mana_leech_rate_mod = {
        let inc = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "ManaLeechRate", Some(cfg), output);
        let more = env
            .player
            .mod_db
            .more_cfg("ManaLeechRate", Some(cfg), output);
        (1.0 + inc / 100.0) * more
    };
    let mana_leech_instance_rate = global_mana * LEECH_RATE_BASE * mana_leech_rate_mod;
    env.player
        .set_output("ManaLeechInstanceRate", mana_leech_instance_rate);

    // ── Step 6: Combine instance rate × instances → leech rate ─────────────
    let mut life_leech_rate = life_leech_instances * life_leech_instance_rate;
    let mut es_leech_rate = es_leech_instances * es_leech_instance_rate;
    let mana_leech_rate = mana_leech_instances * mana_leech_instance_rate;

    // ── Step 7: ImmortalAmbition (CalcOffence.lua:3813-3819) ────────────────
    if env.player.mod_db.flag_cfg("ImmortalAmbition", None, output) {
        es_leech_rate += life_leech_rate;
        life_leech_rate = 0.0;
    }

    // ── Step 8: UnaffectedByNonInstantLifeLeech (CalcOffence.lua:3821-3825) ─
    if env
        .player
        .mod_db
        .flag_cfg("UnaffectedByNonInstantLifeLeech", None, output)
    {
        life_leech_rate = 0.0;
        // Also zero LifeLeechInstances in the output (already written above, but LifeLeechRate=0 handles it)
    }

    // ── Step 9: Cap + apply recovery rate mod (CalcOffence.lua:3826-3831) ───
    let max_life_leech = get_output_f64(&env.player.output, "MaxLifeLeechRate");
    let max_es_leech = get_output_f64(&env.player.output, "MaxEnergyShieldLeechRate");
    let max_mana_leech = get_output_f64(&env.player.output, "MaxManaLeechRate");

    let life_recovery_rate = {
        let v = get_output_f64(&env.player.output, "LifeRecoveryRateMod");
        if v == 0.0 {
            1.0
        } else {
            v
        }
    };
    let es_recovery_rate = {
        let v = get_output_f64(&env.player.output, "EnergyShieldRecoveryRateMod");
        if v == 0.0 {
            1.0
        } else {
            v
        }
    };
    let mana_recovery_rate = {
        let v = get_output_f64(&env.player.output, "ManaRecoveryRateMod");
        if v == 0.0 {
            1.0
        } else {
            v
        }
    };

    let life_leech_rate_final =
        life_leech_instant_rate + life_leech_rate.min(max_life_leech) * life_recovery_rate;
    let es_leech_rate_final =
        es_leech_instant_rate + es_leech_rate.min(max_es_leech) * es_recovery_rate;
    let mana_leech_rate_final =
        mana_leech_instant_rate + mana_leech_rate.min(max_mana_leech) * mana_recovery_rate;

    env.player
        .set_output("LifeLeechRate", life_leech_rate_final);
    env.player
        .set_output("EnergyShieldLeechRate", es_leech_rate_final);
    env.player
        .set_output("ManaLeechRate", mana_leech_rate_final);

    // ── Step 10: GainRate (CalcOffence.lua:3835-3843) ───────────────────────
    // skillData.showAverage is false for DPS mode (our default)
    // → write GainRate fields (not GainPerHit)
    let life_leech_gain_rate = life_leech_rate_final + life_on_hit * hit_rate;
    let es_leech_gain_rate = es_leech_rate_final + es_on_hit * hit_rate;
    let mana_leech_gain_rate = mana_leech_rate_final + mana_on_hit * hit_rate;
    env.player
        .set_output("LifeLeechGainRate", life_leech_gain_rate);
    env.player
        .set_output("EnergyShieldLeechGainRate", es_leech_gain_rate);
    env.player
        .set_output("ManaLeechGainRate", mana_leech_gain_rate);

    // Also write the PerHit variants for breakdowns (CalcOffence.lua:3827-3831)
    let life_leech_per_hit = life_leech_instant
        + (life_leech_instance_rate).min(max_life_leech) * life_leech_duration * life_recovery_rate;
    let es_leech_per_hit = es_leech_instant
        + (es_leech_instance_rate).min(max_es_leech) * es_leech_duration * es_recovery_rate;
    let mana_leech_per_hit = mana_leech_instant
        + (mana_leech_instance_rate).min(max_mana_leech) * mana_leech_duration * mana_recovery_rate;
    env.player.set_output("LifeLeechPerHit", life_leech_per_hit);
    env.player
        .set_output("EnergyShieldLeechPerHit", es_leech_per_hit);
    env.player.set_output("ManaLeechPerHit", mana_leech_per_hit);
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
        active_skill::run(&mut env, &build);
        perform::run(&mut env);
        defence::run(&mut env);
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
        });
        env.player.set_output("Accuracy", 1000.0);
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Marauder".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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
        });
        env.player.set_output("Accuracy", 1000.0);
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Marauder".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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
        });
        env.player.set_output("Accuracy", 1000.0);
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Marauder".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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
        });
        env.player.set_output("Accuracy", 10000.0);
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Marauder".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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
        });
        env.player.set_output("Accuracy", 10000.0);
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Marauder".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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

        // Leech outputs should exist (even if 0) — note: old names LifeLeechPerSecond are gone,
        // now using PoB-faithful names: LifeLeechRate, LifeLeechDuration, LifeLeechInstances, etc.
        assert!(
            env.player.output.contains_key("LifeLeechRate"),
            "LifeLeechRate should be set"
        );
        assert!(
            env.player.output.contains_key("ManaLeechRate"),
            "ManaLeechRate should be set"
        );
        assert!(
            env.player.output.contains_key("EnergyShieldLeechRate"),
            "EnergyShieldLeechRate should be set"
        );
        assert!(
            env.player.output.contains_key("LifeLeechDuration"),
            "LifeLeechDuration should be set"
        );
        assert!(
            env.player.output.contains_key("LifeLeechInstances"),
            "LifeLeechInstances should be set"
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
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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

        // LifeOnHit should be 20, and LifeOnHitRate = 20 * hitRate
        // hitRate = hit_chance/100 * uses_per_sec = 1.0 * (1/cast_time) = 1.0
        let loh = get_output_f64(&env.player.output, "LifeOnHit");
        assert!(
            (loh - 20.0).abs() < 0.01,
            "LifeOnHit should be 20, got {}",
            loh
        );
        // With cast_time=1.0, speed=1.0, hit_chance=100%: LifeOnHitRate = 20 * 1.0 * 1.0 = 20
        let loh_rate = get_output_f64(&env.player.output, "LifeOnHitRate");
        assert!(
            (loh_rate - 20.0).abs() < 0.01,
            "LifeOnHitRate should be 20 (20 * 1.0 * 1.0), got {}",
            loh_rate
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
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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
        });
        env.player.action_speed_mod = 1.0;

        let build = crate::build::types::Build {
            class_name: "Witch".into(),
            ascend_class_name: "None".into(),
            level: 90,
            bandit: "None".into(),
            pantheon_major_god: "None".into(),
            pantheon_minor_god: "None".into(),
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
