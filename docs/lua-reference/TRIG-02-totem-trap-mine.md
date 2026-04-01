# TRIG-02-totem-trap-mine: Totem Placement, Trap Throwing, and Mine Laying

## Output Fields

> **Two `field_groups.rs` entries use wrong names — see notes.**

| `field_groups.rs` name | Actual oracle field | Lua source | Notes |
|------------------------|---------------------|-----------|-------|
| `TotemPlacementSpeed` | `TotemPlacementSpeed` | CalcOffence.lua:1370 | Correct |
| `TotemPlacementTime` | `TotemPlacementTime` | CalcOffence.lua:1371 | Correct |
| `TotemLife` | `TotemLife` | CalcOffence.lua:1391 | Correct |
| `TrapThrowSpeed` | **`TrapThrowingSpeed`** | CalcOffence.lua:1213,1219 | Wrong name — see note 1 |
| `TrapThrowTime` | **`TrapThrowingTime`** | CalcOffence.lua:1220 | Wrong name — see note 1 |
| `TrapCooldown` | `TrapCooldown` | CalcOffence.lua:1246–1250 | Correct — but conditionally written |
| `MineLayingSpeed` | `MineLayingSpeed` | CalcOffence.lua:1301,1310,1313 | Correct |
| `MineLayingTime` | `MineLayingTime` | CalcOffence.lua:1314 | Correct |

> **Note 1 — `TrapThrowSpeed` / `TrapThrowTime` are wrong names.** PoB writes
> `TrapThrowingSpeed` and `TrapThrowingTime` (with `-ing`). Oracle files confirm:
> `realworld_trap_saboteur.expected.json` has `TrapThrowingSpeed: 1.667` and
> `TrapThrowingTime: 0.6`. Update `field_groups.rs` entries accordingly.

> **Note 2 — `TrapCooldown` is conditionally written.** It is only written when the
> skill has `skillData.trapCooldown`, `skillData.cooldown`, or a `CooldownRecovery` BASE
> mod — not for all trap skills. Neither oracle trap build has `TrapCooldown` because the
> traps tested have no cooldown. When a trap does have a cooldown (e.g. Seismic Trap),
> `TrapCooldown` is written.

> **CalcDefence.lua context:** The `field_inventory_output.json` reports
> `TotemPlacementSpeed`, `TotemPlacementTime`, and `TotemLife` as also being written in
> `CalcDefence.lua:618–620`. Those are actually `TotemXResist` fields written inside the
> resistance loop — a false positive from the inventory scanner. The authoritative writes
> are in CalcOffence.lua.

## Dependencies

- `PERF-08-action-speed-conditions` — `output.ActionSpeedMod` must be set before trap,
  mine, and totem placement speeds are computed (all three multiply by it).
- `SETUP-02-active-skill` — `skillFlags.trap`, `skillFlags.mine`, `skillFlags.totem`,
  `skillFlags.ballista`, and `skillData.totemLevel` / `activeSkill.skillTotemId` must be
  set.

## Lua Source

File: `third-party/PathOfBuilding/src/Modules/CalcOffence.lua`  
Commit: `454eff8c85d24356d9b051d596983745ed367476`

Primary line ranges:
- **Trap section:** lines 1207–1268
- **Mine section:** lines 1295–1362
- **Totem section:** lines 1363–1407

Base stat values set in `CalcSetup.lua:52–56`:
```lua
modDB:NewMod("TrapThrowingTime", "BASE", 0.6, "Base")   -- 0.6s base throw time
modDB:NewMod("MineLayingTime",   "BASE", 0.3, "Base")   -- 0.3s base lay time
modDB:NewMod("TotemPlacementTime","BASE", 0.6, "Base")  -- 0.6s base place time
modDB:NewMod("BallistaPlacementTime","BASE", 0.5, "Base")-- 0.5s base place time
```

## Annotated Lua

### 1. Trap throwing speed and time (lines 1207–1268)

The trap section is entered when `skillFlags.trap` is set.

```lua
if skillFlags.trap then
    -- Base speed: 1 / TrapThrowingTime (BASE mod, default 0.6s → 1.667/s)
    local baseSpeed = 1 / skillModList:Sum("BASE", skillCfg, "TrapThrowingTime")
```

> **`1 / Sum("BASE", skillCfg, "TrapThrowingTime")`** — PoB stores base throwing *time*
> (seconds) as the BASE mod and converts to speed by taking the reciprocal. The default
> is 0.6s (set in CalcSetup), so `baseSpeed = 1 / 0.6 ≈ 1.667/s`. Rust:
> `let base_speed = 1.0 / mod_db.sum_cfg(Base, "TrapThrowingTime", Some(cfg), output)`.

```lua
    local timeMod = calcLib.mod(skillModList, skillCfg, "SkillTrapThrowingTime")
    if timeMod > 0 then
        baseSpeed = baseSpeed * (1 / timeMod)
    end
```

> **`SkillTrapThrowingTime`** — a per-projectile time multiplier (from
> `SkillStatMap.lua:1445`: `mod("SkillTrapThrowingTime", "MORE", nil, 0, 0, {type="PerStat",
> stat="ProjectileCount"})`). When a trap fires multiple projectiles per throw, each costs
> more time. `timeMod > 0` guards against zero division. The Rust does not query this stat.

```lua
    output.TrapThrowingSpeed = baseSpeed * calcLib.mod(skillModList, skillCfg, "TrapThrowingSpeed")
                               * output.ActionSpeedMod
```

> **`calcLib.mod(skillModList, skillCfg, "TrapThrowingSpeed")`** = `(1 + INC/100) × More`
> — the combined throwing speed multiplier from passive tree, gear, and support gems
> (e.g. Swift Assembly Support). Rust:
> `let speed_mod = calc_mod(mod_db, Some(cfg), output, "TrapThrowingSpeed")`.

> **`output.ActionSpeedMod`** — the global action speed from Onslaught, auras, etc.
> This is computed in PERF-08 and multiplies all action speeds. The Rust must apply this
> at the end of the speed calculation, not at the base.

```lua
    -- TrapThrowCount: how many traps are thrown per action (e.g. from Multiple Traps Support)
    local trapThrowCount = calcLib.val(skillModList, "TrapThrowCount", skillCfg)
```

> **`calcLib.val(skillModList, "TrapThrowCount", skillCfg)`** (from CalcTools.lua:32–38):
> ```lua
> function calcLib.val(modStore, name, cfg)
>     local baseVal = modStore:Sum("BASE", cfg, name)
>     if baseVal ~= 0 then
>         return baseVal * calcLib.mod(modStore, cfg, name)
>     else
>         return 0
>     end
> end
> ```
> This is `Base × (1 + INC/100) × More`. `TrapThrowCount` is the total number of traps
> thrown per action — base 1, +2 from Multiple Traps Support = 3. The oracle for
> `realworld_trap_saboteur` shows `TrapThrowCount: 3`.

```lua
    if skillData.trapCooldown or skillData.cooldown then
        trapThrowCount = 1  -- cooldown traps throw exactly 1 trap per action
    end
    output.TrapThrowCount = env.modDB:Override(nil, "TrapThrowCount") or trapThrowCount
```

> **`env.modDB:Override(nil, "TrapThrowCount")`** — Config tab can force a specific
> trap throw count. If set, it overrides the calculated value. Rust:
> `mod_db.override_value(None, output, "TrapThrowCount").unwrap_or(trap_throw_count)`.

```lua
    -- Server tick cap: cannot throw faster than one tick
    output.TrapThrowingSpeed = m_min(output.TrapThrowingSpeed, data.misc.ServerTickRate)
    -- data.misc.ServerTickRate = 1 / 0.033 ≈ 30.30/s

    output.TrapThrowingTime = 1 / output.TrapThrowingSpeed
    -- TrapThrowingTime: seconds per throw of one trap (before dividing by count)

    -- Override the skill's cast time with throw time / count
    -- skillData.timeOverride feeds into CalcOffence speed branch (line 2174):
    -- "elseif skillData.timeOverride then ... output.Speed = 1/output.Time"
    skillData.timeOverride = output.TrapThrowingTime / output.TrapThrowCount
```

> **`skillData.timeOverride`** — by setting this, the trap section overrides the normal
> cast/attack speed calculation to use the trap throw time instead. CalcOffence uses
> `skillData.timeOverride` in the speed branch (lines 2174–2178) to set `output.Time` and
> `output.Speed = 1/output.Time`. This is how the DPS calculation gets the correct rate.
> The Rust `dispatch_trigger` does not set `skillData.timeOverride` — instead it sets
> `output.Speed` directly, which achieves a similar effect but bypasses the normal speed
> calculation path entirely.

---

**TrapCooldown** (lines 1243–1260):

```lua
    local baseCooldown = skillData.trapCooldown or skillData.cooldown
    if baseCooldown or skillModList:Sum("BASE", skillCfg, "CooldownRecovery") ~= 0 then
        if baseCooldown then
            -- Apply ICDR: base / (1 + INC_CooldownRecovery/100 × More_CooldownRecovery)
            output.TrapCooldown = baseCooldown / calcLib.mod(skillModList, skillCfg, "CooldownRecovery")
            -- Tick-align: ceil to nearest server tick (33ms)
            output.TrapCooldown = m_ceil(output.TrapCooldown * data.misc.ServerTickRate)
                                  / data.misc.ServerTickRate
        else
            -- Skill has no base cooldown but has a CooldownRecovery modifier:
            -- use the generic calcSkillCooldown helper
            local cooldown, _, _ = calcSkillCooldown(skillModList, skillCfg, skillData)
            output.TrapCooldown = cooldown
        end
    end
    -- NOTE: TrapCooldown is NOT written when there is no cooldown at all.
    -- Most non-cooldown traps (Cluster Traps, Multiple Traps) leave TrapCooldown unset.
```

> **`TrapCooldown` is conditional.** It is only written when `baseCooldown ~= nil` or
> `Sum("BASE", skillCfg, "CooldownRecovery") ~= 0`. The Rust always writes it, which
> means it appears in oracle output even when it shouldn't. For the oracle trap build
> (no cooldown), `TrapCooldown` must NOT be written.

> **`calcLib.mod(skillModList, skillCfg, "CooldownRecovery")`** = `(1 + INC/100) × More`.
> This is the full ICDR multiplier including both INC and More terms. The Rust
> `dispatch_trigger` uses only `INC` (`CooldownRecovery` INC sum) without the More term.

> **`m_ceil(cd × ServerTickRate) / ServerTickRate`** — round UP to the nearest server tick.
> This is the same tick-alignment used in CalcTriggers. The Rust `align_to_server_tick()`
> implements this correctly.

---

**TrapTriggerRadius** (lines 1261–1268) — bonus field computed alongside traps:

```lua
    local incArea, moreArea = calcLib.mods(skillModList, skillCfg, "TrapTriggerAreaOfEffect")
    local areaMod = round(round(incArea * moreArea, 10), 2)  -- double-round
    output.TrapTriggerRadius = calcRadius(data.misc.TrapTriggerRadiusBase, areaMod)
    output.TrapTriggerRadiusMetre = output.TrapTriggerRadius / 10
```

> These are not in TRIG-02's `field_groups.rs` list but appear in the oracle output
> (`TrapTriggerRadius`, `TrapTriggerRadiusMetre`). They use the same double-round AoE
> pattern from OFF-04. The Rust does not implement them.

---

### 2. Mine laying speed and time (lines 1295–1362)

```lua
if skillFlags.mine then
    local baseSpeed = 1 / skillModList:Sum("BASE", skillCfg, "MineLayingTime")
    -- default MineLayingTime BASE = 0.3s → baseSpeed = 1/0.3 ≈ 3.333/s

    local timeMod = calcLib.mod(skillModList, skillCfg, "SkillMineThrowingTime")
    if timeMod > 0 then
        baseSpeed = baseSpeed * (1 / timeMod)  -- per-projectile penalty
    end

    output.MineLayingSpeed = baseSpeed * calcLib.mod(skillModList, skillCfg, "MineLayingSpeed")
                             * output.ActionSpeedMod
```

> **`MineLayingSpeed` formula:** `(1/baseMineLayingTime) × (1+INC/100) × More × ActionSpeedMod`.
> Identical structure to trap throwing speed, but using `MineLayingSpeed` and
> `MineLayingTime` stat names.

```lua
    -- MineThrowCount: how many mines are thrown per action (e.g. Minefield Support)
    local mineThrowCount = calcLib.val(skillModList, "MineThrowCount", skillCfg)
    if skillData.trapCooldown or skillData.cooldown then
        mineThrowCount = 1  -- cooldown mines throw exactly 1 mine
    end
    output.MineThrowCount = env.modDB:Override(nil, "MineThrowCount") or mineThrowCount
    if output.MineThrowCount >= 1 then
        -- Each additional mine thrown costs 10% more time:
        -- 1 mine = ×1.0; 2 mines = ×1/(1+0.1) = ×0.909; 3 mines = ×1/(1+0.2) = ×0.833
        output.MineLayingSpeed = output.MineLayingSpeed / (1 + (output.MineThrowCount - 1) * 0.1)
    end
```

> **Additional mine time penalty** — each extra mine thrown above the first makes the
> throw 10% slower (`1 + (N-1) × 0.1` in the denominator). For Minefield Support (5
> mines): `divisor = 1 + 4×0.1 = 1.4`, so the speed is reduced by 29%. The Rust does
> not implement this penalty.

```lua
    output.MineLayingSpeed = m_min(output.MineLayingSpeed, data.misc.ServerTickRate)
    output.MineLayingTime = 1 / output.MineLayingSpeed

    -- Mine-trap interaction: mine throws traps
    if skillFlags.trap then
        skillData.timeOverride = output.MineLayingTime / output.MineThrowCount / output.TrapThrowCount
    else
        skillData.timeOverride = output.MineLayingTime / output.MineThrowCount
    end
```

> **Mine-trap timeOverride:** A skill can be both a mine and a trap simultaneously (e.g.
> Blastchain Mine Support + Cluster Traps). When both flags are set, the override divides
> by both `MineThrowCount` and `TrapThrowCount` so that the effective DPS rate accounts
> for the combined throw count.

---

### 3. Totem placement speed, time, and life (lines 1363–1407)

```lua
if skillFlags.totem then
    local baseSpeed
    if skillFlags.ballista then
        -- Ballista totems use a separate base placement time (0.5s vs 0.6s for regular totems)
        baseSpeed = 1 / skillModList:Sum("BASE", skillCfg, "BallistaPlacementTime")
    else
        baseSpeed = 1 / skillModList:Sum("BASE", skillCfg, "TotemPlacementTime")
    end
    -- Default TotemPlacementTime BASE = 0.6s; BallistaPlacementTime BASE = 0.5s

    output.TotemPlacementSpeed = baseSpeed * calcLib.mod(skillModList, skillCfg, "TotemPlacementSpeed")
                                 * output.ActionSpeedMod
    output.TotemPlacementTime  = 1 / output.TotemPlacementSpeed
```

> **Ballista branch:** Ballista totems (Siege Ballista, Shrapnel Ballista, etc.) have a
> different base placement time than regular totems. The Rust uses a single hardcoded
> base of 0.6s for all totems — it does not check `skillFlags.ballista` or query
> `BallistaPlacementTime`. For the totem oracle build (Shockwave Totem, not a ballista),
> this is not triggered, but for any ballista oracle build it would be wrong.

> **No server tick cap on `TotemPlacementSpeed`.** Unlike trap/mine, totem placement
> speed is NOT capped at `ServerTickRate`. The Rust current implementation does not cap
> trap or mine either; the Lua does cap both.

```lua
    output.ActiveTotemLimit = skillModList:Sum("BASE", skillCfg, "ActiveTotemLimit",
                                               "ActiveBallistaLimit")
    output.TotemsSummoned = env.modDB:Override(nil, "TotemsSummoned") or output.ActiveTotemLimit
```

> **Two-stat query:** `Sum("BASE", skillCfg, "ActiveTotemLimit", "ActiveBallistaLimit")`
> sums BOTH `ActiveTotemLimit` and `ActiveBallistaLimit` mods in a single call. In Rust:
> `mod_db.sum_cfg(Base, "ActiveTotemLimit", Some(cfg), output) + mod_db.sum_cfg(Base, "ActiveBallistaLimit", Some(cfg), output)`.
> The Rust currently only queries `"ActiveTotemLimit"`.

```lua
    output.TotemLifeMod = calcLib.mod(skillModList, skillCfg, "TotemLife")
    -- TotemLife formula:
    -- floor(monsterAllyLifeTable[totemLevel] × totemLifeMult[skillTotemId]) × TotemLifeMod
    -- then round to nearest integer
    output.TotemLife = round(
        m_floor(
            env.data.monsterAllyLifeTable[skillData.totemLevel]
            * env.data.totemLifeMult[activeSkill.skillTotemId]
        ) * output.TotemLifeMod
    )
```

> **`TotemLife` formula involves two data lookups:**
> 1. `env.data.monsterAllyLifeTable[skillData.totemLevel]` — base life for a totem of
>    this level (indexed by integer level 1–100). Example: level 20 ≈ 200 HP base.
> 2. `env.data.totemLifeMult[activeSkill.skillTotemId]` — multiplier specific to this
>    totem skill type (e.g. Shockwave Totem has a different life pool than Flame Totem).
>
> **Operation order:**
> 1. `floor(baseLife × lifeMult)` — integer floor applied to the base × type multiplier
> 2. `× TotemLifeMod` — then multiply by the player's TotemLife modifier (INC/More combo)
> 3. `round(...)` — round the final result
>
> The Rust uses `(100.0 × (1 + inc/100) × more).round()` as a fixed base of 100 HP,
> ignoring both the level table and the skill type multiplier entirely.

> **`skillData.totemLevel`** — the level of the totem unit, often the active skill's gem
> level (1–21). Used to index `monsterAllyLifeTable`. In Rust, this is part of the skill
> gem data available via `env.player.main_skill`.

> **`activeSkill.skillTotemId`** — an integer identifying the totem type (from gem data).
> Each totem skill has a unique ID that maps to its life multiplier in
> `env.data.totemLifeMult`. The Rust has no equivalent concept.

```lua
    -- Additional totem stats (not in field_groups.rs but appear in oracle):
    output.TotemEnergyShield = skillModList:Sum("BASE", skillCfg, "TotemEnergyShield")
    output.TotemBlockChance  = skillModList:Sum("BASE", skillCfg, "TotemBlockChance")
    output.TotemArmour       = skillModList:Sum("BASE", skillCfg, "TotemArmour")
```

> These three fields are not listed in `field_groups.rs` for TRIG-02 but appear in the
> totem oracle output. They are simple BASE mod queries.

---

### 4. Server tick rate cap for trap and mine

Both trap and mine speeds are capped at `data.misc.ServerTickRate`:

```lua
-- data.misc.ServerTickRate = 1 / 0.033 ≈ 30.3030.../s
output.TrapThrowingSpeed = m_min(output.TrapThrowingSpeed, data.misc.ServerTickRate)
-- and:
output.MineLayingSpeed   = m_min(output.MineLayingSpeed, data.misc.ServerTickRate)
```

> **Totem placement is NOT capped at ServerTickRate.** Only trap and mine speeds get this
> cap. Totem placement speed can theoretically exceed 30/s (though in practice it never
> does with current PoE gear).

---

### 5. `skillData.timeOverride` feeds into CalcOffence speed (lines 2174–2178)

After the trap/mine section sets `skillData.timeOverride`, CalcOffence picks it up:

```lua
-- CalcOffence.lua:2174–2178 (inside the speed calculation block):
if skillData.timeOverride and not skillData.triggeredOnDeath then
    output.Time = skillData.timeOverride
    output.Speed = 1 / output.Time
```

This is how `TrapThrowingTime / TrapThrowCount` or `MineLayingTime / MineThrowCount`
becomes the skill's effective use rate (`output.Speed`). The oracle fields `TrapThrowingSpeed`
and `MineLayingSpeed` are independent of `output.Speed` — they measure the raw throwing
rate, while `output.Speed = 1/timeOverride` measures the per-projectile rate.

---

## Existing Rust Code

File: `crates/pob-calc/src/calc/triggers.rs`

- **`calc_totem_dps`:** lines 202–247
- **`calc_trap_dps`:** lines 251–287
- **`calc_mine_dps`:** lines 291–325

### What exists

**Totem (lines 202–247):**
- Queries `ActiveTotemLimit` BASE → `active_totems`.
- Computes `TotemLife = (100.0 × (1+INC/100) × More).round()`.
- Reads `TotemPlacementTime` BASE (defaults to 0.6s) → writes `TotemPlacementTime`.
- Writes `ActiveTotemLimit`, `TotemLife`, `TotemLifeTotal`, `TotemPlacementTime`,
  `TotemDPS`, `CombinedDPS`.

**Trap (lines 251–287):**
- Base throw time hardcoded to 0.6s.
- Queries `TrapThrowingSpeed` INC and More.
- Computes `throw_time = 0.6 / ((1 + INC/100) × More)`.
- Writes `TrapThrowingTime`, `TrapThrowingSpeed`, `TrapCooldown`, `TrapDPS`, `CombinedDPS`.

**Mine (lines 291–325):**
- Reads `MineLayingTime` BASE (defaults to 0.3s).
- Queries `MineLayingSpeed` INC and More.
- Computes `effective_lay_time = lay_time / ((1 + INC/100) × More)`.
- Writes `MineLayingTime`, `MineLayingSpeed`, `MineDetonationTime`, `MineDPS`, `CombinedDPS`.

### What's missing / wrong

**Field name errors in `field_groups.rs`:**

1. **`TrapThrowSpeed` → `TrapThrowingSpeed`; `TrapThrowTime` → `TrapThrowingTime`.** The
   oracle files confirm the names with `-ing`. The `field_groups.rs` entries are missing
   the `-ing` suffix. (The Rust code itself writes the correct names, so only the
   `field_groups.rs` test registration needs updating.)

**Totem:**

2. **`TotemLife` formula is wrong.** Rust: `100 × (1+INC/100) × More`. Lua:
   `round(floor(monsterAllyLifeTable[level] × totemLifeMult[totemId]) × TotemLifeMod)`
   where `TotemLifeMod = calcLib.mod(skillModList, skillCfg, "TotemLife")`. The Rust
   ignores the monster level life table and the per-totem-type life multiplier, using a
   fixed base of 100 HP. The oracle for the hierophant totem build shows `TotemLife: 1478`,
   which is far from 100.

3. **`TotemPlacementSpeed` not written.** The Rust writes `TotemPlacementTime` but never
   writes `TotemPlacementSpeed` (the reciprocal), which the oracle asserts directly.

4. **`TotemPlacementSpeed` does not apply `ActionSpeedMod`.** Lua multiplies by
   `output.ActionSpeedMod`. The Rust does not apply this modifier.

5. **`TotemPlacementSpeed` does not apply INC/More `TotemPlacementSpeed` mods.** Lua uses
   `calcLib.mod(skillModList, skillCfg, "TotemPlacementSpeed")`. The Rust reads only the
   BASE `TotemPlacementTime` and inverts it — it ignores all incremental/more multipliers
   on placement speed.

6. **Ballista branch absent.** For ballista skills, Lua reads `BallistaPlacementTime`
   (base 0.5s). The Rust always uses `TotemPlacementTime` (0.6s), which overestimates
   placement time for all ballista builds.

7. **`ActiveTotemLimit` query incomplete.** Lua queries `Sum("BASE", skillCfg,
   "ActiveTotemLimit", "ActiveBallistaLimit")` — both stat names in one call. The Rust
   queries only `"ActiveTotemLimit"`. This means builds with `ActiveBallistaLimit` mods
   (from Ballista Totem Support or passives) will show the wrong totem limit.

8. **`TotemsSummoned` not written.** Lua writes `output.TotemsSummoned = Override(nil,
   "TotemsSummoned") or output.ActiveTotemLimit`. The oracle shows `TotemsSummoned: 3`
   for the hierophant build.

9. **`TotemActionSpeed` not written.** The `output.TotemActionSpeed` from CalcOffence
   (set in the speed block for totem skills at line ~2311) is set by the totem speed
   calculation in CalcOffence, not CalcTriggers. However, the Rust `calc_totem_dps` runs
   in `triggers.rs` and does not propagate any interaction with the totem action speed.

**Trap:**

10. **`TrapThrowingSpeed` formula is wrong.** Lua: `(1/base_time) × calcLib.mod(...,
    "TrapThrowingSpeed") × ActionSpeedMod`. Rust: `0.6 / ((1+INC/100) × More)`. The Lua
    inverts to speed first (`1/base_time`) and then multiplies by the combined modifier;
    the Rust divides the time by the modifier, which is equivalent only when
    `ActionSpeedMod = 1.0`. The Lua also uses `skillCfg` context for the queries, while
    the Rust uses no cfg. Most importantly, `ActionSpeedMod` is not applied.

11. **`SkillTrapThrowingTime` per-projectile multiplier not applied.** Lua queries
    `calcLib.mod(skillModList, skillCfg, "SkillTrapThrowingTime")` and divides `baseSpeed`
    by it when `timeMod > 0`. The Rust ignores this.

12. **`TrapThrowCount` not computed or applied.** Lua derives `TrapThrowCount` from
    `calcLib.val(skillModList, "TrapThrowCount", skillCfg)`, applies the cooldown override
    if needed, applies the Config tab override, and then divides `TrapThrowingTime` by the
    count to set `skillData.timeOverride`. The Rust never queries `TrapThrowCount` and
    sets `skillData.timeOverride` is not used at all.

13. **Server tick rate cap absent for trap speed.** Lua: `TrapThrowingSpeed =
    min(TrapThrowingSpeed, ServerTickRate)`. The Rust does not cap the speed.

14. **`TrapCooldown` always written.** Lua only writes `TrapCooldown` when
    `baseCooldown ~= nil` or `Sum("BASE", skillCfg, "CooldownRecovery") ~= 0`. The Rust
    always writes it (defaulting to 4.0s), which means trap builds without a cooldown
    would have `TrapCooldown` in oracle output when it shouldn't be there.

15. **`TrapCooldown` ICDR formula missing More term.** Lua: `baseCooldown /
    calcLib.mod(..., "CooldownRecovery")` = `baseCooldown / ((1+INC/100) × More)`. The Rust
    queries a `TrapCooldown` BASE mod directly, not applying ICDR at all.

16. **`TrapTriggerRadius` and `TrapTriggerRadiusMetre` not written.** These fields appear
    in the oracle output but are not in `field_groups.rs` and not in the Rust.

**Mine:**

17. **`MineLayingSpeed` formula misses `ActionSpeedMod`.** Lua multiplies by
    `output.ActionSpeedMod`. The Rust does not apply it.

18. **`SkillMineThrowingTime` per-projectile multiplier not applied.** Same as trap — Lua
    queries this and divides `baseSpeed` by it when `timeMod > 0`.

19. **`MineThrowCount` additional mine penalty absent.** Lua divides `MineLayingSpeed` by
    `(1 + (MineThrowCount - 1) × 0.1)` for each additional mine thrown. The Rust does not
    query `MineThrowCount` and never applies this slowdown.

20. **Server tick rate cap absent for mine speed.** Same as trap — Lua caps at
    `ServerTickRate`.

21. **`MineDetonationTime` formula is wrong.** The Rust adds a hardcoded 0.25s detonation
    delay (`effective_lay_time + 0.25`). PoB does not compute `MineDetonationTime` this
    way in CalcOffence. The detonation radius and related fields are computed at lines
    1345–1361 but there is no `output.MineDetonationTime` written in CalcOffence. The
    "mine DPS = TotalDPS / detonation_time" formula in the Rust trigger handler is also
    not how PoB computes mine DPS — PoB uses `skillData.timeOverride = MineLayingTime /
    MineThrowCount` and the normal speed/DPS pipeline.

22. **`MineDetonationRadius` and `MineDetonationRadiusMetre` / `MineAuraRadius` and
    `MineAuraRadiusMetre` not written.** These appear in the mine oracle build but are
    absent from the Rust.

## What Needs to Change

1. **Fix `field_groups.rs` entries:** rename `"TrapThrowSpeed"` → `"TrapThrowingSpeed"` and
   `"TrapThrowTime"` → `"TrapThrowingTime"`.

2. **Fix `TotemLife` formula.** Replace the fixed-100 formula with:
   ```rust
   // env.player.main_skill provides totem_level and skill_totem_id
   let base_life = data.monster_ally_life_table[totem_level];  // indexed 1–100
   let type_mult = data.totem_life_mult[skill_totem_id];       // per-totem multiplier
   let life_mod  = calc_mod(mod_db, Some(cfg), output, "TotemLife");
   let totem_life = (base_life * type_mult).floor() * life_mod;
   let totem_life = totem_life.round();
   env.player.set_output("TotemLife", totem_life);
   ```

3. **Write `TotemPlacementSpeed`.** After computing the placement time, also write its
   reciprocal:
   ```rust
   let speed = 1.0 / placement_time;
   env.player.set_output("TotemPlacementSpeed", speed);
   env.player.set_output("TotemPlacementTime", placement_time);
   ```

4. **Apply `ActionSpeedMod` to `TotemPlacementSpeed`.** Query `output.ActionSpeedMod`
   and multiply:
   ```rust
   let action_speed = get_output_f64(&env.player.output, "ActionSpeedMod").max(0.001);
   let speed = base_speed * speed_mod * action_speed;
   ```

5. **Apply INC/More `TotemPlacementSpeed` mods.** Replace the current bare time
   query with the full formula:
   ```rust
   let base_speed = 1.0 / mod_db.sum_cfg(Base, "TotemPlacementTime", Some(cfg), output);
   let speed_mod  = calc_mod(mod_db, Some(cfg), output, "TotemPlacementSpeed");
   let speed = base_speed * speed_mod * action_speed;
   ```

6. **Implement ballista branch.** Check `skill_flags.ballista` and use
   `BallistaPlacementTime` (base 0.5s) instead of `TotemPlacementTime`:
   ```rust
   let time_stat = if skill_flags.ballista { "BallistaPlacementTime" } else { "TotemPlacementTime" };
   let base_speed = 1.0 / mod_db.sum_cfg(Base, time_stat, Some(cfg), output);
   ```

7. **Query both `ActiveTotemLimit` and `ActiveBallistaLimit`:**
   ```rust
   let active_totems = mod_db.sum_cfg(Base, "ActiveTotemLimit", Some(cfg), output)
                     + mod_db.sum_cfg(Base, "ActiveBallistaLimit", Some(cfg), output);
   ```

8. **Write `TotemsSummoned`:**
   ```rust
   let totems_summoned = mod_db.override_value(None, output, "TotemsSummoned")
       .unwrap_or(active_totems);
   env.player.set_output("TotemsSummoned", totems_summoned);
   ```

9. **Fix `TrapThrowingSpeed` formula.** Change from `0.6 / (inc_more)` to:
   ```rust
   let base_speed = 1.0 / mod_db.sum_cfg(Base, "TrapThrowingTime", Some(cfg), output);
   // Apply SkillTrapThrowingTime per-projectile multiplier (if > 0):
   let time_mod = calc_mod(mod_db, Some(cfg), output, "SkillTrapThrowingTime");
   let base_speed = if time_mod > 0.0 { base_speed * (1.0 / time_mod) } else { base_speed };
   let speed_mod = calc_mod(mod_db, Some(cfg), output, "TrapThrowingSpeed");
   let action_speed = get_output_f64(&env.player.output, "ActionSpeedMod").max(0.001);
   let speed = (base_speed * speed_mod * action_speed).min(SERVER_TICK_RATE);
   ```

10. **Compute and write `TrapThrowCount`:**
    ```rust
    let trap_throw_count = if has_cooldown {
        1.0
    } else {
        mod_db.override_value(None, output, "TrapThrowCount")
            .unwrap_or_else(|| calc_val(mod_db, Some(cfg), output, "TrapThrowCount").max(1.0))
    };
    env.player.set_output("TrapThrowCount", trap_throw_count);
    // Set skillData.timeOverride equivalent:
    skill_data.time_override = throw_time / trap_throw_count;
    ```

11. **Write `TrapCooldown` conditionally.** Only write when `base_cooldown > 0`:
    ```rust
    if let Some(base_cd) = skill_data.trap_cooldown.or(skill_data.cooldown) {
        let icdr = calc_mod(mod_db, Some(cfg), output, "CooldownRecovery");
        let cd = align_to_server_tick(base_cd / icdr);
        env.player.set_output("TrapCooldown", cd);
    }
    // Do NOT write TrapCooldown when there is no base cooldown.
    ```

12. **Fix `MineLayingSpeed` formula.** Add `ActionSpeedMod`, `SkillMineThrowingTime`,
    and the `MineThrowCount` slowdown penalty:
    ```rust
    let base_speed = 1.0 / mod_db.sum_cfg(Base, "MineLayingTime", Some(cfg), output);
    let time_mod = calc_mod(mod_db, Some(cfg), output, "SkillMineThrowingTime");
    let base_speed = if time_mod > 0.0 { base_speed / time_mod } else { base_speed };
    let speed_mod = calc_mod(mod_db, Some(cfg), output, "MineLayingSpeed");
    let action_speed = get_output_f64(&env.player.output, "ActionSpeedMod").max(0.001);
    let mine_count = /* MineThrowCount, same pattern as TrapThrowCount */;
    let slowdown = 1.0 + (mine_count - 1.0) * 0.1;
    let speed = (base_speed * speed_mod * action_speed / slowdown).min(SERVER_TICK_RATE);
    ```

13. **Remove the hardcoded 0.25s `MineDetonationTime` formula.** PoB does not compute
    mine DPS as `skill_dps / detonation_time`. Instead it uses `skillData.timeOverride =
    MineLayingTime / MineThrowCount` to let the normal speed/DPS pipeline handle it.
    Remove `calc_mine_dps` entirely from the "DPS computation" category — it should only
    write `MineLayingSpeed`, `MineLayingTime`, `MineThrowCount`.

14. **Implement `TrapTriggerRadius`/`TrapTriggerRadiusMetre` and
    `MineDetonationRadius`/`MineDetonationRadiusMetre` / `MineAuraRadius`/`MineAuraRadiusMetre`.**
    These use the same `calcRadius` double-round pattern from OFF-04. Not in TRIG-02
    `field_groups.rs` but asserted by the oracle and worth implementing alongside the
    other trap/mine fields.
