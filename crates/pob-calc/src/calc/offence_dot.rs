//! Non-ailment DoT, impale, and combined DPS calculations.
//! Mirrors CalcOffence.lua impale/dot/combined sections from Path of Building.

use super::env::{get_output_f64, CalcEnv};
use crate::calc::offence_utils::DMG_TYPE_NAMES;
use crate::mod_db::types::{ModType, SkillCfg};

// ── Impale ──────────────────────────────────────────────────────────────────

/// Calculate impale DPS and related outputs.
///
/// ImpaleChance base mod, clamped [0,100].
/// Effect: ImpaleEffect base / 100, default 0.1 (10%).
/// Max stacks: ImpaleStacksMax base, default 5.
/// DPS: PhysicalHitAverage * effect * stacks * chance/100.
pub fn calc_impale(env: &mut CalcEnv, cfg: &SkillCfg) {
    let output_snap = env.player.output.clone();

    let impale_chance = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "ImpaleChance", Some(cfg), &output_snap)
        .clamp(0.0, 100.0);
    env.player.set_output("ImpaleChance", impale_chance);

    if impale_chance <= 0.0 {
        env.player.set_output("ImpaleDPS", 0.0);
        env.player.set_output("ImpaleStacks", 0.0);
        return;
    }

    // Effect: base 10% (0.1), plus ImpaleEffect mods
    let effect_base =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ImpaleEffect", Some(cfg), &output_snap);
    let effect = if effect_base > 0.0 {
        effect_base / 100.0
    } else {
        0.1
    };

    // Max stacks: default 5
    let stacks_base =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ImpaleStacksMax", Some(cfg), &output_snap);
    let max_stacks = if stacks_base > 0.0 { stacks_base } else { 5.0 };

    // Physical hit average
    let phys_hit = get_output_f64(&output_snap, "PhysicalHitAverage");
    let phys_hit = if phys_hit > 0.0 {
        phys_hit
    } else {
        // Fallback: from min/max
        let min = get_output_f64(&output_snap, "PhysicalMin");
        let max = get_output_f64(&output_snap, "PhysicalMax");
        (min + max) / 2.0
    };

    let impale_dps = phys_hit * effect * max_stacks * (impale_chance / 100.0);

    env.player.set_output("ImpaleDPS", impale_dps);
    env.player.set_output("ImpaleStacks", max_stacks);
}

// ── Skill DoT (non-ailment) ─────────────────────────────────────────────────

/// Calculate non-ailment DoT per damage type.
///
/// Per damage type: query `{Type}Dot` base. If > 0, apply inc/more for that type
/// + generic Damage + DamageOverTime. Apply DotMultiplier.
pub fn calc_skill_dot(env: &mut CalcEnv, cfg: &SkillCfg) {
    let output_snap = env.player.output.clone();
    let mut total_dot = 0.0_f64;

    // Map damage type names to their dot multiplier name
    let dot_multi_names = [
        "PhysicalDotMultiplier",
        "LightningDotMultiplier",
        "ColdDotMultiplier",
        "FireDotMultiplier",
        "ChaosDotMultiplier",
    ];

    for (i, dtype) in DMG_TYPE_NAMES.iter().enumerate() {
        let dot_stat = format!("{}Dot", dtype);
        let base_dot = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, &dot_stat, Some(cfg), &output_snap);

        if base_dot <= 0.0 {
            continue;
        }

        // Inc: type-specific + generic Damage + DamageOverTime
        let inc = env.player.mod_db.sum_cfg(
            ModType::Inc,
            &format!("{}Damage", dtype),
            Some(cfg),
            &output_snap,
        ) + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "Damage", Some(cfg), &output_snap)
            + env
                .player
                .mod_db
                .sum_cfg(ModType::Inc, "DamageOverTime", Some(cfg), &output_snap);

        // More: type-specific + generic Damage + DamageOverTime
        let more = env
            .player
            .mod_db
            .more_cfg(&format!("{}Damage", dtype), Some(cfg), &output_snap)
            * env
                .player
                .mod_db
                .more_cfg("Damage", Some(cfg), &output_snap)
            * env
                .player
                .mod_db
                .more_cfg("DamageOverTime", Some(cfg), &output_snap);

        // DotMultiplier
        let dot_multi_base =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, dot_multi_names[i], Some(cfg), &output_snap)
                + env.player.mod_db.sum_cfg(
                    ModType::Base,
                    "DotMultiplier",
                    Some(cfg),
                    &output_snap,
                );
        let dot_multi = 1.0 + dot_multi_base / 100.0;

        let dot_dps = base_dot * (1.0 + inc / 100.0) * more * dot_multi;

        env.player.set_output(&dot_stat, dot_dps);
        total_dot += dot_dps;
    }

    env.player.set_output("TotalDot", total_dot);
}

// ── Combined DPS ────────────────────────────────────────────────────────────

/// Calculate combined DPS from all sources.
///
/// TotalDotDPS = ignite + bleed + poison + total_dot + decay.
/// CombinedDPS = (TotalDPS + TotalDotDPS + ImpaleDPS) * CullMultiplier * ReservationDpsMultiplier.
pub fn calc_combined_dps(env: &mut CalcEnv) {
    let output = &env.player.output.clone();

    let total_dps = get_output_f64(output, "TotalDPS");
    let ignite_dps = get_output_f64(output, "IgniteDPS");
    let bleed_dps = get_output_f64(output, "BleedDPS");
    let total_poison_dps = get_output_f64(output, "TotalPoisonDPS");
    let total_dot = get_output_f64(output, "TotalDot");
    let decay_dps = get_output_f64(output, "DecayDPS");
    let impale_dps = get_output_f64(output, "ImpaleDPS");

    let total_dot_dps = ignite_dps + bleed_dps + total_poison_dps + total_dot + decay_dps;
    env.player.set_output("TotalDotDPS", total_dot_dps);

    // Individual "With*DPS" outputs (hit DPS + that specific dot)
    env.player
        .set_output("WithIgniteDPS", total_dps + ignite_dps);
    env.player.set_output("WithBleedDPS", total_dps + bleed_dps);
    env.player
        .set_output("WithPoisonDPS", total_dps + total_poison_dps);
    env.player
        .set_output("WithImpaleDPS", total_dps + impale_dps);
    env.player
        .set_output("WithDotDPS", total_dps + total_dot_dps);

    // Culling strike
    let cull_pct = get_output_f64(output, "CullPercent");
    let cull_pct = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "CullPercent", None, output)
        .max(cull_pct);
    let cull_multiplier = if cull_pct > 0.0 && cull_pct < 100.0 {
        100.0 / (100.0 - cull_pct)
    } else {
        1.0
    };
    env.player.set_output("CullMultiplier", cull_multiplier);

    // Reservation DPS multiplier: CalcOffence.lua:3057
    // globalOutput.ReservationDpsMultiplier = 100 / (100 - enemyDB:Sum("BASE", nil, "LifeReservationPercent"))
    // This models skills that cause the enemy to "reserve" life (e.g. Arakaali's Fang).
    // At 0% enemy life reservation: 100 / (100 - 0) = 1.0 (no change).
    let enemy_life_reservation = {
        let enemy_output = env.enemy.output.clone();
        env.enemy
            .mod_db
            .sum_cfg(ModType::Base, "LifeReservationPercent", None, &enemy_output)
    };
    let reservation_mult = if enemy_life_reservation < 100.0 {
        100.0 / (100.0 - enemy_life_reservation)
    } else {
        1.0
    };
    env.player
        .set_output("ReservationDpsMultiplier", reservation_mult);

    // CombinedDPS before reservation: (TotalDPS + TotalDotDPS + ImpaleDPS) * CullMultiplier
    // CalcOffence.lua:5952: output.CombinedDPS = output.CombinedDPS * bestCull * output.ReservationDpsMultiplier
    let combined_before_reservation = (total_dps + total_dot_dps + impale_dps) * cull_multiplier;

    // ReservationDPS: CalcOffence.lua:5951
    // output.ReservationDPS = output.CombinedDPS * (output.ReservationDpsMultiplier - 1)
    // where output.CombinedDPS is the pre-reservation value (bestCull already applied)
    let reservation_dps = combined_before_reservation * (reservation_mult - 1.0);
    env.player.set_output("ReservationDPS", reservation_dps);

    // Final CombinedDPS includes reservation multiplier
    let combined_dps = combined_before_reservation * reservation_mult;
    env.player.set_output("CombinedDPS", combined_dps);
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::mod_db::types::{Mod, ModSource};
    use crate::mod_db::ModDb;
    use std::sync::Arc;

    fn src() -> ModSource {
        ModSource::new("Test", "test")
    }

    fn make_data() -> Arc<GameData> {
        Arc::new(GameData::from_json(crate::tests::stub_game_data_json()).unwrap())
    }

    fn default_cfg() -> SkillCfg {
        SkillCfg::default()
    }

    // ── Impale tests ─────────────────────────────────────────────────────

    #[test]
    fn impale_basic_dps() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("ImpaleChance", 100.0, src()));

        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("PhysicalHitAverage", 1000.0);

        let cfg = default_cfg();
        calc_impale(&mut env, &cfg);

        let impale_dps = get_output_f64(&env.player.output, "ImpaleDPS");
        // 1000 * 0.1 * 5 * 1.0 = 500
        assert!(
            (impale_dps - 500.0).abs() < 0.01,
            "ImpaleDPS should be 500.0, got {}",
            impale_dps
        );

        let stacks = get_output_f64(&env.player.output, "ImpaleStacks");
        assert!(
            (stacks - 5.0).abs() < 0.01,
            "ImpaleStacks should be 5.0, got {}",
            stacks
        );
    }

    #[test]
    fn impale_with_custom_effect_and_stacks() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("ImpaleChance", 50.0, src()));
        db.add(Mod::new_base("ImpaleEffect", 20.0, src())); // 20% effect
        db.add(Mod::new_base("ImpaleStacksMax", 7.0, src()));

        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("PhysicalHitAverage", 1000.0);

        let cfg = default_cfg();
        calc_impale(&mut env, &cfg);

        let impale_dps = get_output_f64(&env.player.output, "ImpaleDPS");
        // 1000 * 0.2 * 7 * 0.5 = 700
        assert!(
            (impale_dps - 700.0).abs() < 0.01,
            "ImpaleDPS should be 700.0, got {}",
            impale_dps
        );
    }

    #[test]
    fn impale_zero_chance() {
        let db = ModDb::new();
        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("PhysicalHitAverage", 1000.0);

        let cfg = default_cfg();
        calc_impale(&mut env, &cfg);

        let impale_dps = get_output_f64(&env.player.output, "ImpaleDPS");
        assert!(
            impale_dps.abs() < 0.01,
            "ImpaleDPS should be 0 with 0% chance, got {}",
            impale_dps
        );
    }

    // ── Skill DoT tests ─────────────────────────────────────────────────

    #[test]
    fn skill_dot_basic() {
        let mut db = ModDb::new();
        // Base fire dot of 100 DPS
        db.add(Mod::new_base("FireDot", 100.0, src()));

        let mut env = CalcEnv::new(db, ModDb::new(), make_data());

        let cfg = default_cfg();
        calc_skill_dot(&mut env, &cfg);

        let fire_dot = get_output_f64(&env.player.output, "FireDot");
        assert!(
            (fire_dot - 100.0).abs() < 0.01,
            "FireDot should be 100.0, got {}",
            fire_dot
        );

        let total = get_output_f64(&env.player.output, "TotalDot");
        assert!(
            (total - 100.0).abs() < 0.01,
            "TotalDot should be 100.0, got {}",
            total
        );
    }

    #[test]
    fn skill_dot_no_base_produces_zero() {
        let db = ModDb::new();
        let mut env = CalcEnv::new(db, ModDb::new(), make_data());

        let cfg = default_cfg();
        calc_skill_dot(&mut env, &cfg);

        let total = get_output_f64(&env.player.output, "TotalDot");
        assert!(total.abs() < 0.01, "TotalDot should be 0.0, got {}", total);
    }

    // ── Combined DPS tests ──────────────────────────────────────────────

    #[test]
    fn combined_dps_basic_assembly() {
        let db = ModDb::new();
        let mut env = CalcEnv::new(db, ModDb::new(), make_data());

        env.player.set_output("TotalDPS", 1000.0);
        env.player.set_output("IgniteDPS", 100.0);
        env.player.set_output("BleedDPS", 50.0);
        env.player.set_output("TotalPoisonDPS", 200.0);
        env.player.set_output("TotalDot", 0.0);
        env.player.set_output("DecayDPS", 0.0);
        env.player.set_output("ImpaleDPS", 300.0);

        calc_combined_dps(&mut env);

        let total_dot_dps = get_output_f64(&env.player.output, "TotalDotDPS");
        // 100 + 50 + 200 + 0 + 0 = 350
        assert!(
            (total_dot_dps - 350.0).abs() < 0.01,
            "TotalDotDPS should be 350.0, got {}",
            total_dot_dps
        );

        let combined = get_output_f64(&env.player.output, "CombinedDPS");
        // (1000 + 350 + 300) * 1.0 = 1650
        assert!(
            (combined - 1650.0).abs() < 0.01,
            "CombinedDPS should be 1650.0, got {}",
            combined
        );

        // With* outputs
        let with_ignite = get_output_f64(&env.player.output, "WithIgniteDPS");
        assert!(
            (with_ignite - 1100.0).abs() < 0.01,
            "WithIgniteDPS should be 1100.0, got {}",
            with_ignite
        );
    }

    #[test]
    fn combined_dps_with_culling() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("CullPercent", 20.0, src()));

        let mut env = CalcEnv::new(db, ModDb::new(), make_data());

        env.player.set_output("TotalDPS", 1000.0);
        env.player.set_output("IgniteDPS", 0.0);
        env.player.set_output("BleedDPS", 0.0);
        env.player.set_output("TotalPoisonDPS", 0.0);
        env.player.set_output("TotalDot", 0.0);
        env.player.set_output("DecayDPS", 0.0);
        env.player.set_output("ImpaleDPS", 0.0);

        calc_combined_dps(&mut env);

        let cull_multi = get_output_f64(&env.player.output, "CullMultiplier");
        // 100 / (100 - 20) = 1.25
        assert!(
            (cull_multi - 1.25).abs() < 0.01,
            "CullMultiplier should be 1.25, got {}",
            cull_multi
        );

        let combined = get_output_f64(&env.player.output, "CombinedDPS");
        // 1000 * 1.25 = 1250
        assert!(
            (combined - 1250.0).abs() < 0.01,
            "CombinedDPS with culling should be 1250.0, got {}",
            combined
        );
    }
}
