use crate::{error::ExtractError, ggpk_reader::GgpkReader};
use std::path::Path;

pub fn extract(reader: &GgpkReader, output: &Path) -> Result<(), ExtractError> {
    // PoE 1 current tree — stored as a JSON file inside the GGPK
    let tree_json = reader.read_text("Data/PassiveTree.json")?;
    // Validate it parses as JSON
    let _: serde_json::Value = serde_json::from_str(&tree_json)?;
    std::fs::write(output.join("tree").join("poe1_current.json"), tree_json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn tree_json_schema() {
        let Some(ggpk_path) = std::env::var("GGPK_PATH").ok() else {
            eprintln!("GGPK_PATH not set, skipping integration test");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("tree")).unwrap();
        let reader =
            crate::ggpk_reader::GgpkReader::open(std::path::Path::new(&ggpk_path)).unwrap();
        super::extract(&reader, tmp.path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("tree").join("poe1_current.json")).unwrap(),
        )
        .unwrap();
        assert!(
            json.get("nodes").is_some(),
            "missing 'nodes' key in tree JSON"
        );
    }
}
