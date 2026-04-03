//! Tattoo passive data, mirroring PoB's Data/TattooPassives.lua.
//!
//! Tattoos replace allocated passive tree nodes with alternative effects.
//! They are stored as `<Overrides>` XML inside `<Spec>` and looked up by
//! display name (`dn`) in this table.
//!
//! Mirrors `self.tree.tattoo = LoadModule("Data/TattooPassives")` in
//! PassiveTree.lua line 57, and `self.tree.tattoo.nodes[dn]` lookup in
//! PassiveSpec.lua lines 153–169.

use crate::error::DataError;
use serde::Deserialize;
use std::collections::HashMap;

/// A single tattoo passive node entry from TattooPassives.lua.
///
/// Fields map to TattooPassives.lua entry fields:
/// - `dn`            → `["dn"]`           display name (key into the nodes table)
/// - `is_tattoo`     → `["isTattoo"]`     always true for tattoo entries
/// - `override_type` → `["overrideType"]` multiplier key (e.g. "KeystoneTattoo")
/// - `is_keystone`   → `["ks"]`           keystone flag
/// - `is_notable`    → `["not"]`          notable flag
/// - `is_mastery`    → `["m"]`            mastery flag
/// - `stats`         → `["sd"]`           stat description lines
/// - `active_effect_image` → `["activeEffectImage"]` artwork path (for fallback lookup)
/// - `icon`          → `["icon"]`         icon path (for fallback lookup)
#[derive(Debug, Clone, Deserialize)]
pub struct TattooNode {
    pub dn: String,
    #[serde(default)]
    pub is_tattoo: bool,
    #[serde(default)]
    pub override_type: String,
    #[serde(default)]
    pub is_keystone: bool,
    #[serde(default)]
    pub is_notable: bool,
    #[serde(default)]
    pub is_mastery: bool,
    /// Stat description lines (the `sd` field from TattooPassives.lua).
    /// These replace the original passive node's stats when the tattoo is applied.
    #[serde(default)]
    pub stats: Vec<String>,
    /// Art/... path for the active effect background image.
    /// Used in the fallback lookup when the `dn` can't be found directly.
    /// Mirrors the `["activeEffectImage"]` field.
    #[serde(default)]
    pub active_effect_image: String,
    /// Art/... path for the node icon.
    /// Used in the fallback lookup when the `dn` can't be found directly.
    /// Mirrors the `["icon"]` field.
    #[serde(default)]
    pub icon: String,
}

/// The full tattoo data table, keyed by display name (`dn`).
///
/// Mirrors `self.tree.tattoo.nodes` in PassiveSpec.lua.
/// Also provides a secondary index from `(active_effect_image, icon)` pairs
/// to `dn` for the renamed-tattoo fallback (PassiveSpec.lua lines 153–160).
#[derive(Debug, Clone, Default)]
pub struct TattooData {
    /// Primary lookup: dn → TattooNode.
    pub nodes: HashMap<String, TattooNode>,
    /// Secondary lookup: (active_effect_image, icon) → dn.
    /// Built from `nodes` at load time for O(1) fallback lookups.
    /// Mirrors PassiveSpec.lua lines 153–160: the linear scan over
    /// `self.tree.tattoo.nodes` checking both image fields.
    pub image_icon_index: HashMap<(String, String), String>,
}

impl TattooData {
    /// Parse from tattoos.json produced by `scripts/extract_tattoo_passives.py`.
    ///
    /// JSON format:
    /// ```json
    /// { "nodes": { "<dn>": { ... }, ... } }
    /// ```
    pub fn from_json(json: &str) -> Result<Self, DataError> {
        #[derive(Deserialize)]
        struct Root {
            nodes: HashMap<String, TattooNode>,
        }
        let root: Root = serde_json::from_str(json)?;

        // Build secondary index: (active_effect_image, icon) → dn.
        let image_icon_index = root
            .nodes
            .values()
            .filter(|n| !n.active_effect_image.is_empty() || !n.icon.is_empty())
            .map(|n| {
                (
                    (n.active_effect_image.clone(), n.icon.clone()),
                    n.dn.clone(),
                )
            })
            .collect();

        Ok(Self {
            nodes: root.nodes,
            image_icon_index,
        })
    }

    /// Look up a tattoo node by display name (`dn`).
    ///
    /// Returns `None` if the name is not found.
    /// Mirrors `self.tree.tattoo.nodes[dn]` in PassiveSpec.lua.
    pub fn lookup_by_dn(&self, dn: &str) -> Option<&TattooNode> {
        self.nodes.get(dn)
    }

    /// Fallback lookup by `(active_effect_image, icon)` pair.
    ///
    /// Used when the `dn` attribute in the XML doesn't match any known tattoo
    /// name (e.g. after a PoB update renamed a tattoo).
    /// Mirrors PassiveSpec.lua lines 153–160: the linear scan over all nodes
    /// checking both image fields. Our pre-built index makes this O(1).
    pub fn lookup_by_images(&self, active_effect_image: &str, icon: &str) -> Option<&TattooNode> {
        let key = (active_effect_image.to_string(), icon.to_string());
        self.image_icon_index
            .get(&key)
            .and_then(|dn| self.nodes.get(dn))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"{
        "nodes": {
            "Acrobatics": {
                "dn": "Acrobatics",
                "is_tattoo": true,
                "override_type": "KeystoneTattoo",
                "is_keystone": true,
                "is_notable": false,
                "is_mastery": false,
                "stats": [
                    "Modifiers to Chance to Suppress Spell Damage instead apply to Chance to Dodge Spell Hits at 50% of their value",
                    "Maximum Chance to Dodge Spell Hits is 75%",
                    "Limited to 1 Keystone Tattoo"
                ],
                "active_effect_image": "Art/2DArt/UIImages/InGame/AncestralTrial/PassiveTreeTattoos/KeystoneHinekoraPassiveBG.png",
                "icon": "Art/2DArt/SkillIcons/passives/KeystoneAcrobatics.png"
            },
            "Tattoo of the Ramako Archer": {
                "dn": "Tattoo of the Ramako Archer",
                "is_tattoo": true,
                "override_type": "RamakoTattoo",
                "is_keystone": false,
                "is_notable": false,
                "is_mastery": false,
                "stats": ["5% increased Global Accuracy Rating"],
                "active_effect_image": "Art/2DArt/UIImages/InGame/AncestralTrial/PassiveTreeTattoos/RamakoPassiveBG.png",
                "icon": "Art/2DArt/SkillIcons/passives/RangeAttackAccuracy.png"
            }
        }
    }"#;

    #[test]
    fn loads_from_json() {
        let data = TattooData::from_json(SAMPLE_JSON).unwrap();
        assert_eq!(data.nodes.len(), 2);
    }

    #[test]
    fn lookup_by_dn_succeeds() {
        let data = TattooData::from_json(SAMPLE_JSON).unwrap();
        let node = data.lookup_by_dn("Acrobatics").unwrap();
        assert_eq!(node.dn, "Acrobatics");
        assert_eq!(node.override_type, "KeystoneTattoo");
        assert!(node.is_keystone);
        assert!(!node.is_notable);
        assert!(!node.is_mastery);
        assert_eq!(node.stats.len(), 3);
    }

    #[test]
    fn lookup_by_dn_missing_returns_none() {
        let data = TattooData::from_json(SAMPLE_JSON).unwrap();
        assert!(data.lookup_by_dn("NonExistentTattoo").is_none());
    }

    #[test]
    fn lookup_by_images_fallback() {
        let data = TattooData::from_json(SAMPLE_JSON).unwrap();
        // Simulate a renamed tattoo: same images but different dn
        let node = data.lookup_by_images(
            "Art/2DArt/UIImages/InGame/AncestralTrial/PassiveTreeTattoos/RamakoPassiveBG.png",
            "Art/2DArt/SkillIcons/passives/RangeAttackAccuracy.png",
        );
        assert!(node.is_some());
        assert_eq!(node.unwrap().dn, "Tattoo of the Ramako Archer");
    }

    #[test]
    fn lookup_by_images_no_match() {
        let data = TattooData::from_json(SAMPLE_JSON).unwrap();
        let result = data.lookup_by_images("nonexistent.png", "also_nonexistent.png");
        assert!(result.is_none());
    }
}
