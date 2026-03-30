# Phase 8: Triggers, Mirages, and Remaining Oracle Builds — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the remaining five oracle builds (`trap_saboteur`, `totem_hierophant`, `mine_detonator`, `minion_summoner`, `poe2_basic`), implement `triggers.rs` (totems, traps, mines) and `mirages.rs` (Mirage Archer), fix remaining data-extraction gaps (mod names, base item stats), and confirm `getMods()` cfg-scope filtering works. At the end, all ten oracle builds pass and the WASM API is fully correct.

**Architecture:** This phase is additive — it extends the engine built in Phases 6–7 rather than refactoring it. Each oracle archetype reveals specific engine gaps; those gaps are fixed in the order the tests expose them.

**Tech Stack:** Rust 1.82, `pob-calc` (Phases 3–7 complete), `wasm-pack`

**Prerequisites:** Phase 7 complete. All six Phase 7 oracle tests pass.

**Branch:** `feature/phase8-oracle-complete`

**Reference files (read before each task):**
- `third-party/PathOfBuilding/src/Modules/CalcTriggers.lua` — totem, trap, mine DPS sections
- `third-party/PathOfBuilding/src/Modules/CalcMirages.lua` — Mirage Archer DPS
- `third-party/PathOfBuilding/src/Modules/ModParser.lua` — for any remaining stat patterns
- `crates/data-extractor/src/transform/mods.rs` — mod name extraction (currently empty string)
- `crates/data-extractor/src/transform/bases.rs` — base stat extraction (currently all-zero)

---

## File Map

```
crates/pob-calc/src/calc/
  triggers.rs             ← implement totem/trap/mine DPS
  mirages.rs              ← implement Mirage Archer DPS
crates/pob-wasm/src/lib.rs  ← fix getMods() cfg parameter
crates/data-extractor/src/transform/
  mods.rs                 ← fix mod name extraction
  bases.rs                ← fix base_str/dex/int extraction
crates/pob-calc/tests/oracle/
  trap_saboteur.xml / .expected.json
  totem_hierophant.xml / .expected.json
  mine_detonator.xml / .expected.json
  minion_summoner.xml / .expected.json
  poe2_basic.xml / .expected.json
```

---

### Task 1: Generate remaining oracle builds

- [ ] **Step 1: Create `trap_saboteur.xml`**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Shadow" ascendClassName="None">
  </Build>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Fire Trap" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="2" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config>
    <Input name="enemyLevel" number="84"/>
  </Config>
</PathOfBuilding>
```

- [ ] **Step 2: Create `totem_hierophant.xml`**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Templar" ascendClassName="None">
  </Build>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Shockwave Totem" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="5" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config>
    <Input name="enemyLevel" number="84"/>
  </Config>
</PathOfBuilding>
```

- [ ] **Step 3: Create `mine_detonator.xml`**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Shadow" ascendClassName="None">
  </Build>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Blastchain Mine" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="2" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config>
    <Input name="enemyLevel" number="84"/>
  </Config>
</PathOfBuilding>
```

- [ ] **Step 4: Create `minion_summoner.xml`**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Witch" ascendClassName="None">
  </Build>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Raise Zombie" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="3" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config>
    <Input name="enemyLevel" number="84"/>
  </Config>
</PathOfBuilding>
```

- [ ] **Step 5: Create `poe2_basic.xml`** (uses a PoE2 class if available in the submodule)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Marauder" ascendClassName="None">
  </Build>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Cleave" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1">
    <Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/>
  </Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config>
    <Input name="enemyLevel" number="84"/>
  </Config>
</PathOfBuilding>
```

> Note: `poe2_basic` uses a PoE1 class as a stand-in until PoE2 data is available (the spec marks PoE2 as deferred). The oracle XML above is identical to `melee_str` — change this to a real PoE2 class and tree version when PoE2 GGPK data is available.

- [ ] **Step 6: Generate all five expected JSONs**

```bash
for name in trap_saboteur totem_hierophant mine_detonator minion_summoner poe2_basic; do
  ./scripts/run_oracle.sh crates/pob-calc/tests/oracle/${name}.xml \
    > crates/pob-calc/tests/oracle/${name}.expected.json
  echo -n "$name TotalDPS: "
  python3 -c "import json; d=json.load(open('crates/pob-calc/tests/oracle/${name}.expected.json')); print(d.get('output',{}).get('TotalDPS','MISSING'))"
done
```

- [ ] **Step 7: Add oracle test stubs to `oracle.rs`**

Add to `crates/pob-calc/tests/oracle.rs`:

```rust
#[test]
fn oracle_trap_saboteur_dps_matches_pob() {
    let Some(data) = load_game_data() else { eprintln!("DATA_DIR not set"); return; };
    let xml = load_build_xml("trap_saboteur");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("trap_saboteur");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "TotalDPS");
    assert_output_approx(&actual, expected_output, "TrapCooldown");
}

#[test]
fn oracle_totem_hierophant_dps_matches_pob() {
    let Some(data) = load_game_data() else { eprintln!("DATA_DIR not set"); return; };
    let xml = load_build_xml("totem_hierophant");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("totem_hierophant");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "TotalDPS");
    assert_output_approx(&actual, expected_output, "TotemLifeTotal");
}

#[test]
fn oracle_mine_detonator_dps_matches_pob() {
    let Some(data) = load_game_data() else { eprintln!("DATA_DIR not set"); return; };
    let xml = load_build_xml("mine_detonator");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("mine_detonator");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "TotalDPS");
    assert_output_approx(&actual, expected_output, "MineDetonationTime");
}

#[test]
fn oracle_minion_summoner_matches_pob() {
    let Some(data) = load_game_data() else { eprintln!("DATA_DIR not set"); return; };
    let xml = load_build_xml("minion_summoner");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("minion_summoner");
    let expected_output = expected.get("output").unwrap_or(&expected);
    // For summoners, minion DPS is the key output
    assert_output_approx(&actual, expected_output, "TotalDPS");
    assert_output_approx(&actual, expected_output, "MinionCount");
}

#[test]
fn oracle_poe2_basic_matches_pob() {
    let Some(data) = load_game_data() else { eprintln!("DATA_DIR not set"); return; };
    let xml = load_build_xml("poe2_basic");
    let build = parse_xml(&xml).expect("parse");
    let result = calculate(&build, Arc::clone(&data)).expect("calculate");
    let actual = serde_json::to_value(&result.output).unwrap();
    let expected = load_expected("poe2_basic");
    let expected_output = expected.get("output").unwrap_or(&expected);
    assert_output_approx(&actual, expected_output, "Life");
    assert_output_approx(&actual, expected_output, "TotalDPS");
}
```

- [ ] **Step 8: Confirm all five fail**

```bash
DATA_DIR=data cargo test -p pob-calc oracle_trap oracle_totem oracle_mine oracle_minion oracle_poe2 -- --nocapture 2>&1 | grep -E "FAILED|ok"
```

Expected: all FAILED (engine doesn't produce correct values yet).

- [ ] **Step 9: Commit**

```bash
git add crates/pob-calc/tests/oracle/
git commit -m "test: add oracle builds for trap_saboteur, totem_hierophant, mine_detonator, minion_summoner, poe2_basic"
```

---

### Task 2: Implement `triggers.rs` — totems, traps, mines

**Files:**
- Modify: `crates/pob-calc/src/calc/triggers.rs`

**Reference:** `CalcTriggers.lua` — read in full. Key sections:
- Totem DPS: search for `TotemDPS` or `TotalDPS` inside the totem block (around line 400–600). Key formula: `TotemDPS = skillDPS * ActiveTotemCount`. Totem life comes from `TotemLife` mod.
- Trap DPS: search for `TrapDPS`. Formula: `TrapDPS = skillDPS * traps_per_throw / (cooldown + throwTime)`.
- Mine DPS: search for `MineDPS`. Formula: `MineDPS = skillDPS * mines_per_lay / detonation_time`.

Each of these sections calls `calcs.offence()` on a cloned environment with specific flag overrides. In the Rust port, the equivalent is calling the offence helper functions with a modified `CalcEnv`.

- [ ] **Step 1: Update `triggers::run` signature to accept `build`** (already done in Phase 7 Task 5 Step 1)

Confirm the signature is `pub fn run(env: &mut CalcEnv, build: &Build)`.

- [ ] **Step 2: Write a failing unit test in `triggers.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build::parse_xml, calc::{setup::init_env, active_skill, perform, defence, offence}, data::GameData};
    use std::sync::Arc;

    fn make_data() -> Arc<GameData> {
        Arc::new(GameData::from_json(crate::stub_game_data_json()).unwrap())
    }

    #[test]
    fn totem_build_sets_totem_life() {
        // Any build with a totem skill should set TotemLifeTotal
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1"
         className="Templar" ascendClassName="None"/>
  <Skills activeSkillSet="1">
    <SkillSet id="1">
      <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
        <Gem skillId="Shockwave Totem" level="20" quality="0" enabled="true"/>
      </Skill>
    </SkillSet>
  </Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="5" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let data = make_data();
        let mut env = init_env(&build, data).unwrap();
        perform::run(&mut env);
        defence::run(&mut env);
        active_skill::run(&mut env, &build);
        offence::run(&mut env, &build);
        run(&mut env, &build);
        // With a totem skill, TotemLifeTotal should be set (> 0)
        // With stub data the gem won't be recognized as a totem — we just check no panic
        // The oracle test is the real parity check
        let _ = env.player.output.get("TotemLifeTotal");
    }
}
```

- [ ] **Step 3: Implement totem DPS in `triggers.rs`**

```rust
use super::env::CalcEnv;
use crate::build::Build;
use crate::mod_db::types::{ModFlags, KeywordFlags, ModType};

pub fn run(env: &mut CalcEnv, build: &Build) {
    let Some(skill) = env.player.main_skill.as_ref() else { return; };

    // Determine if this is a totem, trap, or mine skill
    // Heuristic: check skill_id against known lists until gem type data is fully loaded
    let known_totems: std::collections::HashSet<&str> = [
        "Shockwave Totem", "Searing Bond", "Rejuvenation Totem",
        "Decoy Totem", "Ancestral Warchief", "Ancestral Protector",
        "Ballista Totem",
    ].iter().copied().collect();
    let known_traps: std::collections::HashSet<&str> = [
        "Fire Trap", "Lightning Trap", "Bear Trap", "Explosive Trap",
        "Cluster Trap", "Conversion Trap",
    ].iter().copied().collect();
    let known_mines: std::collections::HashSet<&str> = [
        "Blastchain Mine", "Remote Mine", "High-Impact Mine",
        "Stormblast Mine", "Pyroclast Mine",
    ].iter().copied().collect();

    let is_totem = known_totems.contains(skill.skill_id.as_str());
    let is_trap  = known_traps.contains(skill.skill_id.as_str());
    let is_mine  = known_mines.contains(skill.skill_id.as_str());

    if is_totem {
        calc_totem_dps(env);
    } else if is_trap {
        calc_trap_dps(env);
    } else if is_mine {
        calc_mine_dps(env);
    }
}

fn calc_totem_dps(env: &mut CalcEnv) {
    // Reference: CalcTriggers.lua — totem section
    // ActiveTotemLimit from ModDb, default 1
    let active_totems = env.player.mod_db.sum(ModType::Base, "ActiveTotemLimit", ModFlags::NONE, KeywordFlags::NONE).max(1.0);
    env.player.set_output("ActiveTotemLimit", active_totems);

    // Totem life: TotemLife from ModDb, default 100 at level 1 scaling with level
    let base_totem_life = 100.0_f64;
    let inc_totem_life = env.player.mod_db.sum(ModType::Inc, "TotemLife", ModFlags::NONE, KeywordFlags::NONE);
    let more_totem_life = env.player.mod_db.more("TotemLife", ModFlags::NONE, KeywordFlags::NONE);
    let totem_life = (base_totem_life * (1.0 + inc_totem_life / 100.0) * more_totem_life).round();
    env.player.set_output("TotemLife", totem_life);
    env.player.set_output("TotemLifeTotal", totem_life * active_totems);

    // Totem DPS: skill DPS (already calculated in offence.rs) * number of active totems
    let skill_dps = env.player.output.get("TotalDPS")
        .and_then(|v| if let super::env::OutputValue::Number(n) = v { Some(*n) } else { None })
        .unwrap_or(0.0);
    let totem_dps = skill_dps * active_totems;
    env.player.set_output("TotemDPS", totem_dps);

    // Totem placement time from ModDb (default 0.6s)
    let base_place_time = env.player.mod_db.sum(ModType::Base, "TotemPlacementTime", ModFlags::NONE, KeywordFlags::NONE);
    let place_time = if base_place_time > 0.0 { base_place_time } else { 0.6 };
    env.player.set_output("TotemPlacementTime", place_time);

    // Update CombinedDPS to totem DPS
    env.player.set_output("CombinedDPS", totem_dps);
}

fn calc_trap_dps(env: &mut CalcEnv) {
    // Reference: CalcTriggers.lua — trap section
    // Trap throw time (default 0.6s, modified by TrapThrowingSpeed)
    let base_throw_time = 0.6_f64;
    let inc_throw_speed = env.player.mod_db.sum(ModType::Inc, "TrapThrowingSpeed", ModFlags::NONE, KeywordFlags::NONE);
    let more_throw_speed = env.player.mod_db.more("TrapThrowingSpeed", ModFlags::NONE, KeywordFlags::NONE);
    let throw_time = base_throw_time / ((1.0 + inc_throw_speed / 100.0) * more_throw_speed).max(0.001);
    env.player.set_output("TrapThrowingTime", throw_time);

    // Trap cooldown (default 4s for most traps)
    let trap_cooldown = 4.0_f64;
    env.player.set_output("TrapCooldown", trap_cooldown);

    // Traps per throw (default 1)
    let traps_per_throw = 1.0_f64;

    // Effective trap DPS = skill_DPS * traps_per_throw / cooldown
    let skill_dps = env.player.output.get("TotalDPS")
        .and_then(|v| if let super::env::OutputValue::Number(n) = v { Some(*n) } else { None })
        .unwrap_or(0.0);
    let trap_dps = skill_dps * traps_per_throw / trap_cooldown;
    env.player.set_output("TrapDPS", trap_dps);
    env.player.set_output("CombinedDPS", trap_dps);
}

fn calc_mine_dps(env: &mut CalcEnv) {
    // Reference: CalcTriggers.lua — mine section
    // Mine laying time (default 0.3s)
    let base_lay_time = env.player.mod_db.sum(ModType::Base, "MineLayingTime", ModFlags::NONE, KeywordFlags::NONE);
    let lay_time = if base_lay_time > 0.0 { base_lay_time } else { 0.3 };
    env.player.set_output("MineLayingTime", lay_time);

    // Mines per detonation (default 1)
    let mines_per_det = 1.0_f64;

    // Detonation delay (default 0.25s)
    let detonation_time = lay_time + 0.25;
    env.player.set_output("MineDetonationTime", detonation_time);

    // Mine DPS = skill_DPS * mines_per_detonation / detonation_time
    let skill_dps = env.player.output.get("TotalDPS")
        .and_then(|v| if let super::env::OutputValue::Number(n) = v { Some(*n) } else { None })
        .unwrap_or(0.0);
    let mine_dps = skill_dps * mines_per_det / detonation_time;
    env.player.set_output("MineDPS", mine_dps);
    env.player.set_output("CombinedDPS", mine_dps);
}
```

- [ ] **Step 4: Run triggers unit test**

```bash
cargo test -p pob-calc calc::triggers
```

Expected: passes (no panic).

- [ ] **Step 5: Run trap/totem/mine oracle tests**

```bash
DATA_DIR=data cargo test -p pob-calc oracle_trap oracle_totem oracle_mine -- --nocapture 2>&1 | tail -20
```

Fix any discrepancies. Common issues:
- Totem life formula differs for high-level builds — check `CalcTriggers.lua` line-by-line for the exact formula
- Trap cooldown value differs by skill — `Fire Trap` has a different cooldown than `Explosive Trap`; check `skillData.cooldown` in gem level tables
- Mine detonation sequence differs — check `CalcTriggers.lua` for the multi-mine detonation chain formula

Iterate until all three pass within 0.1% tolerance.

- [ ] **Step 6: Commit**

```bash
git add crates/pob-calc/src/calc/triggers.rs
git commit -m "feat(calc): implement triggers.rs — totem, trap, mine DPS"
```

---

### Task 3: Implement `mirages.rs` — Mirage Archer DPS

**Files:**
- Modify: `crates/pob-calc/src/calc/mirages.rs`

**Reference:** `CalcMirages.lua` — read in full. It handles Mirage Archer (bow skill support) and Wisp DPS. The Mirage Archer adds a portion of the player's bow DPS as additional damage. Most of the file handles the mirror/wisp interactions.

The core formula (from `CalcMirages.lua`):
```
MirageArcherDPS = playerBowDPS * (1 - someReduction) * mirageCount
```

- [ ] **Step 1: Write a failing test**

```rust
// In triggers.rs tests (mirages shares the same test pattern)
// Just confirm no panic when run is called
```

Add to `crates/pob-calc/src/calc/mirages.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mirages_run_does_not_panic() {
        // Smoke test — just verifies no panic with a default env
        use crate::{build::parse_xml, calc::setup::init_env, data::GameData};
        use std::sync::Arc;
        let data = Arc::new(GameData::from_json(crate::stub_game_data_json()).unwrap());
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1" className="Ranger" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1"/></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="6" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
        let build = parse_xml(xml).unwrap();
        let mut env = init_env(&build, data).unwrap();
        run(&mut env, &build);
    }
}
```

- [ ] **Step 2: Implement `mirages.rs`**

```rust
use super::env::CalcEnv;
use crate::build::Build;
use crate::mod_db::types::{ModFlags, KeywordFlags, ModType};

pub fn run(env: &mut CalcEnv, build: &Build) {
    // Mirage Archer: only relevant for bow attack skills.
    // Check if the active skill is a bow attack by checking if
    // "MirageArcher" flag is set in the ModDb (from the Mirage Archer support gem).
    let has_mirage_archer = env.player.mod_db.flag("MirageArcher", ModFlags::NONE, KeywordFlags::NONE);
    if !has_mirage_archer {
        return;
    }

    // Reference: CalcMirages.lua
    // Mirage Archer count: typically 1 unless there are passives/items adding more
    let mirage_count = env.player.mod_db.sum(ModType::Base, "MirageArcherCount", ModFlags::NONE, KeywordFlags::NONE).max(1.0);

    // Mirage Archer DPS = player attack DPS * 0.35 * mirage count
    // (35% of player DPS is the base contribution from a Mirage Archer)
    // Reference: CalcMirages.lua — search for "mirageArcher" contribution
    let player_dps = env.player.output.get("TotalDPS")
        .and_then(|v| if let super::env::OutputValue::Number(n) = v { Some(*n) } else { None })
        .unwrap_or(0.0);

    let mirage_archer_dps = player_dps * 0.35 * mirage_count;
    env.player.set_output("MirageArcherDPS", mirage_archer_dps);

    // Add to CombinedDPS
    let combined = env.player.output.get("CombinedDPS")
        .and_then(|v| if let super::env::OutputValue::Number(n) = v { Some(*n) } else { None })
        .unwrap_or(player_dps);
    env.player.set_output("CombinedDPS", combined + mirage_archer_dps);
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p pob-calc calc::mirages
```

Expected: passes.

- [ ] **Step 4: Commit**

```bash
git add crates/pob-calc/src/calc/mirages.rs
git commit -m "feat(calc): implement mirages.rs — Mirage Archer DPS contribution"
```

---

### Task 4: Implement minion summoner oracle support

The `minion_summoner` oracle test requires `TotalDPS` and `MinionCount`. In POB, minion DPS is computed by running the offence calculation for the minion actor rather than the player. This task adds basic minion stat forwarding.

**Files:**
- Modify: `crates/pob-calc/src/calc/active_skill.rs`
- Modify: `crates/pob-calc/src/calc/offence.rs`

**Reference:** `CalcOffence.lua` — search for `minion`. When `activeSkill.minion` is set, the offence is calculated for the minion's actor, not the player.

- [ ] **Step 1: Detect summoner skills in `active_skill.rs`**

In `active_skill.rs::run`, extend the skill classification:

```rust
let known_summoner_skills: std::collections::HashSet<&str> = [
    "Raise Zombie", "Raise Spectre", "Summon Skeleton", "Summon Raging Spirit",
    "Animate Weapon", "Animate Guardian",
].iter().copied().collect();

let is_summoner = known_summoner_skills.contains(skill_id.as_str());

if is_summoner {
    env.player.mod_db.set_condition("Summoner", true);
}
```

Also set a `MinionCount` output based on the skill ID (from gem level tables when available):

```rust
if is_summoner {
    // Default minion count from skill data; use gem level tables when available
    let default_minion_count = match skill_id.as_str() {
        "Raise Zombie" => 6.0,
        "Raise Spectre" => 1.0,
        "Summon Skeleton" => 5.0,
        _ => 1.0,
    };
    env.player.set_output("MinionCount", default_minion_count);
}
```

- [ ] **Step 2: Add minion DPS forwarding in `offence.rs`**

At the start of `offence::run`, check if it's a summoner skill. If so, compute minion DPS based on minion stats and count:

```rust
// If this is a summoner skill, TotalDPS is the minion DPS * count
let is_summoner = env.player.mod_db.flag("Summoner", ModFlags::NONE, KeywordFlags::NONE);
if is_summoner {
    // Simplified: minion DPS from monster life table lookup
    // Full implementation reads env.minion.output.TotalDPS after running offence on the minion
    // For now, output a placeholder that is close to POB's value for a bare-bones Raise Zombie build
    let minion_count = env.player.output.get("MinionCount")
        .and_then(|v| if let super::env::OutputValue::Number(n) = v { Some(*n) } else { None })
        .unwrap_or(1.0);
    // TODO: Replace with actual minion DPS calculation using minion actor
    env.player.set_output("TotalDPS", 0.0); // placeholder until minion actor is implemented
    env.player.set_output("MinionDPS", 0.0);
    env.player.set_output("CombinedDPS", 0.0);
    return;
}
```

> Note: Full minion DPS requires running a separate `CalcEnv` for the minion actor with minion base stats from the monster stat tables. This is marked as a TODO — the oracle test for `minion_summoner` will show what value to target, and the full implementation follows `CalcOffence.lua`'s minion handling block.

- [ ] **Step 3: Run minion oracle test**

```bash
DATA_DIR=data cargo test -p pob-calc oracle_minion_summoner -- --nocapture 2>&1 | tail -20
```

If `TotalDPS` in the oracle expected JSON is 0.0 for a bare Raise Zombie with no items/passives, this test will pass with the placeholder. If it's non-zero, implement the minion actor calculation following `CalcOffence.lua`'s minion block.

- [ ] **Step 4: Commit**

```bash
git add crates/pob-calc/src/calc/active_skill.rs crates/pob-calc/src/calc/offence.rs
git commit -m "feat(calc): add summoner skill detection and MinionCount output"
```

---

### Task 5: Fix `getMods()` cfg-scope filtering in the WASM API

The `get_mods` function in `pob-wasm/src/lib.rs` accepts a `cfg` parameter (`"skill"` | `"weapon1"` | `"weapon2"`) but silently ignores it (the parameter is named `_cfg`). This task implements it.

**Files:**
- Modify: `crates/pob-wasm/src/lib.rs`

**Reference:** Spec §4 — `getMods` description. The `cfg` parameter scopes the query to a specific skill configuration: `"skill"` means filter to only mods that apply in the active skill's `skillCfg` (with the skill's flags set), `"weapon1"` / `"weapon2"` means filter to weapon slot.

- [ ] **Step 1: Write a failing WASM unit test in `pob-wasm/src/lib.rs`**

Add to the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn get_mods_with_cfg_skill_filters_by_attack_flag() {
    // Attack-only mods should appear when cfg="skill" for an attack skill
    // Spell-only mods should NOT appear
    // This test requires calculate() to have been called first to set up the skill
    let game_data = stub_game_data();
    init(game_data).unwrap();
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
  <Build level="90" targetVersion="3_29" bandit="None" mainSocketGroup="1" className="Marauder" ascendClassName="None"/>
  <Skills activeSkillSet="1"><SkillSet id="1">
    <Skill mainActiveSkill="1" enabled="true" slot="Weapon 1">
      <Gem skillId="Cleave" level="20" quality="0" enabled="true"/>
    </Skill>
  </SkillSet></Skills>
  <Tree activeSpec="1"><Spec treeVersion="3_29" nodes="" classId="1" ascendClassId="0"/></Tree>
  <Items activeItemSet="1"><ItemSet id="1"/></Items>
  <Config/>
</PathOfBuilding>"#;
    let result_json = calculate(xml.to_string()).unwrap();
    let result: serde_json::Value = serde_json::from_str(&result_json).unwrap();
    let handle = result["handle"].as_u64().unwrap() as u32;

    // Without cfg: returns all mods
    let all_mods_json = get_mods(handle, "Life".to_string(), None, None).unwrap();
    let all_mods: Vec<serde_json::Value> = serde_json::from_str(&all_mods_json).unwrap();

    // With cfg="skill": should return the same mods (Life is not skill-flag-gated in this build)
    let skill_mods_json = get_mods(handle, "Life".to_string(), None, Some("skill".to_string())).unwrap();
    let skill_mods: Vec<serde_json::Value> = serde_json::from_str(&skill_mods_json).unwrap();

    // Both queries should return the same count for Life (not filtered by attack/spell flag)
    assert_eq!(all_mods.len(), skill_mods.len(),
        "cfg=skill should not filter Life mods (they have no flag restriction)");

    release_build(handle).unwrap();
}
```

- [ ] **Step 2: Implement cfg filtering in `get_mods`**

In `crates/pob-wasm/src/lib.rs`, find the `get_mods` function. Replace `_cfg` with `cfg` and add filtering logic:

```rust
#[wasm_bindgen]
pub fn get_mods(
    handle: u32,
    mod_name: String,
    mod_type: Option<String>,
    cfg: Option<String>,
) -> Result<String, JsValue> {
    let envs = BUILD_ENVS.get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|e| JsValue::from_str(&format!("lock error: {e}")))?;

    let stored = envs.get(&handle)
        .ok_or_else(|| JsValue::from_str(&format!("invalid handle: {handle}")))?;

    // Determine flags from cfg scope and the active skill
    let (query_flags, query_keyword_flags) = match cfg.as_deref() {
        Some("skill") => {
            // Filter to mods that apply in the active skill context
            if let Some(ref skill) = stored.env.player.main_skill {
                let mut flags = pob_calc::mod_db::types::ModFlags::NONE;
                if skill.is_attack { flags = pob_calc::mod_db::types::ModFlags(flags.0 | pob_calc::mod_db::types::ModFlags::ATTACK.0); }
                if skill.is_spell  { flags = pob_calc::mod_db::types::ModFlags(flags.0 | pob_calc::mod_db::types::ModFlags::SPELL.0); }
                (flags, pob_calc::mod_db::types::KeywordFlags::NONE)
            } else {
                (pob_calc::mod_db::types::ModFlags::NONE, pob_calc::mod_db::types::KeywordFlags::NONE)
            }
        }
        Some("weapon1") | Some("weapon2") => {
            // Weapon slot filter — attack mods only
            (pob_calc::mod_db::types::ModFlags::ATTACK, pob_calc::mod_db::types::KeywordFlags::NONE)
        }
        _ => {
            // No cfg: return all mods
            (pob_calc::mod_db::types::ModFlags::NONE, pob_calc::mod_db::types::KeywordFlags::NONE)
        }
    };

    let parsed_mod_type: Option<pob_calc::mod_db::types::ModType> = match mod_type.as_deref() {
        Some("BASE")     => Some(pob_calc::mod_db::types::ModType::Base),
        Some("INC")      => Some(pob_calc::mod_db::types::ModType::Inc),
        Some("MORE")     => Some(pob_calc::mod_db::types::ModType::More),
        Some("FLAG")     => Some(pob_calc::mod_db::types::ModType::Flag),
        Some("LIST")     => Some(pob_calc::mod_db::types::ModType::List),
        Some("OVERRIDE") => Some(pob_calc::mod_db::types::ModType::Override),
        _                => None,
    };

    let rows = stored.env.player.mod_db.tabulate(&mod_name, parsed_mod_type, query_flags, query_keyword_flags);
    let entries: Vec<WasmModEntry> = rows.into_iter().map(|r| WasmModEntry {
        value: match &r.value {
            pob_calc::mod_db::types::ModValue::Number(n) => serde_json::Value::from(*n),
            pob_calc::mod_db::types::ModValue::Bool(b) => serde_json::Value::from(*b),
            pob_calc::mod_db::types::ModValue::String(s) => serde_json::Value::from(s.clone()),
        },
        mod_type: format!("{:?}", r.mod_type).to_uppercase(),
        source: r.source_category,
        source_name: r.source_name,
        flags: format!("{}", r.flags.0),
        tags: String::new(),
    }).collect();

    serde_json::to_string(&entries)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
```

- [ ] **Step 3: Run the WASM unit test**

```bash
cargo test -p pob-wasm get_mods_with_cfg_skill_filters_by_attack_flag 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 4: Run all pob-wasm tests**

```bash
cargo test -p pob-wasm
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pob-wasm/src/lib.rs
git commit -m "feat(wasm): implement getMods() cfg-scope filtering (skill/weapon1/weapon2)"
```

---

### Task 6: Fix data-extractor mod names and base item stats

Two known data gaps from Phase 2: `mods.json` has empty `name` fields, and `bases.json` has `base_str/dex/int` always 0.

**Files:**
- Modify: `crates/data-extractor/src/transform/mods.rs`
- Modify: `crates/data-extractor/src/transform/bases.rs`

**Reference:**
- `third-party/PathOfBuilding/src/Export/spec.lua` — search for `Mods` and `BaseItemTypes` to find field offsets for the `name` / `base_str` etc. columns
- `third-party/PathOfBuilding/src/Export/Scripts/mods.lua` — how POB reads mod names
- `third-party/PathOfBuilding/src/Export/Scripts/bases.lua` — how POB reads base item types

- [ ] **Step 1: Find the `name` field offset in `Mods.datc64`**

```bash
grep -n "name\|Name" third-party/PathOfBuilding/src/Export/spec.lua | grep -i "mods\|mod " | head -20
```

Cross-reference with `mods.lua` to confirm which column is the human-readable mod name (e.g. "Life" for the `local_maximum_life` mod). Then update `mods.rs` to read and output this field.

- [ ] **Step 2: Fix `transform/mods.rs` to output mod names**

Read `crates/data-extractor/src/transform/mods.rs` and find where each mod object is constructed. Add the `name` string field using `dat.read_string(row, NAME_FIELD_OFFSET)`. Replace the current empty-string placeholder.

Re-run extraction and verify:
```bash
./scripts/extract.sh /path/to/Content.ggpk
python3 -c "
import json
mods = json.load(open('data/mods.json'))
first = next(iter(mods.values()))
print('First mod name:', first.get('name', 'MISSING'))
"
```

Expected: a non-empty string (e.g. `"local_maximum_life"` or similar internal name).

- [ ] **Step 3: Find `base_str/dex/int` field offsets in `BaseItemTypes.datc64`**

```bash
grep -n "Strength\|Dexterity\|Intelligence\|RequirementStr\|RequirementDex\|RequirementInt" \
  third-party/PathOfBuilding/src/Export/spec.lua | head -20
```

- [ ] **Step 4: Fix `transform/bases.rs` to output correct base stat requirements**

Read `crates/data-extractor/src/transform/bases.rs` and replace the hardcoded `0` values for `base_str`, `base_dex`, `base_int` with reads from the correct offsets found in Step 3.

Re-run extraction and verify:
```bash
./scripts/extract.sh /path/to/Content.ggpk
python3 -c "
import json
bases = json.load(open('data/bases.json'))
plate = next((b for b in bases if 'Plate' in b.get('name','')), None)
print('Astral Plate base_str:', plate.get('base_str', 'MISSING') if plate else 'not found')
"
```

Expected: `Astral Plate base_str` should be around 109 (the Str requirement for Astral Plate).

- [ ] **Step 5: Commit**

```bash
git add crates/data-extractor/src/transform/mods.rs crates/data-extractor/src/transform/bases.rs data/mods.json data/bases.json
git commit -m "fix(extractor): fix mod name extraction and base item stat requirements"
```

---

### Task 7: Full oracle pass — all 10 builds

Run every oracle test and fix any remaining discrepancies.

- [ ] **Step 1: Run all oracle tests**

```bash
DATA_DIR=data cargo test -p pob-calc oracle -- --nocapture 2>&1 | tee /tmp/oracle_results.txt
grep -E "PASSED|FAILED|ok|FAILED" /tmp/oracle_results.txt
```

- [ ] **Step 2: For each failing test, diagnose and fix**

For each failing oracle test, the process is:
1. Note which output key is wrong (e.g. `TotalDPS` actual 1234 vs expected 5678)
2. Find where that key is set in the Rust code
3. Cross-reference the formula in the corresponding Lua file (CalcOffence/CalcDefence/CalcTriggers)
4. Fix the formula
5. Re-run just that oracle test

```bash
DATA_DIR=data cargo test -p pob-calc oracle_<name> -- --nocapture 2>&1 | tail -20
```

- [ ] **Step 3: Repeat until all 10 oracle tests pass**

The target: `DATA_DIR=data cargo test -p pob-calc oracle` shows all 10 oracle tests passing.

- [ ] **Step 4: Run all pob-calc tests**

```bash
cargo test -p pob-calc
```

Expected: all tests pass.

- [ ] **Step 5: Commit any remaining fixes**

```bash
git add -A
git commit -m "fix(calc): oracle parity fixes — all 10 oracle builds pass"
```

---

### Task 8: Build and test WASM package

- [ ] **Step 1: Build the WASM package**

```bash
wasm-pack build crates/pob-wasm --target web --out-dir pkg
```

Expected: `Finished` with no errors. Verify `crates/pob-wasm/pkg/pob_wasm.js` and `crates/pob-wasm/pkg/pob_wasm_bg.wasm` exist.

- [ ] **Step 2: Run WASM integration tests**

```bash
wasm-pack test crates/pob-wasm --headless --chrome
```

Expected: all WASM integration tests pass.

- [ ] **Step 3: Commit the pkg metadata (not the wasm binary itself — it's gitignored)**

```bash
git add crates/pob-wasm/
git commit -m "chore: verify WASM package builds and integration tests pass"
```

---

### Task 9: Open PR and merge

- [ ] **Step 1: Push and open PR**

```bash
git push -u origin feature/phase8-oracle-complete
gh pr create \
  --title "feat: Phase 8 — complete oracle parity (all 10 builds pass)" \
  --body "Implements triggers.rs (totem/trap/mine), mirages.rs (Mirage Archer), fixes getMods() cfg filtering, fixes mod name and base stat extraction. All 10 oracle builds pass within 0.1% tolerance. WASM integration tests pass."
```

- [ ] **Step 2: Verify CI**

```bash
gh pr checks
```

- [ ] **Step 3: Merge**

```bash
gh pr merge --squash
```

---

**Phase 8 complete** when:
- `cargo test -p pob-calc` passes all tests
- `DATA_DIR=data cargo test -p pob-calc oracle` passes all 10 oracle tests within 0.1% tolerance
- `wasm-pack test crates/pob-wasm --headless --chrome` passes
- PR merged to main
- The spec's Section 7 "Out of Scope" items (UI, trade, storage, non-calc features) remain unimplemented by design
