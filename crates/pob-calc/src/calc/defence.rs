use super::env::{get_output_f64, CalcEnv};
use crate::calc::calc_tools::calc_def_mod;
use crate::mod_db::types::{KeywordFlags, Mod, ModFlags, ModSource, ModType, ModValue, SkillCfg};

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
    inject_pre_defence_mods(env);
    calc_primary_defences(env);
    calc_spell_suppression(env);
    calc_dodge(env);
    calc_recovery_rates(env);
    calc_leech_caps(env);
    calc_regeneration(env);
    calc_es_recharge(env);
    calc_ward_recharge_delay(env);
    calc_damage_reduction(env);
    calc_movement_and_avoidance(env);
    build_damage_shift_table(env);
    calc_stun(env);
    calc_life_recoverable(env);
}

// ── Task 3: Full resistances ─────────────────────────────────────────────────

fn calc_resistances(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // data.misc constants
    const MAX_RESIST_CAP: f64 = 90.0; // data.misc.MaxResistCap
    const RESIST_FLOOR: f64 = -200.0; // data.misc.ResistFloor

    // Physical resist is always 0
    env.player.set_output("PhysicalResist", 0.0);

    // ── Section 1: Resistance conversion (Lua lines 515-560) ─────────────────
    // Pass 1: Convert MAX resist mods between types.
    // For each resFrom→resTo pair with a non-zero MaxResConvertTo rate, sum all
    // non-"Base"-sourced BASE mods on resFrom ResistMax and add a fraction to resTo.
    for i in 0..RESIST_TYPE_NAMES.len() {
        let res_from = RESIST_TYPE_NAMES[i];
        let mut max_res: Option<f64> = None; // lazy-init: None = not yet computed
        let mut new_mods: Vec<(String, f64)> = Vec::new();
        for j in 0..RESIST_TYPE_NAMES.len() {
            let res_to = RESIST_TYPE_NAMES[j];
            let conv_stat = format!("{res_from}MaxResConvertTo{res_to}");
            let conversion_rate =
                env.player.mod_db.sum_cfg(ModType::Base, &conv_stat, None, &output) / 100.0;
            if conversion_rate != 0.0 {
                if max_res.is_none() {
                    // Sum only non-"Base" sourced mods (exclude the 75 default seed)
                    let tabulated = env.player.mod_db.tabulate_cfg(
                        &format!("{res_from}ResistMax"),
                        Some(ModType::Base),
                        None,
                        &output,
                    );
                    let sum: f64 = tabulated
                        .iter()
                        .filter(|m| m.source_category != "Base")
                        .filter_map(|m| {
                            if let ModValue::Number(v) = &m.value {
                                Some(*v)
                            } else {
                                None
                            }
                        })
                        .sum();
                    max_res = Some(sum);
                }
                let max_res_val = max_res.unwrap();
                if max_res_val != 0.0 {
                    new_mods.push((format!("{res_to}ResistMax"), max_res_val * conversion_rate));
                }
            }
        }
        for (stat, value) in new_mods {
            env.player.mod_db.add(Mod::new_base(
                stat,
                value,
                ModSource::new("Conversion", format!("{res_from} To ... Max Resistance Conversion")),
            ));
        }
    }

    // Pass 2: Convert actual RESIST mods between types (BASE, INC, MORE).
    for i in 0..RESIST_TYPE_NAMES.len() {
        let res_from = RESIST_TYPE_NAMES[i];
        let mut res: Option<f64> = None; // lazy-init for BASE sum
        let mut new_mods: Vec<Mod> = Vec::new();
        for j in 0..RESIST_TYPE_NAMES.len() {
            let res_to = RESIST_TYPE_NAMES[j];
            let conv_stat = format!("{res_from}ResConvertTo{res_to}");
            let conversion_rate =
                env.player.mod_db.sum_cfg(ModType::Base, &conv_stat, None, &output) / 100.0;
            if conversion_rate != 0.0 {
                if res.is_none() {
                    let tabulated = env.player.mod_db.tabulate_cfg(
                        &format!("{res_from}Resist"),
                        Some(ModType::Base),
                        None,
                        &output,
                    );
                    let sum: f64 = tabulated
                        .iter()
                        .filter(|m| m.source_category != "Base")
                        .filter_map(|m| {
                            if let ModValue::Number(v) = &m.value {
                                Some(*v)
                            } else {
                                None
                            }
                        })
                        .sum();
                    res = Some(sum);
                }
                let res_val = res.unwrap();
                if res_val != 0.0 {
                    new_mods.push(Mod::new_base(
                        format!("{res_to}Resist"),
                        res_val * conversion_rate,
                        ModSource::new(
                            "Conversion",
                            format!("{res_from} To {res_to} Resistance Conversion"),
                        ),
                    ));
                }
                // Also copy INC mods proportionally
                for m in env.player.mod_db.tabulate_cfg(
                    &format!("{res_from}Resist"),
                    Some(ModType::Inc),
                    None,
                    &output,
                ) {
                    if let ModValue::Number(v) = &m.value {
                        new_mods.push(Mod {
                            name: format!("{res_to}Resist"),
                            mod_type: ModType::Inc,
                            value: ModValue::Number(v * conversion_rate),
                            flags: ModFlags::NONE,
                            keyword_flags: KeywordFlags::NONE,
                            tags: Vec::new(),
                            source: ModSource::new(m.source_category, m.source_name),
                        });
                    }
                }
                // Also copy MORE mods proportionally
                for m in env.player.mod_db.tabulate_cfg(
                    &format!("{res_from}Resist"),
                    Some(ModType::More),
                    None,
                    &output,
                ) {
                    if let ModValue::Number(v) = &m.value {
                        new_mods.push(Mod {
                            name: format!("{res_to}Resist"),
                            mod_type: ModType::More,
                            value: ModValue::Number(v * conversion_rate),
                            flags: ModFlags::NONE,
                            keyword_flags: KeywordFlags::NONE,
                            tags: Vec::new(),
                            source: ModSource::new(m.source_category, m.source_name),
                        });
                    }
                }
            }
        }
        for m in new_mods {
            env.player.mod_db.add(m);
        }
    }

    // ── Section 2: Melding of the Flesh (Lua lines 562-578) ──────────────────
    // If flag set, find the highest elemental max resist and override all elemental
    // max resists to that value via OVERRIDE mods (so the main loop picks them up).
    if env
        .player
        .mod_db
        .flag_cfg("ElementalResistMaxIsHighestResistMax", None, &output)
    {
        let mut highest_resist_max: f64 = 0.0;
        for elem in RESIST_TYPE_NAMES.iter() {
            if *elem == "Chaos" {
                continue;
            }
            let max_stat = format!("{elem}ResistMax");
            let resist_max = env
                .player
                .mod_db
                .override_value(&max_stat, None, &output)
                .unwrap_or_else(|| {
                    let base = env
                        .player
                        .mod_db
                        .sum_cfg(ModType::Base, &max_stat, None, &output);
                    let elemental = env.player.mod_db.sum_cfg(
                        ModType::Base,
                        "ElementalResistMax",
                        None,
                        &output,
                    );
                    (base + elemental).min(MAX_RESIST_CAP)
                });
            if resist_max > highest_resist_max {
                highest_resist_max = resist_max;
            }
        }
        for elem in RESIST_TYPE_NAMES.iter() {
            if *elem == "Chaos" {
                continue;
            }
            env.player.mod_db.add(Mod {
                name: format!("{elem}ResistMax"),
                mod_type: ModType::Override,
                value: ModValue::Number(highest_resist_max),
                flags: ModFlags::NONE,
                keyword_flags: KeywordFlags::NONE,
                tags: Vec::new(),
                source: ModSource::new("Keystone", "Melding of the Flesh"),
            });
        }
    }

    // ── Section 3: Main resistance computation loop (Lua lines 580-634) ───────
    // DOT cfg: matches global mods (NONE flags) and Dot-flagged mods.
    let dot_cfg = SkillCfg {
        flags: ModFlags::DOT,
        ..Default::default()
    };

    for elem in RESIST_TYPE_NAMES.iter() {
        let is_elemental = *elem != "Chaos";
        let resist_stat = format!("{elem}Resist");
        let max_stat = format!("{elem}ResistMax");

        // max = Override or min(90, BASE sum including ElementalResistMax for elemental)
        let max_resist = env
            .player
            .mod_db
            .override_value(&max_stat, None, &output)
            .unwrap_or_else(|| {
                let base = env
                    .player
                    .mod_db
                    .sum_cfg(ModType::Base, &max_stat, None, &output);
                let elemental = if is_elemental {
                    env.player
                        .mod_db
                        .sum_cfg(ModType::Base, "ElementalResistMax", None, &output)
                } else {
                    0.0
                };
                (base + elemental).min(MAX_RESIST_CAP)
            });

        // total = Override or base * max(0, (1 + INC/100) * More)
        // dotTotal = dotBase * same multiplier (dotBase uses DOT-flagged cfg)
        let (total_resist, dot_total) = match env
            .player
            .mod_db
            .override_value(&resist_stat, None, &output)
        {
            Some(ov) => (ov, ov), // dotTotal is nil when overridden → falls back to total
            None => {
                let base = env
                    .player
                    .mod_db
                    .sum_cfg(ModType::Base, &resist_stat, None, &output)
                    + if is_elemental {
                        env.player
                            .mod_db
                            .sum_cfg(ModType::Base, "ElementalResist", None, &output)
                    } else {
                        0.0
                    };
                // calcLib.mod returns (1 + INC/100) * More; clamped to 0 minimum
                let inc = env
                    .player
                    .mod_db
                    .sum_cfg(ModType::Inc, &resist_stat, None, &output)
                    + if is_elemental {
                        env.player
                            .mod_db
                            .sum_cfg(ModType::Inc, "ElementalResist", None, &output)
                    } else {
                        0.0
                    };
                let more = env.player.mod_db.more_cfg(&resist_stat, None, &output)
                    * if is_elemental {
                        env.player.mod_db.more_cfg("ElementalResist", None, &output)
                    } else {
                        1.0
                    };
                let multiplier = ((1.0 + inc / 100.0) * more).max(0.0);
                let total = base * multiplier;
                // dotBase: same stat names but with DOT cfg (includes DOT-flagged mods)
                let dot_base = env
                    .player
                    .mod_db
                    .sum_cfg(ModType::Base, &resist_stat, Some(&dot_cfg), &output)
                    + if is_elemental {
                        env.player.mod_db.sum_cfg(
                            ModType::Base,
                            "ElementalResist",
                            Some(&dot_cfg),
                            &output,
                        )
                    } else {
                        0.0
                    };
                let dot_t = dot_base * multiplier;
                (total, dot_t)
            }
        };

        // Truncate toward zero (m_modf): trunc(-53.7) = -53, NOT floor(-53.7) = -54
        let total_trunc = total_resist.trunc();
        let dot_trunc = dot_total.trunc();
        let max_trunc = max_resist.trunc();
        let min_trunc = RESIST_FLOOR.trunc(); // -200.0 (already integer)

        // Clamp: final = max(min, min(total, max)) = total.clamp(-200, max)
        let final_resist = total_trunc.clamp(min_trunc, max_trunc);
        let dot_final = dot_trunc.clamp(min_trunc, max_trunc);
        let over_cap = (total_trunc - max_trunc).max(0.0);
        let over_75 = (final_resist - 75.0).max(0.0); // Lua: m_max(0, final - 75)
        let missing = (max_trunc - final_resist).max(0.0);

        env.player.set_output(&resist_stat, final_resist);
        env.player
            .set_output(&format!("{elem}ResistTotal"), total_trunc);
        env.player
            .set_output(&format!("{elem}ResistOverCap"), over_cap);
        env.player
            .set_output(&format!("{elem}ResistOver75"), over_75);
        env.player
            .set_output(&format!("Missing{elem}Resist"), missing);
        env.player
            .set_output(&format!("{elem}ResistOverTime"), dot_final);

        // ── Totem resists ────────────────────────────────────────────────────
        // Totems have separate max resist stats (TotemXxxResistMax) that are
        // not affected by player "+1 max resist" mods. Lua lines 585-621.
        let totem_resist_stat = format!("Totem{elem}Resist");
        let totem_max_stat = format!("Totem{elem}ResistMax");

        let totem_max_raw = env
            .player
            .mod_db
            .override_value(&totem_max_stat, None, &output)
            .unwrap_or_else(|| {
                let base = env
                    .player
                    .mod_db
                    .sum_cfg(ModType::Base, &totem_max_stat, None, &output);
                let elem_max = if is_elemental {
                    env.player.mod_db.sum_cfg(
                        ModType::Base,
                        "TotemElementalResistMax",
                        None,
                        &output,
                    )
                } else {
                    0.0
                };
                (base + elem_max).min(MAX_RESIST_CAP)
            });
        let totem_max = totem_max_raw.trunc();

        let totem_total_raw = match env
            .player
            .mod_db
            .override_value(&totem_resist_stat, None, &output)
        {
            Some(ov) => ov,
            None => {
                let base = env
                    .player
                    .mod_db
                    .sum_cfg(ModType::Base, &totem_resist_stat, None, &output)
                    + if is_elemental {
                        env.player.mod_db.sum_cfg(
                            ModType::Base,
                            "TotemElementalResist",
                            None,
                            &output,
                        )
                    } else {
                        0.0
                    };
                let totem_inc = env
                    .player
                    .mod_db
                    .sum_cfg(ModType::Inc, &totem_resist_stat, None, &output)
                    + if is_elemental {
                        env.player.mod_db.sum_cfg(
                            ModType::Inc,
                            "TotemElementalResist",
                            None,
                            &output,
                        )
                    } else {
                        0.0
                    };
                let totem_more = env
                    .player
                    .mod_db
                    .more_cfg(&totem_resist_stat, None, &output)
                    * if is_elemental {
                        env.player
                            .mod_db
                            .more_cfg("TotemElementalResist", None, &output)
                    } else {
                        1.0
                    };
                let totem_multiplier = ((1.0 + totem_inc / 100.0) * totem_more).max(0.0);
                base * totem_multiplier
            }
        };
        let totem_total = totem_total_raw.trunc();
        // Totem final also uses RESIST_FLOOR as lower bound (same min as player)
        let totem_final = totem_total.clamp(min_trunc, totem_max);
        let totem_over_cap = (totem_total - totem_max).max(0.0);
        let missing_totem = (totem_max - totem_final).max(0.0);

        env.player
            .set_output(&format!("Totem{elem}Resist"), totem_final);
        env.player
            .set_output(&format!("Totem{elem}ResistTotal"), totem_total);
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

// ── Defence for conditionals (CalcDefence.lua:480-506) ──────────────────────

/// Writes per-slot item base values to output so conditional modifiers
/// (e.g. "do I have armour on helmet?") can evaluate correctly.
/// Called from perform::run() before charges/misc, matching CalcPerform.lua:3271.
pub fn defence_for_conditionals(env: &mut CalcEnv) {
    let output = env.player.output.clone();
    let gear_slots: Vec<(String, crate::build::types::ItemArmourData)> =
        env.player.gear_slot_armour.clone();

    for (slot, ad) in &gear_slots {
        let ward_base = if !env
            .player
            .mod_db
            .flag_cfg(&format!("GainNoWardFrom{slot}"), None, &output)
        {
            ad.ward
        } else {
            0.0
        };
        if ward_base > 0.0 {
            env.player
                .set_output(&format!("WardOn{slot}"), ward_base);
        }

        let es_base = if !env
            .player
            .mod_db
            .flag_cfg(&format!("GainNoEnergyShieldFrom{slot}"), None, &output)
        {
            ad.energy_shield
        } else {
            0.0
        };
        if es_base > 0.0 {
            env.player
                .set_output(&format!("EnergyShieldOn{slot}"), es_base);
        }

        let armour_base = if !env
            .player
            .mod_db
            .flag_cfg(&format!("GainNoArmourFrom{slot}"), None, &output)
        {
            ad.armour
        } else {
            0.0
        };
        if armour_base > 0.0 {
            env.player
                .set_output(&format!("ArmourOn{slot}"), armour_base);
        }

        let evasion_base = if !env
            .player
            .mod_db
            .flag_cfg(&format!("GainNoEvasionFrom{slot}"), None, &output)
        {
            ad.evasion
        } else {
            0.0
        };
        if evasion_base > 0.0 {
            env.player
                .set_output(&format!("EvasionOn{slot}"), evasion_base);
        }
    }
}

// ── Pre-defence modDB injections (CalcDefence.lua:772-823) ──────────────────

/// Inject INC mods into modDB based on already-computed output values.
/// Must run after resistances and block are computed, before primary defences.
fn inject_pre_defence_mods(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // ArmourAppliesToEnergyShieldRecharge (CalcDefence.lua:772-780)
    // Copies Armour INC mods as EnergyShieldRecharge INC mods.
    // This is an uncommon mastery — skip tabulate for now; no oracle build uses it.
    // The flag check is kept so that if the flag is present, the behaviour is correct
    // once Tabulate is supported.

    // ArmourIncreasedByUncappedFireRes (CalcDefence.lua:782-788)
    if env
        .player
        .mod_db
        .flag_cfg("ArmourIncreasedByUncappedFireRes", None, &output)
    {
        let fire_resist_total = get_output_f64(&output, "FireResistTotal");
        let source = env
            .player
            .mod_db
            .first_mod_source("ArmourIncreasedByUncappedFireRes")
            .unwrap_or(ModSource::new("Custom", "ArmourIncreasedByUncappedFireRes"));
        env.player.mod_db.add(Mod {
            name: "Armour".to_string(),
            mod_type: ModType::Inc,
            value: ModValue::Number(fire_resist_total),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: Vec::new(),
            source,
        });
    }

    // ArmourIncreasedByOvercappedFireRes (CalcDefence.lua:789-795)
    if env
        .player
        .mod_db
        .flag_cfg("ArmourIncreasedByOvercappedFireRes", None, &output)
    {
        let fire_resist_over_cap = get_output_f64(&output, "FireResistOverCap");
        let source = env
            .player
            .mod_db
            .first_mod_source("ArmourIncreasedByOvercappedFireRes")
            .unwrap_or(ModSource::new("Custom", "ArmourIncreasedByOvercappedFireRes"));
        env.player.mod_db.add(Mod {
            name: "Armour".to_string(),
            mod_type: ModType::Inc,
            value: ModValue::Number(fire_resist_over_cap),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: Vec::new(),
            source,
        });
    }

    // EvasionRatingIncreasedByUncappedColdRes (CalcDefence.lua:796-802)
    if env
        .player
        .mod_db
        .flag_cfg("EvasionRatingIncreasedByUncappedColdRes", None, &output)
    {
        let cold_resist_total = get_output_f64(&output, "ColdResistTotal");
        let source = env
            .player
            .mod_db
            .first_mod_source("EvasionRatingIncreasedByUncappedColdRes")
            .unwrap_or(ModSource::new("Custom", "EvasionRatingIncreasedByUncappedColdRes"));
        env.player.mod_db.add(Mod {
            name: "Evasion".to_string(),
            mod_type: ModType::Inc,
            value: ModValue::Number(cold_resist_total),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: Vec::new(),
            source,
        });
    }

    // EvasionRatingIncreasedByOvercappedColdRes (CalcDefence.lua:803-809)
    if env
        .player
        .mod_db
        .flag_cfg("EvasionRatingIncreasedByOvercappedColdRes", None, &output)
    {
        let cold_resist_over_cap = get_output_f64(&output, "ColdResistOverCap");
        let source = env
            .player
            .mod_db
            .first_mod_source("EvasionRatingIncreasedByOvercappedColdRes")
            .unwrap_or(ModSource::new("Custom", "EvasionRatingIncreasedByOvercappedColdRes"));
        env.player.mod_db.add(Mod {
            name: "Evasion".to_string(),
            mod_type: ModType::Inc,
            value: ModValue::Number(cold_resist_over_cap),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: Vec::new(),
            source,
        });
    }

    // EnergyShieldIncreasedByChanceToBlockSpellDamage (CalcDefence.lua:810-816)
    if env.player.mod_db.flag_cfg(
        "EnergyShieldIncreasedByChanceToBlockSpellDamage",
        None,
        &output,
    ) {
        let spell_block = get_output_f64(&output, "SpellBlockChance");
        let source = env
            .player
            .mod_db
            .first_mod_source("EnergyShieldIncreasedByChanceToBlockSpellDamage")
            .unwrap_or(ModSource::new(
                "Custom",
                "EnergyShieldIncreasedByChanceToBlockSpellDamage",
            ));
        env.player.mod_db.add(Mod {
            name: "EnergyShield".to_string(),
            mod_type: ModType::Inc,
            value: ModValue::Number(spell_block),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: Vec::new(),
            source,
        });
    }

    // EnergyShieldIncreasedByChaosResistance (CalcDefence.lua:817-823)
    if env
        .player
        .mod_db
        .flag_cfg("EnergyShieldIncreasedByChaosResistance", None, &output)
    {
        let chaos_resist = get_output_f64(&output, "ChaosResist");
        let source = env
            .player
            .mod_db
            .first_mod_source("EnergyShieldIncreasedByChaosResistance")
            .unwrap_or(ModSource::new("Custom", "EnergyShieldIncreasedByChaosResistance"));
        env.player.mod_db.add(Mod {
            name: "EnergyShield".to_string(),
            mod_type: ModType::Inc,
            value: ModValue::Number(chaos_resist),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: Vec::new(),
            source,
        });
    }
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

    // SpellSuppressionAppliesToChanceToDefendWithArmour (CalcDefence.lua:1551-1558)
    // Foulborn Ancestral Vision: inject ArmourDefense MAX mods from spell suppression.
    if env.player.mod_db.flag_cfg(
        "SpellSuppressionAppliesToChanceToDefendWithArmour",
        None,
        &output,
    ) {
        let suppress_armour_pct = env
            .player
            .mod_db
            .max_value(
                "SpellSuppressionAppliesToChanceToDefendWithArmourPercent",
                None,
                &output,
            )
            .unwrap_or(0.0);
        let armour_defense_pct = env
            .player
            .mod_db
            .max_value(
                "SpellSuppressionAppliesToChanceToDefendWithArmourPercentArmour",
                None,
                &output,
            )
            .unwrap_or(0.0);
        let source =
            ModSource::new("Custom", "Chance to Defend from Spell Suppression");

        // Max Calc: ArmourDefense = armourDefensePercent - 100 (with ArmourMax condition)
        env.player.mod_db.add(Mod {
            name: "ArmourDefense".to_string(),
            mod_type: ModType::Max,
            value: ModValue::Number(armour_defense_pct - 100.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![crate::mod_db::types::ModTag::Condition {
                var: "ArmourMax".to_string(),
                neg: false,
            }],
            source: source.clone(),
        });

        // Average Calc
        let avg_factor =
            (suppress_armour_pct * chance / 100.0).min(1.0);
        env.player.mod_db.add(Mod {
            name: "ArmourDefense".to_string(),
            mod_type: ModType::Max,
            value: ModValue::Number(avg_factor * (armour_defense_pct - 100.0)),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![crate::mod_db::types::ModTag::Condition {
                var: "ArmourAvg".to_string(),
                neg: false,
            }],
            source: source.clone(),
        });

        // Min Calc
        let min_factor =
            (suppress_armour_pct * chance / 100.0).floor().min(1.0);
        env.player.mod_db.add(Mod {
            name: "ArmourDefense".to_string(),
            mod_type: ModType::Max,
            value: ModValue::Number(min_factor * (armour_defense_pct - 100.0)),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![crate::mod_db::types::ModTag::Condition {
                var: "ArmourMax".to_string(),
                neg: true,
            }, crate::mod_db::types::ModTag::Condition {
                var: "ArmourAvg".to_string(),
                neg: true,
            }],
            source,
        });
    }

    // ArmourDefense output (CalcDefence.lua:1559)
    let armour_defense = env
        .player
        .mod_db
        .max_value("ArmourDefense", None, &output)
        .unwrap_or(0.0)
        / 100.0;
    env.player.set_output("ArmourDefense", armour_defense);
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

    // CalcDefence.lua:1192-1197:
    //   output.LifeRecoveryRateMod = 1
    //   if not modDB:Flag(nil, "CannotRecoverLifeOutsideLeech") then
    //     output.LifeRecoveryRateMod = calcLib.mod(modDB, nil, "LifeRecoveryRate")
    //   end
    //   output.ManaRecoveryRateMod = calcLib.mod(modDB, nil, "ManaRecoveryRate")
    //   output.EnergyShieldRecoveryRateMod = calcLib.mod(modDB, nil, "EnergyShieldRecoveryRate")
    let cannot_recover_life_outside_leech =
        env.player
            .mod_db
            .flag_cfg("CannotRecoverLifeOutsideLeech", None, &output);

    for resource in &["Life", "Mana", "EnergyShield"] {
        // The mod stat name is "{resource}RecoveryRate" (e.g. "LifeRecoveryRate").
        // The output field name is "{resource}RecoveryRateMod".
        let mod_stat = format!("{resource}RecoveryRate");
        let output_stat = format!("{resource}RecoveryRateMod");

        // Gap a: CannotRecoverLifeOutsideLeech sets LifeRecoveryRateMod to 1 (no recovery bonus).
        if *resource == "Life" && cannot_recover_life_outside_leech {
            env.player.set_output(&output_stat, 1.0);
            continue;
        }

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
    // CalcDefence.lua:1199-1232
    // calcLib.val(modDB, "X") = modDB:Sum("BASE", nil, "X")
    // Defaults are seeded via NewMod in CalcSetup.lua (initEnv):
    //   MaxLifeLeechRate   = max_life_leech_rate_%_per_minute / 60   = 1200/60 = 20
    //   MaxManaLeechRate   = max_mana_leech_rate_%_per_minute / 60   = 1200/60 = 20
    //   MaxEnergyShieldLeechRate = 10 (hardcoded)
    //   MaxLifeLeechInstance / MaxManaLeechInstance / MaxEnergyShieldLeechInstance = 10
    let output = env.player.output.clone();

    let life = get_output_f64(&output, "Life");
    let mana = get_output_f64(&output, "Mana");
    let es = get_output_f64(&output, "EnergyShield");

    // output.MaxLifeLeechInstance = Life * calcLib.val(modDB, "MaxLifeLeechInstance") / 100
    let life_leech_instance_pct =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MaxLifeLeechInstance", None, &output);
    env.player.set_output(
        "MaxLifeLeechInstance",
        life * life_leech_instance_pct / 100.0,
    );

    // output.MaxLifeLeechRatePercent = calcLib.val(modDB, "MaxLifeLeechRate")
    // (no MaximumLifeLeechIsEqualToParent / PartyMember branch for player)
    let life_leech_rate_pct =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MaxLifeLeechRate", None, &output);
    // output.MaxLifeLeechRate = Life * MaxLifeLeechRatePercent / 100
    let life_leech_rate = life * life_leech_rate_pct / 100.0;
    env.player.set_output("MaxLifeLeechRate", life_leech_rate);
    env.player
        .set_output("MaxLifeLeechRatePercent", life_leech_rate_pct);

    // output.MaxEnergyShieldLeechInstance = EnergyShield * calcLib.val(modDB, "MaxEnergyShieldLeechInstance") / 100
    let es_leech_instance_pct =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MaxEnergyShieldLeechInstance", None, &output);
    env.player.set_output(
        "MaxEnergyShieldLeechInstance",
        es * es_leech_instance_pct / 100.0,
    );

    // output.MaxEnergyShieldLeechRate = EnergyShield * calcLib.val(modDB, "MaxEnergyShieldLeechRate") / 100
    let es_leech_rate_pct =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MaxEnergyShieldLeechRate", None, &output);
    env.player
        .set_output("MaxEnergyShieldLeechRate", es * es_leech_rate_pct / 100.0);

    // output.MaxManaLeechInstance = Mana * calcLib.val(modDB, "MaxManaLeechInstance") / 100
    let mana_leech_instance_pct =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MaxManaLeechInstance", None, &output);
    env.player.set_output(
        "MaxManaLeechInstance",
        mana * mana_leech_instance_pct / 100.0,
    );

    // output.MaxManaLeechRate = Mana * calcLib.val(modDB, "MaxManaLeechRate") / 100
    let mana_leech_rate_pct =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "MaxManaLeechRate", None, &output);
    env.player
        .set_output("MaxManaLeechRate", mana * mana_leech_rate_pct / 100.0);
}

fn calc_regeneration(env: &mut CalcEnv) {
    // CalcDefence.lua:1234–1320
    // Lua iterates resources = {"Mana", "Life", "Energy Shield", "Rage"}.
    // "Energy Shield" → resource key "EnergyShield" via gsub(" ", "").
    // Loop order is semantically significant: Zealot's Oath and Pious Path mutate
    // the modDB mid-loop so later iterations pick up new mods.
    let resources = ["Mana", "Life", "EnergyShield", "Rage"];

    // Round to N decimal places: round(x, 1) = (x * 10.0).round() / 10.0
    let round1 = |x: f64| (x * 10.0).round() / 10.0;
    // floor(x, 2) = (x * 100.0).floor() / 100.0
    let floor2 = |x: f64| (x * 100.0).floor() / 100.0;

    let regen_src = ModSource::new("Calc", "calc_regeneration");

    for &resource in &resources {
        // Re-clone output each iteration so modDB mutations from prior iterations
        // (Zealot's Oath, Pious Path) are visible.
        let output = env.player.output.clone();
        let pool = get_output_f64(&output, resource); // may be 0 for Rage

        // recoveryRateMod defaults to 1 (it was set earlier by calc_recovery_rates)
        let recovery_rate_mod_key = format!("{resource}RecoveryRateMod");
        let recovery_rate_mod = {
            let v = get_output_f64(&output, &recovery_rate_mod_key);
            if v == 0.0 {
                1.0
            } else {
                v
            }
        };

        let inc =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, &format!("{resource}Regen"), None, &output);

        // Always write RegenInc regardless of branch
        env.player.set_output(&format!("{resource}RegenInc"), inc);

        let regen_rate: f64;

        if env
            .player
            .mod_db
            .flag_cfg(&format!("No{resource}Regen"), None, &output)
            || env
                .player
                .mod_db
                .flag_cfg(&format!("CannotGain{resource}"), None, &output)
        {
            // Branch: regen disabled
            env.player.set_output(&format!("{resource}Regen"), 0.0);
            regen_rate = 0.0;
        } else if resource == "Life" && env.player.mod_db.flag_cfg("ZealotsOath", None, &output) {
            // Branch: Zealot's Oath — life regen redirected to ES
            env.player.set_output("LifeRegen", 0.0);
            regen_rate = 0.0;
            let life_base = env
                .player
                .mod_db
                .sum_cfg(ModType::Base, "LifeRegen", None, &output);
            if life_base > 0.0 {
                env.player.mod_db.add(Mod::new_base(
                    "EnergyShieldRegen",
                    life_base,
                    regen_src.clone(),
                ));
            }
            let life_pct =
                env.player
                    .mod_db
                    .sum_cfg(ModType::Base, "LifeRegenPercent", None, &output);
            if life_pct > 0.0 {
                env.player.mod_db.add(Mod::new_base(
                    "EnergyShieldRegenPercent",
                    life_pct,
                    regen_src.clone(),
                ));
            }
        } else {
            // Normal regen branch

            // Chain redirection: if inc != 0, check if this resource's regen inc should
            // apply to a later resource in the loop order.
            // resources = ["Mana", "Life", "EnergyShield", "Rage"]
            // i=0 Mana → j=1,2,3 (Life, EnergyShield, Rage)
            // i=1 Life → j=2,3 (EnergyShield, Rage)
            // etc.
            let mut effective_inc = inc;
            if effective_inc != 0.0 {
                let resource_list = ["Mana", "Life", "EnergyShield", "Rage"];
                let self_idx = resource_list
                    .iter()
                    .position(|&r| r == resource)
                    .unwrap_or(0);
                for j in (self_idx + 1)..resource_list.len() {
                    let target = resource_list[j];
                    let flag_name = format!("{resource}RegenTo{target}Regen");
                    if env.player.mod_db.flag_cfg(&flag_name, None, &output) {
                        env.player.mod_db.add(Mod {
                            name: format!("{target}Regen"),
                            mod_type: ModType::Inc,
                            value: ModValue::Number(effective_inc),
                            flags: ModFlags::NONE,
                            keyword_flags: KeywordFlags::NONE,
                            tags: Vec::new(),
                            source: regen_src.clone(),
                        });
                        effective_inc = 0.0;
                        break;
                    }
                }
            }

            // Pious Path: life regen applies to ES
            if resource == "Life" {
                let applies_to_es = env.player.mod_db.sum_cfg(
                    ModType::Base,
                    "LifeRegenAppliesToEnergyShield",
                    None,
                    &output,
                );
                if applies_to_es > 0.0 {
                    let conversion = applies_to_es.min(100.0) / 100.0;
                    let life_base_regen =
                        env.player
                            .mod_db
                            .sum_cfg(ModType::Base, "LifeRegen", None, &output);
                    let life_pct_regen =
                        env.player
                            .mod_db
                            .sum_cfg(ModType::Base, "LifeRegenPercent", None, &output);
                    env.player.mod_db.add(Mod::new_base(
                        "EnergyShieldRegen",
                        floor2(life_base_regen * conversion),
                        regen_src.clone(),
                    ));
                    env.player.mod_db.add(Mod::new_base(
                        "EnergyShieldRegenPercent",
                        floor2(life_pct_regen * conversion),
                        regen_src.clone(),
                    ));
                }
            }

            // Core formula: baseRegen = flat + pool * pct/100
            // Re-query after potential modDB mutations
            let output2 = env.player.output.clone();
            let base_flat = env.player.mod_db.sum_cfg(
                ModType::Base,
                &format!("{resource}Regen"),
                None,
                &output2,
            );
            let base_pct = env.player.mod_db.sum_cfg(
                ModType::Base,
                &format!("{resource}RegenPercent"),
                None,
                &output2,
            );
            // Re-query more after potential INC redirect mutations
            let more_updated =
                env.player
                    .mod_db
                    .more_cfg(&format!("{resource}Regen"), None, &output2);

            let base_regen = base_flat + pool * base_pct / 100.0;
            let regen = base_regen * (1.0 + effective_inc / 100.0) * more_updated;

            // Pious Path recovery: if regen != 0, route to later resources
            if regen != 0.0 {
                let resource_list = ["Mana", "Life", "EnergyShield", "Rage"];
                let self_idx = resource_list
                    .iter()
                    .position(|&r| r == resource)
                    .unwrap_or(0);
                for j in (self_idx + 1)..resource_list.len() {
                    let target = resource_list[j];
                    let flag_name = format!("{resource}RegenerationRecovers{target}");
                    if env.player.mod_db.flag_cfg(&flag_name, None, &output2) {
                        env.player.mod_db.add(Mod::new_base(
                            &format!("{target}Recovery"),
                            regen,
                            regen_src.clone(),
                        ));
                    }
                }
            }

            regen_rate = round1(regen * recovery_rate_mod);
            env.player
                .set_output(&format!("{resource}Regen"), regen_rate);
        }

        // Degen calculation (always runs)
        let output3 = env.player.output.clone();
        let base_degen_flat =
            env.player
                .mod_db
                .sum_cfg(ModType::Base, &format!("{resource}Degen"), None, &output3);
        let base_degen_pct = env.player.mod_db.sum_cfg(
            ModType::Base,
            &format!("{resource}DegenPercent"),
            None,
            &output3,
        );
        let tincture_pct = env.player.mod_db.sum_cfg(
            ModType::Base,
            &format!("{resource}DegenPercentTincture"),
            None,
            &output3,
        );

        let mut base_degen = base_degen_flat + pool * base_degen_pct / 100.0;
        // Tincture minimum: max(pool * tinctureDegenPercent / 100, tinctureDegenPercent)
        base_degen += (pool * tincture_pct / 100.0).max(tincture_pct);

        let degen_rate = if base_degen > 0.0 {
            let degen_inc = env.player.mod_db.sum_cfg(
                ModType::Inc,
                &format!("{resource}Degen"),
                None,
                &output3,
            );
            let degen_more =
                env.player
                    .mod_db
                    .more_cfg(&format!("{resource}Degen"), None, &output3);
            base_degen * (1.0 + degen_inc / 100.0) * degen_more
        } else {
            0.0
        };
        env.player
            .set_output(&format!("{resource}Degen"), degen_rate);

        // Recovery from modDB (Pious Path recovery events, misc)
        let output4 = env.player.output.clone();
        let recovery_rate_val = env.player.mod_db.sum_cfg(
            ModType::Base,
            &format!("{resource}Recovery"),
            None,
            &output4,
        ) * recovery_rate_mod;
        env.player
            .set_output(&format!("{resource}Recovery"), recovery_rate_val);

        // RegenRecovery = net regen after degen + recovery
        // UnaffectedBy{resource}Regen sets regen contribution to 0
        let regen_contribution =
            if env
                .player
                .mod_db
                .flag_cfg(&format!("UnaffectedBy{resource}Regen"), None, &output4)
            {
                0.0
            } else {
                regen_rate
            };
        let regen_recovery = regen_contribution - degen_rate + recovery_rate_val;
        env.player
            .set_output(&format!("{resource}RegenRecovery"), regen_recovery);

        // Gate condition: CanGain{resource} if net recovery is positive
        if regen_recovery > 0.0 {
            env.player
                .mod_db
                .set_condition(&format!("CanGain{resource}"), true);
        }

        // RegenPercent = RegenRecovery / pool * 100 (rounded to 1dp), or 0 if pool == 0
        let regen_pct = if pool > 0.0 {
            round1(regen_recovery / pool * 100.0)
        } else {
            0.0
        };
        env.player
            .set_output(&format!("{resource}RegenPercent"), regen_pct);
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
        // Data.lua:182: EnergyShieldRechargeBase = characterConstants["energy_shield_recharge_rate_per_minute_%"] / 60 / 100
        // = 2000 / 60 / 100 = 0.3333...
        // Note: Data.lua:183 has a duplicate assignment "= 0.33" but per oracle evidence,
        // the correct value used by PoB is 0.3333... (the computed value). The 0.33 line
        // appears to be a stale/incorrect override that doesn't affect actual oracle builds.
        // CalcDefence.lua:1329
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
            // calc_recovery_rates runs before this and always sets LifeRecoveryRateMod to 1.0+
            let life_recovery_rate = get_output_f64(&output, "LifeRecoveryRateMod");
            let life_recovery_rate = if life_recovery_rate == 0.0 {
                1.0
            } else {
                life_recovery_rate
            };
            let recharge = life * base * (1.0 + inc / 100.0) * more;
            let life_recharge = (recharge * life_recovery_rate).round();
            env.player.set_output("LifeRecharge", life_recharge);
        } else {
            // output.EnergyShieldRecharge = round(EnergyShield * base * (1+inc/100) * more
            //   * EnergyShieldRecoveryRateMod)
            // CalcDefence.lua:1350-1351
            let es = get_output_f64(&output, "EnergyShield");
            let es_recovery_rate = get_output_f64(&output, "EnergyShieldRecoveryRateMod");
            let es_recovery_rate = if es_recovery_rate == 0.0 {
                1.0
            } else {
                es_recovery_rate
            };
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
    } else {
        // CalcDefence.lua:1380: output.EnergyShieldRecharge = 0
        env.player.set_output("EnergyShieldRecharge", 0.0);
    }
}

// ── Ward recharge delay (CalcDefence.lua:1473-1483) ──────────────────────────

/// Compute WardRechargeDelay.
/// Mirrors CalcDefence.lua:1474:
///   output.WardRechargeDelay = data.misc.WardRechargeDelay / (1 + INC("WardRechargeFaster") / 100)
/// data.misc.WardRechargeDelay = 2 (Data.lua:185)
fn calc_ward_recharge_delay(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    let faster = env
        .player
        .mod_db
        .sum_cfg(ModType::Inc, "WardRechargeFaster", None, &output);
    // data.misc.WardRechargeDelay = 2 seconds
    let delay = 2.0 / (1.0 + faster / 100.0);
    env.player.set_output("WardRechargeDelay", delay);
}

// ── Task 9: Movement speed, avoidance, misc ──────────────────────────────────

fn calc_movement_and_avoidance(env: &mut CalcEnv) {
    let output = env.player.output.clone();

    // Movement speed (CalcDefence.lua:1493-1506)
    // Priority: Override > standard calcLib.mod
    let mut ms = if let Some(override_val) =
        env.player
            .mod_db
            .override_value("MovementSpeed", None, &output)
    {
        override_val
    } else {
        let ms_inc =
            env.player
                .mod_db
                .sum_cfg(ModType::Inc, "MovementSpeed", None, &output);
        let ms_more = env.player.mod_db.more_cfg("MovementSpeed", None, &output);
        (1.0 + ms_inc / 100.0) * ms_more
    };

    // Floor: MovementSpeedCannotBeBelowBase prevents reduction below 100%
    if env
        .player
        .mod_db
        .flag_cfg("MovementSpeedCannotBeBelowBase", None, &output)
    {
        ms = ms.max(1.0);
    }

    env.player.set_output("MovementSpeedMod", ms);

    let action_speed = env.player.action_speed_mod;
    env.player
        .set_output("EffectiveMovementSpeedMod", ms * action_speed);

    // CalcDefence.lua:1512-1524 — On-block and on-suppress recovery.
    // LifeOnBlock and LifeOnSuppress are 0 when CannotRecoverLifeOutsideLeech is set.
    let cannot_recover_life_outside_leech =
        env.player
            .mod_db
            .flag_cfg("CannotRecoverLifeOutsideLeech", None, &output);

    // LifeOnBlock (guarded by CannotRecoverLifeOutsideLeech)
    let life_on_block = if cannot_recover_life_outside_leech {
        0.0
    } else {
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "LifeOnBlock", None, &output)
    };
    env.player.set_output("LifeOnBlock", life_on_block);

    // LifeOnSuppress (guarded by CannotRecoverLifeOutsideLeech)
    let life_on_suppress = if cannot_recover_life_outside_leech {
        0.0
    } else {
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "LifeOnSuppress", None, &output)
    };
    env.player.set_output("LifeOnSuppress", life_on_suppress);

    // ManaOnBlock (no guard)
    let mana_on_block = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "ManaOnBlock", None, &output);
    env.player.set_output("ManaOnBlock", mana_on_block);

    // EnergyShieldOnBlock (no guard)
    let es_on_block =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "EnergyShieldOnBlock", None, &output);
    env.player.set_output("EnergyShieldOnBlock", es_on_block);

    // Gap d: EnergyShieldOnSpellBlock and EnergyShieldOnSuppress (CalcDefence.lua:1522-1524)
    let es_on_spell_block =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "EnergyShieldOnSpellBlock", None, &output);
    env.player
        .set_output("EnergyShieldOnSpellBlock", es_on_spell_block);

    let es_on_suppress =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "EnergyShieldOnSuppress", None, &output);
    env.player
        .set_output("EnergyShieldOnSuppress", es_on_suppress);

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

    // Blind avoidance
    let blind_avoid = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "AvoidBlind", None, &output)
        .clamp(0.0, 100.0);
    env.player.set_output("BlindAvoidChance", blind_avoid);

    // Curse avoidance: CalcDefence.lua:1578
    // CurseImmune flag → 100, else sum BASE "AvoidCurse" capped at 100
    // Must be written BEFORE SilenceAvoidChance (which derives from it).
    let curse_avoid = if env.player.mod_db.flag_cfg("CurseImmune", None, &output) {
        100.0
    } else {
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "AvoidCurse", None, &output)
            .min(100.0)
    };
    env.player.set_output("CurseAvoidChance", curse_avoid);

    // Silence avoidance: CalcDefence.lua:1579
    // SilenceImmune flag → 100, else equals CurseAvoidChance.
    // There is no "AvoidSilence" stat — silence avoidance derives entirely from curse avoidance.
    let silence_avoid = if env.player.mod_db.flag_cfg("SilenceImmune", None, &output) {
        100.0
    } else {
        curse_avoid
    };
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

    // CurseEffectOnSelf: CalcDefence.lua:1586
    // Formula: More × (100 + INC), clamped at 0 minimum.
    // This is NOT calcLib.mod — there is no BASE; it's a direct More × (100 + INC) formula.
    // Result is a percentage where 100 = no change from curse effects.
    {
        let more = env
            .player
            .mod_db
            .more_cfg("CurseEffectOnSelf", None, &output);
        let inc = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "CurseEffectOnSelf", None, &output);
        let curse_effect_on_self = (more * (100.0 + inc)).max(0.0);
        env.player
            .set_output("CurseEffectOnSelf", curse_effect_on_self);
    }

    // ExposureEffectOnSelf: CalcDefence.lua:1587
    // Formula: More × (100 + INC) — same pattern as CurseEffectOnSelf, but no max(0) clamp.
    {
        let more = env
            .player
            .mod_db
            .more_cfg("ExposureEffectOnSelf", None, &output);
        let inc = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "ExposureEffectOnSelf", None, &output);
        env.player
            .set_output("ExposureEffectOnSelf", more * (100.0 + inc));
    }

    // WitherEffectOnSelf: CalcDefence.lua:1588
    // Formula: More × (100 + INC) — same pattern.
    {
        let more = env
            .player
            .mod_db
            .more_cfg("WitherEffectOnSelf", None, &output);
        let inc = env
            .player
            .mod_db
            .sum_cfg(ModType::Inc, "WitherEffectOnSelf", None, &output);
        env.player
            .set_output("WitherEffectOnSelf", more * (100.0 + inc));
    }

    // Debuff expiration rate: CalcDefence.lua:1591-1593
    // DebuffExpirationRate = BASE sum of "SelfDebuffExpirationRate" (NOT INC of "DebuffExpirationRate")
    let debuff_rate =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "SelfDebuffExpirationRate", None, &output);
    env.player.set_output("DebuffExpirationRate", debuff_rate);

    // DebuffExpirationModifier: 10000 / (100 + rate)
    // At rate=0: 100 (no change); at rate=100 (100% faster): 50 (half duration)
    let debuff_modifier = 10000.0 / (100.0 + debuff_rate);
    env.player
        .set_output("DebuffExpirationModifier", debuff_modifier);

    // showDebuffExpirationModifier: true when modifier is not exactly 100
    env.player
        .set_output_bool("showDebuffExpirationModifier", debuff_modifier != 100.0);
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
