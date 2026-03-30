use crate::lua_env::create_pob_lua_env;
use crate::types::{ArmourData, BaseItemData, BaseRequirements, FlaskData, WeaponData};
use mlua::prelude::*;
use std::path::Path;

/// Lua base item files to load from Data/Bases/
const BASE_FILES: &[&str] = &[
    "sword.lua",
    "axe.lua",
    "mace.lua",
    "dagger.lua",
    "claw.lua",
    "staff.lua",
    "wand.lua",
    "bow.lua",
    "body.lua",
    "helmet.lua",
    "gloves.lua",
    "boots.lua",
    "shield.lua",
    "quiver.lua",
    "belt.lua",
    "amulet.lua",
    "ring.lua",
    "flask.lua",
    "jewel.lua",
];

/// Extract tags from a Lua table where keys are tag names and values are `true`.
fn extract_tags(item_table: &LuaTable) -> mlua::Result<Vec<String>> {
    let val: LuaValue = item_table.get("tags")?;
    let mut tags = Vec::new();
    if let LuaValue::Table(tbl) = val {
        for pair in tbl.pairs::<String, LuaValue>() {
            let (k, v) = pair?;
            if let LuaValue::Boolean(true) = v {
                tags.push(k);
            }
        }
    }
    tags.sort();
    Ok(tags)
}

/// Extract the `implicit` field. It can be absent, a single string, or (theoretically) a table.
/// Returns a Vec<String>.
fn extract_implicit(item_table: &LuaTable) -> mlua::Result<Vec<String>> {
    let val: LuaValue = item_table.get("implicit")?;
    match val {
        LuaValue::String(s) => {
            let s = s.to_str()?.to_string();
            if s.is_empty() {
                Ok(Vec::new())
            } else {
                Ok(vec![s])
            }
        }
        LuaValue::Table(tbl) => {
            let mut implicits = Vec::new();
            for pair in tbl.pairs::<LuaValue, LuaValue>() {
                let (_, v) = pair?;
                if let LuaValue::String(s) = v {
                    implicits.push(s.to_str()?.to_string());
                }
            }
            Ok(implicits)
        }
        _ => Ok(Vec::new()),
    }
}

/// Extract weapon data from the `weapon` sub-table.
fn extract_weapon(item_table: &LuaTable) -> mlua::Result<Option<WeaponData>> {
    let val: LuaValue = item_table.get("weapon")?;
    match val {
        LuaValue::Table(tbl) => Ok(Some(WeaponData {
            physical_min: tbl.get("PhysicalMin").unwrap_or(0.0),
            physical_max: tbl.get("PhysicalMax").unwrap_or(0.0),
            crit_chance_base: tbl.get("CritChanceBase").unwrap_or(0.0),
            attack_rate_base: tbl.get("AttackRateBase").unwrap_or(0.0),
            range: tbl.get("Range").unwrap_or(0),
        })),
        _ => Ok(None),
    }
}

/// Extract armour data from the `armour` sub-table.
fn extract_armour(item_table: &LuaTable) -> mlua::Result<Option<ArmourData>> {
    let val: LuaValue = item_table.get("armour")?;
    match val {
        LuaValue::Table(tbl) => Ok(Some(ArmourData {
            armour_min: tbl.get("ArmourBaseMin").unwrap_or(0.0),
            armour_max: tbl.get("ArmourBaseMax").unwrap_or(0.0),
            evasion_min: tbl.get("EvasionBaseMin").unwrap_or(0.0),
            evasion_max: tbl.get("EvasionBaseMax").unwrap_or(0.0),
            energy_shield_min: tbl.get("EnergyShieldBaseMin").unwrap_or(0.0),
            energy_shield_max: tbl.get("EnergyShieldBaseMax").unwrap_or(0.0),
            ward_min: tbl.get("WardBaseMin").unwrap_or(0.0),
            ward_max: tbl.get("WardBaseMax").unwrap_or(0.0),
            block_chance: tbl.get("BlockChance").unwrap_or(0),
            movement_penalty: tbl.get("MovementPenalty").unwrap_or(0),
        })),
        _ => Ok(None),
    }
}

/// Extract flask data from the `flask` sub-table.
fn extract_flask(item_table: &LuaTable) -> mlua::Result<Option<FlaskData>> {
    let val: LuaValue = item_table.get("flask")?;
    match val {
        LuaValue::Table(tbl) => Ok(Some(FlaskData {
            life: tbl.get("life").unwrap_or(0.0),
            mana: tbl.get("mana").unwrap_or(0.0),
            duration: tbl.get("duration").unwrap_or(0.0),
            charges_used: tbl.get("chargesUsed").unwrap_or(0),
            charges_max: tbl.get("chargesMax").unwrap_or(0),
        })),
        _ => Ok(None),
    }
}

/// Extract requirements from the `req` sub-table.
fn extract_requirements(item_table: &LuaTable) -> mlua::Result<Option<BaseRequirements>> {
    let val: LuaValue = item_table.get("req")?;
    match val {
        LuaValue::Table(tbl) => {
            let req = BaseRequirements {
                level: tbl.get("level").unwrap_or(0),
                str_req: tbl.get("str").unwrap_or(0),
                dex_req: tbl.get("dex").unwrap_or(0),
                int_req: tbl.get("int").unwrap_or(0),
            };
            // Only include if at least one field is non-zero
            if req.level == 0 && req.str_req == 0 && req.dex_req == 0 && req.int_req == 0 {
                Ok(None)
            } else {
                Ok(Some(req))
            }
        }
        _ => Ok(None),
    }
}

/// Parse a single base item entry from the Lua table into BaseItemData.
fn parse_base_item(name: &str, item_table: &LuaTable) -> mlua::Result<BaseItemData> {
    let item_type: String = item_table.get("type")?;
    let sub_type: Option<String> = item_table.get("subType")?;
    let socket_limit: u32 = item_table.get("socketLimit").unwrap_or(0);

    let tags = extract_tags(item_table)?;
    let implicit = extract_implicit(item_table)?;
    let weapon = extract_weapon(item_table)?;
    let armour = extract_armour(item_table)?;
    let flask = extract_flask(item_table)?;
    let req = extract_requirements(item_table)?;

    Ok(BaseItemData {
        name: name.to_string(),
        item_type,
        sub_type,
        socket_limit,
        tags,
        implicit,
        weapon,
        armour,
        flask,
        req,
    })
}

pub fn extract(pob_src: &str, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let lua = create_pob_lua_env(pob_src)?;
    let bases_dir = format!("{}/Data/Bases", pob_src);

    let mut all_bases: Vec<BaseItemData> = Vec::new();

    for filename in BASE_FILES {
        let filepath = format!("{}/{}", bases_dir, filename);
        let code = match std::fs::read_to_string(&filepath) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  Warning: could not read {}: {}", filepath, e);
                continue;
            }
        };

        // Create a fresh table to receive the base items via vararg
        let item_bases_table = lua.create_table()?;

        // Each file starts with `local itemBases = ...` so we pass the table as vararg
        let chunk = lua.load(&code);
        match chunk.call::<()>(item_bases_table.clone()) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("  Warning: failed to evaluate {}: {}", filename, e);
                continue;
            }
        }

        // Iterate the populated table
        let mut file_count = 0u32;
        for pair in item_bases_table.pairs::<String, LuaValue>() {
            let (name, value) = pair?;
            if let LuaValue::Table(entry) = value {
                match parse_base_item(&name, &entry) {
                    Ok(base) => {
                        all_bases.push(base);
                        file_count += 1;
                    }
                    Err(e) => {
                        eprintln!("  Warning: failed to parse base '{}': {}", name, e);
                    }
                }
            }
        }
        println!("  Loaded {} ({} items)", filename, file_count);
    }

    // Sort by name for stable output
    all_bases.sort_by(|a, b| a.name.cmp(&b.name));

    println!("  Extracted {} base items total", all_bases.len());

    // Write output
    let out_path = output.join("bases.json");
    let json = serde_json::to_string_pretty(&all_bases)?;
    std::fs::write(&out_path, json)?;
    println!("  Written to {}", out_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pob_src_dir() -> String {
        std::env::var("POB_SRC").unwrap_or_else(|_| {
            let workspace = env!("CARGO_MANIFEST_DIR");
            format!("{}/../../third-party/PathOfBuilding/src", workspace)
        })
    }

    #[test]
    fn extract_produces_bases() {
        let pob_src = pob_src_dir();
        let tmp = tempfile::tempdir().unwrap();
        extract(&pob_src, tmp.path()).unwrap();

        let json_path = tmp.path().join("bases.json");
        assert!(json_path.exists(), "bases.json should be created");

        let data: Vec<BaseItemData> =
            serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();

        // At least 200 base items total
        assert!(
            data.len() >= 200,
            "Expected at least 200 base items, got {}",
            data.len()
        );

        // "Rusted Sword" exists with weapon data
        let rusted_sword = data
            .iter()
            .find(|b| b.name == "Rusted Sword")
            .expect("Rusted Sword should exist");
        assert_eq!(rusted_sword.item_type, "One Handed Sword");
        let weapon = rusted_sword
            .weapon
            .as_ref()
            .expect("should have weapon data");
        assert!(
            weapon.physical_min > 0.0,
            "Rusted Sword physical_min should be > 0"
        );
        assert!(
            weapon.attack_rate_base > 0.0,
            "Rusted Sword attack_rate_base should be > 0"
        );
        assert!(
            !rusted_sword.implicit.is_empty(),
            "Rusted Sword should have an implicit"
        );

        // At least one body armour ("Plate Vest") with armour data
        let plate_vest = data
            .iter()
            .find(|b| b.name == "Plate Vest")
            .expect("Plate Vest should exist");
        assert_eq!(plate_vest.item_type, "Body Armour");
        let armour = plate_vest.armour.as_ref().expect("should have armour data");
        assert!(
            armour.armour_min > 0.0,
            "Plate Vest armour_min should be > 0"
        );

        // At least one shield with block_chance > 0
        let shield_with_block = data
            .iter()
            .find(|b| {
                b.item_type == "Shield"
                    && b.armour
                        .as_ref()
                        .map(|a| a.block_chance > 0)
                        .unwrap_or(false)
            })
            .expect("Should have at least one shield with block_chance > 0");
        assert!(shield_with_block.armour.as_ref().unwrap().block_chance > 0);

        // At least one flask with duration > 0
        let flask_with_duration = data
            .iter()
            .find(|b| b.flask.as_ref().map(|f| f.duration > 0.0).unwrap_or(false))
            .expect("Should have at least one flask with duration > 0");
        assert!(flask_with_duration.flask.as_ref().unwrap().duration > 0.0);
    }
}
