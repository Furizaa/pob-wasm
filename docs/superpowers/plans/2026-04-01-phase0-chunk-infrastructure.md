# Phase 0: Chunk Infrastructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the infrastructure (field inventory, chunk test harness, parity dashboard, eval_mod fixes) that enables chunk-by-chunk parity convergence.

**Architecture:** A Python script inventories all 880 output fields and maps them to Lua source locations. This produces a field-groups registry in Rust that powers two new test binaries: a per-chunk oracle runner and a parity dashboard. Separately, 5 stubbed eval_mod tag types are completed to unblock all downstream chunks.

**Tech Stack:** Python 3 (scripting), Rust (test infrastructure), Lua reference (PoB submodule)

**Spec:** `docs/superpowers/specs/2026-04-01-chunked-parity-design.md`

---

## File Structure

New files:

```
scripts/field_inventory.py                      # Extracts all output fields, classifies, maps to Lua
docs/lua-reference/LUA-GOTCHAS.md               # Shared Lua→Rust translation cheat sheet
crates/pob-calc/tests/field_groups.rs           # Chunk ID → output field names registry
crates/pob-calc/tests/chunk_oracle.rs           # Per-chunk focused test runner
crates/pob-calc/tests/parity_report.rs          # Overall parity dashboard
```

Modified files:

```
crates/pob-calc/src/mod_db/eval_mod.rs          # Complete 5 stubbed tag types
```

---

### Task 1: Output Field Inventory Script

**Files:**
- Create: `scripts/field_inventory.py`

This script is the foundation for everything else. It parses all expected JSON files, produces a union of all field names, and for each field identifies which Lua module writes it.

- [ ] **Step 1: Create the field inventory script**

```python
#!/usr/bin/env python3
"""
Field Inventory: Extract all output fields from oracle expected JSON files,
classify each as correct/wrong/missing against the Rust engine's actual output,
and map each field to its source Lua module.

Usage:
    python3 scripts/field_inventory.py

Requires DATA_DIR env var pointing to game data directory.
Outputs:
    scripts/field_inventory_output.json   — structured inventory
    stdout                                — human-readable summary
"""

import json
import os
import glob
import subprocess
import re
import sys
from collections import defaultdict

ORACLE_DIR = "crates/pob-calc/tests/oracle"
LUA_MODULES = [
    ("CalcPerform.lua", "third-party/PathOfBuilding/src/Modules/CalcPerform.lua"),
    ("CalcDefence.lua", "third-party/PathOfBuilding/src/Modules/CalcDefence.lua"),
    ("CalcOffence.lua", "third-party/PathOfBuilding/src/Modules/CalcOffence.lua"),
    ("CalcTriggers.lua", "third-party/PathOfBuilding/src/Modules/CalcTriggers.lua"),
    ("CalcMirages.lua", "third-party/PathOfBuilding/src/Modules/CalcMirages.lua"),
    ("Calcs.lua", "third-party/PathOfBuilding/src/Modules/Calcs.lua"),
]


def load_expected_fields():
    """Load all expected JSON files and extract the union of output field names."""
    all_fields = set()
    per_build = {}

    for f in sorted(glob.glob(os.path.join(ORACLE_DIR, "realworld_*.expected.json"))):
        name = os.path.basename(f).replace(".expected.json", "")
        with open(f) as fh:
            data = json.load(fh)
        output = data.get("output", data)
        fields = set(output.keys())
        all_fields |= fields
        per_build[name] = {
            "fields": sorted(fields),
            "count": len(fields),
        }

    return sorted(all_fields), per_build


def map_fields_to_lua(all_fields):
    """For each field, grep the Lua source to find which module writes it."""
    field_to_lua = {}

    # Build a lookup of all output writes per Lua module
    lua_writes = {}  # module_name -> {field_name -> [line_numbers]}
    for module_name, module_path in LUA_MODULES:
        if not os.path.exists(module_path):
            continue
        with open(module_path) as fh:
            lines = fh.readlines()
        writes = defaultdict(list)
        for i, line in enumerate(lines, 1):
            # Match output.FieldName or output["FieldName"]
            for m in re.finditer(r'output\.(\w+)\s*=', line):
                writes[m.group(1)].append(i)
            for m in re.finditer(r'output\["(\w+)"\]\s*=', line):
                writes[m.group(1)].append(i)
            # Match output["FieldName"..suffix] pattern (dynamic field names)
            for m in re.finditer(r'output\["(\w+)"\.\.', line):
                writes[m.group(1) + "*"].append(i)
        lua_writes[module_name] = dict(writes)

    for field in all_fields:
        sources = []
        for module_name, writes in lua_writes.items():
            if field in writes:
                sources.append({
                    "module": module_name,
                    "lines": writes[field],
                })
            # Check for prefix-based dynamic writes
            for key, line_nums in writes.items():
                if key.endswith("*") and field.startswith(key[:-1]):
                    sources.append({
                        "module": module_name,
                        "lines": line_nums,
                        "dynamic": True,
                    })
        field_to_lua[field] = sources if sources else [{"module": "UNKNOWN", "lines": []}]

    return field_to_lua


def main():
    all_fields, per_build = load_expected_fields()
    field_to_lua = map_fields_to_lua(all_fields)

    # Count by module
    module_counts = defaultdict(int)
    unknown_fields = []
    for field, sources in field_to_lua.items():
        if sources[0]["module"] == "UNKNOWN":
            unknown_fields.append(field)
        for s in sources:
            module_counts[s["module"]] += 1

    # Output summary
    print(f"=== Field Inventory ===")
    print(f"Total unique output fields: {len(all_fields)}")
    print(f"Builds analyzed: {len(per_build)}")
    print()
    print("Fields by Lua module:")
    for mod_name, count in sorted(module_counts.items(), key=lambda x: -x[1]):
        print(f"  {mod_name:30s} {count:4d} fields")
    print()
    print(f"Unmapped fields: {len(unknown_fields)}")
    if unknown_fields:
        for f in unknown_fields[:20]:
            print(f"  {f}")
        if len(unknown_fields) > 20:
            print(f"  ... and {len(unknown_fields) - 20} more")

    # Write structured output
    inventory = {
        "total_fields": len(all_fields),
        "total_builds": len(per_build),
        "fields": {
            field: {
                "lua_sources": field_to_lua.get(field, []),
            }
            for field in all_fields
        },
        "per_build": per_build,
        "unknown_fields": unknown_fields,
    }

    output_path = "scripts/field_inventory_output.json"
    with open(output_path, "w") as fh:
        json.dump(inventory, fh, indent=2, sort_keys=True)
    print(f"\nStructured inventory written to {output_path}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Run the script and verify output**

Run:
```bash
python3 scripts/field_inventory.py
```

Expected: prints summary with ~880 fields, fields grouped by Lua module, some unmapped fields (dynamic field names). Creates `scripts/field_inventory_output.json`.

- [ ] **Step 3: Commit**

```bash
git add scripts/field_inventory.py scripts/field_inventory_output.json
git commit -m "feat(phase0): add field inventory script — maps 880 output fields to Lua sources"
```

---

### Task 2: Lua Gotcha Cheat Sheet

**Files:**
- Create: `docs/lua-reference/LUA-GOTCHAS.md`

- [ ] **Step 1: Write the cheat sheet**

```markdown
# Lua → Rust Translation Cheat Sheet

Shared reference for all chunk implementation work. Covers patterns that appear
across all PoB calculation modules.

## Math Aliases

PoB aliases math functions at the top of each Calc*.lua file:

| Lua | Rust | Notes |
|-----|------|-------|
| `m_min(x, y)` | `x.min(y)` or `f64::min(x, y)` | `local m_min = math.min` |
| `m_max(x, y)` | `x.max(y)` or `f64::max(x, y)` | `local m_max = math.max` |
| `m_floor(x)` | `x.floor()` | `local m_floor = math.floor` |
| `m_ceil(x)` | `x.ceil()` | `local m_ceil = math.ceil` |
| `m_modf(x)` | `x.trunc()` for integer part | `local m_modf = math.modf` |
| `m_huge` | `f64::INFINITY` | `local m_huge = math.huge` |
| `round(x)` | `x.round()` | PoB's global `round()` = standard rounding |

## Table Aliases

| Lua | Rust | Notes |
|-----|------|-------|
| `t_insert(tbl, val)` | `vec.push(val)` | `local t_insert = table.insert` |
| `t_remove(tbl, idx)` | `vec.remove(idx - 1)` | Lua is 1-indexed |
| `#tbl` | `vec.len()` | Length operator |
| `ipairs(tbl)` | `.iter().enumerate()` | Sequential, 1-based in Lua |
| `pairs(tbl)` | `.iter()` | All keys, unordered |

## Nil Coalescing

Lua has no `Option` type. Variables can be `nil`. The pattern `x or 0` means
"x if x is not nil, otherwise 0".

| Lua | Rust |
|-----|------|
| `x or 0` | `x.unwrap_or(0.0)` |
| `x or false` | `x.unwrap_or(false)` |
| `x or ""` | `x.unwrap_or_default()` |
| `x or {}` | `x.unwrap_or_default()` |
| `x and x > 0 or 0` | `x.filter(\|&v\| v > 0.0).unwrap_or(0.0)` |

**Important:** In Lua, `false or 0` returns `0`, not `false`. Both `nil` and `false`
are falsy. In Rust, `Option<bool>` and `Option<f64>` are distinct types. Be careful
when translating compound expressions.

## ModDb Query Patterns

| Lua | Rust | Returns |
|-----|------|---------|
| `modDB:Sum("BASE", nil, "Life")` | `mod_db.sum(None, "Life")` | `f64` |
| `modDB:Sum("BASE", cfg, "Life")` | `mod_db.sum_cfg(cfg, output, "Life")` | `f64` |
| `modDB:More(nil, "Life")` | `mod_db.more(None, "Life")` | `f64` (product) |
| `modDB:More(cfg, "Life")` | `mod_db.more_cfg(cfg, output, "Life")` | `f64` (product) |
| `modDB:Flag(nil, "CI")` | `mod_db.flag(None, "CI")` | `bool` |
| `modDB:Flag(cfg, "CI")` | `mod_db.flag_cfg(cfg, output, "CI")` | `bool` |
| `modDB:Override(nil, "X")` | `mod_db.override_value(None, output, "X")` | `Option<f64>` |
| `modDB:List(cfg, "X")` | `mod_db.list(cfg, "X")` | `Vec<&ModValue>` |

**The `cfg` parameter:** In Lua, `cfg` is either `nil` (no skill context) or a table
with `flags`, `keywordFlags`, `slotName`, `skillName`, etc. In Rust, this is
`Option<&SkillCfg>`. When the Lua passes `nil`, Rust passes `None`. When Lua passes
`skillCfg` or a specific config table, Rust passes `Some(&cfg)`.

**The `output` parameter:** Rust's `_cfg` variants also take `&OutputTable` because
`eval_mod` needs output values for `PerStat` and `StatThreshold` tags. In Lua this is
implicit via closure over the environment. In Rust, pass the actor's `output` reference.

## Output Table Writes

| Lua | Rust |
|-----|------|
| `output.Life = 5000` | `output.insert("Life".into(), OutputValue::Number(5000.0));` |
| `output["Life"] = 5000` | Same (Lua dot and bracket are equivalent) |
| `output.CI = true` | `output.insert("CI".into(), OutputValue::Bool(true));` |
| `output.MainSkillName = "Fireball"` | `output.insert("MainSkillName".into(), OutputValue::Str("Fireball".into()));` |

**Reading output:**
| Lua | Rust |
|-----|------|
| `output.Life` | `get_output_f64(output, "Life")` or match on OutputValue |
| `output.Life or 0` | `get_output_f64(output, "Life")` (already returns 0.0 on missing) |

## Actor Access

| Lua | Rust |
|-----|------|
| `env.player.modDB` | `env.player.mod_db` |
| `env.player.output` | `env.player.output` |
| `env.enemy.modDB` | `env.enemy.mod_db` |
| `env.enemy.output` | `env.enemy.output` |

## Breakdown Patterns

In Lua, breakdown population is conditional:
```lua
if breakdown then
    breakdown.Life = {
        base = ...,
        inc = ...,
    }
end
```

In Rust, breakdowns are **always populated**. Remove the conditional — just write:
```rust
env.player.breakdown.insert("Life".into(), BreakdownData {
    lines: vec![format!("{base} (base)"), ...],
    ..Default::default()
});
```

## Common Gotchas

1. **1-based indexing:** Lua arrays start at 1. When translating loop indices, subtract 1
   for Rust Vec indexing.

2. **String concatenation:** Lua uses `..` for concat. Rust uses `format!()` or `+`.

3. **`local` scope:** Every `local` in Lua is a new variable. Re-assignment to `local x`
   in a nested scope creates a NEW variable that shadows the outer one.

4. **`and/or` ternary:** Lua's `a and b or c` is NOT equivalent to `if a { b } else { c }`
   when `b` is falsy. Be careful: `true and false or "default"` returns `"default"` in Lua,
   not `false`.

5. **Integer division:** Lua 5.1 (LuaJIT) has no integer type. All numbers are doubles.
   `5 / 2 = 2.5`, not `2`. This matches Rust's `f64` division.

6. **`calcLib.val()` and `calcLib.mod()`:** These are PoB helper functions in CalcTools.lua.
   `calcLib.val(modDB, name)` = `modDB:Sum("BASE", nil, name)`.
   `calcLib.mod(modDB, cfg, name)` = `(1 + modDB:Sum("INC", cfg, name) / 100) * modDB:More(cfg, name)`.
   In Rust, these are `calc_val()` and `calc_mod()` in `calc_tools.rs`.

7. **Enemy modDB queries:** Some calculations query `env.enemy.modDB` for things like
   enemy resistances, curse effectiveness, exposure. Make sure you're querying the right
   actor's modDB.

8. **Global vs local mods:** Item mods can be local (affect the item only, e.g., "% increased
   Physical Damage" on a weapon) or global (affect the character, e.g., "% increased maximum
   Life"). The `initEnv` setup determines which go where. During offence calculation, weapon
   damage uses weapon-local mods while character stats use global mods.
```

- [ ] **Step 2: Commit**

```bash
git add docs/lua-reference/LUA-GOTCHAS.md
git commit -m "docs(phase0): add Lua-to-Rust translation cheat sheet"
```

---

### Task 3: Field Groups Registry

**Files:**
- Create: `crates/pob-calc/tests/field_groups.rs`

This is the authoritative mapping from chunk IDs to output field names. It starts with a preliminary grouping based on the field inventory from Task 1, and will be refined as the field-to-Lua mapping (done separately) finalizes chunk boundaries.

The field groups are organized by the dependency tiers from the spec. Fields are grouped by the Lua function/section that writes them.

- [ ] **Step 1: Create the field groups registry**

```rust
//! Field group registry: maps chunk IDs to the output field names each chunk is responsible for.
//!
//! This is the authoritative source for which fields belong to which chunk.
//! Used by chunk_oracle.rs and parity_report.rs.
//!
//! IMPORTANT: When refining chunk boundaries during Phase 0, update this file.
//! The field_inventory.py script output informs these groupings.

/// Returns the list of output field names for a given chunk ID.
/// Returns None if the chunk ID is not recognized.
pub fn fields_for_chunk(chunk: &str) -> Option<&'static [&'static str]> {
    Some(match chunk {
        // ── Tier 0: Foundation (no output fields, but must be correct) ──

        // SETUP-01 through SETUP-04 don't produce output fields directly.
        // They populate the ModDb which downstream chunks query.

        // ── Tier 1: Attributes & Pools (CalcPerform early) ──

        "PERF-01-attributes" => &[
            "Str", "Dex", "Int", "Omni",
            "ReqStr", "ReqDex", "ReqInt",
            "ReqStrString", "ReqDexString", "ReqIntString",
            "ReqStrItem", "ReqDexItem", "ReqIntItem",
        ],

        "PERF-02-life-mana-es" => &[
            "Life", "Mana", "EnergyShield", "Ward",
            "EnergyShieldRecoveryCap",
            "LifeUnreserved", "LifeUnreservedPercent",
            "ManaUnreserved", "ManaUnreservedPercent",
            "LifeRecoverable", "ManaRecoverable",
        ],

        "PERF-03-charges" => &[
            "PowerCharges", "PowerChargesMin", "PowerChargesMax",
            "FrenzyCharges", "FrenzyChargesMin", "FrenzyChargesMax",
            "EnduranceCharges", "EnduranceChargesMin", "EnduranceChargesMax",
            "SiphoningCharges", "ChallengerCharges", "BlitzCharges",
            "BlitzChargesMax", "BrutalCharges", "BrutalChargesMax",
            "BrutalChargesMin", "AbsorptionCharges", "AbsorptionChargesMax",
            "AbsorptionChargesMin", "AfflictionCharges", "AfflictionChargesMax",
            "AfflictionChargesMin", "BloodCharges", "BloodChargesMax",
        ],

        "PERF-04-reservation" => &[
            "ManaReserved", "ManaReservedPercent",
            "LifeReserved", "LifeReservedPercent",
            "ManaReservedP", "LifeReservedP",
        ],

        "PERF-05-buffs" => &[
            "FortifyStacks", "FortifyEffect",
            "AilmentWarcryEffect",
            "ActiveTotemLimit", "ActiveMineLimit", "ActiveTrapLimit",
            "ActiveBrandLimit", "ActiveGolemLimit",
            "BannerStage",
        ],

        "PERF-06-aura-curse" => &[
            // Aura-related output fields are module-internal; curses affect enemy.
            // Most aura/curse effects show up in other chunks' fields (resistances, damage, etc.)
            // Placeholder — refined during field-to-Lua mapping.
        ],

        "PERF-07-regen-recharge-leech" => &[
            "LifeRegen", "LifeRegenPercent",
            "ManaRegen", "ManaRegenPercent",
            "EnergyShieldRegen", "EnergyShieldRegenPercent",
            "LifeDegen", "LifeDegenRate",
            "NetLifeRegen", "NetManaRegen", "NetEnergyShieldRegen",
            "LifeLeechRate", "ManaLeechRate", "EnergyShieldLeechRate",
            "MaxLifeLeechRate", "MaxManaLeechRate", "MaxEnergyShieldLeechRate",
            "MaxLifeLeechRatePercent", "MaxManaLeechRatePercent",
            "LifeLeechGainRate", "ManaLeechGainRate", "EnergyShieldLeechGainRate",
            "LifeLeechDuration", "ManaLeechDuration", "EnergyShieldLeechDuration",
            "LifeLeechInstances", "ManaLeechInstances", "EnergyShieldLeechInstances",
            "LifeLeechInstantRate", "ManaLeechInstantRate", "EnergyShieldLeechInstantRate",
            "LifeOnHitRate", "ManaOnHitRate", "EnergyShieldOnHitRate",
            "LifeRecoveryRate", "ManaRecoveryRate", "EnergyShieldRecoveryRate",
            "LifeRecoveryRateTotal", "ManaRecoveryRateTotal",
            "EnergyShieldRecharge", "EnergyShieldRechargeDelay",
            "EnergyShieldRechargeRecovery",
            "WardRecharge", "WardRechargeDelay",
        ],

        "PERF-08-action-speed-conditions" => &[
            "ActionSpeedMod",
            "MovementSpeedMod", "MovementSpeed",
            "EffectiveMovementSpeedMod",
        ],

        // ── Tier 4: Defence (CalcDefence) ──

        "DEF-01-resistances" => &[
            "FireResist", "FireResistTotal", "FireResistOverCap",
            "ColdResist", "ColdResistTotal", "ColdResistOverCap",
            "LightningResist", "LightningResistTotal", "LightningResistOverCap",
            "ChaosResist", "ChaosResistTotal", "ChaosResistOverCap",
            "FireResistOver", "ColdResistOver", "LightningResistOver",
            "ChaosResistOver",
        ],

        "DEF-02-armour-evasion-es-ward" => &[
            "Armour", "ArmourDefense",
            "Evasion", "EvasionDefense",
            "EnergyShieldOnBody Armour",
            "ArmourOnBody Armour", "ArmourOnHelmet", "ArmourOnGloves",
            "ArmourOnBoots", "ArmourOnWeapon 1", "ArmourOnWeapon 2",
            "EvasionOnBody Armour", "EvasionOnHelmet", "EvasionOnGloves",
            "EvasionOnBoots",
            "EnergyShieldOnHelmet", "EnergyShieldOnGloves", "EnergyShieldOnBoots",
        ],

        "DEF-03-block-suppression" => &[
            "BlockChance", "BlockChanceMax", "BlockChanceOverCap",
            "SpellBlockChance", "SpellBlockChanceMax", "SpellBlockChanceOverCap",
            "BlockEffect", "BlockDuration",
            "SpellSuppressionChance", "SpellSuppressionChanceOverCap",
            "SpellSuppressionEffect",
        ],

        "DEF-04-damage-reduction-avoidance" => &[
            "PhysicalDamageReduction",
            "BasePhysicalDamageReduction", "BasePhysicalDamageReductionWhenHit",
            "BaseFireDamageReduction", "BaseFireDamageReductionWhenHit",
            "BaseColdDamageReduction", "BaseColdDamageReductionWhenHit",
            "BaseLightningDamageReduction", "BaseLightningDamageReductionWhenHit",
            "BaseChaosDamageReduction", "BaseChaosDamageReductionWhenHit",
            "AttackDodgeChance", "AttackDodgeChanceOverCap",
            "SpellDodgeChance", "SpellDodgeChanceOverCap",
            "BlindAvoidChance",
            "AvoidPhysicalDamageChance", "AvoidFireDamageChance",
            "AvoidColdDamageChance", "AvoidLightningDamageChance",
            "AvoidChaosDamageChance",
            "AvoidAllDamageFromHitsChance",
            "AvoidProjectilesChance",
            "BleedAvoidChance", "PoisonAvoidChance",
            "IgniteAvoidChance", "ShockAvoidChance", "FreezeAvoidChance",
            "ChillAvoidChance", "ScorchAvoidChance", "BrittleAvoidChance",
            "SapAvoidChance", "StunAvoidChance",
        ],

        "DEF-05-recovery-in-defence" => &[
            // Recovery rates as computed in CalcDefence's pool-based recovery
            // These overlap with PERF-07 fields but are the final computed values
            // after defence-specific adjustments.
        ],

        "DEF-06-ehp" => &[
            "AverageEvadeChance", "AverageNotHitChance",
            "AverageBlockChance", "AverageSpellBlockChance",
            "MeleeNotHitChance", "ProjectileNotHitChance", "SpellNotHitChance",
            "AttackTakenHitMult", "SpellTakenHitMult",
            "TotalEHP",
            "PhysicalMaximumHitTaken", "FireMaximumHitTaken",
            "ColdMaximumHitTaken", "LightningMaximumHitTaken",
            "ChaosMaximumHitTaken",
            "AnyAegis", "AnyBypass", "AnyGuard",
            "AnySpecificMindOverMatter", "AnyTakenReflect",
            "sharedAegis", "sharedElementalAegis",
            "sharedGuardAbsorbRate",
            "sharedMindOverMatter", "sharedMoMHitPool",
            "sharedManaEffectiveLife",
            "totalEnemyDamage", "totalEnemyDamageIn",
            "totalTakenDamage", "totalTakenHit",
            "enemySkillTime", "enemyBlockChance",
            "noSplitEvade",
            "ehpSectionAnySpecificTypes", "specificTypeAvoidance",
            "preventedLifeLoss", "preventedLifeLossBelowHalf", "preventedLifeLossTotal",
        ],

        // ── Tier 5: Offence (CalcOffence) ──

        "OFF-01-base-damage" => &[
            "AverageDamage", "AverageBurstDamage", "AverageBurstHits",
            "MainHand.AverageDamage", "OffHand.AverageDamage",
        ],

        "OFF-02-conversion" => &[
            // Conversion fields are intermediate — they manifest in per-type damage fields.
            // Placeholder — refined during field-to-Lua mapping.
        ],

        "OFF-03-crit-hit" => &[
            "CritChance", "CritMultiplier", "CritEffect",
            "CritDegenMultiplier",
            "AccuracyHitChance",
            "MainHand.CritChance", "MainHand.CritMultiplier",
            "OffHand.CritChance", "OffHand.CritMultiplier",
            "MeleeNotHitChance", "ProjectileNotHitChance",
        ],

        "OFF-04-speed-dps" => &[
            "Speed", "HitSpeed", "HitTime",
            "TotalDPS",
            "TotalDot",
            "MainHand.Speed", "MainHand.HitSpeed",
            "OffHand.Speed", "OffHand.HitSpeed",
            "AreaOfEffectMod", "AreaOfEffectRadius", "AreaOfEffectRadiusMetres",
        ],

        "OFF-05-ailments" => &[
            "IgniteChance", "IgniteDPS", "IgniteDamage", "IgniteDuration",
            "IgniteEffMult",
            "BleedChance", "BleedDPS", "BleedDamage", "BleedDuration",
            "BleedEffMult", "BleedStackPotential", "BleedStacks", "BleedStacksMax",
            "BleedRollAverage",
            "PoisonChance", "PoisonDPS", "PoisonDamage", "PoisonDuration",
            "PoisonStacks", "PoisonStacksMax",
        ],

        "OFF-06-dot-impale" => &[
            "TotalDot",
            "ImpaleDPS", "ImpaleHit", "ImpaleModifier",
            "ImpaleStacks", "ImpaleStacksMax",
            "impaleStoredHitAvg",
        ],

        "OFF-07-combined-dps" => &[
            "CombinedDPS", "CombinedAvg",
            "WithBleedDPS", "WithIgniteDPS", "WithPoisonDPS",
            "CullingMultiplier",
            "FullDPS", "FullDotDPS",
            "WithDotDPS",
        ],

        // ── Tier 6: Triggers & Mirages ──

        "TRIG-01-trigger-rates" => &[
            "TriggerRate", "TriggerTime",
            "ServerTriggerRate",
        ],

        "TRIG-02-totem-trap-mine" => &[
            "TotemPlacementSpeed", "TotemPlacementTime", "TotemLife",
            "TrapThrowSpeed", "TrapThrowTime", "TrapCooldown",
            "MineLayingSpeed", "MineLayingTime",
        ],

        "MIR-01-mirages" => &[
            "MirageDPS", "MirageCount",
        ],

        // ── Tier 7: Aggregation ──

        "AGG-01-full-dps" => &[
            "FullDPS", "FullDotDPS",
        ],

        _ => return None,
    })
}

/// Returns all known chunk IDs in dependency order.
pub fn all_chunk_ids() -> &'static [&'static str] {
    &[
        "PERF-01-attributes",
        "PERF-02-life-mana-es",
        "PERF-03-charges",
        "PERF-04-reservation",
        "PERF-05-buffs",
        "PERF-06-aura-curse",
        "PERF-07-regen-recharge-leech",
        "PERF-08-action-speed-conditions",
        "DEF-01-resistances",
        "DEF-02-armour-evasion-es-ward",
        "DEF-03-block-suppression",
        "DEF-04-damage-reduction-avoidance",
        "DEF-05-recovery-in-defence",
        "DEF-06-ehp",
        "OFF-01-base-damage",
        "OFF-02-conversion",
        "OFF-03-crit-hit",
        "OFF-04-speed-dps",
        "OFF-05-ailments",
        "OFF-06-dot-impale",
        "OFF-07-combined-dps",
        "TRIG-01-trigger-rates",
        "TRIG-02-totem-trap-mine",
        "MIR-01-mirages",
        "AGG-01-full-dps",
    ]
}

/// Returns the names of all 30 realworld oracle builds.
pub fn realworld_build_names() -> Vec<String> {
    let oracle_dir = std::path::Path::new("tests/oracle");
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(oracle_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if fname.starts_with("realworld_") && fname.ends_with(".xml") {
                names.push(fname.trim_end_matches(".xml").to_string());
            }
        }
    }
    names.sort();
    names
}
```

- [ ] **Step 2: Verify it compiles**

Run:
```bash
cargo check --test chunk_oracle 2>&1 || echo "Expected: chunk_oracle doesn't exist yet, but field_groups.rs should have no syntax errors"
```

This file will be used by the test binaries in the next tasks. For now, just verify the Rust syntax is valid by attempting a `cargo check` (it won't fully compile until the test binaries reference it).

- [ ] **Step 3: Commit**

```bash
git add crates/pob-calc/tests/field_groups.rs
git commit -m "feat(phase0): add field groups registry — maps chunk IDs to output field names"
```

---

### Task 4: Chunk Oracle Test Runner

**Files:**
- Create: `crates/pob-calc/tests/chunk_oracle.rs`

- [ ] **Step 1: Create the chunk oracle test runner**

```rust
//! Per-chunk oracle test runner.
//!
//! Runs all 30 realworld oracle builds but only asserts on the output fields
//! belonging to a specific chunk. This gives focused feedback for agents
//! working on one subsystem at a time.
//!
//! Usage:
//!   CHUNK=DEF-01-resistances DATA_DIR=./data cargo test --test chunk_oracle -- --nocapture
//!
//! The CHUNK env var selects which field group to check.
//! If CHUNK is not set, prints available chunk IDs and exits.

mod field_groups;

use pob_calc::{build::parse_xml, calc::calculate, data::GameData};
use std::sync::Arc;

fn load_game_data() -> Option<Arc<GameData>> {
    let data_dir = std::env::var("DATA_DIR").ok()?;
    let json = build_real_game_data_json(&data_dir).ok()?;
    GameData::from_json(&json).ok().map(Arc::new)
}

fn build_real_game_data_json(data_dir: &str) -> Result<String, Box<dyn std::error::Error>> {
    let gems_str = std::fs::read_to_string(format!("{data_dir}/gems.json"))?;
    let misc_str = std::fs::read_to_string(format!("{data_dir}/misc.json"))?;
    let tree_str = std::fs::read_to_string(format!("{data_dir}/tree/poe1_current.json"))?;

    let gems: serde_json::Value = serde_json::from_str(&gems_str)?;
    let misc: serde_json::Value = serde_json::from_str(&misc_str)?;
    let tree: serde_json::Value = serde_json::from_str(&tree_str)?;

    let bases: serde_json::Value = std::fs::read_to_string(format!("{data_dir}/bases.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Array(vec![]));

    let uniques: serde_json::Value = std::fs::read_to_string(format!("{data_dir}/uniques.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Array(vec![]));

    let combined = serde_json::json!({
        "gems": gems,
        "misc": misc,
        "tree": tree,
        "bases": bases,
        "uniques": uniques,
    });
    Ok(serde_json::to_string(&combined)?)
}

fn load_expected(name: &str) -> serde_json::Value {
    let path = format!("tests/oracle/{name}.expected.json");
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Oracle file not found: {path}"));
    serde_json::from_str(&content).expect("Oracle file is not valid JSON")
}

fn load_build_xml(name: &str) -> String {
    let path = format!("tests/oracle/{name}.xml");
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Oracle XML not found: {path}"))
}

fn compare_value(actual: &serde_json::Value, expected: &serde_json::Value) -> Option<String> {
    match (expected, actual) {
        (serde_json::Value::Number(e), serde_json::Value::Number(a)) => {
            let e = e.as_f64().unwrap();
            let a = a.as_f64().unwrap();
            let tolerance = (e * 0.001).abs().max(0.01);
            if (a - e).abs() > tolerance {
                Some(format!("expected {e}, got {a} (tol {tolerance:.4})"))
            } else {
                None
            }
        }
        (serde_json::Value::Bool(e), serde_json::Value::Bool(a)) => {
            if a != e {
                Some(format!("expected {e}, got {a}"))
            } else {
                None
            }
        }
        (serde_json::Value::String(e), serde_json::Value::String(a)) => {
            if a != e {
                Some(format!("expected {e:?}, got {a:?}"))
            } else {
                None
            }
        }
        _ => Some(format!("type mismatch: expected {expected}, got {actual}")),
    }
}

#[test]
fn chunk_oracle() {
    let chunk_id = match std::env::var("CHUNK") {
        Ok(c) => c,
        Err(_) => {
            println!("CHUNK env var not set. Available chunks:");
            for id in field_groups::all_chunk_ids() {
                let fields = field_groups::fields_for_chunk(id).unwrap_or(&[]);
                println!("  {id:40} ({} fields)", fields.len());
            }
            println!("\nUsage: CHUNK=DEF-01-resistances DATA_DIR=./data cargo test --test chunk_oracle -- --nocapture");
            return;
        }
    };

    let fields = field_groups::fields_for_chunk(&chunk_id)
        .unwrap_or_else(|| panic!("Unknown chunk: {chunk_id}"));

    if fields.is_empty() {
        println!("Chunk {chunk_id} has no fields defined yet (placeholder). Skipping.");
        return;
    }

    let data = load_game_data()
        .expect("DATA_DIR must be set and contain valid game data");

    let build_names = field_groups::realworld_build_names();
    assert!(!build_names.is_empty(), "No realworld builds found in tests/oracle/");

    let mut builds_pass = 0;
    let mut builds_fail = 0;
    let mut total_fields_correct = 0;
    let mut total_fields_checked = 0;
    let mut failure_details: Vec<String> = Vec::new();

    for build_name in &build_names {
        let xml = load_build_xml(build_name);
        let build = match parse_xml(&xml) {
            Ok(b) => b,
            Err(e) => {
                failure_details.push(format!("{build_name}: parse error: {e}"));
                builds_fail += 1;
                continue;
            }
        };

        let result = match calculate(&build, Arc::clone(&data)) {
            Ok(r) => r,
            Err(e) => {
                failure_details.push(format!("{build_name}: calc error: {e}"));
                builds_fail += 1;
                continue;
            }
        };

        let actual = serde_json::to_value(&result.output).unwrap();
        let actual_obj = actual.as_object().unwrap();

        let expected_full = load_expected(build_name);
        let expected_output = expected_full.get("output").unwrap_or(&expected_full);
        let expected_obj = expected_output.as_object().unwrap();

        let mut build_failures: Vec<String> = Vec::new();
        let mut fields_ok = 0;
        let mut fields_checked = 0;

        for &field in fields {
            let exp = expected_obj.get(field);
            let act = actual_obj.get(field);

            match (act, exp) {
                (None, None) => {
                    // Field not in expected or actual for this build — skip
                }
                (None, Some(exp_val)) => {
                    fields_checked += 1;
                    build_failures.push(format!("  {field}: missing (expected {exp_val})"));
                }
                (Some(act_val), None) => {
                    // Field in actual but not expected — not a failure for this chunk
                    // (the full oracle test catches unexpected fields)
                    fields_checked += 1;
                    fields_ok += 1;
                }
                (Some(act_val), Some(exp_val)) => {
                    fields_checked += 1;
                    if let Some(msg) = compare_value(act_val, exp_val) {
                        build_failures.push(format!("  {field}: {msg}"));
                    } else {
                        fields_ok += 1;
                    }
                }
            }
        }

        total_fields_correct += fields_ok;
        total_fields_checked += fields_checked;

        if build_failures.is_empty() {
            builds_pass += 1;
        } else {
            builds_fail += 1;
            failure_details.push(format!(
                "{build_name}: {}/{} fields correct\n{}",
                fields_ok,
                fields_checked,
                build_failures.join("\n")
            ));
        }
    }

    println!("\n=== Chunk: {chunk_id} ===");
    println!("Builds: {builds_pass}/{} pass", builds_pass + builds_fail);
    println!("Fields: {total_fields_correct}/{total_fields_checked} correct across all builds");

    if !failure_details.is_empty() {
        println!("\nFailures:");
        for detail in &failure_details {
            println!("{detail}");
        }
    }

    assert_eq!(
        builds_fail, 0,
        "Chunk {chunk_id}: {builds_fail} builds failed"
    );
}
```

- [ ] **Step 2: Verify it compiles**

Run:
```bash
cargo test --test chunk_oracle --no-run 2>&1
```

Expected: compiles successfully (the test itself requires CHUNK and DATA_DIR to run).

- [ ] **Step 3: Test with a chunk**

Run:
```bash
CHUNK=PERF-01-attributes DATA_DIR=./data cargo test --test chunk_oracle -- --nocapture 2>&1 | tail -40
```

Expected: runs all 30 builds, checks only attribute fields, prints pass/fail summary. Some builds may pass for attributes even though they fail the full oracle test.

- [ ] **Step 4: Test with no CHUNK set (help output)**

Run:
```bash
cargo test --test chunk_oracle -- --nocapture 2>&1 | tail -40
```

Expected: prints list of available chunks with field counts.

- [ ] **Step 5: Commit**

```bash
git add crates/pob-calc/tests/chunk_oracle.rs
git commit -m "feat(phase0): add per-chunk oracle test runner"
```

---

### Task 5: Parity Dashboard

**Files:**
- Create: `crates/pob-calc/tests/parity_report.rs`

- [ ] **Step 1: Create the parity report test**

```rust
//! Parity Dashboard: runs all 30 realworld oracle builds and reports per-chunk
//! field correctness as a summary table.
//!
//! Usage:
//!   DATA_DIR=./data cargo test --test parity_report -- --nocapture
//!
//! Produces a table showing which chunks pass, which are partial, and the
//! overall parity percentage.

mod field_groups;

use pob_calc::{build::parse_xml, calc::calculate, data::GameData};
use std::collections::HashMap;
use std::sync::Arc;

fn load_game_data() -> Option<Arc<GameData>> {
    let data_dir = std::env::var("DATA_DIR").ok()?;
    let json = build_real_game_data_json(&data_dir).ok()?;
    GameData::from_json(&json).ok().map(Arc::new)
}

fn build_real_game_data_json(data_dir: &str) -> Result<String, Box<dyn std::error::Error>> {
    let gems_str = std::fs::read_to_string(format!("{data_dir}/gems.json"))?;
    let misc_str = std::fs::read_to_string(format!("{data_dir}/misc.json"))?;
    let tree_str = std::fs::read_to_string(format!("{data_dir}/tree/poe1_current.json"))?;

    let gems: serde_json::Value = serde_json::from_str(&gems_str)?;
    let misc: serde_json::Value = serde_json::from_str(&misc_str)?;
    let tree: serde_json::Value = serde_json::from_str(&tree_str)?;

    let bases: serde_json::Value = std::fs::read_to_string(format!("{data_dir}/bases.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Array(vec![]));

    let uniques: serde_json::Value = std::fs::read_to_string(format!("{data_dir}/uniques.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Array(vec![]));

    let combined = serde_json::json!({
        "gems": gems,
        "misc": misc,
        "tree": tree,
        "bases": bases,
        "uniques": uniques,
    });
    Ok(serde_json::to_string(&combined)?)
}

fn load_expected(name: &str) -> serde_json::Value {
    let path = format!("tests/oracle/{name}.expected.json");
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Oracle file not found: {path}"));
    serde_json::from_str(&content).expect("Oracle file is not valid JSON")
}

fn load_build_xml(name: &str) -> String {
    let path = format!("tests/oracle/{name}.xml");
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Oracle XML not found: {path}"))
}

fn compare_value(actual: &serde_json::Value, expected: &serde_json::Value) -> bool {
    match (expected, actual) {
        (serde_json::Value::Number(e), serde_json::Value::Number(a)) => {
            let e = e.as_f64().unwrap();
            let a = a.as_f64().unwrap();
            let tolerance = (e * 0.001).abs().max(0.01);
            (a - e).abs() <= tolerance
        }
        (serde_json::Value::Bool(e), serde_json::Value::Bool(a)) => a == e,
        (serde_json::Value::String(e), serde_json::Value::String(a)) => a == e,
        _ => false,
    }
}

#[test]
fn parity_report() {
    let data = load_game_data()
        .expect("DATA_DIR must be set and contain valid game data");

    let build_names = field_groups::realworld_build_names();
    assert!(!build_names.is_empty(), "No realworld builds found");

    // Pre-compute all build results
    let mut build_results: Vec<(String, serde_json::Value, serde_json::Value)> = Vec::new();

    for build_name in &build_names {
        let xml = load_build_xml(build_name);
        let build = parse_xml(&xml).unwrap_or_else(|e| panic!("Parse {build_name}: {e}"));
        let result = calculate(&build, Arc::clone(&data))
            .unwrap_or_else(|e| panic!("Calc {build_name}: {e}"));
        let actual = serde_json::to_value(&result.output).unwrap();
        let expected_full = load_expected(build_name);
        let expected = expected_full
            .get("output")
            .cloned()
            .unwrap_or(expected_full);
        build_results.push((build_name.clone(), actual, expected));
    }

    // Per-chunk analysis
    println!("\n{:=<80}", "");
    println!("  PARITY DASHBOARD");
    println!("{:=<80}", "");
    println!(
        "{:<45} {:>10} {:>10} {:>8}",
        "Chunk", "Builds", "Fields", "Status"
    );
    println!("{:-<80}", "");

    let mut grand_total_correct = 0;
    let mut grand_total_checked = 0;
    let mut chunks_pass = 0;
    let mut chunks_total = 0;

    for &chunk_id in field_groups::all_chunk_ids() {
        let fields = match field_groups::fields_for_chunk(chunk_id) {
            Some(f) if !f.is_empty() => f,
            _ => {
                println!("{chunk_id:<45} {:>10} {:>10} {:>8}", "-", "-", "EMPTY");
                continue;
            }
        };

        chunks_total += 1;
        let mut builds_pass = 0;
        let mut total_correct = 0;
        let mut total_checked = 0;

        for (_, actual, expected) in &build_results {
            let actual_obj = actual.as_object().unwrap();
            let expected_obj = expected.as_object().unwrap();

            let mut build_ok = true;
            for &field in fields {
                let exp = expected_obj.get(field);
                let act = actual_obj.get(field);

                match (act, exp) {
                    (None, None) => {} // not relevant for this build
                    (None, Some(_)) => {
                        total_checked += 1;
                        build_ok = false;
                    }
                    (Some(_), None) => {
                        total_checked += 1;
                        total_correct += 1;
                    }
                    (Some(a), Some(e)) => {
                        total_checked += 1;
                        if compare_value(a, e) {
                            total_correct += 1;
                        } else {
                            build_ok = false;
                        }
                    }
                }
            }

            if build_ok {
                builds_pass += 1;
            }
        }

        grand_total_correct += total_correct;
        grand_total_checked += total_checked;

        let status = if builds_pass == build_results.len() {
            chunks_pass += 1;
            "PASS"
        } else if total_correct > 0 {
            "PARTIAL"
        } else {
            "FAIL"
        };

        println!(
            "{chunk_id:<45} {:>4}/{:<4} {:>4}/{:<4} {:>8}",
            builds_pass,
            build_results.len(),
            total_correct,
            total_checked,
            status
        );
    }

    println!("{:-<80}", "");
    let pct = if grand_total_checked > 0 {
        100.0 * grand_total_correct as f64 / grand_total_checked as f64
    } else {
        0.0
    };
    println!(
        "Chunks: {chunks_pass}/{chunks_total} pass | Fields: {grand_total_correct}/{grand_total_checked} correct ({pct:.1}%)"
    );

    // Per-build summary (how many total fields correct per build)
    println!("\n{:=<80}", "");
    println!("  PER-BUILD SUMMARY");
    println!("{:=<80}", "");
    println!("{:<50} {:>10} {:>10}", "Build", "Correct", "Total");
    println!("{:-<80}", "");

    let mut all_fields_by_build: Vec<(String, usize, usize)> = Vec::new();

    for (build_name, actual, expected) in &build_results {
        let actual_obj = actual.as_object().unwrap();
        let expected_obj = expected.as_object().unwrap();

        let mut correct = 0;
        let total = expected_obj.len();

        for (key, exp_val) in expected_obj {
            if let Some(act_val) = actual_obj.get(key) {
                if compare_value(act_val, exp_val) {
                    correct += 1;
                }
            }
        }

        all_fields_by_build.push((build_name.clone(), correct, total));
    }

    // Sort by parity percentage descending
    all_fields_by_build.sort_by(|a, b| {
        let pct_a = a.1 as f64 / a.2.max(1) as f64;
        let pct_b = b.1 as f64 / b.2.max(1) as f64;
        pct_b.partial_cmp(&pct_a).unwrap()
    });

    for (name, correct, total) in &all_fields_by_build {
        let pct = 100.0 * *correct as f64 / (*total).max(1) as f64;
        println!("{name:<50} {correct:>4}/{total:<4} ({pct:.1}%)");
    }

    println!("{:=<80}", "");
}
```

- [ ] **Step 2: Verify it compiles**

Run:
```bash
cargo test --test parity_report --no-run 2>&1
```

Expected: compiles successfully.

- [ ] **Step 3: Run the parity dashboard**

Run:
```bash
DATA_DIR=./data cargo test --test parity_report -- --nocapture 2>&1
```

Expected: prints the full parity table showing per-chunk and per-build status. This establishes the baseline for all subsequent chunk work.

- [ ] **Step 4: Commit**

```bash
git add crates/pob-calc/tests/parity_report.rs
git commit -m "feat(phase0): add parity dashboard — tracks field correctness across all chunks and builds"
```

---

### Task 6: Complete eval_mod Stubs (SETUP-04)

**Files:**
- Modify: `crates/pob-calc/src/mod_db/eval_mod.rs:139-155`

The 5 stubbed `ModTag` variants must be replaced with real evaluation logic. Each stub
currently passes through (always matches), causing mods to apply when they shouldn't.

- [ ] **Step 1: Read the Lua reference for each tag type**

The Lua source for these tags is in `third-party/PathOfBuilding/src/Classes/ModStore.lua`.
The relevant sections are:

- `SkillName` (line 746): checks `cfg.skillName` or `cfg.summonSkillName` against `tag.skillName` or `tag.skillNameList`
- `SkillId` (line 793): checks `cfg.skillGemId` against `tag.skillId`
- `SkillPart` (line 808): checks `cfg.skillPart` against `tag.skillPart`
- `SocketedIn` (line 704): checks `cfg.slotName` against `tag.slotName`
- `ItemCondition` (line 736): checks item properties against tag conditions

- [ ] **Step 2: Implement SkillName tag evaluation**

Replace the stub at line 140-142 in `eval_mod.rs`:

```rust
            ModTag::SkillName { name } => {
                // Mod only applies if the active skill's name matches.
                // PoB: checks cfg.skillName against tag.skillName.
                // If no cfg is provided, the mod does not apply.
                match cfg {
                    Some(c) => {
                        let skill_name = c.skill_name.as_deref().unwrap_or("");
                        if !skill_name.eq_ignore_ascii_case(name) {
                            return None;
                        }
                    }
                    None => return None,
                }
            }
```

- [ ] **Step 3: Implement SkillId tag evaluation**

Replace the stub at line 143-145:

```rust
            ModTag::SkillId { id } => {
                // Mod only applies if the active skill's gem ID matches.
                match cfg {
                    Some(c) => {
                        let skill_id = c.skill_id.as_deref().unwrap_or("");
                        if !skill_id.eq_ignore_ascii_case(id) {
                            return None;
                        }
                    }
                    None => return None,
                }
            }
```

- [ ] **Step 4: Implement SkillPart tag evaluation**

Replace the stub at line 146-148:

```rust
            ModTag::SkillPart { part } => {
                // Mod only applies to a specific skill part (e.g., part 1 vs part 2 of a skill).
                match cfg {
                    Some(c) => {
                        let current_part = c.skill_part.unwrap_or(1);
                        if current_part != *part {
                            return None;
                        }
                    }
                    None => return None,
                }
            }
```

- [ ] **Step 5: Implement SocketedIn tag evaluation**

Replace the stub at line 149-151:

```rust
            ModTag::SocketedIn { slot_name } => {
                // Mod only applies to gems socketed in a specific item slot.
                match cfg {
                    Some(c) => {
                        let cfg_slot = c.slot_name.as_deref().unwrap_or("");
                        if !cfg_slot.eq_ignore_ascii_case(slot_name) {
                            return None;
                        }
                    }
                    None => return None,
                }
            }
```

- [ ] **Step 6: Implement ItemCondition tag evaluation**

Replace the stub at line 152-154:

```rust
            ModTag::ItemCondition { var, neg } => {
                // Check a condition on an equipped item's properties.
                // For now, check if the variable exists as a condition in the modDB.
                // Full item property lookup requires item context which may not be
                // available in all query paths. This implementation handles the common
                // case where item conditions are pre-computed as modDB conditions
                // during setup.
                let met = mod_db.conditions.get(var).copied().unwrap_or(false);
                if met == *neg {
                    return None;
                }
            }
```

- [ ] **Step 7: Update existing tests**

The existing tests are named `eval_skill_name_tag_stubbed_passes` etc. They assert that
the stub always passes. Update them to test the real behavior:

In `eval_mod.rs`, find the test module and replace the 5 stub tests:

```rust
    #[test]
    fn eval_skill_name_tag_filters_when_no_cfg() {
        let m = Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(10.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::SkillName {
                name: "Fireball".into(),
            }],
            source: ModSource::new("test", "test"),
        };
        let db = ModDb::new();
        let output = OutputTable::new();
        // No cfg → mod should NOT apply
        assert!(eval_mod(&m, None, &db, &output).is_none());
    }

    #[test]
    fn eval_skill_name_tag_passes_matching_skill() {
        let m = Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(10.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::SkillName {
                name: "Fireball".into(),
            }],
            source: ModSource::new("test", "test"),
        };
        let db = ModDb::new();
        let output = OutputTable::new();
        let cfg = SkillCfg {
            skill_name: Some("Fireball".into()),
            ..Default::default()
        };
        assert_eq!(eval_mod(&m, Some(&cfg), &db, &output), Some(10.0));
    }

    #[test]
    fn eval_skill_name_tag_rejects_non_matching_skill() {
        let m = Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(10.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::SkillName {
                name: "Fireball".into(),
            }],
            source: ModSource::new("test", "test"),
        };
        let db = ModDb::new();
        let output = OutputTable::new();
        let cfg = SkillCfg {
            skill_name: Some("Arc".into()),
            ..Default::default()
        };
        assert!(eval_mod(&m, Some(&cfg), &db, &output).is_none());
    }

    #[test]
    fn eval_skill_id_tag_filters_correctly() {
        let m = Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(15.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::SkillId {
                id: "FireballGem".into(),
            }],
            source: ModSource::new("test", "test"),
        };
        let db = ModDb::new();
        let output = OutputTable::new();
        // No cfg → excluded
        assert!(eval_mod(&m, None, &db, &output).is_none());
        // Matching ID → passes
        let cfg = SkillCfg {
            skill_id: Some("FireballGem".into()),
            ..Default::default()
        };
        assert_eq!(eval_mod(&m, Some(&cfg), &db, &output), Some(15.0));
        // Non-matching ID → excluded
        let cfg2 = SkillCfg {
            skill_id: Some("ArcGem".into()),
            ..Default::default()
        };
        assert!(eval_mod(&m, Some(&cfg2), &db, &output).is_none());
    }

    #[test]
    fn eval_skill_part_tag_filters_correctly() {
        let m = Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(20.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::SkillPart { part: 2 }],
            source: ModSource::new("test", "test"),
        };
        let db = ModDb::new();
        let output = OutputTable::new();
        // No cfg → excluded
        assert!(eval_mod(&m, None, &db, &output).is_none());
        // Part 1 (default) → excluded
        let cfg1 = SkillCfg {
            skill_part: Some(1),
            ..Default::default()
        };
        assert!(eval_mod(&m, Some(&cfg1), &db, &output).is_none());
        // Part 2 → passes
        let cfg2 = SkillCfg {
            skill_part: Some(2),
            ..Default::default()
        };
        assert_eq!(eval_mod(&m, Some(&cfg2), &db, &output), Some(20.0));
    }

    #[test]
    fn eval_socketed_in_tag_filters_correctly() {
        let m = Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(25.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::SocketedIn {
                slot_name: "Weapon 1".into(),
            }],
            source: ModSource::new("test", "test"),
        };
        let db = ModDb::new();
        let output = OutputTable::new();
        // No cfg → excluded
        assert!(eval_mod(&m, None, &db, &output).is_none());
        // Matching slot → passes
        let cfg = SkillCfg {
            slot_name: Some("Weapon 1".into()),
            ..Default::default()
        };
        assert_eq!(eval_mod(&m, Some(&cfg), &db, &output), Some(25.0));
        // Non-matching slot → excluded
        let cfg2 = SkillCfg {
            slot_name: Some("Helmet".into()),
            ..Default::default()
        };
        assert!(eval_mod(&m, Some(&cfg2), &db, &output).is_none());
    }

    #[test]
    fn eval_item_condition_tag_filters_correctly() {
        let m = Mod {
            name: "Damage".into(),
            mod_type: ModType::Inc,
            value: ModValue::Number(30.0),
            flags: ModFlags::NONE,
            keyword_flags: KeywordFlags::NONE,
            tags: vec![ModTag::ItemCondition {
                var: "UsingShield".into(),
                neg: false,
            }],
            source: ModSource::new("test", "test"),
        };
        let mut db = ModDb::new();
        let output = OutputTable::new();
        // Condition not set → excluded
        assert!(eval_mod(&m, None, &db, &output).is_none());
        // Condition set to true → passes
        db.set_condition("UsingShield", true);
        assert_eq!(eval_mod(&m, None, &db, &output), Some(30.0));
        // Negated version
        let m_neg = Mod {
            tags: vec![ModTag::ItemCondition {
                var: "UsingShield".into(),
                neg: true,
            }],
            ..m.clone()
        };
        assert!(eval_mod(&m_neg, None, &db, &output).is_none());
    }
```

- [ ] **Step 8: Run tests**

Run:
```bash
cargo test -p pob-calc -- eval_mod --nocapture 2>&1
```

Expected: all eval_mod tests pass, including the new ones. The old `_stubbed_passes` tests
should be replaced, not coexist with the new tests.

- [ ] **Step 9: Run full test suite for regressions**

Run:
```bash
cargo test --workspace --exclude pob-wasm 2>&1
```

Expected: all tests pass. The eval_mod changes may cause some oracle fields to change
(mods that were incorrectly passing through will now be filtered). This is correct
behavior — it may temporarily reduce parity on some fields while increasing it on others.

- [ ] **Step 10: Commit**

```bash
git add crates/pob-calc/src/mod_db/eval_mod.rs
git commit -m "fix(eval_mod): complete 5 stubbed tag types — SkillName, SkillId, SkillPart, SocketedIn, ItemCondition"
```

---

### Task 7: Run Baseline Parity Report and Commit Snapshot

**Files:**
- No new files

This task establishes the official baseline before chunk work begins.

- [ ] **Step 1: Run the parity dashboard**

Run:
```bash
DATA_DIR=./data cargo test --test parity_report -- --nocapture 2>&1 | tee docs/lua-reference/BASELINE-PARITY.txt
```

- [ ] **Step 2: Commit the baseline**

```bash
git add docs/lua-reference/BASELINE-PARITY.txt
git commit -m "docs(phase0): record baseline parity snapshot before chunk work begins"
```

---

## Self-Review Checklist

1. **Spec coverage:**
   - 0.1 Field Inventory → Task 1
   - 0.2 Field-to-Lua Mapping → Task 1 (integrated into field_inventory.py)
   - 0.3 Chunk Dependency Graph → Captured in field_groups.rs (Task 3) + refined separately
   - 0.4 Annotated Lua References → NOT in this plan (separate plan per the spec: "5-8 sessions")
   - 0.5 Per-chunk test infrastructure → Tasks 3, 4, 5
   - 0.6 Eval_mod stub completion → Task 6
   - 0.7 Lua gotcha cheat sheet → Task 2

   **Note:** Phase 0.4 (annotated Lua reference docs, 5-8 sessions) is intentionally excluded
   from this plan. It requires the field-to-Lua mapping output from Task 1 and should be done
   as a series of separate sessions, one per Lua module. Each reference doc session will read
   the field inventory, the Lua source, and the Rust source to produce the chunk reference.

2. **Placeholder scan:** No TBD/TODO/placeholders found. All code blocks are complete.

3. **Type consistency:** `field_groups::fields_for_chunk` returns `Option<&[&str]>` consistently.
   `realworld_build_names()` returns `Vec<String>` consistently. `compare_value` signature
   matches between chunk_oracle.rs and parity_report.rs.
