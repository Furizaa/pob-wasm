//! Timeless jewel node replacement pipeline.
//!
//! Mirrors PoB's `BuildAllDependsAndPaths()` timeless jewel section
//! (PassiveSpec.lua lines 1064–1303) plus helpers:
//!   - `ReplaceNode` (lines 1541–1562)
//!   - `ReconnectNodeToClassStart` (lines 1564–1574)
//!   - `NodeAdditionOrReplacementFromString` (lines 2104–2143)
//!   - `data.readLUT` (DataLegionLookUpTableHelper.lua lines 292–331)
//!
//! Entry point: [`apply_timeless_jewels`].

use crate::{
    build::{mod_parser, types::Build},
    data::GameData,
    mod_db::types::{
        ConqueredBy, ConquerorType, KeywordFlags, Mod, ModFlags, ModSource, ModType, ModValue,
    },
    passive_tree::{NodeType, PassiveNode, PassiveTree},
};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Conqueror list (ModParser.lua lines 41–65)
// ─────────────────────────────────────────────────────────────────────────────

/// Look up a conqueror by lowercased name.
/// Returns `(conqueror_type_str, conqueror_id_str)`.
/// Mirrors the `conquerorList` table in ModParser.lua.
pub fn lookup_conqueror(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "xibaqua" => Some(("vaal", "1")),
        "zerphi" => Some(("vaal", "2")),
        "doryani" => Some(("vaal", "3")),
        "ahuana" => Some(("vaal", "2_v2")),
        "deshret" => Some(("maraketh", "1")),
        "asenath" => Some(("maraketh", "2")),
        "nasima" => Some(("maraketh", "3")),
        "balbala" => Some(("maraketh", "1_v2")),
        "cadiro" => Some(("eternal", "1")),
        "victario" => Some(("eternal", "2")),
        "chitus" => Some(("eternal", "3")),
        "caspiro" => Some(("eternal", "3_v2")),
        "kaom" => Some(("karui", "1")),
        "rakiata" => Some(("karui", "2")),
        "kiloava" => Some(("karui", "3")),
        "akoya" => Some(("karui", "3_v2")),
        "venarius" => Some(("templar", "1")),
        "dominus" => Some(("templar", "2")),
        "avarius" => Some(("templar", "3")),
        "maxarius" => Some(("templar", "1_v2")),
        "vorana" => Some(("kalguur", "1")),
        "uhtred" => Some(("kalguur", "2")),
        "medved" => Some(("kalguur", "3")),
        _ => None,
    }
}

/// Build a `JewelData` LIST mod with `conqueredBy` value.
/// Used by the mod parser for timeless jewel implicit lines (rules 1976–1981).
pub fn make_conquered_by_mod(seed: u64, name: &str, source: &ModSource) -> Vec<Mod> {
    let (ctype_str, cid_str) = match lookup_conqueror(name) {
        Some(v) => v,
        None => {
            // Unknown conqueror name — return empty (graceful degradation)
            return vec![];
        }
    };
    let conqueror_type = match ConquerorType::from_str(ctype_str) {
        Some(ct) => ct,
        None => return vec![],
    };
    vec![Mod {
        name: "JewelData".to_string(),
        mod_type: ModType::List,
        value: ModValue::ConqueredBy(ConqueredBy {
            seed,
            conqueror_type,
            conqueror_id: cid_str.to_string(),
        }),
        flags: ModFlags::NONE,
        keyword_flags: KeywordFlags::NONE,
        tags: vec![],
        source: source.clone(),
    }]
}

// ─────────────────────────────────────────────────────────────────────────────
// Timeless jewel constants (Data.lua lines 797–825)
// ─────────────────────────────────────────────────────────────────────────────

/// Number of "addition" entries in legionAdditions (= 96, the boundary index).
pub const TIMELESS_JEWEL_ADDITIONS: usize = 96;

/// Jewel type index (1-based, matching Lua's timelessJewelTypes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JewelType {
    GloriousVanity = 1,
    LethalPride = 2,
    BrutalRestraint = 3,
    MilitantFaith = 4,
    ElegantHubris = 5,
    HeroicTragedy = 6,
}

impl JewelType {
    pub fn from_conqueror_type(ct: &ConquerorType) -> Self {
        match ct {
            ConquerorType::Vaal => Self::GloriousVanity,
            ConquerorType::Karui => Self::LethalPride,
            ConquerorType::Maraketh => Self::BrutalRestraint,
            ConquerorType::Templar => Self::MilitantFaith,
            ConquerorType::Eternal => Self::ElegantHubris,
            ConquerorType::Kalguur => Self::HeroicTragedy,
        }
    }

    pub fn seed_min(self) -> u64 {
        match self {
            Self::GloriousVanity => 100,
            Self::LethalPride => 10000,
            Self::BrutalRestraint => 500,
            Self::MilitantFaith => 2000,
            Self::ElegantHubris => 100, // 2000 / 20
            Self::HeroicTragedy => 100,
        }
    }

    pub fn seed_max(self) -> u64 {
        match self {
            Self::GloriousVanity => 8000,
            Self::LethalPride => 18000,
            Self::BrutalRestraint => 8000,
            Self::MilitantFaith => 10000,
            Self::ElegantHubris => 8000, // 160000 / 20
            Self::HeroicTragedy => 8000,
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Self::GloriousVanity => "GloriousVanity",
            Self::LethalPride => "LethalPride",
            Self::BrutalRestraint => "BrutalRestraint",
            Self::MilitantFaith => "MilitantFaith",
            Self::ElegantHubris => "ElegantHubris",
            Self::HeroicTragedy => "HeroicTragedy",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Per-node state for timeless jewel replacement
// ─────────────────────────────────────────────────────────────────────────────

/// Mutable state for a single passive node during timeless jewel processing.
/// This mirrors PoB's `node` table fields that get modified by ReplaceNode /
/// NodeAdditionOrReplacementFromString.
#[derive(Debug, Clone)]
pub struct NodeState {
    pub node_id: u32,
    pub node_type: NodeType,
    pub display_name: String,
    pub stats: Vec<String>,
    pub is_tattoo: bool,
    pub conquered_by: Option<ConqueredBy>,
    /// Connected node IDs (for ReconnectNodeToClassStart).
    pub linked_ids: Vec<u32>,
}

impl NodeState {
    pub fn from_passive_node(node: &PassiveNode) -> Self {
        Self {
            node_id: node.id,
            node_type: node.node_type,
            display_name: node.name.clone(),
            stats: node.stats.clone(),
            is_tattoo: false,
            conquered_by: None,
            linked_ids: node.linked_ids.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// `replaceHelperFunc` (PassiveSpec.lua lines 1147–1164)
// ─────────────────────────────────────────────────────────────────────────────

/// Apply a roll value to a stat description string.
///
/// Mirrors PoB's `replaceHelperFunc`:
/// - If fmt == "g" and key contains "per_minute": value = round(value / 60, 1)
/// - If fmt == "g" and key contains "permyriad": value = value / 100
/// - If fmt == "g" and key contains "_ms": value = value / 1000
/// - If min != max: replace "(min-max)" with value
/// - Else if min != value: replace occurrences of min string with value
/// - Otherwise: return unchanged
pub fn replace_helper_func(
    stat_to_fix: &str,
    stat_key: &str,
    fmt: &str,
    stat_min: f64,
    stat_max: f64,
    mut value: f64,
) -> String {
    if fmt == "g" {
        if stat_key.contains("per_minute") {
            // round to 1 decimal place
            value = (value / 60.0 * 10.0).round() / 10.0;
        } else if stat_key.contains("permyriad") {
            value = value / 100.0;
        } else if stat_key.contains("_ms") {
            value = value / 1000.0;
        }
    }

    if stat_min != stat_max {
        // Replace "(min-max)" pattern
        let pattern = format!("({}-{})", stat_min as i64, stat_max as i64);
        let val_str = format_value(value);
        stat_to_fix.replace(&pattern, &val_str)
    } else if stat_min as i64 != value as i64 || stat_min != value {
        // Replace occurrences of the min value string
        let min_str = format_value(stat_min);
        let val_str = format_value(value);
        stat_to_fix.replace(&min_str, &val_str)
    } else {
        stat_to_fix.to_string()
    }
}

fn format_value(v: f64) -> String {
    // PoB uses Lua's default number formatting: integers show without decimal point
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// `NodeAdditionOrReplacementFromString` (PassiveSpec.lua lines 2104–2143)
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a stat string (possibly containing `\n`) and add/replace mods on a node.
///
/// If `replacement` is true, clears the node's current stats first (first line).
/// Returns the new set of stats after modification.
pub fn node_addition_or_replacement_from_string(
    current_stats: &[String],
    sd: &str,
    replacement: bool,
) -> Vec<String> {
    // Split on newlines (the " \n" prefix trick from Lua)
    let lines: Vec<&str> = sd
        .split('\n')
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if replacement {
        // Replace: start fresh with the new lines
        lines.iter().map(|s| s.to_string()).collect()
    } else {
        // Addition: append the new lines to existing stats
        let mut result = current_stats.to_vec();
        result.extend(lines.iter().map(|s| s.to_string()));
        result
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// `ReplaceNode` (PassiveSpec.lua lines 1541–1562)
// ─────────────────────────────────────────────────────────────────────────────

/// Copy stats from a legion node template into the target NodeState.
/// Mirrors PoB's `ReplaceNode`.
pub fn replace_node(target: &mut NodeState, new_stats: &[String], new_name: &str) {
    target.stats = new_stats.to_vec();
    target.display_name = new_name.to_string();
}

// ─────────────────────────────────────────────────────────────────────────────
// Main pipeline: apply_timeless_jewels
// ─────────────────────────────────────────────────────────────────────────────

/// Apply timeless jewel node replacements for the two-pass algorithm.
///
/// Returns a map from `node_id → Vec<String>` containing the overridden stats
/// for nodes that were replaced or augmented by a timeless jewel. Nodes not in
/// this map keep their original stats.
///
/// Pass 1: For each jewel socket with a conqueredBy jewel, mark all nodes in
///         the jewel's radius as conquered.
/// Pass 2: For each conquered node, apply replacement/addition logic.
pub fn apply_timeless_jewels(
    build: &Build,
    tree: &PassiveTree,
    data: &GameData,
) -> HashMap<u32, Vec<String>> {
    // ── Pass 1: assign conqueredBy to nodes in radius ────────────────────────

    // jewel_sockets maps socket_node_id → ConqueredBy
    // We build this by scanning jewels in the tree socket positions.
    let mut jewel_socket_conquered: HashMap<u32, ConqueredBy> = HashMap::new();

    // The build's jewels field maps socket_node_id → item_id
    // (populated from <Socket nodeId="..." itemId="..."/> in <Sockets>)
    for (&socket_node_id, &item_id) in &build.passive_spec.jewels {
        if item_id == 0 {
            continue;
        }

        // The item must exist
        let item = match build.items.get(&item_id) {
            Some(i) => i,
            None => continue,
        };

        // The socket node must be allocated
        if !build.passive_spec.allocated_nodes.contains(&socket_node_id) {
            continue;
        }

        // The item must have a JewelData mod with conqueredBy
        let source = ModSource::new("Item", format!("{} (Jewel)", item.base_type));
        let all_lines: Vec<&str> = item
            .implicits
            .iter()
            .chain(item.explicits.iter())
            .chain(item.crafted_mods.iter())
            .map(|s| s.as_str())
            .collect();

        let mut conquered_by: Option<ConqueredBy> = None;

        for line in &all_lines {
            let mods = mod_parser::parse_mod(line, source.clone());
            for m in &mods {
                if m.name == "JewelData" {
                    if let ModValue::ConqueredBy(cb) = &m.value {
                        conquered_by = Some(cb.clone());
                    }
                }
            }
        }

        let conquered_by = match conquered_by {
            Some(cb) => cb,
            None => continue,
        };

        // Timeless jewels use "Large" radius = index 3 (1-based in Lua).
        // In PoB, `item.jewelRadiusIndex` is set from "Radius: Large" on the item
        // text. Timeless jewels are always Large radius.
        let radius_idx: usize = 3;

        // Get nodes in radius for this socket node
        let socket_node = match tree.nodes.get(&socket_node_id) {
            Some(n) => n,
            None => continue,
        };

        // Check nodesInRadius on the socket node
        let nodes_in_radius = match socket_node.nodes_in_radius.get(&radius_idx) {
            Some(n) => n,
            None => continue,
        };

        // Mark all eligible nodes in radius as conquered
        for &node_id in nodes_in_radius {
            let node = match tree.nodes.get(&node_id) {
                Some(n) => n,
                None => continue,
            };
            // Skip ClassStart, Socket, and ascendancy nodes (Lua: type ~= "ClassStart" and type ~= "Socket" and not ascendancyName)
            if node.node_type == NodeType::ClassStart
                || node.node_type == NodeType::JewelSocket
                || node.ascendancy_name.is_some()
            {
                continue;
            }
            jewel_socket_conquered.insert(node_id, conquered_by.clone());
        }
    }

    // ── Pass 2: apply replacements ───────────────────────────────────────────

    let mut overrides: HashMap<u32, Vec<String>> = HashMap::new();

    for (node_id, conquered_by) in &jewel_socket_conquered {
        let node = match tree.nodes.get(node_id) {
            Some(n) => n,
            None => continue,
        };

        if node.node_type == NodeType::JewelSocket {
            continue;
        }

        let jewel_type = JewelType::from_conqueror_type(&conquered_by.conqueror_type);

        // Compute the effective seed (Elegant Hubris divides by 20)
        let seed = if jewel_type == JewelType::ElegantHubris {
            conquered_by.seed / 20
        } else {
            conquered_by.seed
        };

        // Validate seed range
        let seed_min = jewel_type.seed_min();
        let seed_max = jewel_type.seed_max();
        if seed < seed_min || seed > seed_max {
            continue; // out of range, skip silently
        }

        let new_stats = match node.node_type {
            NodeType::Notable => {
                apply_notable_replacement(node, conquered_by, jewel_type, seed, data)
            }
            NodeType::Keystone => apply_keystone_replacement(node, conquered_by, data),
            NodeType::Small => apply_normal_replacement(node, conquered_by, data),
            _ => None,
        };

        if let Some(stats) = new_stats {
            overrides.insert(*node_id, stats);
        }
    }

    overrides
}

// ─────────────────────────────────────────────────────────────────────────────
// Notable replacement (PassiveSpec.lua lines 1166–1252)
// ─────────────────────────────────────────────────────────────────────────────

fn apply_notable_replacement(
    node: &PassiveNode,
    _conquered_by: &ConqueredBy,
    jewel_type: JewelType,
    seed: u64,
    data: &GameData,
) -> Option<Vec<String>> {
    // Read LUT for this (seed, node_id, jewel_type)
    let jewel_data_tbl = data.legion.read_lut(seed, node.id, jewel_type)?;

    if jewel_data_tbl.is_empty() {
        return None; // Missing LUT entry
    }

    if jewel_type == JewelType::GloriousVanity {
        let header_size = jewel_data_tbl.len();

        if header_size == 2 || header_size == 3 {
            // Simple replacement
            let replace_idx = jewel_data_tbl[0] as usize + 1 - TIMELESS_JEWEL_ADDITIONS;
            let legion_node = data.legion.nodes.get(replace_idx.wrapping_sub(1))?;

            let mut new_stats = legion_node.sd.clone();

            // Apply stat rolls using replaceHelperFunc
            for (i, stat_str) in new_stats.iter_mut().enumerate() {
                if i < legion_node.sorted_stats.len() {
                    let stat_key = &legion_node.sorted_stats[i];
                    if let Some(stat_mod) = legion_node.stats.get(stat_key) {
                        let roll_byte = jewel_data_tbl
                            .get(stat_mod.index.saturating_sub(1))
                            .copied()
                            .unwrap_or(0);
                        *stat_str = replace_helper_func(
                            stat_str,
                            stat_key,
                            &stat_mod.fmt,
                            stat_mod.min,
                            stat_mod.max,
                            roll_byte as f64,
                        );
                    }
                }
            }

            Some(new_stats)
        } else if header_size == 6 || header_size == 8 {
            // Might/Legacy of the Vaal
            let half = header_size / 2;
            let mut bias: i32 = 0;
            for &val in jewel_data_tbl[..half].iter() {
                if val <= 21 {
                    bias += 1;
                } else {
                    bias -= 1;
                }
            }

            // legion_nodes[77] = Might of the Vaal (1-indexed), legion_nodes[78] = Legacy
            // 0-indexed: 76 and 77
            let base_node_idx = if bias >= 0 { 76usize } else { 77usize };
            let base_node = data.legion.nodes.get(base_node_idx)?;
            let mut new_stats = base_node.sd.clone();

            // Aggregate additions
            let mut additions: HashMap<u8, f64> = HashMap::new();
            for i in 0..half {
                let add_type = jewel_data_tbl[i];
                let roll = jewel_data_tbl[i + half] as f64;
                let entry = additions.entry(add_type).or_insert(0.0);
                *entry += roll;
            }

            for (add_type, &val) in &additions {
                let add_idx = *add_type as usize; // 0-based, legion additions is 0-based in Rust
                if let Some(addition) = data.legion.additions.get(add_idx) {
                    for add_stat in &addition.sd {
                        let mut fixed_stat = add_stat.clone();
                        for (stat_key, stat_mod) in &addition.stats {
                            fixed_stat = replace_helper_func(
                                &fixed_stat,
                                stat_key,
                                &stat_mod.fmt,
                                stat_mod.min,
                                stat_mod.max,
                                val,
                            );
                        }
                        new_stats.push(fixed_stat);
                    }
                }
            }

            Some(new_stats)
        } else {
            None // Unhandled headerSize
        }
    } else {
        // Non-GV jewels: single-byte LUT result
        let mut new_stats = node.stats.clone();
        let mut replaced = false;

        for &jewel_data in &jewel_data_tbl {
            if jewel_data as usize >= TIMELESS_JEWEL_ADDITIONS {
                // Replacement
                let replace_idx = jewel_data as usize + 1 - TIMELESS_JEWEL_ADDITIONS;
                if let Some(legion_node) = data.legion.nodes.get(replace_idx.wrapping_sub(1)) {
                    if !replaced {
                        new_stats = legion_node.sd.clone();
                        replaced = true;
                    }
                }
            } else if jewel_data > 0 {
                // Addition (jewel_data as 0-based index into additions)
                let add_idx = jewel_data as usize; // Lua: legionAdditions[jewelData + 1], so 0-based = jewelData
                if let Some(addition) = data.legion.additions.get(add_idx) {
                    for add_stat in &addition.sd {
                        new_stats.push(add_stat.clone());
                    }
                }
            }
        }

        if replaced || new_stats.len() != node.stats.len() {
            Some(new_stats)
        } else {
            None
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Keystone replacement (PassiveSpec.lua lines 1253–1260)
// ─────────────────────────────────────────────────────────────────────────────

fn apply_keystone_replacement(
    _node: &PassiveNode,
    conquered_by: &ConqueredBy,
    data: &GameData,
) -> Option<Vec<String>> {
    // Match string: "<type>_keystone_<id>", e.g. "vaal_keystone_1"
    let match_str = format!(
        "{}_keystone_{}",
        conquered_by.conqueror_type.as_str(),
        conquered_by.conqueror_id
    );

    for legion_node in &data.legion.nodes {
        if legion_node.id == match_str {
            return Some(legion_node.sd.clone());
        }
    }

    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Normal node replacement (PassiveSpec.lua lines 1261–1300)
// ─────────────────────────────────────────────────────────────────────────────

/// Attribute display names (for the "small bonus" check).
const ATTRIBUTE_NAMES: &[&str] = &["Dexterity", "Intelligence", "Strength"];

fn apply_normal_replacement(
    node: &PassiveNode,
    conquered_by: &ConqueredBy,
    data: &GameData,
) -> Option<Vec<String>> {
    let is_attribute_node = ATTRIBUTE_NAMES.contains(&node.name.as_str());
    // Note: `is_tattoo` is false by default (SETUP-14 would set it)
    let is_tattoo = false;
    let small_bonus = is_attribute_node || is_tattoo;

    match &conquered_by.conqueror_type {
        ConquerorType::Vaal => {
            // Glorious Vanity normal nodes: LUT-based replacement (same as notable)
            // But we can't call apply_notable_replacement directly without seed/jewel_type.
            // This branch is handled by the notable path above; for normal nodes with Vaal,
            // the same LUT lookup applies.
            // For now return None (the vaal normal path needs its own LUT lookup).
            None
        }
        ConquerorType::Karui => {
            // Lethal Pride: +2 or +4 Strength
            let val = if small_bonus { "2" } else { "4" };
            let mut stats = node.stats.clone();
            stats.push(format!("+{} to Strength", val));
            Some(stats)
        }
        ConquerorType::Maraketh => {
            // Brutal Restraint: +2 or +4 Dexterity
            let val = if small_bonus { "2" } else { "4" };
            let mut stats = node.stats.clone();
            stats.push(format!("+{} to Dexterity", val));
            Some(stats)
        }
        ConquerorType::Kalguur => {
            // Heroic Tragedy: 1% or 2% increased Ward
            let val = if small_bonus { "1" } else { "2" };
            let mut stats = node.stats.clone();
            stats.push(format!("{}% increased Ward", val));
            Some(stats)
        }
        ConquerorType::Templar => {
            // Militant Faith: replace with devotion node OR add +5 Devotion
            if small_bonus {
                // Replace with templar_devotion_node (legionNodes[91], 0-indexed: 90)
                if let Some(legion_node) = data.legion.nodes.get(90) {
                    Some(legion_node.sd.clone())
                } else {
                    None
                }
            } else {
                let mut stats = node.stats.clone();
                stats.push("+5 to Devotion".to_string());
                Some(stats)
            }
        }
        ConquerorType::Eternal => {
            // Elegant Hubris: replace with blank node (legionNodes[110], 0-indexed: 109)
            if let Some(legion_node) = data.legion.nodes.get(109) {
                Some(legion_node.sd.clone())
            } else {
                Some(vec![]) // blank
            }
        }
    }
}
