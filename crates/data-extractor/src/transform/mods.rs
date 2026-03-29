use crate::{dat64::Dat64, error::ExtractError, ggpk_reader::GgpkReader};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Serialize)]
pub struct ModEntry {
    pub id: String,
    pub name: String,
    pub mod_type: String,
    pub domain: u32,
    pub generation_type: u32,
}

pub fn extract(reader: &GgpkReader, output: &Path) -> Result<(), ExtractError> {
    // Mods.datc64 row_size = 654 (PoE1 modern bundle format, probed layout):
    // offset 0:   Id (str, 8)
    // offset 504: Domain (u32, 4)
    // offset 508: GenerationType (u32, 4)
    // Name field not yet calibrated — stored as empty for now
    // NOTE: these offsets may need calibration; see docs/superpowers/plans for Phase 3 notes
    let bytes = reader.read_bytes("Data/Mods.datc64")?;
    let dat = Dat64::parse_datc64(bytes, 654, "Mods.datc64")?;

    let mut mods: HashMap<String, ModEntry> = HashMap::new();
    for i in 0..dat.row_count {
        let id = dat.read_string(i, 0);
        if id.is_empty() {
            continue;
        }
        let name = String::new(); // Name field calibration deferred to Phase 3
        let domain = dat.read_u32(i, 504);
        let generation_type = dat.read_u32(i, 508);

        let mod_type = match generation_type {
            1 => "Prefix",
            2 => "Suffix",
            3 => "Unique",
            4 => "Nemesis",
            5 => "Corrupted",
            6 => "Bloodlines",
            7 => "Torment",
            8 => "Tempest",
            9 => "Talisman",
            10 => "Enchantment",
            11 => "EssenceMonster",
            _ => "Unknown",
        }
        .to_string();

        mods.insert(
            id.clone(),
            ModEntry {
                id,
                name,
                mod_type,
                domain,
                generation_type,
            },
        );
    }

    let json = serde_json::to_string_pretty(&mods)?;
    std::fs::write(output.join("mods.json"), json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn mods_json_schema() {
        let Some(ggpk_path) = std::env::var("GGPK_PATH").ok() else {
            eprintln!("GGPK_PATH not set, skipping integration test");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let reader =
            crate::ggpk_reader::GgpkReader::open(std::path::Path::new(&ggpk_path)).unwrap();
        super::extract(&reader, tmp.path()).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(tmp.path().join("mods.json")).unwrap())
                .unwrap();
        let obj = json.as_object().expect("mods.json must be an object");
        assert!(!obj.is_empty(), "mods.json must not be empty");
        // Spot-check: at least one mod with domain == 1 (item domain)
        let has_item_domain = obj
            .values()
            .any(|v| v.get("domain").and_then(|d| d.as_u64()) == Some(1));
        assert!(has_item_domain, "no item-domain (domain=1) mod found");
    }
}
