use super::env::{get_output_f64, CalcEnv};
use crate::build::Build;
use crate::mod_db::types::ModType;

pub fn run(env: &mut CalcEnv, _build: &Build) {
    // --- Mirage Archer ---
    let has_mirage_archer = env
        .player
        .mod_db
        .flag_cfg("MirageArcher", None, &env.player.output);
    if has_mirage_archer {
        calc_mirage_archer(env);
        return;
    }

    // --- Saviour Reflection ---
    let has_reflection = env
        .player
        .mod_db
        .flag_cfg("SaviourReflection", None, &env.player.output);
    if has_reflection {
        calc_saviour_reflection(env);
        return;
    }

    // --- General's Cry ---
    let has_generals_cry = env
        .player
        .mod_db
        .flag_cfg("GeneralsCry", None, &env.player.output);
    if has_generals_cry {
        calc_generals_cry(env);
    }
}

/// Mirage Archer: copies the player's attack at reduced damage and speed.
/// Mirrors CalcMirages.lua L63-115.
fn calc_mirage_archer(env: &mut CalcEnv) {
    let mirage_count = env
        .player
        .mod_db
        .sum_cfg(ModType::Base, "MirageArcherCount", None, &env.player.output)
        .max(1.0);

    // Less damage multiplier (from Mirage Archer support gem, default ~31% less = 0.69 mult)
    let less_dmg = env.player.mod_db.sum_cfg(
        ModType::Base,
        "MirageArcherLessDamage",
        None,
        &env.player.output,
    );
    let dmg_mult = if less_dmg > 0.0 {
        1.0 - less_dmg / 100.0
    } else {
        0.69
    };

    // Less attack speed multiplier (default ~30% less = 0.7 mult)
    let less_speed = env.player.mod_db.sum_cfg(
        ModType::Base,
        "MirageArcherLessAttackSpeed",
        None,
        &env.player.output,
    );
    let speed_mult = if less_speed > 0.0 {
        1.0 - less_speed / 100.0
    } else {
        0.7
    };

    let player_dps = get_output_f64(&env.player.output, "TotalDPS");
    let mirage_dps = player_dps * dmg_mult * speed_mult * mirage_count;

    env.player.set_output("MirageArcherDPS", mirage_dps);
    env.player.set_output("MirageDPS", mirage_dps);

    let combined = get_output_f64(&env.player.output, "CombinedDPS");
    env.player.set_output("CombinedDPS", combined + mirage_dps);
}

/// The Saviour's Reflection: creates copies that deal less damage.
/// Mirrors CalcMirages.lua L116-173.
fn calc_saviour_reflection(env: &mut CalcEnv) {
    let less_dmg =
        env.player
            .mod_db
            .sum_cfg(ModType::Base, "SaviourLessDamage", None, &env.player.output);
    let dmg_mult = if less_dmg > 0.0 {
        1.0 - less_dmg / 100.0
    } else {
        0.5
    };
    let reflection_count = 2.0;

    let player_dps = get_output_f64(&env.player.output, "TotalDPS");
    let mirage_dps = player_dps * dmg_mult * reflection_count;

    env.player.set_output("MirageDPS", mirage_dps);

    let combined = get_output_f64(&env.player.output, "CombinedDPS");
    env.player.set_output("CombinedDPS", combined + mirage_dps);
}

/// General's Cry: triggers exerted attacks from mirage warriors on cooldown.
/// Mirrors CalcMirages.lua L355-433.
fn calc_generals_cry(env: &mut CalcEnv) {
    let base_cd = 4.0;
    let icdr =
        env.player
            .mod_db
            .sum_cfg(ModType::Inc, "CooldownRecovery", None, &env.player.output);
    let effective_cd =
        super::triggers::align_to_server_tick(super::triggers::apply_icdr(base_cd, icdr));

    let generals_speed = if effective_cd > 0.0 {
        1.0 / effective_cd
    } else {
        0.0
    };

    let mirage_count = env
        .player
        .mod_db
        .sum_cfg(
            ModType::Base,
            "GeneralsCryMirageCount",
            None,
            &env.player.output,
        )
        .max(1.0);

    let player_hit = get_output_f64(&env.player.output, "AverageHit");
    let mirage_dps = player_hit * generals_speed * mirage_count;

    env.player.set_output("MirageDPS", mirage_dps);

    let combined = get_output_f64(&env.player.output, "CombinedDPS");
    env.player.set_output("CombinedDPS", combined + mirage_dps);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::env::OutputValue;
    use crate::{build::parse_xml, calc::setup::init_env, data::GameData};
    use std::sync::Arc;

    #[test]
    fn mirages_no_flag_does_nothing() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Ranger" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="6" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let data = Arc::new(GameData::from_json(crate::tests::stub_game_data_json()).unwrap());
        let mut env = init_env(&build, data).unwrap();
        run(&mut env, &build);
        assert!(
            env.player.output.get("MirageArcherDPS").is_none(),
            "MirageArcherDPS should not be set when no MirageArcher flag"
        );
    }

    #[test]
    fn mirage_archer_with_flag_sets_dps() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Ranger" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="6" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let data = Arc::new(GameData::from_json(crate::tests::stub_game_data_json()).unwrap());
        let mut env = init_env(&build, data).unwrap();
        use crate::mod_db::types::{Mod, ModSource};
        env.player.mod_db.add(Mod::new_flag(
            "MirageArcher",
            ModSource::new("Item", "Mirage Archer"),
        ));
        env.player.set_output("TotalDPS", 1000.0);
        env.player.set_output("CombinedDPS", 1000.0);
        run(&mut env, &build);
        let mirage_dps = match env.player.output.get("MirageArcherDPS") {
            Some(OutputValue::Number(n)) => *n,
            _ => panic!("MirageArcherDPS not set"),
        };
        // Default: 1000 * 0.69 * 0.7 * 1 = 483
        assert!(
            mirage_dps > 400.0 && mirage_dps < 600.0,
            "MirageArcherDPS should be ~483, got {mirage_dps}"
        );
    }

    #[test]
    fn mirage_archer_with_explicit_modifiers() {
        let data = Arc::new(GameData::default_for_test());
        let mut env = CalcEnv::new(
            crate::mod_db::ModDb::new(),
            crate::mod_db::ModDb::new(),
            data,
        );
        use crate::mod_db::types::{Mod, ModSource};
        env.player.mod_db.add(Mod::new_flag(
            "MirageArcher",
            ModSource::new("Item", "Mirage Archer"),
        ));
        env.player.mod_db.add(Mod::new_base(
            "MirageArcherLessDamage",
            40.0,
            ModSource::new("Gem", "MA"),
        ));
        env.player.mod_db.add(Mod::new_base(
            "MirageArcherLessAttackSpeed",
            25.0,
            ModSource::new("Gem", "MA"),
        ));
        env.player.set_output("TotalDPS", 2000.0);
        env.player.set_output("CombinedDPS", 2000.0);
        let build = parse_xml(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Ranger" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="6" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#,
        )
        .unwrap();
        run(&mut env, &build);
        let dps = match env.player.output.get("MirageArcherDPS") {
            Some(OutputValue::Number(n)) => *n,
            _ => panic!("MirageArcherDPS not set"),
        };
        // 2000 * (1-0.4) * (1-0.25) * 1 = 2000 * 0.6 * 0.75 = 900
        assert!(
            (dps - 900.0).abs() < 1.0,
            "MirageArcherDPS should be 900, got {dps}"
        );
    }
}
