//! Ailment DPS calculations — ignite, bleed, poison.
//! Mirrors CalcOffence.lua ailment sections from Path of Building.

use super::env::{get_output_f64, CalcEnv, OutputTable};
use crate::mod_db::types::{ModType, SkillCfg};

/// Context values from the main offence calculation needed by ailment functions.
pub struct AilmentContext {
    pub crit_chance: f64,
    pub crit_multiplier: f64,
    pub hit_chance: f64,
    pub speed: f64,
    pub is_attack: bool,
}

// ── Ignite ──────────────────────────────────────────────────────────────────

/// Calculate ignite DPS and related outputs.
///
/// Source: fire hit average (or all types if ShaperOfFlames).
/// Formula (3.16+): base_dps = source * 0.9 / 4.0 (90% of hit over 4s).
pub fn calc_ignite(env: &mut CalcEnv, cfg: &SkillCfg, ctx: &AilmentContext) {
    let output_snap = env.player.output.clone();

    // Ignite chance
    let ignite_chance = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "EnemyIgniteChance", Some(cfg), &output_snap)
        .clamp(0.0, 100.0);
    env.player.set_output("IgniteChanceOnHit", ignite_chance);

    if ignite_chance <= 0.0 {
        env.player.set_output("IgniteDPS", 0.0);
        env.player.set_output("IgniteDuration", 0.0);
        env.player.set_output("IgniteDamage", 0.0);
        return;
    }

    // Source damage: fire hit average, or all types if ShaperOfFlames
    let shaper_of_flames = env
        .player
        .mod_db
        .flag_cfg("ShaperOfFlames", Some(cfg), &output_snap);

    let source = if shaper_of_flames {
        // Sum all damage type hit averages
        get_output_f64(&output_snap, "PhysicalHitAverage")
            + get_output_f64(&output_snap, "LightningHitAverage")
            + get_output_f64(&output_snap, "ColdHitAverage")
            + get_output_f64(&output_snap, "FireHitAverage")
            + get_output_f64(&output_snap, "ChaosHitAverage")
    } else {
        get_output_f64(&output_snap, "FireHitAverage")
    };

    if source <= 0.0 {
        // Fallback: compute from post-resist min/max
        let fire_min = get_output_f64(&output_snap, "FireMin");
        let fire_max = get_output_f64(&output_snap, "FireMax");
        let avg = (fire_min + fire_max) / 2.0;
        let crit_rate = ctx.crit_chance / 100.0;
        let crit_weighted = avg * (1.0 - crit_rate) + avg * ctx.crit_multiplier * crit_rate;
        calc_ignite_from_source(env, cfg, ctx, crit_weighted * ctx.hit_chance, &output_snap);
        return;
    }

    calc_ignite_from_source(env, cfg, ctx, source, &output_snap);
}

fn calc_ignite_from_source(
    env: &mut CalcEnv,
    cfg: &SkillCfg,
    _ctx: &AilmentContext,
    source: f64,
    output_snap: &OutputTable,
) {
    // Base DPS: 90% of hit over 4 seconds
    let base_dps = source * 0.9 / 4.0;

    // Inc mods: FireDamage + BurningDamage + ElementalDamage + Damage + AilmentDamage + DamageOverTime
    let inc = env
        .player
        .mod_db
        .sum_cfg(ModType::Inc, "FireDamage", Some(cfg), output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "BurningDamage", Some(cfg), output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "ElementalDamage", Some(cfg), output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "Damage", Some(cfg), output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "AilmentDamage", Some(cfg), output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "DamageOverTime", Some(cfg), output_snap);

    // More mods: same names, multiply together
    let more = env
        .player
        .mod_db
        .more_cfg("FireDamage", Some(cfg), output_snap)
        * env
            .player
            .mod_db
            .more_cfg("BurningDamage", Some(cfg), output_snap)
        * env
            .player
            .mod_db
            .more_cfg("ElementalDamage", Some(cfg), output_snap)
        * env.player.mod_db.more_cfg("Damage", Some(cfg), output_snap)
        * env
            .player
            .mod_db
            .more_cfg("AilmentDamage", Some(cfg), output_snap)
        * env
            .player
            .mod_db
            .more_cfg("DamageOverTime", Some(cfg), output_snap);

    // DotMultiplier: 1 + (FireDotMultiplier + DotMultiplier) base / 100
    let dot_multi_base =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "FireDotMultiplier", Some(cfg), output_snap)
            + env
                .player
                .mod_db
                .sum_cfg(ModType::Base, "DotMultiplier", Some(cfg), output_snap);
    let dot_multi = 1.0 + dot_multi_base / 100.0;

    let ignite_dps = base_dps * (1.0 + inc / 100.0) * more * dot_multi;

    // Duration: 4.0 * (1 + EnemyIgniteDuration inc / 100) * EnemyIgniteDuration more
    let dur_inc =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "EnemyIgniteDuration", Some(cfg), output_snap);
    let dur_more = env
        .player
        .mod_db
        .more_cfg("EnemyIgniteDuration", Some(cfg), output_snap);
    let duration = 4.0 * (1.0 + dur_inc / 100.0) * dur_more;

    let ignite_damage = ignite_dps * duration;

    env.player.set_output("IgniteDPS", ignite_dps);
    env.player.set_output("IgniteDuration", duration);
    env.player.set_output("IgniteDamage", ignite_damage);
}

// ── Bleed ───────────────────────────────────────────────────────────────────

/// Calculate bleed DPS and related outputs.
///
/// Source: physical hit average.
/// Formula: base_dps = source * 0.7 / 5.0 (70% of hit over 5s).
pub fn calc_bleed(env: &mut CalcEnv, cfg: &SkillCfg, ctx: &AilmentContext) {
    let output_snap = env.player.output.clone();

    // Bleed chance
    let bleed_chance = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "BleedChance", Some(cfg), &output_snap)
        .clamp(0.0, 100.0);
    env.player.set_output("BleedChanceOnHit", bleed_chance);

    if bleed_chance <= 0.0 {
        env.player.set_output("BleedDPS", 0.0);
        env.player.set_output("BleedMovingDPS", 0.0);
        env.player.set_output("BleedDuration", 0.0);
        return;
    }

    // Source: physical hit average
    let source = {
        let phys_hit = get_output_f64(&output_snap, "PhysicalHitAverage");
        if phys_hit > 0.0 {
            phys_hit
        } else {
            // Fallback: compute from post-resist min/max
            let phys_min = get_output_f64(&output_snap, "PhysicalMin");
            let phys_max = get_output_f64(&output_snap, "PhysicalMax");
            let avg = (phys_min + phys_max) / 2.0;
            let crit_rate = ctx.crit_chance / 100.0;
            let crit_weighted = avg * (1.0 - crit_rate) + avg * ctx.crit_multiplier * crit_rate;
            crit_weighted * ctx.hit_chance
        }
    };

    // Base DPS: 70% of hit over 5 seconds
    let base_dps = source * 0.7 / 5.0;

    // Inc mods
    let inc = env
        .player
        .mod_db
        .sum_cfg(ModType::Inc, "PhysicalDamage", Some(cfg), &output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "BleedDamage", Some(cfg), &output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "Damage", Some(cfg), &output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "AilmentDamage", Some(cfg), &output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "DamageOverTime", Some(cfg), &output_snap);

    // More mods
    let more = env
        .player
        .mod_db
        .more_cfg("PhysicalDamage", Some(cfg), &output_snap)
        * env
            .player
            .mod_db
            .more_cfg("BleedDamage", Some(cfg), &output_snap)
        * env
            .player
            .mod_db
            .more_cfg("Damage", Some(cfg), &output_snap)
        * env
            .player
            .mod_db
            .more_cfg("AilmentDamage", Some(cfg), &output_snap)
        * env
            .player
            .mod_db
            .more_cfg("DamageOverTime", Some(cfg), &output_snap);

    // DotMultiplier
    let dot_multi_base =
        env.player.mod_db.sum_cfg(
            ModType::Base,
            "PhysicalDotMultiplier",
            Some(cfg),
            &output_snap,
        ) + env
            .player
            .mod_db
            .sum_cfg(ModType::Base, "DotMultiplier", Some(cfg), &output_snap);
    let dot_multi = 1.0 + dot_multi_base / 100.0;

    let bleed_dps = base_dps * (1.0 + inc / 100.0) * more * dot_multi;

    // Moving multiplier: 140% while moving (default)
    let bleed_moving_dps = bleed_dps * 1.4;

    // Duration: 5.0 * (1 + EnemyBleedDuration inc / 100)
    let dur_inc =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "EnemyBleedDuration", Some(cfg), &output_snap);
    let duration = 5.0 * (1.0 + dur_inc / 100.0);

    // Crimson Dance: max 8 stacks
    let crimson_dance = env
        .player
        .mod_db
        .flag_cfg("CrimsonDance", Some(cfg), &output_snap);

    if crimson_dance {
        let stacks_raw = ctx.speed * (ctx.hit_chance / 100.0) * (bleed_chance / 100.0) * duration;
        let stacks = stacks_raw.min(8.0);
        let total_bleed_dps = bleed_dps * stacks;
        env.player.set_output("BleedDPS", total_bleed_dps);
        env.player.set_output("BleedMovingDPS", total_bleed_dps); // Crimson Dance: no moving bonus
        env.player.set_output("BleedStacks", stacks);
    } else {
        env.player.set_output("BleedDPS", bleed_dps);
        env.player.set_output("BleedMovingDPS", bleed_moving_dps);
    }

    env.player.set_output("BleedDuration", duration);
}

// ── Poison ──────────────────────────────────────────────────────────────────

/// Calculate poison DPS and related outputs.
///
/// Source: physical + chaos hit average.
/// Formula: base_dps_per_stack = source * 0.3 / 2.0 (30% of hit over 2s).
pub fn calc_poison(env: &mut CalcEnv, cfg: &SkillCfg, ctx: &AilmentContext) {
    let output_snap = env.player.output.clone();

    // Poison chance
    let poison_chance = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "PoisonChance", Some(cfg), &output_snap)
        .clamp(0.0, 100.0);
    env.player.set_output("PoisonChanceOnHit", poison_chance);

    if poison_chance <= 0.0 {
        env.player.set_output("PoisonDPS", 0.0);
        env.player.set_output("TotalPoisonDPS", 0.0);
        env.player.set_output("PoisonDuration", 0.0);
        env.player.set_output("PoisonStacks", 0.0);
        env.player.set_output("PoisonDamage", 0.0);
        return;
    }

    // Source: physical + chaos hit average
    let source = {
        let phys_hit = get_output_f64(&output_snap, "PhysicalHitAverage");
        let chaos_hit = get_output_f64(&output_snap, "ChaosHitAverage");
        let sum = phys_hit + chaos_hit;
        if sum > 0.0 {
            sum
        } else {
            // Fallback: compute from post-resist min/max
            let phys_min = get_output_f64(&output_snap, "PhysicalMin");
            let phys_max = get_output_f64(&output_snap, "PhysicalMax");
            let chaos_min = get_output_f64(&output_snap, "ChaosMin");
            let chaos_max = get_output_f64(&output_snap, "ChaosMax");
            let avg = (phys_min + phys_max + chaos_min + chaos_max) / 2.0;
            let crit_rate = ctx.crit_chance / 100.0;
            let crit_weighted = avg * (1.0 - crit_rate) + avg * ctx.crit_multiplier * crit_rate;
            crit_weighted * ctx.hit_chance
        }
    };

    // Base DPS per stack: 30% of hit over 2 seconds
    let base_dps = source * 0.3 / 2.0;

    // Inc mods
    let inc = env
        .player
        .mod_db
        .sum_cfg(ModType::Inc, "ChaosDamage", Some(cfg), &output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "PoisonDamage", Some(cfg), &output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "Damage", Some(cfg), &output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "AilmentDamage", Some(cfg), &output_snap)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "DamageOverTime", Some(cfg), &output_snap);

    // More mods
    let more = env
        .player
        .mod_db
        .more_cfg("ChaosDamage", Some(cfg), &output_snap)
        * env
            .player
            .mod_db
            .more_cfg("PoisonDamage", Some(cfg), &output_snap)
        * env
            .player
            .mod_db
            .more_cfg("Damage", Some(cfg), &output_snap)
        * env
            .player
            .mod_db
            .more_cfg("AilmentDamage", Some(cfg), &output_snap)
        * env
            .player
            .mod_db
            .more_cfg("DamageOverTime", Some(cfg), &output_snap);

    // DotMultiplier
    let dot_multi_base =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ChaosDotMultiplier", Some(cfg), &output_snap)
            + env
                .player
                .mod_db
                .sum_cfg(ModType::Base, "DotMultiplier", Some(cfg), &output_snap);
    let dot_multi = 1.0 + dot_multi_base / 100.0;

    let poison_dps_per_stack = base_dps * (1.0 + inc / 100.0) * more * dot_multi;

    // Duration: 2.0 * (1 + EnemyPoisonDuration inc / 100) * EnemyPoisonDuration more
    let dur_inc =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "EnemyPoisonDuration", Some(cfg), &output_snap);
    let dur_more = env
        .player
        .mod_db
        .more_cfg("EnemyPoisonDuration", Some(cfg), &output_snap);
    let duration = 2.0 * (1.0 + dur_inc / 100.0) * dur_more;

    // Stack count: speed * hit_chance/100 * poison_chance/100 * duration
    let stacks = ctx.speed * (ctx.hit_chance / 100.0) * (poison_chance / 100.0) * duration;

    let total_poison_dps = poison_dps_per_stack * stacks;
    let poison_damage = poison_dps_per_stack * duration;

    env.player.set_output("PoisonDPS", poison_dps_per_stack);
    env.player.set_output("TotalPoisonDPS", total_poison_dps);
    env.player.set_output("PoisonChanceOnHit", poison_chance);
    env.player.set_output("PoisonDuration", duration);
    env.player.set_output("PoisonStacks", stacks);
    env.player.set_output("PoisonDamage", poison_damage);
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::mod_db::types::{Mod, ModSource, ModValue};
    use crate::mod_db::ModDb;
    use std::collections::HashMap;
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

    fn default_ctx() -> AilmentContext {
        AilmentContext {
            crit_chance: 0.0,
            crit_multiplier: 1.5,
            hit_chance: 100.0,
            speed: 1.0,
            is_attack: true,
        }
    }

    // ── Ignite tests ─────────────────────────────────────────────────────

    #[test]
    fn ignite_basic_fire_damage() {
        let db = ModDb::new();
        let mut env = CalcEnv::new(db, ModDb::new(), make_data());

        // Set fire hit average (simulating post-hit fire damage)
        env.player.set_output("FireHitAverage", 1000.0);

        // Add ignite chance
        env.player
            .mod_db
            .add(Mod::new_base("EnemyIgniteChance", 100.0, src()));

        let cfg = default_cfg();
        let ctx = default_ctx();

        calc_ignite(&mut env, &cfg, &ctx);

        let ignite_dps = get_output_f64(&env.player.output, "IgniteDPS");
        // Base: 1000 * 0.9 / 4.0 = 225
        assert!(
            (ignite_dps - 225.0).abs() < 0.01,
            "IgniteDPS should be 225.0, got {}",
            ignite_dps
        );

        let duration = get_output_f64(&env.player.output, "IgniteDuration");
        assert!(
            (duration - 4.0).abs() < 0.01,
            "IgniteDuration should be 4.0, got {}",
            duration
        );

        let ignite_damage = get_output_f64(&env.player.output, "IgniteDamage");
        assert!(
            (ignite_damage - 900.0).abs() < 0.01,
            "IgniteDamage should be 900.0 (225 * 4), got {}",
            ignite_damage
        );

        let chance = get_output_f64(&env.player.output, "IgniteChanceOnHit");
        assert!(
            (chance - 100.0).abs() < 0.01,
            "IgniteChanceOnHit should be 100.0, got {}",
            chance
        );
    }

    #[test]
    fn ignite_zero_chance_produces_zero_dps() {
        let db = ModDb::new();
        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("FireHitAverage", 1000.0);

        let cfg = default_cfg();
        let ctx = default_ctx();

        calc_ignite(&mut env, &cfg, &ctx);

        let ignite_dps = get_output_f64(&env.player.output, "IgniteDPS");
        assert!(
            ignite_dps.abs() < 0.01,
            "IgniteDPS should be 0 with 0% chance, got {}",
            ignite_dps
        );
    }

    #[test]
    fn ignite_with_dot_multiplier() {
        let mut db = ModDb::new();
        // 50% fire dot multiplier
        db.add(Mod::new_base("FireDotMultiplier", 50.0, src()));
        db.add(Mod::new_base("EnemyIgniteChance", 100.0, src()));

        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("FireHitAverage", 1000.0);

        let cfg = default_cfg();
        let ctx = default_ctx();

        calc_ignite(&mut env, &cfg, &ctx);

        let ignite_dps = get_output_f64(&env.player.output, "IgniteDPS");
        // Base: 225.0 * (1 + 50/100) = 225 * 1.5 = 337.5
        assert!(
            (ignite_dps - 337.5).abs() < 0.01,
            "IgniteDPS with 50% dot multi should be 337.5, got {}",
            ignite_dps
        );
    }

    #[test]
    fn ignite_with_inc_burning_damage() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("EnemyIgniteChance", 100.0, src()));
        // 100% increased burning damage
        db.add(Mod {
            name: "BurningDamage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(100.0),
            flags: Default::default(),
            keyword_flags: Default::default(),
            tags: vec![],
            source: src(),
        });

        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("FireHitAverage", 1000.0);

        let cfg = default_cfg();
        let ctx = default_ctx();

        calc_ignite(&mut env, &cfg, &ctx);

        let ignite_dps = get_output_f64(&env.player.output, "IgniteDPS");
        // Base: 225.0 * (1 + 100/100) = 225 * 2.0 = 450
        assert!(
            (ignite_dps - 450.0).abs() < 0.01,
            "IgniteDPS with 100% inc burning should be 450, got {}",
            ignite_dps
        );
    }

    // ── Bleed tests ──────────────────────────────────────────────────────

    #[test]
    fn bleed_basic_physical_damage() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("BleedChance", 100.0, src()));

        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("PhysicalHitAverage", 1000.0);

        let cfg = default_cfg();
        let ctx = default_ctx();

        calc_bleed(&mut env, &cfg, &ctx);

        let bleed_dps = get_output_f64(&env.player.output, "BleedDPS");
        // Base: 1000 * 0.7 / 5.0 = 140
        assert!(
            (bleed_dps - 140.0).abs() < 0.01,
            "BleedDPS should be 140.0, got {}",
            bleed_dps
        );

        let moving_dps = get_output_f64(&env.player.output, "BleedMovingDPS");
        // Moving: 140 * 1.4 = 196
        assert!(
            (moving_dps - 196.0).abs() < 0.01,
            "BleedMovingDPS should be 196.0, got {}",
            moving_dps
        );

        let duration = get_output_f64(&env.player.output, "BleedDuration");
        assert!(
            (duration - 5.0).abs() < 0.01,
            "BleedDuration should be 5.0, got {}",
            duration
        );
    }

    #[test]
    fn bleed_zero_chance_produces_zero_dps() {
        let db = ModDb::new();
        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("PhysicalHitAverage", 1000.0);

        let cfg = default_cfg();
        let ctx = default_ctx();

        calc_bleed(&mut env, &cfg, &ctx);

        let bleed_dps = get_output_f64(&env.player.output, "BleedDPS");
        assert!(
            bleed_dps.abs() < 0.01,
            "BleedDPS should be 0 with 0% chance, got {}",
            bleed_dps
        );
    }

    #[test]
    fn bleed_crimson_dance_stacks() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("BleedChance", 100.0, src()));
        db.add(Mod::new_flag("CrimsonDance", src()));

        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("PhysicalHitAverage", 1000.0);

        let cfg = default_cfg();
        let ctx = AilmentContext {
            crit_chance: 0.0,
            crit_multiplier: 1.5,
            hit_chance: 100.0,
            speed: 2.0, // 2 attacks/sec
            is_attack: true,
        };

        calc_bleed(&mut env, &cfg, &ctx);

        let bleed_dps = get_output_f64(&env.player.output, "BleedDPS");
        let stacks = get_output_f64(&env.player.output, "BleedStacks");

        // Stacks = speed * hit_chance * bleed_chance * duration = 2.0 * 1.0 * 1.0 * 5.0 = 10.0 → capped at 8
        assert!(
            (stacks - 8.0).abs() < 0.01,
            "BleedStacks should be 8.0 (capped), got {}",
            stacks
        );

        // Per stack DPS = 1000 * 0.7 / 5.0 = 140. Total = 140 * 8 = 1120
        assert!(
            (bleed_dps - 1120.0).abs() < 0.01,
            "BleedDPS with Crimson Dance should be 1120.0, got {}",
            bleed_dps
        );
    }

    // ── Poison tests ─────────────────────────────────────────────────────

    #[test]
    fn poison_basic_stacks_with_speed() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("PoisonChance", 100.0, src()));

        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("PhysicalHitAverage", 500.0);
        env.player.set_output("ChaosHitAverage", 500.0);

        let cfg = default_cfg();
        let ctx = AilmentContext {
            crit_chance: 0.0,
            crit_multiplier: 1.5,
            hit_chance: 100.0,
            speed: 4.0, // 4 hits/sec
            is_attack: true,
        };

        calc_poison(&mut env, &cfg, &ctx);

        let poison_dps = get_output_f64(&env.player.output, "PoisonDPS");
        // Per stack: (500+500) * 0.3 / 2.0 = 150
        assert!(
            (poison_dps - 150.0).abs() < 0.01,
            "PoisonDPS per stack should be 150.0, got {}",
            poison_dps
        );

        let stacks = get_output_f64(&env.player.output, "PoisonStacks");
        // Stacks = 4.0 * 1.0 * 1.0 * 2.0 = 8
        assert!(
            (stacks - 8.0).abs() < 0.01,
            "PoisonStacks should be 8.0, got {}",
            stacks
        );

        let total_dps = get_output_f64(&env.player.output, "TotalPoisonDPS");
        // 150 * 8 = 1200
        assert!(
            (total_dps - 1200.0).abs() < 0.01,
            "TotalPoisonDPS should be 1200.0, got {}",
            total_dps
        );

        let poison_damage = get_output_f64(&env.player.output, "PoisonDamage");
        // 150 * 2.0 = 300
        assert!(
            (poison_damage - 300.0).abs() < 0.01,
            "PoisonDamage should be 300.0, got {}",
            poison_damage
        );

        let duration = get_output_f64(&env.player.output, "PoisonDuration");
        assert!(
            (duration - 2.0).abs() < 0.01,
            "PoisonDuration should be 2.0, got {}",
            duration
        );
    }

    #[test]
    fn poison_zero_chance_produces_zero_dps() {
        let db = ModDb::new();
        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("PhysicalHitAverage", 1000.0);

        let cfg = default_cfg();
        let ctx = default_ctx();

        calc_poison(&mut env, &cfg, &ctx);

        let total = get_output_f64(&env.player.output, "TotalPoisonDPS");
        assert!(
            total.abs() < 0.01,
            "TotalPoisonDPS should be 0 with 0% chance, got {}",
            total
        );
    }

    #[test]
    fn poison_with_dot_multiplier() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("PoisonChance", 100.0, src()));
        db.add(Mod::new_base("ChaosDotMultiplier", 40.0, src()));
        db.add(Mod::new_base("DotMultiplier", 10.0, src()));

        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("PhysicalHitAverage", 500.0);
        env.player.set_output("ChaosHitAverage", 500.0);

        let cfg = default_cfg();
        let ctx = AilmentContext {
            crit_chance: 0.0,
            crit_multiplier: 1.5,
            hit_chance: 100.0,
            speed: 1.0,
            is_attack: true,
        };

        calc_poison(&mut env, &cfg, &ctx);

        let poison_dps = get_output_f64(&env.player.output, "PoisonDPS");
        // Per stack: 150 * (1 + (40+10)/100) = 150 * 1.5 = 225
        assert!(
            (poison_dps - 225.0).abs() < 0.01,
            "PoisonDPS with dot multi should be 225, got {}",
            poison_dps
        );
    }

    #[test]
    fn poison_with_increased_duration() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("PoisonChance", 100.0, src()));
        // 50% increased poison duration
        db.add(Mod {
            name: "EnemyPoisonDuration".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(50.0),
            flags: Default::default(),
            keyword_flags: Default::default(),
            tags: vec![],
            source: src(),
        });

        let mut env = CalcEnv::new(db, ModDb::new(), make_data());
        env.player.set_output("PhysicalHitAverage", 1000.0);

        let cfg = default_cfg();
        let ctx = AilmentContext {
            crit_chance: 0.0,
            crit_multiplier: 1.5,
            hit_chance: 100.0,
            speed: 1.0,
            is_attack: true,
        };

        calc_poison(&mut env, &cfg, &ctx);

        let duration = get_output_f64(&env.player.output, "PoisonDuration");
        // 2.0 * (1 + 50/100) = 3.0
        assert!(
            (duration - 3.0).abs() < 0.01,
            "PoisonDuration should be 3.0, got {}",
            duration
        );

        let stacks = get_output_f64(&env.player.output, "PoisonStacks");
        // 1.0 * 1.0 * 1.0 * 3.0 = 3.0
        assert!(
            (stacks - 3.0).abs() < 0.01,
            "PoisonStacks should be 3.0, got {}",
            stacks
        );
    }
}
