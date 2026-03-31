use crate::build::types::{Item, ItemArmourData, ItemFlaskData, ItemRequirements, ItemWeaponData};
use crate::data::bases::BaseItemMap;

/// Resolve base item stats from GameData.bases and apply quality scaling.
///
/// Given an `Item` whose `base_type` has been parsed from item text, looks up
/// the base in `bases` and populates weapon/armour/flask data with quality
/// scaling applied.
///
/// Quality scaling rules:
/// - **Weapon physical damage**: `base * (1 + quality / 100)`
/// - **Armour / Evasion / ES**: `avg(min, max) * (1 + quality / 100)`
/// - **Ward**: `avg(min, max)` — no quality scaling
/// - **Block**: no quality scaling
pub fn resolve_item_base(item: &mut Item, bases: &BaseItemMap) {
    let base = match bases.get(&item.base_type) {
        Some(b) => b,
        None => return,
    };

    // 1. Set item_type from base
    item.item_type = base.item_type.clone();

    // 2. Quality factor
    let quality_factor = 1.0 + item.quality as f64 / 100.0;

    // 3. Weapon data with quality scaling on physical damage
    if let Some(ref weapon) = base.weapon {
        item.weapon_data = Some(ItemWeaponData {
            phys_min: weapon.physical_min * quality_factor,
            phys_max: weapon.physical_max * quality_factor,
            attack_rate: weapon.attack_rate_base,
            crit_chance: weapon.crit_chance_base,
            range: weapon.range,
        });
    }

    // 4. Armour data with quality scaling on armour/evasion/ES, no scaling on ward/block
    if let Some(ref armour) = base.armour {
        let avg = |min: f64, max: f64| (min + max) / 2.0;

        item.armour_data = Some(ItemArmourData {
            armour: avg(armour.armour_min, armour.armour_max) * quality_factor,
            evasion: avg(armour.evasion_min, armour.evasion_max) * quality_factor,
            energy_shield: avg(armour.energy_shield_min, armour.energy_shield_max) * quality_factor,
            ward: avg(armour.ward_min, armour.ward_max),
            block: armour.block_chance as f64,
        });
    }

    // 5. Flask data (no quality scaling)
    if let Some(ref flask) = base.flask {
        item.flask_data = Some(ItemFlaskData {
            life: flask.life,
            mana: flask.mana,
            duration: flask.duration,
            charges_used: flask.charges_used,
            charges_max: flask.charges_max,
        });
    }

    // 6. Requirements from base
    if let Some(ref req) = base.req {
        item.requirements = ItemRequirements {
            level: req.level,
            str_req: req.str_req,
            dex_req: req.dex_req,
            int_req: req.int_req,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::types::{ItemInfluence, ItemRarity};
    use crate::data::bases::{
        ArmourData, BaseItemData, BaseItemMap, BaseRequirements, FlaskData, WeaponData,
    };

    /// Helper to build a minimal item for testing.
    fn make_item(base_type: &str, quality: u32) -> Item {
        Item {
            id: 1,
            rarity: ItemRarity::Normal,
            name: String::new(),
            base_type: base_type.to_string(),
            item_type: String::new(),
            quality,
            sockets: Vec::new(),
            implicits: Vec::new(),
            explicits: Vec::new(),
            crafted_mods: Vec::new(),
            enchant_mods: Vec::new(),
            corrupted: false,
            influence: ItemInfluence::default(),
            weapon_data: None,
            armour_data: None,
            flask_data: None,
            requirements: ItemRequirements::default(),
        }
    }

    fn weapon_base() -> BaseItemData {
        BaseItemData {
            name: "Rusted Sword".to_string(),
            item_type: "One Handed Sword".to_string(),
            sub_type: Some("Sword".to_string()),
            socket_limit: 3,
            tags: vec!["sword".into(), "weapon".into()],
            implicit: vec![],
            weapon: Some(WeaponData {
                physical_min: 10.0,
                physical_max: 20.0,
                crit_chance_base: 5.0,
                attack_rate_base: 1.4,
                range: 11,
            }),
            armour: None,
            flask: None,
            req: Some(BaseRequirements {
                level: 1,
                str_req: 8,
                dex_req: 0,
                int_req: 0,
            }),
        }
    }

    fn armour_base() -> BaseItemData {
        BaseItemData {
            name: "Vaal Regalia".to_string(),
            item_type: "Body Armour".to_string(),
            sub_type: Some("Body Armour".to_string()),
            socket_limit: 6,
            tags: vec!["body_armour".into()],
            implicit: vec![],
            weapon: None,
            armour: Some(ArmourData {
                armour_min: 0.0,
                armour_max: 0.0,
                evasion_min: 0.0,
                evasion_max: 0.0,
                energy_shield_min: 175.0,
                energy_shield_max: 210.0,
                ward_min: 0.0,
                ward_max: 0.0,
                block_chance: 0,
                movement_penalty: 3,
            }),
            flask: None,
            req: Some(BaseRequirements {
                level: 68,
                str_req: 0,
                dex_req: 0,
                int_req: 194,
            }),
        }
    }

    fn flask_base() -> BaseItemData {
        BaseItemData {
            name: "Divine Life Flask".to_string(),
            item_type: "Flask".to_string(),
            sub_type: None,
            socket_limit: 0,
            tags: vec!["flask".into()],
            implicit: vec![],
            weapon: None,
            armour: None,
            flask: Some(FlaskData {
                life: 2400.0,
                mana: 0.0,
                duration: 7.0,
                charges_used: 15,
                charges_max: 45,
            }),
            req: Some(BaseRequirements {
                level: 35,
                str_req: 0,
                dex_req: 0,
                int_req: 0,
            }),
        }
    }

    fn make_bases(items: Vec<BaseItemData>) -> BaseItemMap {
        BaseItemMap::from_vec(items)
    }

    #[test]
    fn weapon_base_resolution_with_quality() {
        let bases = make_bases(vec![weapon_base()]);
        let mut item = make_item("Rusted Sword", 20);
        resolve_item_base(&mut item, &bases);

        assert_eq!(item.item_type, "One Handed Sword");

        let wd = item
            .weapon_data
            .as_ref()
            .expect("weapon_data should be set");
        // phys_min = 10.0 * (1 + 20/100) = 10.0 * 1.2 = 12.0
        assert!(
            (wd.phys_min - 12.0).abs() < 1e-9,
            "phys_min: {}",
            wd.phys_min
        );
        // phys_max = 20.0 * 1.2 = 24.0
        assert!(
            (wd.phys_max - 24.0).abs() < 1e-9,
            "phys_max: {}",
            wd.phys_max
        );
        assert!((wd.attack_rate - 1.4).abs() < 1e-9);
        assert!((wd.crit_chance - 5.0).abs() < 1e-9);
        assert_eq!(wd.range, 11);

        assert!(item.armour_data.is_none());
        assert!(item.flask_data.is_none());

        assert_eq!(item.requirements.level, 1);
        assert_eq!(item.requirements.str_req, 8);
    }

    #[test]
    fn armour_base_resolution_with_quality() {
        let bases = make_bases(vec![armour_base()]);
        let mut item = make_item("Vaal Regalia", 20);
        resolve_item_base(&mut item, &bases);

        assert_eq!(item.item_type, "Body Armour");

        let ad = item
            .armour_data
            .as_ref()
            .expect("armour_data should be set");
        // ES avg = (175 + 210) / 2 = 192.5, scaled = 192.5 * 1.2 = 231.0
        assert!(
            (ad.energy_shield - 231.0).abs() < 1e-9,
            "energy_shield: {}",
            ad.energy_shield
        );
        // armour and evasion are 0
        assert!((ad.armour - 0.0).abs() < 1e-9);
        assert!((ad.evasion - 0.0).abs() < 1e-9);
        // ward = 0 (no quality scaling)
        assert!((ad.ward - 0.0).abs() < 1e-9);
        // block = 0
        assert!((ad.block - 0.0).abs() < 1e-9);

        assert!(item.weapon_data.is_none());
        assert!(item.flask_data.is_none());

        assert_eq!(item.requirements.level, 68);
        assert_eq!(item.requirements.int_req, 194);
    }

    #[test]
    fn flask_base_resolution() {
        let bases = make_bases(vec![flask_base()]);
        let mut item = make_item("Divine Life Flask", 20);
        resolve_item_base(&mut item, &bases);

        assert_eq!(item.item_type, "Flask");

        let fd = item.flask_data.as_ref().expect("flask_data should be set");
        assert!((fd.life - 2400.0).abs() < 1e-9);
        assert!((fd.mana - 0.0).abs() < 1e-9);
        assert!((fd.duration - 7.0).abs() < 1e-9);
        assert_eq!(fd.charges_used, 15);
        assert_eq!(fd.charges_max, 45);

        assert!(item.weapon_data.is_none());
        assert!(item.armour_data.is_none());

        assert_eq!(item.requirements.level, 35);
    }

    #[test]
    fn unknown_base_type_is_noop() {
        let bases = make_bases(vec![weapon_base()]);
        let mut item = make_item("Nonexistent Sword", 20);
        let original_type = item.item_type.clone();

        resolve_item_base(&mut item, &bases);

        // Nothing should change
        assert_eq!(item.item_type, original_type);
        assert!(item.weapon_data.is_none());
        assert!(item.armour_data.is_none());
        assert!(item.flask_data.is_none());
        assert_eq!(item.requirements, ItemRequirements::default());
    }

    #[test]
    fn zero_quality_no_scaling() {
        let bases = make_bases(vec![weapon_base()]);
        let mut item = make_item("Rusted Sword", 0);
        resolve_item_base(&mut item, &bases);

        let wd = item
            .weapon_data
            .as_ref()
            .expect("weapon_data should be set");
        // With 0 quality: factor = 1.0, so phys values are base values
        assert!(
            (wd.phys_min - 10.0).abs() < 1e-9,
            "phys_min: {}",
            wd.phys_min
        );
        assert!(
            (wd.phys_max - 20.0).abs() < 1e-9,
            "phys_max: {}",
            wd.phys_max
        );
    }

    #[test]
    fn zero_quality_armour_no_scaling() {
        let bases = make_bases(vec![armour_base()]);
        let mut item = make_item("Vaal Regalia", 0);
        resolve_item_base(&mut item, &bases);

        let ad = item
            .armour_data
            .as_ref()
            .expect("armour_data should be set");
        // ES avg = 192.5, factor = 1.0 → 192.5
        assert!(
            (ad.energy_shield - 192.5).abs() < 1e-9,
            "energy_shield: {}",
            ad.energy_shield
        );
    }

    #[test]
    fn ward_has_no_quality_scaling() {
        let base = BaseItemData {
            name: "Ward Base".to_string(),
            item_type: "Armour".to_string(),
            sub_type: None,
            socket_limit: 4,
            tags: vec![],
            implicit: vec![],
            weapon: None,
            armour: Some(ArmourData {
                armour_min: 0.0,
                armour_max: 0.0,
                evasion_min: 0.0,
                evasion_max: 0.0,
                energy_shield_min: 0.0,
                energy_shield_max: 0.0,
                ward_min: 80.0,
                ward_max: 100.0,
                block_chance: 30,
                movement_penalty: 0,
            }),
            flask: None,
            req: None,
        };
        let bases = make_bases(vec![base]);
        let mut item = make_item("Ward Base", 20);
        resolve_item_base(&mut item, &bases);

        let ad = item.armour_data.as_ref().unwrap();
        // ward avg = (80 + 100) / 2 = 90, NO quality scaling
        assert!((ad.ward - 90.0).abs() < 1e-9, "ward: {}", ad.ward);
        // block = 30, no quality scaling
        assert!((ad.block - 30.0).abs() < 1e-9, "block: {}", ad.block);
    }
}
