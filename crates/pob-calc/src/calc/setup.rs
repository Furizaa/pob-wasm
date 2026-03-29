use super::env::CalcEnv;
use crate::{build::Build, data::GameData, error::CalcError, mod_db::ModDb};
use std::sync::Arc;

pub fn init_env(build: &Build, data: Arc<GameData>) -> Result<CalcEnv, CalcError> {
    let player_mod_db = ModDb::new();
    let enemy_mod_db = ModDb::new();
    Ok(CalcEnv::new(player_mod_db, enemy_mod_db, data))
}
