use serde::Deserialize;
use std::collections::HashMap;

/// A unique item definition extracted from game data.
#[derive(Debug, Clone, Deserialize)]
pub struct UniqueItemData {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub base_type: String,
    #[serde(default)]
    pub implicits: Vec<String>,
    #[serde(default)]
    pub explicits: Vec<String>,
    #[serde(default)]
    pub variants: Vec<String>,
}

/// Lookup map for unique items, keyed by name.
#[derive(Debug, Clone)]
pub struct UniqueItemMap {
    inner: HashMap<String, UniqueItemData>,
}

impl UniqueItemMap {
    /// Build the map from a flat list of unique items.
    pub fn from_vec(items: Vec<UniqueItemData>) -> Self {
        let inner = items
            .into_iter()
            .map(|item| (item.name.clone(), item))
            .collect();
        Self { inner }
    }

    pub fn get(&self, name: &str) -> Option<&UniqueItemData> {
        self.inner.get(name)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_item_deserializes() {
        let json = r#"{
            "name": "Headhunter",
            "base_type": "Leather Belt",
            "implicits": ["+25 to maximum Life"],
            "explicits": [
                "+60 to Strength",
                "+60 to Dexterity",
                "+50 to maximum Life",
                "20% increased Damage with Hits against Rare monsters",
                "When you Kill a Rare monster, you gain its Modifiers for 20 seconds"
            ],
            "variants": []
        }"#;
        let item: UniqueItemData = serde_json::from_str(json).unwrap();
        assert_eq!(item.name, "Headhunter");
        assert_eq!(item.base_type, "Leather Belt");
        assert_eq!(item.implicits.len(), 1);
        assert_eq!(item.explicits.len(), 5);
        assert!(item.variants.is_empty());
    }

    #[test]
    fn unique_with_variants_deserializes() {
        let json = r#"{
            "name": "Doryani's Invitation",
            "base_type": "Heavy Belt",
            "implicits": ["+25 to Strength"],
            "explicits": ["+100 to maximum Life"],
            "variants": ["Fire", "Cold", "Lightning", "Physical"]
        }"#;
        let item: UniqueItemData = serde_json::from_str(json).unwrap();
        assert_eq!(item.name, "Doryani's Invitation");
        assert_eq!(item.variants.len(), 4);
        assert_eq!(item.variants[0], "Fire");
    }

    #[test]
    fn minimal_unique_deserializes() {
        let json = r#"{
            "name": "Tabula Rasa",
            "base_type": "Simple Robe"
        }"#;
        let item: UniqueItemData = serde_json::from_str(json).unwrap();
        assert_eq!(item.name, "Tabula Rasa");
        assert_eq!(item.base_type, "Simple Robe");
        assert!(item.implicits.is_empty());
        assert!(item.explicits.is_empty());
        assert!(item.variants.is_empty());
    }

    #[test]
    fn unique_item_map_operations() {
        let items = vec![
            UniqueItemData {
                name: "Headhunter".to_string(),
                base_type: "Leather Belt".to_string(),
                implicits: vec![],
                explicits: vec![],
                variants: vec![],
            },
            UniqueItemData {
                name: "Tabula Rasa".to_string(),
                base_type: "Simple Robe".to_string(),
                implicits: vec![],
                explicits: vec![],
                variants: vec![],
            },
        ];
        let map = UniqueItemMap::from_vec(items);
        assert_eq!(map.len(), 2);
        assert!(!map.is_empty());
        assert!(map.get("Headhunter").is_some());
        assert_eq!(map.get("Headhunter").unwrap().base_type, "Leather Belt");
        assert!(map.get("Tabula Rasa").is_some());
        assert!(map.get("Nonexistent").is_none());
    }

    #[test]
    fn empty_unique_item_map() {
        let map = UniqueItemMap::from_vec(vec![]);
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
        assert!(map.get("anything").is_none());
    }
}
