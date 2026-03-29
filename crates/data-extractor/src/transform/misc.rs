use crate::{dat64::Dat64, error::ExtractError, ggpk_reader::GgpkReader};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct MiscData {
    game_constants: std::collections::HashMap<String, f64>,
    character_constants: std::collections::HashMap<String, f64>,
    monster_life_table: Vec<i32>,
    monster_damage_table: Vec<i32>,
    monster_evasion_table: Vec<i32>,
    monster_accuracy_table: Vec<i32>,
    monster_ally_life_table: Vec<i32>,
    monster_ally_damage_table: Vec<i32>,
    monster_ailment_threshold_table: Vec<i32>,
    monster_phys_conversion_multi_table: Vec<f32>,
}

pub fn extract(reader: &GgpkReader, output: &Path) -> Result<(), ExtractError> {
    let game_constants = extract_game_constants(reader)?;
    let character_constants = extract_character_constants(reader)?;
    let (
        monster_life_table,
        monster_damage_table,
        monster_evasion_table,
        monster_accuracy_table,
        monster_ally_life_table,
        monster_ally_damage_table,
        monster_ailment_threshold_table,
        monster_phys_conversion_multi_table,
    ) = extract_monster_stats(reader)?;

    let data = MiscData {
        game_constants,
        character_constants,
        monster_life_table,
        monster_damage_table,
        monster_evasion_table,
        monster_accuracy_table,
        monster_ally_life_table,
        monster_ally_damage_table,
        monster_ailment_threshold_table,
        monster_phys_conversion_multi_table,
    };

    let json = serde_json::to_string_pretty(&data)?;
    std::fs::write(output.join("misc.json"), json)?;
    Ok(())
}

fn extract_game_constants(
    reader: &GgpkReader,
) -> Result<std::collections::HashMap<String, f64>, ExtractError> {
    // GameConstants.dat64: Id(str,8) Value(i32,4) Divisor(i32,4) = row_size 16
    let bytes = reader.read_bytes("Data/GameConstants.datc64")?;
    let dat = Dat64::parse_datc64(bytes, 16, "GameConstants.datc64")?;
    let mut map = std::collections::HashMap::new();
    for i in 0..dat.row_count {
        let id = dat.read_string(i, 0);
        let value = dat.read_u32(i, 8) as i32;
        let divisor = dat.read_u32(i, 12) as i32;
        if divisor != 0 {
            map.insert(id, value as f64 / divisor as f64);
        } else {
            map.insert(id, value as f64);
        }
    }
    Ok(map)
}

fn extract_character_constants(
    reader: &GgpkReader,
) -> Result<std::collections::HashMap<String, f64>, ExtractError> {
    // Character.ot is a text file with key = value lines inside Stats{} and Pathfinding{} blocks
    let text = reader.read_text("Metadata/Characters/Character.ot")?;
    let mut map = std::collections::HashMap::new();
    let mut in_block = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Stats") || trimmed.starts_with("Pathfinding") {
            in_block = true;
        } else if trimmed == "}" {
            in_block = false;
        } else if in_block {
            if let Some((key, val)) = trimmed.split_once('=') {
                let key = key.trim().to_string();
                let val = val.trim().trim_end_matches(';').trim();
                if let Ok(n) = val.parse::<f64>() {
                    map.insert(key, n);
                }
            }
        }
    }
    Ok(map)
}

fn extract_monster_stats(
    reader: &GgpkReader,
) -> Result<
    (
        Vec<i32>,
        Vec<i32>,
        Vec<i32>,
        Vec<i32>,
        Vec<i32>,
        Vec<i32>,
        Vec<i32>,
        Vec<f32>,
    ),
    ExtractError,
> {
    // DefaultMonsterStats.datc64 row_size = 72 (PoE2)
    // PoE2 layout: offset 0-7 = string ptr (Id), then numeric fields start at 8
    // Probed u32 values at level 1: off12=67, off16=14, off20=22, off24=20, off28=15, off32=16
    // These are best-guess assignments — calibration needed in Phase 3
    // NOTE: exact field positions differ from PoE1 spec
    let bytes = reader.read_bytes("Data/DefaultMonsterStats.datc64")?;
    let dat = Dat64::parse_datc64(bytes, 72, "DefaultMonsterStats.datc64")?;
    let mut life = Vec::new();
    let mut damage = Vec::new();
    let mut evasion = Vec::new();
    let mut accuracy = Vec::new();
    let mut ally_life = Vec::new();
    let mut ally_damage = Vec::new();
    let mut ailment = Vec::new();
    let mut phys_conv = Vec::new();
    for i in 0..dat.row_count {
        life.push(dat.read_u32(i, 12) as i32); // MonsterLife (probed)
        evasion.push(dat.read_u32(i, 16) as i32); // MonsterEvasion (probed)
        accuracy.push(dat.read_u32(i, 20) as i32); // MonsterAccuracy (probed)
        damage.push(dat.read_u32(i, 24) as i32); // MonsterDamage (probed)
        ally_life.push(dat.read_u32(i, 28) as i32);
        ally_damage.push(dat.read_u32(i, 32) as i32);
        ailment.push(dat.read_u32(i, 36) as i32);
        phys_conv.push(dat.read_f32(i, 40));
    }
    Ok((
        life,
        damage,
        evasion,
        accuracy,
        ally_life,
        ally_damage,
        ailment,
        phys_conv,
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn misc_json_schema() {
        let Some(ggpk_path) = std::env::var("GGPK_PATH").ok() else {
            eprintln!("GGPK_PATH not set, skipping integration test");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let reader =
            crate::ggpk_reader::GgpkReader::open(std::path::Path::new(&ggpk_path)).unwrap();
        super::extract(&reader, tmp.path()).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(tmp.path().join("misc.json")).unwrap())
                .unwrap();
        assert!(
            json.get("game_constants").is_some(),
            "missing game_constants"
        );
        assert!(
            json.get("monster_life_table").is_some(),
            "missing monster_life_table"
        );
        assert!(
            json.get("character_constants").is_some(),
            "missing character_constants"
        );
    }
}
