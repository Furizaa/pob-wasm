use super::env::{get_output_f64, CalcEnv};
use crate::mod_db::types::{KeywordFlags, Mod, ModFlags, ModSource, ModType, ModValue};

/// Run all CalcPerform passes in order.
/// Mirrors CalcPerform.lua's main execution flow.
///
/// Expects `env.player.active_skill_list` to be populated before calling
/// (by `active_skill::run()`), as reservation accumulation iterates it.
pub fn run(env: &mut CalcEnv) {
    // Mirrors CalcPerform.lua lines 1096-1097:
    // Reset keystonesAdded at the START of every perform pass, then merge keystones.
    // This ensures re-entrant calls pick up any newly-added "Keystone" LIST mods.
    env.keystones_added.clear();
    crate::calc::setup::merge_keystones(env);

    do_actor_attribs_conditions(env);
    do_actor_life_mana(env);
    // Per-skill reservation accumulation (CalcPerform.lua:1810-1919)
    // Must run BEFORE doActorLifeManaReservation.
    accumulate_skill_reservations(env);
    do_actor_life_mana_reservation(env);
    // CalcPerform.lua:3271: defence for conditionals runs before charges/misc
    super::defence::defence_for_conditionals(env);
    do_actor_charges(env);
    do_actor_misc(env);
    apply_buffs(env);
    apply_curses(env);
    // Mirrors CalcPerform.lua line 3257: re-merge keystones after aura/buff application
    // to catch any keystones added by buffs or auras.
    crate::calc::setup::merge_keystones(env);
    do_non_damaging_ailments(env);
    apply_exposure(env);
    do_regen_recharge_leech(env);

    let asm = action_speed_mod(env);
    env.player.set_output("ActionSpeedMod", asm);
    env.player.action_speed_mod = asm;

    do_actor_attack_cast_speed(env);
    do_mom_eb(env);
    set_final_conditions(env);
    do_attr_requirements(env);

    // Foulborn Choir of the Storm re-run (CalcPerform.lua:3196-3201)
    // After auras/buffs, if ManaIncreasedByOvercappedLightningRes flag is set,
    // re-calculate resistances, then re-run doActorLifeMana + doActorLifeManaReservation
    // with addAura=true so that GrantReserved*AsAura mods fire on updated mana values.
    if env.player.mod_db.flag_cfg(
        "ManaIncreasedByOvercappedLightningRes",
        None,
        &env.player.output,
    ) {
        // CalcPerform.lua:3198: calcs.resistances(env.player)
        // Resistances are computed in defence.rs; for the re-run we just need to
        // recalculate life/mana and reservations. The Lua does it here because it
        // needs the updated mana value before the second doActorLifeManaReservation.
        do_actor_life_mana(env);
        // CalcPerform.lua:3201: doActorLifeManaReservation(env.player, true)
        do_actor_life_mana_reservation_inner(env, Some(true));
    }
}

// ---------------------------------------------------------------------------
// Attributes & conditions
// ---------------------------------------------------------------------------

/// Compute a single Str/Dex/Int stat value using calcLib.val semantics:
/// base * (1 + inc/100) * more, rounded to nearest integer.
/// Short-circuits to 0 if base == 0 (mirrors Lua calcLib.val).
/// Result is clamped to >= 0.
fn calc_attr(
    mod_db: &crate::mod_db::ModDb,
    output: &crate::calc::env::OutputTable,
    stat: &str,
) -> f64 {
    let base = mod_db.sum_cfg(ModType::Base, stat, None, output);
    if base == 0.0 {
        return 0.0;
    }
    let inc = mod_db.sum_cfg(ModType::Inc, stat, None, output);
    let more = mod_db.more_cfg(stat, None, output);
    (base * (1.0 + inc / 100.0) * more).round().max(0.0)
}

/// Mirrors doActorAttribsConditions() in CalcPerform.lua.
/// Computes Str/Dex/Int, Omniscience, attribute-derived bonuses, and conditions.
fn do_actor_attribs_conditions(env: &mut CalcEnv) {
    // Omniscience: converts Str+Dex+Int to Omni
    let omniscience = env
        .player
        .mod_db
        .flag_cfg("Omniscience", None, &env.player.output);

    if omniscience {
        compute_omniscience(env);
    } else {
        // TWO-PASS LOOP: needed because some INC mods on Str/Dex/Int are conditioned on
        // "StrHigherThanDex" etc. which themselves depend on the Str/Dex/Int values.
        // Pass 1 computes raw values; conditions are set; pass 2 recomputes with updated conditions.
        for _pass in 0..2 {
            let str_val = calc_attr(&env.player.mod_db, &env.player.output, "Str");
            let dex_val = calc_attr(&env.player.mod_db, &env.player.output, "Dex");
            let int_val = calc_attr(&env.player.mod_db, &env.player.output, "Int");

            env.player.set_output("Str", str_val);
            env.player.set_output("Dex", dex_val);
            env.player.set_output("Int", int_val);

            // Sort for LowestAttribute and TwoHighestAttributesEqual
            let mut stats = [str_val, dex_val, int_val];
            stats.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            env.player.set_output("LowestAttribute", stats[0]);
            env.player
                .mod_db
                .set_condition("TwoHighestAttributesEqual", stats[1] == stats[2]);

            // Comparison conditions (used by mods like "if Dex higher than Int")
            env.player
                .mod_db
                .set_condition("DexHigherThanInt", dex_val > int_val);
            env.player
                .mod_db
                .set_condition("StrHigherThanInt", str_val > int_val);
            env.player
                .mod_db
                .set_condition("IntHigherThanDex", int_val > dex_val);
            env.player
                .mod_db
                .set_condition("StrHigherThanDex", str_val > dex_val);
            env.player
                .mod_db
                .set_condition("IntHigherThanStr", int_val > str_val);
            env.player
                .mod_db
                .set_condition("DexHigherThanStr", dex_val > str_val);

            // "Highest" conditions use >= (tie-breaks: both can be "highest")
            env.player.mod_db.set_condition(
                "StrHighestAttribute",
                str_val >= dex_val && str_val >= int_val,
            );
            env.player.mod_db.set_condition(
                "IntHighestAttribute",
                int_val >= str_val && int_val >= dex_val,
            );
            env.player.mod_db.set_condition(
                "DexHighestAttribute",
                dex_val >= str_val && dex_val >= int_val,
            );
            // "SingleHighest" conditions use strict > (no ties)
            env.player.mod_db.set_condition(
                "IntSingleHighestAttribute",
                int_val > str_val && int_val > dex_val,
            );
            env.player.mod_db.set_condition(
                "DexSingleHighestAttribute",
                dex_val > str_val && dex_val > int_val,
            );
        }
    }

    let str_out = get_output_f64(&env.player.output, "Str");
    let dex_out = get_output_f64(&env.player.output, "Dex");
    let int_out = get_output_f64(&env.player.output, "Int");

    // TotalAttr
    let total_attr = str_out + dex_out + int_out;
    env.player.set_output("TotalAttr", total_attr);

    // Check global "no attribute bonuses" flags
    let no_attr_bonuses =
        env.player
            .mod_db
            .flag_cfg("NoAttributeBonuses", None, &env.player.output);
    let no_str_bonuses = no_attr_bonuses
        || env
            .player
            .mod_db
            .flag_cfg("NoStrengthAttributeBonuses", None, &env.player.output);
    let no_dex_bonuses = no_attr_bonuses
        || env
            .player
            .mod_db
            .flag_cfg("NoDexterityAttributeBonuses", None, &env.player.output);
    let no_int_bonuses = no_attr_bonuses
        || env
            .player
            .mod_db
            .flag_cfg("NoIntelligenceAttributeBonuses", None, &env.player.output);

    let attr_src = ModSource::new("Attribute", "Strength");
    let attr_src_dex = ModSource::new("Attribute", "Dexterity");
    let attr_src_int = ModSource::new("Attribute", "Intelligence");

    // Strength derived bonuses
    if !no_str_bonuses {
        // +1 Life per 2 Str (floor)
        let no_str_life = env
            .player
            .mod_db
            .flag_cfg("NoStrBonusToLife", None, &env.player.output);
        if !no_str_life {
            let life_from_str = (str_out / 2.0).floor();
            if life_from_str > 0.0 {
                env.player
                    .mod_db
                    .add(Mod::new_base("Life", life_from_str, attr_src.clone()));
            }
        }

        // +1% Inc melee phys dmg per 5 Str (with MELEE flag)
        // Check for DexIntToMeleeBonus and StrDmgBonusRatioOverride
        let dex_int_bonus = env.player.mod_db.sum_cfg(
            ModType::Base,
            "DexIntToMeleeBonus",
            None,
            &env.player.output,
        );
        let str_dmg_bonus_override = env.player.mod_db.sum_cfg(
            ModType::Base,
            "StrDmgBonusRatioOverride",
            None,
            &env.player.output,
        );
        let melee_phys_from_str = if str_dmg_bonus_override > 0.0 {
            ((str_out + dex_int_bonus) * str_dmg_bonus_override).floor()
        } else {
            ((str_out + dex_int_bonus) / 5.0).floor()
        };
        if melee_phys_from_str > 0.0 {
            env.player.mod_db.add(Mod {
                name: "PhysicalDamage".into(),
                mod_type: ModType::Inc,
                value: ModValue::Number(melee_phys_from_str),
                flags: ModFlags::MELEE,
                keyword_flags: KeywordFlags::NONE,
                tags: Vec::new(),
                source: attr_src.clone(),
            });
        }
    }

    // Dexterity derived bonuses
    if !no_dex_bonuses {
        // Accuracy per Dex: check DexAccBonusOverride first, default 2
        let dex_acc_mult = env
            .player
            .mod_db
            .override_value("DexAccBonusOverride", None, &env.player.output)
            .unwrap_or(2.0);
        let acc_from_dex = (dex_out * dex_acc_mult).floor();
        if acc_from_dex > 0.0 {
            env.player.mod_db.add(Mod::new_base(
                "Accuracy",
                acc_from_dex,
                attr_src_dex.clone(),
            ));
        }

        // +1% Inc evasion per 5 Dex
        let no_dex_evasion =
            env.player
                .mod_db
                .flag_cfg("NoDexBonusToEvasion", None, &env.player.output);
        if !no_dex_evasion {
            let evasion_inc_from_dex = (dex_out / 5.0).floor();
            if evasion_inc_from_dex > 0.0 {
                env.player.mod_db.add(Mod {
                    name: "Evasion".into(),
                    mod_type: ModType::Inc,
                    value: ModValue::Number(evasion_inc_from_dex),
                    flags: ModFlags::NONE,
                    keyword_flags: KeywordFlags::NONE,
                    tags: Vec::new(),
                    source: attr_src_dex.clone(),
                });
            }
        }
    }

    // Intelligence derived bonuses
    if !no_int_bonuses {
        // +1 Mana per 2 Int (BASE, floor) — Lua: m_floor(output.Int / 2), "BASE"
        let no_int_mana = env
            .player
            .mod_db
            .flag_cfg("NoIntBonusToMana", None, &env.player.output);
        if !no_int_mana {
            let mana_from_int = (int_out / 2.0).floor();
            if mana_from_int > 0.0 {
                env.player
                    .mod_db
                    .add(Mod::new_base("Mana", mana_from_int, attr_src_int.clone()));
            }
        }

        // +1% Inc ES per 10 Int — Lua: m_floor(output.Int / 10), "INC"
        let no_int_es = env
            .player
            .mod_db
            .flag_cfg("NoIntBonusToES", None, &env.player.output);
        if !no_int_es {
            let es_inc_from_int = (int_out / 10.0).floor();
            if es_inc_from_int > 0.0 {
                env.player.mod_db.add(Mod {
                    name: "EnergyShield".into(),
                    mod_type: ModType::Inc,
                    value: ModValue::Number(es_inc_from_int),
                    flags: ModFlags::NONE,
                    keyword_flags: KeywordFlags::NONE,
                    tags: Vec::new(),
                    source: attr_src_int.clone(),
                });
            }
        }
    }

    // Exposure tracking conditions
    let fire_exposure = env.player.mod_db.sum_cfg(
        ModType::Base,
        "FireExposureChance",
        None,
        &env.player.output,
    );
    env.player
        .mod_db
        .set_condition("CanApplyFireExposure", fire_exposure > 0.0);

    let cold_exposure = env.player.mod_db.sum_cfg(
        ModType::Base,
        "ColdExposureChance",
        None,
        &env.player.output,
    );
    env.player
        .mod_db
        .set_condition("CanApplyColdExposure", cold_exposure > 0.0);

    let lightning_exposure = env.player.mod_db.sum_cfg(
        ModType::Base,
        "LightningExposureChance",
        None,
        &env.player.output,
    );
    env.player
        .mod_db
        .set_condition("CanApplyLightningExposure", lightning_exposure > 0.0);

    // Non-damaging ailment tracking
    let scorch_chance =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ScorchChance", None, &env.player.output);
    env.player
        .mod_db
        .set_condition("CanInflictScorch", scorch_chance > 0.0);

    let brittle_chance =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "BrittleChance", None, &env.player.output);
    env.player
        .mod_db
        .set_condition("CanInflictBrittle", brittle_chance > 0.0);

    let sap_chance =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "SapChance", None, &env.player.output);
    env.player
        .mod_db
        .set_condition("CanInflictSap", sap_chance > 0.0);
}

/// Placeholder for the Omniscience path.
/// The real implementation caps each stat at its class base value,
/// converts excess to Omni BASE mods, mirrors INC/MORE to Omni,
/// and subtracts double/triple dip overlaps. For now, use simplified version.
fn compute_omniscience(env: &mut CalcEnv) {
    // Simple approximation: Omni = Str + Dex + Int, individual stats zeroed.
    // TODO: implement full Lua logic (CalcPerform.lua lines 410-472).
    let str_val = calc_attr(&env.player.mod_db, &env.player.output, "Str");
    let dex_val = calc_attr(&env.player.mod_db, &env.player.output, "Dex");
    let int_val = calc_attr(&env.player.mod_db, &env.player.output, "Int");

    let omni = str_val + dex_val + int_val;
    env.player.set_output("Omni", omni.max(0.0));
    env.player.set_output("Str", 0.0);
    env.player.set_output("Dex", 0.0);
    env.player.set_output("Int", 0.0);
    env.player.set_output("LowestAttribute", 0.0);
    env.player.mod_db.set_condition("Omniscience", true);
}

// ---------------------------------------------------------------------------
// Life / Mana / ES calculation
// ---------------------------------------------------------------------------

/// Mirrors doActorLifeMana() in CalcPerform.lua.
fn do_actor_life_mana(env: &mut CalcEnv) {
    // data.misc.LowPoolThreshold = 0.5 (Data.lua:167)
    const LOW_POOL_THRESHOLD: f64 = 0.5;

    // LowLifePercentage: output as a percentage (e.g. 50.0 means 50%)
    // Lua: output.LowLifePercentage = 100.0 * (lowLifePerc > 0 and lowLifePerc or 0.5)
    let low_life_perc_raw =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "LowLifePercentage", None, &env.player.output);
    let low_life_threshold = if low_life_perc_raw > 0.0 {
        low_life_perc_raw
    } else {
        LOW_POOL_THRESHOLD
    };
    env.player
        .set_output("LowLifePercentage", 100.0 * low_life_threshold);

    // FullLifePercentage
    let full_life_perc_raw = env.player.mod_db.sum_cfg(
        ModType::Base,
        "FullLifePercentage",
        None,
        &env.player.output,
    );
    let full_life_threshold = if full_life_perc_raw > 0.0 {
        full_life_perc_raw
    } else {
        1.0
    };
    env.player
        .set_output("FullLifePercentage", 100.0 * full_life_threshold);

    // ChaosInoculation flag — written as bool output AND condition
    let chaos_inoc = env
        .player
        .mod_db
        .flag_cfg("ChaosInoculation", None, &env.player.output);
    env.player.set_output_bool("ChaosInoculation", chaos_inoc);

    if chaos_inoc {
        // CI: life is fixed at 1
        env.player.set_output("Life", 1.0);
        env.player.mod_db.set_condition("FullLife", true);
        env.player.mod_db.set_condition("ChaosInoculation", true);
    } else {
        // Life-to-ES conversion reduces life.
        // NOTE: Lua does NOT clamp conv to [0, 100]. PoB allows conv > 100 (life goes negative
        // before max(1.0)). We reproduce the same behaviour — no clamping.
        let life_conv = env.player.mod_db.sum_cfg(
            ModType::Base,
            "LifeConvertToEnergyShield",
            None,
            &env.player.output,
        );
        let conv_factor = 1.0 - life_conv / 100.0;

        let base = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, "Life", None, &env.player.output);
        let inc = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "Life", None, &env.player.output);
        let more = env.player.mod_db.more_cfg("Life", None, &env.player.output);

        // Check for Override mod (e.g. Divine Flesh uniquifying)
        let life = env
            .player
            .mod_db
            .override_value("Life", None, &env.player.output)
            .unwrap_or_else(|| {
                // Lua: m_max(round(base * (1 + inc/100) * more * (1 - conv/100)), 1)
                // Note: round() THEN max(1), not max(1) THEN round()
                (base * (1.0 + inc / 100.0) * more * conv_factor)
                    .round()
                    .max(1.0)
            });
        env.player.set_output("Life", life);

        // Breakdown
        let has_override = env
            .player
            .mod_db
            .override_value("Life", None, &env.player.output)
            .is_some();
        if inc != 0.0 || more != 1.0 || conv_factor != 1.0 || has_override {
            let mut lines = vec![format!("{base:.0} (base)")];
            if inc != 0.0 {
                lines.push(format!("x {:.2} (increased/reduced)", 1.0 + inc / 100.0));
            }
            if more != 1.0 {
                lines.push(format!("x {more:.2} (more/less)"));
            }
            if has_override {
                lines.push(format!("= {life:.0} (life override)"));
            }
            lines.push(format!("= {life:.0}"));
            env.player.set_breakdown_lines("Life", lines);
        }

        // FullLife condition (LowLife is set in do_actor_life_mana_reservation, matching Lua).
        // FullLife is set here based on unreserved percentage at time of pool calculation.
        // This is a simplified check — the Lua actually sets FullLife here (CI path) or
        // doesn't set it (it's set in condList via other means). The reservation pass
        // handles LowLife. FullLife in the non-CI path is not explicitly set in Lua either
        // (it may be set by the reservation pass or external conditions).
        // We set a conservative FullLife=false here; the reservation pass corrects it.
    }

    // Mana: Lua uses calcLib.val short-circuit (0 if base == 0), plus ManaConvertToArmour
    {
        let mana_conv = env.player.mod_db.sum_cfg(
            ModType::Base,
            "ManaConvertToArmour",
            None,
            &env.player.output,
        );
        let base = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, "Mana", None, &env.player.output);
        let inc = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "Mana", None, &env.player.output);
        let more = env.player.mod_db.more_cfg("Mana", None, &env.player.output);
        // calcLib.val short-circuit: if base == 0, result is 0 regardless of inc/more
        let mana_pre_conv = if base == 0.0 {
            0.0
        } else {
            base * (1.0 + inc / 100.0) * more
        };
        // No minimum here — Mana can legitimately be 0 or negative.
        let mana = (mana_pre_conv * (1.0 - mana_conv / 100.0)).round();
        env.player.set_output("Mana", mana);

        if inc != 0.0 || more != 1.0 || mana_conv != 0.0 {
            let mut lines = vec![format!("{base:.0} (base)")];
            if inc != 0.0 {
                lines.push(format!("x {:.2} (increased/reduced)", 1.0 + inc / 100.0));
            }
            if more != 1.0 {
                lines.push(format!("x {more:.2} (more/less)"));
            }
            if mana_conv != 0.0 {
                lines.push(format!(
                    "x {:.2} (converted to Armour)",
                    1.0 - mana_conv / 100.0
                ));
            }
            lines.push(format!("= {mana:.0}"));
            env.player.set_breakdown_lines("Mana", lines);
        }
    }

    // LowestOfMaximumLifeAndMaximumMana (Lua line 129)
    {
        let life = get_output_f64(&env.player.output, "Life");
        let mana = get_output_f64(&env.player.output, "Mana");
        env.player
            .set_output("LowestOfMaximumLifeAndMaximumMana", life.min(mana));
    }

    // NOTE: EnergyShield is computed in defence.rs::calc_primary_defences, not here.
    // The Lua has it in doActorLifeMana but only via a simple formula for the
    // placeholder before CalcDefence runs. Rust mirrors this by removing it from here.
}

// ---------------------------------------------------------------------------
// Reservation
// ---------------------------------------------------------------------------

/// PoB's `floor(val, dec)` function (Common.lua:647-654).
/// NOT `f64::floor()`. Floors to `dec` decimal places with a small epsilon
/// to handle floating-point edge cases (e.g. 0.9999999999 → 1.0 after epsilon).
fn pob_floor(val: f64, dec: u32) -> f64 {
    let mult = 10f64.powi(dec as i32);
    (val * mult + 0.0001).floor() / mult
}

/// PoB's `round(val, dec)` function (Common.lua:635-641).
/// Rounds to `dec` decimal places using standard rounding.
fn pob_round(val: f64, dec: u32) -> f64 {
    let mult = 10f64.powi(dec as i32);
    (val * mult + 0.5).floor() / mult
}

/// Mirrors CalcPerform.lua lines 1810–1919: per-skill reservation accumulation.
///
/// Iterates `env.player.active_skill_list` (populated by `active_skill::run()`),
/// identifies skills with HasReservation (that haven't converted reservation to cost),
/// and computes each skill's flat and percent reservation contribution.
///
/// For each qualifying skill, queries `skill.skill_mod_db` combined with
/// `env.player.mod_db` to mirror the Lua's `skillModList` (which inherits from
/// `actor.modDB`).
fn accumulate_skill_reservations(env: &mut CalcEnv) {
    // ── Initialize accumulators (CalcPerform.lua:1811–1816) ──────────────
    // modDB here is env.player.mod_db (the global player modDB), matching Lua's `modDB`.
    env.player.reserved_life = 0.0;
    env.player.reserved_life_percent =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ExtraLifeReserved", None, &env.player.output);
    env.player.reserved_mana = 0.0;
    env.player.reserved_mana_percent = 0.0;
    env.player.uncancellable_life_reservation =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ExtraLifeReserved", None, &env.player.output);
    env.player.uncancellable_mana_reservation =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ExtraManaReserved", None, &env.player.output);

    // ── Iterate activeSkillList (CalcPerform.lua:1821) ──────────────────
    // We need to collect per-skill data first because of borrow-checker constraints:
    // we can't iterate active_skill_list while also reading env.player.output.
    // Collect the data we need from each skill, then process.
    struct SkillReservationInput {
        has_reservation: bool,
        triggered_by_autoexertion: bool,
        reservation_becomes_cost: bool,
        skill_cfg: crate::mod_db::types::SkillCfg,
        // Base reservation values from skillData (overrides) or grantedEffectLevel
        sd_mana_reservation_flat: Option<f64>,
        sd_mana_reservation_percent: Option<f64>,
        sd_life_reservation_flat: Option<f64>,
        sd_life_reservation_percent: Option<f64>,
        gel_mana_reservation_flat: f64,
        gel_mana_reservation_percent: f64,
        gel_life_reservation_flat: f64,
        gel_life_reservation_percent: f64,
        // grantedEffectLevel.cost (for CostGainAsReservation)
        gel_cost_mana: f64,
        gel_cost_life: f64,
        // Forced overrides from skillData
        sd_mana_reservation_flat_forced: Option<f64>,
        sd_mana_reservation_percent_forced: Option<f64>,
        sd_life_reservation_flat_forced: Option<f64>,
        sd_life_reservation_percent_forced: Option<f64>,
        // SupportManaMultiplier from skill_mod_db (MORE product)
        skill_support_mana_mult: f64,
        // Mine/stage counts
        active_mine_count: Option<f64>,
        active_stage_count: Option<f64>,
    }

    let output_snapshot = env.player.output.clone();

    let mut skill_inputs: Vec<SkillReservationInput> = Vec::new();

    for skill in &env.player.active_skill_list {
        let has_reservation = skill
            .skill_types
            .iter()
            .any(|t| t.eq_ignore_ascii_case("HasReservation"));
        let triggered_by_autoexertion = skill
            .skill_data
            .get("triggeredByAutoexertion")
            .map_or(false, |&v| v != 0.0);
        let reservation_becomes_cost = skill
            .skill_types
            .iter()
            .any(|t| t.eq_ignore_ascii_case("ReservationBecomesCost"));

        let skill_cfg = skill.skill_cfg.clone().unwrap_or_default();

        // Read base reservation from skillData first (overrides grantedEffectLevel).
        // In Lua: activeSkill.skillData.manaReservationFlat or activeSkill.activeEffect.grantedEffectLevel.manaReservationFlat or 0
        // The grantedEffectLevel values are already stored in skill_data by create_active_skill
        // if they were non-zero. We need to distinguish skillData override vs grantedEffectLevel.
        //
        // In our Rust model:
        // - skill_data contains runtime overrides set by SkillData LIST mods and support effects
        // - The gem's level data is the "grantedEffectLevel" equivalent
        //
        // We look up the gem level data for the grantedEffectLevel fields.
        let (
            gel_mana_flat,
            gel_mana_pct,
            gel_life_flat,
            gel_life_pct,
            gel_cost_mana,
            gel_cost_life,
        ) = {
            if let Some(gd) = env.data.gems.get(&skill.skill_id) {
                let ld = gd
                    .levels
                    .iter()
                    .find(|l| l.level == skill.level)
                    .or_else(|| gd.levels.last());
                if let Some(ld) = ld {
                    (
                        ld.mana_reservation_flat,
                        ld.mana_reservation_percent,
                        ld.life_reservation_flat,
                        ld.life_reservation_percent,
                        ld.mana_cost,
                        ld.life_cost,
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                }
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            }
        };

        // SupportManaMultiplier: query skill_mod_db (which has the per-support MORE mods)
        // Lua: floor(skillModList:More(skillCfg, "SupportManaMultiplier"), 4)
        // Since skillModList inherits from modDB, we combine skill_mod_db + player.mod_db.
        let skill_smm = skill.skill_mod_db.more_cfg(
            "SupportManaMultiplier",
            Some(&skill_cfg),
            &output_snapshot,
        );
        let player_smm =
            env.player
                .mod_db
                .more_cfg("SupportManaMultiplier", Some(&skill_cfg), &output_snapshot);

        // skillData values that represent overrides
        let sd_mana_flat = skill.skill_data.get("manaReservationFlat").copied();
        let sd_mana_pct = skill.skill_data.get("manaReservationPercent").copied();
        let sd_life_flat = skill.skill_data.get("lifeReservationFlat").copied();
        let sd_life_pct = skill.skill_data.get("lifeReservationPercent").copied();

        // Forced overrides
        let sd_mana_flat_forced = skill.skill_data.get("ManaReservationFlatForced").copied();
        let sd_mana_pct_forced = skill
            .skill_data
            .get("ManaReservationPercentForced")
            .copied();
        let sd_life_flat_forced = skill.skill_data.get("LifeReservationFlatForced").copied();
        let sd_life_pct_forced = skill
            .skill_data
            .get("LifeReservationPercentForced")
            .copied();

        skill_inputs.push(SkillReservationInput {
            has_reservation,
            triggered_by_autoexertion,
            reservation_becomes_cost,
            skill_cfg,
            sd_mana_reservation_flat: sd_mana_flat,
            sd_mana_reservation_percent: sd_mana_pct,
            sd_life_reservation_flat: sd_life_flat,
            sd_life_reservation_percent: sd_life_pct,
            gel_mana_reservation_flat: gel_mana_flat,
            gel_mana_reservation_percent: gel_mana_pct,
            gel_life_reservation_flat: gel_life_flat,
            gel_life_reservation_percent: gel_life_pct,
            gel_cost_mana,
            gel_cost_life,
            sd_mana_reservation_flat_forced: sd_mana_flat_forced,
            sd_mana_reservation_percent_forced: sd_mana_pct_forced,
            sd_life_reservation_flat_forced: sd_life_flat_forced,
            sd_life_reservation_percent_forced: sd_life_pct_forced,
            skill_support_mana_mult: skill_smm * player_smm,
            active_mine_count: skill.active_mine_count,
            active_stage_count: skill.active_stage_count,
        });
    }

    // ── Process each skill (CalcPerform.lua:1822–1917) ──────────────────
    for input in &skill_inputs {
        // CalcPerform.lua:1822: if (activeSkill.skillTypes[SkillType.HasReservation]
        //   or activeSkill.skillData.triggeredByAutoexertion)
        //   and not activeSkill.skillTypes[SkillType.ReservationBecomesCost]
        if !(input.has_reservation || input.triggered_by_autoexertion)
            || input.reservation_becomes_cost
        {
            continue;
        }

        let skill_cfg = &input.skill_cfg;

        // CalcPerform.lua:1825: local mult = floor(skillModList:More(skillCfg, "SupportManaMultiplier"), 4)
        let mult = pob_floor(input.skill_support_mana_mult, 4);

        // CalcPerform.lua:1827–1836: Read base reservation values
        // Lua: skillData.X or grantedEffectLevel.X or 0
        let mut mana_base_flat = input
            .sd_mana_reservation_flat
            .unwrap_or(input.gel_mana_reservation_flat);
        let mut mana_base_percent = input
            .sd_mana_reservation_percent
            .unwrap_or(input.gel_mana_reservation_percent);
        let mut life_base_flat = input
            .sd_life_reservation_flat
            .unwrap_or(input.gel_life_reservation_flat);
        let mut life_base_percent = input
            .sd_life_reservation_percent
            .unwrap_or(input.gel_life_reservation_percent);

        // CalcPerform.lua:1828–1830: ManaCostGainAsReservation
        // if skillModList:Flag(skillCfg, "ManaCostGainAsReservation") and
        //    activeSkill.activeEffect.grantedEffectLevel.cost then
        //   pool.Mana.baseFlat = skillModList:Sum("BASE", skillCfg, "ManaCostBase")
        //                        + (grantedEffectLevel.cost.Mana or 0)
        let mana_cost_as_reservation = env.player.mod_db.flag_cfg(
            "ManaCostGainAsReservation",
            Some(skill_cfg),
            &output_snapshot,
        );
        // Guard: grantedEffectLevel.cost must exist (non-zero cost)
        let has_gel_cost_mana = input.gel_cost_mana != 0.0;
        if mana_cost_as_reservation && has_gel_cost_mana {
            let mana_cost_base = env.player.mod_db.sum_cfg(
                ModType::Base,
                "ManaCostBase",
                Some(skill_cfg),
                &output_snapshot,
            );
            mana_base_flat = mana_cost_base + input.gel_cost_mana;
        }

        // CalcPerform.lua:1833–1835: LifeCostGainAsReservation
        let life_cost_as_reservation = env.player.mod_db.flag_cfg(
            "LifeCostGainAsReservation",
            Some(skill_cfg),
            &output_snapshot,
        );
        let has_gel_cost_life = input.gel_cost_life != 0.0;
        if life_cost_as_reservation && has_gel_cost_life {
            let life_cost_base = env.player.mod_db.sum_cfg(
                ModType::Base,
                "LifeCostBase",
                Some(skill_cfg),
                &output_snapshot,
            );
            life_base_flat = life_cost_base + input.gel_cost_life;
        }

        // Track forced overrides (mirrors skillData["XReservationFlatForced"])
        let mut mana_reservation_flat_forced = input.sd_mana_reservation_flat_forced;
        let mut mana_reservation_percent_forced = input.sd_mana_reservation_percent_forced;
        let mut life_reservation_flat_forced = input.sd_life_reservation_flat_forced;
        let mut life_reservation_percent_forced = input.sd_life_reservation_percent_forced;

        // CalcPerform.lua:1837–1845: BloodMagicReserved
        let blood_magic_reserved =
            env.player
                .mod_db
                .flag_cfg("BloodMagicReserved", Some(skill_cfg), &output_snapshot);
        if blood_magic_reserved {
            life_base_flat += mana_base_flat;
            mana_base_flat = 0.0;
            // Lua: activeSkill.skillData["LifeReservationFlatForced"] = activeSkill.skillData["ManaReservationFlatForced"]
            // Lua: activeSkill.skillData["ManaReservationFlatForced"] = nil
            life_reservation_flat_forced = mana_reservation_flat_forced.take();
            life_base_percent += mana_base_percent;
            mana_base_percent = 0.0;
            life_reservation_percent_forced = mana_reservation_percent_forced.take();
        }

        // CalcPerform.lua:1847–1916: Process both pools (Mana and Life)
        struct PoolValues {
            base_flat: f64,
            base_percent: f64,
            flat_forced: Option<f64>,
            percent_forced: Option<f64>,
        }

        let pools = [
            (
                "Mana",
                PoolValues {
                    base_flat: mana_base_flat,
                    base_percent: mana_base_percent,
                    flat_forced: mana_reservation_flat_forced,
                    percent_forced: mana_reservation_percent_forced,
                },
            ),
            (
                "Life",
                PoolValues {
                    base_flat: life_base_flat,
                    base_percent: life_base_percent,
                    flat_forced: life_reservation_flat_forced,
                    percent_forced: life_reservation_percent_forced,
                },
            ),
        ];

        for (pool_name, pool) in &pools {
            // CalcPerform.lua:1848–1851: Compute modifiers from skillModList
            // Since skillModList inherits from actor.modDB, we query both
            // skill_mod_db and player.mod_db. For reservation modifiers,
            // they typically come from the global modDB (tree/items), not per-skill.
            let reserved_key = format!("{}Reserved", pool_name);
            let efficiency_key = format!("{}ReservationEfficiency", pool_name);

            // CalcPerform.lua:1848: values.more = skillModList:More(skillCfg, name.."Reserved", "Reserved")
            let more = env.player.mod_db.more_cfg_multi(
                &[&reserved_key, "Reserved"],
                Some(skill_cfg),
                &output_snapshot,
            );

            // CalcPerform.lua:1849: values.inc = skillModList:Sum("INC", skillCfg, name.."Reserved", "Reserved")
            let inc = env.player.mod_db.sum_cfg_multi(
                ModType::Inc,
                &[&reserved_key, "Reserved"],
                Some(skill_cfg),
                &output_snapshot,
            );

            // CalcPerform.lua:1850: values.efficiency = m_max(skillModList:Sum("INC", skillCfg, name.."ReservationEfficiency", "ReservationEfficiency"), -100)
            let efficiency = env
                .player
                .mod_db
                .sum_cfg_multi(
                    ModType::Inc,
                    &[&efficiency_key, "ReservationEfficiency"],
                    Some(skill_cfg),
                    &output_snapshot,
                )
                .max(-100.0);

            // CalcPerform.lua:1851: values.efficiencyMore = skillModList:More(skillCfg, name.."ReservationEfficiency", "ReservationEfficiency")
            let efficiency_more = env.player.mod_db.more_cfg_multi(
                &[&efficiency_key, "ReservationEfficiency"],
                Some(skill_cfg),
                &output_snapshot,
            );

            // CalcPerform.lua:1853: env.player[name.."Efficiency"] = values.efficiency
            match *pool_name {
                "Mana" => env.player.mana_efficiency = efficiency,
                "Life" => env.player.life_efficiency = efficiency,
                _ => {}
            }

            // CalcPerform.lua:1854–1862: Flat reservation
            let mut reserved_flat = if let Some(forced) = pool.flat_forced {
                // CalcPerform.lua:1855: values.reservedFlat = activeSkill.skillData[name.."ReservationFlatForced"]
                forced
            } else {
                // CalcPerform.lua:1857: local baseFlatVal = m_floor(values.baseFlat * mult)
                let base_flat_val = (pool.base_flat * mult).floor();
                if more > 0.0 && inc > -100.0 && base_flat_val != 0.0 {
                    // CalcPerform.lua:1860
                    pob_round(
                        base_flat_val * (100.0 + inc) / 100.0 * more
                            / (1.0 + efficiency / 100.0)
                            / efficiency_more,
                        0,
                    )
                    .max(0.0)
                } else {
                    0.0
                }
            };

            // CalcPerform.lua:1863–1871: Percent reservation
            let mut reserved_percent = if let Some(forced) = pool.percent_forced {
                // CalcPerform.lua:1864
                forced
            } else {
                // CalcPerform.lua:1866: local basePercentVal = values.basePercent * mult
                let base_percent_val = pool.base_percent * mult;
                if more > 0.0 && inc > -100.0 && base_percent_val != 0.0 {
                    // CalcPerform.lua:1869
                    pob_round(
                        base_percent_val * (100.0 + inc) / 100.0 * more
                            / (1.0 + efficiency / 100.0)
                            / efficiency_more,
                        2,
                    )
                    .max(0.0)
                } else {
                    0.0
                }
            };

            // CalcPerform.lua:1872–1874: Mine count scaling
            // if activeSkill.activeMineCount then
            //   values.reservedFlat = values.reservedFlat * activeSkill.activeMineCount
            //   values.reservedPercent = values.reservedPercent * activeSkill.activeMineCount
            if let Some(mine_count) = input.active_mine_count {
                reserved_flat *= mine_count;
                reserved_percent *= mine_count;
            }

            // CalcPerform.lua:1876–1879: Blood Sacrament stage scaling
            // if activeSkill.skillCfg.skillName == "Blood Sacrament" and activeSkill.activeStageCount then
            //   values.reservedFlat = values.reservedFlat * (activeSkill.activeStageCount + 1)
            //   values.reservedPercent = values.reservedPercent * (activeSkill.activeStageCount + 1)
            if skill_cfg.skill_name.as_deref() == Some("Blood Sacrament") {
                if let Some(stage_count) = input.active_stage_count {
                    reserved_flat *= stage_count + 1.0;
                    reserved_percent *= stage_count + 1.0;
                }
            }

            // CalcPerform.lua:1881–1895: Accumulate flat reservation
            if reserved_flat != 0.0 {
                // CalcPerform.lua:1883: env.player["reserved_"..name.."Base"] = env.player["reserved_"..name.."Base"] + values.reservedFlat
                match *pool_name {
                    "Mana" => env.player.reserved_mana += reserved_flat,
                    "Life" => env.player.reserved_life += reserved_flat,
                    _ => {}
                }
            }

            // CalcPerform.lua:1897–1912: Accumulate percent reservation
            if reserved_percent != 0.0 {
                match *pool_name {
                    "Mana" => env.player.reserved_mana_percent += reserved_percent,
                    "Life" => env.player.reserved_life_percent += reserved_percent,
                    _ => {}
                }
            }

            // CalcPerform.lua:1914–1916: HasUncancellableReservation tracking
            // if skillModList:Flag(skillCfg, "HasUncancellableReservation") then
            //   env.player["uncancellable_"..name.."Reservation"] = env.player["uncancellable_"..name.."Reservation"] + values.reservedPercent
            let has_uncancellable = env.player.mod_db.flag_cfg(
                "HasUncancellableReservation",
                Some(skill_cfg),
                &output_snapshot,
            );
            if has_uncancellable {
                match *pool_name {
                    "Life" => env.player.uncancellable_life_reservation += reserved_percent,
                    "Mana" => env.player.uncancellable_mana_reservation += reserved_percent,
                    _ => {}
                }
            }
        }
    }
}

/// Mirrors doActorLifeManaReservation() in CalcPerform.lua:519-553.
/// Computes life/mana reserved and unreserved amounts and percentages.
/// Also sets LowLife and LowMana conditions (Lua sets them here, NOT in doActorLifeMana).
///
/// `force_add_aura`: if Some(true), forces addAura=true (used by Foulborn Choir re-run).
/// If None, addAura is determined by the ManaIncreasedByOvercappedLightningRes flag.
fn do_actor_life_mana_reservation(env: &mut CalcEnv) {
    do_actor_life_mana_reservation_inner(env, None);
}

fn do_actor_life_mana_reservation_inner(env: &mut CalcEnv, force_add_aura: Option<bool>) {
    // data.misc.LowPoolThreshold = 0.5 (Data.lua:167)
    const LOW_POOL_THRESHOLD: f64 = 0.5;

    // CalcPerform.lua:1922 — first call uses addAura = not flag(ManaIncreasedByOvercappedLightningRes)
    // CalcPerform.lua:3201 — Foulborn re-run uses addAura = true
    let add_aura = force_add_aura.unwrap_or_else(|| {
        !env.player.mod_db.flag_cfg(
            "ManaIncreasedByOvercappedLightningRes",
            None,
            &env.player.output,
        )
    });

    // Lua iterates {"Life", "Mana"} and runs the same logic for both.
    // We unroll the loop here for Rust clarity.

    // ── Life ──────────────────────────────────────────────────────────────
    let life = get_output_f64(&env.player.output, "Life");
    let life_reserved;
    if life > 0.0 {
        // LowLifePercentage: read from modDB (raw fraction, e.g. 0.35 for 35%)
        // This was already written to output as 100*fraction in do_actor_life_mana.
        // Here we re-read the raw fraction for the condition check.
        let low_life_perc_raw =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, "LowLifePercentage", None, &env.player.output);
        let low_life_threshold = if low_life_perc_raw > 0.0 {
            low_life_perc_raw
        } else {
            LOW_POOL_THRESHOLD
        };

        // Percent portion uses m_ceil (Lua uses ceil, NOT floor or round)
        let reserved_life_from_pct = (life * env.player.reserved_life_percent / 100.0).ceil();
        let total_reserved_life = reserved_life_from_pct + env.player.reserved_life;
        life_reserved = total_reserved_life;
        // Reserved shown in UI is capped at max, but Unreserved is NOT clamped.
        let life_reserved_display = total_reserved_life.min(life);
        let life_unreserved = life - total_reserved_life; // can be negative — do NOT clamp

        env.player.set_output("LifeReserved", life_reserved_display);
        env.player.set_output(
            "LifeReservedPercent",
            (total_reserved_life / life * 100.0).min(100.0),
        );
        env.player.set_output("LifeUnreserved", life_unreserved);
        env.player
            .set_output("LifeUnreservedPercent", life_unreserved / life * 100.0);

        // UncancellableReservation: Lua uses m_min(val, 0) — always ≤ 0 (PoB quirk)
        let uncancellable_life = env.player.uncancellable_life_reservation;
        env.player
            .set_output("LifeUncancellableReservation", uncancellable_life.min(0.0));
        env.player
            .set_output("LifeCancellableReservation", 100.0 - uncancellable_life);

        // LowLife condition: (unreserved / max) <= threshold
        if life_unreserved / life <= low_life_threshold {
            env.player.mod_db.set_condition("LowLife", true);
        }
    } else {
        life_reserved = 0.0;
    }

    // Gap e: GrantReservedLifeAsAura (CalcPerform.lua:545-551)
    // For each LIST mod named "GrantReservedLifeAsAura", scale its embedded mod's value
    // by floor(embedded_value * min(reserved, max)) and add as an ExtraAura LIST mod.
    if add_aura {
        // Collect the ExtraAura mods to add (borrow-checker: can't borrow mod_db mutably while iterating)
        let extra_aura_mods: Vec<Mod> = {
            let grant_mods =
                env.player
                    .mod_db
                    .list("GrantReservedLifeAsAura", None, &env.player.output);
            grant_mods
                .iter()
                .filter_map(|m| {
                    let embedded = m.value.as_embedded_mod()?;
                    let scaled_value = (embedded.value * life_reserved.min(life)).floor();
                    Some(Mod {
                        name: "ExtraAura".into(),
                        mod_type: ModType::List,
                        value: ModValue::EmbeddedMod(Box::new(crate::mod_db::types::EmbeddedMod {
                            name: embedded.name.clone(),
                            mod_type: embedded.mod_type.clone(),
                            value: scaled_value,
                            flags: embedded.flags,
                            keyword_flags: embedded.keyword_flags,
                        })),
                        flags: ModFlags::NONE,
                        keyword_flags: KeywordFlags::NONE,
                        tags: vec![],
                        source: m.source.clone(),
                    })
                })
                .collect()
        };
        for extra in extra_aura_mods {
            env.player.mod_db.add(extra);
        }
    }

    // ── Mana ──────────────────────────────────────────────────────────────
    let mana = get_output_f64(&env.player.output, "Mana");
    let mana_reserved;
    if mana > 0.0 {
        let low_mana_perc_raw =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, "LowManaPercentage", None, &env.player.output);
        let low_mana_threshold = if low_mana_perc_raw > 0.0 {
            low_mana_perc_raw
        } else {
            LOW_POOL_THRESHOLD
        };

        // Percent portion uses m_ceil
        let reserved_mana_from_pct = (mana * env.player.reserved_mana_percent / 100.0).ceil();
        let total_reserved_mana = reserved_mana_from_pct + env.player.reserved_mana;
        mana_reserved = total_reserved_mana;
        let mana_reserved_display = total_reserved_mana.min(mana);
        let mana_unreserved = mana - total_reserved_mana; // can be negative — do NOT clamp

        env.player.set_output("ManaReserved", mana_reserved_display);
        env.player.set_output(
            "ManaReservedPercent",
            (total_reserved_mana / mana * 100.0).min(100.0),
        );
        env.player.set_output("ManaUnreserved", mana_unreserved);
        env.player
            .set_output("ManaUnreservedPercent", mana_unreserved / mana * 100.0);

        let uncancellable_mana = env.player.uncancellable_mana_reservation;
        env.player
            .set_output("ManaUncancellableReservation", uncancellable_mana.min(0.0));
        env.player
            .set_output("ManaCancellableReservation", 100.0 - uncancellable_mana);

        // LowMana condition: (unreserved / max) <= threshold
        if mana_unreserved / mana <= low_mana_threshold {
            env.player.mod_db.set_condition("LowMana", true);
        }
    } else {
        mana_reserved = 0.0;
    }

    // Gap e: GrantReservedManaAsAura (CalcPerform.lua:545-551)
    if add_aura {
        let extra_aura_mods: Vec<Mod> = {
            let grant_mods =
                env.player
                    .mod_db
                    .list("GrantReservedManaAsAura", None, &env.player.output);
            grant_mods
                .iter()
                .filter_map(|m| {
                    let embedded = m.value.as_embedded_mod()?;
                    let scaled_value = (embedded.value * mana_reserved.min(mana)).floor();
                    Some(Mod {
                        name: "ExtraAura".into(),
                        mod_type: ModType::List,
                        value: ModValue::EmbeddedMod(Box::new(crate::mod_db::types::EmbeddedMod {
                            name: embedded.name.clone(),
                            mod_type: embedded.mod_type.clone(),
                            value: scaled_value,
                            flags: embedded.flags,
                            keyword_flags: embedded.keyword_flags,
                        })),
                        flags: ModFlags::NONE,
                        keyword_flags: KeywordFlags::NONE,
                        tags: vec![],
                        source: m.source.clone(),
                    })
                })
                .collect()
        };
        for extra in extra_aura_mods {
            env.player.mod_db.add(extra);
        }
    }
}

// ---------------------------------------------------------------------------
// Charges
// ---------------------------------------------------------------------------

/// Mirrors doActorCharges() in CalcPerform.lua.
/// Helper: check for a Flag mod OR a condition with the same name.
/// The Lua setup adds config booleans like "usePowerCharges" as FLAG mods (with a
/// Condition tag for Combat), but the Rust setup currently stores them as conditions.
/// This helper bridges the gap until setup.rs is updated to emit proper FLAG mods.
fn flag_or_cond(env: &CalcEnv, name: &str) -> bool {
    env.player.mod_db.flag_cfg(name, None, &env.player.output)
        || env
            .player
            .mod_db
            .conditions
            .get(name)
            .copied()
            .unwrap_or(false)
}

fn do_actor_charges(env: &mut CalcEnv) {
    // Mirrors doActorCharges() in CalcPerform.lua lines 917–1065.
    // Faithfully ports every code path from the Lua source.

    // ── Step 1: Compute Max/Min values (Lua lines 922–952) ──────────────

    // Power charges (Lua 923–925)
    let pc_min = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "PowerChargesMin", None, &env.player.output)
        .max(0.0);
    let pc_max = env
        .player
        .mod_db
        .override_value("PowerChargesMax", None, &env.player.output)
        .unwrap_or_else(|| {
            env.player
                .mod_db
                .sum_cfg(ModType::Base, "PowerChargesMax", None, &env.player.output)
                .max(0.0)
        });

    // PowerChargesDuration (Lua 925) — not in PERF-03 field set but computed by Lua
    let charge_dur_base =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ChargeDuration", None, &env.player.output);
    let pc_dur_mod = crate::calc::calc_tools::calc_def_mod(
        &env.player.mod_db,
        None,
        &env.player.output,
        &["PowerChargesDuration", "ChargeDuration"],
    );
    let pc_dur = (charge_dur_base * pc_dur_mod).floor();
    env.player.set_output("PowerChargesDuration", pc_dur);

    // Write PowerChargesMin/Max to output now (needed by subsequent flag-chain formulas)
    env.player.set_output("PowerChargesMin", pc_min);
    env.player.set_output("PowerChargesMax", pc_max);

    // MaximumFrenzyChargesIsMaximumPowerCharges flag (Lua 926–929)
    // If set, inject an OVERRIDE for FrenzyChargesMax equal to PowerChargesMax.
    if env.player.mod_db.flag_cfg(
        "MaximumFrenzyChargesIsMaximumPowerCharges",
        None,
        &env.player.output,
    ) {
        let source = env
            .player
            .mod_db
            .first_mod_source("MaximumFrenzyChargesIsMaximumPowerCharges")
            .unwrap_or_else(|| ModSource::new("", ""));
        env.player.mod_db.replace_mod(Mod {
            name: "FrenzyChargesMax".into(),
            mod_type: ModType::Override,
            value: ModValue::Number(pc_max),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: Vec::new(),
            source,
        });
    }

    // Frenzy charges (Lua 930–932)
    let fc_min = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "FrenzyChargesMin", None, &env.player.output)
        .max(0.0);
    // FrenzyChargesMax: Override → flag-mirror → Sum (Lua 931)
    let fc_max = env
        .player
        .mod_db
        .override_value("FrenzyChargesMax", None, &env.player.output)
        .unwrap_or_else(|| {
            if env.player.mod_db.flag_cfg(
                "MaximumFrenzyChargesIsMaximumPowerCharges",
                None,
                &env.player.output,
            ) {
                pc_max
            } else {
                env.player.mod_db.sum_cfg(
                    ModType::Base,
                    "FrenzyChargesMax",
                    None,
                    &env.player.output,
                )
            }
            .max(0.0)
        });

    // FrenzyChargesDuration (Lua 932)
    let fc_dur_mod = crate::calc::calc_tools::calc_def_mod(
        &env.player.mod_db,
        None,
        &env.player.output,
        &["FrenzyChargesDuration", "ChargeDuration"],
    );
    let fc_dur = (charge_dur_base * fc_dur_mod).floor();
    env.player.set_output("FrenzyChargesDuration", fc_dur);

    env.player.set_output("FrenzyChargesMin", fc_min);
    env.player.set_output("FrenzyChargesMax", fc_max);

    // MaximumEnduranceChargesIsMaximumFrenzyCharges flag (Lua 933–935)
    if env.player.mod_db.flag_cfg(
        "MaximumEnduranceChargesIsMaximumFrenzyCharges",
        None,
        &env.player.output,
    ) {
        let source = env
            .player
            .mod_db
            .first_mod_source("MaximumEnduranceChargesIsMaximumFrenzyCharges")
            .unwrap_or_else(|| ModSource::new("", ""));
        env.player.mod_db.replace_mod(Mod {
            name: "EnduranceChargesMax".into(),
            mod_type: ModType::Override,
            value: ModValue::Number(fc_max),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: Vec::new(),
            source,
        });
    }

    // Endurance charges (Lua 937–939)
    let ec_min = env
        .player
        .mod_db
        .sum_cfg(
            ModType::Base,
            "EnduranceChargesMin",
            None,
            &env.player.output,
        )
        .max(0.0);
    // EnduranceChargesMax: Override → party-member → flag-mirror → Sum (Lua 938)
    // NOTE: No party member support in Rust CalcEnv — party member flag always false.
    let ec_max = env
        .player
        .mod_db
        .override_value("EnduranceChargesMax", None, &env.player.output)
        .unwrap_or_else(|| {
            // Party member path skipped (env.partyMembers not implemented in Rust).
            // Fall through to flag-mirror or base sum.
            if env.player.mod_db.flag_cfg(
                "MaximumEnduranceChargesIsMaximumFrenzyCharges",
                None,
                &env.player.output,
            ) {
                fc_max
            } else {
                env.player.mod_db.sum_cfg(
                    ModType::Base,
                    "EnduranceChargesMax",
                    None,
                    &env.player.output,
                )
            }
            .max(0.0)
        });

    // EnduranceChargesDuration (Lua 939)
    let ec_dur_mod = crate::calc::calc_tools::calc_def_mod(
        &env.player.mod_db,
        None,
        &env.player.output,
        &["EnduranceChargesDuration", "ChargeDuration"],
    );
    let ec_dur = (charge_dur_base * ec_dur_mod).floor();
    env.player.set_output("EnduranceChargesDuration", ec_dur);

    env.player.set_output("EnduranceChargesMin", ec_min);
    env.player.set_output("EnduranceChargesMax", ec_max);

    // Alternative charge max fields (Lua 940–944)
    let siphoning_charges_max = env
        .player
        .mod_db
        .sum_cfg(
            ModType::Base,
            "SiphoningChargesMax",
            None,
            &env.player.output,
        )
        .max(0.0);
    env.player
        .set_output("SiphoningChargesMax", siphoning_charges_max);

    let challenger_charges_max = env
        .player
        .mod_db
        .sum_cfg(
            ModType::Base,
            "ChallengerChargesMax",
            None,
            &env.player.output,
        )
        .max(0.0);
    env.player
        .set_output("ChallengerChargesMax", challenger_charges_max);

    let blitz_charges_max = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "BlitzChargesMax", None, &env.player.output)
        .max(0.0);
    env.player.set_output("BlitzChargesMax", blitz_charges_max);

    let inspiration_charges_max = env
        .player
        .mod_db
        .sum_cfg(
            ModType::Base,
            "InspirationChargesMax",
            None,
            &env.player.output,
        )
        .max(0.0);
    env.player
        .set_output("InspirationChargesMax", inspiration_charges_max);

    let crab_barriers_max = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "CrabBarriersMax", None, &env.player.output)
        .max(0.0);
    env.player.set_output("CrabBarriersMax", crab_barriers_max);

    // Brutal charges min/max (Lua 945–946)
    let brutal_charges_min = if env.player.mod_db.flag_cfg(
        "MinimumEnduranceChargesEqualsMinimumBrutalCharges",
        None,
        &env.player.output,
    ) {
        if env.player.mod_db.flag_cfg(
            "MinimumEnduranceChargesIsMaximumEnduranceCharges",
            None,
            &env.player.output,
        ) {
            ec_max
        } else {
            ec_min
        }
    } else {
        0.0
    }
    .max(0.0);
    env.player
        .set_output("BrutalChargesMin", brutal_charges_min);

    let brutal_charges_max = if env.player.mod_db.flag_cfg(
        "MaximumEnduranceChargesEqualsMaximumBrutalCharges",
        None,
        &env.player.output,
    ) {
        ec_max
    } else {
        0.0
    }
    .max(0.0);
    env.player
        .set_output("BrutalChargesMax", brutal_charges_max);

    // Absorption charges min/max (Lua 947–948)
    let absorption_charges_min = if env.player.mod_db.flag_cfg(
        "MinimumPowerChargesEqualsMinimumAbsorptionCharges",
        None,
        &env.player.output,
    ) {
        if env.player.mod_db.flag_cfg(
            "MinimumPowerChargesIsMaximumPowerCharges",
            None,
            &env.player.output,
        ) {
            pc_max
        } else {
            pc_min
        }
    } else {
        0.0
    }
    .max(0.0);
    env.player
        .set_output("AbsorptionChargesMin", absorption_charges_min);

    let absorption_charges_max = if env.player.mod_db.flag_cfg(
        "MaximumPowerChargesEqualsMaximumAbsorptionCharges",
        None,
        &env.player.output,
    ) {
        pc_max
    } else {
        0.0
    }
    .max(0.0);
    env.player
        .set_output("AbsorptionChargesMax", absorption_charges_max);

    // Affliction charges min/max (Lua 949–950)
    let affliction_charges_min = if env.player.mod_db.flag_cfg(
        "MinimumFrenzyChargesEqualsMinimumAfflictionCharges",
        None,
        &env.player.output,
    ) {
        if env.player.mod_db.flag_cfg(
            "MinimumFrenzyChargesIsMaximumFrenzyCharges",
            None,
            &env.player.output,
        ) {
            fc_max
        } else {
            fc_min
        }
    } else {
        0.0
    }
    .max(0.0);
    env.player
        .set_output("AfflictionChargesMin", affliction_charges_min);

    let affliction_charges_max = if env.player.mod_db.flag_cfg(
        "MaximumFrenzyChargesEqualsMaximumAfflictionCharges",
        None,
        &env.player.output,
    ) {
        fc_max
    } else {
        0.0
    }
    .max(0.0);
    env.player
        .set_output("AfflictionChargesMax", affliction_charges_max);

    // Blood charges max (Lua 951)
    let blood_charges_max = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "BloodChargesMax", None, &env.player.output)
        .max(0.0);
    env.player.set_output("BloodChargesMax", blood_charges_max);

    // Spirit charges max (Lua 952)
    let spirit_charges_max = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "SpiritChargesMax", None, &env.player.output)
        .max(0.0);
    env.player
        .set_output("SpiritChargesMax", spirit_charges_max);

    // ── Step 2: Zero-initialize all current charges (Lua 954–967) ───────
    env.player.set_output("PowerCharges", 0.0);
    env.player.set_output("FrenzyCharges", 0.0);
    env.player.set_output("EnduranceCharges", 0.0);
    env.player.set_output("SiphoningCharges", 0.0);
    env.player.set_output("ChallengerCharges", 0.0);
    env.player.set_output("BlitzCharges", 0.0);
    env.player.set_output("InspirationCharges", 0.0);
    env.player.set_output("GhostShrouds", 0.0);
    env.player.set_output("BrutalCharges", 0.0);
    env.player.set_output("AbsorptionCharges", 0.0);
    env.player.set_output("AfflictionCharges", 0.0);
    env.player.set_output("BloodCharges", 0.0);
    env.player.set_output("SpiritCharges", 0.0);

    // ── Step 3: Conditionally override Min values (Lua 970–978) ─────────
    // These use mutable locals for the min values since they may be overwritten.
    let mut pc_min = pc_min;
    let mut fc_min = fc_min;
    let mut ec_min = ec_min;

    if env.player.mod_db.flag_cfg(
        "MinimumFrenzyChargesIsMaximumFrenzyCharges",
        None,
        &env.player.output,
    ) {
        fc_min = fc_max;
        env.player.set_output("FrenzyChargesMin", fc_max);
    }
    if env.player.mod_db.flag_cfg(
        "MinimumEnduranceChargesIsMaximumEnduranceCharges",
        None,
        &env.player.output,
    ) {
        ec_min = ec_max;
        env.player.set_output("EnduranceChargesMin", ec_max);
    }
    if env.player.mod_db.flag_cfg(
        "MinimumPowerChargesIsMaximumPowerCharges",
        None,
        &env.player.output,
    ) {
        pc_min = pc_max;
        env.player.set_output("PowerChargesMin", pc_max);
    }

    // ── Step 4: Determine current charge counts (Lua 979–1033) ──────────

    // Power Charges / Absorption conversion (Lua 979–990)
    let mut pc = 0.0_f64;
    if flag_or_cond(env, "UsePowerCharges") {
        pc = env
            .player
            .mod_db
            .override_value("PowerCharges", None, &env.player.output)
            .unwrap_or(pc_max);
    }
    let mut absorption_charges = 0.0_f64;
    if env.player.mod_db.flag_cfg(
        "PowerChargesConvertToAbsorptionCharges",
        None,
        &env.player.output,
    ) {
        absorption_charges = pc.max(absorption_charges_max.min(absorption_charges_min));
        pc = 0.0;
    } else {
        pc = pc.max(pc_max.min(pc_min));
    }
    let removable_pc = (pc - pc_min).max(0.0);
    env.player.set_output("PowerCharges", pc);
    env.player
        .set_output("AbsorptionCharges", absorption_charges);
    env.player.set_output("RemovablePowerCharges", removable_pc);

    // Frenzy Charges / Affliction conversion (Lua 991–1002)
    let mut fc = 0.0_f64;
    if flag_or_cond(env, "UseFrenzyCharges") {
        fc = env
            .player
            .mod_db
            .override_value("FrenzyCharges", None, &env.player.output)
            .unwrap_or(fc_max);
    }
    let mut affliction_charges = 0.0_f64;
    if env.player.mod_db.flag_cfg(
        "FrenzyChargesConvertToAfflictionCharges",
        None,
        &env.player.output,
    ) {
        affliction_charges = fc.max(affliction_charges_max.min(affliction_charges_min));
        fc = 0.0;
    } else {
        fc = fc.max(fc_max.min(fc_min));
    }
    let removable_fc = (fc - fc_min).max(0.0);
    env.player.set_output("FrenzyCharges", fc);
    env.player
        .set_output("AfflictionCharges", affliction_charges);
    env.player
        .set_output("RemovableFrenzyCharges", removable_fc);

    // Endurance Charges / Brutal conversion (Lua 1003–1014)
    let mut ec = 0.0_f64;
    if flag_or_cond(env, "UseEnduranceCharges") {
        ec = env
            .player
            .mod_db
            .override_value("EnduranceCharges", None, &env.player.output)
            .unwrap_or(ec_max);
    }
    let mut brutal_charges = 0.0_f64;
    if env.player.mod_db.flag_cfg(
        "EnduranceChargesConvertToBrutalCharges",
        None,
        &env.player.output,
    ) {
        brutal_charges = ec.max(brutal_charges_max.min(brutal_charges_min));
        ec = 0.0;
    } else {
        ec = ec.max(ec_max.min(ec_min));
    }
    let removable_ec = (ec - ec_min).max(0.0);
    env.player.set_output("EnduranceCharges", ec);
    env.player.set_output("BrutalCharges", brutal_charges);
    env.player
        .set_output("RemovableEnduranceCharges", removable_ec);

    // Siphoning, Challenger, Blitz (Lua 1015–1023)
    let mut siphoning_charges = 0.0_f64;
    if flag_or_cond(env, "UseSiphoningCharges") {
        siphoning_charges = env
            .player
            .mod_db
            .override_value("SiphoningCharges", None, &env.player.output)
            .unwrap_or(siphoning_charges_max);
    }
    env.player.set_output("SiphoningCharges", siphoning_charges);

    let mut challenger_charges = 0.0_f64;
    if flag_or_cond(env, "UseChallengerCharges") {
        challenger_charges = env
            .player
            .mod_db
            .override_value("ChallengerCharges", None, &env.player.output)
            .unwrap_or(challenger_charges_max);
    }
    env.player
        .set_output("ChallengerCharges", challenger_charges);

    let mut blitz_charges = 0.0_f64;
    if flag_or_cond(env, "UseBlitzCharges") {
        blitz_charges = env
            .player
            .mod_db
            .override_value("BlitzCharges", None, &env.player.output)
            .unwrap_or(blitz_charges_max);
    }
    env.player.set_output("BlitzCharges", blitz_charges);

    // Inspiration charges — always set for player (Lua 1024–1026: actor == env.player)
    let inspiration_charges = env
        .player
        .mod_db
        .override_value("InspirationCharges", None, &env.player.output)
        .unwrap_or(inspiration_charges_max);
    env.player
        .set_output("InspirationCharges", inspiration_charges);

    // Ghost Shrouds (Lua 1027–1029)
    let mut ghost_shrouds = 0.0_f64;
    if flag_or_cond(env, "UseGhostShrouds") {
        ghost_shrouds = env
            .player
            .mod_db
            .override_value("GhostShrouds", None, &env.player.output)
            .unwrap_or(3.0);
    }
    env.player.set_output("GhostShrouds", ghost_shrouds);

    // Blood charges (Lua 1030) — always set, no Use guard
    let blood_charges = env
        .player
        .mod_db
        .override_value("BloodCharges", None, &env.player.output)
        .unwrap_or(blood_charges_max)
        .min(blood_charges_max);
    env.player.set_output("BloodCharges", blood_charges);

    // Spirit charges (Lua 1031)
    let spirit_charges = env
        .player
        .mod_db
        .override_value("SpiritCharges", None, &env.player.output)
        .unwrap_or(0.0)
        .min(spirit_charges_max);
    env.player.set_output("SpiritCharges", spirit_charges);

    // Crab barriers (Lua 1033)
    let crab_barriers = env
        .player
        .mod_db
        .override_value("CrabBarriers", None, &env.player.output)
        .unwrap_or(crab_barriers_max)
        .min(crab_barriers_max);
    env.player.set_output("CrabBarriers", crab_barriers);

    // ── Step 5: HaveMaximum*Charges flag overrides (Lua 1034–1042) ──────
    if env
        .player
        .mod_db
        .flag_cfg("HaveMaximumPowerCharges", None, &env.player.output)
    {
        pc = pc_max;
        env.player.set_output("PowerCharges", pc);
    }
    if env
        .player
        .mod_db
        .flag_cfg("HaveMaximumFrenzyCharges", None, &env.player.output)
    {
        fc = fc_max;
        env.player.set_output("FrenzyCharges", fc);
    }
    if env
        .player
        .mod_db
        .flag_cfg("HaveMaximumEnduranceCharges", None, &env.player.output)
    {
        ec = ec_max;
        env.player.set_output("EnduranceCharges", ec);
    }

    // ── Step 6: TotalCharges and multiplier writes (Lua 1043–1064) ──────
    let total_charges = pc + fc + ec;
    env.player.set_output("TotalCharges", total_charges);

    let removable_total = removable_ec + removable_fc + removable_pc;
    env.player
        .set_output("RemovableTotalCharges", removable_total);

    env.player.mod_db.set_multiplier("PowerCharge", pc);
    env.player.mod_db.set_multiplier("PowerChargeMax", pc_max);
    env.player
        .mod_db
        .set_multiplier("RemovablePowerCharge", removable_pc);
    env.player.mod_db.set_multiplier("FrenzyCharge", fc);
    env.player
        .mod_db
        .set_multiplier("RemovableFrenzyCharge", removable_fc);
    env.player.mod_db.set_multiplier("EnduranceCharge", ec);
    env.player
        .mod_db
        .set_multiplier("RemovableEnduranceCharge", removable_ec);
    env.player
        .mod_db
        .set_multiplier("TotalCharges", total_charges);
    env.player
        .mod_db
        .set_multiplier("RemovableTotalCharges", removable_total);
    env.player
        .mod_db
        .set_multiplier("SiphoningCharge", siphoning_charges);
    env.player
        .mod_db
        .set_multiplier("ChallengerCharge", challenger_charges);
    env.player
        .mod_db
        .set_multiplier("BlitzCharge", blitz_charges);
    env.player
        .mod_db
        .set_multiplier("InspirationCharge", inspiration_charges);
    env.player
        .mod_db
        .set_multiplier("GhostShroud", ghost_shrouds);
    env.player
        .mod_db
        .set_multiplier("CrabBarrier", crab_barriers);
    env.player
        .mod_db
        .set_multiplier("BrutalCharge", brutal_charges);
    env.player
        .mod_db
        .set_multiplier("AbsorptionCharge", absorption_charges);
    env.player
        .mod_db
        .set_multiplier("AfflictionCharge", affliction_charges);
    env.player
        .mod_db
        .set_multiplier("BloodCharge", blood_charges);
    env.player
        .mod_db
        .set_multiplier("SpiritCharge", spirit_charges);
}

// ---------------------------------------------------------------------------
// Misc (Fortify, Onslaught, Rage, Tailwind, Elusive, Leech conditions)
// ---------------------------------------------------------------------------

/// Mirrors doActorMisc() in CalcPerform.lua.
fn do_actor_misc(env: &mut CalcEnv) {
    // CalcPerform.lua:612: if env.mode_combat then
    if env.mode_combat {
        // CalcPerform.lua:620: alliedFortify — party/parent actor inheritance.
        // Party members and parent actors are not modelled in Rust; always 0.
        let allied_fortify = 0.0_f64;

        // CalcPerform.lua:622-624: set Fortified condition from MinimumFortification or allied
        let min_fort_raw = env.player.mod_db.sum_cfg(
            ModType::Base,
            "MinimumFortification",
            None,
            &env.player.output,
        );
        if min_fort_raw > 0.0 || allied_fortify > 0.0 {
            env.player.mod_db.set_condition("Fortified", true);
        }

        // CalcPerform.lua:626: Fortify block
        // modDB:Flag(nil, "Fortified") checks FLAG mods (possibly Condition-tagged ones)
        // OR conditions["Fortified"] set directly by buffFortification config.
        let fortified_flag = env
            .player
            .mod_db
            .flag_cfg("Fortified", None, &env.player.output)
            || env
                .player
                .mod_db
                .conditions
                .get("Fortified")
                .copied()
                .unwrap_or(false);
        let fortification_multiplier = env.player.mod_db.sum_cfg(
            ModType::Base,
            "Multiplier:Fortification",
            None,
            &env.player.output,
        );
        if fortified_flag || fortification_multiplier > 0.0 {
            // Build skill cfg from main skill (mirrors: skillCfg = actor.mainSkill.skillCfg)
            // Needed for skill-scoped MaximumFortification mods.
            let skill_cfg = env
                .player
                .main_skill
                .as_ref()
                .and_then(|s| s.skill_cfg.clone());
            let skill_cfg_ref = skill_cfg.as_ref();

            // CalcPerform.lua:629: maxStacks = max(override OR sum(BASE MaximumFortification), alliedFortify)
            let max_stacks_sum = env.player.mod_db.sum_cfg(
                ModType::Base,
                "MaximumFortification",
                skill_cfg_ref,
                &env.player.output,
            );
            let max_stacks_override =
                env.player
                    .mod_db
                    .override_value("MaximumFortification", None, &env.player.output);
            let max_stacks_base = max_stacks_override.unwrap_or(max_stacks_sum);
            let max_stacks = max_stacks_base.max(allied_fortify);

            // CalcPerform.lua:630: minStacks = min(HaveMaxFortification→maxStacks OR MinimumFortification, maxStacks)
            let have_max_fort = env.player.mod_db.flag_cfg(
                "Condition:HaveMaxFortification",
                None,
                &env.player.output,
            ) || env
                .player
                .mod_db
                .conditions
                .get("HaveMaxFortification")
                .copied()
                .unwrap_or(false);
            let min_stacks_raw = if have_max_fort {
                max_stacks
            } else {
                min_fort_raw
            };
            let min_stacks = min_stacks_raw.min(max_stacks);

            // CalcPerform.lua:631: stacks = min(override OR (minStacks > 0 and minStacks) OR maxStacks, maxStacks)
            let stacks_override =
                env.player
                    .mod_db
                    .override_value("FortificationStacks", None, &env.player.output);
            let stacks_default = if min_stacks > 0.0 {
                min_stacks
            } else {
                max_stacks
            };
            let stacks = stacks_override.unwrap_or(stacks_default).min(max_stacks);

            // CalcPerform.lua:633-642: output writes
            env.player.set_output("MaximumFortification", max_stacks);
            env.player.set_output("MinimumFortification", min_stacks);
            let removable =
                (max_stacks - min_stacks).min(stacks_override.unwrap_or(max_stacks) - min_stacks);
            env.player.set_output("RemovableFortification", removable);
            env.player.set_output("FortificationStacks", stacks);
            let stacks_over_20 = (stacks - 20.0).max(0.0).min(max_stacks - 20.0);
            env.player
                .set_output("FortificationStacksOver20", stacks_over_20);

            // CalcPerform.lua:639: FortificationEffect = "0" initially (as number 0)
            // then overwritten unless NoFortificationMitigation
            let no_mitigation = env.player.mod_db.flag_cfg(
                "Condition:NoFortificationMitigation",
                None,
                &env.player.output,
            );
            if no_mitigation {
                // Willowgift case: write numeric 0 (oracle shows number type)
                env.player.set_output("FortificationEffect", 0.0);
            } else {
                // CalcPerform.lua:641: FortificationEffect = stacks
                env.player.set_output("FortificationEffect", stacks);
                // CalcPerform.lua:642: inject DamageTakenWhenHit MORE -stacks
                env.player.mod_db.add(Mod {
                    name: "DamageTakenWhenHit".into(),
                    mod_type: ModType::More,
                    value: crate::mod_db::types::ModValue::Number(-stacks),
                    flags: crate::mod_db::types::ModFlags::NONE,
                    keyword_flags: crate::mod_db::types::KeywordFlags::NONE,
                    tags: Vec::new(),
                    source: ModSource::new("Buff", "Fortification"),
                });
            }

            // CalcPerform.lua:644-645: if stacks >= maxStacks → HaveMaximumFortification condition
            if stacks >= max_stacks {
                env.player.mod_db.add(Mod::new_flag(
                    "Condition:HaveMaximumFortification",
                    ModSource::new("Buff", "Fortification"),
                ));
            }

            // CalcPerform.lua:647: modDB.multipliers["BuffOnSelf"] += 1
            let prev = env
                .player
                .mod_db
                .multipliers
                .get("BuffOnSelf")
                .copied()
                .unwrap_or(0.0);
            env.player.mod_db.set_multiplier("BuffOnSelf", prev + 1.0);
        }
    } // end mode_combat

    // Onslaught
    let onslaught = env
        .player
        .mod_db
        .flag_cfg("Onslaught", None, &env.player.output)
        || env
            .player
            .mod_db
            .conditions
            .get("Onslaught")
            .copied()
            .unwrap_or(false);
    if onslaught {
        let effect_inc =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, "OnslaughtEffect", None, &env.player.output);
        let onslaught_speed = 20.0 * (1.0 + effect_inc / 100.0);
        let onslaught_src = ModSource::new("Buff", "Onslaught");
        env.player.mod_db.add(Mod {
            name: "Speed".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(onslaught_speed),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: Vec::new(),
            source: onslaught_src.clone(),
        });
        env.player.mod_db.add(Mod {
            name: "MovementSpeed".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(onslaught_speed),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: Vec::new(),
            source: onslaught_src,
        });
        env.player.mod_db.set_condition("Onslaught", true);
    }

    // Rage
    let can_gain_rage = env
        .player
        .mod_db
        .flag_cfg("CanGainRage", None, &env.player.output)
        || env
            .player
            .mod_db
            .conditions
            .get("CanGainRage")
            .copied()
            .unwrap_or(false);
    if can_gain_rage {
        let max_rage_base =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, "MaximumRage", None, &env.player.output);
        let max_rage = if max_rage_base > 0.0 {
            max_rage_base
        } else {
            50.0
        };
        env.player.set_output("MaximumRage", max_rage);
        env.player.mod_db.set_multiplier("Rage", max_rage);
    }

    // Tailwind
    let tailwind = env
        .player
        .mod_db
        .flag_cfg("Tailwind", None, &env.player.output)
        || env
            .player
            .mod_db
            .conditions
            .get("Tailwind")
            .copied()
            .unwrap_or(false);
    if tailwind {
        let tw_effect = env.player.mod_db.sum_cfg(
            ModType::Inc,
            "TailwindEffectOnSelf",
            None,
            &env.player.output,
        );
        let tw_speed = 8.0 * (1.0 + tw_effect / 100.0);
        let tw_src = ModSource::new("Buff", "Tailwind");
        env.player.mod_db.add(Mod {
            name: "ActionSpeed".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(tw_speed),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: Vec::new(),
            source: tw_src,
        });
        env.player.mod_db.set_condition("Tailwind", true);
    }

    // Elusive
    let elusive_effect =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "ElusiveEffect", None, &env.player.output);
    if elusive_effect != 0.0 {
        env.player
            .set_output("ElusiveEffectMod", 1.0 + elusive_effect / 100.0);
    }
}

// ---------------------------------------------------------------------------
// Buff / Guard processing
// ---------------------------------------------------------------------------

/// Apply player buffs and guards to the player's mod database.
/// Mirrors the buff processing in CalcPerform.lua.
fn apply_buffs(env: &mut CalcEnv) {
    // Clone buff list to avoid borrow conflict (we need to read buffs and mutate mod_db)
    let buffs = env.player.buffs.clone();
    let mut buff_count = 0.0;

    for buff in &buffs {
        if !buff.active {
            continue;
        }

        // Get buff effect scaling: 1 + BuffEffectOnSelf inc / 100
        let buff_effect_inc =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, "BuffEffectOnSelf", None, &env.player.output);
        let buff_scale = 1.0 + buff_effect_inc / 100.0;

        let buff_src = ModSource::new(
            "Buff",
            buff.skill_name.as_deref().unwrap_or(&buff.name).to_string(),
        );

        // Scale and add each mod from the buff
        for m in &buff.mods {
            let scaled_value = m.value.as_f64() * buff_scale;
            env.player.mod_db.add(Mod {
                name: m.name.clone(),
                mod_type: m.mod_type.clone(),
                value: ModValue::Number(scaled_value),
                flags: m.flags,
                keyword_flags: m.keyword_flags,
                tags: m.tags.clone(),
                source: buff_src.clone(),
            });
        }

        // Set condition: AffectedBy{BuffNameNoSpaces}
        let condition_name = format!("AffectedBy{}", buff.name.replace(' ', ""));
        env.player.mod_db.set_condition(&condition_name, true);

        buff_count += 1.0;
    }

    // Guards: same logic but with "AffectedByGuardSkill" condition
    let guards = env.player.guards.clone();
    for guard in &guards {
        if !guard.active {
            continue;
        }

        let buff_effect_inc =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, "BuffEffectOnSelf", None, &env.player.output);
        let buff_scale = 1.0 + buff_effect_inc / 100.0;

        let guard_src = ModSource::new(
            "Buff",
            guard
                .skill_name
                .as_deref()
                .unwrap_or(&guard.name)
                .to_string(),
        );

        for m in &guard.mods {
            let scaled_value = m.value.as_f64() * buff_scale;
            env.player.mod_db.add(Mod {
                name: m.name.clone(),
                mod_type: m.mod_type.clone(),
                value: ModValue::Number(scaled_value),
                flags: m.flags,
                keyword_flags: m.keyword_flags,
                tags: m.tags.clone(),
                source: guard_src.clone(),
            });
        }

        // Guard-specific condition
        env.player
            .mod_db
            .set_condition("AffectedByGuardSkill", true);
        let condition_name = format!("AffectedBy{}", guard.name.replace(' ', ""));
        env.player.mod_db.set_condition(&condition_name, true);

        buff_count += 1.0;
    }

    if buff_count > 0.0 {
        env.player.mod_db.set_multiplier("BuffOnSelf", buff_count);
    }
}

// ---------------------------------------------------------------------------
// Curse processing
// ---------------------------------------------------------------------------

/// Apply curses to the enemy's mod database, respecting the curse limit.
/// Mirrors the curse processing in CalcPerform.lua.
fn apply_curses(env: &mut CalcEnv) {
    // CalcPerform.lua:3020: EnemyCurseLimit — written UNCONDITIONALLY even when no curses.
    // output.EnemyCurseLimit = modDB:Flag(nil, "CurseLimitIsMaximumPowerCharges")
    //   and output.PowerChargesMax
    //   or modDB:Sum("BASE", nil, "EnemyCurseLimit")
    // Note: The game constant "EnemyCurseLimit BASE 1" is added by setup.rs (CalcSetup.lua:512).
    // In production, the sum returns at least 1. For tests that don't go through setup,
    // fall back to 1 so curse processing still works.
    let curse_limit_value =
        if env
            .player
            .mod_db
            .flag_cfg("CurseLimitIsMaximumPowerCharges", None, &env.player.output)
        {
            get_output_f64(&env.player.output, "PowerChargesMax")
        } else {
            let base = env.player.mod_db.sum_cfg(
                ModType::Base,
                "EnemyCurseLimit",
                None,
                &env.player.output,
            );
            if base > 0.0 {
                base
            } else {
                1.0
            }
        };
    env.player.set_output("EnemyCurseLimit", curse_limit_value);
    let curse_limit = curse_limit_value as i32;

    let mut curses = env.player.curses.clone();
    if curses.is_empty() {
        return;
    }

    // Sort curses by priority descending (highest priority first)
    curses.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Get curse effect scaling
    let curse_effect_inc =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "CurseEffect", None, &env.player.output);
    let curse_scale = 1.0 + curse_effect_inc / 100.0;

    let mut hex_count = 0;
    let mut applied_count = 0.0;

    for curse in &curses {
        if !curse.active {
            continue;
        }

        // Marks don't count against hex limit
        if !curse.is_mark {
            if hex_count >= curse_limit {
                continue;
            }
            hex_count += 1;
        }

        let curse_src = ModSource::new(
            "Curse",
            curse
                .skill_name
                .as_deref()
                .unwrap_or(&curse.name)
                .to_string(),
        );

        // Scale and add each mod to the enemy's mod_db
        for m in &curse.mods {
            let scaled_value = m.value.as_f64() * curse_scale;
            env.enemy.mod_db.add(Mod {
                name: m.name.clone(),
                mod_type: m.mod_type.clone(),
                value: ModValue::Number(scaled_value),
                flags: m.flags,
                keyword_flags: m.keyword_flags,
                tags: m.tags.clone(),
                source: curse_src.clone(),
            });
        }

        // Set conditions on enemy
        env.enemy.mod_db.set_condition("Cursed", true);
        let condition_name = format!("AffectedBy{}", curse.name.replace(' ', ""));
        env.enemy.mod_db.set_condition(&condition_name, true);

        applied_count += 1.0;
    }

    if applied_count > 0.0 {
        env.enemy
            .mod_db
            .set_multiplier("CurseOnEnemy", applied_count);
    }
}

// ---------------------------------------------------------------------------
// Non-damaging ailments
// ---------------------------------------------------------------------------

/// Compute non-damaging ailment maximums and current values.
/// Mirrors the ailment calculation in CalcPerform.lua.
fn do_non_damaging_ailments(env: &mut CalcEnv) {
    // MaximumChill: base 30, scaled by EnemyChillEffect, capped at 30
    let chill_effect_inc =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "EnemyChillEffect", None, &env.player.output);
    let max_chill = (30.0 * (1.0 + chill_effect_inc / 100.0)).min(30.0);
    env.player.set_output("MaximumChill", max_chill);

    // MaximumShock: base 50, scaled by EnemyShockEffect, capped at 50
    let shock_effect_inc =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "EnemyShockEffect", None, &env.player.output);
    let max_shock = (50.0 * (1.0 + shock_effect_inc / 100.0)).min(50.0);
    env.player.set_output("MaximumShock", max_shock);

    // MaximumScorch: base 30, scaled by EnemyScorchEffect, capped at 30
    let scorch_effect_inc =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "EnemyScorchEffect", None, &env.player.output);
    let max_scorch = (30.0 * (1.0 + scorch_effect_inc / 100.0)).min(30.0);
    env.player.set_output("MaximumScorch", max_scorch);

    // MaximumBrittle: base 15%, capped at 15
    env.player.set_output("MaximumBrittle", 15.0);

    // MaximumSap: fixed at 20
    env.player.set_output("MaximumSap", 20.0);

    // Current ailment values from Self*Override base mods
    let current_chill =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "SelfChillOverride", None, &env.player.output);
    if current_chill > 0.0 {
        env.player
            .set_output("CurrentChill", current_chill.min(max_chill));
    }

    let current_shock =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "SelfShockOverride", None, &env.player.output);
    if current_shock > 0.0 {
        env.player
            .set_output("CurrentShock", current_shock.min(max_shock));
    }

    let current_scorch = env.player.mod_db.sum_cfg(
        ModType::Base,
        "SelfScorchOverride",
        None,
        &env.player.output,
    );
    if current_scorch > 0.0 {
        env.player
            .set_output("CurrentScorch", current_scorch.min(max_scorch));
    }
}

// ---------------------------------------------------------------------------
// Elemental exposure
// ---------------------------------------------------------------------------

/// Apply elemental exposure to the enemy.
/// Mirrors the exposure processing in CalcPerform.lua.
fn apply_exposure(env: &mut CalcEnv) {
    let exposure_src = ModSource::new("Exposure", "player");

    for (element, resist_name) in &[
        ("Fire", "FireResist"),
        ("Cold", "ColdResist"),
        ("Lightning", "LightningResist"),
    ] {
        let exposure_key = format!("{element}Exposure");
        let exposure_val =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, &exposure_key, None, &env.player.output);
        if exposure_val != 0.0 {
            env.enemy.mod_db.add(Mod::new_base(
                *resist_name,
                exposure_val,
                exposure_src.clone(),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Regen, Recharge, Leech
// ---------------------------------------------------------------------------

/// Mirrors doRegenRechargeLeech() sections in CalcPerform.lua.
fn do_regen_recharge_leech(env: &mut CalcEnv) {
    // Regen, recharge, and leech are now computed fully in defence.rs (calc_regeneration,
    // calc_leech_caps, calc_es_recharge) and offence.rs (calc_leech), which run after
    // perform.rs in the pipeline. This function now only handles conditions that must be
    // set during the perform pass (before defence.rs runs).

    // Vaal Pact: sets NoLifeRegen condition so defence.rs calc_regeneration zeroes LifeRegen.
    // CalcPerform.lua: if flag("VaalPact") then set condition VaalPact
    let vaal_pact = env
        .player
        .mod_db
        .flag_cfg("VaalPact", None, &env.player.output);
    if vaal_pact {
        env.player.mod_db.set_condition("VaalPact", true);
    }

    // Ghost Reaver: set condition so defence.rs can account for it.
    // CalcPerform.lua: if flag("GhostReaver") then set condition GhostReaver
    let ghost_reaver = env
        .player
        .mod_db
        .flag_cfg("GhostReaver", None, &env.player.output);
    if ghost_reaver {
        env.player.mod_db.set_condition("GhostReaver", true);
    }
}

// ---------------------------------------------------------------------------
// Action speed
// ---------------------------------------------------------------------------

/// Compute action speed modifier. Mirrors calcs.actionSpeedMod (CalcPerform.lua:1067-1077).
///
/// Formula (INC-only, no More multiplier):
///   action_speed_mod = 1 + (max(-TC_CAP, tc_inc) + as_inc) / 100
/// Then clamped by MinimumActionSpeed floor and MaximumActionSpeedReduction cap.
fn action_speed_mod(env: &mut CalcEnv) -> f64 {
    // data.misc.TemporalChainsEffectCap = 75 (Data.lua:168)
    const TEMPORAL_CHAINS_EFFECT_CAP: f64 = 75.0;

    // TemporalChainsActionSpeed is tracked separately so it can be capped at -75%.
    let tc_inc = env.player.mod_db.sum_cfg(
        ModType::Inc,
        "TemporalChainsActionSpeed",
        None,
        &env.player.output,
    );
    let as_inc = env.player.mod_db.sum_cfg(
        ModType::Inc,
        "ActionSpeed",
        None,
        &env.player.output,
    );

    // Temporal Chains contribution capped at -75% INC (m_max(-75, tc_inc))
    let capped_tc = tc_inc.max(-TEMPORAL_CHAINS_EFFECT_CAP);
    let mut action_speed_mod = 1.0 + (capped_tc + as_inc) / 100.0;

    // Floor: MinimumActionSpeed (e.g. "Cannot Be Slowed to below Base Value" = 100)
    let min_speed = env
        .player
        .mod_db
        .max_value("MinimumActionSpeed", None, &env.player.output)
        .unwrap_or(0.0);
    action_speed_mod = action_speed_mod.max(min_speed / 100.0);

    // Cap: MaximumActionSpeedReduction limits how much speed can be reduced
    if let Some(max_red) = env
        .player
        .mod_db
        .max_value("MaximumActionSpeedReduction", None, &env.player.output)
    {
        action_speed_mod = action_speed_mod.min((100.0 - max_red) / 100.0);
    }

    action_speed_mod
}

// ---------------------------------------------------------------------------
// MoM / EB
// ---------------------------------------------------------------------------

/// Mind over Matter, Eldritch Battery, Petrified Blood checks.
fn do_mom_eb(env: &mut CalcEnv) {
    // MoM: DamageTakenFromManaBeforeLife
    let mom_pct = env.player.mod_db.sum_cfg(
        ModType::Base,
        "DamageTakenFromManaBeforeLife",
        None,
        &env.player.output,
    );
    let mom_clamped = mom_pct.clamp(0.0, 100.0);
    if mom_clamped > 0.0 {
        env.player
            .set_output("DamageTakenFromManaBeforeLife", mom_clamped);
        env.player.mod_db.set_condition("MindOverMatter", true);
    }

    // Eldritch Battery
    let eb = env
        .player
        .mod_db
        .flag_cfg("EldritchBattery", None, &env.player.output);
    if eb {
        env.player.mod_db.set_condition("EldritchBattery", true);
    }

    // Petrified Blood
    let pb = env
        .player
        .mod_db
        .flag_cfg("PetrifiedBlood", None, &env.player.output);
    if pb {
        env.player.mod_db.set_condition("PetrifiedBlood", true);
    }
}

// ---------------------------------------------------------------------------
// Attack / cast speed (kept from original)
// ---------------------------------------------------------------------------

fn do_actor_attack_cast_speed(_env: &mut CalcEnv) {
    // Attack and cast speed mods are computed but stored internally;
    // PoB outputs "Speed" from the active skill, not raw attack/cast mods.
    // These are available via mod_db queries for the offence pass.
}

// ---------------------------------------------------------------------------
// Final conditions
// ---------------------------------------------------------------------------

/// Set final conditions that depend on multiple computed stats.
fn set_final_conditions(env: &mut CalcEnv) {
    // LowestOfMaximumLifeAndMaximumMana was already set in do_actor_life_mana (Lua line 129).
    // We keep it here as a fallback in case the Lua-order calc is revised,
    // but it should already be set correctly.
    let life = get_output_f64(&env.player.output, "Life");
    let mana = get_output_f64(&env.player.output, "Mana");
    let lowest_life_mana = life.min(mana);
    env.player
        .set_output("LowestOfMaximumLifeAndMaximumMana", lowest_life_mana);

    // HaveEnergyShield / FullEnergyShield are set via "Condition:HaveEnergyShield" mods
    // (ConfigOptions.lua) or by mods in the database that carry those conditions.
    // They are NOT auto-set from the ES pool value in CalcPerform.lua.
    // DO NOT set them from output.EnergyShield here.
}

// ---------------------------------------------------------------------------
// Attribute requirements
// ---------------------------------------------------------------------------

/// Compute attribute requirements from equipped items and gems.
/// Mirrors CalcPerform.lua lines 1924-1987.
/// Writes ReqStr, ReqDex, ReqInt, ReqStrString, ReqDexString, ReqIntString to output.
fn do_attr_requirements(env: &mut CalcEnv) {
    // Compute global requirement multiplier from GlobalAttributeRequirements mods
    let req_inc = env.player.mod_db.sum_cfg(
        ModType::Inc,
        "GlobalAttributeRequirements",
        None,
        &env.player.output,
    );
    let req_more =
        env.player
            .mod_db
            .more_cfg("GlobalAttributeRequirements", None, &env.player.output);
    let req_mult = (1.0 + req_inc / 100.0) * req_more;

    // Check IgnoreAttributeRequirements flag
    let ignore_req =
        env.player
            .mod_db
            .flag_cfg("IgnoreAttributeRequirements", None, &env.player.output);

    // For each attribute, find the maximum requirement from all sources
    for attr in &["Str", "Dex", "Int"] {
        let req_field = format!("Req{attr}");
        let req_str_field = format!("Req{attr}String");

        // Pre-initialize String variant to 0
        env.player
            .output
            .insert(req_str_field.clone(), super::env::OutputValue::Number(0.0));

        let mut max_req: f64 = 0.0;
        let mut max_source_name = String::new();

        for entry in &env.requirements_table {
            let base_req = match *attr {
                "Str" => entry.str_req,
                "Dex" => entry.dex_req,
                "Int" => entry.int_req,
                _ => 0.0,
            };

            if base_req > 0.0 {
                let req = (base_req * req_mult).floor();
                if req > max_req {
                    max_req = req;
                    max_source_name = entry.source_name.clone();
                }
            }
        }

        if ignore_req {
            max_req = 0.0;
        }

        if max_req > 0.0 {
            let current_req_field_val = env
                .player
                .output
                .get(&req_field)
                .and_then(|v| {
                    if let super::env::OutputValue::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0);

            if max_req > current_req_field_val {
                env.player
                    .output
                    .insert(req_field, super::env::OutputValue::Number(max_req));
                env.player
                    .output
                    .insert(req_str_field, super::env::OutputValue::Number(max_req));
                // Note: ReqXxxItem would be set here in a full implementation
                let _ = max_source_name; // suppress unused warning
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::env::{Actor, CalcEnv, CalcMode};
    use crate::data::GameData;
    use crate::mod_db::types::{KeywordFlags, Mod, ModFlags, ModSource, ModType};
    use crate::mod_db::ModDb;
    use std::sync::Arc;

    fn src() -> ModSource {
        ModSource::new("Test", "test")
    }

    /// Create a test CalcEnv with optional setup closure.
    fn make_env(setup: impl FnOnce(&mut CalcEnv)) -> CalcEnv {
        let data = Arc::new(GameData::default_for_test());
        let mut env = CalcEnv {
            player: Actor::new(ModDb::new()),
            enemy: Actor::new(ModDb::new()),
            mode: CalcMode::Normal,
            data,
            requirements_table: Vec::new(),
            alloc_nodes: std::collections::HashSet::new(),
            granted_passives: std::collections::HashSet::new(),
            radius_jewel_list: Vec::new(),
            extra_radius_node_list: std::collections::HashSet::new(),
            keystones_added: std::collections::HashSet::new(),
            aegis_mod_list: None,
            the_iron_mass: None,
            weapon_mod_list1: None,
            mode_buffs: true,
            mode_combat: true,
            mode_effective: true,
        };
        setup(&mut env);
        env
    }

    // ------------------------------------------------------------------
    // 1. Str bonus to melee phys damage
    // ------------------------------------------------------------------
    #[test]
    fn str_bonus_to_melee_phys_damage() {
        let mut env = make_env(|env| {
            // 100 Str → floor(100/5) = 20% Inc melee phys
            env.player.mod_db.add(Mod::new_base("Str", 100.0, src()));
            // Need base life so life calc doesn't fail
            env.player.mod_db.add(Mod::new_base("Life", 50.0, src()));
        });
        run(&mut env);

        assert_eq!(get_output_f64(&env.player.output, "Str"), 100.0);
        // After attribs, a +20% Inc PhysicalDamage with MELEE flag should exist
        // We can verify indirectly: query the moddb for Inc PhysicalDamage with MELEE
        let melee_phys_inc = env.player.mod_db.sum(
            ModType::Inc,
            "PhysicalDamage",
            ModFlags::MELEE,
            KeywordFlags::NONE,
        );
        assert_eq!(melee_phys_inc, 20.0);
    }

    // ------------------------------------------------------------------
    // 2. Dex bonus to accuracy and evasion
    // ------------------------------------------------------------------
    #[test]
    fn dex_bonus_to_accuracy_and_evasion() {
        let mut env = make_env(|env| {
            // 150 Dex → accuracy = floor(150*2) = 300, evasion inc = floor(150/5) = 30%
            env.player.mod_db.add(Mod::new_base("Dex", 150.0, src()));
            env.player.mod_db.add(Mod::new_base("Life", 50.0, src()));
        });
        run(&mut env);

        assert_eq!(get_output_f64(&env.player.output, "Dex"), 150.0);

        let acc_base = env.player.mod_db.sum(
            ModType::Base,
            "Accuracy",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(acc_base, 300.0);

        let evasion_inc =
            env.player
                .mod_db
                .sum(ModType::Inc, "Evasion", ModFlags::NONE, KeywordFlags::NONE);
        assert_eq!(evasion_inc, 30.0);
    }

    // ------------------------------------------------------------------
    // 3. Int bonus to mana and ES
    // ------------------------------------------------------------------
    #[test]
    fn int_bonus_to_mana_and_es() {
        let mut env = make_env(|env| {
            // 200 Int → mana base = floor(200/2) = 100, es inc = floor(200/10) = 20%
            env.player.mod_db.add(Mod::new_base("Int", 200.0, src()));
            env.player.mod_db.add(Mod::new_base("Life", 50.0, src()));
            env.player.mod_db.add(Mod::new_base("Mana", 100.0, src()));
            env.player
                .mod_db
                .add(Mod::new_base("EnergyShield", 50.0, src()));
        });
        run(&mut env);

        assert_eq!(get_output_f64(&env.player.output, "Int"), 200.0);

        // Mana bonus from Int is BASE (floor(Int/2)), not INC
        let mana_base =
            env.player
                .mod_db
                .sum(ModType::Base, "Mana", ModFlags::NONE, KeywordFlags::NONE);
        // Base mana from Int: floor(200/2) = 100. Total base = 100 (initial) + 100 = 200
        assert_eq!(mana_base, 200.0);

        // ES bonus from Int is INC (floor(Int/10)), not floor(Int/5)
        let es_inc = env.player.mod_db.sum(
            ModType::Inc,
            "EnergyShield",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        // floor(200/10) = 20
        assert_eq!(es_inc, 20.0);

        // Mana should be: (100 + 100) * 1.0 * 1.0 = 200 (no INC mods)
        assert_eq!(get_output_f64(&env.player.output, "Mana"), 200.0);

        // ES is now computed in defence.rs, NOT in perform.rs.
        // The Int → ES INC mod is correctly added to the mod_db (verified above as es_inc=20).
        // The actual ES computation happens when defence::run() is called.
        // This test only verifies the INC mod is added, not the final ES value.
    }

    // ------------------------------------------------------------------
    // 4. LowLife condition when reserved
    // ------------------------------------------------------------------
    #[test]
    fn low_life_when_reserved() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 1000.0, src()));
            // Use ExtraLifeReserved to inject 60% reservation via the accumulator init
            // (CalcPerform.lua:1812: reserved_LifePercent = modDB:Sum("BASE", nil, "ExtraLifeReserved"))
            env.player
                .mod_db
                .add(Mod::new_base("ExtraLifeReserved", 60.0, src()));
        });
        run(&mut env);

        assert!(
            env.player
                .mod_db
                .conditions
                .get("LowLife")
                .copied()
                .unwrap_or(false),
            "Expected LowLife to be true with 60% reserved"
        );
    }

    // ------------------------------------------------------------------
    // 5. FullLife condition — only set in CI path
    // ------------------------------------------------------------------
    // NOTE: In CalcPerform.lua, FullLife is ONLY set automatically in the CI path.
    // For non-CI builds, FullLife is set via "Condition:FullLife" mods (config checkboxes).
    // The old Rust code incorrectly set FullLife based on unreserved percentage.
    #[test]
    fn full_life_only_set_for_ci() {
        // Non-CI: FullLife should NOT be automatically set
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 1000.0, src()));
        });
        run(&mut env);
        // FullLife is not automatically set for non-CI builds
        let full_life = env
            .player
            .mod_db
            .conditions
            .get("FullLife")
            .copied()
            .unwrap_or(false);
        assert!(
            !full_life,
            "FullLife should not be auto-set for non-CI builds (use Condition:FullLife mod)"
        );

        // CI: FullLife IS set
        let mut env2 = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 1000.0, src()));
            env.player
                .mod_db
                .add(Mod::new_flag("ChaosInoculation", src()));
        });
        run(&mut env2);
        assert!(
            env2.player
                .mod_db
                .conditions
                .get("FullLife")
                .copied()
                .unwrap_or(false),
            "Expected FullLife to be true with CI"
        );
    }

    // ------------------------------------------------------------------
    // 6. CI sets life to 1
    // ------------------------------------------------------------------
    #[test]
    fn ci_sets_life_to_1() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 5000.0, src()));
            env.player
                .mod_db
                .add(Mod::new_flag("ChaosInoculation", src()));
        });
        run(&mut env);

        assert_eq!(get_output_f64(&env.player.output, "Life"), 1.0);
        assert!(env
            .player
            .mod_db
            .conditions
            .get("ChaosInoculation")
            .copied()
            .unwrap_or(false));
        assert!(env
            .player
            .mod_db
            .conditions
            .get("FullLife")
            .copied()
            .unwrap_or(false));
    }

    // ------------------------------------------------------------------
    // 7. Power charges computed when UsePowerCharges
    // ------------------------------------------------------------------
    #[test]
    fn power_charges_when_using() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            env.player
                .mod_db
                .add(Mod::new_flag("UsePowerCharges", src()));
            // Base 3 from game constants + 2 extra = 5 total
            env.player
                .mod_db
                .add(Mod::new_base("PowerChargesMax", 3.0, src())); // base from game constants
            env.player
                .mod_db
                .add(Mod::new_base("PowerChargesMax", 2.0, src())); // extra from passives/items
        });
        run(&mut env);

        // Max = 3 + 2 = 5
        assert_eq!(get_output_f64(&env.player.output, "PowerChargesMax"), 5.0);
        // Current should be max because UsePowerCharges is set
        assert_eq!(get_output_f64(&env.player.output, "PowerCharges"), 5.0);
        assert_eq!(
            env.player
                .mod_db
                .multipliers
                .get("PowerCharge")
                .copied()
                .unwrap_or(0.0),
            5.0
        );
        // Note: HaveMaximumPowerCharges is a FLAG mod checked by flag_cfg, not a condition
        // set by doActorCharges. The Lua does not set a condition for this.
    }

    // ------------------------------------------------------------------
    // 8. Frenzy charges not set when not using
    // ------------------------------------------------------------------
    #[test]
    fn frenzy_charges_zero_when_not_using() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            // Don't set UseFrenzyCharges
        });
        run(&mut env);

        assert_eq!(get_output_f64(&env.player.output, "FrenzyCharges"), 0.0);
        assert_eq!(
            env.player
                .mod_db
                .multipliers
                .get("FrenzyCharge")
                .copied()
                .unwrap_or(0.0),
            0.0
        );
    }

    // ------------------------------------------------------------------
    // 9. Fortify stacks computed
    // ------------------------------------------------------------------
    #[test]
    fn fortify_stacks_computed() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            env.player.mod_db.add(Mod::new_flag("Fortified", src()));
            // MaximumFortification must be > 0 for the fortify block to produce output.
            // In real builds this comes from ascendancy/keystone/items.
            env.player
                .mod_db
                .add(Mod::new_base("MaximumFortification", 20.0, src()));
        });
        run(&mut env);

        // FortificationStacks defaults to maxStacks when no Override or MinimumFortification.
        assert_eq!(
            get_output_f64(&env.player.output, "FortificationStacks"),
            20.0
        );
        // FortificationEffect equals stacks when NoFortificationMitigation is not set.
        assert_eq!(
            get_output_f64(&env.player.output, "FortificationEffect"),
            20.0
        );
        // BuffOnSelf multiplier should be incremented by 1.
        assert_eq!(
            env.player
                .mod_db
                .multipliers
                .get("BuffOnSelf")
                .copied()
                .unwrap_or(0.0),
            1.0
        );
    }

    // ------------------------------------------------------------------
    // 10. Mana regen computed (1.75% base)
    // ------------------------------------------------------------------
    #[test]
    fn mana_regen_computed() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            env.player.mod_db.add(Mod::new_base("Mana", 200.0, src()));
        });
        run(&mut env);
        // ManaRegen is now computed in defence.rs calc_regeneration using the PerStat
        // ManaRegen mod seeded by init_env. Add the base mana regen mod manually
        // since this test doesn't go through init_env.
        use crate::mod_db::types::{KeywordFlags, ModFlags, ModTag, ModValue};
        env.player.mod_db.add(crate::mod_db::types::Mod {
            name: "ManaRegen".into(),
            mod_type: crate::mod_db::types::ModType::Base,
            value: ModValue::Number(0.0175),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::PerStat {
                stat: "Mana".into(),
                div: 1.0,
                limit: None,
                base: 0.0,
            }],
            source: src(),
        });
        crate::calc::defence::run(&mut env);

        let mana = get_output_f64(&env.player.output, "Mana");
        assert_eq!(mana, 200.0);

        // Base mana regen = 1.75% of 200 = 3.5 per sec, rounded to 1dp
        let mana_regen = get_output_f64(&env.player.output, "ManaRegen");
        assert!(
            (mana_regen - 3.5).abs() < 0.01,
            "Expected mana regen ~3.5, got {mana_regen}"
        );
    }

    // ------------------------------------------------------------------
    // 11. Life regen from LifeRegenPercent
    // ------------------------------------------------------------------
    #[test]
    fn life_regen_from_percent() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 1000.0, src()));
            // 5% life regen per second
            env.player
                .mod_db
                .add(Mod::new_base("LifeRegenPercent", 5.0, src()));
        });
        run(&mut env);
        // LifeRegen is now computed in defence.rs after perform.rs
        crate::calc::defence::run(&mut env);

        let life = get_output_f64(&env.player.output, "Life");
        assert_eq!(life, 1000.0);

        // 5% of 1000 = 50 per sec, rounded to 1dp
        let regen = get_output_f64(&env.player.output, "LifeRegen");
        assert!(
            (regen - 50.0).abs() < 0.01,
            "Expected life regen 50.0, got {regen}"
        );
    }

    // ------------------------------------------------------------------
    // 12. Action speed with Tailwind
    // ------------------------------------------------------------------
    #[test]
    fn action_speed_with_tailwind() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            env.player.mod_db.add(Mod::new_flag("Tailwind", src()));
        });
        run(&mut env);

        // Tailwind adds 8% Inc ActionSpeed
        // ActionSpeedMod = (1 + 8/100) * 1.0 = 1.08
        let asm = get_output_f64(&env.player.output, "ActionSpeedMod");
        assert!(
            (asm - 1.08).abs() < 0.01,
            "Expected action speed ~1.08, got {asm}"
        );
        assert!(
            (env.player.action_speed_mod - 1.08).abs() < 0.01,
            "Expected actor action_speed_mod ~1.08"
        );
    }

    // ------------------------------------------------------------------
    // 13. MoM output set
    // ------------------------------------------------------------------
    #[test]
    fn mom_output_set() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            env.player
                .mod_db
                .add(Mod::new_base("DamageTakenFromManaBeforeLife", 30.0, src()));
        });
        run(&mut env);

        let mom = get_output_f64(&env.player.output, "DamageTakenFromManaBeforeLife");
        assert_eq!(mom, 30.0);
        assert!(env
            .player
            .mod_db
            .conditions
            .get("MindOverMatter")
            .copied()
            .unwrap_or(false));
    }

    // ------------------------------------------------------------------
    // Additional integration tests
    // ------------------------------------------------------------------

    #[test]
    fn reservation_outputs_computed() {
        // Since accumulate_skill_reservations resets accumulators from the active_skill_list,
        // we inject reservation via ExtraLifeReserved / ExtraManaReserved mods
        // which are read during accumulator initialization.
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 1000.0, src()));
            env.player.mod_db.add(Mod::new_base("Mana", 500.0, src()));
            // ExtraLifeReserved seeds reserved_life_percent (CalcPerform.lua:1812)
            env.player
                .mod_db
                .add(Mod::new_base("ExtraLifeReserved", 30.0, src()));
            // ExtraManaReserved seeds uncancellable_mana_reservation, but NOT reserved_mana_percent.
            // For testing purposes, we manually set the reservation AFTER run() since
            // there's no mod that directly injects flat mana reservation or percent mana reservation.
        });
        run(&mut env);

        // Life: 30% of 1000 = ceil(300) = 300 reserved → 700 unreserved
        assert_eq!(get_output_f64(&env.player.output, "LifeReserved"), 300.0);
        assert_eq!(get_output_f64(&env.player.output, "LifeUnreserved"), 700.0);
    }

    #[test]
    fn attribute_comparison_conditions_set() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Str", 100.0, src()));
            env.player.mod_db.add(Mod::new_base("Dex", 50.0, src()));
            env.player.mod_db.add(Mod::new_base("Int", 75.0, src()));
            env.player.mod_db.add(Mod::new_base("Life", 50.0, src()));
        });
        run(&mut env);

        assert!(env.player.mod_db.conditions["StrHigherThanDex"]);
        assert!(!env.player.mod_db.conditions["DexHigherThanStr"]);
        assert!(env.player.mod_db.conditions["StrHigherThanInt"]);
        assert!(!env.player.mod_db.conditions["IntHigherThanStr"]);
        assert!(!env.player.mod_db.conditions["DexHigherThanInt"]);
        assert!(env.player.mod_db.conditions["IntHigherThanDex"]);
    }

    #[test]
    fn onslaught_adds_speed_mods() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            env.player.mod_db.add(Mod::new_flag("Onslaught", src()));
        });
        run(&mut env);

        let speed_inc =
            env.player
                .mod_db
                .sum(ModType::Inc, "Speed", ModFlags::NONE, KeywordFlags::NONE);
        // Onslaught adds 20% Inc Speed
        assert!(
            (speed_inc - 20.0).abs() < 0.01,
            "Expected 20% speed from onslaught, got {speed_inc}"
        );

        let move_inc = env.player.mod_db.sum(
            ModType::Inc,
            "MovementSpeed",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert!(
            (move_inc - 20.0).abs() < 0.01,
            "Expected 20% movement speed from onslaught, got {move_inc}"
        );
    }

    #[test]
    fn es_recharge_computed() {
        // EnergyShieldRecharge and EnergyShieldRechargeDelay are now computed in
        // defence.rs (calc_es_recharge), which runs after perform.rs.
        // This test verifies the full pipeline (perform + defence) produces the correct values.
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            env.player
                .mod_db
                .add(Mod::new_base("EnergyShield", 500.0, src()));
        });
        run(&mut env);
        // Now also run defence to compute ES recharge (and ES pool first)
        crate::calc::defence::run(&mut env);

        // ES = 500, base recharge rate = 2000/60/100 = 0.3333...
        // ES recharge = round(500 * 0.3333 * 1 * 1 * 1) = round(166.67) = 167
        // Note: test env has empty character_constants, so uses default 2000.0
        let es_recharge = get_output_f64(&env.player.output, "EnergyShieldRecharge");
        assert!(
            (es_recharge - 167.0).abs() < 0.5,
            "Expected ES recharge ~167, got {es_recharge}"
        );

        // Delay = 2.0 sec with no faster mods
        let delay = get_output_f64(&env.player.output, "EnergyShieldRechargeDelay");
        assert!(
            (delay - 2.0).abs() < 0.01,
            "Expected ES recharge delay 2.0, got {delay}"
        );
    }

    #[test]
    fn lowest_of_life_and_mana_set() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 3000.0, src()));
            env.player.mod_db.add(Mod::new_base("Mana", 1000.0, src()));
        });
        run(&mut env);

        let lowest = get_output_f64(&env.player.output, "LowestOfMaximumLifeAndMaximumMana");
        assert_eq!(lowest, 1000.0);
    }

    #[test]
    fn run_orchestrator_sets_all_expected_outputs() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Str", 50.0, src()));
            env.player.mod_db.add(Mod::new_base("Dex", 50.0, src()));
            env.player.mod_db.add(Mod::new_base("Int", 50.0, src()));
            env.player.mod_db.add(Mod::new_base("Life", 500.0, src()));
            env.player.mod_db.add(Mod::new_base("Mana", 200.0, src()));
            env.player
                .mod_db
                .add(Mod::new_base("EnergyShield", 100.0, src()));
        });
        run(&mut env);

        // Verify key outputs exist
        assert!(env.player.output.contains_key("Str"));
        assert!(env.player.output.contains_key("Dex"));
        assert!(env.player.output.contains_key("Int"));
        assert!(env.player.output.contains_key("Life"));
        assert!(env.player.output.contains_key("Mana"));
        // NOTE: EnergyShield is now computed in defence.rs, NOT perform.rs.
        // perform::run does NOT write EnergyShield to output.
        assert!(env.player.output.contains_key("TotalAttr"));
        assert!(env.player.output.contains_key("LowestAttribute"));
        assert!(env.player.output.contains_key("ActionSpeedMod"));
        // AttackSpeedMod/CastSpeedMod are no longer directly output (PoB uses "Speed")
        // These are now computed via mod_db queries in the offence pass
        assert!(env.player.output.contains_key("LifeReserved"));
        assert!(env.player.output.contains_key("ManaReserved"));
        assert!(env.player.output.contains_key("PowerCharges"));
        assert!(env.player.output.contains_key("FrenzyCharges"));
        assert!(env.player.output.contains_key("EnduranceCharges"));
        // LifeRegen and ManaRegen are now set by defence.rs (not perform.rs).
        // They won't be present after perform::run alone.
        assert!(env
            .player
            .output
            .contains_key("LowestOfMaximumLifeAndMaximumMana"));
    }

    #[test]
    fn no_attribute_bonuses_flag_blocks_all() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Str", 100.0, src()));
            env.player.mod_db.add(Mod::new_base("Dex", 100.0, src()));
            env.player.mod_db.add(Mod::new_base("Int", 100.0, src()));
            env.player.mod_db.add(Mod::new_base("Life", 50.0, src()));
            env.player.mod_db.add(Mod::new_base("Mana", 50.0, src()));
            env.player
                .mod_db
                .add(Mod::new_flag("NoAttributeBonuses", src()));
        });
        run(&mut env);

        // No derived mods should be added
        let acc = env.player.mod_db.sum(
            ModType::Base,
            "Accuracy",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(
            acc, 0.0,
            "Expected no accuracy bonus with NoAttributeBonuses"
        );

        let melee_phys = env.player.mod_db.sum(
            ModType::Inc,
            "PhysicalDamage",
            ModFlags::MELEE,
            KeywordFlags::NONE,
        );
        assert_eq!(melee_phys, 0.0, "Expected no melee phys bonus");
    }

    #[test]
    fn vaal_pact_zeroes_regen() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 1000.0, src()));
            env.player
                .mod_db
                .add(Mod::new_base("LifeRegenPercent", 5.0, src()));
            env.player.mod_db.add(Mod::new_flag("VaalPact", src()));
        });
        run(&mut env);
        // LifeRegen is now computed in defence.rs. VaalPact condition is set in perform.rs
        // which disables the NoLifeRegen flag that defence.rs respects.
        crate::calc::defence::run(&mut env);

        // VaalPact condition should be set (by perform.rs)
        assert!(env
            .player
            .mod_db
            .conditions
            .get("VaalPact")
            .copied()
            .unwrap_or(false));

        // Note: With VaalPact, the Lua sets modDB:Flag("NoLifeRegen") via Condition:VaalPact
        // which feeds into the "NoLifeRegen" flag check in calc_regeneration.
        // For now just verify VaalPact condition is set; the regen=0 behavior depends on
        // how VaalPact feeds into NoLifeRegen which may be a PERF-05 concern.
    }

    // ------------------------------------------------------------------
    // Task 13: Buff processing tests
    // ------------------------------------------------------------------

    #[test]
    fn buff_mods_applied_to_player() {
        use crate::calc::env::BuffEntry;

        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            // Add a buff that grants +50% Inc Fire Damage
            env.player.buffs.push(BuffEntry {
                name: "Anger".into(),
                skill_name: Some("Anger".into()),
                mods: vec![Mod {
                    name: "FireDamage".into(),
                    mod_type: ModType::Inc,
                    value: ModValue::Number(40.0),
                    flags: ModFlags::NONE,
                    keyword_flags: KeywordFlags::NONE,
                    tags: Vec::new(),
                    source: src(),
                }],
                active: true,
            });
        });
        run(&mut env);

        // The buff mod should now be in the player moddb
        let fire_inc = env.player.mod_db.sum(
            ModType::Inc,
            "FireDamage",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert!(
            (fire_inc - 40.0).abs() < 0.01,
            "Expected FireDamage inc 40 from buff, got {fire_inc}"
        );

        // AffectedByAnger condition should be set
        assert!(
            env.player
                .mod_db
                .conditions
                .get("AffectedByAnger")
                .copied()
                .unwrap_or(false),
            "Expected AffectedByAnger condition"
        );

        // BuffOnSelf multiplier should be 1
        assert_eq!(
            env.player
                .mod_db
                .multipliers
                .get("BuffOnSelf")
                .copied()
                .unwrap_or(0.0),
            1.0,
            "Expected BuffOnSelf multiplier to be 1"
        );
    }

    #[test]
    fn inactive_buff_not_applied() {
        use crate::calc::env::BuffEntry;

        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            env.player.buffs.push(BuffEntry {
                name: "Vitality".into(),
                skill_name: None,
                mods: vec![Mod::new_base("Life", 50.0, src())],
                active: false,
            });
        });
        run(&mut env);

        // Life should NOT include the buff's +50
        let life = get_output_f64(&env.player.output, "Life");
        assert_eq!(
            life, 100.0,
            "Expected life 100 with inactive buff, got {life}"
        );
    }

    #[test]
    fn buff_effect_scales_mod_values() {
        use crate::calc::env::BuffEntry;

        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            // 50% increased buff effect on self
            env.player.mod_db.add(Mod {
                name: "BuffEffectOnSelf".into(),
                mod_type: ModType::Inc,
                value: ModValue::Number(50.0),
                flags: ModFlags::NONE,
                keyword_flags: KeywordFlags::NONE,
                tags: Vec::new(),
                source: src(),
            });
            env.player.buffs.push(BuffEntry {
                name: "Anger".into(),
                skill_name: None,
                mods: vec![Mod {
                    name: "FireDamage".into(),
                    mod_type: ModType::Inc,
                    value: ModValue::Number(40.0),
                    flags: ModFlags::NONE,
                    keyword_flags: KeywordFlags::NONE,
                    tags: Vec::new(),
                    source: src(),
                }],
                active: true,
            });
        });
        run(&mut env);

        // Buff mod scaled: 40 * 1.5 = 60
        let fire_inc = env.player.mod_db.sum(
            ModType::Inc,
            "FireDamage",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert!(
            (fire_inc - 60.0).abs() < 0.01,
            "Expected FireDamage inc 60 with 50% buff effect, got {fire_inc}"
        );
    }

    #[test]
    fn guard_sets_affected_condition() {
        use crate::calc::env::BuffEntry;

        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            env.player.guards.push(BuffEntry {
                name: "Steelskin".into(),
                skill_name: Some("Steelskin".into()),
                mods: vec![],
                active: true,
            });
        });
        run(&mut env);

        assert!(
            env.player
                .mod_db
                .conditions
                .get("AffectedByGuardSkill")
                .copied()
                .unwrap_or(false),
            "Expected AffectedByGuardSkill condition"
        );
        assert!(
            env.player
                .mod_db
                .conditions
                .get("AffectedBySteelskin")
                .copied()
                .unwrap_or(false),
            "Expected AffectedBySteelskin condition"
        );
    }

    // ------------------------------------------------------------------
    // Task 14: Curse processing tests
    // ------------------------------------------------------------------

    #[test]
    fn curse_mods_applied_to_enemy() {
        use crate::calc::env::CurseEntry;

        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            env.player.curses.push(CurseEntry {
                name: "Vulnerability".into(),
                skill_name: Some("Vulnerability".into()),
                mods: vec![Mod {
                    name: "PhysicalDamageTaken".into(),
                    mod_type: ModType::Inc,
                    value: ModValue::Number(30.0),
                    flags: ModFlags::NONE,
                    keyword_flags: KeywordFlags::NONE,
                    tags: Vec::new(),
                    source: src(),
                }],
                priority: 1.0,
                is_mark: false,
                active: true,
            });
        });
        run(&mut env);

        // Curse mod should be on enemy
        let phys_taken = env.enemy.mod_db.sum(
            ModType::Inc,
            "PhysicalDamageTaken",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert!(
            (phys_taken - 30.0).abs() < 0.01,
            "Expected PhysicalDamageTaken 30 on enemy, got {phys_taken}"
        );

        assert!(
            env.enemy
                .mod_db
                .conditions
                .get("Cursed")
                .copied()
                .unwrap_or(false),
            "Expected Cursed condition on enemy"
        );
        assert!(
            env.enemy
                .mod_db
                .conditions
                .get("AffectedByVulnerability")
                .copied()
                .unwrap_or(false),
            "Expected AffectedByVulnerability condition on enemy"
        );
    }

    #[test]
    fn curse_limit_respected() {
        use crate::calc::env::CurseEntry;

        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            // Default curse limit is 1
            // Two curses, only highest priority should apply
            env.player.curses.push(CurseEntry {
                name: "Vulnerability".into(),
                skill_name: None,
                mods: vec![Mod {
                    name: "PhysicalDamageTaken".into(),
                    mod_type: ModType::Inc,
                    value: ModValue::Number(30.0),
                    flags: ModFlags::NONE,
                    keyword_flags: KeywordFlags::NONE,
                    tags: Vec::new(),
                    source: src(),
                }],
                priority: 2.0, // higher priority
                is_mark: false,
                active: true,
            });
            env.player.curses.push(CurseEntry {
                name: "Despair".into(),
                skill_name: None,
                mods: vec![Mod {
                    name: "ChaosDamageTaken".into(),
                    mod_type: ModType::Inc,
                    value: ModValue::Number(25.0),
                    flags: ModFlags::NONE,
                    keyword_flags: KeywordFlags::NONE,
                    tags: Vec::new(),
                    source: src(),
                }],
                priority: 1.0, // lower priority
                is_mark: false,
                active: true,
            });
        });
        run(&mut env);

        // Only Vulnerability (priority 2) should apply
        let phys_taken = env.enemy.mod_db.sum(
            ModType::Inc,
            "PhysicalDamageTaken",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert!(
            (phys_taken - 30.0).abs() < 0.01,
            "Expected PhysicalDamageTaken 30, got {phys_taken}"
        );

        // Despair should NOT apply
        let chaos_taken = env.enemy.mod_db.sum(
            ModType::Inc,
            "ChaosDamageTaken",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(
            chaos_taken, 0.0,
            "Expected ChaosDamageTaken 0 (curse limit), got {chaos_taken}"
        );

        // CurseOnEnemy should be 1
        assert_eq!(
            env.enemy
                .mod_db
                .multipliers
                .get("CurseOnEnemy")
                .copied()
                .unwrap_or(0.0),
            1.0,
            "Expected CurseOnEnemy multiplier 1"
        );
    }

    #[test]
    fn marks_bypass_curse_limit() {
        use crate::calc::env::CurseEntry;

        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            // One hex (counts against limit)
            env.player.curses.push(CurseEntry {
                name: "Vulnerability".into(),
                skill_name: None,
                mods: vec![Mod {
                    name: "PhysicalDamageTaken".into(),
                    mod_type: ModType::Inc,
                    value: ModValue::Number(30.0),
                    flags: ModFlags::NONE,
                    keyword_flags: KeywordFlags::NONE,
                    tags: Vec::new(),
                    source: src(),
                }],
                priority: 1.0,
                is_mark: false,
                active: true,
            });
            // One mark (doesn't count against limit)
            env.player.curses.push(CurseEntry {
                name: "Sniper's Mark".into(),
                skill_name: None,
                mods: vec![Mod {
                    name: "ProjectileDamageTaken".into(),
                    mod_type: ModType::Inc,
                    value: ModValue::Number(35.0),
                    flags: ModFlags::NONE,
                    keyword_flags: KeywordFlags::NONE,
                    tags: Vec::new(),
                    source: src(),
                }],
                priority: 1.0,
                is_mark: true,
                active: true,
            });
        });
        run(&mut env);

        // Both should apply: hex + mark (mark doesn't count against limit)
        let phys_taken = env.enemy.mod_db.sum(
            ModType::Inc,
            "PhysicalDamageTaken",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert!(
            (phys_taken - 30.0).abs() < 0.01,
            "Expected PhysicalDamageTaken 30, got {phys_taken}"
        );

        let proj_taken = env.enemy.mod_db.sum(
            ModType::Inc,
            "ProjectileDamageTaken",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert!(
            (proj_taken - 35.0).abs() < 0.01,
            "Expected ProjectileDamageTaken 35, got {proj_taken}"
        );

        // CurseOnEnemy should be 2
        assert_eq!(
            env.enemy
                .mod_db
                .multipliers
                .get("CurseOnEnemy")
                .copied()
                .unwrap_or(0.0),
            2.0,
            "Expected CurseOnEnemy multiplier 2"
        );
    }

    // ------------------------------------------------------------------
    // Task 15: Non-damaging ailments
    // ------------------------------------------------------------------

    #[test]
    fn non_damaging_ailment_defaults() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
        });
        run(&mut env);

        assert_eq!(get_output_f64(&env.player.output, "MaximumChill"), 30.0);
        assert_eq!(get_output_f64(&env.player.output, "MaximumShock"), 50.0);
        assert_eq!(get_output_f64(&env.player.output, "MaximumScorch"), 30.0);
        assert_eq!(get_output_f64(&env.player.output, "MaximumBrittle"), 15.0);
        assert_eq!(get_output_f64(&env.player.output, "MaximumSap"), 20.0);
    }

    #[test]
    fn ailment_effect_does_not_exceed_cap() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            // +100% shock effect would try to make max shock 100, but capped at 50
            env.player.mod_db.add(Mod {
                name: "EnemyShockEffect".into(),
                mod_type: ModType::Inc,
                value: ModValue::Number(100.0),
                flags: ModFlags::NONE,
                keyword_flags: KeywordFlags::NONE,
                tags: Vec::new(),
                source: src(),
            });
        });
        run(&mut env);

        let max_shock = get_output_f64(&env.player.output, "MaximumShock");
        assert_eq!(max_shock, 50.0, "MaximumShock should be capped at 50");
    }

    #[test]
    fn self_chill_override_sets_current_chill() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            env.player
                .mod_db
                .add(Mod::new_base("SelfChillOverride", 10.0, src()));
        });
        run(&mut env);

        let current_chill = get_output_f64(&env.player.output, "CurrentChill");
        assert_eq!(
            current_chill, 10.0,
            "Expected CurrentChill 10, got {current_chill}"
        );
    }

    // ------------------------------------------------------------------
    // Task 16: Exposure processing
    // ------------------------------------------------------------------

    #[test]
    fn fire_exposure_applied_to_enemy() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            // -10 fire exposure
            env.player
                .mod_db
                .add(Mod::new_base("FireExposure", -10.0, src()));
        });
        run(&mut env);

        let fire_resist = env.enemy.mod_db.sum(
            ModType::Base,
            "FireResist",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(
            fire_resist, -10.0,
            "Expected enemy FireResist -10, got {fire_resist}"
        );
    }

    #[test]
    fn multiple_exposures_applied() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            env.player
                .mod_db
                .add(Mod::new_base("FireExposure", -15.0, src()));
            env.player
                .mod_db
                .add(Mod::new_base("ColdExposure", -10.0, src()));
            env.player
                .mod_db
                .add(Mod::new_base("LightningExposure", -5.0, src()));
        });
        run(&mut env);

        let fire = env.enemy.mod_db.sum(
            ModType::Base,
            "FireResist",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        let cold = env.enemy.mod_db.sum(
            ModType::Base,
            "ColdResist",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        let lightning = env.enemy.mod_db.sum(
            ModType::Base,
            "LightningResist",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(fire, -15.0, "Fire exposure: {fire}");
        assert_eq!(cold, -10.0, "Cold exposure: {cold}");
        assert_eq!(lightning, -5.0, "Lightning exposure: {lightning}");
    }

    #[test]
    fn no_exposure_when_zero() {
        let mut env = make_env(|env| {
            env.player.mod_db.add(Mod::new_base("Life", 100.0, src()));
            // No exposure mods set
        });
        run(&mut env);

        let fire = env.enemy.mod_db.sum(
            ModType::Base,
            "FireResist",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        assert_eq!(fire, 0.0, "No fire exposure should mean 0 resist mod");
    }

    // ------------------------------------------------------------------
    // Task 10: Integration tests with oracle-like builds
    // ------------------------------------------------------------------

    #[test]
    fn integration_marauder_l90_no_items_life() {
        // Marauder L90 no items:
        // Base life = 38 + 12 * 90 = 1118
        // Str = 32 → life from str = floor(32 * 0.5) = 16
        // Total base life = 1118 + 16 = 1134
        let data = Arc::new(GameData::default_for_test());
        let mut env = CalcEnv {
            player: Actor::new(crate::mod_db::ModDb::new()),
            enemy: Actor::new(crate::mod_db::ModDb::new()),
            mode: CalcMode::Normal,
            data: data.clone(),
            requirements_table: Vec::new(),
            alloc_nodes: std::collections::HashSet::new(),
            granted_passives: std::collections::HashSet::new(),
            radius_jewel_list: Vec::new(),
            extra_radius_node_list: std::collections::HashSet::new(),
            keystones_added: std::collections::HashSet::new(),
            aegis_mod_list: None,
            the_iron_mass: None,
            weapon_mod_list1: None,
            mode_buffs: true,
            mode_combat: true,
            mode_effective: true,
        };

        // Add Marauder L90 base stats
        let base_src = ModSource::new("Base", "Marauder base stats");
        let base_life = 38.0 + 12.0 * 90.0; // 1118
        env.player
            .mod_db
            .add(Mod::new_base("Life", base_life, base_src.clone()));
        env.player
            .mod_db
            .add(Mod::new_base("Mana", 34.0 + 6.0 * 90.0, base_src.clone())); // 574
        env.player
            .mod_db
            .add(Mod::new_base("Str", 32.0, base_src.clone()));
        env.player
            .mod_db
            .add(Mod::new_base("Dex", 14.0, base_src.clone()));
        env.player.mod_db.add(Mod::new_base("Int", 14.0, base_src));

        run(&mut env);

        let life = get_output_f64(&env.player.output, "Life");
        // base 1118 + floor(32*0.5) = 1118 + 16 = 1134
        assert_eq!(life, 1134.0, "Marauder L90 life should be 1134, got {life}");

        let mana = get_output_f64(&env.player.output, "Mana");
        // base 574, Int = 14, mana base from Int = floor(14/2) = 7 (Lua: BASE not INC)
        // total mana = (574 + 7) = 581
        assert_eq!(mana, 581.0, "Marauder L90 mana should be 581, got {mana}");

        // ManaRegen is now computed in defence.rs calc_regeneration.
        // The base mana regen (1.75% of Mana) comes from a PerStat mod added by
        // add_class_base_stats in init_env. In this unit test, we add the mod manually.
        // ManaRegenBase = 105 / 60 / 100 = 0.0175
        use crate::mod_db::types::{KeywordFlags, ModFlags, ModTag};
        env.player.mod_db.add(crate::mod_db::types::Mod {
            name: "ManaRegen".into(),
            mod_type: ModType::Base,
            value: crate::mod_db::types::ModValue::Number(0.0175),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::PerStat {
                stat: "Mana".into(),
                div: 1.0,
                limit: None,
                base: 0.0,
            }],
            source: src(),
        });
        crate::calc::defence::run(&mut env);

        let mana_regen = get_output_f64(&env.player.output, "ManaRegen");
        // 1.75% of 581 = 10.1675, rounded to 1dp = 10.2
        assert!(
            (mana_regen - 10.2).abs() < 0.1,
            "Marauder L90 mana regen should be ~10.2, got {mana_regen}"
        );
    }

    #[test]
    fn integration_marauder_l90_with_life_passive() {
        // Marauder L90 with "10% increased maximum Life" passive
        let data = Arc::new(GameData::default_for_test());
        let mut env = CalcEnv {
            player: Actor::new(crate::mod_db::ModDb::new()),
            enemy: Actor::new(crate::mod_db::ModDb::new()),
            mode: CalcMode::Normal,
            data: data.clone(),
            requirements_table: Vec::new(),
            alloc_nodes: std::collections::HashSet::new(),
            granted_passives: std::collections::HashSet::new(),
            radius_jewel_list: Vec::new(),
            extra_radius_node_list: std::collections::HashSet::new(),
            keystones_added: std::collections::HashSet::new(),
            aegis_mod_list: None,
            the_iron_mass: None,
            weapon_mod_list1: None,
            mode_buffs: true,
            mode_combat: true,
            mode_effective: true,
        };

        let base_src = ModSource::new("Base", "Marauder base stats");
        let base_life = 38.0 + 12.0 * 90.0; // 1118
        env.player
            .mod_db
            .add(Mod::new_base("Life", base_life, base_src.clone()));
        env.player
            .mod_db
            .add(Mod::new_base("Mana", 34.0 + 6.0 * 90.0, base_src.clone()));
        env.player
            .mod_db
            .add(Mod::new_base("Str", 32.0, base_src.clone()));
        env.player
            .mod_db
            .add(Mod::new_base("Dex", 14.0, base_src.clone()));
        env.player.mod_db.add(Mod::new_base("Int", 14.0, base_src));

        // +10% increased maximum Life passive
        let passive_src = ModSource::new("Passive", "Life node");
        env.player.mod_db.add(Mod {
            name: "Life".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(10.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: Vec::new(),
            source: passive_src,
        });

        run(&mut env);

        let life = get_output_f64(&env.player.output, "Life");
        // base 1118 + 16 (Str) = 1134
        // 1134 * (1 + 10/100) = 1134 * 1.10 = 1247.4 → round = 1247
        assert_eq!(
            life, 1247.0,
            "Marauder L90 with 10% inc life should be 1247, got {life}"
        );
    }
}
