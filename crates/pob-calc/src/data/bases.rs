use serde::Deserialize;
use std::collections::HashMap;

/// Weapon-specific stats for a base item.
#[derive(Debug, Clone, Deserialize)]
pub struct WeaponData {
    #[serde(default)]
    pub physical_min: f64,
    #[serde(default)]
    pub physical_max: f64,
    #[serde(default)]
    pub crit_chance_base: f64,
    #[serde(default)]
    pub attack_rate_base: f64,
    #[serde(default)]
    pub range: u32,
}

/// Armour-specific stats for a base item.
#[derive(Debug, Clone, Deserialize)]
pub struct ArmourData {
    #[serde(default)]
    pub armour_min: f64,
    #[serde(default)]
    pub armour_max: f64,
    #[serde(default)]
    pub evasion_min: f64,
    #[serde(default)]
    pub evasion_max: f64,
    #[serde(default)]
    pub energy_shield_min: f64,
    #[serde(default)]
    pub energy_shield_max: f64,
    #[serde(default)]
    pub ward_min: f64,
    #[serde(default)]
    pub ward_max: f64,
    #[serde(default)]
    pub block_chance: u32,
    #[serde(default)]
    pub movement_penalty: u32,
}

/// Flask-specific stats for a base item.
#[derive(Debug, Clone, Deserialize)]
pub struct FlaskData {
    #[serde(default)]
    pub life: f64,
    #[serde(default)]
    pub mana: f64,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub charges_used: u32,
    #[serde(default)]
    pub charges_max: u32,
}

/// Level and attribute requirements for a base item.
#[derive(Debug, Clone, Deserialize)]
pub struct BaseRequirements {
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub str_req: u32,
    #[serde(default)]
    pub dex_req: u32,
    #[serde(default)]
    pub int_req: u32,
}

/// A single base item type (e.g. "Vaal Regalia", "Harbinger Bow").
#[derive(Debug, Clone, Deserialize)]
pub struct BaseItemData {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub item_type: String,
    #[serde(default)]
    pub sub_type: Option<String>,
    #[serde(default)]
    pub socket_limit: u32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub implicit: Vec<String>,
    #[serde(default)]
    pub weapon: Option<WeaponData>,
    #[serde(default)]
    pub armour: Option<ArmourData>,
    #[serde(default)]
    pub flask: Option<FlaskData>,
    #[serde(default)]
    pub req: Option<BaseRequirements>,
}

/// Lookup map for base items, keyed by name.
#[derive(Debug, Clone)]
pub struct BaseItemMap {
    inner: HashMap<String, BaseItemData>,
}

impl BaseItemMap {
    /// Build the map from a flat list of base items.
    pub fn from_vec(items: Vec<BaseItemData>) -> Self {
        let inner = items
            .into_iter()
            .map(|item| (item.name.clone(), item))
            .collect();
        Self { inner }
    }

    pub fn get(&self, name: &str) -> Option<&BaseItemData> {
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
    fn weapon_base_deserializes() {
        let json = r#"{
            "name": "Harbinger Bow",
            "item_type": "Weapon",
            "sub_type": "Bow",
            "socket_limit": 6,
            "tags": ["bow", "ranged", "weapon"],
            "implicit": ["+1 to Level of Socketed Bow Gems"],
            "weapon": {
                "physical_min": 30.0,
                "physical_max": 79.0,
                "crit_chance_base": 5.0,
                "attack_rate_base": 1.2,
                "range": 0
            },
            "req": {
                "level": 50,
                "str_req": 0,
                "dex_req": 170,
                "int_req": 0
            }
        }"#;
        let base: BaseItemData = serde_json::from_str(json).unwrap();
        assert_eq!(base.name, "Harbinger Bow");
        assert_eq!(base.item_type, "Weapon");
        assert_eq!(base.sub_type, Some("Bow".to_string()));
        assert_eq!(base.socket_limit, 6);
        assert_eq!(base.tags, vec!["bow", "ranged", "weapon"]);
        assert_eq!(base.implicit.len(), 1);

        let weapon = base.weapon.unwrap();
        assert!((weapon.physical_min - 30.0).abs() < f64::EPSILON);
        assert!((weapon.physical_max - 79.0).abs() < f64::EPSILON);
        assert!((weapon.crit_chance_base - 5.0).abs() < f64::EPSILON);
        assert!((weapon.attack_rate_base - 1.2).abs() < f64::EPSILON);

        let req = base.req.unwrap();
        assert_eq!(req.level, 50);
        assert_eq!(req.dex_req, 170);
        assert!(base.armour.is_none());
        assert!(base.flask.is_none());
    }

    #[test]
    fn armour_base_deserializes() {
        let json = r#"{
            "name": "Vaal Regalia",
            "item_type": "Armour",
            "sub_type": "Body Armour",
            "socket_limit": 6,
            "tags": ["body_armour", "armour", "int_armour"],
            "implicit": [],
            "armour": {
                "armour_min": 0.0,
                "armour_max": 0.0,
                "evasion_min": 0.0,
                "evasion_max": 0.0,
                "energy_shield_min": 175.0,
                "energy_shield_max": 210.0,
                "ward_min": 0.0,
                "ward_max": 0.0,
                "block_chance": 0,
                "movement_penalty": 3
            },
            "req": {
                "level": 68,
                "str_req": 0,
                "dex_req": 0,
                "int_req": 194
            }
        }"#;
        let base: BaseItemData = serde_json::from_str(json).unwrap();
        assert_eq!(base.name, "Vaal Regalia");
        let armour = base.armour.unwrap();
        assert!((armour.energy_shield_min - 175.0).abs() < f64::EPSILON);
        assert!((armour.energy_shield_max - 210.0).abs() < f64::EPSILON);
        assert_eq!(armour.movement_penalty, 3);
        assert!(base.weapon.is_none());
    }

    #[test]
    fn flask_base_deserializes() {
        let json = r#"{
            "name": "Divine Life Flask",
            "item_type": "Flask",
            "socket_limit": 0,
            "tags": ["flask", "life_flask"],
            "implicit": [],
            "flask": {
                "life": 2400.0,
                "mana": 0.0,
                "duration": 7.0,
                "charges_used": 15,
                "charges_max": 45
            }
        }"#;
        let base: BaseItemData = serde_json::from_str(json).unwrap();
        assert_eq!(base.name, "Divine Life Flask");
        let flask = base.flask.unwrap();
        assert!((flask.life - 2400.0).abs() < f64::EPSILON);
        assert!((flask.duration - 7.0).abs() < f64::EPSILON);
        assert_eq!(flask.charges_used, 15);
        assert_eq!(flask.charges_max, 45);
    }

    #[test]
    fn minimal_base_deserializes() {
        let json = r#"{
            "name": "Simple Robe",
            "item_type": "Armour"
        }"#;
        let base: BaseItemData = serde_json::from_str(json).unwrap();
        assert_eq!(base.name, "Simple Robe");
        assert!(base.sub_type.is_none());
        assert_eq!(base.socket_limit, 0);
        assert!(base.tags.is_empty());
        assert!(base.implicit.is_empty());
        assert!(base.weapon.is_none());
        assert!(base.armour.is_none());
        assert!(base.flask.is_none());
        assert!(base.req.is_none());
    }

    #[test]
    fn base_item_map_operations() {
        let items = vec![
            BaseItemData {
                name: "Short Bow".to_string(),
                item_type: "Weapon".to_string(),
                sub_type: Some("Bow".to_string()),
                socket_limit: 4,
                tags: vec![],
                implicit: vec![],
                weapon: None,
                armour: None,
                flask: None,
                req: None,
            },
            BaseItemData {
                name: "Vaal Regalia".to_string(),
                item_type: "Armour".to_string(),
                sub_type: Some("Body Armour".to_string()),
                socket_limit: 6,
                tags: vec![],
                implicit: vec![],
                weapon: None,
                armour: None,
                flask: None,
                req: None,
            },
        ];
        let map = BaseItemMap::from_vec(items);
        assert_eq!(map.len(), 2);
        assert!(!map.is_empty());
        assert!(map.get("Short Bow").is_some());
        assert!(map.get("Vaal Regalia").is_some());
        assert!(map.get("Nonexistent").is_none());
    }

    #[test]
    fn empty_base_item_map() {
        let map = BaseItemMap::from_vec(vec![]);
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
        assert!(map.get("anything").is_none());
    }
}
