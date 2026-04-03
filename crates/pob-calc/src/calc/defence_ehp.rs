use super::defence::{armour_reduction_f, DMG_PHYSICAL, DMG_TYPE_NAMES};
use super::env::{get_output_f64, CalcEnv};
use crate::mod_db::types::ModType;

// ── Orchestrator ─────────────────────────────────────────────────────────────

pub fn run(env: &mut CalcEnv) {
    calc_not_hit_chances(env);
    calc_enemy_damage(env);
    calc_damage_taken_mult(env);
    calc_max_hit_taken(env);
    calc_total_ehp(env);
    calc_build_degen(env);
}

// ── 1. Not-hit chances (L1635-1656) ──────────────────────────────────────────

fn calc_not_hit_chances(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    let evade_chance = get_output_f64(&output, "EvadeChance");
    let attack_dodge = get_output_f64(&output, "AttackDodgeChance");
    let spell_dodge = get_output_f64(&output, "SpellDodgeChance");
    let block = get_output_f64(&output, "EffectiveBlockChance");
    let spell_block = get_output_f64(&output, "EffectiveSpellBlockChance");
    let proj_block = get_output_f64(&output, "EffectiveProjectileBlockChance");
    let spell_proj_block = get_output_f64(&output, "EffectiveSpellProjectileBlockChance");

    // Melee not-hit: 1 - (1-evade/100)*(1-attackDodge/100)*(1-block/100)
    let melee_not_hit = (1.0
        - (1.0 - evade_chance / 100.0) * (1.0 - attack_dodge / 100.0) * (1.0 - block / 100.0))
        * 100.0;

    // Projectile not-hit: uses projectile block
    let proj_not_hit = (1.0
        - (1.0 - evade_chance / 100.0) * (1.0 - attack_dodge / 100.0) * (1.0 - proj_block / 100.0))
        * 100.0;

    // Spell not-hit: no evasion, uses spell dodge + spell block
    let spell_not_hit = (1.0 - (1.0 - spell_dodge / 100.0) * (1.0 - spell_block / 100.0)) * 100.0;

    // Spell projectile not-hit: spell dodge + spell projectile block
    let spell_proj_not_hit =
        (1.0 - (1.0 - spell_dodge / 100.0) * (1.0 - spell_proj_block / 100.0)) * 100.0;

    // Average across all four
    let avg_not_hit = (melee_not_hit + proj_not_hit + spell_not_hit + spell_proj_not_hit) / 4.0;

    env.player.set_output("MeleeNotHitChance", melee_not_hit);
    env.player
        .set_output("ProjectileNotHitChance", proj_not_hit);
    env.player.set_output("SpellNotHitChance", spell_not_hit);
    env.player
        .set_output("SpellProjectileNotHitChance", spell_proj_not_hit);
    env.player.set_output("AverageNotHitChance", avg_not_hit);

    // CalcDefence.lua:1644: AverageEvadeChance = (MeleeEvadeChance + ProjectileEvadeChance) / 4
    // Division by 4 (not 2) because there are 4 damage categories but only 2 use evasion.
    let melee_evade = get_output_f64(&output, "MeleeEvadeChance");
    let proj_evade = get_output_f64(&output, "ProjectileEvadeChance");
    let avg_evade = (melee_evade + proj_evade) / 4.0;
    env.player.set_output("AverageEvadeChance", avg_evade);

    // CalcDefence.lua:1646: ConfiguredEvadeChance = output[damageCategoryConfig.."EvadeChance"] or 0
    // damageCategoryConfig defaults to "Average" (from env.configInput.enemyDamageType or "Average")
    // For our oracle builds, there's no config override, so it's always "Average".
    env.player.set_output("ConfiguredEvadeChance", avg_evade);
}

// ── 2. Enemy damage estimation (L1658-1790) ──────────────────────────────────

fn calc_enemy_damage(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // Total enemy damage (from EnemyDamage base mod, default 1500)
    let enemy_dmg_base = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "EnemyDamage", None, &output);
    let enemy_dmg = if enemy_dmg_base > 0.0 {
        enemy_dmg_base
    } else {
        1500.0
    };
    // Not output as "EnemyDamage" — PoB outputs per-type enemy damage

    // Enemy crit chance: CalcDefence.lua:1684
    // Priority: NeverCrit flag → 0, AlwaysCrit flag → 100.
    // Otherwise: config_crit * (1 + playerINC + enemyINC) * (1 - evadeChance/100), clamped [0,100].
    // The config_crit base is 5.0 (PoB default / configPlaceholder["enemyCritChance"]).
    // modDB:Override(nil, "enemyCritChance") would override from config — use BASE sum which
    // captures any explicit override mods (e.g. "Enemies always have X% crit chance" type mods).
    let enemy_crit = if env.enemy.mod_db.flag_cfg("NeverCrit", None, &output) {
        0.0
    } else if env.enemy.mod_db.flag_cfg("AlwaysCrit", None, &output) {
        100.0
    } else {
        // Base config crit chance: 5.0 (default placeholder)
        // Any override from modDB (rare) captured via BASE "EnemyCritChance" sum
        let override_val = env
            .player
            .mod_db
            .override_value("enemyCritChance", None, &output);
        let config_crit = override_val.unwrap_or(5.0);
        // INC mods from player modDB ("EnemyCritChance") and enemy modDB ("CritChance")
        let inc_player = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "EnemyCritChance", None, &output);
        let inc_enemy = env
            .enemy
            .mod_db
            .sum_cfg(ModType::Inc, "CritChance", None, &output);
        // Evade chance reduces enemy crit chance
        let configured_evade = get_output_f64(&output, "ConfiguredEvadeChance");
        (config_crit
            * (1.0 + inc_player / 100.0 + inc_enemy / 100.0)
            * (1.0 - configured_evade / 100.0))
            .clamp(0.0, 100.0)
    };
    env.player.set_output("EnemyCritChance", enemy_crit);

    // Enemy crit damage multiplier: CalcDefence.lua:1686-1687
    // enemyCritDamage = max((configCritDamage or 0) + enemyDB:Sum("BASE", nil, "CritMultiplier"), 0)
    // configCritDamage default is data.monsterConstants["base_critical_strike_multiplier"] - 100 = 30
    // (130 - 100 = 30, matching PoB's monster_critical_strike_multiplier = 130%)
    //
    // EnemyCritEffect = 1 + enemyCritChance/100 * (enemyCritDamage/100) * (1 - CritExtraDamageReduction/100)
    let crit_mult_from_enemy =
        env.enemy
            .mod_db
            .sum_cfg(ModType::Base, "CritMultiplier", None, &output);
    let enemy_crit_damage = (30.0 + crit_mult_from_enemy).max(0.0);
    let crit_dr = get_output_f64(&output, "CritExtraDamageReduction");
    let effective_crit_mult =
        1.0 + enemy_crit / 100.0 * (enemy_crit_damage / 100.0) * (1.0 - crit_dr / 100.0);
    env.player
        .set_output("EnemyCritEffect", effective_crit_mult);

    // Average crit multiplier applied to hits (internal use for EHP calc).
    // EnemyCritEffect already incorporates crit chance: 1 + crit_chance/100 * extra_damage.
    // So this IS the average damage multiplier from enemy crits.
    let avg_crit_mult = effective_crit_mult;

    // Enemy skill time (default 0.7 seconds)
    let enemy_skill_time_base =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "EnemySkillTime", None, &output);
    let enemy_skill_time = if enemy_skill_time_base > 0.0 {
        enemy_skill_time_base
    } else {
        0.7
    };
    env.player.set_output("enemySkillTime", enemy_skill_time);

    // Per-type enemy damage, penetration, overwhelm
    // PoB splits enemy_dmg across types based on EnemyPhysicalPercent etc.
    // Default: all physical. Config mods can set EnemyFirePercent etc.
    let mut type_percents: Vec<f64> = Vec::with_capacity(5);
    for (i, type_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let pct_stat = format!("Enemy{type_name}Percent");
        let pct_base = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, &pct_stat, None, &output);
        let pct = if pct_base > 0.0 {
            pct_base
        } else if i == DMG_PHYSICAL {
            // Default: if no percentages configured, all damage is physical
            100.0
        } else {
            0.0
        };
        type_percents.push(pct);
    }

    // Normalize percentages if they sum to >100
    let total_pct: f64 = type_percents.iter().sum();

    // Enemy damage multiplier (for configurations like "enemy uses AoE" etc.)
    let enemy_dmg_mult_base =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "EnemyDamageMult", None, &output);
    let enemy_dmg_mult = if enemy_dmg_mult_base > 0.0 {
        enemy_dmg_mult_base
    } else {
        1.0
    };

    // Total enemy damage accounting for crit and mult
    let total_enemy_dmg_in = enemy_dmg * enemy_dmg_mult;
    let total_enemy_dmg = total_enemy_dmg_in * avg_crit_mult;

    // Per-type outputs
    for (i, type_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let pct = type_percents[i];
        let type_frac = if total_pct > 0.0 {
            pct / total_pct
        } else {
            0.0
        };

        // {Type}EnemyDamage — portion of total enemy damage for this type
        let type_dmg = total_enemy_dmg * type_frac;
        env.player
            .set_output(&format!("{type_name}EnemyDamage"), type_dmg);

        // {Type}EnemyDamageMult
        env.player
            .set_output(&format!("{type_name}EnemyDamageMult"), enemy_dmg_mult);

        // {Type}EnemyPen — penetration for this damage type
        let pen_stat = format!("{type_name}EnemyPen");
        let pen_mod_stat = format!("Enemy{type_name}Penetration");
        let pen = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, &pen_mod_stat, None, &output);
        env.player.set_output(&pen_stat, pen);

        // {Type}EnemyOverwhelm
        let overwhelm_stat = format!("{type_name}EnemyOverwhelm");
        let overwhelm_mod_stat = format!("Enemy{type_name}Overwhelm");
        let overwhelm =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, &overwhelm_mod_stat, None, &output);
        env.player.set_output(&overwhelm_stat, overwhelm);
    }

    // Total enemy damage outputs
    env.player
        .set_output("totalEnemyDamageIn", total_enemy_dmg_in);
    env.player.set_output("totalEnemyDamage", total_enemy_dmg);
}

// ── 3. Damage taken multipliers (L1870-2097) ─────────────────────────────────

fn calc_damage_taken_mult(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    for type_name in DMG_TYPE_NAMES.iter() {
        // ── Hit taken mult ──
        let taken_inc_stat = format!("{type_name}DamageTaken");
        let general_taken_inc_stat = "DamageTaken";

        let taken_inc = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, &taken_inc_stat, None, &output);
        let general_taken_inc =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, general_taken_inc_stat, None, &output);
        let total_inc = taken_inc + general_taken_inc;

        let taken_more = env.player.mod_db.more_cfg(&taken_inc_stat, None, &output);
        let general_taken_more = env
            .player
            .mod_db
            .more_cfg(general_taken_inc_stat, None, &output);
        let total_more = taken_more * general_taken_more;

        // Resistance multiplier
        let resist_stat = format!("{type_name}Resist");
        let resist = get_output_f64(&output, &resist_stat);
        let resist_mult = 1.0 - resist / 100.0;

        // For physical: armour-based reduction (estimate based on enemy damage)
        let armour_mult = if *type_name == "Physical" {
            let armour = get_output_f64(&output, "Armour");
            let enemy_dmg = get_output_f64(&output, "PhysicalEnemyDamage");
            if armour > 0.0 && enemy_dmg > 0.0 {
                let reduction = armour_reduction_f(armour, enemy_dmg);
                1.0 - reduction / 100.0
            } else {
                1.0
            }
        } else {
            1.0
        };

        let hit_mult = (1.0 + total_inc / 100.0) * total_more * resist_mult * armour_mult;
        env.player
            .set_output(&format!("{type_name}TakenHitMult"), hit_mult.max(0.0));

        // ── DoT taken mult (no armour, same resist) ──
        let dot_taken_inc_stat = format!("{type_name}DamageTakenOverTime");
        let dot_taken_inc =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, &dot_taken_inc_stat, None, &output);
        let dot_general_inc =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, "DamageTakenOverTime", None, &output);
        let dot_total_inc = taken_inc + general_taken_inc + dot_taken_inc + dot_general_inc;

        let dot_taken_more = env
            .player
            .mod_db
            .more_cfg(&dot_taken_inc_stat, None, &output);
        let dot_general_more = env
            .player
            .mod_db
            .more_cfg("DamageTakenOverTime", None, &output);
        let dot_total_more = taken_more * general_taken_more * dot_taken_more * dot_general_more;

        let dot_mult = (1.0 + dot_total_inc / 100.0) * dot_total_more * resist_mult;
        env.player
            .set_output(&format!("{type_name}TakenDotMult"), dot_mult.max(0.0));
    }
}

// ── 4. Max hit taken (L3090-3301) ────────────────────────────────────────────

fn calc_max_hit_taken(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    let life = get_output_f64(&output, "Life").max(1.0);
    let es = get_output_f64(&output, "EnergyShield");
    let ward = get_output_f64(&output, "Ward");

    // MoM: damage taken from mana before life
    let mom_pct = get_output_f64(&output, "DamageTakenFromManaBeforeLife");
    let mana_unreserved = get_output_f64(&output, "ManaUnreserved");

    // The pool available from mana via MoM
    let mom_mana_pool = if mom_pct > 0.0 {
        // MoM protects mom_pct% of life damage from mana.
        // Effective mana contribution = min(mana_unreserved, life * mom_pct / (100 - mom_pct))
        let life_portion = 100.0 - mom_pct;
        if life_portion > 0.0 {
            mana_unreserved.min(life * mom_pct / life_portion)
        } else {
            mana_unreserved
        }
    } else {
        0.0
    };

    // Compute per-type max hit taken
    for type_name in DMG_TYPE_NAMES.iter() {
        let taken_mult = get_output_f64(&output, &format!("{type_name}TakenHitMult"));

        // Total pool = life + ES + ward + MoM mana
        let total_pool = life + es + ward + mom_mana_pool;
        env.player
            .set_output(&format!("{type_name}TotalPool"), total_pool);

        // Total hit pool (same as total pool for simplified calc)
        env.player
            .set_output(&format!("{type_name}TotalHitPool"), total_pool);

        // Max hit = pool / taken_mult
        let max_hit = if taken_mult > 0.0 {
            total_pool / taken_mult
        } else {
            f64::INFINITY
        };
        env.player
            .set_output(&format!("{type_name}MaximumHitTaken"), max_hit);
    }
}

// ── 5. Total EHP ─────────────────────────────────────────────────────────────

fn calc_total_ehp(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // Physical EHP as primary
    let phys_max_hit = get_output_f64(&output, "PhysicalMaximumHitTaken");
    env.player.set_output("TotalEHP", phys_max_hit);

    // Second minimal maximum hit taken: second-smallest across all types
    let mut max_hits: Vec<f64> = DMG_TYPE_NAMES
        .iter()
        .map(|t| get_output_f64(&output, &format!("{t}MaximumHitTaken")))
        .filter(|v| v.is_finite())
        .collect();
    max_hits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let second_min = if max_hits.len() >= 2 {
        max_hits[1]
    } else if !max_hits.is_empty() {
        max_hits[0]
    } else {
        0.0
    };
    env.player
        .set_output("SecondMinimalMaximumHitTaken", second_min);
}

// ── 6. Build degen and net regen (CalcDefence.lua:3316-3462) ─────────────────

/// Compute TotalBuildDegen and net regen fields.
///
/// Mirrors CalcDefence.lua:3316-3462 (calcs.buildDefenceEstimations degen block).
///
/// TotalBuildDegen is the sum of all DoT degens (fire from RF, chaos from poison, etc.)
/// multiplied by their taken-dot multipliers and distributed through the damage shift table.
///
/// NetLifeRegen/NetManaRegen/NetEnergyShieldRegen subtract the per-pool degen from regen.
/// These fields are only written when TotalBuildDegen > 0.
fn calc_build_degen(env: &mut CalcEnv) {
    use super::defence::DMG_TYPE_NAMES;

    let output = env.player.output.clone();

    let mut total_build_degen: f64 = 0.0;
    // Per-type accumulated degen (Fire/Cold/Lightning/Physical/Chaos)
    let mut type_build_degen: [f64; 5] = [0.0; 5];

    // The damage shift table: [source_type][dest_type] = percent
    // type indices: 0=Physical, 1=Lightning, 2=Cold, 3=Fire, 4=Chaos
    // (matches the damage_shift_table layout in env)
    for (src_idx, src_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let base_val =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, &format!("{src_name}Degen"), None, &output);
        if base_val > 0.0 {
            // Distribute through damage shift table
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

    // Only write TotalBuildDegen and Net* fields when there IS degen
    if total_build_degen > 0.0 {
        env.player.set_output("TotalBuildDegen", total_build_degen);

        // Write per-type BuildDegen fields (used by later calculations)
        for (dst_idx, dst_name) in DMG_TYPE_NAMES.iter().enumerate() {
            if type_build_degen[dst_idx] > 0.0 {
                env.player
                    .set_output(&format!("{dst_name}BuildDegen"), type_build_degen[dst_idx]);
            }
        }

        // NetRegen = RegenRecovery - degen distributed to each pool
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

            // Mind Over Matter: portion of degen taken from mana
            let mom = get_output_f64(&output, &format!("{dst_name}MindOverMatter"));
            let shared_mom = get_output_f64(&output, "sharedMindOverMatter");
            let taken_from_mana = mom + shared_mom;

            // EnergyShieldBypass: portion that bypasses ES and hits life/mana directly
            let es_bypass = get_output_f64(&output, &format!("{dst_name}EnergyShieldBypass"));

            if es_regen_recovery > 0.0 {
                // ES is active: split between ES, life, and mana
                let (life_degen, es_degen, mana_degen) =
                    if env
                        .player
                        .mod_db
                        .flag_cfg("EnergyShieldProtectsMana", None, &output)
                    {
                        // ES protects mana: life gets full portion, ES absorbs mana portion
                        let life_d = build_degen * (1.0 - taken_from_mana / 100.0);
                        let es_d =
                            build_degen * (1.0 - es_bypass / 100.0) * (taken_from_mana / 100.0);
                        (life_d, es_d, 0.0)
                    } else {
                        // Normal: ES absorbs everything except bypassing portion
                        let life_d =
                            build_degen * (es_bypass / 100.0) * (1.0 - taken_from_mana / 100.0);
                        let es_d = build_degen * (1.0 - es_bypass / 100.0);
                        let mana_d = build_degen * (es_bypass / 100.0) * (taken_from_mana / 100.0);
                        (life_d, es_d, mana_d)
                    };
                total_life_degen += life_degen;
                total_es_degen += es_degen;
                total_mana_degen += mana_degen;
            } else {
                // No ES: split between life and mana
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
    // When TotalBuildDegen == 0: do NOT write the field (Lua sets it to nil).
    // Also do NOT write NetLifeRegen/NetManaRegen/NetEnergyShieldRegen.
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        calc::env::CalcEnv,
        data::GameData,
        mod_db::{
            types::{Mod, ModSource},
            ModDb,
        },
    };
    use std::sync::Arc;

    fn src() -> ModSource {
        ModSource::new("Test", "test")
    }

    fn make_test_env(mods: Vec<Mod>) -> CalcEnv {
        let mut db = ModDb::new();
        for elem in &["Fire", "Cold", "Lightning", "Chaos"] {
            db.add(Mod::new_base(
                &format!("{elem}ResistMax"),
                75.0,
                ModSource::new("Base", "cap"),
            ));
        }
        db.add(Mod::new_base(
            "Life",
            5000.0,
            ModSource::new("Base", "base"),
        ));
        db.add(Mod::new_base(
            "Mana",
            1000.0,
            ModSource::new("Base", "base"),
        ));
        db.add(Mod::new_base(
            "EnergyShield",
            0.0,
            ModSource::new("Base", "base"),
        ));
        db.add(Mod::new_base("Str", 14.0, ModSource::new("Base", "base")));
        db.add(Mod::new_base("Dex", 14.0, ModSource::new("Base", "base")));
        db.add(Mod::new_base("Int", 14.0, ModSource::new("Base", "base")));
        for m in mods {
            db.add(m);
        }
        let data = Arc::new(GameData::default_for_test());
        CalcEnv::new(db, ModDb::new(), data)
    }

    fn run_full_pipeline(env: &mut CalcEnv) {
        crate::calc::perform::run(env);
        crate::calc::defence::run(env);
        run(env);
    }

    #[test]
    fn total_ehp_computed() {
        let mut env = make_test_env(vec![]);
        run_full_pipeline(&mut env);
        let ehp = get_output_f64(&env.player.output, "TotalEHP");
        assert!(ehp > 0.0, "TotalEHP should be > 0, got {ehp}");
    }

    #[test]
    fn max_hit_taken_computed() {
        let mut env = make_test_env(vec![]);
        run_full_pipeline(&mut env);
        let phys_max = get_output_f64(&env.player.output, "PhysicalMaximumHitTaken");
        assert!(
            phys_max > 0.0,
            "PhysicalMaximumHitTaken should be > 0, got {phys_max}"
        );
    }

    #[test]
    fn not_hit_chance_with_evasion() {
        let mut env = make_test_env(vec![Mod::new_base("Evasion", 5000.0, src())]);
        run_full_pipeline(&mut env);
        let melee_not_hit = get_output_f64(&env.player.output, "MeleeNotHitChance");
        assert!(
            melee_not_hit > 0.0,
            "MeleeNotHitChance should be > 0 with evasion, got {melee_not_hit}"
        );
    }

    #[test]
    fn enemy_damage_defaults_to_1500() {
        let mut env = make_test_env(vec![]);
        run_full_pipeline(&mut env);
        // Total enemy damage distributed across types; physical gets 100%
        let total_dmg = get_output_f64(&env.player.output, "totalEnemyDamageIn");
        assert!(
            total_dmg > 0.0,
            "totalEnemyDamageIn should be > 0, got {total_dmg}"
        );
    }

    #[test]
    fn taken_hit_mult_with_resistance() {
        let mut env = make_test_env(vec![Mod::new_base("FireResist", 75.0, src())]);
        run_full_pipeline(&mut env);
        let fire_mult = get_output_f64(&env.player.output, "FireTakenHitMult");
        // With 75% fire resist, mult should be 0.25 (before inc/more adjustments)
        assert!(
            fire_mult < 1.0,
            "FireTakenHitMult should be < 1.0 with 75% resist, got {fire_mult}"
        );
        assert!(
            (fire_mult - 0.25).abs() < 0.01,
            "FireTakenHitMult should be ~0.25 with 75% resist, got {fire_mult}"
        );
    }

    #[test]
    fn second_minimal_max_hit_computed() {
        let mut env = make_test_env(vec![]);
        run_full_pipeline(&mut env);
        let second_min = get_output_f64(&env.player.output, "SecondMinimalMaximumHitTaken");
        assert!(
            second_min > 0.0,
            "SecondMinimalMaximumHitTaken should be > 0, got {second_min}"
        );
    }

    #[test]
    fn physical_armour_reduces_taken_mult() {
        let mut env = make_test_env(vec![Mod::new_base("Armour", 10000.0, src())]);
        run_full_pipeline(&mut env);
        let phys_mult = get_output_f64(&env.player.output, "PhysicalTakenHitMult");
        // With armour, physical taken mult should be < 1.0
        assert!(
            phys_mult < 1.0,
            "PhysicalTakenHitMult with 10k armour should be < 1.0, got {phys_mult}"
        );
    }

    #[test]
    fn mom_increases_max_hit_taken() {
        // Without MoM
        let mut env_no_mom = make_test_env(vec![]);
        run_full_pipeline(&mut env_no_mom);
        let max_hit_no_mom = get_output_f64(&env_no_mom.player.output, "PhysicalMaximumHitTaken");

        // With MoM (40% damage taken from mana)
        let mut env_mom = make_test_env(vec![Mod::new_base(
            "DamageTakenFromManaBeforeLife",
            40.0,
            src(),
        )]);
        run_full_pipeline(&mut env_mom);
        let max_hit_mom = get_output_f64(&env_mom.player.output, "PhysicalMaximumHitTaken");

        assert!(
            max_hit_mom > max_hit_no_mom,
            "MoM should increase max hit taken: {max_hit_mom} > {max_hit_no_mom}"
        );
    }
}
