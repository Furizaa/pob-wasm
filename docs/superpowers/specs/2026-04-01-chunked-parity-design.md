# Chunked Parity Convergence Design

**Date:** 2026-04-01
**Status:** Draft
**Predecessor:** `2026-03-30-pob-wasm-parity-design.md` (Phases 1-10)
**Scope:** Replace Phase 10 ("Long Tail") with a structured, agent-safe convergence strategy

## 1. Problem Statement

The original parity design defined 10 phases. Phases 1-9 were committed as complete, but
the Rust calculation modules are roughly 52% the line count of their Lua equivalents. Running
all 30 oracle tests produces **0 passes out of 30**. A representative build (phys melee slayer)
shows **413 output fields missing, 106 with wrong values, and only ~112 correct out of 631** —
approximately 18% field parity.

Phase 10 ("Long Tail") was defined as iterative convergence to fix all remaining gaps. This
phase is too large and unstructured for agent-based execution. Agents given Phase 10 as a task:

- Cannot hold the full scope (880 unique output fields across 41 builds) in context
- Have no focused feedback loop — running the full oracle suite produces thousands of failure lines
- Fail silently when sub-agents hang or encounter connection errors
- Claim completion without evidence, because there's no scoped test to disprove the claim

## 2. Goal

**100% field parity** with Path of Building's PoE 1 calculation engine, measured by:

- All 30+ realworld oracle builds pass `assert_output_strict` with 0.1% numeric tolerance
- All 880 unique output fields correct across all builds
- Zero regressions — `cargo test --workspace --exclude pob-wasm` green
- Progress tracked via a monotonically-increasing parity percentage

## 3. Approach: Annotated Lua Chunks + Per-Chunk Tests

The work is decomposed into ~28-35 **chunks**. Each chunk is a coherent subsystem of PoB's
calculation engine (50-500 lines of Lua) that writes a specific set of output fields. Chunks
are ordered by data dependencies and executed one at a time.

Three artifacts make each chunk independently completable and verifiable:

1. **Annotated Lua reference** — the relevant Lua source with semantics notes, gotchas, output
   fields, and a delta against the current Rust code
2. **Per-chunk test** — a test that runs all 30 oracle builds but only asserts on the chunk's
   output fields, giving focused pass/fail feedback
3. **Parity dashboard** — a summary report tracking correct fields out of 880 across all builds

## 4. Chunk Definition

Each chunk is defined by:

| Field | Description |
|-------|-------------|
| **ID** | `{MODULE}-{NN}-{name}`, e.g. `DEF-01-resistances` |
| **Module** | Which Lua file(s) the logic lives in |
| **Lua lines** | Specific line range(s) in the Lua source |
| **Output fields** | The exact set of output keys this chunk writes |
| **Dependencies** | Which chunks must be correct before this one |
| **Rust file(s)** | Which Rust file(s) need modification |
| **Scope** | Small (< 100 Lua lines), Medium (100-300), Large (300-500) |

## 5. Dependency Tiers

Based on PoB's execution flow (`CalcSetup -> CalcPerform -> CalcDefence/CalcOffence ->
CalcTriggers -> CalcMirages -> Calcs.lua`):

```
Tier 0: Foundation (CalcSetup)
  Item processing, passive tree mods, ModDb population, eval_mod stubs
  No output fields directly, but everything depends on this being correct.

Tier 1: Attributes & Pools (CalcPerform early)
  Str/Dex/Int, Life, Mana, ES, charges, conditions

Tier 2: Buffs & Auras (CalcPerform mid)
  Reservation, aura effects, curse effects, named buffs

Tier 3: Regen & Recovery (CalcPerform late)
  Life/Mana/ES regen, leech, recharge, action speed

Tier 4: Defence (CalcDefence)
  Resistances, Armour, Evasion, Block, Suppression, EHP

Tier 5: Offence (CalcOffence)
  Base damage, conversion, crit, speed, hit DPS, ailments, DoT, combined DPS

Tier 6: Triggers & Mirages (CalcTriggers, CalcMirages)
  Trigger rates, totem/trap/mine, mirage actors

Tier 7: Aggregation (Calcs.lua)
  FullDPS, multi-skill summation
```

### 5.1 Preliminary Chunk List

This is refined during Phase 0 when the field-to-Lua mapping is complete. Estimated
breakdown based on Lua module structure:

```
SETUP-01  Item mod parsing & slot assignment                  setup.rs
SETUP-02  Support gem matching & active skill construction    active_skill.rs
SETUP-03  Flask, jewel, & aura/curse buff setup               setup.rs
SETUP-04  eval_mod stub completion (5 tag types)              eval_mod.rs
SETUP-05  Cluster jewel subgraph generation                   passive_tree/, setup.rs

PERF-01   Attributes (Str/Dex/Int/Omni)                      perform.rs
PERF-02   Life/Mana/ES pools                                  perform.rs
PERF-03   Charges (power/frenzy/endurance/special)            perform.rs
PERF-04   Reservation (mana + life)                           perform.rs
PERF-05   Buffs & debuffs (Fortify, Onslaught, Elusive, ...) perform.rs
PERF-06   Aura & curse application                            perform.rs
PERF-07   Regen / recharge / leech                            perform.rs
PERF-08   Action speed, conditions, MoM/EB                    perform.rs

DEF-01    Resistances (5 elements, uncapped, overcap)         defence.rs
DEF-02    Armour / Evasion / ES / Ward from gear              defence.rs
DEF-03    Block (attack + spell) & spell suppression          defence.rs
DEF-04    Damage reduction, avoidance, damage shift           defence.rs
DEF-05    Recovery rates (regen, leech, recharge in defence)  defence.rs
DEF-06    EHP calculations                                    defence_ehp.rs

OFF-01    Base damage & weapon integration                    offence.rs
OFF-02    Damage conversion chain                             offence.rs, offence_utils.rs
OFF-03    Crit (chance, multiplier, effective) & hit chance   offence.rs
OFF-04    Attack/cast speed & hit DPS                         offence.rs
OFF-05    Ailments (ignite, bleed, poison)                    offence_ailments.rs
OFF-06    Non-ailment DoT, impale                             offence_dot.rs
OFF-07    Combined DPS, breakdowns, minion offence            offence.rs, offence_dot.rs

TRIG-01   Trigger rates (CoC/CwC/CWDT/craft)                 triggers.rs
TRIG-02   Totem / trap / mine / ballista                      triggers.rs

MIR-01    Mirage actors (Mirage Archer, Saviour, General)     mirages.rs

AGG-01    FullDPS & multi-skill aggregation                   calcs.rs

TAIL-01+  Edge cases, special uniques, minion actors          (various)
```

Total: ~31 chunks, plus a variable number of TAIL chunks for edge cases discovered
during execution.

**Note on SETUP-05 (cluster jewels):** Cluster jewels generate a dynamic sub-tree
of passive nodes (large/medium/small clusters with notables and small passives).
These synthetic nodes contribute attribute bonuses, damage mods, and other stats
that affect any build using cluster jewels. Without this chunk, builds with cluster
jewels will fail parity checks in every downstream chunk. Two of the 30 oracle
builds (`realworld_cluster_jewel`, `realworld_coc_trigger`) are known to fail
PERF-01 due to missing cluster jewel attribute bonuses. SETUP-05 must be completed
before all 30 builds can pass Tier 1+ chunks.

## 6. Annotated Lua Reference Docs

### 6.1 Location

`docs/lua-reference/<chunk-id>.md` — one file per chunk.

### 6.2 Format

```markdown
# {CHUNK-ID}: {Title}

## Output Fields
List of exact output key names this chunk must write.

## Dependencies
List of chunk IDs that must be correct before this chunk.

## Lua Source
File: {filename}, lines {start}-{end}
Commit: {submodule commit hash}

## Annotated Lua
The Lua source with inline comments explaining:
- Lua-specific semantics (nil coalescing, 1-based indexing, local aliases)
- PoB-specific patterns (modDB:Sum/More/Flag signatures, output[] writes)
- Tricky logic branches, edge cases, special-case items/keystones

## Existing Rust Code
File: {filename}, lines {start}-{end}
Summary of what exists, what's missing, what's wrong.

## What Needs to Change
Numbered list of specific changes to make.
```

### 6.3 Creation Process

An exploration agent generates the reference doc by reading the Lua source and the
existing Rust code simultaneously. A human reviews each doc for correctness of the
Lua semantics annotations before the chunk is assigned for implementation.

### 6.4 Lua Gotcha Cheat Sheet

A shared reference at `docs/lua-reference/LUA-GOTCHAS.md` covering patterns that
appear across all chunks:

| Lua Pattern | Rust Equivalent | Notes |
|-------------|-----------------|-------|
| `x or 0` | `.unwrap_or(0.0)` | Nil coalescing |
| `m_min(x, y)` | `x.min(y)` | Aliased at file top: `local m_min = math.min` |
| `m_max(x, y)` | `x.max(y)` | Same aliasing pattern |
| `m_floor(x)` | `x.floor()` | |
| `m_ceil(x)` | `x.ceil()` | |
| `m_huge` | `f64::INFINITY` | `local m_huge = math.huge` |
| `round(x)` | `x.round()` | PoB's `round()` is standard rounding |
| `modDB:Sum("BASE", cfg, "X")` | `mod_db.sum_cfg(cfg, output, "X")` | `cfg` can be `nil`/`None` |
| `modDB:More(cfg, "X")` | `mod_db.more_cfg(cfg, output, "X")` | Returns product of all `(1 + value)` |
| `modDB:Flag(cfg, "X")` | `mod_db.flag_cfg(cfg, output, "X")` | Returns bool |
| `modDB:Override(cfg, "X")` | `mod_db.override_value(cfg, output, "X")` | Returns Option<f64> |
| `modDB:List(cfg, "X")` | `mod_db.list(cfg, "X")` | Returns Vec of mod values |
| `output["X"] = val` | `output.insert("X".into(), val.into())` | Always OutputValue |
| `output.X = val` | Same | Lua dot and bracket access are equivalent |
| `t_insert(list, item)` | `list.push(item)` | `local t_insert = table.insert` |
| `ipairs(t)` | `.iter().enumerate()` | 1-based, sequential only |
| `pairs(t)` | `.iter()` | All keys, unordered |
| `#t` | `.len()` | Length operator |
| `env.player.modDB` | `env.player.mod_db` | Actor's modifier database |
| `env.enemy.modDB` | `env.enemy.mod_db` | Enemy's modifier database |
| `breakdown` guarded blocks | Always populate in Rust | PoB guards with `if breakdown then`; Rust always populates |

## 7. Per-Chunk Test Infrastructure

### 7.1 Field Group Registry

New file: `crates/pob-calc/tests/field_groups.rs`

Defines a function `fields_for_chunk(chunk_id) -> &[&str]` returning the output field
names that belong to each chunk. This is the authoritative mapping from chunk ID to
expected output fields.

The registry also includes `all_chunk_ids() -> &[&str]` for enumeration.

### 7.2 Chunk Test Runner

New test binary: `crates/pob-calc/tests/chunk_oracle.rs`

Invoked via:
```bash
CHUNK=DEF-01-resistances DATA_DIR=./data cargo test --test chunk_oracle -- --nocapture
```

Behavior:
1. Read `CHUNK` env var, look up field list from `field_groups.rs`
2. Load game data from `DATA_DIR`
3. For each of the 30 realworld oracle builds (not the simpler/legacy builds):
   a. Parse XML, run `calculate()`, get actual output
   b. Load expected JSON
   c. Compare only the chunk's fields (0.1% tolerance)
   d. Record pass/fail per build
4. Print summary: `DEF-01-resistances: 28/30 builds pass`
5. Print per-build details for failures
6. Exit with non-zero status if any build fails

### 7.3 Parity Dashboard

New test binary: `crates/pob-calc/tests/parity_report.rs`

Invoked via:
```bash
DATA_DIR=./data cargo test --test parity_report -- --nocapture
```

Behavior:
1. For each realworld oracle build: parse, calculate, load expected
2. For each chunk: check its fields across all builds
3. Print summary table:

```
Chunk                         Builds   Fields   Status
PERF-01-attributes            30/30    18/18    PASS
PERF-02-life-mana-es          27/30    12/15    PARTIAL
DEF-01-resistances             0/30     0/12    FAIL
...
Total: 234/880 fields correct (26.6%)
```

4. Also print overall per-build summaries (which builds are closest to passing)

### 7.4 Relationship to Existing Oracle Tests

The existing `oracle.rs` tests (30 `oracle_*` tests using `assert_output_strict`) remain
unchanged. They are the final gate: a build passes only when ALL its output fields are
correct. The chunk tests and parity dashboard are additive — they provide focused
feedback during development without replacing the final strict check.

## 8. Agent Execution Workflow

### 8.0 Guiding Principle: Port the Lua, Not the Tests

**The goal is a faithful port of the Lua source code, not making the 30 oracle tests
pass.** The oracle tests are a verification tool, not the definition of correctness.
The Lua source is the source of truth.

This means:

- **Port every code path** in the Lua section, even if no current oracle build exercises
  it. If the Lua has an `if modDB:Flag(nil, "SomeRareKeystone")` branch, the Rust must
  have it too — even if none of the 30 builds use that keystone.
- **No dead code, no empty fallbacks.** If a function has a fallback comment like
  "if not found in tree, use enchant text as-is" but the fallback block is empty, that's
  a bug. Port the Lua's fallback behavior.
- **No `let _ = variable;` suppressions** that hide unfinished work. If a field is parsed
  (like `inc_effect`) it must be applied where the Lua applies it.
- **The reference doc's "What Needs to Change" section is a minimum**, not a maximum. If
  reading the Lua reveals additional code paths not mentioned in the reference doc, port
  those too.
- **Tests verify completeness, they don't define it.** After porting all Lua logic, run
  the chunk test. If it passes, good. If it fails, you have a bug to fix. If it passes
  but you skipped Lua code paths, you're not done.

### 8.1 One Chunk Per Session

Each agent session works on exactly one chunk. The session follows this sequence:

1. **Load** the chunk's annotated Lua reference from `docs/lua-reference/<chunk-id>.md`
2. **Read the Lua source directly** — open the actual Lua file(s) listed in the reference
   doc and read the full section, not just the annotated excerpts. The reference doc may
   have missed code paths.
3. **Read** the current Rust code that the reference doc points to
4. **Port all Lua logic** for this chunk's section into Rust. Every branch, every edge
   case, every fallback. Use the reference doc for guidance on Lua semantics but trust
   the Lua source when they disagree.
5. **Run chunk test** — verify the port is correct against oracle builds
6. **Run** `cargo test --workspace --exclude pob-wasm` — verify no regressions
7. **Commit** with message: `parity({CHUNK-ID}): {description} — {N}/{M} fields across 30 builds`

### 8.2 Agent Constraints

- **No sub-agents for implementation.** The main agent does the work directly. Sub-agents
  are permitted only for read-only exploration (tracing a dependency, checking a Lua function).
- **Hard success criterion.** The agent must produce chunk test output showing pass/fail.
  "Done" without test evidence is not done.
- **Chunk test must pass before commit.** No deferring fixes to the next chunk.
- **One chunk at a time.** Never ask an agent to do two chunks in one session.
- **No skipped Lua code paths.** If the Lua has a branch that handles a rare case, the
  Rust must handle it too. Do not skip code because "no oracle build tests this."

### 8.3 Failure Recovery

Because each chunk is small (one subsystem, 50-500 Lua lines):

- **Agent hangs or disconnects:** Maximum lost work is ~30 minutes. Start a new session,
  give it the same chunk reference doc. The chunk test tells the new agent exactly where
  things stand.
- **Agent claims completion but test fails:** The chunk test output proves it. Tell the
  agent to continue, or start a fresh session.
- **Agent introduces regressions:** `cargo test` catches compilation and unit test failures.
  The chunk test catches output field regressions within the chunk's scope.

### 8.4 Session Prompt Template

When starting a chunk, use this prompt structure:

```
You are porting PoB's Lua calculations to Rust. Work on chunk {CHUNK-ID}.

THE LUA SOURCE IS THE SOURCE OF TRUTH, NOT THE TESTS. Your job is to faithfully
port every code path in the Lua section — including branches no current test
exercises. The oracle tests verify your port; they do not define completeness.

1. Read docs/lua-reference/{CHUNK-ID}.md for annotated Lua source and instructions
2. Open the actual Lua file(s) listed in the reference doc and read the FULL section
   for this chunk. The reference doc may have missed code paths — port them anyway.
3. Read the current Rust file(s) listed in the reference doc
4. Port ALL Lua logic for this section into Rust. Every branch, every edge case,
   every fallback, every flag check. If the Lua handles a rare keystone or a
   conditional path, the Rust must handle it too.
5. Run: CHUNK={CHUNK-ID} DATA_DIR=./data cargo test --test chunk_oracle -- --nocapture
   to verify the port against oracle builds
6. Run: cargo test --workspace --exclude pob-wasm
   to verify no regressions
7. Commit your changes

Rules:
- Do NOT use sub-agents for implementation
- Do NOT claim completion without showing chunk test output
- Do NOT skip Lua code paths because "no test exercises this"
- Do NOT leave empty fallbacks, `let _ = var;` suppressions, or TODO comments
- If you parse a value (like inc_effect), you MUST apply it where the Lua applies it
- The chunk test must pass before committing
```

## 9. Eval_mod Stub Completion (SETUP-04)

Five `ModTag` variants in `eval_mod.rs` are currently stubbed (always pass through):

| Tag | Purpose | Impact of Stub |
|-----|---------|----------------|
| `SkillName` | Mod only applies to named skill | Mods bleed across all skills |
| `SkillId` | Mod only applies to skill by ID | Same as SkillName |
| `SkillPart` | Mod only applies to skill part N | Multi-part skills get wrong values |
| `SocketedIn` | Mod scoped to gems in a specific slot | Item socket mods apply globally |
| `ItemCondition` | Condition on equipped item property | Conditional item mods always active |

These stubs cause cascading incorrect values across all chunks. They must be completed
before chunk execution begins (hence SETUP-04 as a prerequisite).

## 10. Upstream PoB Sync Strategy

### 10.1 During Parity Work

The `third-party/PathOfBuilding` submodule stays pinned to its current commit. All
annotated Lua references record the commit hash. No syncing during the convergence effort.

### 10.2 After Parity Achieved

When updating to a new PoB release:

1. Bump the submodule: `cd third-party/PathOfBuilding && git checkout <new-tag>`
2. Diff calc files: `git diff <old>..<new> -- src/Modules/Calc*.lua`
3. Identify affected chunks by cross-referencing line ranges in reference docs
4. Regenerate expected JSON: `./scripts/generate_all_oracles.sh`
5. Run parity report — any chunk that regresses needs re-porting
6. Update affected reference docs with new Lua source and annotations

### 10.3 Oracle Build Freshness

Oracle builds use real PoB XML files. When PoB's data format changes (new gem stats,
tree changes), builds may need updating. The `gen_oracle.lua` script regenerates
expected JSON from the PoB submodule, which handles data-driven changes automatically.
Structural XML format changes require manual build updates.

## 11. Phase 0 Deliverables & Sequencing

Phase 0 is the upfront investment before any chunk implementation begins.

### 11.1 Deliverables

| ID | Deliverable | Description | Sessions |
|----|-------------|-------------|----------|
| 0.1 | Output field inventory | Script parsing all expected JSON, producing union of 880 fields, classified as correct/wrong/missing | 1 |
| 0.2 | Field-to-Lua mapping | Map each output field to the Lua function that writes it | 1-2 |
| 0.3 | Chunk dependency graph | Final ordered chunk list with IDs, field sets, Lua line ranges, Rust targets | 1 |
| 0.4 | Annotated Lua references | One markdown file per chunk in `docs/lua-reference/` | 5-8 |
| 0.5 | Per-chunk test infrastructure | `field_groups.rs`, `chunk_oracle.rs`, `parity_report.rs` | 1-2 |
| 0.6 | Eval_mod stub completion | Implement 5 stubbed ModTag evaluators | 1 |
| 0.7 | Lua gotcha cheat sheet | `docs/lua-reference/LUA-GOTCHAS.md` | 1 |

**Total Phase 0 estimate: 11-16 sessions**

### 11.2 Sequencing

```
0.1 → 0.2 → 0.3    (sequential: each builds on the previous)
       ↓
      0.4           (can start once 0.3 produces the chunk list)
       ↓
      0.7           (can be written alongside 0.4)

0.5                 (independent: test infrastructure, can start after 0.3)
0.6                 (independent: eval_mod work, can start anytime)
```

After Phase 0, chunk execution proceeds in dependency order (Tier 0 → Tier 7).

### 11.3 Chunk Execution Estimate

- ~30 chunks at 1-3 sessions each
- Estimated 40-60 sessions for full convergence
- Plus ~5-10 sessions for TAIL chunks (edge cases discovered during execution)
- **Total estimate including Phase 0: 55-85 sessions**

## 12. Current State Baseline

As measured from running all 30 realworld oracle tests:

| Metric | Value |
|--------|-------|
| Oracle builds | 41 total (30 realworld + 11 simpler) |
| Unique output fields | 880 |
| Avg fields per build | 661 |
| Oracle tests passing | 0 / 30 |
| Fields correct (slayer sample) | ~112 / 631 (~18%) |
| Rust calc line count | ~11,461 lines |
| Lua calc line count | ~22,048 lines |
| Rust/Lua line ratio | 52% |

### 12.1 Output Writes by Lua Module

| Lua Module | `output.X` writes | `output["X"]` writes | Role |
|------------|-------------------|---------------------|------|
| CalcPerform.lua | 186 | 44 | Attributes, pools, buffs, regen |
| CalcDefence.lua | 490 | 437 | Resistances, defences, EHP |
| CalcOffence.lua | 950 | 96 | Damage, crit, speed, DPS, ailments |
| CalcTriggers.lua | 76 | 0 | Trigger rates |
| CalcMirages.lua | 4 | 0 | Mirage actors |
| Calcs.lua | 116 | 15 | FullDPS, aggregation |

CalcDefence and CalcOffence together account for the vast majority of output fields.

## 13. File Organization

New files created by this effort:

```
docs/
  lua-reference/
    LUA-GOTCHAS.md                    # Shared Lua→Rust translation reference
    SETUP-01-item-mods.md             # Per-chunk annotated Lua references
    SETUP-02-support-gems.md
    SETUP-03-flask-jewel-buff.md
    SETUP-04-eval-mod-stubs.md
    SETUP-05-cluster-jewels.md
    PERF-01-attributes.md
    PERF-02-life-mana-es.md
    ...                               # ~30 total chunk references
    DEF-01-resistances.md
    ...
    OFF-01-base-damage.md
    ...
  superpowers/specs/
    2026-04-01-chunked-parity-design.md   # This document

crates/pob-calc/tests/
  field_groups.rs                     # Chunk → field name mapping
  chunk_oracle.rs                     # Per-chunk focused test runner
  parity_report.rs                    # Overall parity dashboard

scripts/
  field_inventory.py                  # Phase 0.1: extract and classify all fields
```

Existing files modified during chunk execution:

```
crates/pob-calc/src/
  calc/perform.rs                     # Chunks PERF-01 through PERF-08
  calc/defence.rs                     # Chunks DEF-01 through DEF-05
  calc/defence_ehp.rs                 # Chunk DEF-06
  calc/offence.rs                     # Chunks OFF-01 through OFF-04, OFF-07
  calc/offence_ailments.rs            # Chunk OFF-05
  calc/offence_dot.rs                 # Chunk OFF-06
  calc/offence_utils.rs               # Chunk OFF-02 (conversion)
  calc/triggers.rs                    # Chunks TRIG-01, TRIG-02
  calc/mirages.rs                     # Chunk MIR-01
  calc/calcs.rs                       # Chunk AGG-01
  calc/active_skill.rs                # Chunk SETUP-02
  calc/setup.rs                       # Chunks SETUP-01, SETUP-03, SETUP-05
  passive_tree/mod.rs                 # Chunk SETUP-05
  mod_db/eval_mod.rs                  # Chunk SETUP-04
```

## 14. Success Criteria

### 14.1 Phase 0 Complete When

- [ ] Field inventory script produces 880 unique fields with correct/wrong/missing classification
- [ ] Every output field mapped to its source Lua function and assigned to a chunk
- [ ] Chunk dependency graph finalized with ~30 chunks
- [ ] Annotated Lua reference docs written for all chunks, human-reviewed
- [ ] `chunk_oracle.rs` and `parity_report.rs` compile and run
- [ ] All 5 eval_mod stubs replaced with real implementations
- [ ] Lua gotcha cheat sheet written

### 14.2 Full Parity Achieved When

- [ ] All 30 realworld oracle tests pass `assert_output_strict` (0.1% tolerance, all fields)
- [ ] Parity report shows all chunks passing: for every chunk, every field in that chunk is correct across all 30 builds that include that field in their expected output (fields absent from both expected and actual are not counted as failures)
- [ ] `cargo test --workspace --exclude pob-wasm` passes with zero failures
- [ ] No `#[ignore]` attributes on oracle tests that should be running

## 15. Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Phase 0 takes longer than estimated | Medium | Delays chunk execution by 1-2 weeks | Parallelize independent Phase 0 tasks (0.5 and 0.6 can start early) |
| Chunk boundaries are wrong (fields depend on multiple chunks) | Medium | Agent can't get chunk test green in isolation | During Phase 0.2, carefully trace data flow; some chunks may need to be merged or reordered |
| Some output fields are written conditionally (only for certain builds) | High | Field group test passes on builds that don't produce the field, giving false confidence | The chunk test should distinguish "field not in expected AND not in actual" (OK) from "field in expected but not in actual" (failure) |
| CalcOffence is too large for reasonable chunk sizes | Medium | OFF chunks exceed 500 Lua lines | Split further: e.g., OFF-01 per damage type, OFF-05a ignite, OFF-05b bleed, OFF-05c poison |
| Existing Rust code structure diverges from Lua structure | Low | Harder to map Lua sections to Rust code | Reference docs explicitly note structural differences; refactoring within a chunk is permitted if it aids parity |
| PoB upstream changes during the effort | Low | Annotated references become stale | Submodule is pinned; do not update until parity is achieved |
| Agent misinterprets Lua semantics despite annotations | Medium | Wrong Rust code, test failures | Chunk tests catch errors; Lua gotcha cheat sheet reduces common mistakes; human review of tricky chunks |
