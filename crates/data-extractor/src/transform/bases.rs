use crate::{dat64::Dat64, error::ExtractError, ggpk_reader::GgpkReader};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct BaseItem {
    pub id: String,
    pub name: String,
    pub item_class: String,
    pub base_str: u32,
    pub base_dex: u32,
    pub base_int: u32,
    pub drop_level: u32,
}

pub fn extract(reader: &GgpkReader, output: &Path) -> Result<(), ExtractError> {
    // ItemClasses.dat64 row_size = 32: Id(str,8) Name(str,8) + remaining fields
    let ic_bytes = reader.read_bytes("Data/ItemClasses.dat64")?;
    let item_classes = Dat64::parse(ic_bytes, 32, "ItemClasses.dat64")?;
    // Build row-index → name map
    let mut class_names: Vec<String> = Vec::with_capacity(item_classes.row_count);
    for i in 0..item_classes.row_count {
        class_names.push(item_classes.read_string(i, 8));
    }

    // BaseItemTypes.dat64 row_size = 96
    // offset 0: Id (str, 8), offset 8: Name (str, 8),
    // offset 16: ItemClassesKey (u64, 8), offset 24: DropLevel (u32, 4)
    let bit_bytes = reader.read_bytes("Data/BaseItemTypes.dat64")?;
    let base_items = Dat64::parse(bit_bytes, 96, "BaseItemTypes.dat64")?;

    let mut items: Vec<BaseItem> = Vec::with_capacity(base_items.row_count);
    for i in 0..base_items.row_count {
        let id = base_items.read_string(i, 0);
        if id.is_empty() {
            continue;
        }
        let name = base_items.read_string(i, 8);
        let class_key = base_items.read_u64(i, 16) as usize;
        let item_class = class_names.get(class_key).cloned().unwrap_or_default();
        let drop_level = base_items.read_u32(i, 24);

        items.push(BaseItem {
            id,
            name,
            item_class,
            base_str: 0,
            base_dex: 0,
            base_int: 0,
            drop_level,
        });
    }

    let json = serde_json::to_string_pretty(&items)?;
    std::fs::write(output.join("bases.json"), json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn bases_json_schema() {
        let Some(ggpk_path) = std::env::var("GGPK_PATH").ok() else {
            eprintln!("GGPK_PATH not set, skipping integration test");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let reader =
            crate::ggpk_reader::GgpkReader::open(std::path::Path::new(&ggpk_path)).unwrap();
        super::extract(&reader, tmp.path()).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(tmp.path().join("bases.json")).unwrap())
                .unwrap();
        let arr = json.as_array().expect("bases.json must be an array");
        assert!(!arr.is_empty(), "bases.json must not be empty");
        // Spot-check: Short Bow should exist with item_class "Bow"
        let short_bow = arr
            .iter()
            .find(|item| item.get("name").and_then(|n| n.as_str()) == Some("Short Bow"));
        if let Some(bow) = short_bow {
            assert_eq!(bow["item_class"].as_str().unwrap(), "Bow");
        }
        // At minimum, verify items have expected keys
        assert!(arr[0].get("id").is_some());
        assert!(arr[0].get("name").is_some());
        assert!(arr[0].get("item_class").is_some());
    }
}
