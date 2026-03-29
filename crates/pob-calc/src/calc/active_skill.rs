// Stub: mark the main skill slot and skill flags in the output
//
// Reference: third-party/PathOfBuilding/src/Modules/CalcActiveSkill.lua
// Full implementation resolves the main active skill from build.skill_sets,
// builds skillCfg (flags, keyword flags), and sets conditions like UsingAttack,
// UsingSpell, IsMainSkill, etc.
//
// For now: set IsMainSkill condition so offence.rs has something to work with.

use super::env::CalcEnv;

pub fn run(env: &mut CalcEnv) {
    // Stub: set IsMainSkill condition so offence.rs has something to work with
    env.player.mod_db.set_condition("IsMainSkill", true);
}
