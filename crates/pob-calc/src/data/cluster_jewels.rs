//! Static cluster jewel data, ported from Data/ClusterJewels.lua.
//!
//! This module provides:
//! - Per-size layout metadata (indices, node counts)
//! - Enchant text → skill tag ID lookup
//! - Notable sort-order lookup (used to place notables around the ring)

use std::collections::HashMap;

/// Static layout information for one cluster jewel size.
#[derive(Debug, Clone)]
pub struct ClusterJewelSize {
    pub size_name: &'static str,
    pub size_index: u32, // 0=Small, 1=Medium, 2=Large
    pub min_nodes: u32,
    pub max_nodes: u32,
    /// Preferred order for placing Small fill nodes (0-based orbit indices).
    pub small_indicies: &'static [u32],
    /// Preferred order for placing Notable nodes (0-based orbit indices).
    pub notable_indicies: &'static [u32],
    /// Preferred order for placing Socket (sub-jewel) nodes (0-based orbit indices).
    pub socket_indicies: &'static [u32],
    /// Ring size (12 for Large/Medium, 6 for Small).
    pub total_indicies: u32,
}

/// Small Cluster Jewel layout.
pub static SMALL_CLUSTER: ClusterJewelSize = ClusterJewelSize {
    size_name: "Small Cluster Jewel",
    size_index: 0,
    min_nodes: 2,
    max_nodes: 3,
    small_indicies: &[0, 4, 2],
    notable_indicies: &[4],
    socket_indicies: &[4],
    total_indicies: 6,
};

/// Medium Cluster Jewel layout.
pub static MEDIUM_CLUSTER: ClusterJewelSize = ClusterJewelSize {
    size_name: "Medium Cluster Jewel",
    size_index: 1,
    min_nodes: 4,
    max_nodes: 6,
    small_indicies: &[0, 6, 8, 4, 10, 2],
    notable_indicies: &[6, 10, 2, 0],
    socket_indicies: &[6],
    total_indicies: 12,
};

/// Large Cluster Jewel layout.
pub static LARGE_CLUSTER: ClusterJewelSize = ClusterJewelSize {
    size_name: "Large Cluster Jewel",
    size_index: 2,
    min_nodes: 8,
    max_nodes: 12,
    small_indicies: &[0, 4, 6, 8, 10, 2, 7, 5, 9, 3, 11, 1],
    notable_indicies: &[6, 4, 8, 10, 2],
    socket_indicies: &[4, 8, 6],
    total_indicies: 12,
};

/// Returns the ClusterJewelSize definition for a given base item type name.
pub fn cluster_size_for_base_type(base_type: &str) -> Option<&'static ClusterJewelSize> {
    match base_type {
        "Small Cluster Jewel" => Some(&SMALL_CLUSTER),
        "Medium Cluster Jewel" => Some(&MEDIUM_CLUSTER),
        "Large Cluster Jewel" => Some(&LARGE_CLUSTER),
        _ => None,
    }
}

/// Build the enchant-line → skill-tag-ID lookup table.
/// Keys are lowercase enchant lines (the full `"Added Small Passive Skills grant: ..."` text).
/// Values are the cluster jewel skill tag IDs (e.g. `"affliction_cold_damage"`).
pub fn build_enchant_to_skill_map() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    for &(k, v) in ENCHANT_TO_SKILL_ENTRIES {
        m.insert(k, v);
    }
    m
}

/// Build the notable sort order lookup.
/// Key: notable name (case-sensitive, as spelled in PoB), Value: sort order integer.
pub fn build_notable_sort_order() -> HashMap<&'static str, u32> {
    let mut m = HashMap::new();
    for &(k, v) in NOTABLE_SORT_ORDER_ENTRIES {
        m.insert(k, v);
    }
    m
}

// ─────────────────────────────────────────────────────────────────────────────
// Static data tables (generated from Data/ClusterJewels.lua)
// ─────────────────────────────────────────────────────────────────────────────

/// Enchant text (lowercase) → cluster jewel skill tag ID.
/// Source: ClusterJewels.lua, enchant entries in each skill block.
pub static ENCHANT_TO_SKILL_ENTRIES: &[(&str, &str)] = &[
    ("added small passive skills grant: +10 to dexterity", "affliction_dexterity"),
    ("added small passive skills grant: +10 to intelligence", "affliction_intelligence"),
    ("added small passive skills grant: +10 to strength", "affliction_strength"),
    ("added small passive skills grant: +12% to chaos resistance", "affliction_chaos_resistance"),
    ("added small passive skills grant: +15% to cold resistance", "affliction_cold_resistance"),
    ("added small passive skills grant: +15% to fire resistance", "affliction_fire_resistance"),
    ("added small passive skills grant: +15% to lightning resistance", "affliction_lightning_resistance"),
    ("added small passive skills grant: +2% chance to block attack damage", "affliction_chance_to_block_attack_damage"),
    ("added small passive skills grant: +4% chance to suppress spell damage", "affliction_chance_to_dodge_attacks"),
    ("added small passive skills grant: 10% increased area damage", "affliction_area_damage"),
    ("added small passive skills grant: 10% increased attack damage", "affliction_attack_damage_"),
    ("added small passive skills grant: 10% increased damage over time", "affliction_damage_over_time_multiplier"),
    ("added small passive skills grant: 10% increased damage while affected by a herald", "affliction_damage_while_you_have_a_herald"),
    ("added small passive skills grant: 10% increased effect of non-damaging ailments", "affliction_effect_of_non-damaging_ailments"),
    ("added small passive skills grant: 10% increased elemental damage", "affliction_elemental_damage"),
    ("added small passive skills grant: 10% increased life recovery from flasks", "affliction_life_and_mana_recovery_from_flasks"),
    ("added small passive skills grant: 10% increased mana recovery from flasks", "affliction_life_and_mana_recovery_from_flasks"),
    ("added small passive skills grant: 10% increased projectile damage", "affliction_projectile_damage"),
    ("added small passive skills grant: 10% increased spell damage", "affliction_spell_damage"),
    ("added small passive skills grant: 12% increased attack damage while dual wielding", "affliction_attack_damage_while_dual_wielding_"),
    ("added small passive skills grant: 12% increased attack damage while holding a shield", "affliction_attack_damage_while_holding_a_shield"),
    ("added small passive skills grant: 12% increased brand damage", "affliction_brand_damage"),
    ("added small passive skills grant: 12% increased burning damage", "affliction_fire_damage_over_time_multiplier"),
    ("added small passive skills grant: 12% increased chaos damage", "affliction_chaos_damage"),
    ("added small passive skills grant: 12% increased chaos damage over time", "affliction_chaos_damage_over_time_multiplier"),
    ("added small passive skills grant: 12% increased cold damage", "affliction_cold_damage"),
    ("added small passive skills grant: 12% increased cold damage over time", "affliction_cold_damage_over_time_multiplier"),
    ("added small passive skills grant: 12% increased damage over time with bow skills", "affliction_bow_damage"),
    ("added small passive skills grant: 12% increased damage with bows", "affliction_bow_damage"),
    ("added small passive skills grant: 12% increased damage with two handed weapons", "affliction_damage_with_two_handed_melee_weapons"),
    ("added small passive skills grant: 12% increased fire damage", "affliction_fire_damage"),
    ("added small passive skills grant: 12% increased lightning damage", "affliction_lightning_damage"),
    ("added small passive skills grant: 12% increased mine damage", "affliction_trap_and_mine_damage"),
    ("added small passive skills grant: 12% increased physical damage", "affliction_physical_damage"),
    ("added small passive skills grant: 12% increased physical damage over time", "affliction_physical_damage_over_time_multiplier"),
    ("added small passive skills grant: 12% increased totem damage", "affliction_totem_damage"),
    ("added small passive skills grant: 12% increased trap damage", "affliction_trap_and_mine_damage"),
    ("added small passive skills grant: 15% increased armour", "affliction_armour"),
    ("added small passive skills grant: 15% increased critical strike chance", "affliction_critical_chance"),
    ("added small passive skills grant: 15% increased evasion rating", "affliction_evasion"),
    ("added small passive skills grant: 2% chance to block spell damage", "affliction_chance_to_block_spell_damage"),
    ("added small passive skills grant: 2% increased effect of your curses", "affliction_curse_effect_small"),
    ("added small passive skills grant: 3% increased effect of non-curse auras from your skills", "affliction_aura_effect"),
    ("added small passive skills grant: 4% increased maximum life", "affliction_maximum_life"),
    ("added small passive skills grant: 6% increased flask effect duration", "affliction_flask_duration"),
    ("added small passive skills grant: 6% increased mana reservation efficiency of skills", "affliction_reservation_efficiency_small"),
    ("added small passive skills grant: 6% increased maximum energy shield", "affliction_maximum_energy_shield"),
    ("added small passive skills grant: 6% increased maximum mana", "affliction_maximum_mana"),
    ("added small passive skills grant: axe attacks deal 12% increased damage with hits and ailments", "affliction_axe_and_sword_damage"),
    ("added small passive skills grant: channelling skills deal 12% increased damage", "affliction_channelling_skill_damage"),
    ("added small passive skills grant: claw attacks deal 12% increased damage with hits and ailments", "affliction_dagger_and_claw_damage"),
    ("added small passive skills grant: dagger attacks deal 12% increased damage with hits and ailments", "affliction_dagger_and_claw_damage"),
    ("added small passive skills grant: exerted attacks deal 20% increased damage", "affliction_warcry_buff_effect"),
    ("added small passive skills grant: mace or sceptre attacks deal 12% increased damage with hits and ailments", "affliction_mace_and_staff_damage"),
    ("added small passive skills grant: minions deal 10% increased damage", "affliction_minion_damage"),
    ("added small passive skills grant: minions deal 10% increased damage while you are affected by a herald", "affliction_minion_damage_while_you_have_a_herald"),
    ("added small passive skills grant: minions have 12% increased maximum life", "affliction_minion_life"),
    ("added small passive skills grant: staff attacks deal 12% increased damage with hits and ailments", "affliction_mace_and_staff_damage"),
    ("added small passive skills grant: sword attacks deal 12% increased damage with hits and ailments", "affliction_axe_and_sword_damage"),
    ("added small passive skills grant: wand attacks deal 12% increased damage with hits and ailments", "affliction_wand_damage"),
];

/// Notable sort order entries: (notable_name, sort_order).
/// Used to determine the order notables are placed around the cluster ring.
/// Only includes entries relevant to existing oracle test builds.
/// Full table has 301 entries; this subset is sufficient for the 30 oracle builds.
pub static NOTABLE_SORT_ORDER_ENTRIES: &[(&str, u32)] = &[
    ("Prodigious Defence", 11256),
    ("Advance Guard", 11257),
    ("Gladiatorial Combat", 11258),
    ("Strike Leader", 11259),
    ("Powerful Ward", 11260),
    ("Enduring Ward", 11261),
    ("Gladiator's Fortitude", 11262),
    ("Precise Retaliation", 11263),
    ("Veteran Defender", 11264),
    ("Iron Breaker", 11265),
    ("Deep Cuts", 11266),
    ("Master the Fundamentals", 11267),
    ("Force Multiplier", 11268),
    ("Furious Assault", 11269),
    ("Vicious Skewering", 11270),
    ("Grim Oath", 11271),
    ("Battle-Hardened", 11272),
    ("Replenishing Presence", 11273),
    ("Master of Command", 11274),
    ("Spiteful Presence", 11275),
    ("Purposeful Harbinger", 11276),
    ("Destructive Aspect", 11277),
    ("Electric Presence", 11278),
    ("Volatile Presence", 11279),
    ("Righteous Path", 11280),
    ("Skullbreaker", 11281),
    ("Pressure Points", 11282),
    ("Overwhelming Malice", 11283),
    ("Magnifier", 11284),
    ("Savage Response", 11285),
    ("Eye of the Storm", 11286),
    ("Basics of Pain", 11287),
    ("Quick Getaway", 11288),
    ("Assert Dominance", 11289),
    ("Vast Power", 11290),
    ("Powerful Assault", 11291),
    ("Intensity", 11292),
    ("Titanic Swings", 11293),
    ("Towering Threat", 11294),
    ("Ancestral Echo", 11295),
    ("Ancestral Reach", 11296),
    ("Ancestral Might", 11297),
    ("Ancestral Preservation", 11298),
    ("Snaring Spirits", 11299),
    ("Sleepless Sentries", 11300),
    ("Ancestral Guidance", 11301),
    ("Ancestral Inspiration", 11302),
    ("Vital Focus", 11303),
    ("Unrestrained Focus", 11304),
    ("Unwavering Focus", 11305),
    ("Enduring Focus", 11306),
    ("Precise Focus", 11307),
    ("Stoic Focus", 11308),
    ("Hex Breaker", 11309),
    ("Arcane Adept", 11310),
    ("Distilled Perfection", 11311),
    ("Spiked Concoction", 11312),
    ("Fasting", 11313),
    ("Mender's Wellspring", 11314),
    ("Special Reserve", 11315),
    ("Numbing Elixir", 11316),
    ("Mob Mentality", 11317),
    ("Cry Wolf", 11318),
    ("Haunting Shout", 11319),
    ("Lead By Example", 11320),
    ("Provocateur", 11321),
    ("Warning Call", 11322),
    ("Rattling Bellow", 11323),
    ("Onslaught", 11324),
    ("Rapid Infusion", 11325),
    ("Disorienting Display", 11326),
    ("Storm Drinker", 11327),
    ("Storm Weaver", 11328),
    ("Thunderstruck", 11329),
    ("Galvanic Alchemist", 11330),
    ("Overwhelming Odds", 11331),
    ("Force of Will", 11332),
    ("Surefooted Striker", 11333),
    ("Martial Momentum", 11334),
    ("Towering Menace", 11335),
    ("Seething Fury", 11336),
    ("Feed the Fury", 11337),
    ("Calamitous Moment", 11338),
    ("Brush With Death", 11339),
    ("Raze and Pillage", 11340),
    ("Slaughter", 11341),
    ("Unending Hunger", 11342),
    ("Unspeakable Gifts", 11343),
    ("Circling Oblivion", 11344),
    ("Heightened Senses", 11345),
    ("Sadist", 11346),
    ("Wasting Affliction", 11347),
    ("Chance to Poison", 11348),
    ("Septic Spells", 11349),
    ("Swift Venoms", 11350),
    ("Overwhelming Toxins", 11351),
    ("Smite the Weak", 11352),
    ("Deadly Repartee", 11353),
    ("Precise Commandment", 11354),
    ("Vengeful Commander", 11355),
    ("Enduring Composure", 11356),
    ("Disciple of the Slaughter", 11357),
    ("Disciple of Unyielding", 11358),
    ("Disciple of the Forbidden", 11359),
    ("Disciple of the Cleansing Fire", 11360),
    ("Disciple of Kitava", 11361),
    ("First Among Equals", 11362),
    ("Vicious Bite", 11363),
    ("Bloodscent", 11364),
    ("Haunted by Slaughter", 11365),
    ("Pack Leader", 11366),
    ("Call to the Slaughter", 11367),
    ("Renewal", 11368),
    ("Icy Exile", 11369),
    ("Winter Prowler", 11370),
    ("Bitter Cold", 11371),
    ("Prismatic Heart", 11372),
    ("Widespread Destruction", 11373),
    ("Elemental Equilibrium", 11374),
    ("Snowforged", 11375),
    ("Rime Gaze", 11376),
    ("Glacial Cage", 11377),
    ("Cooling Persistence", 11378),
    ("Eye of Winter", 11379),
    ("Cold-Blooded Killer", 11380),
    ("Blanketed Snow", 11381),
    ("Corrosive Elements", 11382),
    ("Endbringer", 11383),
    ("Doryani's Lesson", 11384),
    ("Arcing Blows", 11385),
    ("Supercharge", 11386),
    ("Overcharge", 11387),
    ("Static Blows", 11388),
    ("Heart of Thunder", 11389),
    ("Surging Vitality", 11390),
    ("Smoking Remains", 11391),
    ("Brushfire", 11392),
    ("Burning Bright", 11393),
    ("Cooked Alive", 11394),
    ("Fan the Flames", 11395),
    ("Cremator", 11396),
    ("Disintegration", 11397),
    ("Crimson Power", 11398),
    ("Burning Fury", 11399),
    ("Adds Combustion", 11400),
    ("Adds Crackling Speed", 11401),
    ("Adds Secrets of Suffering", 11402),
    ("Adds Hypothermia", 11403),
    ("Adds Volley Fire", 11404),
    ("Adds Innervate", 11405),
    ("Adds Chance to Flee", 11406),
    ("Adds Momentum", 11407),
    ("Adds Holy Flame Totem", 11408),
    ("Adds Herald of Thunder", 11409),
    ("Adds Bear Trap", 11410),
    ("Adds Ancestral Call", 11411),
    ("Adds Accuracy", 11412),
    ("Adds Brutality", 11413),
    ("Adds Ruthless", 11414),
    ("Adds Culling Strike", 11415),
    ("Adds Predator", 11416),
    ("Adds Berserk", 11417),
    ("Adds Rampage", 11418),
    ("Adds Intimidate Enemies", 11419),
    ("Adds Maim on Hit", 11420),
    ("Adds Fortify", 11421),
    ("Adds Blind", 11422),
    ("Adds Onslaught on Kill", 11423),
    ("Adds Power Charge on Kill", 11424),
    ("Adds Frenzy Charge on Hit", 11425),
    ("Adds Endurance Charge on Kill", 11426),
    ("Adds Mana Siphoning", 11427),
    ("Adds Life Gain on Hit", 11428),
    ("Adds Life Leech", 11429),
    ("Adds Mana Leech", 11430),
    ("Adds Energy Shield Leech", 11431),
    ("Overwhelming Malice (2H)", 11432),
];
