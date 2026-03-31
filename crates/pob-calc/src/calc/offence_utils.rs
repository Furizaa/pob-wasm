//! Offence utility types and helpers for the damage calculation pipeline.
//! Mirrors CalcOffence.lua's conversion table logic, cooldown/duration helpers, etc.

use crate::calc::env::OutputTable;
use crate::mod_db::types::{ModType, SkillCfg};
use crate::mod_db::ModDb;

// Re-export damage type indices from defence for consistency.
pub use crate::calc::defence::{DMG_CHAOS, DMG_COLD, DMG_FIRE, DMG_LIGHTNING, DMG_PHYSICAL};

/// Number of damage types.
pub const NUM_DMG_TYPES: usize = 5;

/// Canonical damage type names indexed by DMG_* constants.
pub const DMG_TYPE_NAMES: [&str; NUM_DMG_TYPES] =
    ["Physical", "Lightning", "Cold", "Fire", "Chaos"];

/// PoE conversion chain order: Physical → Lightning → Cold → Fire → Chaos.
/// Each element is the index of the damage type in the chain.
pub const CONVERSION_ORDER: [usize; NUM_DMG_TYPES] = [
    DMG_PHYSICAL,  // 0
    DMG_LIGHTNING, // 1
    DMG_COLD,      // 2
    DMG_FIRE,      // 3
    DMG_CHAOS,     // 4
];

/// PoE server tick interval in seconds (33.33ms).
const SERVER_TICK: f64 = 1.0 / 30.0;

/// Damage conversion and gain-as-extra table.
///
/// `base[src][dst]` = fraction of `src` damage converted to `dst` (rows sum to 1.0).
/// `extra[src][dst]` = fraction of `src` gained as extra `dst` damage (additive, no cap).
#[derive(Debug, Clone)]
pub struct ConversionTable {
    /// Conversion fractions. `base[src][dst]` = fraction converted (0.0 – 1.0).
    pub base: [[f64; NUM_DMG_TYPES]; NUM_DMG_TYPES],
    /// Gain-as-extra fractions. `extra[src][dst]` = fraction gained as extra.
    pub extra: [[f64; NUM_DMG_TYPES]; NUM_DMG_TYPES],
}

impl Default for ConversionTable {
    /// Identity matrix: each type stays as itself, no extra.
    fn default() -> Self {
        let mut base = [[0.0; NUM_DMG_TYPES]; NUM_DMG_TYPES];
        for i in 0..NUM_DMG_TYPES {
            base[i][i] = 1.0;
        }
        Self {
            base,
            extra: [[0.0; NUM_DMG_TYPES]; NUM_DMG_TYPES],
        }
    }
}

/// Build a conversion table by querying the mod database for conversion and gain-as-extra mods.
///
/// Queries `{Src}DamageConvertTo{Dst}` (Base mods) for conversion fractions.
/// Caps total conversion per source at 100% and distributes any excess proportionally.
/// Sets the remainder as self-conversion.
///
/// Queries `{Src}DamageGainAs{Dst}` (Base mods) for gain-as-extra fractions.
pub fn build_conversion_table(
    mod_db: &ModDb,
    output: &OutputTable,
    cfg: Option<&SkillCfg>,
) -> ConversionTable {
    let mut table = ConversionTable {
        base: [[0.0; NUM_DMG_TYPES]; NUM_DMG_TYPES],
        extra: [[0.0; NUM_DMG_TYPES]; NUM_DMG_TYPES],
    };

    for &src in &CONVERSION_ORDER {
        let src_name = DMG_TYPE_NAMES[src];

        let mut total_conv = 0.0_f64;
        let mut conv_amounts = [0.0_f64; NUM_DMG_TYPES];

        for &dst in &CONVERSION_ORDER {
            if src == dst {
                continue;
            }
            let dst_name = DMG_TYPE_NAMES[dst];

            // Conversion: "{Src}DamageConvertTo{Dst}"
            let conv_stat = format!("{src_name}DamageConvertTo{dst_name}");
            let conv_pct = mod_db.sum_cfg(ModType::Base, &conv_stat, cfg, output);
            if conv_pct > 0.0 {
                conv_amounts[dst] = conv_pct;
                total_conv += conv_pct;
            }

            // Gain-as-extra: "{Src}DamageGainAs{Dst}"
            let gain_stat = format!("{src_name}DamageGainAs{dst_name}");
            let gain_pct = mod_db.sum_cfg(ModType::Base, &gain_stat, cfg, output);
            if gain_pct != 0.0 {
                table.extra[src][dst] = gain_pct / 100.0;
            }
        }

        // Cap total conversion at 100%
        if total_conv > 100.0 {
            let scale = 100.0 / total_conv;
            for dst in 0..NUM_DMG_TYPES {
                conv_amounts[dst] *= scale;
            }
            total_conv = 100.0;
        }

        // Set conversion fractions (as 0.0–1.0)
        for dst in 0..NUM_DMG_TYPES {
            if src == dst {
                continue;
            }
            table.base[src][dst] = conv_amounts[dst] / 100.0;
        }

        // Remainder stays as the source type
        table.base[src][src] = (100.0 - total_conv) / 100.0;
    }

    table
}

/// Calculate effective skill cooldown, rounding up to the nearest server tick (33ms).
///
/// `base_cd`: base cooldown in seconds.
/// `cd_recovery_inc`: increased cooldown recovery rate (sum of Inc mods, e.g. 50 means 50%).
/// `cd_recovery_more`: multiplied cooldown recovery rate (product of More mods).
///
/// Returns effective cooldown in seconds.
pub fn calc_skill_cooldown(base_cd: f64, cd_recovery_inc: f64, cd_recovery_more: f64) -> f64 {
    if base_cd <= 0.0 {
        return 0.0;
    }
    let recovery_rate = (1.0 + cd_recovery_inc / 100.0) * cd_recovery_more;
    let raw_cd = base_cd / recovery_rate;
    // Round up to nearest server tick
    (raw_cd / SERVER_TICK).ceil() * SERVER_TICK
}

/// Calculate effective skill duration.
///
/// `base_dur`: base duration in seconds.
/// `dur_inc`: increased duration (sum of Inc mods, e.g. 50 means 50%).
/// `dur_more`: multiplied duration (product of More mods).
pub fn calc_skill_duration(base_dur: f64, dur_inc: f64, dur_more: f64) -> f64 {
    if base_dur <= 0.0 {
        return 0.0;
    }
    base_dur * (1.0 + dur_inc / 100.0) * dur_more
}

/// Average a stat between main-hand and off-hand (e.g. attack speed).
pub fn combine_stat_avg(mh: f64, oh: f64) -> f64 {
    (mh + oh) / 2.0
}

/// Add stats from main-hand and off-hand (e.g. flat added damage).
pub fn combine_stat_add(mh: f64, oh: f64) -> f64 {
    mh + oh
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mod_db::types::{Mod, ModSource};
    use crate::mod_db::ModDb;
    use std::collections::HashMap;

    fn src() -> ModSource {
        ModSource::new("Test", "test")
    }

    fn empty_output() -> OutputTable {
        HashMap::new()
    }

    #[test]
    fn conversion_table_default_is_identity() {
        let table = ConversionTable::default();
        for src in 0..NUM_DMG_TYPES {
            for dst in 0..NUM_DMG_TYPES {
                if src == dst {
                    assert_eq!(
                        table.base[src][dst], 1.0,
                        "base[{src}][{dst}] should be 1.0"
                    );
                } else {
                    assert_eq!(
                        table.base[src][dst], 0.0,
                        "base[{src}][{dst}] should be 0.0"
                    );
                }
                assert_eq!(
                    table.extra[src][dst], 0.0,
                    "extra[{src}][{dst}] should be 0.0"
                );
            }
        }
    }

    #[test]
    fn conversion_table_phys_to_lightning_50() {
        let mut db = ModDb::new();
        // 50% phys converted to lightning
        db.add(Mod::new_base(
            "PhysicalDamageConvertToLightning",
            50.0,
            src(),
        ));

        let table = build_conversion_table(&db, &empty_output(), None);

        // Physical → Lightning: 50%
        assert!(
            (table.base[DMG_PHYSICAL][DMG_LIGHTNING] - 0.5).abs() < 1e-9,
            "phys→lightning should be 0.5, got {}",
            table.base[DMG_PHYSICAL][DMG_LIGHTNING]
        );
        // Physical → Physical: 50% remaining
        assert!(
            (table.base[DMG_PHYSICAL][DMG_PHYSICAL] - 0.5).abs() < 1e-9,
            "phys→phys should be 0.5, got {}",
            table.base[DMG_PHYSICAL][DMG_PHYSICAL]
        );
        // Other types unchanged
        assert!(
            (table.base[DMG_LIGHTNING][DMG_LIGHTNING] - 1.0).abs() < 1e-9,
            "lightning→lightning should be 1.0"
        );
    }

    #[test]
    fn conversion_table_capped_at_100() {
        let mut db = ModDb::new();
        // 60% phys→lightning + 60% phys→cold = 120% total, should be capped at 100%
        db.add(Mod::new_base(
            "PhysicalDamageConvertToLightning",
            60.0,
            src(),
        ));
        db.add(Mod::new_base("PhysicalDamageConvertToCold", 60.0, src()));

        let table = build_conversion_table(&db, &empty_output(), None);

        let to_lightning = table.base[DMG_PHYSICAL][DMG_LIGHTNING];
        let to_cold = table.base[DMG_PHYSICAL][DMG_COLD];
        let to_self = table.base[DMG_PHYSICAL][DMG_PHYSICAL];

        // Total conversion should be 100%, self should be 0%
        assert!(
            (to_lightning + to_cold - 1.0).abs() < 1e-9,
            "total conversion should be 1.0, got {}",
            to_lightning + to_cold
        );
        assert!(to_self.abs() < 1e-9, "self should be 0.0, got {}", to_self);
        // Proportional: 60:60 = 50:50
        assert!(
            (to_lightning - 0.5).abs() < 1e-9,
            "phys→lightning should be 0.5, got {}",
            to_lightning
        );
    }

    #[test]
    fn conversion_table_gain_as_extra() {
        let mut db = ModDb::new();
        db.add(Mod::new_base("PhysicalDamageGainAsFire", 20.0, src()));

        let table = build_conversion_table(&db, &empty_output(), None);

        assert!(
            (table.extra[DMG_PHYSICAL][DMG_FIRE] - 0.2).abs() < 1e-9,
            "phys gain as fire should be 0.2, got {}",
            table.extra[DMG_PHYSICAL][DMG_FIRE]
        );
        // No conversion, phys stays phys
        assert!(
            (table.base[DMG_PHYSICAL][DMG_PHYSICAL] - 1.0).abs() < 1e-9,
            "phys→phys should remain 1.0"
        );
    }

    #[test]
    fn skill_cooldown_rounds_to_server_tick() {
        // Server tick is 1/30 ≈ 0.03333s
        let tick = 1.0 / 30.0;

        // Base CD = 0.05s, no modifiers → ceil(0.05 / tick) * tick = ceil(1.5) * tick = 2 * tick
        let cd = calc_skill_cooldown(0.05, 0.0, 1.0);
        let expected = 2.0 * tick;
        assert!(
            (cd - expected).abs() < 1e-9,
            "expected {expected:.6}, got {cd:.6}"
        );

        // Base CD = 4s, 50% inc recovery → 4 / 1.5 = 2.6667s → ceil(80) * tick = 80 * tick
        let cd2 = calc_skill_cooldown(4.0, 50.0, 1.0);
        let raw = 4.0 / 1.5;
        let expected2 = (raw / tick).ceil() * tick;
        assert!(
            (cd2 - expected2).abs() < 1e-9,
            "expected {expected2:.6}, got {cd2:.6}"
        );

        // Zero base CD
        assert_eq!(calc_skill_cooldown(0.0, 0.0, 1.0), 0.0);
    }

    #[test]
    fn skill_duration_computed() {
        let dur = calc_skill_duration(2.0, 50.0, 1.2);
        // 2.0 * (1 + 50/100) * 1.2 = 2.0 * 1.5 * 1.2 = 3.6
        assert!((dur - 3.6).abs() < 1e-9, "expected 3.6, got {dur}");
        assert_eq!(calc_skill_duration(0.0, 50.0, 1.0), 0.0);
    }

    #[test]
    fn combine_stat_helpers() {
        assert_eq!(combine_stat_avg(10.0, 20.0), 15.0);
        assert_eq!(combine_stat_add(10.0, 20.0), 30.0);
    }
}
