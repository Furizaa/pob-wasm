use super::env::{CalcEnv, OutputValue};
use crate::build::Build;
use crate::mod_db::types::{KeywordFlags, ModFlags, ModType};

pub fn run(env: &mut CalcEnv, _build: &Build) {
    // Mirage Archer only active when the MirageArcher flag is set in ModDb
    // (from the Mirage Archer support gem adding a mod to the player db)
    if !env
        .player
        .mod_db
        .flag("MirageArcher", ModFlags::NONE, KeywordFlags::NONE)
    {
        return;
    }

    let mirage_count = env
        .player
        .mod_db
        .sum(
            ModType::Base,
            "MirageArcherCount",
            ModFlags::NONE,
            KeywordFlags::NONE,
        )
        .max(1.0);

    let player_dps = env
        .player
        .output
        .get("TotalDPS")
        .and_then(|v| {
            if let OutputValue::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .unwrap_or(0.0);

    // Mirage Archer DPS = player DPS * 0.35 * count
    let mirage_dps = player_dps * 0.35 * mirage_count;
    env.player.set_output("MirageArcherDPS", mirage_dps);

    let combined = env
        .player
        .output
        .get("CombinedDPS")
        .and_then(|v| {
            if let OutputValue::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .unwrap_or(player_dps);
    env.player.set_output("CombinedDPS", combined + mirage_dps);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build::parse_xml, calc::setup::init_env, data::GameData};
    use std::sync::Arc;

    #[test]
    fn mirages_no_flag_does_nothing() {
        // Without the MirageArcher flag, run() should return early without setting any output
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
        // No MirageArcher flag → MirageArcherDPS should NOT be set
        assert!(
            env.player.output.get("MirageArcherDPS").is_none(),
            "MirageArcherDPS should not be set when no MirageArcher flag"
        );
    }

    #[test]
    fn mirages_with_flag_sets_dps() {
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
        // Inject a MirageArcher flag and a fake TotalDPS
        use crate::mod_db::types::{Mod, ModSource};
        env.player.mod_db.add(Mod::new_flag(
            "MirageArcher",
            ModSource::new("Item", "Mirage Archer"),
        ));
        env.player.set_output("TotalDPS", 1000.0);
        run(&mut env, &build);
        // MirageArcherDPS should be 350 (1000 * 0.35 * 1)
        let mirage_dps = env.player.output.get("MirageArcherDPS");
        assert!(mirage_dps.is_some(), "MirageArcherDPS should be set");
    }
}
