use serde::Deserialize;
use std::collections::HashMap;

/// A single pantheon god (major or minor).
/// Mirrors the `Data/Pantheons.lua` table structure:
/// `{ isMajorGod = true/false, souls = { [1] = { name = "...", mods = { ... } } } }`
#[derive(Debug, Clone, Deserialize)]
pub struct PantheonGod {
    /// Whether this is a major god (true) or minor god (false).
    #[serde(rename = "is_major_god")]
    pub is_major_god: bool,
    /// Ordered list of soul tiers for this god.
    /// Index 0 = primary soul (soul tier 1 in Lua), which provides the source name.
    pub souls: Vec<PantheonSoul>,
}

/// A single soul tier for a pantheon god.
/// Mirrors `{ name = "...", mods = { [1] = { line = "...", value = {...} } } }`
#[derive(Debug, Clone, Deserialize)]
pub struct PantheonSoul {
    /// The soul's display name (e.g. "Soul of Arakaali", "Hybrid Widow").
    pub name: String,
    /// The mod lines for this soul tier.
    pub mods: Vec<PantheonMod>,
}

/// A single mod line within a pantheon soul.
/// Mirrors `{ line = "...", value = {...} }` in the Lua data.
#[derive(Debug, Clone, Deserialize)]
pub struct PantheonMod {
    /// The stat text to be parsed (e.g. "10% reduced Damage taken from Damage Over Time").
    pub line: String,
}

/// The full pantheons map, keyed by god name (e.g. "Arakaali", "Shakari").
/// Mirrors `env.data.pantheons` in PoB.
pub type PantheonMap = HashMap<String, PantheonGod>;
