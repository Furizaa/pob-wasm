use super::defence::{armour_reduction, DMG_CHAOS, DMG_PHYSICAL, DMG_TYPE_NAMES, DMG_FIRE, DMG_COLD, DMG_LIGHTNING};
use super::env::{get_output_f64, CalcEnv};
use crate::mod_db::types::ModType;

fn is_elemental(idx: usize) -> bool {
    idx == DMG_FIRE || idx == DMG_COLD || idx == DMG_LIGHTNING
}

// ── Orchestrator ─────────────────────────────────────────────────────────────

pub fn run(env: &mut CalcEnv) {
    calc_not_hit_chances(env);
    calc_enemy_damage(env);
    calc_damage_taken_as(env);
    calc_damage_taken_mult(env);
    calc_incoming_hit_damage(env);
    calc_life_recoverable(env);
    calc_prevented_life_loss(env);
    calc_es_bypass(env);
    calc_mind_over_matter(env);
    calc_guard(env);
    calc_aegis(env);
    calc_frost_shield_and_allies(env);
    calc_total_pool(env);
    calc_number_of_hits_to_die(env);
    calc_total_ehp(env);
    calc_max_hit_taken(env);
    calc_dot_ehp(env);
    calc_build_degen(env);
}

// ── 1. Not-hit chances (L1635-1656) ──────────────────────────────────────────

fn calc_not_hit_chances(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    let melee_evade = get_output_f64(&output, "MeleeEvadeChance");
    let proj_evade = get_output_f64(&output, "ProjectileEvadeChance");
    let attack_dodge = get_output_f64(&output, "EffectiveAttackDodgeChance");
    let spell_dodge = get_output_f64(&output, "EffectiveSpellDodgeChance");
    let avoid_all = get_output_f64(&output, "AvoidAllDamageFromHitsChance");
    let avoid_proj = get_output_f64(&output, "AvoidProjectilesChance");
    let specific_type_avoidance = env
        .player
        .output
        .get("specificTypeAvoidance")
        .map(|v| matches!(v, crate::calc::env::OutputValue::Bool(true)))
        .unwrap_or(false);

    // Lua:1638 MeleeNotHitChance = 100 - (1-evade/100)*(1-dodge/100)*(1-avoidAll/100)*100
    let melee_not_hit = (1.0
        - (1.0 - melee_evade / 100.0) * (1.0 - attack_dodge / 100.0) * (1.0 - avoid_all / 100.0))
        * 100.0;

    // Lua:1639 ProjectileNotHitChance: adds AvoidProjectilesChance if !specificTypeAvoidance
    let proj_avoid = if specific_type_avoidance {
        0.0
    } else {
        avoid_proj
    };
    let proj_not_hit = (1.0
        - (1.0 - proj_evade / 100.0)
            * (1.0 - attack_dodge / 100.0)
            * (1.0 - avoid_all / 100.0)
            * (1.0 - proj_avoid / 100.0))
        * 100.0;

    // Lua:1640 SpellNotHitChance: spell dodge + avoidAll (no evasion)
    let spell_not_hit =
        (1.0 - (1.0 - spell_dodge / 100.0) * (1.0 - avoid_all / 100.0)) * 100.0;

    // Lua:1641 SpellProjectileNotHitChance: spell dodge + avoidAll + proj avoidance
    let spell_proj_not_hit = (1.0
        - (1.0 - spell_dodge / 100.0)
            * (1.0 - avoid_all / 100.0)
            * (1.0 - proj_avoid / 100.0))
        * 100.0;

    // Lua:1642 UntypedNotHitChance = avoidAll only
    let untyped_not_hit = avoid_all;

    let avg_not_hit =
        (melee_not_hit + proj_not_hit + spell_not_hit + spell_proj_not_hit) / 4.0;

    // Lua:1644 AverageEvadeChance = (MeleeEvadeChance + ProjectileEvadeChance) / 4
    let avg_evade = (melee_evade + proj_evade) / 4.0;

    env.player.set_output("MeleeNotHitChance", melee_not_hit);
    env.player.set_output("ProjectileNotHitChance", proj_not_hit);
    env.player.set_output("SpellNotHitChance", spell_not_hit);
    env.player
        .set_output("SpellProjectileNotHitChance", spell_proj_not_hit);
    env.player.set_output("UntypedNotHitChance", untyped_not_hit);
    env.player.set_output("AverageNotHitChance", avg_not_hit);
    env.player.set_output("AverageEvadeChance", avg_evade);

    // Lua:1645-1646 ConfiguredNotHitChance/ConfiguredEvadeChance
    // damageCategoryConfig defaults to "Average"
    env.player.set_output("ConfiguredNotHitChance", avg_not_hit);
    env.player.set_output("ConfiguredEvadeChance", avg_evade);

    // Lua:1089-1094 noSplitEvade: true when melee and projectile evade are equal
    if (melee_evade - proj_evade).abs() < 0.001 {
        env.player.set_output_bool("noSplitEvade", true);
        env.player.set_output("EvadeChance", melee_evade);
    } else {
        env.player.set_output_bool("splitEvade", true);
    }
}

// ── 2. Enemy damage estimation (L1658-1790) ──────────────────────────────────

fn calc_enemy_damage(env: &mut CalcEnv) {
    let output = env.player.output.clone();
    let data = env.data.clone();

    // ── Determine boss type and compute per-type enemy damage placeholders ──
    // Lua: enemyIsBoss from configInput, with Sirus/Shaper/etc. mapped to Pinnacle
    let boss_type = env
        .config_strings
        .get("enemyIsBoss")
        .cloned()
        .unwrap_or_else(|| "Pinnacle".into());
    // Map legacy values
    let boss_type = match boss_type.as_str() {
        "Sirus" | "Shaper" => "Pinnacle",
        "Uber Atziri" => "Boss",
        other => other,
    };

    let enemy_level = env.enemy_level;
    // Lua: monsterDamageTable is 1-indexed; our Vec is 0-indexed
    // PoB level L maps to Lua table[L], which is our Vec index L-1
    let base_monster_dmg = if enemy_level >= 1 && enemy_level <= data.misc.monster_damage_table.len()
    {
        data.misc.monster_damage_table[enemy_level - 1]
    } else {
        // Fallback: level 84
        data.misc
            .monster_damage_table
            .get(83)
            .copied()
            .unwrap_or(821.73)
    };

    // Compute default per-type damage based on boss type
    // ConfigOptions.lua:2027-2131
    let pob = &data.misc.pob_misc;
    let (phys_default, ele_default, chaos_default) = match boss_type {
        "None" => {
            // Normal: only physical gets damage
            let dmg = (base_monster_dmg * 1.5).round();
            (dmg, 0.0, 0.0)
        }
        "Boss" => {
            let dmg = (base_monster_dmg * 1.5 * pob.std_boss_dps_mult).round();
            let chaos = (dmg / 2.5).round();
            (dmg, dmg, chaos)
        }
        "Pinnacle" => {
            let dmg = (base_monster_dmg * 1.5 * pob.pinnacle_boss_dps_mult).round();
            let chaos = (dmg / 2.5).round();
            (dmg, dmg, chaos)
        }
        "Uber" => {
            let dmg = (base_monster_dmg * 1.5 * pob.uber_boss_dps_mult).round();
            let chaos = (dmg / 2.5).round();
            (dmg, dmg, chaos)
        }
        _ => {
            // Default to Pinnacle
            let dmg = (base_monster_dmg * 1.5 * pob.pinnacle_boss_dps_mult).round();
            let chaos = (dmg / 2.5).round();
            (dmg, dmg, chaos)
        }
    };

    // Read per-type damage from config or use computed defaults
    // Lua: tonumber(env.configInput["enemy"..damageType.."Damage"]) or configPlaceholder
    let mut enemy_damage_per_type: [f64; 5] = [0.0; 5];
    for (i, type_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let config_key = format!("enemy{type_name}Damage");
        let configured = env.config_numbers.get(&config_key).copied();
        let placeholder = if i == DMG_PHYSICAL {
            phys_default
        } else if i == DMG_CHAOS {
            chaos_default
        } else {
            ele_default
        };
        enemy_damage_per_type[i] = configured.unwrap_or(placeholder);
    }

    // Enemy crit chance (L1684)
    let enemy_crit = if env.enemy.mod_db.flag_cfg("NeverCrit", None, &output) {
        0.0
    } else if env.enemy.mod_db.flag_cfg("AlwaysCrit", None, &output) {
        100.0
    } else {
        let override_val = env
            .player
            .mod_db
            .override_value("enemyCritChance", None, &output);
        let config_crit = override_val.unwrap_or_else(|| {
            env.config_numbers
                .get("enemyCritChance")
                .copied()
                .unwrap_or(5.0)
        });
        let inc_player = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "EnemyCritChance", None, &output);
        let inc_enemy = env
            .enemy
            .mod_db
            .sum_cfg(ModType::Inc, "CritChance", None, &output);
        let configured_evade = get_output_f64(&output, "ConfiguredEvadeChance");
        (config_crit
            * (1.0 + inc_player / 100.0 + inc_enemy / 100.0)
            * (1.0 - configured_evade / 100.0))
            .clamp(0.0, 100.0)
    };
    env.player.set_output("EnemyCritChance", enemy_crit);

    // Enemy crit damage multiplier (L1686-1687)
    let crit_mult_from_enemy = env
        .enemy
        .mod_db
        .sum_cfg(ModType::Base, "CritMultiplier", None, &output);
    let config_crit_damage = env
        .config_numbers
        .get("enemyCritDamage")
        .copied()
        .unwrap_or(30.0); // default = base_critical_strike_multiplier - 100 = 130-100 = 30
    let enemy_crit_damage = (config_crit_damage + crit_mult_from_enemy).max(0.0);
    let crit_dr = get_output_f64(&output, "CritExtraDamageReduction");
    let enemy_crit_effect =
        1.0 + enemy_crit / 100.0 * (enemy_crit_damage / 100.0) * (1.0 - crit_dr / 100.0);
    env.player.set_output("EnemyCritEffect", enemy_crit_effect);

    // Per-type enemy damage loop (L1690-1789)
    let mut total_enemy_dmg_in = 0.0;
    let mut total_enemy_dmg = 0.0;

    for (i, type_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let enemy_damage = enemy_damage_per_type[i];

        // Enemy damage multiplier: calcLib.mod(enemyDB, nil, "Damage", type.."Damage", ...)
        let enemy_dmg_mult = calc_def_mod(
            &env.enemy.mod_db,
            None,
            &output,
            &["Damage", &format!("{type_name}Damage")],
            if is_elemental(i) {
                Some("ElementalDamage")
            } else {
                None
            },
        );

        // Enemy pen from config (L1693)
        let config_pen_key = format!("enemy{type_name}Pen");
        let enemy_pen = env
            .config_numbers
            .get(&config_pen_key)
            .copied()
            .unwrap_or_else(|| {
                // Pinnacle bosses have default pen for elemental types
                if is_elemental(i) {
                    match boss_type {
                        "Pinnacle" => pob.pinnacle_boss_pen,
                        "Uber" => pob.uber_boss_pen,
                        _ => 0.0,
                    }
                } else {
                    0.0
                }
            });

        // Enemy overwhelm from config (L1694)
        let config_overwhelm_key = format!("enemy{type_name}Overwhelm");
        let mut enemy_overwhelm = env
            .config_numbers
            .get(&config_overwhelm_key)
            .copied()
            .unwrap_or(0.0);
        // L1753: add base overwhelm from enemy/player mods (physical only in practice)
        if i == DMG_PHYSICAL {
            enemy_overwhelm += env
                .enemy
                .mod_db
                .sum_cfg(ModType::Base, "PhysicalOverwhelm", None, &output);
            enemy_overwhelm += env
                .player
                .mod_db
                .sum_cfg(ModType::Base, "EnemyPhysicalOverwhelm", None, &output);
        }

        // No conversion for now (L1711-1751 — enemy conversion is rare, skip for simplicity)
        // totalEnemyDamageIn accumulates raw damage
        total_enemy_dmg_in += enemy_damage;

        // L1759: type_dmg = rawDmg * (1 - conversionTotal/100) * mult * critEffect
        let type_dmg = enemy_damage * enemy_dmg_mult * enemy_crit_effect;
        total_enemy_dmg += type_dmg;

        env.player
            .set_output(&format!("{type_name}EnemyPen"), enemy_pen);
        env.player
            .set_output(&format!("{type_name}EnemyDamageMult"), enemy_dmg_mult);
        env.player
            .set_output(&format!("{type_name}EnemyOverwhelm"), enemy_overwhelm);
        env.player
            .set_output(&format!("{type_name}EnemyDamage"), type_dmg);
    }

    env.player
        .set_output("totalEnemyDamageIn", total_enemy_dmg_in);
    env.player.set_output("totalEnemyDamage", total_enemy_dmg);

    // Enemy skill time (L2889-2891)
    let config_enemy_speed = env
        .config_numbers
        .get("enemySpeed")
        .copied()
        .unwrap_or(700.0);
    let enemy_speed_inc = env
        .enemy
        .mod_db
        .sum_cfg(ModType::Inc, "Speed", None, &output);
    let enemy_skill_time_ms = config_enemy_speed / (1.0 + enemy_speed_inc / 100.0);
    // Divide by 1000 for seconds, then by action speed mod
    // calcs.actionSpeedMod(actor.enemy) ≈ 1.0 for most builds
    let enemy_action_speed = calc_action_speed_mod(env);
    let enemy_skill_time = enemy_skill_time_ms / 1000.0 / enemy_action_speed;
    env.player.set_output("enemySkillTime", enemy_skill_time);
}

/// Simplified calcs.actionSpeedMod for the enemy.
/// Lua: calcs.actionSpeedMod reads modDB:Sum("INC", nil, "ActionSpeed") etc.
fn calc_action_speed_mod(env: &CalcEnv) -> f64 {
    let output = &env.enemy.output;
    let action_speed_inc = env
        .enemy
        .mod_db
        .sum_cfg(ModType::Inc, "ActionSpeed", None, output);
    (1.0 + action_speed_inc / 100.0).max(0.0)
}

/// calcLib.mod(db, cfg, names...) = (1 + Sum("INC", cfg, names) / 100) * More(cfg, names)
fn calc_def_mod(
    db: &crate::mod_db::ModDb,
    cfg: Option<&crate::mod_db::types::SkillCfg>,
    output: &crate::calc::env::OutputTable,
    names: &[&str],
    extra: Option<&str>,
) -> f64 {
    let mut inc = 0.0;
    let mut more = 1.0;
    for &name in names {
        inc += db.sum_cfg(ModType::Inc, name, cfg, output);
        more *= db.more_cfg(name, cfg, output);
    }
    if let Some(extra_name) = extra {
        inc += db.sum_cfg(ModType::Inc, extra_name, cfg, output);
        more *= db.more_cfg(extra_name, cfg, output);
    }
    (1.0 + inc / 100.0) * more
}

// ── 3. Damage taken as conversion (L1792-1868) ──────────────────────────────

fn calc_damage_taken_as(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // Use the pre-computed damage shift table from defence.rs
    // Apply per-type TakenDamage (after conversion)
    for (src_idx, src_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let src_dmg = get_output_f64(&output, &format!("{src_name}EnemyDamage"));
        let self_pct = env.player.damage_shift_table[src_idx][src_idx];
        let taken = src_dmg * self_pct / 100.0;
        env.player
            .set_output(&format!("{src_name}TakenDamage"), taken);
    }

    // Add converted amounts from other types
    for (src_idx, src_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let src_dmg = get_output_f64(&output, &format!("{src_name}EnemyDamage"));
        for (dst_idx, dst_name) in DMG_TYPE_NAMES.iter().enumerate() {
            if src_idx != dst_idx {
                let convert_pct = env.player.damage_shift_table[src_idx][dst_idx];
                if convert_pct > 0.0 {
                    let damage = src_dmg * convert_pct / 100.0;
                    let current = get_output_f64(&env.player.output, &format!("{dst_name}TakenDamage"));
                    env.player
                        .set_output(&format!("{dst_name}TakenDamage"), current + damage);
                }
            }
        }
    }

    // Total taken damage
    let output = env.player.output.clone();
    let mut total = 0.0;
    for type_name in DMG_TYPE_NAMES.iter() {
        total += get_output_f64(&output, &format!("{type_name}TakenDamage"));
    }
    env.player.set_output("totalTakenDamage", total);
}

// ── 4. Damage taken multipliers (L1870-1930) ─────────────────────────────────

fn calc_damage_taken_mult(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    env.player.set_output_bool("AnyTakenReflect", false);

    for (i, type_name) in DMG_TYPE_NAMES.iter().enumerate() {
        // L1873-1878: base taken INC/More (DamageTaken + TypeDamageTaken + ElementalDamageTaken)
        let type_taken_stat = format!("{type_name}DamageTaken");
        let mut base_taken_inc = env.player.mod_db.sum_cfg(ModType::Inc, "DamageTaken", None, &output)
            + env.player.mod_db.sum_cfg(ModType::Inc, &type_taken_stat, None, &output);
        let mut base_taken_more = env.player.mod_db.more_cfg("DamageTaken", None, &output)
            * env.player.mod_db.more_cfg(&type_taken_stat, None, &output);
        if is_elemental(i) {
            base_taken_inc += env.player.mod_db.sum_cfg(ModType::Inc, "ElementalDamageTaken", None, &output);
            base_taken_more *= env.player.mod_db.more_cfg("ElementalDamageTaken", None, &output);
        }

        // L1879-1886: Hit multiplier (shared baseline)
        let type_when_hit = format!("{type_name}DamageTakenWhenHit");
        let mut taken_inc = base_taken_inc
            + env.player.mod_db.sum_cfg(ModType::Inc, "DamageTakenWhenHit", None, &output)
            + env.player.mod_db.sum_cfg(ModType::Inc, &type_when_hit, None, &output);
        let mut taken_more = base_taken_more
            * env.player.mod_db.more_cfg("DamageTakenWhenHit", None, &output)
            * env.player.mod_db.more_cfg(&type_when_hit, None, &output);
        if is_elemental(i) {
            taken_inc += env.player.mod_db.sum_cfg(ModType::Inc, "ElementalDamageTakenWhenHit", None, &output);
            taken_more *= env.player.mod_db.more_cfg("ElementalDamageTakenWhenHit", None, &output);
        }
        // L1886: base type TakenHitMult (before attack/spell split)
        let type_taken_hit_mult = ((1.0 + taken_inc / 100.0) * taken_more).max(0.0);
        env.player
            .set_output(&format!("{type_name}TakenHitMult"), type_taken_hit_mult);

        // L1888-1893: Per-hit-source (Attack, Spell) multipliers
        for hit_type in &["Attack", "Spell"] {
            let hit_taken_stat = format!("{hit_type}DamageTaken");
            let base_taken_inc_type = taken_inc
                + env.player.mod_db.sum_cfg(ModType::Inc, &hit_taken_stat, None, &output);
            let base_taken_more_type = taken_more
                * env.player.mod_db.more_cfg(&hit_taken_stat, None, &output);
            let hit_type_mult = ((1.0 + base_taken_inc_type / 100.0) * base_taken_more_type).max(0.0);
            // L1891: output["AttackTakenHitMult"] / output["SpellTakenHitMult"]
            env.player
                .set_output(&format!("{hit_type}TakenHitMult"), hit_type_mult);
            // L1892: output["PhysicalAttackTakenHitMult"] etc.
            env.player
                .set_output(&format!("{type_name}{hit_type}TakenHitMult"), hit_type_mult);
        }

        // L1894-1906: Reflect multiplier
        let reflect_inc = taken_inc
            + env.player.mod_db.sum_cfg(ModType::Inc, "ReflectedDamageTaken", None, &output)
            + env.player.mod_db.sum_cfg(ModType::Inc, &format!("{type_name}ReflectedDamageTaken"), None, &output);
        let mut reflect_more = taken_more
            * env.player.mod_db.more_cfg("ReflectedDamageTaken", None, &output)
            * env.player.mod_db.more_cfg(&format!("{type_name}ReflectedDamageTaken"), None, &output);
        if is_elemental(i) {
            // L1899-1900
            let _reflect_inc_extra = env.player.mod_db.sum_cfg(ModType::Inc, "ElementalReflectedDamageTaken", None, &output);
            reflect_more *= env.player.mod_db.more_cfg("ElementalReflectedDamageTaken", None, &output);
        }
        let taken_reflect = ((1.0 + reflect_inc / 100.0) * reflect_more).max(0.0);
        env.player
            .set_output(&format!("{type_name}TakenReflect"), taken_reflect);
        // L1904: AnyTakenReflect = false (always — commented out in Lua)

        // L1908-1929: DoT taken mult
        let dot_taken_stat = format!("{type_name}DamageTakenOverTime");
        let mut dot_taken_inc = base_taken_inc
            + env.player.mod_db.sum_cfg(ModType::Inc, "DamageTakenOverTime", None, &output)
            + env.player.mod_db.sum_cfg(ModType::Inc, &dot_taken_stat, None, &output);
        let mut dot_taken_more = base_taken_more
            * env.player.mod_db.more_cfg("DamageTakenOverTime", None, &output)
            * env.player.mod_db.more_cfg(&dot_taken_stat, None, &output);
        if is_elemental(i) {
            dot_taken_inc += env.player.mod_db.sum_cfg(ModType::Inc, "ElementalDamageTakenOverTime", None, &output);
            dot_taken_more *= env.player.mod_db.more_cfg("ElementalDamageTakenOverTime", None, &output);
        }
        // L1915-1917: DoT mult includes resist and base DR
        let resist = if env.player.mod_db.flag_cfg(&format!("SelfIgnore{type_name}Resistance"), None, &output) {
            0.0
        } else {
            let rot = get_output_f64(&output, &format!("{type_name}ResistOverTime"));
            if rot != 0.0 { rot } else { get_output_f64(&output, &format!("{type_name}Resist")) }
        };
        let reduction = if env.player.mod_db.flag_cfg(&format!("SelfIgnoreBase{type_name}DamageReduction"), None, &output) {
            0.0
        } else {
            get_output_f64(&output, &format!("Base{type_name}DamageReduction"))
        };
        let dot_mult = ((1.0 - resist / 100.0) * (1.0 - reduction / 100.0)
            * (1.0 + dot_taken_inc / 100.0) * dot_taken_more).max(0.0);
        env.player
            .set_output(&format!("{type_name}TakenDotMult"), dot_mult);
    }
}

// ── 5. Incoming hit damage multipliers (L1932-2097) ──────────────────────────

fn calc_incoming_hit_damage(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // L1946: enemyImpaleChance (simplified: for Average config, half attack chance)
    // We skip impale for now as it's complex and rarely affects max hit

    let mut total_taken_hit = 0.0;
    for (i, type_name) in DMG_TYPE_NAMES.iter().enumerate() {
        // L1949-1952: resist, reduction, pen, overwhelm with SelfIgnore checks
        let resist = if env.player.mod_db.flag_cfg(&format!("SelfIgnore{type_name}Resistance"), None, &output) {
            0.0
        } else {
            let rwh = get_output_f64(&output, &format!("{type_name}ResistWhenHit"));
            if rwh != 0.0 { rwh } else { get_output_f64(&output, &format!("{type_name}Resist")) }
        };
        let reduction = if env.player.mod_db.flag_cfg(&format!("SelfIgnoreBase{type_name}DamageReduction"), None, &output) {
            0.0
        } else {
            let rwh = get_output_f64(&output, &format!("Base{type_name}DamageReductionWhenHit"));
            if rwh != 0.0 { rwh } else { get_output_f64(&output, &format!("Base{type_name}DamageReduction")) }
        };
        let enemy_pen = if env.player.mod_db.flag_cfg(&format!("SelfIgnore{type_name}Resistance"), None, &output)
            || env.player.mod_db.flag_cfg(&format!("EnemyCannotPen{type_name}Resistance"), None, &output)
        {
            0.0
        } else {
            get_output_f64(&output, &format!("{type_name}EnemyPen"))
        };
        let enemy_overwhelm = if env.player.mod_db.flag_cfg(&format!("SelfIgnore{type_name}DamageReduction"), None, &output) {
            0.0
        } else {
            get_output_f64(&output, &format!("{type_name}EnemyOverwhelm"))
        };

        let damage = get_output_f64(&output, &format!("{type_name}TakenDamage"));

        // L1957-1963: Armour applies
        let percent_of_armour_applies = if !env.player.mod_db.flag_cfg(&format!("ArmourDoesNotApplyTo{type_name}DamageTaken"), None, &output) {
            env.player.mod_db.sum_cfg(ModType::Base, &format!("ArmourAppliesTo{type_name}DamageTaken"), None, &output).min(100.0)
        } else {
            0.0
        };
        let armour_defense = get_output_f64(&output, "ArmourDefense");
        let mut effective_applied_armour = get_output_f64(&output, "Armour") * percent_of_armour_applies / 100.0 * (1.0 + armour_defense);

        // L1959-1963: PhysicalReductionBasedOnWard
        let phys_reduction_based_on_ward = i == DMG_PHYSICAL
            && env.player.mod_db.flag_cfg("PhysicalReductionBasedOnWard", None, &output);
        if phys_reduction_based_on_ward {
            let multiplier = env
                .player
                .mod_db
                .override_value("PhysicalReductionBasedOnWardPercent", None, &output)
                .unwrap_or(100.0) / 100.0;
            effective_applied_armour = get_output_f64(&output, "Ward") * multiplier;
        }

        let res_mult = 1.0 - (resist - enemy_pen) / 100.0;

        // L1966: takenFlat (BASE flat damage taken)
        let mut taken_flat = env.player.mod_db.sum_cfg(ModType::Base, "DamageTaken", None, &output)
            + env.player.mod_db.sum_cfg(ModType::Base, &format!("{type_name}DamageTaken"), None, &output)
            + env.player.mod_db.sum_cfg(ModType::Base, "DamageTakenWhenHit", None, &output)
            + env.player.mod_db.sum_cfg(ModType::Base, &format!("{type_name}DamageTakenWhenHit"), None, &output);
        // L1971-1972: for Average config, add attack/spell flat averaged
        taken_flat += env.player.mod_db.sum_cfg(ModType::Base, "DamageTakenFromAttacks", None, &output) / 2.0
            + env.player.mod_db.sum_cfg(ModType::Base, &format!("{type_name}DamageTakenFromAttacks"), None, &output) / 2.0
            + env.player.mod_db.sum_cfg(ModType::Base, &format!("{type_name}DamageTakenFromProjectileAttacks"), None, &output) / 4.0
            + env.player.mod_db.sum_cfg(ModType::Base, "DamageTakenFromSpells", None, &output) / 2.0
            + env.player.mod_db.sum_cfg(ModType::Base, &format!("{type_name}DamageTakenFromSpells"), None, &output) / 2.0
            + env.player.mod_db.sum_cfg(ModType::Base, "DamageTakenFromSpellProjectiles", None, &output) / 4.0
            + env.player.mod_db.sum_cfg(ModType::Base, &format!("{type_name}DamageTakenFromSpellProjectiles"), None, &output) / 4.0;
        env.player.set_output(&format!("{type_name}takenFlat"), taken_flat);

        // L1975-1983: armour reduction
        let mut armour_reduct = 0.0;
        let dr_max = get_output_f64(&output, "DamageReductionMax");
        if percent_of_armour_applies > 0.0 || phys_reduction_based_on_ward {
            armour_reduct = armour_reduction(effective_applied_armour, damage * res_mult);
            armour_reduct = armour_reduct.min(dr_max);
        }

        let total_reduct = (armour_reduct + reduction).min(dr_max);
        let reduct_mult = 1.0 - (total_reduct - enemy_overwhelm).clamp(0.0, dr_max) / 100.0;
        env.player
            .set_output(&format!("{type_name}DamageReduction"), 100.0 - reduct_mult * 100.0);

        // L2012-2022: select takenMult based on damage category (Average)
        let attack_taken = get_output_f64(&env.player.output, &format!("{type_name}AttackTakenHitMult"));
        let spell_taken = get_output_f64(&env.player.output, &format!("{type_name}SpellTakenHitMult"));
        let taken_mult = (spell_taken + attack_taken) / 2.0;

        // L2018-2021: spell suppression effect for Average config
        let eff_suppress = get_output_f64(&output, "EffectiveSpellSuppressionChance");
        let suppress_effect = get_output_f64(&output, "SpellSuppressionEffect");
        let spell_suppress_mult = if eff_suppress >= 100.0 {
            1.0 - suppress_effect / 100.0 / 2.0
        } else {
            1.0
        };

        env.player
            .set_output(&format!("{type_name}EffectiveAppliedArmour"), effective_applied_armour);
        env.player
            .set_output(&format!("{type_name}ResistTakenHitMulti"), res_mult);

        let after_reduction_multi = taken_mult * spell_suppress_mult;
        env.player
            .set_output(&format!("{type_name}AfterReductionTakenHitMulti"), after_reduction_multi);

        let base_mult = res_mult * reduct_mult;
        env.player
            .set_output(&format!("{type_name}BaseTakenHitMult"), base_mult * after_reduction_multi);

        // L2031: TakenHit = max(damage * baseMult + takenFlat, 0) * takenMult * spellSuppressMult
        let taken_hit = (damage * base_mult + taken_flat).max(0.0) * taken_mult * spell_suppress_mult;
        env.player
            .set_output(&format!("{type_name}TakenHit"), taken_hit);

        // L2032: overwrite TakenHitMult with complete multiplier
        let taken_hit_mult = if damage > 0.0 {
            taken_hit / damage
        } else {
            0.0
        };
        env.player
            .set_output(&format!("{type_name}TakenHitMult"), taken_hit_mult);

        total_taken_hit += taken_hit;
    }
    env.player.set_output("totalTakenHit", total_taken_hit);

    // L2099 area: stun — write enemyBlockChance = 0 for non-attack oracle builds
    env.player.set_output("enemyBlockChance", 0.0);
}

// ── 6. Life Recoverable (L2204-2218) ─────────────────────────────────────────
// (Already implemented in defence.rs, this is a no-op if already written)

fn calc_life_recoverable(env: &mut CalcEnv) {
    // Check if LifeRecoverable already written by defence.rs
    let existing = get_output_f64(&env.player.output, "LifeRecoverable");
    if existing > 0.0 {
        return; // Already computed
    }
    // Fallback: LifeRecoverable = LifeUnreserved
    let life_unreserved = get_output_f64(&env.player.output, "LifeUnreserved");
    env.player
        .set_output("LifeRecoverable", life_unreserved.max(1.0));
}

// ── 7. Prevented life loss / Petrified Blood (L2220-2266) ────────────────────

fn calc_prevented_life_loss(env: &mut CalcEnv) {
    let output = env.player.output.clone();
    let recoverable = get_output_f64(&output, "LifeRecoverable");
    let life = get_output_f64(&output, "Life");

    let prevented_life_loss = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "LifeLossPrevented", None, &output)
        .min(100.0);
    env.player
        .set_output("preventedLifeLoss", prevented_life_loss);

    let initial_below_half = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "LifeLossBelowHalfPrevented", None, &output);
    let prevented_below_half = (1.0 - prevented_life_loss / 100.0) * initial_below_half;
    env.player
        .set_output("preventedLifeLossBelowHalf", prevented_below_half);

    let condition_low_life = env
        .config_booleans
        .get("conditionLowLife")
        .copied()
        .unwrap_or(false);
    if !condition_low_life {
        let portion_life = (life * 0.5 / recoverable).min(1.0);
        env.player.set_output(
            "preventedLifeLossTotal",
            prevented_life_loss + prevented_below_half * portion_life,
        );
    } else {
        env.player.set_output(
            "preventedLifeLossTotal",
            prevented_life_loss + prevented_below_half,
        );
    }

    // L2235: LifeHitPool = calcLifeHitPoolWithLossPrevention(...)
    let life_hit_pool =
        calc_life_hit_pool_with_loss_prevention(recoverable, life, prevented_life_loss, initial_below_half);
    env.player.set_output("LifeHitPool", life_hit_pool);
}

/// Lua:152-156 calcLifeHitPoolWithLossPrevention
fn calc_life_hit_pool_with_loss_prevention(
    life: f64,
    max_life: f64,
    life_loss_prevented: f64,
    life_loss_below_half_prevented: f64,
) -> f64 {
    let half_life = max_life * 0.5;
    let above_low = (life - half_life).max(0.0);
    above_low / (1.0 - life_loss_prevented / 100.0)
        + life.min(half_life) / (1.0 - life_loss_below_half_prevented / 100.0)
            / (1.0 - life_loss_prevented / 100.0)
}

// ── 8. ES bypass / AnyBypass (L2268-2290) ────────────────────────────────────

fn calc_es_bypass(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    env.player.set_output_bool("AnyBypass", false);
    env.player.set_output("MinimumBypass", 100.0);

    let unblocked_bypasses = env
        .player
        .mod_db
        .flag_cfg("UnblockedDamageDoesBypassES", None, &output);

    for (i, type_name) in DMG_TYPE_NAMES.iter().enumerate() {
        if unblocked_bypasses {
            env.player
                .set_output(&format!("{type_name}EnergyShieldBypass"), 100.0);
            env.player.set_output_bool("AnyBypass", true);
        } else {
            let override_val = env.player.mod_db.override_value(
                &format!("{type_name}EnergyShieldBypass"),
                None,
                &output,
            );
            let mut bypass = override_val.unwrap_or_else(|| {
                env.player.mod_db.sum_cfg(
                    ModType::Base,
                    &format!("{type_name}EnergyShieldBypass"),
                    None,
                    &output,
                )
            });
            if bypass != 0.0 {
                env.player.set_output_bool("AnyBypass", true);
            }
            if i == DMG_CHAOS {
                if !env
                    .player
                    .mod_db
                    .flag_cfg("ChaosNotBypassEnergyShield", None, &output)
                {
                    bypass += 100.0;
                } else {
                    env.player.set_output_bool("AnyBypass", true);
                }
            }
            bypass = bypass.clamp(0.0, 100.0);
            env.player
                .set_output(&format!("{type_name}EnergyShieldBypass"), bypass);
        }

        let bypass = get_output_f64(&env.player.output, &format!("{type_name}EnergyShieldBypass"));
        let min_bypass = get_output_f64(&env.player.output, "MinimumBypass");
        env.player
            .set_output("MinimumBypass", min_bypass.min(bypass));
    }
}

// ── 9. Mind over Matter (L2292-2367) ─────────────────────────────────────────

fn calc_mind_over_matter(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    env.player
        .set_output_bool("ehpSectionAnySpecificTypes", false);
    env.player
        .set_output_bool("OnlySharedMindOverMatter", false);
    env.player
        .set_output_bool("AnySpecificMindOverMatter", false);

    let shared_mom = env
        .player
        .mod_db
        .sum_cfg(
            ModType::Base,
            "DamageTakenFromManaBeforeLife",
            None,
            &output,
        )
        .min(100.0);
    env.player
        .set_output("sharedMindOverMatter", shared_mom);

    if shared_mom > 0.0 {
        let mom_effect = shared_mom / 100.0;
        let es_bypass = get_output_f64(&output, "MinimumBypass") / 100.0;
        let life_recoverable = get_output_f64(&output, "LifeRecoverable");
        let life_hit_pool = get_output_f64(&output, "LifeHitPool");

        let (shared_mom_pool, _, _, _) =
            calc_mom_eb_pool(env, life_recoverable, mom_effect, es_bypass);
        env.player
            .set_output("sharedManaEffectiveLife", shared_mom_pool);
        let (shared_mom_hit_pool, _, _, _) =
            calc_mom_eb_pool(env, life_hit_pool, mom_effect, es_bypass);
        env.player
            .set_output("sharedMoMHitPool", shared_mom_hit_pool);
    } else {
        let life_recoverable = get_output_f64(&output, "LifeRecoverable");
        let life_hit_pool = get_output_f64(&output, "LifeHitPool");
        env.player
            .set_output("sharedManaEffectiveLife", life_recoverable);
        env.player
            .set_output("sharedMoMHitPool", life_hit_pool);
    }

    // L2336-2367: per-type MoM
    let output = env.player.output.clone();
    for (_i, type_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let type_mom = env
            .player
            .mod_db
            .sum_cfg(
                ModType::Base,
                &format!("{type_name}DamageTakenFromManaBeforeLife"),
                None,
                &output,
            )
            .min(100.0 - shared_mom);
        env.player
            .set_output(&format!("{type_name}MindOverMatter"), type_mom);

        let type_bypass = get_output_f64(&output, &format!("{type_name}EnergyShieldBypass"));
        let min_bypass = get_output_f64(&output, "MinimumBypass");

        if type_mom > 0.0 || (type_bypass > min_bypass && shared_mom > 0.0) {
            env.player
                .set_output_bool("ehpSectionAnySpecificTypes", true);
            env.player
                .set_output_bool("AnySpecificMindOverMatter", true);
            env.player
                .set_output_bool("OnlySharedMindOverMatter", false);

            let total_mom = (type_mom + shared_mom) / 100.0;
            let es_bypass_frac = type_bypass / 100.0;
            let life_recoverable = get_output_f64(&output, "LifeRecoverable");
            let life_hit_pool = get_output_f64(&output, "LifeHitPool");

            let (typed_pool, _, _, _) =
                calc_mom_eb_pool(env, life_recoverable, total_mom, es_bypass_frac);
            env.player
                .set_output(&format!("{type_name}ManaEffectiveLife"), typed_pool);
            let (typed_hit_pool, _, _, _) =
                calc_mom_eb_pool(env, life_hit_pool, total_mom, es_bypass_frac);
            env.player
                .set_output(&format!("{type_name}MoMHitPool"), typed_hit_pool);
        } else {
            let shared_effective = get_output_f64(&output, "sharedManaEffectiveLife");
            let shared_hit = get_output_f64(&output, "sharedMoMHitPool");
            env.player
                .set_output(&format!("{type_name}ManaEffectiveLife"), shared_effective);
            env.player
                .set_output(&format!("{type_name}MoMHitPool"), shared_hit);
        }
    }
}

/// Lua: calcMoMEBPool (L2297-2308)
fn calc_mom_eb_pool(
    env: &CalcEnv,
    life_pool: f64,
    mom_effect: f64,
    es_bypass: f64,
) -> (f64, f64, f64, f64) {
    let output = &env.player.output;
    let mana = get_output_f64(output, "ManaUnreserved").max(0.0);
    let max_mom_pool = if mom_effect < 1.0 {
        life_pool / (1.0 - mom_effect) - life_pool
    } else {
        f64::INFINITY
    };
    let max_mana_usable = mana.min(max_mom_pool).floor();
    let max_es_usable = if env
        .player
        .mod_db
        .flag_cfg("EnergyShieldProtectsMana", None, output)
        && es_bypass < 1.0
    {
        let es_cap = get_output_f64(output, "EnergyShieldRecoveryCap");
        es_cap
            .min(max_mom_pool * (1.0 - es_bypass))
            .min(
                (life_pool + max_mana_usable) / (1.0 - (1.0 - es_bypass) * mom_effect)
                    - (life_pool + max_mana_usable),
            )
            .floor()
    } else {
        0.0
    };
    let mana_used = (max_mom_pool - max_es_usable).min(max_mana_usable).floor();
    (
        life_pool + mana_used + max_es_usable,
        max_mana_usable,
        mana_used,
        max_es_usable,
    )
}

// ── 10. Guard (L2369-2403) ──────────────────────────────────────────────────

fn calc_guard(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    env.player.set_output_bool("AnyGuard", false);

    let shared_guard_rate = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "GuardAbsorbRate", None, &output)
        .min(100.0);
    env.player
        .set_output("sharedGuardAbsorbRate", shared_guard_rate);

    if shared_guard_rate > 0.0 {
        env.player
            .set_output_bool("OnlySharedGuard", true);
        let guard_absorb = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, "GuardAbsorbLimit", None, &output);
        env.player.set_output("sharedGuardAbsorb", guard_absorb);
    }

    // Per-type guard (L2386-2403)
    for type_name in DMG_TYPE_NAMES.iter() {
        let type_rate = env
            .player
            .mod_db
            .sum_cfg(
                ModType::Base,
                &format!("{type_name}GuardAbsorbRate"),
                None,
                &output,
            )
            .min(100.0);
        env.player
            .set_output(&format!("{type_name}GuardAbsorbRate"), type_rate);
        if type_rate > 0.0 {
            env.player
                .set_output_bool("ehpSectionAnySpecificTypes", true);
            env.player.set_output_bool("AnyGuard", true);
            env.player
                .set_output_bool("OnlySharedGuard", false);
            let absorb = env
                .player
                .mod_db
                .sum_cfg(
                    ModType::Base,
                    &format!("{type_name}GuardAbsorbLimit"),
                    None,
                    &output,
                );
            env.player
                .set_output(&format!("{type_name}GuardAbsorb"), absorb);
        }
    }
}

// ── 11. Aegis (L2406-2429) ──────────────────────────────────────────────────

fn calc_aegis(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    env.player.set_output_bool("AnyAegis", false);

    let shared_aegis = env
        .player
        .mod_db
        .max_value("AegisValue", None, &output)
        .unwrap_or(0.0);
    env.player.set_output("sharedAegis", shared_aegis);

    let shared_ele_aegis = env
        .player
        .mod_db
        .max_value("ElementalAegisValue", None, &output)
        .unwrap_or(0.0);
    env.player
        .set_output("sharedElementalAegis", shared_ele_aegis);

    if shared_aegis > 0.0 {
        env.player.set_output_bool("AnyAegis", true);
    }
    if shared_ele_aegis > 0.0 {
        env.player
            .set_output_bool("ehpSectionAnySpecificTypes", true);
        env.player.set_output_bool("AnyAegis", true);
    }

    // Per-type aegis (L2417-2429)
    for (i, type_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let aegis_val = env
            .player
            .mod_db
            .max_value(&format!("{type_name}AegisValue"), None, &output)
            .unwrap_or(0.0);
        if aegis_val > 0.0 {
            env.player
                .set_output_bool("ehpSectionAnySpecificTypes", true);
            env.player.set_output_bool("AnyAegis", true);
            env.player
                .set_output(&format!("{type_name}Aegis"), aegis_val);
        } else {
            env.player
                .set_output(&format!("{type_name}Aegis"), 0.0);
        }
        if is_elemental(i) {
            env.player.set_output(
                &format!("{type_name}AegisDisplay"),
                aegis_val + shared_ele_aegis,
            );
        }
    }
}

// ── 12. Frost Shield and allies (L2432-2494) ─────────────────────────────────

fn calc_frost_shield_and_allies(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    env.player.set_output(
        "FrostShieldLife",
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "FrostGlobeHealth", None, &output),
    );
    env.player.set_output(
        "FrostShieldDamageMitigation",
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "FrostGlobeDamageMitigation", None, &output),
    );
    env.player.set_output(
        "VaalArcticArmourLife",
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "VaalArcticArmourMaxHits", None, &output),
    );
    let vaal_arctic_more = env
        .player
        .mod_db
        .more_cfg("VaalArcticArmourMitigation", None, &output);
    let vaal_arctic_mit = (-(vaal_arctic_more - 1.0)).min(1.0).max(0.0);
    env.player
        .set_output("VaalArcticArmourMitigation", vaal_arctic_mit);

    // Spectre/Totem/etc. ally mitigation — write zero defaults
    env.player.set_output("SpectreAllyDamageMitigation", 0.0);
    env.player.set_output("TotemAllyDamageMitigation", 0.0);
    env.player
        .set_output("VaalRejuvenationTotemAllyDamageMitigation", 0.0);
    env.player
        .set_output("RadianceSentinelAllyDamageMitigation", 0.0);
    env.player
        .set_output("VoidSpawnAllyDamageMitigation", 0.0);
    env.player.set_output("SoulLinkMitigation", 0.0);
}

// ── 13. Total pool (L2496-2527) ──────────────────────────────────────────────

fn calc_total_pool(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    for type_name in DMG_TYPE_NAMES.iter() {
        let mom_effective = get_output_f64(&output, &format!("{type_name}ManaEffectiveLife"));
        let mom_hit = get_output_f64(&output, &format!("{type_name}MoMHitPool"));
        let mut total_pool = mom_effective;
        let mut total_hit_pool = mom_hit;

        let es_bypass = get_output_f64(&output, &format!("{type_name}EnergyShieldBypass"));
        let es_cap = get_output_f64(&output, "EnergyShieldRecoveryCap");

        if es_bypass < 100.0 {
            if !env
                .player
                .mod_db
                .flag_cfg("EnergyShieldProtectsMana", None, &output)
            {
                if es_bypass > 0.0 {
                    let pool_protected = es_cap / (1.0 - es_bypass / 100.0) * (es_bypass / 100.0);
                    total_pool = (total_pool - pool_protected).max(0.0)
                        + total_pool.min(pool_protected) / (es_bypass / 100.0);
                    total_hit_pool = (total_hit_pool - pool_protected).max(0.0)
                        + total_hit_pool.min(pool_protected) / (es_bypass / 100.0);
                } else {
                    total_pool += es_cap;
                    total_hit_pool += es_cap;
                }
            }
        }

        env.player
            .set_output(&format!("{type_name}TotalPool"), total_pool);
        env.player
            .set_output(&format!("{type_name}TotalHitPool"), total_hit_pool);
    }
}

// ── 14. numberOfHitsToDie simulation (L2529-2826) ────────────────────────────

fn calc_number_of_hits_to_die(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // Simplified numberOfHitsToDie: uses pool / damage approach
    // The full Lua simulation (iterative pool reduction) is complex.
    // We implement the key components: block, suppress, avoidance, then divide.

    // L2707-2714: NumberOfDamagingHits (raw, no block/suppress)
    let mut total_damage_per_hit = 0.0;
    for type_name in DMG_TYPE_NAMES.iter() {
        total_damage_per_hit += get_output_f64(&output, &format!("{type_name}TakenHit"));
    }

    // Use TotalHitPool (which includes MoM + ES + ward from calc_total_pool + calc_max_hit_taken)
    // For the simplified model, use PhysicalTotalHitPool as representative
    let total_hit_pool = get_output_f64(&output, "PhysicalTotalHitPool");
    let ward = get_output_f64(&output, "Ward");
    let pool_with_ward = total_hit_pool + ward;

    let num_damaging_hits = if total_damage_per_hit > 0.0 {
        pool_with_ward / total_damage_per_hit
    } else {
        f64::INFINITY
    };
    env.player
        .set_output("NumberOfDamagingHits", num_damaging_hits);

    // L2717-2826: NumberOfMitigatedDamagingHits (with block, suppress, avoidance)
    let block_chance = get_output_f64(&output, "EffectiveAverageBlockChance") / 100.0;
    let block_effect = get_output_f64(&output, "BlockEffect");
    let block_mult = 1.0 - block_chance * block_effect / 100.0;

    let suppress_chance = get_output_f64(&output, "EffectiveSpellSuppressionChance") / 100.0;
    let suppress_eff = get_output_f64(&output, "SpellSuppressionEffect");
    let suppress_mult = if suppress_chance < 1.0 {
        // For Average config, suppression applies to half of hits
        1.0 - (suppress_chance / 2.0) * suppress_eff / 100.0
    } else {
        1.0 // Already factored into damage taken
    };

    let configured_damage_chance = 100.0 * block_mult * suppress_mult;
    env.player
        .set_output("ConfiguredDamageChance", configured_damage_chance);

    let num_mitigated = if configured_damage_chance != 100.0 && total_damage_per_hit > 0.0 {
        // Iteratively compute with block/suppress reducing damage
        let mitigated_dmg = total_damage_per_hit * block_mult * suppress_mult;
        if mitigated_dmg > 0.0 {
            pool_with_ward / mitigated_dmg
        } else {
            f64::INFINITY
        }
    } else {
        num_damaging_hits
    };
    env.player
        .set_output("NumberOfMitigatedDamagingHits", num_mitigated);

    // L2830: TotalNumberOfHits = NumberOfMitigatedDamagingHits / (1 - ConfiguredNotHitChance/100)
    let not_hit_chance = get_output_f64(&output, "ConfiguredNotHitChance");
    let total_hits = if not_hit_chance < 100.0 {
        num_mitigated / (1.0 - not_hit_chance / 100.0)
    } else {
        f64::INFINITY
    };
    env.player.set_output("TotalNumberOfHits", total_hits);
}

// ── 15. Total EHP (L2877-2900) ──────────────────────────────────────────────

fn calc_total_ehp(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // L2878: TotalEHP = TotalNumberOfHits * totalEnemyDamageIn
    let total_hits = get_output_f64(&output, "TotalNumberOfHits");
    let total_enemy_dmg_in = get_output_f64(&output, "totalEnemyDamageIn");
    env.player
        .set_output("TotalEHP", total_hits * total_enemy_dmg_in);
}

// ── 16. Max hit taken (L3090-3301) ──────────────────────────────────────────

fn calc_max_hit_taken(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // L3091-3150: fix total pools (add ward, aegis, guard)
    for (i, type_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let mut hit_pool = get_output_f64(&output, &format!("{type_name}TotalHitPool"));

        // Ward
        let ward_bypass = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, "WardBypass", None, &output);
        let ward = get_output_f64(&output, "Ward");
        if ward_bypass > 0.0 {
            let pool_protected = ward / (1.0 - ward_bypass / 100.0) * (ward_bypass / 100.0);
            hit_pool = (hit_pool - pool_protected).max(0.0)
                + hit_pool.min(pool_protected) / (ward_bypass / 100.0);
        } else {
            hit_pool += ward;
        }

        // Aegis
        let type_aegis = get_output_f64(&output, &format!("{type_name}Aegis"));
        let shared_aegis = get_output_f64(&output, "sharedAegis");
        let display_aegis = if is_elemental(i) {
            get_output_f64(&output, &format!("{type_name}AegisDisplay"))
        } else {
            0.0
        };
        hit_pool += type_aegis.max(shared_aegis).max(display_aegis);

        // Guard
        let guard_rate =
            get_output_f64(&output, "sharedGuardAbsorbRate")
                + get_output_f64(&output, &format!("{type_name}GuardAbsorbRate"));
        if guard_rate > 0.0 {
            let guard_absorb = get_output_f64(&output, "sharedGuardAbsorb")
                + get_output_f64(&output, &format!("{type_name}GuardAbsorb"));
            if guard_rate >= 100.0 {
                hit_pool += guard_absorb;
            } else {
                let pool_protected =
                    guard_absorb / (guard_rate / 100.0) * (1.0 - guard_rate / 100.0);
                hit_pool = (hit_pool - pool_protected).max(0.0)
                    + hit_pool.min(pool_protected) / (1.0 - guard_rate / 100.0);
            }
        }

        // Frost shield
        let frost_life = get_output_f64(&output, "FrostShieldLife");
        let frost_mit = get_output_f64(&output, "FrostShieldDamageMitigation");
        if frost_life > 0.0 && frost_mit > 0.0 {
            let pool_protected = frost_life / (frost_mit / 100.0) * (1.0 - frost_mit / 100.0);
            hit_pool = (hit_pool - pool_protected).max(0.0)
                + hit_pool.min(pool_protected) / (1.0 - frost_mit / 100.0);
        }

        env.player
            .set_output(&format!("{type_name}TotalHitPool"), hit_pool);
    }

    // L3152-3301: per-type max hit calculation
    let output = env.player.output.clone();
    let dr_max = get_output_f64(&output, "DamageReductionMax");

    for (src_idx, src_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let mut part_min = f64::INFINITY;

        for (dst_idx, dst_name) in DMG_TYPE_NAMES.iter().enumerate() {
            let convert_pct = env.player.damage_shift_table[src_idx][dst_idx];
            let taken_flat = get_output_f64(&output, &format!("{dst_name}takenFlat"));

            if convert_pct > 0.0 || taken_flat != 0.0 {
                let effective_armour =
                    get_output_f64(&output, &format!("{dst_name}EffectiveAppliedArmour"));
                let damage_converted_multi = convert_pct / 100.0;
                let total_hit_pool = get_output_f64(&output, &format!("{dst_name}TotalHitPool"));
                let total_taken_multi =
                    get_output_f64(&output, &format!("{dst_name}AfterReductionTakenHitMulti"))
                        * (1.0 - get_output_f64(&output, "VaalArcticArmourMitigation"));

                let hit_taken;
                if effective_armour == 0.0 && convert_pct == 100.0 {
                    // Simple path: no armour DR
                    let dr_multi = get_output_f64(&output, &format!("{dst_name}ResistTakenHitMulti"))
                        * (1.0 - get_output_f64(&output, &format!("{dst_name}DamageReduction")) / 100.0);
                    hit_taken = if dr_multi != 0.0 && total_taken_multi != 0.0 {
                        (total_hit_pool / damage_converted_multi / dr_multi - taken_flat).max(0.0)
                            / total_taken_multi
                    } else {
                        f64::INFINITY
                    };
                } else {
                    // Quadratic path with armour
                    let total_resist_mult =
                        get_output_f64(&output, &format!("{dst_name}ResistTakenHitMulti"));
                    let reduction_pct = if env.player.mod_db.flag_cfg(
                        &format!("SelfIgnoreBase{dst_name}DamageReduction"),
                        None,
                        &output,
                    ) {
                        0.0
                    } else {
                        let rwh = get_output_f64(&output, &format!("Base{dst_name}DamageReductionWhenHit"));
                        if rwh != 0.0 { rwh } else { get_output_f64(&output, &format!("Base{dst_name}DamageReduction")) }
                    };
                    let flat_dr = reduction_pct / 100.0;
                    let enemy_overwhelm_pct = if env.player.mod_db.flag_cfg(
                        &format!("SelfIgnore{dst_name}DamageReduction"),
                        None,
                        &output,
                    ) {
                        0.0
                    } else {
                        get_output_f64(&output, &format!("{dst_name}EnemyOverwhelm"))
                    };

                    let resist_x_convert = total_resist_mult * damage_converted_multi;
                    let a = 5.0
                        * (1.0 - flat_dr + enemy_overwhelm_pct / 100.0)
                        * total_taken_multi
                        * resist_x_convert
                        * resist_x_convert;
                    let b = ((enemy_overwhelm_pct / 100.0 - flat_dr) * effective_armour
                        * total_taken_multi
                        - 5.0 * (total_hit_pool - taken_flat * total_taken_multi))
                        * resist_x_convert;
                    let c = -effective_armour * (total_hit_pool - taken_flat * total_taken_multi);

                    let discriminant = b * b - 4.0 * a * c;
                    if a != 0.0 && discriminant >= 0.0 {
                        let raw = (discriminant.sqrt() - b) / (2.0 * a);
                        let no_dr_max_hit = if total_taken_multi != 0.0 && total_resist_mult != 0.0 {
                            total_hit_pool / damage_converted_multi / total_resist_mult
                                / total_taken_multi
                                * (1.0 - taken_flat * total_taken_multi / total_hit_pool)
                        } else {
                            f64::INFINITY
                        };
                        let max_dr_max_hit =
                            no_dr_max_hit / (1.0 - (dr_max - enemy_overwhelm_pct) / 100.0);
                        hit_taken = raw.min(max_dr_max_hit).max(no_dr_max_hit).floor();
                    } else {
                        hit_taken = f64::INFINITY;
                    }
                }

                part_min = part_min.min(hit_taken);
            }
        }

        let enemy_dmg_mult = get_output_f64(&output, &format!("{src_name}EnemyDamageMult"));
        let final_max_hit = if part_min == f64::INFINITY {
            f64::INFINITY
        } else if enemy_dmg_mult > 0.0 {
            (part_min / enemy_dmg_mult).round()
        } else {
            f64::INFINITY
        };

        env.player
            .set_output(&format!("{src_name}MaximumHitTaken"), final_max_hit);
    }

    // Second minimum (L3289-3300)
    let output = env.player.output.clone();
    let mut minimum = f64::INFINITY;
    let mut second_minimum = f64::INFINITY;
    for type_name in DMG_TYPE_NAMES.iter() {
        let val = get_output_f64(&output, &format!("{type_name}MaximumHitTaken"));
        if val < minimum {
            second_minimum = minimum;
            minimum = val;
        } else if val < second_minimum {
            second_minimum = val;
        }
    }
    env.player
        .set_output("SecondMinimalMaximumHitTaken", second_minimum);
}

// ── 17. Dot EHP (L3303-3314) ────────────────────────────────────────────────

fn calc_dot_ehp(env: &mut CalcEnv) {
    let output = env.player.output.clone();
    for type_name in DMG_TYPE_NAMES.iter() {
        let total_pool = get_output_f64(&output, &format!("{type_name}TotalPool"));
        let dot_mult = get_output_f64(&output, &format!("{type_name}TakenDotMult"));
        if dot_mult > 0.0 {
            env.player
                .set_output(&format!("{type_name}DotEHP"), total_pool / dot_mult);
        }
    }
}

// ── 18. Build degen and net regen (CalcDefence.lua:3316-3462) ────────────────

fn calc_build_degen(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    let mut total_build_degen: f64 = 0.0;
    let mut type_build_degen: [f64; 5] = [0.0; 5];

    for (src_idx, src_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let base_val = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, &format!("{src_name}Degen"), None, &output);
        if base_val > 0.0 {
            for (dst_idx, dst_name) in DMG_TYPE_NAMES.iter().enumerate() {
                let convert_percent = env.player.damage_shift_table[src_idx][dst_idx];
                if convert_percent > 0.0 {
                    let taken_dot_mult =
                        get_output_f64(&output, &format!("{dst_name}TakenDotMult"));
                    let total = base_val * (convert_percent / 100.0) * taken_dot_mult;
                    type_build_degen[dst_idx] += total;
                    total_build_degen += total;
                }
            }
        }
    }

    if total_build_degen > 0.0 {
        env.player.set_output("TotalBuildDegen", total_build_degen);

        for (dst_idx, dst_name) in DMG_TYPE_NAMES.iter().enumerate() {
            if type_build_degen[dst_idx] > 0.0 {
                env.player
                    .set_output(&format!("{dst_name}BuildDegen"), type_build_degen[dst_idx]);
            }
        }

        let life_regen_recovery = get_output_f64(&output, "LifeRegenRecovery");
        let mana_regen_recovery = get_output_f64(&output, "ManaRegenRecovery");
        let es_regen_recovery = get_output_f64(&output, "EnergyShieldRegenRecovery");

        let mut total_life_degen: f64 = 0.0;
        let mut total_mana_degen: f64 = 0.0;
        let mut total_es_degen: f64 = 0.0;

        for (dst_idx, dst_name) in DMG_TYPE_NAMES.iter().enumerate() {
            let build_degen = type_build_degen[dst_idx];
            if build_degen == 0.0 {
                continue;
            }

            let mom = get_output_f64(&output, &format!("{dst_name}MindOverMatter"));
            let shared_mom = get_output_f64(&output, "sharedMindOverMatter");
            let taken_from_mana = mom + shared_mom;
            let es_bypass =
                get_output_f64(&output, &format!("{dst_name}EnergyShieldBypass"));

            if es_regen_recovery > 0.0 {
                if env
                    .player
                    .mod_db
                    .flag_cfg("EnergyShieldProtectsMana", None, &output)
                {
                    let life_d = build_degen * (1.0 - taken_from_mana / 100.0);
                    let es_d =
                        build_degen * (1.0 - es_bypass / 100.0) * (taken_from_mana / 100.0);
                    total_life_degen += life_d;
                    total_es_degen += es_d;
                } else {
                    let life_d =
                        build_degen * (es_bypass / 100.0) * (1.0 - taken_from_mana / 100.0);
                    let es_d = build_degen * (1.0 - es_bypass / 100.0);
                    let mana_d =
                        build_degen * (es_bypass / 100.0) * (taken_from_mana / 100.0);
                    total_life_degen += life_d;
                    total_es_degen += es_d;
                    total_mana_degen += mana_d;
                }
            } else {
                let life_d = build_degen * (1.0 - taken_from_mana / 100.0);
                let mana_d = build_degen * (taken_from_mana / 100.0);
                total_life_degen += life_d;
                total_mana_degen += mana_d;
            }
        }

        let net_life_regen = life_regen_recovery - total_life_degen;
        let net_mana_regen = mana_regen_recovery - total_mana_degen;
        let net_es_regen = es_regen_recovery - total_es_degen;

        env.player.set_output("NetLifeRegen", net_life_regen);
        env.player.set_output("NetManaRegen", net_mana_regen);
        env.player.set_output("NetEnergyShieldRegen", net_es_regen);
        env.player.set_output(
            "TotalNetRegen",
            net_life_regen + net_mana_regen + net_es_regen,
        );
    }
}
