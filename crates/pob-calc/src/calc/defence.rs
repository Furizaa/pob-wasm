use super::env::CalcEnv;
use crate::mod_db::types::{KeywordFlags, ModFlags, ModType};

pub fn run(env: &mut CalcEnv) {
    calc_resistances(env);
    calc_armour_evasion_es(env);
    calc_block(env);
}

fn calc_resistances(env: &mut CalcEnv) {
    for (stat, max_stat) in &[
        ("FireResist", "FireResistMax"),
        ("ColdResist", "ColdResistMax"),
        ("LightningResist", "LightningResistMax"),
        ("ChaosResist", "ChaosResistMax"),
    ] {
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
        let max =
            env.player
                .mod_db
                .sum(ModType::Base, max_stat, ModFlags::NONE, KeywordFlags::NONE);
        let raw = base * (1.0 + inc / 100.0) * more;
        let capped = raw.min(max);
        env.player.set_output(stat, capped);
    }
}

fn calc_armour_evasion_es(env: &mut CalcEnv) {
    for stat in &["Armour", "Evasion", "EnergyShieldBase"] {
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
        let val = (base * (1.0 + inc / 100.0) * more).floor().max(0.0);
        env.player.set_output(stat, val);

        if base > 0.0 && (inc != 0.0 || more != 1.0) {
            let lines = vec![
                format!("{base:.0} (base)"),
                if inc != 0.0 {
                    format!("x {:.2} (increased/reduced)", 1.0 + inc / 100.0)
                } else {
                    String::new()
                },
                if more != 1.0 {
                    format!("x {more:.2} (more/less)")
                } else {
                    String::new()
                },
                format!("= {val:.0}"),
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect();
            env.player.set_breakdown_lines(stat, lines);
        }
    }
}

fn calc_block(env: &mut CalcEnv) {
    let base_block = env.player.mod_db.sum(
        ModType::Base,
        "BlockChance",
        ModFlags::NONE,
        KeywordFlags::NONE,
    );
    let inc_block = env.player.mod_db.sum(
        ModType::Inc,
        "BlockChance",
        ModFlags::NONE,
        KeywordFlags::NONE,
    );
    let max_block = env.player.mod_db.sum(
        ModType::Base,
        "BlockChanceMax",
        ModFlags::NONE,
        KeywordFlags::NONE,
    );
    let block = (base_block * (1.0 + inc_block / 100.0))
        .min(max_block)
        .max(0.0);
    env.player.set_output("BlockChance", block);
}

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

    fn make_env_with_mods(mods: Vec<Mod>) -> CalcEnv {
        let mut db = ModDb::new();
        db.add(Mod::new_base(
            "FireResistMax",
            75.0,
            ModSource::new("Base", "cap"),
        ));
        db.add(Mod::new_base(
            "ColdResistMax",
            75.0,
            ModSource::new("Base", "cap"),
        ));
        db.add(Mod::new_base(
            "LightningResistMax",
            75.0,
            ModSource::new("Base", "cap"),
        ));
        db.add(Mod::new_base(
            "ChaosResistMax",
            75.0,
            ModSource::new("Base", "cap"),
        ));
        for m in mods {
            db.add(m);
        }
        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        CalcEnv::new(db, ModDb::new(), Arc::new(game_data))
    }

    #[test]
    fn fire_resist_capped_at_75() {
        let mut env = make_env_with_mods(vec![Mod::new_base(
            "FireResist",
            120.0,
            ModSource::new("Item", "test"),
        )]);
        run(&mut env);
        let fire = match env.player.output.get("FireResist") {
            Some(crate::calc::env::OutputValue::Number(n)) => *n,
            _ => panic!("FireResist not set"),
        };
        assert_eq!(fire, 75.0, "Fire resist should be capped at 75, got {fire}");
    }

    #[test]
    fn chaos_resist_uncapped_negative() {
        let mut env = make_env_with_mods(vec![]);
        run(&mut env);
        let chaos = match env.player.output.get("ChaosResist") {
            Some(crate::calc::env::OutputValue::Number(n)) => *n,
            _ => 0.0,
        };
        assert!(
            chaos <= 0.0,
            "Default chaos resist should be 0 or negative, got {chaos}"
        );
    }
}
