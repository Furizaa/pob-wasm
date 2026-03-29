use super::env::{CalcEnv, OutputValue};
use crate::mod_db::types::{KeywordFlags, ModFlags, ModType};

pub fn run(env: &mut CalcEnv) {
    do_actor_life_mana(env);
    do_actor_attribs(env);
    do_actor_attack_cast_speed(env);
}

fn do_actor_attack_cast_speed(env: &mut CalcEnv) {
    let inc_attack =
        env.player
            .mod_db
            .sum(ModType::Inc, "Speed", ModFlags::ATTACK, KeywordFlags::NONE);
    let more_attack = env
        .player
        .mod_db
        .more("Speed", ModFlags::ATTACK, KeywordFlags::NONE);
    env.player.set_output(
        "AttackSpeedMod",
        1.0 * (1.0 + inc_attack / 100.0) * more_attack,
    );

    let inc_cast =
        env.player
            .mod_db
            .sum(ModType::Inc, "Speed", ModFlags::SPELL, KeywordFlags::NONE);
    let more_cast = env
        .player
        .mod_db
        .more("Speed", ModFlags::SPELL, KeywordFlags::NONE);
    env.player
        .set_output("CastSpeedMod", 1.0 * (1.0 + inc_cast / 100.0) * more_cast);
}

/// Mirrors doActorLifeMana() in CalcPerform.lua lines 68–130.
fn do_actor_life_mana(env: &mut CalcEnv) {
    // Chaos Inoculation: life is fixed at 1
    let chaos_inoc = env
        .player
        .mod_db
        .flag("ChaosInoculation", ModFlags::NONE, KeywordFlags::NONE);
    if chaos_inoc {
        env.player.set_output("Life", 1.0);
        env.player.mod_db.set_condition("FullLife", true);
        env.player.mod_db.set_condition("ChaosInoculation", true);
    } else {
        let base = env
            .player
            .mod_db
            .sum(ModType::Base, "Life", ModFlags::NONE, KeywordFlags::NONE);
        let inc = env
            .player
            .mod_db
            .sum(ModType::Inc, "Life", ModFlags::NONE, KeywordFlags::NONE);
        let more = env
            .player
            .mod_db
            .more("Life", ModFlags::NONE, KeywordFlags::NONE);
        let life = (base * (1.0 + inc / 100.0) * more).max(1.0).round();
        env.player.set_output("Life", life);

        // Breakdown
        if inc != 0.0 || more != 1.0 {
            let mut lines = vec![format!("{base:.0} (base)")];
            if inc != 0.0 {
                lines.push(format!("x {:.2} (increased/reduced)", 1.0 + inc / 100.0));
            }
            if more != 1.0 {
                lines.push(format!("x {more:.2} (more/less)"));
            }
            lines.push(format!("= {life:.0}"));
            env.player.set_breakdown_lines("Life", lines);
        }
    }

    // Mana
    {
        let base = env
            .player
            .mod_db
            .sum(ModType::Base, "Mana", ModFlags::NONE, KeywordFlags::NONE);
        let inc = env
            .player
            .mod_db
            .sum(ModType::Inc, "Mana", ModFlags::NONE, KeywordFlags::NONE);
        let more = env
            .player
            .mod_db
            .more("Mana", ModFlags::NONE, KeywordFlags::NONE);
        let mana = (base * (1.0 + inc / 100.0) * more).max(0.0).round();
        env.player.set_output("Mana", mana);

        if inc != 0.0 || more != 1.0 {
            let mut lines = vec![format!("{base:.0} (base)")];
            if inc != 0.0 {
                lines.push(format!("x {:.2} (increased/reduced)", 1.0 + inc / 100.0));
            }
            if more != 1.0 {
                lines.push(format!("x {more:.2} (more/less)"));
            }
            lines.push(format!("= {mana:.0}"));
            env.player.set_breakdown_lines("Mana", lines);
        }
    }

    // Energy Shield
    {
        let base = env.player.mod_db.sum(
            ModType::Base,
            "EnergyShield",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        let inc = env.player.mod_db.sum(
            ModType::Inc,
            "EnergyShield",
            ModFlags::NONE,
            KeywordFlags::NONE,
        );
        let more = env
            .player
            .mod_db
            .more("EnergyShield", ModFlags::NONE, KeywordFlags::NONE);
        let es = (base * (1.0 + inc / 100.0) * more).max(0.0).round();
        env.player.set_output("EnergyShield", es);
    }
}

/// Mirrors doActorAttribsConditions() in CalcPerform.lua lines 132–300.
/// Computes Str/Dex/Int and derived bonuses.
fn do_actor_attribs(env: &mut CalcEnv) {
    for stat in &["Str", "Dex", "Int"] {
        let base = env
            .player
            .mod_db
            .sum(ModType::Base, stat, ModFlags::NONE, KeywordFlags::NONE);
        let inc = env
            .player
            .mod_db
            .sum(ModType::Inc, stat, ModFlags::NONE, KeywordFlags::NONE);
        let more = env
            .player
            .mod_db
            .more(stat, ModFlags::NONE, KeywordFlags::NONE);
        let val = (base * (1.0 + inc / 100.0) * more).floor();
        env.player.set_output(stat, val);
    }

    // Strength bonus life: +1 max life per 2 Str (POB: life_per_str = 0.5)
    let str_val = env
        .player
        .output
        .get("Str")
        .and_then(|v| {
            if let OutputValue::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .unwrap_or(0.0);
    let life_from_str = (str_val * 0.5).floor();
    // Add to the existing Life base — re-run life calc with updated base
    // (Simplified: add life_from_str directly to the output. Full impl re-runs the pass.)
    if let Some(OutputValue::Number(life)) = env.player.output.get_mut("Life") {
        *life += life_from_str;
    }
}
