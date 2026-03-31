//! Final DPS aggregation.
//! Mirrors the final calculation steps from Calcs.lua in Path of Building.

use super::env::{get_output_f64, CalcEnv};

/// Compute FullDPS and related aggregate DPS values.
/// Called after all other calculation passes (offence, triggers, mirages).
pub fn calc_full_dps(env: &mut CalcEnv) {
    let combined_dps = get_output_f64(&env.player.output, "CombinedDPS");
    let mirage_dps = get_output_f64(&env.player.output, "MirageDPS");

    // FullDPS = CombinedDPS + MirageDPS (only if MirageDPS isn't already in CombinedDPS)
    // By convention, mirage modules add to CombinedDPS directly, so we use CombinedDPS
    // as the primary aggregate. MirageDPS is tracked separately for display.
    // If MirageDPS > 0 and CombinedDPS already includes it, use CombinedDPS directly.
    // If MirageDPS > 0 but CombinedDPS doesn't include it, add them.
    //
    // Current convention: mirages::run adds mirage DPS to CombinedDPS.
    // So FullDPS = CombinedDPS (which already includes mirage).
    // But we also want FullDPS to be max(CombinedDPS, TotalDPS + MirageDPS) for
    // cases where CombinedDPS was not set.
    let total_dps = get_output_f64(&env.player.output, "TotalDPS");

    let full_dps = if combined_dps > 0.0 {
        combined_dps
    } else {
        total_dps + mirage_dps
    };
    env.player.set_output("FullDPS", full_dps);

    // Pass-through for DOT-based DPS
    let total_dot_dps = get_output_f64(&env.player.output, "TotalDotDPS");
    env.player.set_output("FullDotDPS", total_dot_dps);

    // Pass-through for ailment DPS components
    let with_poison = get_output_f64(&env.player.output, "WithPoisonDPS");
    if with_poison > 0.0 {
        env.player.set_output("FullWithPoisonDPS", with_poison);
    }
    let with_ignite = get_output_f64(&env.player.output, "WithIgniteDPS");
    if with_ignite > 0.0 {
        env.player.set_output("FullWithIgniteDPS", with_ignite);
    }
    let with_bleed = get_output_f64(&env.player.output, "WithBleedDPS");
    if with_bleed > 0.0 {
        env.player.set_output("FullWithBleedDPS", with_bleed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{calc::env::CalcEnv, data::GameData, mod_db::ModDb};
    use std::sync::Arc;

    fn make_env() -> CalcEnv {
        let game_data = GameData::from_json(crate::tests::stub_game_data_json()).unwrap();
        CalcEnv::new(ModDb::new(), ModDb::new(), Arc::new(game_data))
    }

    #[test]
    fn full_dps_equals_combined_when_set() {
        let mut env = make_env();
        env.player.set_output("CombinedDPS", 5000.0);
        env.player.set_output("TotalDPS", 3000.0);
        env.player.set_output("MirageDPS", 1000.0);
        calc_full_dps(&mut env);
        assert_eq!(get_output_f64(&env.player.output, "FullDPS"), 5000.0);
    }

    #[test]
    fn full_dps_falls_back_to_total_plus_mirage() {
        let mut env = make_env();
        // CombinedDPS not set (0)
        env.player.set_output("TotalDPS", 3000.0);
        env.player.set_output("MirageDPS", 1000.0);
        calc_full_dps(&mut env);
        assert_eq!(get_output_f64(&env.player.output, "FullDPS"), 4000.0);
    }

    #[test]
    fn full_dps_includes_mirage_when_combined_not_set() {
        let mut env = make_env();
        env.player.set_output("TotalDPS", 2000.0);
        env.player.set_output("MirageDPS", 500.0);
        calc_full_dps(&mut env);
        let full = get_output_f64(&env.player.output, "FullDPS");
        assert_eq!(full, 2500.0);
    }

    #[test]
    fn full_dot_dps_pass_through() {
        let mut env = make_env();
        env.player.set_output("TotalDotDPS", 1234.0);
        calc_full_dps(&mut env);
        assert_eq!(get_output_f64(&env.player.output, "FullDotDPS"), 1234.0);
    }

    #[test]
    fn ailment_dps_pass_through() {
        let mut env = make_env();
        env.player.set_output("WithPoisonDPS", 100.0);
        env.player.set_output("WithIgniteDPS", 200.0);
        env.player.set_output("WithBleedDPS", 300.0);
        calc_full_dps(&mut env);
        assert_eq!(
            get_output_f64(&env.player.output, "FullWithPoisonDPS"),
            100.0
        );
        assert_eq!(
            get_output_f64(&env.player.output, "FullWithIgniteDPS"),
            200.0
        );
        assert_eq!(
            get_output_f64(&env.player.output, "FullWithBleedDPS"),
            300.0
        );
    }
}
