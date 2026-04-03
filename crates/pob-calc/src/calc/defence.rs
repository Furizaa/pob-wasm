use super::env::{get_output_f64, CalcEnv};
use crate::calc::calc_tools::calc_def_mod;
use crate::mod_db::types::{ModType, SkillCfg};

// ── Constants ────────────────────────────────────────────────────────────────

pub const DMG_PHYSICAL: usize = 0;
pub const DMG_LIGHTNING: usize = 1;
pub const DMG_COLD: usize = 2;
pub const DMG_FIRE: usize = 3;
pub const DMG_CHAOS: usize = 4;

pub const DMG_TYPE_NAMES: [&str; 5] = ["Physical", "Lightning", "Cold", "Fire", "Chaos"];
pub const RESIST_TYPE_NAMES: [&str; 4] = ["Fire", "Cold", "Lightning", "Chaos"];

// ── Utility functions ────────────────────────────────────────────────────────

/// PoB hit-chance formula: accuracy / (accuracy + (evasion/5)^0.9) * 125, clamped [5, 100].
pub fn hit_chance(evasion: f64, accuracy: f64) -> f64 {
    if accuracy <= 0.0 {
        return 5.0;
    }
    if evasion <= 0.0 {
        return 100.0;
    }
    let raw = accuracy / (accuracy + (evasion / 5.0).powf(0.9)) * 125.0;
    raw.clamp(5.0, 100.0)
}

/// Armour reduction (float): armour / (armour + raw * 5) * 100. Returns 0 if either <= 0.
pub fn armour_reduction_f(armour: f64, raw: f64) -> f64 {
    if armour <= 0.0 || raw <= 0.0 {
        return 0.0;
    }
    armour / (armour + raw * 5.0) * 100.0
}

/// Armour reduction (floored integer).
pub fn armour_reduction(armour: f64, raw: f64) -> f64 {
    armour_reduction_f(armour, raw).floor()
}

// ── Orchestrator ─────────────────────────────────────────────────────────────

pub fn run(env: &mut CalcEnv) {
    calc_resistances(env);
    calc_block(env);
    calc_primary_defences(env);
    calc_spell_suppression(env);
    calc_dodge(env);
    calc_recovery_rates(env);
    calc_leech_caps(env);
    calc_regeneration(env);
    calc_es_recharge(env);
    calc_damage_reduction(env);
    calc_movement_and_avoidance(env);
    build_damage_shift_table(env);
    calc_stun(env);
    calc_life_recoverable(env);
}

// ── Task 3: Full resistances ─────────────────────────────────────────────────

fn calc_resistances(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // Physical resist is always 0
    env.player.set_output("PhysicalResist", 0.0);

    // Compute max resists first so we can apply Melding
    let elemental_types = ["Fire", "Cold", "Lightning"];
    let mut max_resists: [f64; 4] = [0.0; 4]; // Fire, Cold, Lightning, Chaos

    for (i, elem) in RESIST_TYPE_NAMES.iter().enumerate() {
        let max_stat = format!("{elem}ResistMax");
        let max_val = env
            .player
            .mod_db
            .override_value(&max_stat, None, &output)
            .unwrap_or_else(|| {
                let base_max = env
                    .player
                    .mod_db
                    .sum_cfg(ModType::Base, &max_stat, None, &output);
                let elemental_max = if elemental_types.contains(elem) {
                    env.player
                        .mod_db
                        .sum_cfg(ModType::Base, "ElementalResistMax", None, &output)
                } else {
                    0.0
                };
                base_max + elemental_max
            });
        max_resists[i] = max_val;
    }

    // Melding of the Flesh: if flag set, all elemental max resists = highest
    if env
        .player
        .mod_db
        .flag_cfg("ElementalResistMaxIsHighestResistMax", None, &output)
    {
        let highest = max_resists[0].max(max_resists[1]).max(max_resists[2]);
        max_resists[0] = highest; // Fire
        max_resists[1] = highest; // Cold
        max_resists[2] = highest; // Lightning
                                  // Chaos (index 3) is not affected
    }

    // Now compute each resist type
    for (i, elem) in RESIST_TYPE_NAMES.iter().enumerate() {
        let resist_stat = format!("{elem}Resist");
        let max_resist = max_resists[i];

        let total_resist = env
            .player
            .mod_db
            .override_value(&resist_stat, None, &output)
            .unwrap_or_else(|| {
                let base = env
                    .player
                    .mod_db
                    .sum_cfg(ModType::Base, &resist_stat, None, &output);
                let elemental = if elemental_types.contains(elem) {
                    env.player
                        .mod_db
                        .sum_cfg(ModType::Base, "ElementalResist", None, &output)
                } else {
                    0.0
                };
                base + elemental
            });

        let capped = total_resist.min(max_resist);
        let over_cap = total_resist - max_resist;
        let over_75 = total_resist - 75.0;
        let missing = max_resist - capped;

        env.player.set_output(&resist_stat, capped);
        env.player
            .set_output(&format!("{elem}ResistTotal"), total_resist);
        env.player
            .set_output(&format!("{elem}ResistOverCap"), over_cap.max(0.0));
        env.player
            .set_output(&format!("{elem}ResistOver75"), over_75.max(0.0));
        env.player
            .set_output(&format!("Missing{elem}Resist"), missing.max(0.0));

        // Over-time resist (same as capped for now)
        env.player
            .set_output(&format!("{elem}ResistOverTime"), capped);

        // Totem resists: use their own base resist values (not player resists)
        let totem_resist_stat = format!("Totem{elem}Resist");
        let totem_base_resist =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, &totem_resist_stat, None, &output);
        let totem_max = max_resist; // Totems share the player's max resist cap
        let totem_capped = totem_base_resist.min(totem_max);
        let totem_over_cap = (totem_base_resist - totem_max).max(0.0);
        let missing_totem = (totem_max - totem_capped).max(0.0);
        env.player
            .set_output(&format!("Totem{elem}Resist"), totem_capped);
        env.player
            .set_output(&format!("Totem{elem}ResistTotal"), totem_base_resist);
        env.player
            .set_output(&format!("Totem{elem}ResistOverCap"), totem_over_cap);
        env.player
            .set_output(&format!("MissingTotem{elem}Resist"), missing_totem);
    }
}

// ── Task 4: Full block ───────────────────────────────────────────────────────

fn calc_block(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // Block chance max
    let block_max = {
        let v = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, "BlockChanceMax", None, &output);
        if v == 0.0 {
            75.0
        } else {
            v
        }
    };
    env.player.set_output("BlockChanceMax", block_max);

    // Attack block
    let attack_block = if env
        .player
        .mod_db
        .flag_cfg("CannotBlockAttacks", None, &output)
    {
        0.0
    } else {
        let shield_block =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, "ShieldBlockChance", None, &output);
        let base_block = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, "BlockChance", None, &output);
        let inc_block = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "BlockChance", None, &output);
        ((shield_block + base_block) * (1.0 + inc_block / 100.0))
            .min(block_max)
            .max(0.0)
    };
    env.player.set_output("BlockChance", attack_block);

    // Projectile block
    let extra_proj_block =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ProjectileBlockChance", None, &output);
    env.player.set_output(
        "ProjectileBlockChance",
        (attack_block + extra_proj_block).min(block_max).max(0.0),
    );

    // Spell block max
    let spell_block_max =
        if env
            .player
            .mod_db
            .flag_cfg("SpellBlockChanceMaxIsBlockChanceMax", None, &output)
        {
            block_max
        } else {
            let v = env
                .player
                .mod_db
                .sum_cfg(ModType::Base, "SpellBlockChanceMax", None, &output);
            if v == 0.0 {
                75.0
            } else {
                v
            }
        };
    env.player
        .set_output("SpellBlockChanceMax", spell_block_max);

    // Spell block
    let spell_block = if env
        .player
        .mod_db
        .flag_cfg("CannotBlockSpells", None, &output)
    {
        0.0
    } else if env
        .player
        .mod_db
        .flag_cfg("SpellBlockChanceIsBlockChance", None, &output)
    {
        attack_block.min(spell_block_max)
    } else {
        let base_spell_block =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, "SpellBlockChance", None, &output);
        let inc_spell_block =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, "SpellBlockChance", None, &output);
        (base_spell_block * (1.0 + inc_spell_block / 100.0))
            .min(spell_block_max)
            .max(0.0)
    };
    env.player.set_output("SpellBlockChance", spell_block);

    // Spell projectile block
    let extra_spell_proj =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "SpellProjectileBlockChance", None, &output);
    env.player.set_output(
        "SpellProjectileBlockChance",
        (spell_block + extra_spell_proj)
            .min(spell_block_max)
            .max(0.0),
    );

    // Glancing Blows
    let block_effect = if env.player.mod_db.flag_cfg("GlancingBlows", None, &output) {
        65.0
    } else {
        100.0
    };
    env.player.set_output("BlockEffect", block_effect);
    env.player
        .set_output("DamageTakenOnBlock", 100.0 - block_effect);

    // Effective block chances (lucky/unlucky)
    let lucky_block = env.player.mod_db.flag_cfg("LuckyBlock", None, &output);
    let unlucky_block = env.player.mod_db.flag_cfg("UnluckyBlock", None, &output);

    let effective = |chance: f64| -> f64 {
        let x = chance / 100.0;
        let eff = if lucky_block {
            1.0 - (1.0 - x) * (1.0 - x)
        } else if unlucky_block {
            x * x
        } else {
            x
        };
        eff * 100.0
    };

    let eff_block = effective(attack_block);
    let eff_spell_block = effective(spell_block);
    let proj_block_val = get_output_f64(&env.player.output, "ProjectileBlockChance");
    let spell_proj_val = get_output_f64(&env.player.output, "SpellProjectileBlockChance");

    env.player.set_output("EffectiveBlockChance", eff_block);
    env.player
        .set_output("EffectiveSpellBlockChance", eff_spell_block);
    env.player
        .set_output("EffectiveProjectileBlockChance", effective(proj_block_val));
    env.player.set_output(
        "EffectiveSpellProjectileBlockChance",
        effective(spell_proj_val),
    );
    env.player.set_output(
        "EffectiveAverageBlockChance",
        (eff_block + eff_spell_block) / 2.0,
    );
}

// ── Task 5: Primary defences ─────────────────────────────────────────────────

/// Build a SkillCfg that only has a slot_name set (used for per-slot INC/MORE queries).
/// Mirrors the `slotCfg` table in CalcDefence.lua:842 (`local slotCfg = wipeTable(tempTable1)`).
fn slot_cfg(slot_name: &str) -> SkillCfg {
    SkillCfg {
        slot_name: Some(slot_name.to_string()),
        ..Default::default()
    }
}

fn calc_primary_defences(env: &mut CalcEnv) {
    // data.misc.LowPoolThreshold = 0.5 (Data.lua:167)
    const LOW_POOL_THRESHOLD: f64 = 0.5;

    let output = env.player.output.clone();

    // Pre-compute flags needed by the slot loop and global sections.
    let iron_reflexes = env.player.mod_db.flag_cfg("IronReflexes", None, &output);
    let es_to_ward = env
        .player
        .mod_db
        .flag_cfg("EnergyShieldToWard", None, &output);
    let convert_armour_es_to_life =
        env.player
            .mod_db
            .flag_cfg("ConvertArmourESToLife", None, &output);

    // Accumulators — mirrors `local ward, energyShield, armour, evasion = 0, 0, 0, 0`
    // at CalcDefence.lua:827-830.
    let mut ward = 0.0_f64;
    let mut energy_shield = 0.0_f64;
    let mut armour = 0.0_f64;
    let mut evasion = 0.0_f64;

    // Gear trackers for Gear:* output fields (CalcDefence.lua:838-841).
    let mut gear_ward = 0.0_f64;
    let mut gear_energy_shield = 0.0_f64;
    let mut gear_armour = 0.0_f64;
    let mut gear_evasion = 0.0_f64;

    // ── Per-slot gear loop (CalcDefence.lua:843-923) ──────────────────────────
    // Iterates armour slots, queries INC/MORE with slotCfg.slotName = slot so
    // that mods like "100% increased Energy Shield from Body Armour" are scoped
    // correctly.
    //
    // gear_slot_armour was populated by setup.rs instead of adding global BASE
    // mods, so this is the only place those base values are multiplied.
    let gear_slots: Vec<(String, crate::build::types::ItemArmourData)> =
        env.player.gear_slot_armour.clone();

    for (slot, ad) in &gear_slots {
        let cfg = slot_cfg(slot);

        // CalcDefence.lua:847-850 — GainNo{Defence}From{Slot} checks (Gap B).
        let es_base =
            if !env
                .player
                .mod_db
                .flag_cfg(&format!("GainNoEnergyShieldFrom{slot}"), None, &output)
            {
                ad.energy_shield
            } else {
                0.0
            };
        let mut arm_base =
            if !env
                .player
                .mod_db
                .flag_cfg(&format!("GainNoArmourFrom{slot}"), None, &output)
            {
                ad.armour
            } else {
                0.0
            };
        let mut eva_base =
            if !env
                .player
                .mod_db
                .flag_cfg(&format!("GainNoEvasionFrom{slot}"), None, &output)
            {
                ad.evasion
            } else {
                0.0
            };
        let mut ward_base =
            if !env
                .player
                .mod_db
                .flag_cfg(&format!("GainNoWardFrom{slot}"), None, &output)
            {
                ad.ward
            } else {
                0.0
            };

        // CalcDefence.lua:851-858 — Body Armour Armour/Evasion → Ward conversion.
        if slot == "Body Armour"
            && env
                .player
                .mod_db
                .flag_cfg("ConvertBodyArmourArmourEvasionToWard", None, &output)
        {
            let pct = env.player.mod_db.sum_cfg(
                ModType::Base,
                "BodyArmourArmourEvasionToWardPercent",
                None,
                &output,
            ) / 100.0;
            let conversion = pct.min(1.0);
            let converted_armour = arm_base * conversion;
            let converted_evasion = eva_base * conversion;
            arm_base -= converted_armour;
            eva_base -= converted_evasion;
            ward_base += converted_evasion + converted_armour;
        }

        // CalcDefence.lua:859-882 — Ward from slot.
        if ward_base > 0.0 {
            if es_to_ward {
                // EnergyShieldToWard: ward uses INC from "Ward"+"Defences"+"EnergyShield".
                let slot_ward = ward_base
                    * calc_def_mod(
                        &env.player.mod_db,
                        Some(&cfg),
                        &output,
                        &["Ward", "Defences", "EnergyShield"],
                    );
                ward += slot_ward;
            } else {
                let slot_ward = ward_base
                    * calc_def_mod(
                        &env.player.mod_db,
                        Some(&cfg),
                        &output,
                        &["Ward", "Defences"],
                    );
                ward += slot_ward;
            }
            gear_ward += ward_base;
        }

        // CalcDefence.lua:883-903 — ES from slot.
        if es_base > 0.0 {
            if es_to_ward {
                // EnergyShieldToWard: ES contributes only via More (no INC).
                // Lua line 885-886: energyShield += esBase * modDB:More(slotCfg, "EnergyShield", "Defences")
                let more = env
                    .player
                    .mod_db
                    .more_cfg("EnergyShield", Some(&cfg), &output)
                    * env.player.mod_db.more_cfg("Defences", Some(&cfg), &output);
                energy_shield += es_base * more;
            } else if !convert_armour_es_to_life {
                // CalcDefence.lua:898 — slot ES uses "{slot}ESAndArmour" as extra INC/MORE stat (Gap C).
                let slot_es_and_armour = format!("{slot}ESAndArmour");
                let slot_es = es_base
                    * calc_def_mod(
                        &env.player.mod_db,
                        Some(&cfg),
                        &output,
                        &["EnergyShield", "Defences", &slot_es_and_armour],
                    );
                energy_shield += slot_es;
            }
            gear_energy_shield += es_base;
        }

        // CalcDefence.lua:905-910 — Armour from slot.
        // Uses "{slot}ESAndArmour" as extra INC/MORE stat (Gap C).
        if arm_base > 0.0 {
            let slot_es_and_armour = format!("{slot}ESAndArmour");
            let slot_arm = arm_base
                * calc_def_mod(
                    &env.player.mod_db,
                    Some(&cfg),
                    &output,
                    &[
                        "Armour",
                        "ArmourAndEvasion",
                        "Defences",
                        &slot_es_and_armour,
                    ],
                );
            armour += slot_arm;
            gear_armour += arm_base;
        }

        // CalcDefence.lua:912-921 — Evasion from slot.
        // Gap D: Iron Reflexes per-slot conversion uses "Armour" INC/MORE in addition.
        if eva_base > 0.0 {
            gear_evasion += eva_base;
            if iron_reflexes {
                // CalcDefence.lua:917-918: IronReflexes converts per-slot evasion to armour
                // using slot-scoped Armour+Evasion+ArmourAndEvasion+Defences INC/MORE.
                let slot_arm = eva_base
                    * calc_def_mod(
                        &env.player.mod_db,
                        Some(&cfg),
                        &output,
                        &["Armour", "Evasion", "ArmourAndEvasion", "Defences"],
                    );
                armour += slot_arm;
            } else {
                // CalcDefence.lua:919-920
                let slot_eva = eva_base
                    * calc_def_mod(
                        &env.player.mod_db,
                        Some(&cfg),
                        &output,
                        &["Evasion", "ArmourAndEvasion", "Defences"],
                    );
                evasion += slot_eva;
            }
        }
    }

    // ── Global (non-slot) Ward (CalcDefence.lua:925-948) ─────────────────────
    // modDB:Sum("BASE", nil, "Ward") — picks up non-slot Ward sources.
    let global_ward_base = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "Ward", None, &output);
    if global_ward_base > 0.0 {
        if es_to_ward {
            let inc = env
                .player
                .mod_db
                .sum_cfg(ModType::Inc, "Ward", None, &output)
                + env
                    .player
                    .mod_db
                    .sum_cfg(ModType::Inc, "Defences", None, &output)
                + env
                    .player
                    .mod_db
                    .sum_cfg(ModType::Inc, "EnergyShield", None, &output);
            let more = env.player.mod_db.more_cfg("Ward", None, &output)
                * env.player.mod_db.more_cfg("Defences", None, &output);
            ward += global_ward_base * (1.0 + inc / 100.0) * more;
        } else {
            ward += global_ward_base
                * calc_def_mod(&env.player.mod_db, None, &output, &["Ward", "Defences"]);
        }
    }

    // ── Global (non-slot) ES (CalcDefence.lua:949-968) ────────────────────────
    // modDB:Sum("BASE", nil, "EnergyShield") — passives, buffs, flasks, non-slot sources.
    let global_es_base = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "EnergyShield", None, &output);
    if global_es_base > 0.0 {
        if es_to_ward {
            // Lua line 952: energyShield += esBase * modDB:More(nil, "EnergyShield", "Defences")
            let more = env.player.mod_db.more_cfg("EnergyShield", None, &output)
                * env.player.mod_db.more_cfg("Defences", None, &output);
            energy_shield += global_es_base * more;
        } else {
            energy_shield += global_es_base
                * calc_def_mod(
                    &env.player.mod_db,
                    None,
                    &output,
                    &["EnergyShield", "Defences"],
                );
        }
    }

    // ── Global (non-slot) Armour (CalcDefence.lua:969-975) ───────────────────
    let global_arm_base = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "Armour", None, &output)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Base, "ArmourAndEvasion", None, &output);
    if global_arm_base > 0.0 {
        armour += global_arm_base
            * calc_def_mod(
                &env.player.mod_db,
                None,
                &output,
                &["Armour", "ArmourAndEvasion", "Defences"],
            );
    }

    // ── Global (non-slot) Evasion (CalcDefence.lua:976-988) ──────────────────
    let global_eva_base = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "Evasion", None, &output)
        + env
            .player
            .mod_db
            .sum_cfg(ModType::Base, "ArmourAndEvasion", None, &output);
    if global_eva_base > 0.0 {
        if iron_reflexes {
            // Global evasion also converted to armour under Iron Reflexes.
            armour += global_eva_base
                * calc_def_mod(
                    &env.player.mod_db,
                    None,
                    &output,
                    &["Armour", "Evasion", "ArmourAndEvasion", "Defences"],
                );
        } else {
            evasion += global_eva_base
                * calc_def_mod(
                    &env.player.mod_db,
                    None,
                    &output,
                    &["Evasion", "ArmourAndEvasion", "Defences"],
                );
        }
    }

    // ── Mana → Armour conversion (CalcDefence.lua:990-998) ───────────────────
    // Lua: armourBase = 2 * Mana_BASE * convManaToArmour / 100
    //      total = armourBase * calcLib.mod(modDB, nil, "Mana", "Armour", "ArmourAndEvasion", "Defences")
    let conv_mana_to_armour =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ManaConvertToArmour", None, &output);
    if conv_mana_to_armour > 0.0 {
        let mana_base = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, "Mana", None, &output);
        let arm_from_mana_base = 2.0 * mana_base * conv_mana_to_armour / 100.0;
        let arm_from_mana_mult = calc_def_mod(
            &env.player.mod_db,
            None,
            &output,
            &["Mana", "Armour", "ArmourAndEvasion", "Defences"],
        );
        armour += arm_from_mana_base * arm_from_mana_mult;
    }

    // ── Mana → ES conversion (CalcDefence.lua:999-1006) ──────────────────────
    // Lua: energyShieldBase = Mana_BASE * convManaToES / 100
    //      energyShield += esBase * calcLib.mod(modDB, nil, "Mana", "EnergyShield", "Defences")
    let conv_mana_to_es =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ManaGainAsEnergyShield", None, &output);
    if conv_mana_to_es > 0.0 {
        let mana_base = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, "Mana", None, &output);
        let es_from_mana_base = mana_base * conv_mana_to_es / 100.0;
        let es_from_mana_mult = calc_def_mod(
            &env.player.mod_db,
            None,
            &output,
            &["Mana", "EnergyShield", "Defences"],
        );
        energy_shield += es_from_mana_base * es_from_mana_mult;
    }

    // ── Life → Armour conversion (CalcDefence.lua:1007-1020) ─────────────────
    // Lua: convLifeToArmour = sum("LifeGainAsArmour")
    //      if CI: total = 1 else: total = Life_BASE * conv/100 * calcLib.mod(...)
    let conv_life_to_armour =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "LifeGainAsArmour", None, &output);
    if conv_life_to_armour > 0.0 {
        let arm_from_life = if env
            .player
            .mod_db
            .flag_cfg("ChaosInoculation", None, &output)
        {
            1.0
        } else {
            let life_base = env
                .player
                .mod_db
                .sum_cfg(ModType::Base, "Life", None, &output);
            let arm_from_life_base = life_base * conv_life_to_armour / 100.0;
            let arm_from_life_mult = calc_def_mod(
                &env.player.mod_db,
                None,
                &output,
                &["Life", "Armour", "ArmourAndEvasion", "Defences"],
            );
            arm_from_life_base * arm_from_life_mult
        };
        armour += arm_from_life;
    }

    // ── Life → ES conversion (CalcDefence.lua:1021-1034) ─────────────────────
    // Lua: convLifeToES = sum("LifeConvertToEnergyShield") + sum("LifeGainAsEnergyShield")
    //      if CI: total = 1
    //      else: total = Life_BASE * convLifeToES / 100 * calcLib.mod("Life","EnergyShield","Defences")
    let conv_life_to_es =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "LifeConvertToEnergyShield", None, &output)
            + env
                .player
                .mod_db
                .sum_cfg(ModType::Base, "LifeGainAsEnergyShield", None, &output);
    if conv_life_to_es > 0.0 {
        let life_es_total = if env
            .player
            .mod_db
            .flag_cfg("ChaosInoculation", None, &output)
        {
            1.0 // CI: life is 1
        } else {
            let life_base = env
                .player
                .mod_db
                .sum_cfg(ModType::Base, "Life", None, &output);
            let es_from_life_base = life_base * conv_life_to_es / 100.0;
            let es_from_life_mult = calc_def_mod(
                &env.player.mod_db,
                None,
                &output,
                &["Life", "EnergyShield", "Defences"],
            );
            es_from_life_base * es_from_life_mult
        };
        energy_shield += life_es_total;
    }

    // ── Evasion → Armour conversion (CalcDefence.lua:1035-1043) ──────────────
    // Gap F: Lua line 1037 uses `(modDB:Sum("BASE", nil, "Evasion", "ArmourAndEvasion") + gearEvasion)`
    // as the base for this conversion.  gearEvasion is the sum of raw evasion from gear slots
    // BEFORE per-slot INC/MORE was applied — it is NOT the same as the global_eva_base sum.
    // In Rust, global_eva_base is the sum of non-slot Evasion+ArmourAndEvasion BASE mods
    // (passives, buffs, etc.), and gear_evasion tracks the raw gear base values.
    let conv_evasion_to_armour =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "EvasionGainAsArmour", None, &output);
    if conv_evasion_to_armour > 0.0 {
        let arm_from_eva_base = (global_eva_base + gear_evasion) * conv_evasion_to_armour / 100.0;
        let arm_from_eva_mult = calc_def_mod(
            &env.player.mod_db,
            None,
            &output,
            &["Evasion", "Armour", "ArmourAndEvasion", "Defences"],
        );
        armour += arm_from_eva_base * arm_from_eva_mult;
    }

    // ── Final output (CalcDefence.lua:1044-1054) ──────────────────────────────
    let energy_shield = env
        .player
        .mod_db
        .override_value("EnergyShield", None, &output)
        .unwrap_or_else(|| energy_shield.round().max(0.0));
    let armour = armour.round().max(0.0);
    let evasion = evasion.round().max(0.0);
    let ward = ward.floor().max(0.0);

    env.player.set_output("EnergyShield", energy_shield);
    env.player.set_output("Armour", armour);
    env.player.set_output("Evasion", evasion);
    env.player.set_output("Ward", ward);

    // MeleeEvasion and ProjectileEvasion (CalcDefence.lua:1047-1048)
    let melee_eva_mult = calc_def_mod(&env.player.mod_db, None, &output, &["MeleeEvasion"]);
    let proj_eva_mult = calc_def_mod(&env.player.mod_db, None, &output, &["ProjectileEvasion"]);
    env.player
        .set_output("MeleeEvasion", (evasion * melee_eva_mult).round().max(0.0));
    env.player.set_output(
        "ProjectileEvasion",
        (evasion * proj_eva_mult).round().max(0.0),
    );

    env.player
        .set_output("LowestOfArmourAndEvasion", armour.min(evasion));

    // Gap E: Gear:* output fields (CalcDefence.lua:1051-1054).
    env.player.set_output("Gear:Ward", gear_ward);
    env.player
        .set_output("Gear:EnergyShield", gear_energy_shield);
    env.player.set_output("Gear:Armour", gear_armour);
    env.player.set_output("Gear:Evasion", gear_evasion);

    // ── EnergyShieldRecoveryCap (CalcDefence.lua:1055-1062) ───────────────────
    // CappingES: true when ArmourESRecoveryCap or EvasionESRecoveryCap flag is set
    //            AND the respective defence is less than ES,
    //            OR the "conditionLowEnergyShield" config checkbox is set.
    let armour_es_cap = env
        .player
        .mod_db
        .flag_cfg("ArmourESRecoveryCap", None, &output);
    let evasion_es_cap = env
        .player
        .mod_db
        .flag_cfg("EvasionESRecoveryCap", None, &output);
    let condition_low_es = env
        .player
        .mod_db
        .flag_cfg("conditionLowEnergyShield", None, &output);

    let capping_es = (armour_es_cap && armour < energy_shield)
        || (evasion_es_cap && evasion < energy_shield)
        || condition_low_es;

    let es_recover_cap = if capping_es {
        // Priority: both flags → min(Armour, Evasion)
        //           only ArmourESRecoveryCap → Armour
        //           only EvasionESRecoveryCap → Evasion
        //           neither (only conditionLowES) → EnergyShield
        let cap = if armour_es_cap && evasion_es_cap {
            armour.min(evasion)
        } else if armour_es_cap {
            armour
        } else if evasion_es_cap {
            evasion
        } else {
            energy_shield
        };
        // Additional cap from conditionLowEnergyShield: min(ES * 0.5, cap)
        if condition_low_es {
            (energy_shield * LOW_POOL_THRESHOLD).min(cap)
        } else {
            cap
        }
    } else {
        energy_shield
    };
    env.player
        .set_output("EnergyShieldRecoveryCap", es_recover_cap);

    // ── Evade chance ──────────────────────────────────────────────────────────
    let evade_chance = if env.player.mod_db.flag_cfg("CannotEvade", None, &output)
        || env.enemy.mod_db.flag_cfg("CannotBeEvaded", None, &output)
    {
        0.0
    } else if env.player.mod_db.flag_cfg("AlwaysEvade", None, &output) {
        100.0
    } else {
        let enemy_accuracy = get_output_f64(&env.enemy.output, "Accuracy");
        let acc = if enemy_accuracy > 0.0 {
            enemy_accuracy
        } else {
            500.0
        };
        100.0 - hit_chance(evasion, acc)
    };
    env.player.set_output("EvadeChance", evade_chance);
    env.player.set_output("MeleeEvadeChance", evade_chance);
    env.player.set_output("ProjectileEvadeChance", evade_chance);
}

// ── Task 6: Spell suppression + dodge ────────────────────────────────────────

fn calc_spell_suppression(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    let base = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "SpellSuppressionChance", None, &output);
    let inc = env
        .player
        .mod_db
        .sum_cfg(ModType::Inc, "SpellSuppressionChance", None, &output);
    let chance = (base * (1.0 + inc / 100.0)).clamp(0.0, 100.0);
    env.player.set_output("SpellSuppressionChance", chance);

    // Suppression effect (default 50%)
    let effect_base =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "SpellSuppressionEffect", None, &output);
    let effect = if effect_base == 0.0 {
        50.0
    } else {
        effect_base
    };
    env.player.set_output("SpellSuppressionEffect", effect);

    // Lucky/unlucky suppression
    let lucky = env
        .player
        .mod_db
        .flag_cfg("LuckySuppression", None, &output);
    let unlucky = env
        .player
        .mod_db
        .flag_cfg("UnluckySuppression", None, &output);
    let x = chance / 100.0;
    let eff = if lucky {
        (1.0 - (1.0 - x) * (1.0 - x)) * 100.0
    } else if unlucky {
        x * x * 100.0
    } else {
        chance
    };
    env.player
        .set_output("EffectiveSpellSuppressionChance", eff);
}

fn calc_dodge(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    let attack_dodge = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "AttackDodgeChance", None, &output)
        .clamp(0.0, 75.0);
    env.player.set_output("AttackDodgeChance", attack_dodge);

    let spell_dodge = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "SpellDodgeChance", None, &output)
        .clamp(0.0, 75.0);
    env.player.set_output("SpellDodgeChance", spell_dodge);
}

// ── Task 7: Damage reduction ─────────────────────────────────────────────────

fn calc_damage_reduction(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    let dr_max_base = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "DamageReductionMax", None, &output);
    let dr_max = if dr_max_base == 0.0 {
        90.0
    } else {
        dr_max_base
    };
    env.player.set_output("DamageReductionMax", dr_max);

    for type_name in DMG_TYPE_NAMES.iter() {
        let stat = format!("Base{type_name}DamageReduction");
        let val = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, &stat, None, &output);
        env.player.set_output(&stat, val);

        let stat_hit = format!("Base{type_name}DamageReductionWhenHit");
        let val_hit = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, &stat_hit, None, &output);
        env.player.set_output(&stat_hit, val_hit);
    }
}

// ── Task 8: Recovery, leech, regen, ES recharge ──────────────────────────────

fn calc_recovery_rates(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    for resource in &["Life", "Mana", "EnergyShield"] {
        // The mod stat name is "{resource}RecoveryRate" (e.g. "LifeRecoveryRate").
        // The output field name is "{resource}RecoveryRateMod".
        // Lua: calcLib.mod(modDB, nil, "LifeRecoveryRate") == (1 + INC/100) * More
        // CalcDefence.lua:1194-1197
        let mod_stat = format!("{resource}RecoveryRate");
        let output_stat = format!("{resource}RecoveryRateMod");
        let inc = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, &mod_stat, None, &output);
        let more = env.player.mod_db.more_cfg(&mod_stat, None, &output);
        let rate = (1.0 + inc / 100.0) * more;
        env.player.set_output(&output_stat, rate);
    }
}

fn calc_leech_caps(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    let life = get_output_f64(&output, "Life").max(1.0);
    let mana = get_output_f64(&output, "Mana").max(1.0);
    let es = get_output_f64(&output, "EnergyShield").max(0.0);

    // Leech caps: moddb Base values are percentages of pool.
    // Defaults come from game_constants in add_base_constants:
    //   MaxLifeLeechRate = 20 (% of life), MaxLifeLeechInstance = 10 (% of life)
    //   MaxManaLeechRate = 20 (% of mana), MaxManaLeechInstance = 10 (% of mana)
    //   MaxEnergyShieldLeechInstance = 10 (% of ES)

    // Max life leech instance (% of life → absolute)
    let life_leech_instance_pct =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MaxLifeLeechInstance", None, &output);
    let life_leech_instance_pct = if life_leech_instance_pct > 0.0 {
        life_leech_instance_pct
    } else {
        10.0
    };
    let life_leech_instance = life * life_leech_instance_pct / 100.0;
    env.player
        .set_output("MaxLifeLeechInstance", life_leech_instance);

    // Max life leech rate (% of life → absolute)
    let life_leech_rate_pct =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MaxLifeLeechRate", None, &output);
    let life_leech_rate_pct = if life_leech_rate_pct > 0.0 {
        life_leech_rate_pct
    } else {
        20.0
    };
    let life_leech_rate = life * life_leech_rate_pct / 100.0;
    env.player.set_output("MaxLifeLeechRate", life_leech_rate);
    env.player
        .set_output("MaxLifeLeechRatePercent", life_leech_rate_pct);

    // Max ES leech instance (% of ES → absolute)
    let es_leech_instance_pct =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MaxEnergyShieldLeechInstance", None, &output);
    let es_leech_instance_pct = if es_leech_instance_pct > 0.0 {
        es_leech_instance_pct
    } else {
        10.0
    };
    let es_leech_instance = es.max(1.0) * es_leech_instance_pct / 100.0;
    env.player
        .set_output("MaxEnergyShieldLeechInstance", es_leech_instance);

    // Max ES leech rate (% of ES → absolute). Default: same as leech rate for ES
    let es_leech_rate_pct =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MaxEnergyShieldLeechRate", None, &output);
    let es_leech_rate_pct = if es_leech_rate_pct > 0.0 {
        es_leech_rate_pct
    } else {
        10.0
    };
    let es_leech_rate = es.max(1.0) * es_leech_rate_pct / 100.0;
    env.player
        .set_output("MaxEnergyShieldLeechRate", es_leech_rate);

    // Max mana leech instance (% of mana → absolute)
    let mana_leech_instance_pct =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MaxManaLeechInstance", None, &output);
    let mana_leech_instance_pct = if mana_leech_instance_pct > 0.0 {
        mana_leech_instance_pct
    } else {
        10.0
    };
    let mana_leech_instance = mana * mana_leech_instance_pct / 100.0;
    env.player
        .set_output("MaxManaLeechInstance", mana_leech_instance);

    // Max mana leech rate (% of mana → absolute)
    let mana_leech_rate_pct =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MaxManaLeechRate", None, &output);
    let mana_leech_rate_pct = if mana_leech_rate_pct > 0.0 {
        mana_leech_rate_pct
    } else {
        20.0
    };
    let mana_leech_rate = mana * mana_leech_rate_pct / 100.0;
    env.player.set_output("MaxManaLeechRate", mana_leech_rate);
}

fn calc_regeneration(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    let zealots_oath = env.player.mod_db.flag_cfg("ZealotsOath", None, &output);

    for resource in &["Life", "Mana", "EnergyShield"] {
        let pool = get_output_f64(&output, resource).max(1.0);
        let recovery_stat = format!("{resource}RecoveryRateMod");
        let recovery_rate = get_output_f64(&output, &recovery_stat).max(1.0);

        let regen_stat = format!("{resource}Regen");
        let percent_stat = format!("{resource}RegenPercent");

        let percent = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, &percent_stat, None, &output);
        let flat = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, &regen_stat, None, &output);
        let inc = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, &regen_stat, None, &output);

        let regen = (pool * percent / 100.0 + flat) * (1.0 + inc / 100.0) * recovery_rate;
        env.player.set_output(&regen_stat, regen);
    }

    // Zealot's Oath: life regen applies to ES instead
    if zealots_oath {
        let life_regen = get_output_f64(&env.player.output, "LifeRegen");
        let es_regen = get_output_f64(&env.player.output, "EnergyShieldRegen");
        env.player
            .set_output("EnergyShieldRegen", es_regen + life_regen);
        env.player.set_output("LifeRegen", 0.0);
    }
}

fn calc_es_recharge(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // CalcDefence.lua:1322-1369
    // output.EnergyShieldRechargeAppliesToLife = flag(EnergyShieldRechargeAppliesToLife)
    //   and not flag(CannotRecoverLifeOutsideLeech)
    // output.EnergyShieldRechargeAppliesToEnergyShield = not (flag(NoEnergyShieldRecharge)
    //   or flag(CannotGainEnergyShield) or EnergyShieldRechargeAppliesToLife)
    let applies_to_life =
        env.player
            .mod_db
            .flag_cfg("EnergyShieldRechargeAppliesToLife", None, &output)
            && !env
                .player
                .mod_db
                .flag_cfg("CannotRecoverLifeOutsideLeech", None, &output);
    let applies_to_es = !(env
        .player
        .mod_db
        .flag_cfg("NoEnergyShieldRecharge", None, &output)
        || env
            .player
            .mod_db
            .flag_cfg("CannotGainEnergyShield", None, &output)
        || applies_to_life);

    env.player
        .set_output_bool("EnergyShieldRechargeAppliesToLife", applies_to_life);
    env.player
        .set_output_bool("EnergyShieldRechargeAppliesToEnergyShield", applies_to_es);

    if applies_to_life || applies_to_es {
        // Inc/More mod names are "EnergyShieldRecharge" (NOT "EnergyShieldRechargeRate").
        // CalcDefence.lua:1327-1328
        let inc = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "EnergyShieldRecharge", None, &output);
        let more = env
            .player
            .mod_db
            .more_cfg("EnergyShieldRecharge", None, &output);

        // base = modDB:Override(nil, "EnergyShieldRecharge") or data.misc.EnergyShieldRechargeBase
        // EnergyShieldRechargeBase = character_constants["energy_shield_recharge_rate_per_minute_%"] / 60 / 100
        // CalcDefence.lua:1329 / Data.lua:182-183
        let base = env
            .player
            .mod_db
            .override_value("EnergyShieldRecharge", None, &output)
            .unwrap_or_else(|| {
                env.data
                    .misc
                    .character_constants
                    .get("energy_shield_recharge_rate_per_minute_%")
                    .copied()
                    .unwrap_or(2000.0)
                    / 60.0
                    / 100.0
            });

        if applies_to_life {
            // output.LifeRecharge = round(Life * base * (1+inc/100) * more * LifeRecoveryRateMod)
            // CalcDefence.lua:1331-1332
            let life = get_output_f64(&output, "Life");
            let life_recovery_rate = get_output_f64(&output, "LifeRecoveryRateMod").max(1.0);
            let recharge = life * base * (1.0 + inc / 100.0) * more;
            let life_recharge = (recharge * life_recovery_rate).round();
            env.player.set_output("LifeRecharge", life_recharge);
        } else {
            // output.EnergyShieldRecharge = round(EnergyShield * base * (1+inc/100) * more
            //   * EnergyShieldRecoveryRateMod)
            // CalcDefence.lua:1350-1351
            let es = get_output_f64(&output, "EnergyShield").max(0.0);
            let es_recovery_rate = get_output_f64(&output, "EnergyShieldRecoveryRateMod").max(1.0);
            let recharge = es * base * (1.0 + inc / 100.0) * more;
            let es_recharge = (recharge * es_recovery_rate).round();
            env.player.set_output("EnergyShieldRecharge", es_recharge);
        }

        // ES recharge delay: data.misc.EnergyShieldRechargeDelay / (1 + faster/100)
        // CalcDefence.lua:1369 — EnergyShieldRechargeDelay = 2 (from Data.lua:184)
        let faster =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, "EnergyShieldRechargeFaster", None, &output);
        let delay = 2.0 / (1.0 + faster / 100.0);
        env.player.set_output("EnergyShieldRechargeDelay", delay);
    }
}

// ── Task 9: Movement speed, avoidance, misc ──────────────────────────────────

fn calc_movement_and_avoidance(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // Movement speed
    let ms_inc = env
        .player
        .mod_db
        .sum_cfg(ModType::Inc, "MovementSpeed", None, &output);
    let ms_more = env.player.mod_db.more_cfg("MovementSpeed", None, &output);
    let ms = (1.0 + ms_inc / 100.0) * ms_more;
    env.player.set_output("MovementSpeedMod", ms);

    let action_speed = env.player.action_speed_mod;
    env.player
        .set_output("EffectiveMovementSpeedMod", ms * action_speed);

    // Life/Mana/ES on block
    for resource in &["Life", "Mana", "EnergyShield"] {
        let stat = format!("{resource}OnBlock");
        let val = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, &stat, None, &output);
        env.player.set_output(&stat, val);
    }

    // Life/Mana/ES on suppress — computed but not output (PoB doesn't output these directly)

    // Ailment avoidance
    let elemental_ailments = [
        "Ignite", "Shock", "Freeze", "Chill", "Scorch", "Brittle", "Sap",
    ];
    let non_elemental_ailments = ["Bleed", "Poison", "Impale"];
    let all_ailments: Vec<&str> = elemental_ailments
        .iter()
        .chain(non_elemental_ailments.iter())
        .copied()
        .collect();

    let elemental_avoid =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "ElementalAilmentAvoidance", None, &output);

    for ailment in &all_ailments {
        let mod_stat = format!("Avoid{ailment}");
        let base = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, &mod_stat, None, &output);
        let extra = if elemental_ailments.contains(ailment) {
            elemental_avoid
        } else {
            0.0
        };
        let val = (base + extra).clamp(0.0, 100.0);
        // PoB output key is "{Ailment}AvoidChance"
        let output_key = format!("{ailment}AvoidChance");
        env.player.set_output(&output_key, val);
    }

    // Blind, Silence, InterruptStun avoidance
    let blind_avoid = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "AvoidBlind", None, &output)
        .clamp(0.0, 100.0);
    env.player.set_output("BlindAvoidChance", blind_avoid);

    let silence_avoid = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "AvoidSilence", None, &output)
        .clamp(0.0, 100.0);
    env.player.set_output("SilenceAvoidChance", silence_avoid);

    let interrupt_stun_avoid = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "AvoidInterruptStun", None, &output)
        .clamp(0.0, 100.0);
    env.player
        .set_output("InterruptStunAvoidChance", interrupt_stun_avoid);

    // Per-damage-type avoidance
    for type_name in DMG_TYPE_NAMES.iter() {
        let mod_stat = format!("Avoid{type_name}Damage");
        let val = env
            .player
            .mod_db
            .sum_cfg(ModType::Base, &mod_stat, None, &output)
            .clamp(0.0, 100.0);
        // PoB output key is "Avoid{Type}DamageChance"
        let output_key = format!("Avoid{type_name}DamageChance");
        env.player.set_output(&output_key, val);
    }

    // All damage from hits avoidance
    let avoid_all_hits = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "AvoidAllDamageFromHits", None, &output)
        .clamp(0.0, 100.0);
    env.player
        .set_output("AvoidAllDamageFromHitsChance", avoid_all_hits);

    // Projectile avoidance
    let avoid_proj = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "AvoidProjectiles", None, &output)
        .clamp(0.0, 100.0);
    env.player.set_output("AvoidProjectilesChance", avoid_proj);

    // Curse avoidance
    let curse_avoid = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "AvoidCurse", None, &output)
        .clamp(0.0, 100.0);
    env.player.set_output("CurseAvoidChance", curse_avoid);

    // Stun avoidance (separate from stun calc, this is the avoidance stat)
    // Handled in calc_stun

    // Immunities — set conditions but don't output (PoB doesn't output these)
    let cb_immune = env
        .player
        .mod_db
        .flag_cfg("CorruptedBloodImmunity", None, &output);
    if cb_immune {
        env.player
            .mod_db
            .set_condition("CorruptedBloodImmunity", true);
    }

    let maim_immune = env.player.mod_db.flag_cfg("MaimImmunity", None, &output);
    if maim_immune {
        env.player.mod_db.set_condition("MaimImmunity", true);
    }

    let hinder_immune = env.player.mod_db.flag_cfg("HinderImmunity", None, &output);
    if hinder_immune {
        env.player.mod_db.set_condition("HinderImmunity", true);
    }

    // Crit extra damage reduction
    let crit_dr =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "CritExtraDamageReduction", None, &output);
    env.player.set_output("CritExtraDamageReduction", crit_dr);

    // Debuff expiration rate: PoB outputs the raw inc% (0 = no change)
    let debuff_rate_inc =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "DebuffExpirationRate", None, &output);
    env.player
        .set_output("DebuffExpirationRate", debuff_rate_inc);

    // DebuffExpirationModifier: 100 + inc  (100 = base)
    let debuff_modifier = 100.0 + debuff_rate_inc;
    env.player
        .set_output("DebuffExpirationModifier", debuff_modifier);

    // showDebuffExpirationModifier: true if modifier != 100
    env.player
        .set_output_bool("showDebuffExpirationModifier", debuff_rate_inc != 0.0);
}

// ── Task 10: Damage shift table ──────────────────────────────────────────────

fn build_damage_shift_table(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // Reset to identity
    for src in 0..5 {
        for dst in 0..5 {
            env.player.damage_shift_table[src][dst] = if src == dst { 100.0 } else { 0.0 };
        }
    }

    // Build from {Source}DamageTakenAs{Dest} mods
    for (src, src_name) in DMG_TYPE_NAMES.iter().enumerate() {
        let mut total_shifted: f64 = 0.0;

        for (dst, dst_name) in DMG_TYPE_NAMES.iter().enumerate() {
            if src == dst {
                continue;
            }

            let stat = format!("{src_name}DamageTakenAs{dst_name}");
            let mut shift = env
                .player
                .mod_db
                .sum_cfg(ModType::Base, &stat, None, &output);

            // ElementalDamageTakenAs{Dest} applies to Fire/Cold/Lightning sources
            let is_elemental = src == DMG_LIGHTNING || src == DMG_COLD || src == DMG_FIRE;
            if is_elemental {
                let elem_stat = format!("ElementalDamageTakenAs{dst_name}");
                shift += env
                    .player
                    .mod_db
                    .sum_cfg(ModType::Base, &elem_stat, None, &output);
            }

            if shift != 0.0 {
                env.player.damage_shift_table[src][dst] = shift;
                total_shifted += shift;
            }
        }

        // Remaining stays as original type
        env.player.damage_shift_table[src][src] = (100.0 - total_shifted).max(0.0);
    }
}

// ── Task 11: Stun ────────────────────────────────────────────────────────────

fn calc_stun(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    let stun_immune = env.player.mod_db.flag_cfg("StunImmune", None, &output);

    if stun_immune {
        env.player.set_output("StunThreshold", f64::INFINITY);
        env.player.set_output("AvoidStun", 100.0);
        env.player.set_output("StunDuration", 0.0);
        return;
    }

    // Stun threshold pool
    let pool = if env
        .player
        .mod_db
        .flag_cfg("StunThresholdBasedOnManaInsteadOfLife", None, &output)
    {
        get_output_f64(&output, "Mana").max(1.0)
    } else if env.player.mod_db.flag_cfg(
        "StunThresholdBasedOnEnergyShieldInsteadOfLife",
        None,
        &output,
    ) {
        get_output_f64(&output, "EnergyShield").max(1.0)
    } else {
        let mut p = get_output_f64(&output, "Life").max(1.0);
        // CI + AddESToStunThreshold
        let ci = env
            .player
            .mod_db
            .flag_cfg("ChaosInoculation", None, &output);
        let add_es = env
            .player
            .mod_db
            .flag_cfg("AddESToStunThreshold", None, &output);
        if ci && add_es {
            p += get_output_f64(&output, "EnergyShield");
        }
        p
    };

    let inc = env
        .player
        .mod_db
        .sum_cfg(ModType::Inc, "StunThreshold", None, &output);
    let threshold = pool * 0.5 * (1.0 + inc / 100.0);
    env.player.set_output("StunThreshold", threshold);

    // Stun avoidance
    let avoid = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "AvoidStun", None, &output)
        .clamp(0.0, 100.0);
    env.player.set_output("StunAvoidChance", avoid);

    // Stun duration: 0.35 / (1 + recovery_inc/100) * (1 + duration_inc/100)
    let recovery_inc = env
        .player
        .mod_db
        .sum_cfg(ModType::Inc, "StunRecovery", None, &output);
    let duration_inc = env
        .player
        .mod_db
        .sum_cfg(ModType::Inc, "StunDuration", None, &output);
    let stun_duration = 0.35 / (1.0 + recovery_inc / 100.0) * (1.0 + duration_inc / 100.0);
    env.player.set_output("StunDuration", stun_duration);
}

// ── LifeRecoverable (CalcDefence.lua:2204-2218) ───────────────────────────────

/// Compute LifeRecoverable: the amount of life that can actually be recovered.
/// Mirrors CalcDefence.lua lines 2204-2218.
fn calc_life_recoverable(env: &mut CalcEnv) {
    // data.misc.LowPoolThreshold = 0.5 (Data.lua:167)
    const LOW_POOL_THRESHOLD: f64 = 0.5;

    let output = env.player.output.clone();
    let life = get_output_f64(&output, "Life");
    let life_unreserved = get_output_f64(&output, "LifeUnreserved");

    // Default: equal to unreserved life.
    let mut life_recoverable = life_unreserved;

    // conditionLowLife: simulates being perpetually at low life.
    // In Rust, this is exposed as a mod_db flag "conditionLowLife".
    if env
        .player
        .mod_db
        .flag_cfg("conditionLowLife", None, &output)
    {
        // LowLifePercentage is stored in output as 100*fraction (e.g. 50.0 for 50%).
        // Divide by 100 to get the fraction back.
        let low_life_pct = get_output_f64(&output, "LowLifePercentage");
        let threshold_fraction = if low_life_pct > 0.0 {
            low_life_pct / 100.0
        } else {
            LOW_POOL_THRESHOLD
        };
        let cap = life * threshold_fraction;
        life_recoverable = cap.min(life_unreserved);
        if life_recoverable < life_unreserved {
            env.player.set_output_bool("CappingLife", true);
        }
    }

    // Dissolution of the Flesh: life recovery based on cancellable reservation.
    // Lua: output.LifeRecoverable = (output.LifeCancellableReservation / 100) * output.Life
    if env
        .player
        .mod_db
        .flag_cfg("DamageInsteadReservesLife", None, &output)
    {
        let cancellable = get_output_f64(&output, "LifeCancellableReservation");
        life_recoverable = (cancellable / 100.0) * life;
    }

    // Always at least 1 to prevent division-by-zero in EHP calculations.
    life_recoverable = life_recoverable.max(1.0);
    env.player.set_output("LifeRecoverable", life_recoverable);
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        calc::env::CalcEnv,
        data::GameData,
        mod_db::{
            types::{KeywordFlags, Mod, ModFlags, ModSource, ModType, ModValue},
            ModDb,
        },
    };
    use std::sync::Arc;

    fn src() -> ModSource {
        ModSource::new("Test", "test")
    }

    fn make_env_with_mods(mods: Vec<Mod>) -> CalcEnv {
        let mut db = ModDb::new();
        // Default max resists
        db.add(Mod::new_base("FireResistMax", 75.0, src()));
        db.add(Mod::new_base("ColdResistMax", 75.0, src()));
        db.add(Mod::new_base("LightningResistMax", 75.0, src()));
        db.add(Mod::new_base("ChaosResistMax", 75.0, src()));
        // Default block max
        db.add(Mod::new_base("BlockChanceMax", 75.0, src()));
        for m in mods {
            db.add(m);
        }
        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        CalcEnv::new(db, ModDb::new(), Arc::new(game_data))
    }

    // ── Utility function tests ───────────────────────────────────────────

    #[test]
    fn hit_chance_formula() {
        // accuracy=1000, evasion=1000: 1000 / (1000 + (200)^0.9) * 125
        let result = hit_chance(1000.0, 1000.0);
        // (1000/5)^0.9 = 200^0.9 ≈ 148.7
        // 1000 / (1000 + 148.7) * 125 ≈ 108.8 → clamped to 100
        // Actually let's compute: 200^0.9
        let evasion_term = (1000.0_f64 / 5.0).powf(0.9);
        let expected = (1000.0 / (1000.0 + evasion_term) * 125.0).clamp(5.0, 100.0);
        assert!(
            (result - expected).abs() < 0.01,
            "got {result}, expected {expected}"
        );
    }

    #[test]
    fn hit_chance_clamped() {
        // Very high accuracy → clamped to 100
        assert_eq!(hit_chance(100.0, 100000.0), 100.0);
        // Zero accuracy → clamped to 5
        assert_eq!(hit_chance(100.0, 0.0), 5.0);
        // Zero evasion → 100
        assert_eq!(hit_chance(0.0, 100.0), 100.0);
    }

    #[test]
    fn armour_reduction_formula() {
        // 10000 armour, 1000 raw → 10000 / (10000 + 5000) * 100 = 66.67
        let result = armour_reduction_f(10000.0, 1000.0);
        let expected = 10000.0 / (10000.0 + 5000.0) * 100.0;
        assert!(
            (result - expected).abs() < 0.01,
            "got {result}, expected {expected}"
        );
        // Floor version
        assert_eq!(armour_reduction(10000.0, 1000.0), 66.0);
    }

    // ── Resistance tests ─────────────────────────────────────────────────

    #[test]
    fn resistance_overcap_tracked() {
        let mut env = make_env_with_mods(vec![Mod::new_base("FireResist", 120.0, src())]);
        run(&mut env);
        assert_eq!(get_output_f64(&env.player.output, "FireResist"), 75.0);
        assert_eq!(get_output_f64(&env.player.output, "FireResistTotal"), 120.0);
        assert_eq!(
            get_output_f64(&env.player.output, "FireResistOverCap"),
            45.0
        );
    }

    #[test]
    fn max_resist_increases_cap() {
        let mut env = make_env_with_mods(vec![
            Mod::new_base("FireResist", 80.0, src()),
            Mod::new_base("FireResistMax", 5.0, src()), // adds to the 75 base
        ]);
        run(&mut env);
        // Max is 75 + 5 = 80, resist is 80, capped at 80
        assert_eq!(get_output_f64(&env.player.output, "FireResist"), 80.0);
    }

    #[test]
    fn melding_equalizes_max() {
        let mut env = make_env_with_mods(vec![
            Mod::new_base("FireResistMax", 5.0, src()), // 75 + 5 = 80
            Mod::new_flag("ElementalResistMaxIsHighestResistMax", src()),
            Mod::new_base("FireResist", 80.0, src()),
            Mod::new_base("ColdResist", 80.0, src()),
            Mod::new_base("LightningResist", 80.0, src()),
        ]);
        run(&mut env);
        // All elemental max should be 80 (highest)
        assert_eq!(get_output_f64(&env.player.output, "FireResist"), 80.0);
        assert_eq!(get_output_f64(&env.player.output, "ColdResist"), 80.0);
        assert_eq!(get_output_f64(&env.player.output, "LightningResist"), 80.0);
    }

    #[test]
    fn physical_resist_always_zero() {
        let mut env = make_env_with_mods(vec![]);
        run(&mut env);
        assert_eq!(get_output_f64(&env.player.output, "PhysicalResist"), 0.0);
    }

    // ── Block tests ──────────────────────────────────────────────────────

    #[test]
    fn block_chance_capped() {
        let mut env = make_env_with_mods(vec![Mod::new_base("BlockChance", 90.0, src())]);
        run(&mut env);
        assert_eq!(get_output_f64(&env.player.output, "BlockChance"), 75.0);
    }

    #[test]
    fn spell_block_computed() {
        let mut env = make_env_with_mods(vec![Mod::new_base("SpellBlockChance", 40.0, src())]);
        run(&mut env);
        assert_eq!(get_output_f64(&env.player.output, "SpellBlockChance"), 40.0);
    }

    #[test]
    fn glancing_blows_effect() {
        let mut env = make_env_with_mods(vec![
            Mod::new_flag("GlancingBlows", src()),
            Mod::new_base("BlockChance", 50.0, src()),
        ]);
        run(&mut env);
        assert_eq!(get_output_f64(&env.player.output, "BlockEffect"), 65.0);
        assert_eq!(
            get_output_f64(&env.player.output, "DamageTakenOnBlock"),
            35.0
        );
    }

    // ── Primary defence tests ────────────────────────────────────────────

    #[test]
    fn armour_with_iron_reflexes() {
        let mut env = make_env_with_mods(vec![
            Mod::new_base("Armour", 1000.0, src()),
            Mod::new_base("Evasion", 2000.0, src()),
            Mod::new_flag("IronReflexes", src()),
        ]);
        run(&mut env);
        assert_eq!(get_output_f64(&env.player.output, "Armour"), 3000.0);
        assert_eq!(get_output_f64(&env.player.output, "Evasion"), 0.0);
    }

    // ── Spell suppression tests ──────────────────────────────────────────

    #[test]
    fn spell_suppression_capped_at_100() {
        let mut env =
            make_env_with_mods(vec![Mod::new_base("SpellSuppressionChance", 150.0, src())]);
        run(&mut env);
        assert_eq!(
            get_output_f64(&env.player.output, "SpellSuppressionChance"),
            100.0
        );
    }

    // ── Movement speed tests ─────────────────────────────────────────────

    #[test]
    fn movement_speed_computed() {
        let mut env = make_env_with_mods(vec![Mod {
            name: "MovementSpeed".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(30.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![],
            source: src(),
        }]);
        run(&mut env);
        let ms = get_output_f64(&env.player.output, "MovementSpeedMod");
        assert!((ms - 1.30).abs() < 0.01, "expected ~1.30, got {ms}");
    }

    // ── Damage reduction tests ───────────────────────────────────────────

    #[test]
    fn damage_reduction_max_default_90() {
        let mut env = make_env_with_mods(vec![]);
        run(&mut env);
        assert_eq!(
            get_output_f64(&env.player.output, "DamageReductionMax"),
            90.0
        );
    }

    // ── Damage shift table tests ─────────────────────────────────────────

    #[test]
    fn damage_shift_phys_taken_as_fire() {
        let mut env = make_env_with_mods(vec![Mod::new_base(
            "PhysicalDamageTakenAsFire",
            25.0,
            src(),
        )]);
        run(&mut env);
        // Physical → Fire should be 25%, Physical → Physical should be 75%
        assert_eq!(env.player.damage_shift_table[DMG_PHYSICAL][DMG_FIRE], 25.0);
        assert_eq!(
            env.player.damage_shift_table[DMG_PHYSICAL][DMG_PHYSICAL],
            75.0
        );
    }

    // ── Stun tests ───────────────────────────────────────────────────────

    #[test]
    fn stun_threshold_based_on_life() {
        let mut env = make_env_with_mods(vec![Mod::new_base("Life", 5000.0, src())]);
        // Set Life output directly so stun calc can read it
        env.player.set_output("Life", 5000.0);
        run(&mut env);
        // threshold = 5000 * 0.5 * 1.0 = 2500
        let threshold = get_output_f64(&env.player.output, "StunThreshold");
        assert_eq!(threshold, 2500.0);
    }

    // ── Backward-compatible existing tests ───────────────────────────────

    #[test]
    fn fire_resist_capped_at_75() {
        let mut env = make_env_with_mods(vec![Mod::new_base("FireResist", 120.0, src())]);
        run(&mut env);
        let fire = get_output_f64(&env.player.output, "FireResist");
        assert_eq!(fire, 75.0, "Fire resist should be capped at 75, got {fire}");
    }

    #[test]
    fn chaos_resist_uncapped_negative() {
        let mut env = make_env_with_mods(vec![]);
        run(&mut env);
        let chaos = get_output_f64(&env.player.output, "ChaosResist");
        assert!(
            chaos <= 0.0,
            "Default chaos resist should be 0 or negative, got {chaos}"
        );
    }
}
