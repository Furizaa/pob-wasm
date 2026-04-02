//! Legion jewel data — parsed from LegionPassives.lua and the LUT binary files.
//!
//! Mirrors the `tree.legion` table in PoB's PassiveTree.lua:
//!   - `legion.nodes`     → `LegionData::nodes`
//!   - `legion.additions` → `LegionData::additions`
//!
//! Also provides `read_lut()` — mirrors DataLegionLookUpTableHelper.lua:readLUT().

use crate::timeless_jewels::JewelType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Legion passive node / addition entry
// ─────────────────────────────────────────────────────────────────────────────

/// Stat modifier info within a legion node or addition.
/// Mirrors the `stats[key]` sub-table in LegionPassives.lua.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LegionStatMod {
    /// Format: "d" (display as integer) or "g" (game format — requires unit conversion).
    pub fmt: String,
    /// 1-based index into the LUT data bytes array.
    pub index: usize,
    pub min: f64,
    pub max: f64,
    #[serde(default)]
    pub stat_order: u32,
}

/// A single legion passive node (replacement) or addition.
/// Mirrors a `nodes[N]` or `additions[N]` entry in LegionPassives.lua.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LegionNode {
    /// Internal ID string, e.g. "vaal_keystone_1", "templar_devotion_node".
    pub id: String,
    /// Display name.
    pub dn: String,
    /// Stat description strings (what's shown on the node tooltip).
    pub sd: Vec<String>,
    /// Ordered list of stat keys (for LUT index alignment).
    pub sorted_stats: Vec<String>,
    /// Stat metadata keyed by stat key.
    pub stats: HashMap<String, LegionStatMod>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Node index mapping (NodeIndexMapping.lua)
// ─────────────────────────────────────────────────────────────────────────────

/// Entry in the NodeIndexMapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeIndexEntry {
    /// 0-based index into the LUT data array.
    pub index: usize,
    /// Size of the data block for this node (bytes).
    pub size: usize,
}

/// JSON representation of NodeIndexMapping (with string keys for JSON compat).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RawNodeIndexMapping {
    pub size: usize,
    pub size_notable: usize,
    /// Per-node index entries. Keys are node IDs as strings.
    pub entries: HashMap<String, NodeIndexEntry>,
    /// localId → globalId mapping per jewel type.
    /// Keys are jewel type indices (1-6) as strings.
    /// Inner keys are local IDs as strings.
    #[serde(default)]
    pub local_to_global: HashMap<String, HashMap<String, u8>>,
}

/// The full node index mapping.
#[derive(Debug, Clone, Default)]
pub struct NodeIndexMapping {
    pub size: usize,
    pub size_notable: usize,
    pub entries: HashMap<u32, NodeIndexEntry>,
    /// jewel_type → local_id → global_id
    pub local_to_global: HashMap<u8, HashMap<u8, u8>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// LUT data
// ─────────────────────────────────────────────────────────────────────────────

/// Raw binary LUT data for a single jewel type.
#[derive(Debug, Clone, Default)]
pub struct JewelLut {
    /// Flat byte array for non-GV jewels: data[index * seed_size + seed_offset]
    pub data: Vec<u8>,
    /// For GV: sizes prefix (nodecount * seed_size bytes).
    pub gv_sizes: Vec<u8>,
    /// For GV: per-node expanded data. Indexed by [node_index][seed_offset] → bytes.
    pub gv_data: Vec<Vec<Vec<u8>>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// JSON input format
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct LegionDataJson {
    additions: Vec<LegionNode>,
    nodes: Vec<LegionNode>,
    node_index: RawNodeIndexMapping,
    /// Base64-encoded flat byte arrays, keyed by jewel type index (2-6).
    #[serde(default)]
    luts: HashMap<String, String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// LegionData — the main container
// ─────────────────────────────────────────────────────────────────────────────

/// All legion jewel data needed for timeless jewel node replacement.
#[derive(Debug, Clone, Default)]
pub struct LegionData {
    /// Replacement node templates (Lua: `tree.legion.nodes`, 1-indexed in Lua).
    /// Stored 0-indexed here.
    pub nodes: Vec<LegionNode>,
    /// Addition stat templates (Lua: `tree.legion.additions`, 1-indexed in Lua).
    /// Stored 0-indexed here.
    pub additions: Vec<LegionNode>,
    /// Node index mapping for GV LUT lookup.
    pub node_index: NodeIndexMapping,
    /// LUT data per jewel type (1-based index: 1=GV, 2=LP, 3=BR, 4=MF, 5=EH, 6=HT).
    pub luts: HashMap<u8, JewelLut>,
}

impl LegionData {
    /// Parse from the legion.json data file.
    pub fn from_json(json: &str) -> Result<Self, crate::error::DataError> {
        let raw: LegionDataJson = serde_json::from_str(json)?;

        // Convert node index mapping
        let mut entries: HashMap<u32, NodeIndexEntry> = HashMap::new();
        for (k, v) in &raw.node_index.entries {
            if let Ok(id) = k.parse::<u32>() {
                entries.insert(id, v.clone());
            }
        }
        let mut local_to_global: HashMap<u8, HashMap<u8, u8>> = HashMap::new();
        for (jt_str, inner) in &raw.node_index.local_to_global {
            if let Ok(jt) = jt_str.parse::<u8>() {
                let mut m: HashMap<u8, u8> = HashMap::new();
                for (local_str, &global) in inner {
                    if let Ok(local) = local_str.parse::<u8>() {
                        m.insert(local, global);
                    }
                }
                local_to_global.insert(jt, m);
            }
        }
        let node_index = NodeIndexMapping {
            size: raw.node_index.size,
            size_notable: raw.node_index.size_notable,
            entries,
            local_to_global,
        };

        // Decode LUT data
        let mut luts: HashMap<u8, JewelLut> = HashMap::new();
        for (jt_str, b64) in &raw.luts {
            if let Ok(jt) = jt_str.parse::<u8>() {
                use base64::{engine::general_purpose::STANDARD, Engine as _};
                match STANDARD.decode(b64.as_bytes()) {
                    Ok(data) => {
                        luts.insert(
                            jt,
                            JewelLut {
                                data,
                                gv_sizes: vec![],
                                gv_data: vec![],
                            },
                        );
                    }
                    Err(e) => {
                        eprintln!("Warning: failed to decode LUT for jewel type {jt}: {e}");
                    }
                }
            }
        }

        Ok(Self {
            additions: raw.additions,
            nodes: raw.nodes,
            node_index,
            luts,
        })
    }

    /// Read the LUT for a (seed, node_id, jewel_type) combination.
    ///
    /// Mirrors DataLegionLookUpTableHelper.lua:readLUT().
    /// Returns `None` if the LUT isn't loaded or the node has no entry.
    /// Returns `Some(Vec<u8>)` with the byte array result.
    pub fn read_lut(&self, seed: u64, node_id: u32, jewel_type: JewelType) -> Option<Vec<u8>> {
        let jt_idx = jewel_type as u8;
        let lut = self.luts.get(&jt_idx)?;

        let seed_min = jewel_type.seed_min();
        let seed_max = jewel_type.seed_max();
        let seed_size = (seed_max - seed_min + 1) as usize;
        let seed_offset = (seed - seed_min) as usize;

        let index_entry = self.node_index.entries.get(&node_id)?;
        let index = index_entry.index;

        if jewel_type == JewelType::GloriousVanity {
            // GV: sizes prefix + per-node data
            if lut.gv_sizes.is_empty() || lut.gv_data.is_empty() {
                return None;
            }
            let sizes_pos = index * seed_size + seed_offset;
            let data_length = *lut.gv_sizes.get(sizes_pos)? as usize;
            if data_length == 0 {
                return Some(vec![]);
            }

            let node_data = lut.gv_data.get(index)?;
            let seed_data = node_data.get(seed_offset)?;
            let raw: Vec<u8> = seed_data.iter().take(data_length).copied().collect();

            // Convert local IDs to global IDs
            let mut result = raw;
            if data_length == 2 || data_length == 3 {
                if let Some(global) = self.convert_local_to_global(jt_idx, result[0]) {
                    result[0] = global;
                }
            } else if data_length == 6 || data_length == 8 {
                let half = data_length / 2;
                for i in 0..half {
                    if let Some(global) = self.convert_local_to_global(jt_idx, result[i]) {
                        result[i] = global;
                    }
                }
            }
            Some(result)
        } else {
            // Non-GV: flat array indexed by [index * seed_size + seed_offset]
            // Returns empty if index >= sizeNotable (normal nodes, not notables)
            if index >= self.node_index.size_notable {
                return Some(vec![]); // normal node, no replacement data
            }
            let byte_pos = index * seed_size + seed_offset;
            let local_id = *lut.data.get(byte_pos)?;
            if local_id == 0 {
                return Some(vec![]);
            }
            let global_id = self
                .convert_local_to_global(jt_idx, local_id)
                .unwrap_or(local_id);
            Some(vec![global_id])
        }
    }

    fn convert_local_to_global(&self, jewel_type: u8, local_id: u8) -> Option<u8> {
        self.node_index
            .local_to_global
            .get(&jewel_type)
            .and_then(|m| m.get(&local_id))
            .copied()
    }
}
