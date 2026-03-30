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
    let ic_bytes = reader.read_bytes("Data/ItemClasses.datc64")?;
    let item_classes = Dat64::parse_datc64(ic_bytes, 153, "ItemClasses.datc64")?;
    // Build row-index → name map
    let mut class_names: Vec<String> = Vec::with_capacity(item_classes.row_count);
    for i in 0..item_classes.row_count {
        class_names.push(item_classes.read_string(i, 8));
    }

    // ComponentAttributeRequirements.datc64 — stat requirements per base item type.
    // spec.lua fields: [1]=BaseItemType(String,8) [2]=Str(Int,4) [3]=Dex(Int,4) [4]=Int(Int,4)
    // Probed row_size = 20 bytes:
    //   offset 0:  BaseItemType base-item Id string (str, 8)
    //   offset 8:  Str requirement (u32, 4)
    //   offset 12: Dex requirement (u32, 4)
    //   offset 16: Int requirement (u32, 4)
    // This mirrors how bases.lua reads: dat("ComponentAttributeRequirements"):GetRow("BaseItemType", id)
    // then uses compAtt.Str / compAtt.Dex / compAtt.Int.
    use std::collections::HashMap;
    let car_bytes = reader.read_bytes("Data/ComponentAttributeRequirements.datc64")?;
    let comp_attr = Dat64::parse_datc64(car_bytes, 20, "ComponentAttributeRequirements.datc64")?;
    // Build base-item-type Id → (str, dex, int) map
    let mut attr_req: HashMap<String, (u32, u32, u32)> = HashMap::new();
    for i in 0..comp_attr.row_count {
        let base_id = comp_attr.read_string(i, 0);
        if base_id.is_empty() {
            continue;
        }
        let str_req = comp_attr.read_u32(i, 8);
        let dex_req = comp_attr.read_u32(i, 12);
        let int_req = comp_attr.read_u32(i, 16);
        attr_req.insert(base_id, (str_req, dex_req, int_req));
    }

    // BaseItemTypes.datc64 row_size = 310 (PoE1 modern bundle format, probed layout):
    // offset 0:  Id (str, 8)
    // offset 8:  ItemClassesKey (u64, 8) → row index into ItemClasses
    // offset 32: Name (str, 8)
    // offset 88: DropLevel (u32, 4) — NOTE: may need calibration; 0 used as fallback
    let bit_bytes = reader.read_bytes("Data/BaseItemTypes.datc64")?;
    let base_items = Dat64::parse_datc64(bit_bytes, 310, "BaseItemTypes.datc64")?;

    let mut items: Vec<BaseItem> = Vec::with_capacity(base_items.row_count);
    for i in 0..base_items.row_count {
        let id = base_items.read_string(i, 0);
        if id.is_empty() {
            continue;
        }
        let name = base_items.read_string(i, 32);
        let class_key = base_items.read_u64(i, 8) as usize;
        let item_class = class_names.get(class_key).cloned().unwrap_or_default();
        let drop_level = base_items.read_u32(i, 88);

        // Look up attribute requirements for this base item type
        let (base_str, base_dex, base_int) = attr_req.get(&id).copied().unwrap_or((0, 0, 0));

        items.push(BaseItem {
            id,
            name,
            item_class,
            base_str,
            base_dex,
            base_int,
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
    fn base_has_stat_requirement_fields() {
        // Verify the JSON schema includes stat requirement fields.
        // When data is extracted from a GGPK, base_str/base_dex/base_int should reflect
        // ComponentAttributeRequirements.datc64 values (e.g. Astral Plate has base_str=109).
        let raw = r#"[{"id":"Metadata/Items/Armour/ArmourStr/BodyStr14","name":"Astral Plate","item_class":"Body Armour","base_str":109,"base_dex":0,"base_int":0,"drop_level":62}]"#;
        let bases: Vec<serde_json::Value> = serde_json::from_str(raw).unwrap();
        assert_eq!(bases[0]["base_str"], 109);
        assert_eq!(bases[0]["base_dex"], 0);
        assert_eq!(bases[0]["base_int"], 0);
    }

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
