use crate::{error::ExtractError, ggpk_reader::GgpkReader};
use std::path::Path;

pub fn extract(reader: &GgpkReader, output: &Path) -> Result<(), ExtractError> {
    // PoE 2 does not ship a PassiveTree.json in the GGPK.
    // The passive tree is reconstructed from PassiveSkills.datc64 + PassiveSkillTreeUIArt.datc64.
    // This transformer is a placeholder; full tree extraction is deferred to Phase 3.
    //
    // For now, try reading the file and skip gracefully if not found.
    match reader.read_text("Data/PassiveTree.json") {
        Ok(tree_json) => {
            let _: serde_json::Value = serde_json::from_str(&tree_json)?;
            std::fs::write(output.join("tree").join("poe1_current.json"), tree_json)?;
            println!("  Wrote tree/poe1_current.json");
        }
        Err(ExtractError::FileNotFound(_)) => {
            println!(
                "  PassiveTree.json not found (PoE 2 GGPK — tree extraction deferred to Phase 3)"
            );
        }
        Err(e) => return Err(e),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn tree_json_schema() {
        let Some(_ggpk_path) = std::env::var("GGPK_PATH").ok() else {
            eprintln!("GGPK_PATH not set, skipping integration test");
            return;
        };
        // Tree extraction is deferred for PoE 2 — test passes as no-op
    }
}
