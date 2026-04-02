pub mod build;
pub mod calc;
pub mod data;
pub mod error;
pub mod mod_db;
pub mod passive_tree;
pub mod timeless_jewels;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
pub mod tests {
    #[test]
    fn version_is_semver() {
        let v = super::version();
        assert!(v.contains('.'));
    }

    pub fn stub_game_data_json() -> &'static str {
        r#"{
            "gems": {},
            "misc": {
                "game_constants": {
                    "base_maximum_all_resistances_%": 75,
                    "maximum_block_%": 75,
                    "base_maximum_spell_block_%": 75,
                    "max_power_charges": 3,
                    "max_frenzy_charges": 3,
                    "max_endurance_charges": 3,
                    "maximum_life_leech_rate_%_per_minute": 20,
                    "maximum_mana_leech_rate_%_per_minute": 20,
                    "maximum_life_leech_amount_per_leech_%_max_life": 10,
                    "maximum_mana_leech_amount_per_leech_%_max_mana": 10,
                    "maximum_energy_shield_leech_amount_per_leech_%_max_energy_shield": 10,
                    "base_number_of_totems_allowed": 1,
                    "impaled_debuff_number_of_reflected_hits": 8,
                    "soul_eater_maximum_stacks": 40,
                    "maximum_righteous_charges": 10,
                    "maximum_blood_scythe_charges": 8
                },
                "character_constants": {"life_per_str": 0.5},
                "monster_life_table": [],
                "monster_damage_table": [],
                "monster_evasion_table": [],
                "monster_accuracy_table": [],
                "monster_ally_life_table": [],
                "monster_ally_damage_table": [],
                "monster_ailment_threshold_table": [],
                "monster_phys_conversion_multi_table": []
            }
        }"#
    }
}
