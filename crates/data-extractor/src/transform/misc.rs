use crate::{dat64::Dat64, error::ExtractError, ggpk_reader::GgpkReader};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct MiscData {
    game_constants: std::collections::HashMap<String, f64>,
    character_constants: std::collections::HashMap<String, f64>,
    monster_life_table: Vec<i32>,
    monster_damage_table: Vec<f64>,
    monster_evasion_table: Vec<i32>,
    monster_accuracy_table: Vec<i32>,
    monster_ally_life_table: Vec<i32>,
    monster_ally_damage_table: Vec<f64>,
    monster_ailment_threshold_table: Vec<i32>,
    monster_phys_conversion_multi_table: Vec<i32>,
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
        Vec<f64>,
        Vec<i32>,
        Vec<i32>,
        Vec<i32>,
        Vec<f64>,
        Vec<i32>,
        Vec<i32>,
    ),
    ExtractError,
> {
    // DefaultMonsterStats.datc64 row_size = 72 (PoE1 modern bundle format)
    //
    // Schema (from PoB Export/spec.lua "defaultmonsterstats"):
    //   [1] Level               String  offset 0   (8 bytes ptr)
    //   [2] Damage              Float   offset 8   (4 bytes)
    //   [3] Evasion             Int     offset 12  (4 bytes)
    //   [4] Accuracy            Int     offset 16  (4 bytes)
    //   [5] MonsterLife         Int     offset 20  (4 bytes)
    //   [6] XP                  Int     offset 24  (4 bytes)
    //   [7] MinionLife          Int     offset 28  (4 bytes)
    //   [8] Armour              Int     offset 32  (4 bytes)
    //   [9] ResistancePart      Int     offset 36  (4 bytes)
    //  [10] MinionDamage        Float   offset 40  (4 bytes)
    //  [11] AltLife1            Int     offset 44  (4 bytes)
    //  [12] AltDamage1          Float   offset 48  (4 bytes)
    //  [13] AltDamage2          Float   offset 52  (4 bytes)
    //  [14] AltLife2            Int     offset 56  (4 bytes)
    //  [15] EvasiveEvasion      Int     offset 60  (4 bytes)
    //  [16] AilmentThreshold    Int     offset 64  (4 bytes)
    //  [17] PhysConversionMulti Int     offset 68  (4 bytes)
    //  Total = 72 bytes per row
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
        damage.push(dat.read_f32(i, 8) as f64); // Damage (Float)
        evasion.push(dat.read_u32(i, 12) as i32); // Evasion (Int)
        accuracy.push(dat.read_u32(i, 16) as i32); // Accuracy (Int)
        life.push(dat.read_u32(i, 20) as i32); // MonsterLife (Int)
        ally_life.push(dat.read_u32(i, 28) as i32); // MinionLife (Int)
        ally_damage.push(dat.read_f32(i, 40) as f64); // MinionDamage (Float)
        ailment.push(dat.read_u32(i, 64) as i32); // AilmentThreshold (Int)
        phys_conv.push(dat.read_u32(i, 68) as i32); // MonsterPhysConversionMulti (Int)
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
