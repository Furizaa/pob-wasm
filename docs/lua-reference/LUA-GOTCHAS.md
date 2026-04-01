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
