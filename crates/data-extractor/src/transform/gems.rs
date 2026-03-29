use crate::{dat64::Dat64, error::ExtractError, ggpk_reader::GgpkReader};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Serialize)]
pub struct GemLevelData {
    pub level: u8,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub phys_min: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub phys_max: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub fire_min: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub fire_max: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub cold_min: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub cold_max: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub lightning_min: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub lightning_max: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub chaos_min: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub chaos_max: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub crit_chance: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub cast_time: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub attack_speed_mult: f64,
}

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

#[derive(Serialize)]
pub struct GemData {
    pub id: String,
    pub display_name: String,
    pub is_support: bool,
    pub skill_types: Vec<u32>,
    /// Per-level damage/timing data. Empty until GGPK offsets are calibrated.
    pub levels: Vec<GemLevelData>,
}

pub fn extract(reader: &GgpkReader, output: &Path) -> Result<(), ExtractError> {
    // ActiveSkillType.dat64: just need row indices for type IDs
    // row_size = 8 (Id string only)
    let ast_bytes = reader.read_bytes("Data/ActiveSkillType.datc64")?;
    let ast = Dat64::parse_datc64(ast_bytes, 24, "ActiveSkillType.datc64")?;
    let mut type_ids: Vec<u32> = Vec::new();
    for i in 0..ast.row_count {
        // Store the 1-based type number (POB uses _rowIndex+1)
        type_ids.push((i + 1) as u32);
    }

    // ActiveSkills.dat64: row_size = 72 (see plan header)
    let as_bytes = reader.read_bytes("Data/ActiveSkills.datc64")?;
    let active_skills = Dat64::parse_datc64(as_bytes, 257, "ActiveSkills.datc64")?;

    let mut gems: HashMap<String, GemData> = HashMap::new();
    for i in 0..active_skills.row_count {
        let id = active_skills.read_string(i, 0);
        if id.is_empty() {
            continue;
        }
        let display_name = active_skills.read_string(i, 8);
        let is_support = active_skills.read_bool(i, 44);
        // Types is a key-array at offset 28 (16 bytes: count + offset)
        let type_row_indices = active_skills.read_key_array(i, 28);
        let skill_types: Vec<u32> = type_row_indices
            .iter()
            .filter_map(|&idx| type_ids.get(idx as usize).copied())
            .collect();

        gems.insert(
            id.clone(),
            GemData {
                id,
                display_name,
                is_support,
                skill_types,
                levels: Vec::new(), // TODO: populate from GemEffects.datc64 when GGPK offsets are calibrated
            },
        );
    }

    let json = serde_json::to_string_pretty(&gems)?;
    std::fs::write(output.join("gems.json"), json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn gems_json_schema() {
        let Some(ggpk_path) = std::env::var("GGPK_PATH").ok() else {
            eprintln!("GGPK_PATH not set, skipping integration test");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let reader =
            crate::ggpk_reader::GgpkReader::open(std::path::Path::new(&ggpk_path)).unwrap();
        super::extract(&reader, tmp.path()).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(tmp.path().join("gems.json")).unwrap())
                .unwrap();
        let obj = json.as_object().expect("gems.json must be an object");
        assert!(!obj.is_empty(), "gems.json must not be empty");
        let fireball = obj.get("Fireball").expect("Fireball gem not found");
        assert_eq!(fireball["is_support"], false);
        assert!(!fireball["skill_types"].as_array().unwrap().is_empty());
    }
}
