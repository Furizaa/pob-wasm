# Parity Walkthrough

How to get pob-wasm to 100% parity with PoB, session by session.

Read this when you start a new session and don't remember where you left off.

## Quick Status Check

Run this first in every session to see where things stand:

```bash
DATA_DIR=./data cargo test --test parity_report -- --nocapture 2>&1 | tail -50
```

If `parity_report` doesn't exist yet, you're still in Phase 0. Read on.

---

## The Big Picture

```
Phase 0: Build infrastructure (this plan, ~7 tasks)
    |
Phase 0.4: Write annotated Lua references (~30 chunks, 5-8 sessions)
    |
Chunk execution: Port Lua logic chunk by chunk (~30 chunks, 40-60 sessions)
    |
Done: All 30 oracle builds pass, 880/880 fields correct
```

---

## Phase 0: Infrastructure

### Where am I?

Check which Phase 0 tasks are done:

```
[ ] Task 1: scripts/field_inventory.py exists and scripts/field_inventory_output.json exists
[ ] Task 2: docs/lua-reference/LUA-GOTCHAS.md exists
[ ] Task 3: crates/pob-calc/tests/field_groups.rs exists
[ ] Task 4: crates/pob-calc/tests/chunk_oracle.rs exists
[ ] Task 5: crates/pob-calc/tests/parity_report.rs exists
[ ] Task 6: grep "TODO(phase4)" crates/pob-calc/src/mod_db/eval_mod.rs returns nothing
[ ] Task 7: docs/lua-reference/BASELINE-PARITY.txt exists
```

Quick check:

```bash
ls scripts/field_inventory.py docs/lua-reference/LUA-GOTCHAS.md crates/pob-calc/tests/field_groups.rs crates/pob-calc/tests/chunk_oracle.rs crates/pob-calc/tests/parity_report.rs docs/lua-reference/BASELINE-PARITY.txt 2>&1
grep -c "TODO(phase4)" crates/pob-calc/src/mod_db/eval_mod.rs
```

If all files exist and grep returns 0, Phase 0 is done. Skip to "Phase 0.4".

### If Phase 0 is not done

Open a session and use this prompt:

```
Implement the Phase 0 chunk infrastructure plan.

Read docs/superpowers/plans/2026-04-01-phase0-chunk-infrastructure.md
and execute the tasks in order. Skip any task whose output files already exist.

After each task, commit your changes. Do not use sub-agents.
Run tests after task 6 (eval_mod) to verify no regressions:
  cargo test --workspace --exclude pob-wasm
```

If the session crashes mid-way, start a new session with the same prompt. The "skip any task whose output files already exist" instruction handles resumption.

---

## Phase 0.4: Annotated Lua Reference Docs

### Where am I?

Check which reference docs exist:

```bash
ls docs/lua-reference/*.md 2>/dev/null | grep -v LUA-GOTCHAS | grep -v BASELINE
```

Compare against the chunk list in `crates/pob-calc/tests/field_groups.rs` —
every chunk ID in `all_chunk_ids()` should have a matching reference doc.

### How to create one reference doc

Each reference doc covers one chunk. Do them in dependency order. Pick the first
chunk ID from this list that doesn't have a reference doc yet:

```
SETUP-05-cluster-jewels         (done)
SETUP-06-timeless-jewels
SETUP-07-anointments            HIGH — 4 oracle builds affected
SETUP-08-radius-jewels          HIGH — Thread of Hope in coc_trigger
SETUP-09-mastery-selections     MEDIUM — no oracle builds yet (pre-3.16 trees)
SETUP-10-keystone-merging       MEDIUM — keystone-granting uniques
SETUP-11-item-conditions        MEDIUM — per-shaper-item multipliers etc.
SETUP-12-bandit-pantheon        LOW — all oracle builds use "None"
SETUP-13-buff-mode              LOW — oracle assumes EFFECTIVE mode
SETUP-14-tattoo-overrides       LOW — no oracle builds
SETUP-15-forbidden-flesh-flame  LOW — no oracle builds, combine with SETUP-07
SETUP-16-special-uniques        LOW — no oracle builds
PERF-01-attributes
PERF-02-life-mana-es
PERF-03-charges
PERF-04-reservation
PERF-05-buffs
PERF-06-aura-curse
PERF-07-regen-recharge-leech
PERF-08-action-speed-conditions
DEF-01-resistances
DEF-02-armour-evasion-es-ward
DEF-03-block-suppression
DEF-04-damage-reduction-avoidance
DEF-05-recovery-in-defence
DEF-06-ehp
OFF-01-base-damage
OFF-02-conversion
OFF-03-crit-hit
OFF-04-speed-dps
OFF-05-ailments
OFF-06-dot-impale
OFF-07-combined-dps
TRIG-01-trigger-rates
TRIG-02-totem-trap-mine
MIR-01-mirages
AGG-01-full-dps
```

Use this prompt (replace `{CHUNK-ID}` with the actual chunk):

```
Create an annotated Lua reference doc for chunk {CHUNK-ID}.

Context:
- Read docs/superpowers/specs/2026-04-01-chunked-parity-design.md section 6 for the format
- Read docs/lua-reference/LUA-GOTCHAS.md for the translation cheat sheet
- Read crates/pob-calc/tests/field_groups.rs to see which output fields belong to this chunk
- Read scripts/field_inventory_output.json to see which Lua module writes each field

For the Lua source:
- Read the relevant section of third-party/PathOfBuilding/src/Modules/{CalcModule}.lua
- Identify every line that writes an output field belonging to this chunk
- Annotate tricky Lua patterns with Rust equivalents

For the existing Rust code:
- Read the Rust file listed for this chunk in the spec (section 5.1)
- Note what exists, what's missing, what's wrong compared to the Lua

Write the reference doc to docs/lua-reference/{CHUNK-ID}.md following the format in the spec.
Commit when done.

Do NOT implement any code changes. This is a research/documentation task only.
```

### When all reference docs are done

Verify:

```bash
# Count reference docs (should be ~25, one per chunk)
ls docs/lua-reference/*.md | grep -v LUA-GOTCHAS | grep -v BASELINE | wc -l
```

Once all chunks have reference docs, move to chunk execution.

---

## Chunk Execution: The Main Work

### Where am I?

Run the parity dashboard:

```bash
DATA_DIR=./data cargo test --test parity_report -- --nocapture 2>&1 | tail -50
```

Look at the output. Chunks marked `PASS` are done. Pick the first chunk marked
`FAIL` or `PARTIAL` that has all its dependencies satisfied (all chunks above it
in the tier list are `PASS`).

### Dependency order

Work top to bottom. A chunk can only be worked on when everything above it passes:

```
Tier 0 — HIGH priority (do first, affect oracle builds):
  SETUP-05-cluster-jewels         (done)
  SETUP-06-timeless-jewels
  SETUP-07-anointments
  SETUP-08-radius-jewels

Tier 0 — MEDIUM priority (no oracle builds fail yet, but needed for completeness):
  SETUP-09-mastery-selections
  SETUP-10-keystone-merging
  SETUP-11-item-conditions

Tier 0 — LOW priority (all oracle builds use "None"/default, add when needed):
  SETUP-12-bandit-pantheon
  SETUP-13-buff-mode
  SETUP-14-tattoo-overrides
  SETUP-15-forbidden-flesh-flame
  SETUP-16-special-uniques

Tier 1 (needs Tier 0):
  PERF-01-attributes
  PERF-02-life-mana-es
  PERF-03-charges
  PERF-04-reservation

Tier 2 (needs Tier 1):
  PERF-05-buffs
  PERF-06-aura-curse

Tier 3 (needs Tier 2):
  PERF-07-regen-recharge-leech
  PERF-08-action-speed-conditions

Tier 4 (needs Tier 3):
  DEF-01-resistances
  DEF-02-armour-evasion-es-ward
  DEF-03-block-suppression
  DEF-04-damage-reduction-avoidance
  DEF-05-recovery-in-defence
  DEF-06-ehp

Tier 5 (needs Tier 3):
  OFF-01-base-damage
  OFF-02-conversion
  OFF-03-crit-hit
  OFF-04-speed-dps
  OFF-05-ailments
  OFF-06-dot-impale
  OFF-07-combined-dps

Tier 6 (needs Tier 5):
  TRIG-01-trigger-rates
  TRIG-02-totem-trap-mine
  MIR-01-mirages

Tier 7 (needs everything):
  AGG-01-full-dps
```

Note: Tier 4 and Tier 5 are independent of each other. You can work on DEF chunks
and OFF chunks in any interleaved order, as long as Tier 3 is done.

### Prompt for working on a chunk

Replace `{CHUNK-ID}` with the chunk you're working on:

```
You are porting PoB's Lua calculations to Rust. Work on chunk {CHUNK-ID}.

THE LUA SOURCE IS THE SOURCE OF TRUTH, NOT THE TESTS. Your job is to faithfully
port every code path in the Lua section — including branches no current test
exercises. The oracle tests verify your port; they do not define completeness.

1. Read docs/lua-reference/{CHUNK-ID}.md for annotated Lua source and instructions
2. Read docs/lua-reference/LUA-GOTCHAS.md for Lua-to-Rust translation patterns
3. Open the actual Lua file(s) listed in the reference doc and read the FULL section
   for this chunk. The reference doc may have missed code paths — port them anyway.
4. Read the current Rust file(s) listed in the reference doc
5. Port ALL Lua logic for this section into Rust. Every branch, every edge case,
   every fallback, every flag check. If the Lua handles a rare keystone or a
   conditional path, the Rust must handle it too.
6. Run chunk test to verify:
     CHUNK={CHUNK-ID} DATA_DIR=./data cargo test --test chunk_oracle -- --nocapture
7. Run regression check:
     cargo test --workspace --exclude pob-wasm
8. Work in a branch and create a PR with title: parity({CHUNK-ID}): <what you did>

Rules:
- Do NOT use sub-agents for writing code
- Do NOT claim completion without showing chunk test output
- Do NOT skip Lua code paths because "no test exercises this"
- Do NOT leave empty fallbacks, `let _ = var;` suppressions, or TODO comments
- If you parse a value (like inc_effect), you MUST apply it where the Lua applies it
- The chunk test must pass before committing
- If you can't get all 30 passing, get as many as you can and note which builds fail and why
```

### If the session crashes mid-chunk

Start a new session with the exact same prompt. The agent will:

1. Read the reference doc (knows what to do)
2. Run the chunk test (sees current state)
3. Continue from wherever the previous session left off

The chunk test output tells the new agent exactly which fields are still wrong.

### If a chunk is too hard for one session

If the agent gets stuck or can't get all 30 builds passing, note the state:

```bash
# Save current chunk status
CHUNK={CHUNK-ID} DATA_DIR=./data cargo test --test chunk_oracle -- --nocapture 2>&1 > /tmp/chunk-status.txt
# Check how many pass
grep "builds pass" /tmp/chunk-status.txt
```

Then start a new session with a modified prompt:

```
Continue work on chunk {CHUNK-ID}. The previous session got N/30 builds passing.

THE LUA SOURCE IS THE SOURCE OF TRUTH. Port all code paths, not just the ones
that make tests pass.

Read docs/lua-reference/{CHUNK-ID}.md for context.
Open the actual Lua file(s) and re-read the full section for this chunk.
Run the chunk test to see current state:
  CHUNK={CHUNK-ID} DATA_DIR=./data cargo test --test chunk_oracle -- --nocapture

Two things to check:
1. Failing builds: look at which fields are wrong and trace back to the Lua source
2. Skipped Lua code paths: re-read the Lua section and verify EVERY branch has a
   Rust equivalent — even branches no current test exercises

Do NOT use sub-agents. Do NOT leave empty fallbacks or TODO comments.
Show chunk test output before committing.
```

### After completing a chunk

Run the full parity dashboard to see progress:

```bash
DATA_DIR=./data cargo test --test parity_report -- --nocapture 2>&1 | tail -50
```

The total percentage should have gone up. Move to the next chunk in dependency order.

---

## When You Think You're Done

Run the full oracle test suite:

```bash
DATA_DIR=./data cargo test --test oracle -- --ignored --nocapture 2>&1 | tail -40
```

If all 30 `oracle_*` tests pass, you've achieved 100% parity. If some still fail,
the failure output tells you which fields are wrong — trace them back to chunks
and fix.

Also run:

```bash
cargo test --workspace --exclude pob-wasm
```

to verify no regressions in unit tests.

---

## Troubleshooting

### "field_groups.rs needs updating"

The preliminary field groups may not cover all 880 fields. If you find output fields
that aren't assigned to any chunk, add them to the appropriate chunk in `field_groups.rs`.

### "Chunk test passes but full oracle test fails"

This means the fields assigned to the chunk are correct, but there are other fields
in the build that are wrong. This is expected — other chunks handle those fields.
Only the full oracle test checks everything.

### "Eval_mod changes broke things"

After completing the eval_mod stubs (Task 6), some mods that were incorrectly passing
through will now be correctly filtered. This may temporarily reduce parity on some fields.
This is correct behavior — it means the foundation is now right and the chunk work
will build on a solid base.

### "Reference doc seems wrong"

The annotated Lua references are generated by an agent and may have errors. If the
Lua annotations don't match what you see in the actual Lua source, trust the Lua source.
Fix the reference doc and commit the correction.

### "Chunk is too large"

If a chunk has too many fields or too much Lua to port in one session, split it.
For example, `OFF-05-ailments` could become `OFF-05a-ignite`, `OFF-05b-bleed`,
`OFF-05c-poison`. Update `field_groups.rs` accordingly.

---

## File Map

```
docs/
  PARITY-WALKTHROUGH.md                  ← YOU ARE HERE
  superpowers/specs/
    2026-04-01-chunked-parity-design.md  ← The full design spec
  superpowers/plans/
    2026-04-01-phase0-chunk-infrastructure.md  ← Phase 0 implementation plan
  lua-reference/
    LUA-GOTCHAS.md                       ← Lua→Rust translation cheat sheet
    BASELINE-PARITY.txt                  ← Starting parity numbers
    PERF-01-attributes.md                ← Per-chunk annotated Lua references
    DEF-01-resistances.md
    ...

crates/pob-calc/tests/
  oracle.rs                              ← Full oracle tests (final gate)
  field_groups.rs                        ← Chunk → field mapping
  chunk_oracle.rs                        ← Per-chunk focused tests
  parity_report.rs                       ← Parity dashboard

scripts/
  field_inventory.py                     ← Field inventory script
  field_inventory_output.json            ← Field → Lua module mapping
```
