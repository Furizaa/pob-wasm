use crate::types::UniqueItemData;
use mlua::prelude::*;
use regex::Regex;
use std::path::Path;

/// Main unique item files to load from Data/Uniques/
const UNIQUE_FILES: &[&str] = &[
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

/// Metadata header prefixes that should be skipped (not counted as mods).
/// These appear between the base type line and the "Implicits: N" line.
const SKIP_PREFIXES: &[&str] = &[
    "Variant:",
    "Selected Variant:",
    "Selected Alt Variant:",
    "Selected Alt Variant Two:",
    "Selected Alt Variant Three:",
    "League:",
    "Has Alt Variant:",
    "Has Alt Variant Two:",
    "Has Alt Variant Three:",
    "Crafted:",
    "Source:",
    "LevelReq:",
    "Quality:",
    "Sockets:",
    "Upgrade:",
    "Requires ",
    "Requires:",
    "Radius:",
    "Limited to:",
];

/// Strip curly-brace prefixes like `{variant:1,2}`, `{range:0.5}`, `{tags:foo}` from a mod line.
fn strip_curly_prefixes(line: &str) -> &str {
    let mut s = line;
    while s.starts_with('{') {
        if let Some(end) = s.find('}') {
            s = &s[end + 1..];
        } else {
            break;
        }
    }
    s
}

/// Returns true if the line is a metadata header that should be skipped.
fn is_skip_header(line: &str) -> bool {
    SKIP_PREFIXES.iter().any(|prefix| line.starts_with(prefix))
}

/// Parse a single unique item text block into a `UniqueItemData`.
/// Returns `None` for empty/invalid blocks.
fn parse_unique_text(text: &str) -> Option<UniqueItemData> {
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.len() < 2 {
        return None;
    }

    let name = lines[0].to_string();
    let base_type = lines[1].to_string();

    let mut variants: Vec<String> = Vec::new();
    let mut implicits: Vec<String> = Vec::new();
    let mut explicits: Vec<String> = Vec::new();

    let implicit_re = Regex::new(r"^Implicits:\s*(\d+)$").unwrap();

    // Process lines starting from index 2
    let mut idx = 2;

    // Scan metadata headers until we find "Implicits: N"
    let implicit_count;
    loop {
        if idx >= lines.len() {
            // No "Implicits:" line found — treat remaining lines as explicits with 0 implicits
            implicit_count = 0;
            idx = 2;
            // Re-scan: skip metadata headers, rest are explicits
            while idx < lines.len() {
                let line = lines[idx];
                if is_skip_header(line) || line.starts_with("Variant:") {
                    idx += 1;
                    continue;
                }
                break;
            }
            break;
        }

        let line = lines[idx];

        if let Some(caps) = implicit_re.captures(line) {
            implicit_count = caps[1].parse::<usize>().unwrap_or(0);
            idx += 1;
            break;
        }

        if line.starts_with("Variant:") {
            // Extract variant name
            let variant_name = line.strip_prefix("Variant:").unwrap().trim().to_string();
            if !variant_name.is_empty() {
                variants.push(variant_name);
            }
            idx += 1;
            continue;
        }

        if is_skip_header(line) {
            idx += 1;
            continue;
        }

        // If we reach a line that isn't a header and isn't "Implicits:", this block
        // has no Implicits line. Treat everything from here as explicits.
        implicit_count = 0;
        break;
    }

    // Read implicit_count lines as implicits
    for _ in 0..implicit_count {
        if idx >= lines.len() {
            break;
        }
        let stripped = strip_curly_prefixes(lines[idx]);
        if !stripped.is_empty() {
            implicits.push(stripped.to_string());
        }
        idx += 1;
    }

    // Remaining lines are explicits
    while idx < lines.len() {
        let stripped = strip_curly_prefixes(lines[idx]);
        if !stripped.is_empty() {
            explicits.push(stripped.to_string());
        }
        idx += 1;
    }

    Some(UniqueItemData {
        name,
        base_type,
        implicits,
        explicits,
        variants,
    })
}

/// Try to extract an array of strings from a Lua table.
/// Returns None if the table isn't a sequential array of strings.
fn extract_string_array(table: &LuaTable) -> Option<Vec<String>> {
    let mut strings = Vec::new();
    for pair in table.pairs::<LuaValue, LuaValue>() {
        let (key, value) = pair.ok()?;
        // Only accept integer-keyed entries (sequential array)
        match key {
            LuaValue::Integer(_) => {}
            _ => return None,
        }
        match value {
            LuaValue::String(s) => {
                strings.push(s.to_str().ok()?.to_string());
            }
            _ => {
                // If the first non-integer-key value is not a string, this isn't a text array
                return None;
            }
        }
    }
    Some(strings)
}

/// Load unique items from a single Lua file that uses `return { [[...]], ... }` format.
fn load_uniques_from_return_file(
    lua: &Lua,
    filepath: &str,
) -> Result<Vec<UniqueItemData>, Box<dyn std::error::Error>> {
    let code = std::fs::read_to_string(filepath)?;
    let table: LuaTable = lua.load(&code).eval()?;

    let strings = extract_string_array(&table).unwrap_or_default();
    let mut items = Vec::new();
    for text in &strings {
        if let Some(item) = parse_unique_text(text) {
            items.push(item);
        }
    }
    Ok(items)
}

/// Load unique items from a Special file that uses `data.uniques.xxx = { ... }` format.
/// We set up a `data.uniques` table and eval the file, then extract the assigned table.
fn load_uniques_from_data_assign_file(
    filepath: &str,
) -> Result<Vec<UniqueItemData>, Box<dyn std::error::Error>> {
    let code = std::fs::read_to_string(filepath)?;

    // Check if this file assigns to data.uniques.*
    if !code.contains("data.uniques.") {
        return Ok(Vec::new());
    }

    let lua = Lua::new();

    // Set up data.uniques table structure
    lua.load(
        r#"
        data = { uniques = {} }
    "#,
    )
    .exec()?;

    lua.load(&code).exec()?;

    // Iterate over data.uniques.* tables
    let data: LuaTable = lua.globals().get("data")?;
    let uniques_table: LuaTable = data.get("uniques")?;

    let mut items = Vec::new();
    for pair in uniques_table.pairs::<String, LuaValue>() {
        let (_key, value) = pair?;
        if let LuaValue::Table(tbl) = value {
            if let Some(strings) = extract_string_array(&tbl) {
                for text in &strings {
                    if let Some(item) = parse_unique_text(text) {
                        items.push(item);
                    }
                }
            }
        }
    }
    Ok(items)
}

pub fn extract(pob_src: &str, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let lua = Lua::new();
    let uniques_dir = format!("{}/Data/Uniques", pob_src);

    let mut all_uniques: Vec<UniqueItemData> = Vec::new();

    // Load main unique files
    for filename in UNIQUE_FILES {
        let filepath = format!("{}/{}", uniques_dir, filename);
        match load_uniques_from_return_file(&lua, &filepath) {
            Ok(items) => {
                let count = items.len();
                all_uniques.extend(items);
                println!("  Loaded {} ({} uniques)", filename, count);
            }
            Err(e) => {
                eprintln!("  Warning: failed to load {}: {}", filename, e);
            }
        }
    }

    // Load Special/ subdirectory if it exists
    let special_dir = format!("{}/Special", uniques_dir);
    if let Ok(entries) = std::fs::read_dir(&special_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "lua") {
                let filepath = path.to_string_lossy().to_string();
                let filename = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                // First, try as a `return { ... }` file
                match load_uniques_from_return_file(&lua, &filepath) {
                    Ok(items) if !items.is_empty() => {
                        let count = items.len();
                        all_uniques.extend(items);
                        println!("  Loaded Special/{} ({} uniques)", filename, count);
                        continue;
                    }
                    _ => {}
                }

                // Then try as a `data.uniques.xxx = { ... }` file
                match load_uniques_from_data_assign_file(&filepath) {
                    Ok(items) if !items.is_empty() => {
                        let count = items.len();
                        all_uniques.extend(items);
                        println!("  Loaded Special/{} ({} uniques)", filename, count);
                    }
                    Ok(_) => {
                        println!(
                            "  Skipped Special/{} (no text-format uniques found)",
                            filename
                        );
                    }
                    Err(e) => {
                        eprintln!("  Warning: failed to load Special/{}: {}", filename, e);
                    }
                }
            }
        }
    }

    // Sort by name for stable output
    all_uniques.sort_by(|a, b| a.name.cmp(&b.name));

    println!("  Extracted {} unique items total", all_uniques.len());

    // Write output
    let out_path = output.join("uniques.json");
    let json = serde_json::to_string_pretty(&all_uniques)?;
    std::fs::write(&out_path, &json)?;
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
    fn extract_produces_uniques() {
        let pob_src = pob_src_dir();
        let tmp = tempfile::tempdir().unwrap();
        extract(&pob_src, tmp.path()).unwrap();

        let json_path = tmp.path().join("uniques.json");
        assert!(json_path.exists(), "uniques.json should be created");

        let data: Vec<UniqueItemData> =
            serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();

        // At least 500 unique items total
        assert!(
            data.len() >= 500,
            "Expected at least 500 unique items, got {}",
            data.len()
        );

        // "Ahn's Might" exists with base_type "Midnight Blade" and non-empty explicits
        let ahns_might = data
            .iter()
            .find(|u| u.name == "Ahn's Might")
            .expect("Ahn's Might should exist");
        assert_eq!(ahns_might.base_type, "Midnight Blade");
        assert!(
            !ahns_might.explicits.is_empty(),
            "Ahn's Might should have explicits"
        );

        // At least one unique has variants
        let has_variants = data.iter().any(|u| !u.variants.is_empty());
        assert!(has_variants, "At least one unique should have variants");

        // Verify Beltimber Blade has the right variants
        let beltimber = data
            .iter()
            .find(|u| u.name == "Beltimber Blade")
            .expect("Beltimber Blade should exist");
        assert_eq!(beltimber.base_type, "Eternal Sword");
        assert_eq!(beltimber.variants.len(), 2);
        assert!(beltimber.variants.contains(&"Pre 3.5.0".to_string()));
        assert!(beltimber.variants.contains(&"Current".to_string()));

        // Verify mod lines don't contain {variant:...} prefixes
        for unique in &data {
            for m in unique.implicits.iter().chain(unique.explicits.iter()) {
                assert!(
                    !m.starts_with('{'),
                    "Mod line should not start with curly prefix: '{}' in '{}'",
                    m,
                    unique.name
                );
            }
        }
    }

    #[test]
    fn test_parse_unique_text_basic() {
        let text = r#"
Ahn's Might
Midnight Blade
Implicits: 1
40% increased Global Accuracy Rating
Adds (80-115) to (150-205) Physical Damage
(15-25)% increased Critical Strike Chance
-1 to Maximum Frenzy Charges
"#;
        let item = parse_unique_text(text).expect("should parse");
        assert_eq!(item.name, "Ahn's Might");
        assert_eq!(item.base_type, "Midnight Blade");
        assert_eq!(item.implicits.len(), 1);
        assert_eq!(item.implicits[0], "40% increased Global Accuracy Rating");
        assert_eq!(item.explicits.len(), 3);
        assert_eq!(
            item.explicits[0],
            "Adds (80-115) to (150-205) Physical Damage"
        );
        assert_eq!(
            item.explicits[1],
            "(15-25)% increased Critical Strike Chance"
        );
        assert_eq!(item.explicits[2], "-1 to Maximum Frenzy Charges");
        assert!(item.variants.is_empty());
    }

    #[test]
    fn test_parse_unique_text_with_variants() {
        let text = r#"
Beltimber Blade
Eternal Sword
Variant: Pre 3.5.0
Variant: Current
Implicits: 1
+475 to Accuracy Rating
{variant:1}(170-190)% increased Physical Damage
{variant:2}(185-215)% increased Physical Damage
(15-20)% increased Attack Speed
"#;
        let item = parse_unique_text(text).expect("should parse");
        assert_eq!(item.name, "Beltimber Blade");
        assert_eq!(item.base_type, "Eternal Sword");
        assert_eq!(item.variants.len(), 2);
        assert_eq!(item.implicits.len(), 1);
        assert_eq!(item.explicits.len(), 3);
        // variant prefixes should be stripped
        assert_eq!(item.explicits[0], "(170-190)% increased Physical Damage");
    }

    #[test]
    fn test_parse_unique_text_with_tags() {
        let text = r#"
Test Ring
Diamond Ring
Implicits: 1
{tags:critical}(20-30)% increased Global Critical Strike Chance
{tags:life}+(30-50) to maximum Life
"#;
        let item = parse_unique_text(text).expect("should parse");
        assert_eq!(item.implicits.len(), 1);
        assert_eq!(
            item.implicits[0],
            "(20-30)% increased Global Critical Strike Chance"
        );
        assert_eq!(item.explicits.len(), 1);
        assert_eq!(item.explicits[0], "+(30-50) to maximum Life");
    }

    #[test]
    fn test_parse_unique_text_skip_metadata() {
        let text = r#"
Replica Dreamfeather
Eternal Sword
League: Heist
Source: Steal from a unique{Curio Display} during a Grand Heist
Implicits: 1
+475 to Accuracy Rating
Adds (40-65) to (70-100) Physical Damage
"#;
        let item = parse_unique_text(text).expect("should parse");
        assert_eq!(item.name, "Replica Dreamfeather");
        assert_eq!(item.implicits.len(), 1);
        assert_eq!(item.explicits.len(), 1);
        assert!(item.variants.is_empty());
    }

    #[test]
    fn test_parse_unique_text_empty() {
        assert!(parse_unique_text("").is_none());
        assert!(parse_unique_text("  \n  ").is_none());
        assert!(parse_unique_text("OnlyName").is_none());
    }

    #[test]
    fn test_strip_curly_prefixes() {
        assert_eq!(strip_curly_prefixes("{variant:1}foo bar"), "foo bar");
        assert_eq!(strip_curly_prefixes("{variant:1,2}{range:0.5}foo"), "foo");
        assert_eq!(
            strip_curly_prefixes("{tags:life}+(30-50) to maximum Life"),
            "+(30-50) to maximum Life"
        );
        assert_eq!(strip_curly_prefixes("no prefix"), "no prefix");
    }
}
