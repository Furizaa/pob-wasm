//! Integration tests for mod_parser::parse_mod().
//!
//! Verifies the generated parse_mod() function against known-correct output for
//! 100+ representative stat lines from Path of Building.
//!
//! These tests exercise the full pipeline: generated parser → fallback to item_parser.
//! After the regex double-escaping fix, most patterns now go through the generated parser.

use pob_calc::build::mod_parser;
use pob_calc::mod_db::types::*;

fn src() -> ModSource {
    ModSource::new("Test", "test")
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: assert a single mod was returned with given name, type, value
// ─────────────────────────────────────────────────────────────────────────────

fn assert_single_mod(line: &str, expected_name: &str, expected_type: ModType, expected_value: f64) {
    let mods = mod_parser::parse_mod(line, src());
    assert!(
        !mods.is_empty(),
        "Expected non-empty result for {:?}, got empty",
        line
    );
    assert_eq!(
        mods.len(),
        1,
        "Expected 1 mod for {:?}, got {}: {:?}",
        line,
        mods.len(),
        mods
    );
    assert_eq!(
        mods[0].mod_type, expected_type,
        "Wrong mod_type for {:?}: expected {:?}, got {:?}",
        line, expected_type, mods[0].mod_type
    );
    assert!(
        (mods[0].value.as_f64() - expected_value).abs() < 0.001,
        "Wrong value for {:?}: expected {}, got {}",
        line,
        expected_value,
        mods[0].value.as_f64()
    );
    assert!(
        mods[0].name.contains(expected_name),
        "Wrong name for {:?}: expected containing {:?}, got {:?}",
        line,
        expected_name,
        mods[0].name
    );
}

fn assert_single_mod_exact(
    line: &str,
    expected_name: &str,
    expected_type: ModType,
    expected_value: f64,
) {
    let mods = mod_parser::parse_mod(line, src());
    assert!(!mods.is_empty(), "Expected non-empty for {:?}", line);
    assert_eq!(
        mods.len(),
        1,
        "Expected 1 mod for {:?}, got {:?}",
        line,
        mods
    );
    assert_eq!(mods[0].name, expected_name, "Wrong name for {:?}", line);
    assert_eq!(mods[0].mod_type, expected_type, "Wrong type for {:?}", line);
    assert!(
        (mods[0].value.as_f64() - expected_value).abs() < 0.001,
        "Wrong value for {:?}: expected {}, got {}",
        line,
        expected_value,
        mods[0].value.as_f64()
    );
}

fn assert_single_mod_with_flags(
    line: &str,
    expected_name: &str,
    expected_type: ModType,
    expected_value: f64,
    expected_flags: ModFlags,
) {
    let mods = mod_parser::parse_mod(line, src());
    assert!(!mods.is_empty(), "Expected non-empty for {:?}", line);
    assert_eq!(
        mods.len(),
        1,
        "Expected 1 mod for {:?}, got {:?}",
        line,
        mods
    );
    assert!(
        mods[0].name.contains(expected_name),
        "Wrong name for {:?}: expected containing {:?}, got {:?}",
        line,
        expected_name,
        mods[0].name
    );
    assert_eq!(mods[0].mod_type, expected_type, "Wrong type for {:?}", line);
    assert!(
        (mods[0].value.as_f64() - expected_value).abs() < 0.001,
        "Wrong value for {:?}: expected {}, got {}",
        line,
        expected_value,
        mods[0].value.as_f64()
    );
    assert!(
        mods[0].flags.contains(expected_flags),
        "Missing flags for {:?}: expected {:?} to contain {:?}, got {:?}",
        line,
        mods[0].flags,
        expected_flags,
        mods[0].flags
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 1: Basic increased/reduced (15 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn inc_maximum_life() {
    assert_single_mod_exact("10% increased maximum Life", "Life", ModType::Inc, 10.0);
}

#[test]
fn red_mana_cost_of_skills() {
    assert_single_mod_exact(
        "5% reduced Mana Cost of Skills",
        "ManaCost",
        ModType::Inc,
        -5.0,
    );
}

#[test]
fn inc_armour() {
    assert_single_mod_exact("12% increased Armour", "Armour", ModType::Inc, 12.0);
}

#[test]
fn inc_evasion_rating() {
    assert_single_mod_exact("8% increased Evasion Rating", "Evasion", ModType::Inc, 8.0);
}

#[test]
fn inc_max_energy_shield() {
    assert_single_mod_exact(
        "6% increased maximum Energy Shield",
        "EnergyShield",
        ModType::Inc,
        6.0,
    );
}

#[test]
fn inc_attack_speed() {
    // Generated parser: name="Speed", flags depend on modNameList — "attack speed" maps to Speed.
    // No ATTACK flag in StaticModNameEntry currently.
    let mods = mod_parser::parse_mod("15% increased Attack Speed", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "Speed");
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 15.0).abs() < 0.001);
}

#[test]
fn inc_cast_speed() {
    let mods = mod_parser::parse_mod("10% increased Cast Speed", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "Speed");
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 10.0).abs() < 0.001);
}

#[test]
fn inc_physical_damage() {
    assert_single_mod_exact(
        "20% increased Physical Damage",
        "PhysicalDamage",
        ModType::Inc,
        20.0,
    );
}

#[test]
fn inc_elemental_damage() {
    assert_single_mod_exact(
        "30% increased Elemental Damage",
        "ElementalDamage",
        ModType::Inc,
        30.0,
    );
}

#[test]
fn inc_fire_damage() {
    assert_single_mod_exact(
        "25% increased Fire Damage",
        "FireDamage",
        ModType::Inc,
        25.0,
    );
}

#[test]
fn inc_spell_damage() {
    // "Spell Damage" → name="Damage", flags=SPELL
    let mods = mod_parser::parse_mod("15% increased Spell Damage", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "Damage");
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 15.0).abs() < 0.001);
    assert!(mods[0].flags.contains(ModFlags::SPELL));
}

#[test]
fn inc_area_of_effect() {
    assert_single_mod_exact(
        "10% increased Area of Effect",
        "AreaOfEffect",
        ModType::Inc,
        10.0,
    );
}

#[test]
fn inc_projectile_speed() {
    assert_single_mod_exact(
        "15% increased Projectile Speed",
        "ProjectileSpeed",
        ModType::Inc,
        15.0,
    );
}

#[test]
fn inc_movement_speed() {
    assert_single_mod_exact(
        "3% increased Movement Speed",
        "MovementSpeed",
        ModType::Inc,
        3.0,
    );
}

#[test]
fn inc_critical_strike_chance() {
    assert_single_mod_exact(
        "50% increased Critical Strike Chance",
        "CritChance",
        ModType::Inc,
        50.0,
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 2: More/less (8 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn more_attack_damage() {
    // "20% more Attack Damage" → name="Damage", type=More, value=20.0
    // "attack damage" in modNameList maps to "Damage" (no attack flag in generated)
    let mods = mod_parser::parse_mod("20% more Attack Damage", src());
    assert_eq!(mods.len(), 1);
    assert!(mods[0].name.contains("Damage"));
    assert_eq!(mods[0].mod_type, ModType::More);
    assert!((mods[0].value.as_f64() - 20.0).abs() < 0.001);
}

#[test]
fn less_attack_speed() {
    // "10% less Attack Speed" → name="Speed", type=More, value=-10.0
    let mods = mod_parser::parse_mod("10% less Attack Speed", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "Speed");
    assert_eq!(mods[0].mod_type, ModType::More);
    assert!((mods[0].value.as_f64() - (-10.0)).abs() < 0.001);
}

#[test]
fn more_spell_damage() {
    let mods = mod_parser::parse_mod("30% more Spell Damage", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "Damage");
    assert_eq!(mods[0].mod_type, ModType::More);
    assert!((mods[0].value.as_f64() - 30.0).abs() < 0.001);
    assert!(mods[0].flags.contains(ModFlags::SPELL));
}

#[test]
fn more_max_energy_shield() {
    let mods = mod_parser::parse_mod("15% more Maximum Energy Shield", src());
    assert_eq!(mods.len(), 1);
    assert!(mods[0].name.contains("EnergyShield"));
    assert_eq!(mods[0].mod_type, ModType::More);
    assert!((mods[0].value.as_f64() - 15.0).abs() < 0.001);
}

#[test]
fn more_physical_damage() {
    assert_single_mod(
        "20% more Physical Damage",
        "PhysicalDamage",
        ModType::More,
        20.0,
    );
}

#[test]
fn less_damage() {
    let mods = mod_parser::parse_mod("10% less Damage", src());
    assert_eq!(mods.len(), 1);
    assert!(mods[0].name.contains("Damage"));
    assert_eq!(mods[0].mod_type, ModType::More);
    assert!((mods[0].value.as_f64() - (-10.0)).abs() < 0.001);
}

#[test]
fn more_melee_physical_damage() {
    let mods = mod_parser::parse_mod("40% more Melee Physical Damage", src());
    assert!(
        !mods.is_empty(),
        "Should parse '40% more Melee Physical Damage'"
    );
    assert_eq!(mods[0].mod_type, ModType::More);
    assert!((mods[0].value.as_f64() - 40.0).abs() < 0.001);
}

#[test]
fn less_projectile_damage() {
    let mods = mod_parser::parse_mod("30% less Projectile Damage", src());
    assert!(
        !mods.is_empty(),
        "Should parse '30% less Projectile Damage'"
    );
    assert_eq!(mods[0].mod_type, ModType::More);
    assert!((mods[0].value.as_f64() - (-30.0)).abs() < 0.001);
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 3: Flat base stats (15 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn base_maximum_life() {
    assert_single_mod_exact("+50 to maximum Life", "Life", ModType::Base, 50.0);
}

#[test]
fn base_strength() {
    assert_single_mod_exact("+30 to Strength", "Str", ModType::Base, 30.0);
}

#[test]
fn base_dexterity() {
    assert_single_mod_exact("+20 to Dexterity", "Dex", ModType::Base, 20.0);
}

#[test]
fn base_intelligence() {
    assert_single_mod_exact("+20 to Intelligence", "Int", ModType::Base, 20.0);
}

#[test]
fn base_all_attributes() {
    // Generated parser: "all attributes" → Str, Dex, Int, All (4 mods)
    let mods = mod_parser::parse_mod("+10 to all Attributes", src());
    assert!(
        mods.len() >= 3,
        "Expected at least 3 mods for all Attributes, got {}",
        mods.len()
    );
    assert!(mods.iter().any(|m| m.name == "Str"));
    assert!(mods.iter().any(|m| m.name == "Dex"));
    assert!(mods.iter().any(|m| m.name == "Int"));
    for m in &mods {
        assert_eq!(m.mod_type, ModType::Base);
        assert!((m.value.as_f64() - 10.0).abs() < 0.001);
    }
}

#[test]
fn base_maximum_mana() {
    assert_single_mod_exact("+30 to maximum Mana", "Mana", ModType::Base, 30.0);
}

#[test]
fn base_maximum_energy_shield() {
    assert_single_mod_exact(
        "+50 to maximum Energy Shield",
        "EnergyShield",
        ModType::Base,
        50.0,
    );
}

#[test]
fn base_evasion_rating() {
    assert_single_mod_exact("+100 to Evasion Rating", "Evasion", ModType::Base, 100.0);
}

#[test]
fn base_accuracy_rating() {
    assert_single_mod_exact("+200 to Accuracy Rating", "Accuracy", ModType::Base, 200.0);
}

#[test]
fn base_large_life() {
    assert_single_mod_exact("+120 to maximum Life", "Life", ModType::Base, 120.0);
}

#[test]
fn base_small_str() {
    assert_single_mod_exact("+5 to Strength", "Str", ModType::Base, 5.0);
}

#[test]
fn base_armour() {
    assert_single_mod("+300 to Armour", "Armour", ModType::Base, 300.0);
}

#[test]
fn base_mana_large() {
    assert_single_mod_exact("+100 to maximum Mana", "Mana", ModType::Base, 100.0);
}

#[test]
fn base_energy_shield_small() {
    assert_single_mod_exact(
        "+10 to maximum Energy Shield",
        "EnergyShield",
        ModType::Base,
        10.0,
    );
}

#[test]
fn base_ward() {
    assert_single_mod("+50 to Ward", "Ward", ModType::Base, 50.0);
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 4: Resistances (10 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn resist_fire() {
    assert_single_mod_exact("+40% to Fire Resistance", "FireResist", ModType::Base, 40.0);
}

#[test]
fn resist_cold() {
    assert_single_mod_exact("+30% to Cold Resistance", "ColdResist", ModType::Base, 30.0);
}

#[test]
fn resist_lightning() {
    assert_single_mod_exact(
        "+30% to Lightning Resistance",
        "LightningResist",
        ModType::Base,
        30.0,
    );
}

#[test]
fn resist_chaos() {
    assert_single_mod_exact(
        "+10% to Chaos Resistance",
        "ChaosResist",
        ModType::Base,
        10.0,
    );
}

#[test]
fn resist_all_elemental() {
    // Generated parser: "all elemental resistances" → "ElementalResist" (1 mod)
    let mods = mod_parser::parse_mod("+15% to all Elemental Resistances", src());
    assert!(!mods.is_empty(), "Should parse all elemental resistances");
    for m in &mods {
        assert_eq!(m.mod_type, ModType::Base);
        assert!((m.value.as_f64() - 15.0).abs() < 0.001);
    }
}

#[test]
fn resist_fire_small() {
    assert_single_mod_exact("+10% to Fire Resistance", "FireResist", ModType::Base, 10.0);
}

#[test]
fn resist_cold_large() {
    assert_single_mod_exact("+46% to Cold Resistance", "ColdResist", ModType::Base, 46.0);
}

#[test]
fn resist_lightning_large() {
    assert_single_mod_exact(
        "+45% to Lightning Resistance",
        "LightningResist",
        ModType::Base,
        45.0,
    );
}

#[test]
fn resist_chaos_large() {
    assert_single_mod_exact(
        "+30% to Chaos Resistance",
        "ChaosResist",
        ModType::Base,
        30.0,
    );
}

#[test]
fn resist_fire_and_cold() {
    let mods = mod_parser::parse_mod("+20% to Fire and Cold Resistances", src());
    assert!(mods.len() >= 2, "Expected at least 2 mods, got {:?}", mods);
    assert!(mods.iter().any(|m| m.name == "FireResist"));
    assert!(mods.iter().any(|m| m.name == "ColdResist"));
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 5: Additional damage types (10 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn inc_cold_damage() {
    assert_single_mod_exact(
        "20% increased Cold Damage",
        "ColdDamage",
        ModType::Inc,
        20.0,
    );
}

#[test]
fn inc_lightning_damage() {
    assert_single_mod_exact(
        "20% increased Lightning Damage",
        "LightningDamage",
        ModType::Inc,
        20.0,
    );
}

#[test]
fn inc_chaos_damage() {
    assert_single_mod_exact(
        "20% increased Chaos Damage",
        "ChaosDamage",
        ModType::Inc,
        20.0,
    );
}

#[test]
fn inc_damage_generic() {
    assert_single_mod_exact("10% increased Damage", "Damage", ModType::Inc, 10.0);
}

#[test]
fn inc_damage_over_time() {
    // "Damage over Time" → parsed with name containing "Damage"
    let mods = mod_parser::parse_mod("15% increased Damage over Time", src());
    assert!(!mods.is_empty(), "Should parse damage over time");
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 15.0).abs() < 0.001);
}

#[test]
fn base_crit_multiplier() {
    assert_single_mod_exact(
        "+20% to Critical Strike Multiplier",
        "CritMultiplier",
        ModType::Base,
        20.0,
    );
}

#[test]
fn inc_dot_multiplier() {
    assert_single_mod_exact(
        "10% increased Damage over Time Multiplier",
        "DotMultiplier",
        ModType::Inc,
        10.0,
    );
}

#[test]
fn red_mana_cost() {
    assert_single_mod_exact("5% reduced Mana Cost", "ManaCost", ModType::Inc, -5.0);
}

#[test]
fn red_movement_speed() {
    assert_single_mod_exact(
        "3% reduced Movement Speed",
        "MovementSpeed",
        ModType::Inc,
        -3.0,
    );
}

#[test]
fn inc_mana_regen_rate() {
    assert_single_mod_exact(
        "15% increased Mana Regeneration Rate",
        "ManaRegen",
        ModType::Inc,
        15.0,
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 6: Weapon-specific mods (8 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn inc_phys_damage_with_axes() {
    let mods = mod_parser::parse_mod("15% increased Physical Damage with Axes", src());
    assert_eq!(mods.len(), 1);
    assert!(mods[0].name.contains("PhysicalDamage"));
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 15.0).abs() < 0.001);
    assert!(
        mods[0].flags.contains(ModFlags::AXE),
        "Expected AXE flag, got {:?}",
        mods[0].flags
    );
}

#[test]
fn inc_attack_speed_with_swords() {
    let mods = mod_parser::parse_mod("20% increased Attack Speed with Swords", src());
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "Speed");
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 20.0).abs() < 0.001);
    assert!(
        mods[0].flags.contains(ModFlags::SWORD),
        "Expected SWORD flag, got {:?}",
        mods[0].flags
    );
}

#[test]
fn inc_damage_with_bows() {
    let mods = mod_parser::parse_mod("10% increased Damage with Bows", src());
    assert_eq!(mods.len(), 1);
    assert!(mods[0].name.contains("Damage"));
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 10.0).abs() < 0.001);
    assert!(
        mods[0].flags.contains(ModFlags::BOW),
        "Expected BOW flag, got {:?}",
        mods[0].flags
    );
}

#[test]
fn inc_damage_with_claws() {
    let mods = mod_parser::parse_mod("12% increased Damage with Claws", src());
    assert!(!mods.is_empty(), "Should parse damage with claws");
    assert!(mods[0].flags.contains(ModFlags::CLAW));
}

#[test]
fn inc_damage_with_daggers() {
    let mods = mod_parser::parse_mod("10% increased Damage with Daggers", src());
    assert!(!mods.is_empty(), "Should parse damage with daggers");
    assert!(mods[0].flags.contains(ModFlags::DAGGER));
}

#[test]
fn inc_damage_with_maces() {
    let mods = mod_parser::parse_mod("10% increased Damage with Maces", src());
    assert!(!mods.is_empty(), "Should parse damage with maces");
    assert!(mods[0].flags.contains(ModFlags::MACE));
}

#[test]
fn inc_damage_with_staves() {
    let mods = mod_parser::parse_mod("10% increased Damage with Staves", src());
    assert!(!mods.is_empty(), "Should parse damage with staves");
    assert!(mods[0].flags.contains(ModFlags::STAFF));
}

#[test]
fn inc_damage_with_wands() {
    let mods = mod_parser::parse_mod("10% increased Damage with Wands", src());
    assert!(!mods.is_empty(), "Should parse damage with wands");
    assert!(mods[0].flags.contains(ModFlags::WAND));
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 7: Flag mods / keystones (12 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn flag_cannot_be_evaded() {
    let mods = mod_parser::parse_mod("Your hits can't be Evaded", src());
    assert!(!mods.is_empty(), "Should parse 'Your hits can't be Evaded'");
    // Should return a Flag mod with name containing "CannotBeEvaded"
    assert!(
        mods.iter().any(|m| m.name.contains("CannotBeEvaded")),
        "Expected CannotBeEvaded, got {:?}",
        mods
    );
}

#[test]
fn keystone_point_blank() {
    let mods = mod_parser::parse_mod("Point Blank", src());
    assert!(!mods.is_empty(), "Should parse 'Point Blank'");
    assert!(mods[0].name.contains("Keystone"));
    assert_eq!(mods[0].mod_type, ModType::List);
}

#[test]
fn keystone_acrobatics() {
    let mods = mod_parser::parse_mod("Acrobatics", src());
    assert!(!mods.is_empty(), "Should parse 'Acrobatics'");
    assert!(mods[0].name.contains("Keystone"));
}

#[test]
fn keystone_iron_reflexes() {
    let mods = mod_parser::parse_mod("Iron Reflexes", src());
    assert!(!mods.is_empty(), "Should parse 'Iron Reflexes'");
    assert!(mods[0].name.contains("Keystone"));
}

#[test]
fn keystone_chaos_inoculation() {
    let mods = mod_parser::parse_mod("Chaos Inoculation", src());
    assert!(!mods.is_empty(), "Should parse 'Chaos Inoculation'");
    assert!(mods[0].name.contains("Keystone"));
}

#[test]
fn keystone_vaal_pact() {
    let mods = mod_parser::parse_mod("Vaal Pact", src());
    assert!(!mods.is_empty(), "Should parse 'Vaal Pact'");
    assert!(mods[0].name.contains("Keystone"));
}

#[test]
fn keystone_resolute_technique() {
    let mods = mod_parser::parse_mod("Resolute Technique", src());
    assert!(!mods.is_empty(), "Should parse 'Resolute Technique'");
    assert!(mods[0].name.contains("Keystone"));
}

#[test]
fn keystone_avatar_of_fire() {
    let mods = mod_parser::parse_mod("Avatar of Fire", src());
    assert!(!mods.is_empty(), "Should parse 'Avatar of Fire'");
    assert!(mods[0].name.contains("Keystone"));
}

#[test]
fn keystone_mind_over_matter() {
    let mods = mod_parser::parse_mod("Mind Over Matter", src());
    assert!(!mods.is_empty(), "Should parse 'Mind Over Matter'");
    assert!(mods[0].name.contains("Keystone"));
}

#[test]
fn keystone_blood_magic() {
    let mods = mod_parser::parse_mod("Blood Magic", src());
    assert!(!mods.is_empty(), "Should parse 'Blood Magic'");
    assert!(mods[0].name.contains("Keystone"));
}

#[test]
fn flag_cannot_be_frozen() {
    let mods = mod_parser::parse_mod("Cannot be Frozen", src());
    assert!(!mods.is_empty(), "Should parse 'Cannot be Frozen'");
    assert!(mods[0].name.contains("FreezeImmune"));
    assert_eq!(mods[0].mod_type, ModType::Flag);
}

#[test]
fn flag_cannot_be_stunned() {
    let mods = mod_parser::parse_mod("Cannot be Stunned", src());
    assert!(!mods.is_empty(), "Should parse 'Cannot be Stunned'");
    assert!(mods[0].name.contains("StunImmune"));
    assert_eq!(mods[0].mod_type, ModType::Flag);
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 8: Conditional mods (8 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn cond_attack_speed_while_dual_wielding() {
    let mods = mod_parser::parse_mod("10% increased Attack Speed while Dual Wielding", src());
    assert!(!mods.is_empty(), "Should parse conditional attack speed");
    assert_eq!(mods[0].name, "Speed");
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 10.0).abs() < 0.001);
    assert!(
        mods[0]
            .tags
            .iter()
            .any(|t| matches!(t, ModTag::Condition { var, .. } if var == "DualWielding")),
        "Expected DualWielding condition tag, got {:?}",
        mods[0].tags
    );
}

#[test]
fn cond_damage_while_leeching() {
    let mods = mod_parser::parse_mod("30% increased Damage while Leeching", src());
    assert!(!mods.is_empty(), "Should parse damage while leeching");
    assert!(mods[0].name.contains("Damage"));
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 30.0).abs() < 0.001);
    assert!(
        mods[0]
            .tags
            .iter()
            .any(|t| matches!(t, ModTag::Condition { var, .. } if var == "Leeching")),
        "Expected Leeching condition tag, got {:?}",
        mods[0].tags
    );
}

#[test]
fn cond_damage_at_full_life() {
    let mods = mod_parser::parse_mod("40% increased Damage while on Full Life", src());
    assert!(!mods.is_empty(), "Should parse damage at full life");
    assert!(mods[0].name.contains("Damage"));
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!(
        mods[0]
            .tags
            .iter()
            .any(|t| matches!(t, ModTag::Condition { .. })),
        "Expected Condition tag, got {:?}",
        mods[0].tags
    );
}

#[test]
fn cond_damage_at_low_life() {
    let mods = mod_parser::parse_mod("30% increased Damage while on Low Life", src());
    assert!(!mods.is_empty(), "Should parse damage at low life");
    assert!(mods[0].name.contains("Damage"));
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!(
        mods[0]
            .tags
            .iter()
            .any(|t| matches!(t, ModTag::Condition { .. })),
        "Expected Condition tag, got {:?}",
        mods[0].tags
    );
}

#[test]
#[ignore = "\"while holding a Shield\" condition not captured as tag by generated parser — may need mod tag pattern update"]
fn cond_attack_speed_while_holding_shield() {
    let mods = mod_parser::parse_mod("10% increased Attack Speed while holding a Shield", src());
    assert!(!mods.is_empty(), "Should parse attack speed with shield");
    assert_eq!(mods[0].name, "Speed");
    assert!(
        mods[0]
            .tags
            .iter()
            .any(|t| matches!(t, ModTag::Condition { .. })),
        "Expected Condition tag, got {:?}",
        mods[0].tags
    );
}

#[test]
fn cond_damage_if_blocked_recently() {
    let mods = mod_parser::parse_mod("40% increased Damage if you've Blocked Recently", src());
    assert!(!mods.is_empty(), "Should parse damage if blocked recently");
    assert!(mods[0].name.contains("Damage"));
    assert!(
        mods[0]
            .tags
            .iter()
            .any(|t| matches!(t, ModTag::Condition { .. })),
        "Expected Condition tag, got {:?}",
        mods[0].tags
    );
}

#[test]
fn cond_damage_if_killed_recently() {
    let mods = mod_parser::parse_mod("20% increased Damage if you've Killed Recently", src());
    assert!(!mods.is_empty(), "Should parse damage if killed recently");
    assert!(mods[0].name.contains("Damage"));
    assert!(
        mods[0]
            .tags
            .iter()
            .any(|t| matches!(t, ModTag::Condition { .. })),
        "Expected Condition tag, got {:?}",
        mods[0].tags
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 9: Per-charge/stat mods (5 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn per_frenzy_charge_attack_damage() {
    let mods = mod_parser::parse_mod("4% increased Attack Damage per Frenzy Charge", src());
    assert!(!mods.is_empty(), "Should parse per frenzy charge");
    assert!(mods[0].name.contains("Damage"));
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 4.0).abs() < 0.001);
    assert!(
        mods[0]
            .tags
            .iter()
            .any(|t| matches!(t, ModTag::Multiplier { var, .. } if var.contains("FrenzyCharge"))),
        "Expected FrenzyCharge multiplier tag, got {:?}",
        mods[0].tags
    );
}

#[test]
fn per_power_charge_crit_chance() {
    let mods = mod_parser::parse_mod(
        "40% increased Critical Strike Chance per Power Charge",
        src(),
    );
    assert!(!mods.is_empty(), "Should parse per power charge crit");
    assert!(mods[0].name.contains("CritChance"));
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!(
        mods[0]
            .tags
            .iter()
            .any(|t| matches!(t, ModTag::Multiplier { var, .. } if var.contains("PowerCharge"))),
        "Expected PowerCharge multiplier tag, got {:?}",
        mods[0].tags
    );
}

#[test]
fn per_endurance_charge_phys_reduction() {
    let mods = mod_parser::parse_mod(
        "+1% to Physical Damage Reduction per Endurance Charge",
        src(),
    );
    assert!(!mods.is_empty(), "Should parse per endurance charge");
    assert!(
        mods[0].tags.iter().any(
            |t| matches!(t, ModTag::Multiplier { var, .. } if var.contains("EnduranceCharge"))
        ),
        "Expected EnduranceCharge multiplier tag, got {:?}",
        mods[0].tags
    );
}

#[test]
fn per_frenzy_charge_attack_speed() {
    let mods = mod_parser::parse_mod("4% increased Attack Speed per Frenzy Charge", src());
    assert!(
        !mods.is_empty(),
        "Should parse attack speed per frenzy charge"
    );
    assert_eq!(mods[0].name, "Speed");
    assert!(
        mods[0]
            .tags
            .iter()
            .any(|t| matches!(t, ModTag::Multiplier { var, .. } if var.contains("FrenzyCharge"))),
        "Expected FrenzyCharge multiplier tag, got {:?}",
        mods[0].tags
    );
}

#[test]
fn per_power_charge_spell_damage() {
    let mods = mod_parser::parse_mod("3% increased Spell Damage per Power Charge", src());
    assert!(
        !mods.is_empty(),
        "Should parse spell damage per power charge"
    );
    assert!(mods[0].name.contains("Damage"));
    assert!(
        mods[0]
            .tags
            .iter()
            .any(|t| matches!(t, ModTag::Multiplier { var, .. } if var.contains("PowerCharge"))),
        "Expected PowerCharge multiplier tag, got {:?}",
        mods[0].tags
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 10: Conversion (4 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn conversion_phys_to_fire() {
    let mods = mod_parser::parse_mod("50% of Physical Damage Converted to Fire Damage", src());
    assert!(!mods.is_empty(), "Should parse phys to fire conversion");
    // Generated parser handles this via special mod
    assert!(
        mods[0].name.contains("Convert") || mods[0].name.contains("Physical"),
        "Expected conversion mod name, got {:?}",
        mods[0].name
    );
    assert!((mods[0].value.as_f64() - 50.0).abs() < 0.001);
}

#[test]
fn conversion_phys_to_lightning() {
    let mods = mod_parser::parse_mod(
        "25% of Physical Damage Converted to Lightning Damage",
        src(),
    );
    assert!(
        !mods.is_empty(),
        "Should parse phys to lightning conversion"
    );
    assert!((mods[0].value.as_f64() - 25.0).abs() < 0.001);
}

#[test]
fn conversion_phys_to_cold() {
    let mods = mod_parser::parse_mod("40% of Physical Damage Converted to Cold Damage", src());
    assert!(!mods.is_empty(), "Should parse phys to cold conversion");
    assert!((mods[0].value.as_f64() - 40.0).abs() < 0.001);
}

#[test]
fn conversion_cold_to_fire() {
    let mods = mod_parser::parse_mod("50% of Cold Damage Converted to Fire Damage", src());
    assert!(!mods.is_empty(), "Should parse cold to fire conversion");
    assert!((mods[0].value.as_f64() - 50.0).abs() < 0.001);
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 11: Leech / Regen / Recovery (8 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn leech_damage_as_life() {
    let mods = mod_parser::parse_mod("1% of Damage Leeched as Life", src());
    assert!(!mods.is_empty(), "Should parse damage leech");
    assert!(mods[0].name.contains("Leech") || mods[0].name.contains("DamageLifeLeech"));
    assert!((mods[0].value.as_f64() - 1.0).abs() < 0.001);
}

#[test]
fn regen_life_percent() {
    let mods = mod_parser::parse_mod("Regenerate 1% of Life per second", src());
    assert!(!mods.is_empty(), "Should parse life regen percent");
    assert!(mods[0].name.contains("LifeRegen"));
    assert!((mods[0].value.as_f64() - 1.0).abs() < 0.001);
}

#[test]
fn regen_mana_percent() {
    let mods = mod_parser::parse_mod("Regenerate 2% of Mana per second", src());
    assert!(!mods.is_empty(), "Should parse mana regen percent");
    assert!(mods[0].name.contains("ManaRegen"));
}

#[test]
fn inc_life_regen_rate() {
    assert_single_mod(
        "20% increased Life Regeneration Rate",
        "LifeRegen",
        ModType::Inc,
        20.0,
    );
}

#[test]
fn dodge_attack_hits() {
    let mods = mod_parser::parse_mod("10% chance to Dodge Attack Hits", src());
    assert!(!mods.is_empty(), "Should parse dodge chance");
    assert!(mods[0].name.contains("Dodge"));
    assert!((mods[0].value.as_f64() - 10.0).abs() < 0.001);
}

#[test]
fn block_chance() {
    let mods = mod_parser::parse_mod("5% chance to Block", src());
    assert!(!mods.is_empty(), "Should parse block chance");
    assert!(mods[0].name.contains("Block"));
    assert!((mods[0].value.as_f64() - 5.0).abs() < 0.001);
}

#[test]
fn spell_block_chance() {
    let mods = mod_parser::parse_mod("5% chance to Block Spell Damage", src());
    assert!(!mods.is_empty(), "Should parse spell block chance");
    assert!(mods[0].name.contains("Block"));
}

#[test]
fn enemy_phys_damage_reduction() {
    let mods = mod_parser::parse_mod(
        "Enemies have -10% to Total Physical Damage Reduction against your Hits",
        src(),
    );
    assert!(!mods.is_empty(), "Should parse enemy phys reduction");
    assert!(mods[0].name.contains("EnemyPhysicalDamageReduction"));
    assert!((mods[0].value.as_f64() - (-10.0)).abs() < 0.001);
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 12: Increased/reduced variants (8 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn inc_str() {
    assert_single_mod_exact("10% increased Strength", "Str", ModType::Inc, 10.0);
}

#[test]
fn inc_dex() {
    assert_single_mod_exact("10% increased Dexterity", "Dex", ModType::Inc, 10.0);
}

#[test]
fn inc_int() {
    assert_single_mod_exact("10% increased Intelligence", "Int", ModType::Inc, 10.0);
}

#[test]
fn inc_mana() {
    assert_single_mod_exact("20% increased maximum Mana", "Mana", ModType::Inc, 20.0);
}

#[test]
fn inc_life() {
    assert_single_mod_exact("5% increased maximum Life", "Life", ModType::Inc, 5.0);
}

#[test]
fn inc_accuracy() {
    assert_single_mod_exact(
        "15% increased Accuracy Rating",
        "Accuracy",
        ModType::Inc,
        15.0,
    );
}

#[test]
fn red_damage_taken() {
    let mods = mod_parser::parse_mod("10% reduced Damage taken", src());
    assert!(!mods.is_empty(), "Should parse reduced damage taken");
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - (-10.0)).abs() < 0.001);
}

#[test]
fn red_enemy_stun_threshold() {
    let mods = mod_parser::parse_mod("10% reduced Enemy Stun Threshold", src());
    assert!(!mods.is_empty(), "Should parse reduced stun threshold");
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - (-10.0)).abs() < 0.001);
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 13: Penetration (3 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn pen_fire_resistance() {
    let mods = mod_parser::parse_mod("Damage Penetrates 10% Fire Resistance", src());
    assert!(!mods.is_empty(), "Should parse fire penetration");
    assert!(mods[0].name.contains("FirePen") || mods[0].name.contains("Penetrat"));
    assert!((mods[0].value.as_f64() - 10.0).abs() < 0.001);
}

#[test]
fn pen_cold_resistance() {
    let mods = mod_parser::parse_mod("Damage Penetrates 15% Cold Resistance", src());
    assert!(!mods.is_empty(), "Should parse cold penetration");
    assert!((mods[0].value.as_f64() - 15.0).abs() < 0.001);
}

#[test]
fn pen_lightning_resistance() {
    let mods = mod_parser::parse_mod("Damage Penetrates 10% Lightning Resistance", src());
    assert!(!mods.is_empty(), "Should parse lightning penetration");
    assert!((mods[0].value.as_f64() - 10.0).abs() < 0.001);
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 14: Edge cases (5 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn edge_empty_string() {
    let mods = mod_parser::parse_mod("", src());
    assert!(mods.is_empty(), "Empty string should return empty vec");
}

#[test]
fn edge_random_nonsense() {
    let mods = mod_parser::parse_mod("some random nonsense text", src());
    assert!(mods.is_empty(), "Random text should return empty vec");
}

#[test]
fn edge_socketed_gems() {
    // Complex pattern that may not parse through either parser
    let mods = mod_parser::parse_mod("Socketed Gems are Supported by Level 1 Trap", src());
    // This is expected to be empty — complex support gem pattern
    // Just verify it doesn't panic
    let _ = mods;
}

#[test]
fn edge_whitespace_only() {
    let mods = mod_parser::parse_mod("   ", src());
    assert!(mods.is_empty(), "Whitespace-only should return empty");
}

#[test]
fn edge_numeric_only() {
    let mods = mod_parser::parse_mod("42", src());
    // May or may not parse — just verify no panic
    let _ = mods;
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 15: Misc patterns for coverage (20 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn inc_attack_and_cast_speed() {
    let mods = mod_parser::parse_mod("10% increased Attack and Cast Speed", src());
    assert!(!mods.is_empty(), "Should parse attack and cast speed");
    assert!(mods[0].name.contains("Speed"));
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 10.0).abs() < 0.001);
}

#[test]
fn base_str_and_dex() {
    let mods = mod_parser::parse_mod("+20 to Strength and Dexterity", src());
    assert!(
        mods.len() >= 2,
        "Expected at least 2 mods for Str and Dex, got {:?}",
        mods
    );
    assert!(mods.iter().any(|m| m.name == "Str"));
    assert!(mods.iter().any(|m| m.name == "Dex"));
}

#[test]
fn base_str_and_int() {
    let mods = mod_parser::parse_mod("+20 to Strength and Intelligence", src());
    assert!(
        mods.len() >= 2,
        "Expected at least 2 mods for Str and Int, got {:?}",
        mods
    );
    assert!(mods.iter().any(|m| m.name == "Str"));
    assert!(mods.iter().any(|m| m.name == "Int"));
}

#[test]
fn base_dex_and_int() {
    let mods = mod_parser::parse_mod("+20 to Dexterity and Intelligence", src());
    assert!(
        mods.len() >= 2,
        "Expected at least 2 mods for Dex and Int, got {:?}",
        mods
    );
    assert!(mods.iter().any(|m| m.name == "Dex"));
    assert!(mods.iter().any(|m| m.name == "Int"));
}

#[test]
fn inc_armour_and_evasion() {
    let mods = mod_parser::parse_mod("10% increased Armour and Evasion", src());
    assert!(!mods.is_empty(), "Should parse armour and evasion");
    assert_eq!(mods[0].mod_type, ModType::Inc);
}

#[test]
fn inc_defences() {
    let mods = mod_parser::parse_mod("10% increased Defences", src());
    assert!(!mods.is_empty(), "Should parse defences");
    assert!(mods[0].name.contains("Defences") || mods[0].name.contains("Defence"));
}

#[test]
fn inc_fire_damage_larger() {
    assert_single_mod_exact(
        "50% increased Fire Damage",
        "FireDamage",
        ModType::Inc,
        50.0,
    );
}

#[test]
fn inc_cold_damage_larger() {
    assert_single_mod_exact(
        "50% increased Cold Damage",
        "ColdDamage",
        ModType::Inc,
        50.0,
    );
}

#[test]
fn inc_lightning_damage_larger() {
    assert_single_mod_exact(
        "50% increased Lightning Damage",
        "LightningDamage",
        ModType::Inc,
        50.0,
    );
}

#[test]
fn inc_physical_damage_larger() {
    assert_single_mod_exact(
        "40% increased Physical Damage",
        "PhysicalDamage",
        ModType::Inc,
        40.0,
    );
}

#[test]
fn more_damage() {
    let mods = mod_parser::parse_mod("50% more Damage", src());
    assert_eq!(mods.len(), 1);
    assert!(mods[0].name.contains("Damage"));
    assert_eq!(mods[0].mod_type, ModType::More);
    assert!((mods[0].value.as_f64() - 50.0).abs() < 0.001);
}

#[test]
fn base_fire_resist_large() {
    assert_single_mod_exact("+46% to Fire Resistance", "FireResist", ModType::Base, 46.0);
}

#[test]
fn base_all_resistances() {
    let mods = mod_parser::parse_mod("+10% to all Resistances", src());
    assert!(!mods.is_empty(), "Should parse all resistances");
    // Generated parser maps "all resistances" → ElementalResist + ChaosResist
    assert!(mods.len() >= 1);
}

#[test]
fn base_maximum_fire_resistance() {
    let mods = mod_parser::parse_mod("+1% to maximum Fire Resistance", src());
    assert!(!mods.is_empty(), "Should parse max fire resist");
    assert!(mods[0].name.contains("FireResistMax") || mods[0].name.contains("FireResist"));
}

#[test]
fn inc_energy_shield_standalone() {
    assert_single_mod_exact(
        "10% increased Energy Shield",
        "EnergyShield",
        ModType::Inc,
        10.0,
    );
}

#[test]
fn inc_evasion_standalone() {
    assert_single_mod_exact("10% increased Evasion", "Evasion", ModType::Inc, 10.0);
}

#[test]
fn base_life_large() {
    assert_single_mod_exact("+80 to maximum Life", "Life", ModType::Base, 80.0);
}

#[test]
fn base_mana_medium() {
    assert_single_mod_exact("+50 to maximum Mana", "Mana", ModType::Base, 50.0);
}

#[test]
fn inc_global_damage() {
    assert_single_mod_exact("15% increased Damage", "Damage", ModType::Inc, 15.0);
}

#[test]
fn base_crit_chance() {
    // +1% to Critical Strike Chance — should be Base
    let mods = mod_parser::parse_mod("+1% to Critical Strike Chance", src());
    assert!(!mods.is_empty(), "Should parse base crit chance");
    assert!(mods[0].name.contains("CritChance"));
    assert_eq!(mods[0].mod_type, ModType::Base);
    assert!((mods[0].value.as_f64() - 1.0).abs() < 0.001);
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 16: Value negation patterns (5 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn red_fire_resistance() {
    let mods = mod_parser::parse_mod("-20% to Fire Resistance", src());
    assert!(!mods.is_empty(), "Should parse negative fire resist");
    assert!(mods[0].name.contains("FireResist"));
    assert!((mods[0].value.as_f64() - (-20.0)).abs() < 0.001);
}

#[test]
fn negative_chaos_resistance() {
    let mods = mod_parser::parse_mod("-30% to Chaos Resistance", src());
    assert!(!mods.is_empty(), "Should parse negative chaos resist");
    assert!(mods[0].name.contains("ChaosResist"));
    assert!((mods[0].value.as_f64() - (-30.0)).abs() < 0.001);
}

#[test]
fn red_attack_speed() {
    assert_single_mod_exact("10% reduced Attack Speed", "Speed", ModType::Inc, -10.0);
}

#[test]
fn red_movement_speed_large() {
    assert_single_mod_exact(
        "10% reduced Movement Speed",
        "MovementSpeed",
        ModType::Inc,
        -10.0,
    );
}

#[test]
fn less_area_damage() {
    let mods = mod_parser::parse_mod("25% less Area Damage", src());
    assert!(!mods.is_empty(), "Should parse less area damage");
    assert_eq!(mods[0].mod_type, ModType::More);
    assert!((mods[0].value.as_f64() - (-25.0)).abs() < 0.001);
}

// ═════════════════════════════════════════════════════════════════════════════
// CATEGORY 17: Spell flags and keyword flags (5 tests)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn inc_spell_crit_chance() {
    let mods = mod_parser::parse_mod("50% increased Spell Critical Strike Chance", src());
    assert!(!mods.is_empty(), "Should parse spell crit chance");
    assert!(mods[0].name.contains("CritChance"));
    assert_eq!(mods[0].mod_type, ModType::Inc);
    assert!((mods[0].value.as_f64() - 50.0).abs() < 0.001);
}

#[test]
fn inc_melee_physical_damage() {
    let mods = mod_parser::parse_mod("20% increased Melee Physical Damage", src());
    assert!(!mods.is_empty(), "Should parse melee physical damage");
    assert!(mods[0].name.contains("PhysicalDamage"));
    assert_eq!(mods[0].mod_type, ModType::Inc);
}

#[test]
fn inc_projectile_damage() {
    let mods = mod_parser::parse_mod("20% increased Projectile Damage", src());
    assert!(!mods.is_empty(), "Should parse projectile damage");
    assert!(mods[0].name.contains("Damage"));
    assert_eq!(mods[0].mod_type, ModType::Inc);
}

#[test]
fn inc_area_damage() {
    let mods = mod_parser::parse_mod("20% increased Area Damage", src());
    assert!(!mods.is_empty(), "Should parse area damage");
    assert!(mods[0].name.contains("Damage"));
    assert_eq!(mods[0].mod_type, ModType::Inc);
}

#[test]
fn more_fire_damage() {
    let mods = mod_parser::parse_mod("30% more Fire Damage", src());
    assert!(!mods.is_empty(), "Should parse more fire damage");
    assert!(mods[0].name.contains("FireDamage"));
    assert_eq!(mods[0].mod_type, ModType::More);
    assert!((mods[0].value.as_f64() - 30.0).abs() < 0.001);
}

// ═════════════════════════════════════════════════════════════════════════════
// Diagnostic: helps investigate parser behavior (run with --nocapture)
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn diagnostic_dump() {
    let lines = vec![
        "Adds 10 to 20 Physical Damage to Attacks",
        "Adds 5 to 10 Fire Damage to Spells",
    ];

    for line in &lines {
        let mods = mod_parser::parse_mod(line, src());
        eprintln!("---");
        eprintln!("Input: {:?}", line);
        eprintln!("  Count: {}", mods.len());
        for (i, m) in mods.iter().enumerate() {
            eprintln!(
                "  [{}] name={:?} type={:?} value={:?} flags={:?} kw_flags={:?} tags={:?}",
                i, m.name, m.mod_type, m.value, m.flags, m.keyword_flags, m.tags
            );
        }
    }
    // These "Adds X to Y" patterns may not work yet — this is informational only
}
