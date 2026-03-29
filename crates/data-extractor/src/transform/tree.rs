use crate::{
    dat64::{probe_row_size, Dat64},
    error::ExtractError,
    ggpk_reader::GgpkReader,
};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct TreeOutput {
    nodes: HashMap<u32, PassiveNode>,
}

#[derive(Serialize)]
struct PassiveNode {
    id: u32,
    name: String,
    is_keystone: bool,
    is_notable: bool,
    is_jewel_socket: bool,
    is_mastery: bool,
    is_ascendancy_start: bool,
    ascendancy_name: Option<String>,
    icon: String,
    skill_points_granted: i32,
}

// ---------------------------------------------------------------------------
// Field offsets in PassiveSkills.datc64 rows (PoE2 schema)
//
// Calibrated against Content.ggpk (row_size=353, row_count=5592):
//
// Id: string ptr (8 bytes)                           offset 0
// Icon_DDSFile: string ptr (8 bytes)                 offset 8
// Stats: [Stats] array ref (count+offset, 16 bytes)  offset 16
// Stat1..4 Values: i32 × 4                           offset 32
// PassiveSkillGraphId: u16 (2 bytes)                 offset 48
// Name: string ptr (8 bytes)                         offset 50  ← NOT 8-aligned
// Characters: [Characters] array ref (16 bytes)      offset 58
// IsKeystone: bool (1 byte)                          offset 74
// IsNotable: bool (1 byte)                           offset 75
// IsJewelSocket: bool (1 byte)                       offset 84
// Ascendancy: u64 row ref (8 bytes)                  offset 112
// IsAscendancyStartingNode: bool (1 byte)            offset 118
// ---------------------------------------------------------------------------

const OFF_ICON: usize = 8;
const OFF_GRAPH_ID: usize = 48;
const OFF_NAME: usize = 50;
const OFF_IS_KEYSTONE: usize = 74;
const OFF_IS_NOTABLE: usize = 75;
const OFF_IS_JEWEL_SOCKET: usize = 84;
const OFF_ASCENDANCY: usize = 112;
const OFF_IS_ASCENDANCY_START: usize = 118;

// SkillPointsGranted — appears much later in the schema.
// We look for it at a few candidate offsets; the caller validates the
// row_size is large enough before using the offset.
const SKILL_POINTS_CANDIDATES: &[usize] = &[160, 164, 168, 172, 176];

// ---------------------------------------------------------------------------
// Ascendancy table helpers
// ---------------------------------------------------------------------------

/// Read ascendancy names from Ascendancy.datc64.
/// The Name field is at offset 0 (first field, string, 8 bytes).
fn read_ascendancy_names(reader: &GgpkReader) -> Result<Vec<String>, ExtractError> {
    let bytes = match reader.read_bytes("Data/Ascendancy.datc64") {
        Ok(b) => b,
        Err(ExtractError::FileNotFound(_)) => {
            println!("  Ascendancy.datc64 not found, skipping ascendancy names");
            return Ok(vec![]);
        }
        Err(e) => return Err(e),
    };

    let (row_count, row_size) = probe_row_size(&bytes).ok_or_else(|| ExtractError::Dat64Parse {
        file: "Ascendancy.datc64".to_string(),
        message: "probe_row_size failed".to_string(),
    })?;

    if row_count == 0 || row_size < 8 {
        return Ok(vec![]);
    }

    let dat = Dat64::parse_datc64(bytes, row_size, "Ascendancy.datc64")?;
    let mut names = Vec::with_capacity(row_count);
    for i in 0..row_count {
        // Name is the first string field at offset 0
        names.push(dat.read_string(i, 0));
    }
    Ok(names)
}

// ---------------------------------------------------------------------------
// Main extract function
// ---------------------------------------------------------------------------

pub fn extract(reader: &GgpkReader, output: &Path) -> Result<(), ExtractError> {
    // --- Ascendancy lookup table ---
    let ascendancy_names = read_ascendancy_names(reader)?;

    // --- PassiveSkills.datc64 ---
    let bytes = reader
        .read_bytes("Data/PassiveSkills.datc64")
        .map_err(|e| match e {
            ExtractError::FileNotFound(_) => ExtractError::Dat64Parse {
                file: "PassiveSkills.datc64".to_string(),
                message: "PassiveSkills.datc64 not found in GGPK".to_string(),
            },
            other => other,
        })?;

    let (row_count, row_size) = probe_row_size(&bytes).ok_or_else(|| ExtractError::Dat64Parse {
        file: "PassiveSkills.datc64".to_string(),
        message: "probe_row_size failed — could not detect row boundaries".to_string(),
    })?;

    println!(
        "  PassiveSkills.datc64: {} rows × {} bytes/row",
        row_count, row_size
    );

    if row_count == 0 {
        println!("  No rows found in PassiveSkills.datc64, writing empty tree");
        let out = TreeOutput {
            nodes: HashMap::new(),
        };
        write_tree(&out, output)?;
        return Ok(());
    }

    // Validate that the row_size is large enough to cover the last fixed offset we use
    let min_row_size = OFF_IS_ASCENDANCY_START + 1; // 119 bytes minimum
    if row_size < min_row_size {
        return Err(ExtractError::Dat64Parse {
            file: "PassiveSkills.datc64".to_string(),
            message: format!("row_size {row_size} is smaller than minimum expected {min_row_size}"),
        });
    }

    // Determine the SkillPointsGranted offset: pick the largest candidate that
    // still fits within row_size.
    let skill_pts_offset = SKILL_POINTS_CANDIDATES
        .iter()
        .copied()
        .filter(|&off| off + 4 <= row_size)
        .last();

    let dat = Dat64::parse_datc64(bytes, row_size, "PassiveSkills.datc64")?;

    let mut nodes: HashMap<u32, PassiveNode> = HashMap::new();

    for i in 0..row_count {
        // Read PassiveSkillGraphId (u16 at offset 48, little-endian).
        // Read as u32 and mask low 16 bits.
        let graph_id_raw = dat.read_u32(i, OFF_GRAPH_ID);
        let graph_id = (graph_id_raw & 0xFFFF) as u16;

        let name = dat.read_string(i, OFF_NAME);

        // Skip placeholder / empty rows
        if name.is_empty() && graph_id == 0 {
            continue;
        }

        let icon = dat.read_string(i, OFF_ICON);
        let is_keystone = dat.read_bool(i, OFF_IS_KEYSTONE);
        let is_notable = dat.read_bool(i, OFF_IS_NOTABLE);
        let is_jewel_socket = dat.read_bool(i, OFF_IS_JEWEL_SOCKET);
        let is_ascendancy_start = dat.read_bool(i, OFF_IS_ASCENDANCY_START);

        // Ascendancy foreign key (u64 row ref)
        let asc_ref = dat.read_u64(i, OFF_ASCENDANCY);
        let ascendancy_name = if asc_ref == u64::MAX || asc_ref as usize >= ascendancy_names.len() {
            None
        } else {
            let n = &ascendancy_names[asc_ref as usize];
            if n.is_empty() {
                None
            } else {
                Some(n.clone())
            }
        };

        let skill_points_granted = if let Some(off) = skill_pts_offset {
            dat.read_u32(i, off) as i32
        } else {
            0
        };

        nodes.insert(
            graph_id as u32,
            PassiveNode {
                id: graph_id as u32,
                name,
                is_keystone,
                is_notable,
                is_jewel_socket,
                // PoE2 dat schema doesn't have a dedicated IsMastery field visible at
                // these offsets; default false (can be derived later if needed).
                is_mastery: false,
                is_ascendancy_start,
                ascendancy_name,
                icon,
                skill_points_granted,
            },
        );
    }

    println!("  Extracted {} passive tree nodes", nodes.len());

    let out = TreeOutput { nodes };
    write_tree(&out, output)?;

    Ok(())
}

fn write_tree(out: &TreeOutput, output: &Path) -> Result<(), ExtractError> {
    let tree_dir = output.join("tree");
    std::fs::create_dir_all(&tree_dir)?;
    let json = serde_json::to_string_pretty(out)?;
    let dest = tree_dir.join("poe2_current.json");
    std::fs::write(&dest, json)?;
    println!("  Wrote tree/poe2_current.json");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn calibrate() {
        let ggpk_path = std::env::var("GGPK_PATH").expect("GGPK_PATH not set");
        let reader =
            crate::ggpk_reader::GgpkReader::open(std::path::Path::new(&ggpk_path)).unwrap();
        let bytes = reader.read_bytes("Data/PassiveSkills.datc64").unwrap();
        let (row_count, row_size) =
            crate::dat64::probe_row_size(&bytes).expect("probe_row_size failed");
        println!("row_count={row_count}  row_size={row_size}");

        let dat =
            crate::dat64::Dat64::parse_datc64(bytes.clone(), row_size, "PassiveSkills.datc64")
                .unwrap();

        // Print first 20 rows with candidate fields
        println!("\n=== FIRST 20 ROWS: gid@48u16 | name@50 | b74 b75 b84 b101 b118 b139 b140 b182 b244 ===");
        for r in 0..row_count.min(20) {
            let id = dat.read_string(r, 0);
            let name = dat.read_string(r, 50);
            let graph_id = (dat.read_u32(r, 48) & 0xFFFF) as u16;
            let get_bool = |off: usize| -> u8 {
                let w = dat.read_u32(r, off & !3);
                ((w >> ((off & 3) * 8)) & 0xFF) as u8
            };
            println!(
                "  r{:4}: gid={:6} b74={} b75={} b84={} b101={} b118={} b139={} b140={} b182={} b244={} | {:?} | {:?}",
                r, graph_id,
                get_bool(74), get_bool(75), get_bool(84), get_bool(101),
                get_bool(118), get_bool(139), get_bool(140), get_bool(182), get_bool(244),
                name, id
            );
        }

        // Search all rows for known names at offset 50
        println!("\n=== SEARCH ALL ROWS FOR KNOWN NAMES AT OFFSET 50 ===");
        let targets = &[
            "Lava Lash",
            "Thick Skin",
            "Iron Reflexes",
            "Resolute Technique",
            "Iron Will",
            "Acrobatics",
            "Blood Magic",
            "Elemental Equilibrium",
        ];
        for target in targets {
            for r in 0..row_count {
                let name = dat.read_string(r, 50);
                if name == *target {
                    let graph_id = (dat.read_u32(r, 48) & 0xFFFF) as u16;
                    let get_bool = |off: usize| -> u8 {
                        let w = dat.read_u32(r, off & !3);
                        ((w >> ((off & 3) * 8)) & 0xFF) as u8
                    };
                    println!(
                        "  {:?}: row={} gid={} b74={} b75={} b84={} b101={} b118={} b139={} b140={} b182={} b244={}",
                        target, r, graph_id,
                        get_bool(74), get_bool(75), get_bool(84), get_bool(101),
                        get_bool(118), get_bool(139), get_bool(140), get_bool(182), get_bool(244)
                    );
                    break;
                }
            }
        }

        // Bool fields summary (only scan offsets where a full u32 can be read)
        println!("\n=== BOOL FIELDS SUMMARY ===");
        for off in 0..row_size {
            let aligned = off & !3;
            if aligned + 4 > row_size {
                continue;
            }
            let shift = (off & 3) * 8;
            let count_true = (0..row_count)
                .filter(|&r| {
                    let w = dat.read_u32(r, aligned);
                    ((w >> shift) & 0xFF) == 1
                })
                .count();
            let all_bool = (0..row_count).all(|r| {
                let w = dat.read_u32(r, aligned);
                ((w >> shift) & 0xFF) <= 1
            });
            if all_bool && count_true > 0 && count_true < row_count {
                println!(
                    "  offset {:4}: {}/{} true ({:.1}%)",
                    off,
                    count_true,
                    row_count,
                    count_true as f64 / row_count as f64 * 100.0
                );
            }
        }
    }

    #[test]
    fn tree_poe2_schema() {
        let Some(ggpk_path) = std::env::var("GGPK_PATH").ok() else {
            eprintln!("GGPK_PATH not set, skipping integration test");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let reader =
            crate::ggpk_reader::GgpkReader::open(std::path::Path::new(&ggpk_path)).unwrap();
        super::extract(&reader, tmp.path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("tree").join("poe2_current.json")).unwrap(),
        )
        .unwrap();
        let nodes = json
            .get("nodes")
            .expect("missing nodes")
            .as_object()
            .unwrap();
        assert!(!nodes.is_empty(), "nodes must not be empty");
        // Check at least one node has expected keys
        let first = nodes.values().next().unwrap();
        assert!(first.get("id").is_some());
        assert!(first.get("name").is_some());
        assert!(first.get("is_keystone").is_some());
    }
}
