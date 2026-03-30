use crate::types::{ItemModData, ItemModStat};
use mlua::prelude::*;
use std::path::Path;

/// Mod source files and their domain mapping.
/// Files with `keyed = true` use `["ModId"] = { ... }` format.
/// Files with `keyed = false` use `{ ... }` (numeric array) format.
const MOD_FILES: &[(&str, &str, bool)] = &[
    ("ModItem.lua", "item", true),
    ("ModFlask.lua", "flask", true),
    ("ModJewel.lua", "jewel", true),
    ("ModJewelAbyss.lua", "jewel_abyss", true),
    ("ModJewelCluster.lua", "jewel_cluster", true),
    ("ModMaster.lua", "crafted", false),
    ("ModVeiled.lua", "veiled", true),
];

/// Extract stat text strings from a mod entry table.
///
/// Stats are stored as positional (integer-indexed) string values.
/// We iterate integer keys starting from 1 and collect all strings.
fn extract_stats(entry: &LuaTable) -> mlua::Result<Vec<ItemModStat>> {
    let mut stats = Vec::new();
    let mut idx = 1i64;
    loop {
        let val: LuaValue = entry.raw_get(idx)?;
        match val {
            LuaValue::String(s) => {
                stats.push(ItemModStat {
                    stat_id: s.to_str()?.to_string(),
                    min: 0,
                    max: 0,
                });
            }
            LuaValue::Nil => break,
            _ => {
                // Non-string positional value — skip but keep going
                // (shouldn't normally happen)
            }
        }
        idx += 1;
    }
    Ok(stats)
}

/// Parse a single mod entry table into an `ItemModData`.
fn parse_mod_entry(id: &str, entry: &LuaTable, domain: &str) -> mlua::Result<ItemModData> {
    let mod_type: String = entry.get("type").unwrap_or_default();
    let group: Option<String> = entry.get("group").ok();
    let level: u32 = entry.get("level").unwrap_or(0);
    let stats = extract_stats(entry)?;

    Ok(ItemModData {
        id: id.to_string(),
        mod_type: mod_type.clone(),
        domain: domain.to_string(),
        generation_type: mod_type,
        stats,
        group,
        level_requirement: level,
    })
}

pub fn extract(pob_src: &str, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let lua = mlua::Lua::new();
    let data_dir = format!("{}/Data", pob_src);

    let mut all_mods: Vec<ItemModData> = Vec::new();

    for &(filename, domain, keyed) in MOD_FILES {
        let filepath = format!("{}/{}", data_dir, filename);
        let code = match std::fs::read_to_string(&filepath) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  Warning: could not read {}: {}", filepath, e);
                continue;
            }
        };

        let table: LuaTable = match lua.load(&code).eval() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("  Warning: failed to evaluate {}: {}", filename, e);
                continue;
            }
        };

        let mut file_count = 0u32;

        if keyed {
            // String-keyed format: ["ModId"] = { type = ..., "stat text", ... }
            for pair in table.pairs::<String, LuaValue>() {
                let (mod_id, value) = pair?;
                if let LuaValue::Table(entry) = value {
                    let full_id = format!("{}:{}", domain, mod_id);
                    match parse_mod_entry(&full_id, &entry, domain) {
                        Ok(mod_data) => {
                            all_mods.push(mod_data);
                            file_count += 1;
                        }
                        Err(e) => {
                            eprintln!(
                                "  Warning: failed to parse mod '{}' in {}: {}",
                                mod_id, filename, e
                            );
                        }
                    }
                }
            }
        } else {
            // Numeric array format: { { type = ..., "stat text", ... }, ... }
            // No explicit mod ID key — generate one from domain + index.
            for pair in table.pairs::<i64, LuaValue>() {
                let (idx, value) = pair?;
                if let LuaValue::Table(entry) = value {
                    let group: Option<String> = entry.get("group").ok();
                    let synthetic_id = if let Some(ref g) = group {
                        format!("{}:{}_{}", domain, g, idx)
                    } else {
                        format!("{}:entry_{}", domain, idx)
                    };
                    match parse_mod_entry(&synthetic_id, &entry, domain) {
                        Ok(mod_data) => {
                            all_mods.push(mod_data);
                            file_count += 1;
                        }
                        Err(e) => {
                            eprintln!(
                                "  Warning: failed to parse mod index {} in {}: {}",
                                idx, filename, e
                            );
                        }
                    }
                }
            }
        }

        println!("  Loaded {} ({} mods)", filename, file_count);
    }

    // Sort by id for stable output
    all_mods.sort_by(|a, b| a.id.cmp(&b.id));

    println!("  Extracted {} mods total", all_mods.len());

    // Write output
    let out_path = output.join("mods.json");
    let json = serde_json::to_string_pretty(&all_mods)?;
    std::fs::write(&out_path, json)?;
    println!("  Written to {}", out_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ItemModData;

    fn pob_src_dir() -> String {
        std::env::var("POB_SRC").unwrap_or_else(|_| {
            let workspace = env!("CARGO_MANIFEST_DIR");
            format!("{}/../../third-party/PathOfBuilding/src", workspace)
        })
    }

    #[test]
    fn extract_produces_mods() {
        let pob_src = pob_src_dir();
        let tmp = tempfile::tempdir().unwrap();
        extract(&pob_src, tmp.path()).unwrap();

        let json_path = tmp.path().join("mods.json");
        assert!(json_path.exists(), "mods.json should be created");

        let data: Vec<ItemModData> =
            serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();

        // At least 1000 mod entries total
        assert!(
            data.len() >= 1000,
            "Expected at least 1000 mods, got {}",
            data.len()
        );

        // Verify structure: every mod has non-empty id, mod_type, domain
        for m in &data {
            assert!(!m.id.is_empty(), "mod id should not be empty");
            assert!(
                !m.mod_type.is_empty(),
                "mod_type should not be empty for mod {}",
                m.id
            );
            assert!(
                !m.domain.is_empty(),
                "domain should not be empty for mod {}",
                m.id
            );
        }

        // At least some mods from the "item" domain
        let item_mods: Vec<_> = data.iter().filter(|m| m.domain == "item").collect();
        assert!(
            item_mods.len() >= 100,
            "Expected at least 100 item-domain mods, got {}",
            item_mods.len()
        );

        // Verify some item mods have stats
        let mods_with_stats: Vec<_> = item_mods.iter().filter(|m| !m.stats.is_empty()).collect();
        assert!(
            mods_with_stats.len() >= 100,
            "Expected at least 100 item mods with stats, got {}",
            mods_with_stats.len()
        );

        // Verify a known mod exists: Strength1 from ModItem.lua
        let strength1 = data
            .iter()
            .find(|m| m.id == "item:Strength1")
            .expect("item:Strength1 should exist");
        assert_eq!(strength1.mod_type, "Suffix");
        assert_eq!(strength1.domain, "item");
        assert_eq!(strength1.level_requirement, 1);
        assert!(strength1.group.as_deref() == Some("Strength"));
        assert!(!strength1.stats.is_empty());
        assert!(
            strength1.stats[0].stat_id.contains("Strength"),
            "First stat should mention Strength, got: {}",
            strength1.stats[0].stat_id
        );

        // Verify flask domain has mods
        let flask_mods: Vec<_> = data.iter().filter(|m| m.domain == "flask").collect();
        assert!(!flask_mods.is_empty(), "Should have flask-domain mods");

        // Verify crafted domain has mods (from ModMaster.lua)
        let crafted_mods: Vec<_> = data.iter().filter(|m| m.domain == "crafted").collect();
        assert!(!crafted_mods.is_empty(), "Should have crafted-domain mods");

        // Verify jewel domain has mods
        let jewel_mods: Vec<_> = data.iter().filter(|m| m.domain == "jewel").collect();
        assert!(!jewel_mods.is_empty(), "Should have jewel-domain mods");

        // Verify veiled domain has mods
        let veiled_mods: Vec<_> = data.iter().filter(|m| m.domain == "veiled").collect();
        assert!(!veiled_mods.is_empty(), "Should have veiled-domain mods");

        println!("Total mods: {}", data.len());
        println!("  item: {}", item_mods.len());
        println!("  flask: {}", flask_mods.len());
        println!("  jewel: {}", jewel_mods.len());
        println!("  crafted: {}", crafted_mods.len());
        println!("  veiled: {}", veiled_mods.len());
    }
}
