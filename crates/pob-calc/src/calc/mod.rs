pub mod active_skill;
pub mod defence;
pub mod env;
pub mod mirages;
pub mod offence;
pub mod perform;
pub mod setup;
pub mod triggers;

use crate::{build::Build, data::GameData, error::CalcError};
use env::{BreakdownTable, OutputTable};
use serde::Serialize;
use std::sync::Arc;

/// The result of a full calculation pass.
/// Returned by calculate() and calculateSkill() in the WASM API.
#[derive(Debug, Serialize)]
pub struct CalcResult {
    /// Computed stat values. Keys match POB's env.player.output names.
    pub output: OutputTable,
    /// Per-stat formula breakdowns.
    pub breakdown: BreakdownTable,
}

/// Run all calculation passes for a build and return the result.
pub fn calculate(build: &Build, data: Arc<GameData>) -> Result<CalcResult, CalcError> {
    let mut env = setup::init_env(build, data)?;

    perform::run(&mut env);
    defence::run(&mut env);
    active_skill::run(&mut env, build);
    offence::run(&mut env);
    triggers::run(&mut env, build);
    mirages::run(&mut env, build);

    Ok(CalcResult {
        output: env.player.output,
        breakdown: env.player.breakdown,
    })
}
