# PoB WASM Completeness Ledger

This file is the single source of truth for migration completeness.

## Global Rules

- Lua is the source of truth, not current test coverage.
- No skipped Lua branches, no acceptable gaps.
- A chunk is COMPLETE only when all five gates below are satisfied with evidence.
- Canonical execution order is fixed: `SETUP -> FIX -> PERF -> DEF -> OFF -> TRIG -> MIR -> AGG`.

## Status Vocabulary

- `NOT_STARTED`: No implementation attempt in current migration pass.
- `IN_PROGRESS`: Active work, one or more gates still open.
- `BLOCKED`: Cannot proceed due to an upstream chunk or data-pipeline blocker.
- `COMPLETE`: All gates closed with evidence.

## Five Completion Gates (required per chunk)

1. Lua branch parity
2. Architecture parity
3. Chunk oracle (`CHUNK=<id> ... chunk_oracle`) is 30/30
4. Regression (`cargo test --workspace --exclude pob-wasm`) passes
5. Independent review verdict is PASS

## Global Progress Snapshot

- Last parity run: 2026-04-06 (local)
- Command: `DATA_DIR=/Users/andreashoffmann1/projects/pob-wasm/data cargo test --test parity_report -- --nocapture`
- Chunks PASS: `6/28`
- Fields correct: `7396/8643 (85.6%)`
- Workspace tests: not run in this status capture session

## Canonical Chunk Order Checklist

### SETUP

- [ ] SETUP-02-active-skills
- [ ] SETUP-05-cluster-jewels
- [ ] SETUP-06-timeless-jewels
- [ ] SETUP-07-anointments
- [ ] SETUP-08-radius-jewels
- [ ] SETUP-09-radius-jewels (legacy chunk ID in test registry)
- [ ] SETUP-09-mastery-selections
- [ ] SETUP-10-keystone-merging
- [ ] SETUP-11-item-conditions
- [ ] SETUP-12-bandit-pantheon
- [ ] SETUP-13-buff-mode
- [ ] SETUP-14-tattoo-overrides
- [ ] SETUP-15-forbidden-flesh-flame
- [ ] SETUP-16-special-uniques

### FIX

- [ ] FIX-01-stat-name-mismatches
- [ ] FIX-02-per-slot-defence
- [ ] FIX-03-radius-jewel-callbacks
- [ ] FIX-04-glorious-vanity-normals
- [ ] FIX-05-tattoo-data
- [x] FIX-06-perf02-medium-gaps
- [ ] FIX-07-energy-blade
- [x] FIX-08-mana-computation

### PERF

- [x] PERF-01-attributes
- [ ] PERF-02-life-mana-es
- [x] PERF-03-charges
- [x] PERF-04-reservation
- [x] PERF-05-buffs
- [ ] PERF-06-aura-curse
- [ ] PERF-07-regen-recharge-leech
- [ ] PERF-08-action-speed-conditions

### DEF

- [ ] DEF-01-resistances
- [ ] DEF-02-armour-evasion-es-ward
- [ ] DEF-03-block-suppression
- [ ] DEF-04-damage-reduction-avoidance
- [ ] DEF-05-recovery-in-defence
- [ ] DEF-06-ehp

### OFF

- [ ] OFF-01-base-damage
- [ ] OFF-02-conversion
- [ ] OFF-03-crit-hit
- [ ] OFF-04-speed-dps
- [ ] OFF-05-ailments
- [ ] OFF-06-dot-impale
- [ ] OFF-07-combined-dps

### TRIG

- [ ] TRIG-01-trigger-rates
- [ ] TRIG-02-totem-trap-mine

### MIR

- [ ] MIR-01-mirages

### AGG

- [ ] AGG-01-full-dps

## Active Chunk Ledger

### OFF-01-base-damage

- Status: IN_PROGRESS
- Dependencies: Tier-3 chunks complete, canonical order enforced by this file
- Lua scope: `third-party/PathOfBuilding/src/Modules/CalcOffence.lua` (OFF-01 section)
- Rust scope: `crates/pob-calc/src/calc/offence.rs`
- Oracle fields: `AverageDamage`, `AverageBurstDamage`, `AverageBurstHits`

#### Gate 1: Lua branch parity

- [ ] Every if/elseif/else branch in OFF-01 Lua scope mapped
- [ ] Every relevant modDB/skillModList query mapped
- [ ] Every OFF-01 output write mapped
- Evidence:
  - Missing branches count: unknown (full branch audit not yet recorded in this ledger)

#### Gate 2: Architecture parity

- [ ] Uses the same data source semantics as Lua
- [ ] No shortcut algorithm replacing Lua semantics
- Evidence:
  - Pending independent architecture parity review for OFF-01

#### Gate 3: Chunk oracle

- [ ] 30/30 builds pass
- Evidence:
  - Command: `CHUNK=OFF-01-base-damage DATA_DIR=/Users/andreashoffmann1/projects/pob-wasm/data cargo test --test chunk_oracle -- --nocapture`
  - Current: `Builds: 5/30 pass`, `Fields: 40/90 correct across all builds`

#### Gate 4: Regression

- [ ] Workspace tests pass
- Evidence:
  - Not run after current OFF-01 status capture

#### Gate 5: Independent review verdict

- [ ] PASS
- Evidence:
  - No independent OFF-01 review captured yet
