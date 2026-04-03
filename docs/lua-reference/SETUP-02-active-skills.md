# SETUP-02: Active Skill Construction

## Output Fields

SETUP-02 does not write `output.*` values directly. It populates structural
data that every downstream chunk depends on:

| Field | Location | Consumed by |
|-------|----------|-------------|
| `env.player.active_skill_list` | `Actor::active_skill_list` | PERF-04 (reservation), OFF-*, TRIG-* |
| `env.player.main_skill` | `Actor::main_skill` | All offence/defence calculations |
| `activeSkill.skillData.manaReservationPercent` | `skill.skill_data["manaReservationPercent"]` | PERF-04 |
| `activeSkill.skillData.CritChance` | `skill.skill_data["CritChance"]` | OFF-03 |
| `activeSkill.skillData.attackTime` | `skill.skill_data["attackTime"]` | OFF-04 |
| `activeSkill.skillData.attackSpeedMultiplier` | `skill.skill_data["attackSpeedMultiplier"]` | OFF-04 |
| `activeSkill.skillData.cooldown` | `skill.skill_data["cooldown"]` | TRIG-01 |
| `activeSkill.skillData.storedUses` | `skill.skill_data["storedUses"]` | TRIG-01 |
| `activeSkill.skillData.totemLevel` | `skill.skill_data["totemLevel"]` | TRIG-02 |
| `activeSkill.skillModList` | `ActiveSkill::skill_mod_db` | All offence calculations |
| `activeSkill.skillCfg` | `ActiveSkill::skill_cfg` | All `modDB:Sum/Flag/More(skillCfg, …)` calls |
| `activeSkill.triggeredBy` | `ActiveSkill::triggered_by` | TRIG-01 |
| `activeSkill.skillFlags.*` | `ActiveSkill::skill_flags` | Offence/defence dispatch |

## Dependencies

- `SETUP-01` (item mods parsed, `modDB` populated, `env.player.weapon_data1/2` set)

## Lua Source

**File 1:** `third-party/PathOfBuilding/src/Modules/CalcSetup.lua`, lines 1292–1789  
**File 2:** `third-party/PathOfBuilding/src/Modules/CalcActiveSkill.lua`, full file  
**Commit:** `454eff8c85d24356d9b051d596983745ed367476`

---

## Annotated Lua

### CalcSetup.lua — Phase 1: Item-Granted Skill Groups (lines 1305–1403)

```lua
-- env.grantedSkills: populated earlier in CalcSetup by item processing.
-- Each entry has { skillId, level, source, slotName, sourceItem, … }.
-- This block ensures every item-granted skill has a matching socketGroup.
-- NOTE: In Rust, item-granted skills need their own SkillGroup entries
-- created and inserted into the skill_sets before the main loop.

for _, grantedSkill in ipairs(env.grantedSkills) do
    -- Look for an existing socket group with same source + slot
    local group
    for index, socketGroup in pairs(build.skillsTab.socketGroupList) do
        if socketGroup.source == grantedSkill.source
           and socketGroup.slot == grantedSkill.slotName then
            -- Match on first gem skillId AND level (normalized)
            if socketGroup.gemList[1] and ...
```

**Translation notes:**
- `env.grantedSkills` is populated by the item-mod scanner (SETUP-01) when it finds
  `ExtraSkill` mods (e.g., "Socketed Gems are Supported by Level 22 Blasphemy" → no;
  that's an ExtraSupport, not ExtraSkill. ExtraSkill = "Grants Level 20 Summon Skeletons").
- In Rust this list doesn't exist yet. It must be built during `add_item_mods()` by
  scanning for `GrantedSkill` mod values in the item modDB.
- The Lua normalises gem levels via `calcLib.validateGemLevel()`. The Rust equivalent
  is clamping level to the gem's valid level range.
- `group.noSupports = true` on item-granted skills prevents socketed gem supports from
  being applied to those skills. The Rust `Skill::enabled` / `no_supports` flag mirrors this.

### CalcSetup.lua — Phase 2: Weapon Data (lines 1406–1423)

```lua
-- After all item-granted groups are handled, resolve weapon data.
env.player.weaponData1 = env.player.itemList["Weapon 1"]
    and env.player.itemList["Weapon 1"].weaponData
    and env.player.itemList["Weapon 1"].weaponData[1]
    or copyTable(env.data.unarmedWeaponData[env.classId])
-- "or copyTable(…)" → weaponData1 is NEVER nil; falls back to unarmed data.
-- env.data.unarmedWeaponData[env.classId] varies by class.
-- IMPORTANT: Weapon data uses 1-based index [1] = main hand, [2] = off hand.

if env.player.weaponData1.countsAsDualWielding then
    -- Unique weapons (e.g. Varunastra) that count as dual wielding
    env.player.weaponData2 = env.player.itemList["Weapon 1"].weaponData[2]
elseif not env.player.itemList["Weapon 2"] then
    -- Check for Hollow Palm Technique keystone (no weapon + no gloves)
    -- if found, weaponData2 = unarmed table (dual-wield unarmed)
    env.player.weaponData2 = env.player.weaponData2 or { }
    -- empty table {} = no off-hand weapon (NOT nil)
else
    env.player.weaponData2 = env.player.itemList["Weapon 2"].weaponData
        and env.player.itemList["Weapon 2"].weaponData[2] or { }
end
```

**Translation notes:**
- Rust already resolves `weapon_data1` in `setup.rs`. Verify it falls back to
  unarmed data per-class. The unarmed data table is in `env.data.unarmedWeaponData`.
- `weaponData2 = {}` (empty table, not nil) is semantically important — it means
  "no off-hand weapon" without crashing weapon-type checks. In Rust, `weapon_data2`
  is `Option<ItemWeaponData>`, but the `getWeaponFlags` call guards against `None`.

### CalcSetup.lua — Phase 3: Main Socket Group Selection (lines 1425–1432)

```lua
-- Clamp mainSocketGroup index to valid range, then store in env.
build.mainSocketGroup = m_min(
    m_max(#build.skillsTab.socketGroupList, 1),
    build.mainSocketGroup or 1
)
env.mainSocketGroup = build.mainSocketGroup
-- NOTE: Lua is 1-based. env.mainSocketGroup == 1 means first group.
-- Rust's build.main_socket_group is 0-based.
```

### CalcSetup.lua — Phase 4: Support Collection (lines 1434–1552)

This is the first pass over all socket groups. It collects support gem effects
into per-slot `supportLists` tables. The output of this phase is consumed in
Phase 5 when creating active skills.

```lua
-- Cross-linked support groups (e.g. Tabula Rasa, Solstice Vigil "LinkedSupport" mod)
env.crossLinkedSupportGroups = {}
for _, mod in ipairs(env.modDB:Tabulate("LIST", nil, "LinkedSupport")) do
    -- mod.mod.sourceSlot = the supporting slot (e.g. "Body Armour")
    -- mod.value.targetSlotName = the slot being supported
    env.crossLinkedSupportGroups[mod.mod.sourceSlot][...] = ...targetSlotName
end
```

**Translation notes:**
- `Tabulate("LIST", nil, "LinkedSupport")` = `mod_db.list(None, "LinkedSupport")`.
  In Rust this returns `Vec<&ModValue>`. The Rust `mod_db` doesn't have a `Tabulate`
  equivalent that returns the full `(mod, value)` pair; this may require adding a
  `list_with_source` query.
- Cross-linked supports are driven by a `LinkedSupport` LIST mod on the player modDB.
  Only a few items produce these (Tabula Rasa, etc.). They allow supports socketed in
  one slot to support skills in another slot.

```lua
-- supportLists[slotName][group] = list of supportEffect tables
-- supportLists[group] = list of supportEffect tables (for "noSlot" groups)
local supportLists = { }

for index, group in ipairs(build.skillsTab.socketGroupList) do
    -- slotEnabled: whether the socket group's weapon set is the active one
    group.slotEnabled = not slot or not slot.weaponSet
        or slot.weaponSet == (useSecondWeaponSet and 2 or 1)
    -- Only process the main skill group OR enabled groups
    if index == env.mainSocketGroup or (group.enabled and group.slotEnabled) then
        -- ExtraSupport mods: item-implicit supports (e.g. Heretic's Veil Blasphemy)
        if not group.source then  -- not an item-granted skill group
            for _, value in ipairs(env.modDB:List(groupCfg, "ExtraSupport")) do
                -- value.skillId, value.level
                local grantedEffect = env.data.skills[value.skillId]
                -- Disambiguation: some skills share names with supports (e.g. "Barrage")
                -- If env.data.skills[skillId] is not a support, try "Support"..skillId
                if grantedEffect and not grantedEffect.support then
                    grantedEffect = env.data.skills["Support"..value.skillId]
                end
                grantedEffect.fromItem = true
                -- Add to targetListList with level=value.level, quality=0
                t_insert(targetList, {
                    grantedEffect = grantedEffect,
                    level = value.level,
                    quality = 0,
                    enabled = true,
                })
            end
        end

        -- Process explicit gem supports in this socket group
        for gemIndex, gemInstance in ipairs(group.gemList) do
            if gemInstance.enabled then
                local function processGrantedEffect(grantedEffect)
                    if not grantedEffect or not grantedEffect.support then return end
                    local supportEffect = {
                        grantedEffect = grantedEffect,
                        level = gemInstance.level,
                        quality = gemInstance.quality,
                        srcInstance = gemInstance,
                        …
                    }
                    -- addBestSupport: deduplicates supports with same grantedEffect.
                    -- The higher level/quality wins. "Plus version" supersedes base.
                    addBestSupport(supportEffect, targetList, env.mode)
                end
                if gemInstance.gemData then
                    -- Vaal gems have two grantedEffects: [1]=Vaal version, [2]=base version
                    processGrantedEffect(gemInstance.gemData.grantedEffect)
                    processGrantedEffect(gemInstance.gemData.secondaryGrantedEffect)
                else
                    processGrantedEffect(gemInstance.grantedEffect)
                end
            end
        end
    end
end
```

**Translation notes for support collection:**

| Lua pattern | Rust equivalent |
|-------------|-----------------|
| `gemInstance.gemData.grantedEffect` | `gem_data.granted_effect` (primary effect) |
| `gemInstance.gemData.secondaryGrantedEffect` | `gem_data.secondary_granted_effect` (Vaal base) |
| `gemInstance.grantedEffect` | fallback for item-granted skills with no gemData |
| `addBestSupport(effect, list, mode)` | Need to implement: dedup by `granted_effect` identity, keep higher level/quality, "plus version" (awakened) supersedes base |
| `grantedEffect.plusVersionOf` | `gem_data.plus_version_of: Option<String>` — not in current `GemData` struct |
| `groupCfg` | `SkillCfg`-like struct with `slotName`, `propertyModList`, gem counts |
| `env.modDB:List(groupCfg, "ExtraSupport")` | `mod_db.list_cfg(slot_cfg, "ExtraSupport")` — returns item-granted supports |

**`addBestSupport` logic (CalcSetup.lua:320–349):**
```lua
local function addBestSupport(supportEffect, appliedSupportList, mode)
    local add = true
    for index, otherSupport in ipairs(appliedSupportList) do
        if supportEffect.grantedEffect == otherSupport.grantedEffect then
            -- Same support gem type — keep only the best level/quality
            add = false
            if supportEffect.level > otherSupport.level
               or (supportEffect.level == otherSupport.level
                   and supportEffect.quality > otherSupport.quality) then
                otherSupport.superseded = true  -- only in MAIN mode
                appliedSupportList[index] = supportEffect
            else
                supportEffect.superseded = true
            end
            break
        elseif supportEffect.grantedEffect.plusVersionOf == otherSupport.grantedEffect.id then
            -- Awakened version supersedes the base version
            add = false
            otherSupport.superseded = true
            appliedSupportList[index] = supportEffect
        elseif otherSupport.grantedEffect.plusVersionOf == supportEffect.grantedEffect.id then
            -- Base version is superseded by an already-present awakened version
            add = false
            supportEffect.superseded = true
        end
    end
    if add then
        t_insert(appliedSupportList, supportEffect)
    end
end
```

**Key insight:** `grantedEffect` identity comparison (`==`) is Lua table reference
equality. In Rust this maps to comparing `gem_id` strings (e.g. `"SupportBurningDamage"`).

### CalcSetup.lua — Phase 5: Active Skill Creation (lines 1554–1676)

Second pass over socket groups. Creates `activeSkill` objects and appends them
to `env.player.activeSkillList`.

```lua
for index, group in ipairs(build.skillsTab.socketGroupList) do
    if index == env.mainSocketGroup or (group.enabled and group.slotEnabled) then
        local socketGroupSkillList = {}
        local slotHasActiveSkill = false

        for gemIndex, gemInstance in ipairs(group.gemList) do
            if gemInstance.enabled and (gemInstance.gemData or gemInstance.grantedEffect) then
                -- grantedEffectList: Vaal gems grant [1]=Vaal effect, [2]=base effect.
                -- Non-Vaal gems: [1]=sole effect (wrapped in table).
                local grantedEffectList = gemInstance.gemData
                    and gemInstance.gemData.grantedEffectList
                    or { gemInstance.grantedEffect }

                for index, grantedEffect in ipairs(grantedEffectList) do
                    -- Skip: supports, unsupported effects, hasGlobalEffect unless enabled
                    if not grantedEffect.support
                       and not grantedEffect.unsupported
                       and (not grantedEffect.hasGlobalEffect
                            or gemInstance["enableGlobal"..index]) then
                        -- "enableGlobal1" controls Vaal effect (index 1)
                        -- "enableGlobal2" controls base effect (index 2)
                        -- In Rust: Gem::enable_global1 / enable_global2

                        slotHasActiveSkill = true
                        local activeEffect = {
                            grantedEffect = grantedEffect,
                            level = gemInstance.level,
                            quality = gemInstance.quality,
                            srcInstance = gemInstance,
                            gemData = gemInstance.gemData,
                        }

                        -- Resolve applicable supports for this active gem.
                        -- Two sources:
                        --   1. supportLists[group] — supports explicitly in this group
                        --   2. For item-granted skills (group.source ~= nil):
                        --      also merge supportLists[slotName][group] for other groups
                        --      in the same slot
                        --   3. crossLinkedSupportGroups — supports from other slots
                        local appliedSupportList = {}
                        if not group.noSupports then
                            appliedSupportList = copyTable(
                                supportLists[group] or supportLists[slotName][group], true)
                            -- For item-granted skills: add supports from same slot
                            if group.source and supportLists[slotName] then
                                for _, supportGroup in pairs(supportLists[slotName]) do
                                    for _, supportEffect in ipairs(supportGroup) do
                                        addBestSupport(supportEffect, appliedSupportList, …)
                                    end
                                end
                            end
                            -- Add cross-linked supports (LinkedSupport mods)
                            for crossLinkedSupportSlot, crossLinkedSupportGroup in pairs(env.crossLinkedSupportGroups) do
                                for _, crossLinkedSupportedSlot in ipairs(crossLinkedSupportGroup) do
                                    if crossLinkedSupportedSlot == slotName then
                                        -- merge supportLists[crossLinkedSupportSlot] into appliedSupportList
                                    end
                                end
                            end
                        end

                        -- Create the active skill struct
                        local activeSkill = calcs.createActiveSkill(
                            activeEffect, appliedSupportList, env.player, group)
                        -- Set slotName if this is a gem (not an item-granted effect)
                        if gemInstance.gemData then
                            activeSkill.slotName = groupCfg.slotName
                        end
                        t_insert(socketGroupSkillList, activeSkill)
                        t_insert(env.player.activeSkillList, activeSkill) -- ← THE KEY STEP
                    end
                end
            end
        end

        -- Select main skill from this socket group
        if index == env.mainSocketGroup and #socketGroupSkillList > 0 then
            local activeSkillIndex = m_min(#socketGroupSkillList, group.mainActiveSkill or 1)
            env.player.mainSkill = socketGroupSkillList[activeSkillIndex]
        end
    end
end

-- Fallback: if no main skill was found, create a default "Melee" skill
if not env.player.mainSkill then
    local defaultEffect = {
        grantedEffect = env.data.skills.Melee,
        level = 1, quality = 0, enabled = true,
    }
    env.player.mainSkill = calcs.createActiveSkill(defaultEffect, { }, env.player)
    t_insert(env.player.activeSkillList, env.player.mainSkill)
end
```

**Translation notes:**
- `gemInstance["enableGlobal"..index]` → `gem.enable_global1` (index 1) and
  `gem.enable_global2` (index 2). The Rust struct already has these fields.
- `group.noSupports` → `Skill::no_supports` is not in the Rust struct yet.
- The `activeSkill.slotName` assignment is important for `SkillCfg.slotName` which
  gates `SocketedIn` mod tags. Currently the Rust sets `slot_name` from the group
  but not correctly for item-granted skills.

### CalcSetup.lua — Phase 6: Group Property Mods (lines 1678–1693)

```lua
-- After all active skills are created, apply GroupProperty mods.
for _, value in ipairs(env.modDB:List(groupCfg, "GroupProperty")) do
    env.player.modDB:AddMod(modLib.setSource(value.value, groupCfg.slotName or ""))
end
-- GroupProperty mods come from items that affect entire socket groups
-- (e.g. "Socketed Gems have X% increased Attack Speed").
-- These must be added to the player modDB, not the skill modDB.
```

### CalcSetup.lua — Phase 7: Build Skill Mod Lists (lines 1756–1759)

```lua
-- Final step: build the skillModList for every active skill.
-- This calls calcs.buildActiveSkillModList(env, activeSkill) for each.
for _, activeSkill in pairs(env.player.activeSkillList) do
    calcs.buildActiveSkillModList(env, activeSkill)
end
```

This is a separate loop from creation — ALL active skills are created first,
then ALL their mod lists are built. Order matters because some skills reference
others (e.g., triggered skills know their trigger skill).

---

### CalcActiveSkill.lua — `createActiveSkill` (lines 82–161)

Creates the `activeSkill` table from an active effect + support list. The critical
work here is the two-pass skill type propagation.

```lua
function calcs.createActiveSkill(activeEffect, supportList, actor, socketGroup, summonSkill)
    local activeSkill = {
        activeEffect = activeEffect,   -- the active gem's granted effect + level
        supportList = supportList,     -- pre-filtered support effects (NOT yet filtered for support-compatibility)
        actor = actor,                 -- env.player or a minion
        socketGroup = socketGroup,     -- the Skill (socket group) this came from
        skillData = { },               -- key→value bag populated throughout
        buffList = { },                -- global-effect buffs extracted from this skill
    }
    local activeGrantedEffect = activeEffect.grantedEffect

    -- skillTypes: numeric enum → bool map.
    -- Copied from the gem's grantedEffect.skillTypes.
    -- The table key is a SkillType.* integer (e.g. SkillType.Attack = 1).
    activeSkill.skillTypes = copyTable(activeGrantedEffect.skillTypes)
    -- In Rust: ActiveSkill::skill_types is Vec<String>, but the full version
    -- should be HashMap<u32, bool> mirroring Lua's table keyed by SkillType enum.

    -- skillFlags: string → bool map (e.g. "attack", "spell", "projectile").
    -- Copied from the gem's baseFlags table.
    local skillFlags = copyTable(activeGrantedEffect.baseFlags)
    activeSkill.skillFlags = skillFlags
    -- "hit" is implied by: explicit hit flag, Attack type, Damage type, or Projectile type.
    skillFlags.hit = skillFlags.hit
        or activeSkill.skillTypes[SkillType.Attack]
        or activeSkill.skillTypes[SkillType.Damage]
        or activeSkill.skillTypes[SkillType.Projectile]

    -- effectList: ordered list of all effects (active + compatible supports).
    -- Starts with just [activeEffect].
    activeSkill.effectList = { activeEffect }

    -- ── Pass 1: Add skill types from compatible supports ──────────────────
    -- Some supports ADD skill types to the active skill.
    -- Classic example: Blasphemy Support adds HasReservation + other types to
    -- curse skills, making them reserving auras.
    -- This must happen BEFORE pass 2 because a support's compatibility may depend
    -- on types added by another support.

    local rejectedSupportsIndices = {}
    for index, supportEffect in ipairs(supportList) do
        if supportEffect.grantedEffect.support then
            if calcLib.canGrantedEffectSupportActiveSkill(supportEffect.grantedEffect, activeSkill) then
                for _, skillType in pairs(supportEffect.grantedEffect.addSkillTypes) do
                    activeSkill.skillTypes[skillType] = true  -- numeric SkillType key
                end
            else
                t_insert(rejectedSupportsIndices, index)
            end
        end
    end

    -- Iterative re-evaluation: after type additions, some previously-rejected
    -- supports may now be compatible. Repeat until no new supports are added.
    local notAddedNewSupport = true
    repeat
        notAddedNewSupport = true
        for index, supportEffectIndex in ipairs(rejectedSupportsIndices) do
            local supportEffect = supportList[supportEffectIndex]
            if supportEffect.grantedEffect.support then
                if calcLib.canGrantedEffectSupportActiveSkill(supportEffect.grantedEffect, activeSkill) then
                    notAddedNewSupport = false
                    rejectedSupportsIndices[index] = nil  -- remove from rejected list
                    for _, skillType in pairs(supportEffect.grantedEffect.addSkillTypes) do
                        activeSkill.skillTypes[skillType] = true
                    end
                end
            end
        end
    until (notAddedNewSupport)
    -- ← This is a FIXED-POINT loop. In practice 1-2 iterations max.

    -- ── Pass 2: Add compatible supports to effectList ─────────────────────
    for _, supportEffect in ipairs(supportList) do
        if supportEffect.grantedEffect.support then
            if calcLib.canGrantedEffectSupportActiveSkill(supportEffect.grantedEffect, activeSkill) then
                t_insert(activeSkill.effectList, supportEffect)  -- appended in gem order
                -- addFlags: support adds flags to skill (e.g. Remote Mine adds 'mine',
                -- Spell Totem adds 'totem', Trap adds 'trap').
                if supportEffect.grantedEffect.addFlags and not summonSkill then
                    for k in pairs(supportEffect.grantedEffect.addFlags) do
                        skillFlags[k] = true
                    end
                end
            end
        end
    end

    return activeSkill
end
```

**Translation notes for `createActiveSkill`:**
- `skillTypes` is a `HashMap<u32, bool>` in Lua (keyed by SkillType integer enum).
  Current Rust `ActiveSkill::skill_types` is `Vec<String>`. For full parity, the
  Rust type should support both representations: the string list for display AND
  a numeric-keyed set for the `skillTypes[SkillType.X]` lookups in `buildActiveSkillModList`.
- `grantedEffect.addSkillTypes` → `GemData::add_skill_types: Vec<String>` (already in struct).
  In Lua these are numeric SkillType constants; in the data pipeline they're stored as
  integers and must be compared to the `activeSkill.skillTypes` integer keys.
- `grantedEffect.addFlags` → not in current `GemData` struct. Needs to be added.
  These are string → bool tables like `{mine = true}` or `{totem = true}`.
- `canGrantedEffectSupportActiveSkill` (CalcTools.lua:85–144): checks `excludeSkillTypes`
  (must fail type expression match), `requireSkillTypes` (must pass type expression match),
  weapon type compatibility. See the "can_support function" section below.
- The `summonSkill` parameter gates the `addFlags` application. For minion skills
  (e.g. "Default Attack" used by a Raise Zombie), `addFlags` from supports are NOT applied.

---

### CalcActiveSkill.lua — `buildActiveSkillModList` (lines 227–843)

Builds `activeSkill.skillModList` by merging the actor's modDB with all support
gem modifier contributions. Also finalizes `skillCfg`, `skillData`, and extracts
buff entries.

```lua
function calcs.buildActiveSkillModList(env, activeSkill)
    local skillTypes = activeSkill.skillTypes
    local skillFlags = activeSkill.skillFlags

    -- ── Mode flags ─────────────────────────────────────────────────────────
    -- Propagate env mode flags into skillFlags.
    -- This gates conditional mods that check skillFlags.buffs / .combat / .effective.
    if env.mode_buffs    then skillFlags.buffs    = true end
    if env.mode_combat   then skillFlags.combat   = true end
    if env.mode_effective then skillFlags.effective = true end
```

```lua
    -- ── Multi-part skill handling (lines 246–268) ─────────────────────────
    -- Some skills have "parts" (e.g. Glacial Cascade has impact + shockwave,
    -- each with different skillFlags). The active part is determined by
    -- srcInstance.skillPart (user-configurable in MAIN mode).
    -- In CALCS mode, skillPartCalcs is used instead.
    local activeGemParts = activeGrantedEffect.parts
    if activeGemParts and #activeGemParts > 1 then
        activeSkill.skillPart = m_min(#activeGemParts, srcInstance.skillPart or 1)
        local part = activeGemParts[activeSkill.skillPart]
        for k, v in pairs(part) do
            if v == true  then skillFlags[k] = true  end
            if v == false then skillFlags[k] = nil   end
        end
        skillFlags.multiPart = #activeGemParts > 1
    end
    -- Rust: ActiveSkill has no skillPart field yet. Multi-part skills need
    -- a part index stored (default 1). When the config allows selecting parts,
    -- this comes from BuildConfig.
```

```lua
    -- ── Shield requirement check (line 270) ──────────────────────────────
    if (skillTypes[SkillType.RequiresShield] or skillFlags.shieldAttack)
       and not actor.itemList["Weapon 2"] then
        skillFlags.disable = true
        activeSkill.disableReason = "This skill requires a Shield"
    end
    -- Rust: Check if skill has RequiresShield type AND env.player.has_shield == false.
```

```lua
    -- ── Weapon flag resolution (lines 281–331) ───────────────────────────
    -- Spectral Shield Throw special case: shieldAttack uses weapon2 (the shield).
    if skillFlags.shieldAttack then
        skillFlags.weapon2Attack = true
        activeSkill.weapon2Flags = 0
    else
        -- Collect weapon type restrictions from active skill + all supports
        local weaponTypes = { activeGrantedEffect.weaponTypes }
        for _, skillEffect in pairs(activeSkill.effectList) do
            if skillEffect.grantedEffect.support and skillEffect.grantedEffect.weaponTypes then
                t_insert(weaponTypes, skillEffect.grantedEffect.weaponTypes)
            end
        end
        -- getWeaponFlags: checks if the equipped weapon type is in weaponTypes,
        -- returns bit-OR of ModFlag.Weapon | ModFlag.Weapon1H/2H | ModFlag.WeaponMelee/Ranged
        -- | the specific weapon type flag (ModFlag.Axe etc.)
        local weapon1Flags, weapon1Info = getWeaponFlags(env, actor.weaponData1, weaponTypes)
        if weapon1Flags then
            -- Weapon is compatible with this skill
            if skillFlags.attack or skillFlags.dotFromAttack then
                activeSkill.weapon1Flags = weapon1Flags
                skillFlags.weapon1Attack = true
                -- Melee weapon clears projectile flag; ranged clears melee flag
                if weapon1Info.melee and skillFlags.melee then
                    skillFlags.projectile = nil
                elseif not weapon1Info.melee and skillFlags.projectile then
                    skillFlags.melee = nil
                end
            end
        elseif (DualWieldOnly or MainHandOnly or forceMainHand or weapon1Info) then
            -- Required weapon not equipped
            skillFlags.disable = true
        end
        -- Similar check for weapon2 (off-hand): omitted for brevity.
        -- Key result: activeSkill.weapon1Flags, activeSkill.weapon2Flags,
        -- skillFlags.weapon1Attack, skillFlags.weapon2Attack, skillFlags.bothWeaponAttack
    end
    -- Rust: The weapon flag resolution feeds into skillModFlags which determines
    -- whether ModFlag.Weapon, ModFlag.Axe, etc. are set in skillCfg.flags.
    -- The current Rust code in build_skill_cfg() is much simpler and misses this.
```

```lua
    -- ── skillModFlags (lines 340–362) ────────────────────────────────────
    -- Build integer bit-OR of ModFlag constants for the skillCfg.flags field.
    local skillModFlags = 0
    if skillFlags.hit      then skillModFlags = bor(skillModFlags, ModFlag.Hit)    end
    if skillFlags.attack   then skillModFlags = bor(skillModFlags, ModFlag.Attack) end
    else
        skillModFlags = bor(skillModFlags, ModFlag.Cast)  -- spells and non-attacks always get Cast
        if skillFlags.spell then skillModFlags = bor(skillModFlags, ModFlag.Spell) end
    end
    if skillFlags.melee     then skillModFlags = bor(skillModFlags, ModFlag.Melee)      end
    elseif skillFlags.projectile then
        skillModFlags = bor(skillModFlags, ModFlag.Projectile)
        skillFlags.chaining = true  -- projectile skills implicitly chain (relevant for Mirage Archer etc.)
    end
    if skillFlags.area      then skillModFlags = bor(skillModFlags, ModFlag.Area)       end
    -- NOTE: ModFlag.Cast is set for ALL non-attack skills, not just spells.
    -- Current Rust only sets ModFlags::SPELL for spells. ModFlags::CAST is missing.
```

```lua
    -- ── skillKeywordFlags (lines 363–421) ────────────────────────────────
    -- Build integer bit-OR of KeywordFlag constants for skillCfg.keywordFlags.
    local skillKeywordFlags = 0
    if skillFlags.hit                      then skillKeywordFlags |= KeywordFlag.Hit      end
    if skillTypes[SkillType.Aura]          then skillKeywordFlags |= KeywordFlag.Aura     end
    if skillTypes[SkillType.AppliesCurse]  then skillKeywordFlags |= KeywordFlag.Curse    end
    if skillTypes[SkillType.Warcry]        then skillKeywordFlags |= KeywordFlag.Warcry   end
    if skillTypes[SkillType.Movement]      then skillKeywordFlags |= KeywordFlag.Movement end
    if skillTypes[SkillType.Vaal]          then skillKeywordFlags |= KeywordFlag.Vaal     end
    if skillTypes[SkillType.Lightning]     then skillKeywordFlags |= KeywordFlag.Lightning end
    if skillTypes[SkillType.Cold]          then skillKeywordFlags |= KeywordFlag.Cold     end
    if skillTypes[SkillType.Fire]          then skillKeywordFlags |= KeywordFlag.Fire     end
    if skillTypes[SkillType.Chaos]         then skillKeywordFlags |= KeywordFlag.Chaos    end
    if skillTypes[SkillType.Physical]      then skillKeywordFlags |= KeywordFlag.Physical end
    if skillFlags.weapon1Attack
       and band(weapon1Flags, ModFlag.Bow) ~= 0 then
        skillKeywordFlags |= KeywordFlag.Bow  -- bow attack keywords
    end
    if skillFlags.brand  then skillKeywordFlags |= KeywordFlag.Brand  end
    if skillFlags.arrow  then skillKeywordFlags |= KeywordFlag.Arrow  end
    if skillFlags.totem  then skillKeywordFlags |= KeywordFlag.Totem
    elseif skillFlags.trap  then skillKeywordFlags |= KeywordFlag.Trap
    elseif skillFlags.mine  then skillKeywordFlags |= KeywordFlag.Mine
    elseif not skillTypes[SkillType.Triggered] then
        skillFlags.selfCast = true  -- not totem/trap/mine/triggered → selfcast
    end
    if skillTypes[SkillType.Attack]        then skillKeywordFlags |= KeywordFlag.Attack   end
    if skillTypes[SkillType.Spell]         then skillKeywordFlags |= KeywordFlag.Spell    end
    -- Rust: KeywordFlags already has most of these constants. The SkillType → KeywordFlag
    -- mapping must be driven by the gem's skillTypes (integer keys), not string flags.
```

```lua
    -- ── skillCfg construction (lines 446–470) ────────────────────────────
    activeSkill.skillCfg = {
        flags = bor(skillModFlags, activeSkill.weapon1Flags or activeSkill.weapon2Flags or 0),
        keywordFlags = skillKeywordFlags,
        -- skillName strips "Vaal " prefix so Vaal versions match their non-Vaal counterparts
        skillName = activeGrantedEffect.name:gsub("^Vaal ", ""),
        summonSkillName = summonSkill and summonSkill.activeEffect.grantedEffect.name,
        skillGem = activeEffect.gemData,
        skillGrantedEffect = activeGrantedEffect,
        skillPart = activeSkill.skillPart,
        skillTypes = activeSkill.skillTypes,  -- passed through for SkillType tag eval
        skillCond = { },
        skillDist = env.mode_effective and effectiveRange,  -- for proximity shield calc
        slotName = activeSkill.slotName or activeEffect.gemCfg.slotName,
        socketColor = activeEffect.gemCfg.socketColor,
        socketNum = activeEffect.gemCfg.socketNum,
    }
    -- Per-weapon configs for dual-wielding calculations:
    if skillFlags.weapon1Attack then
        activeSkill.weapon1Cfg = copyTable(activeSkill.skillCfg)
        activeSkill.weapon1Cfg.skillCond = { MainHandAttack = true }  -- metatable
        activeSkill.weapon1Cfg.flags = bor(skillModFlags, weapon1Flags)
    end
    if skillFlags.weapon2Attack then
        activeSkill.weapon2Cfg = copyTable(activeSkill.skillCfg)
        activeSkill.weapon2Cfg.skillCond = { OffHandAttack = true }
        activeSkill.weapon2Cfg.flags = bor(skillModFlags, weapon2Flags)
    end
    -- Rust: SkillCfg already has flags/keyword_flags/slot_name/skill_name.
    -- Missing: weapon1Cfg/weapon2Cfg, skillCond map, skillDist, socketNum/socketColor,
    -- skillTypes passthrough (for SkillType tag in eval_mod).
```

```lua
    -- ── Initialize skillModList (lines 474–483) ───────────────────────────
    -- skillModList is a new ModList that chains to the actor's modDB.
    -- All skill-specific lookups go through skillModList first, then fall through
    -- to the actor's global modDB.
    local skillModList = new("ModList", activeSkill.actor.modDB)
    activeSkill.skillModList = skillModList
    activeSkill.baseSkillModList = skillModList

    -- Minion damage fixup: if the actor is a minion with damageFixup stat,
    -- apply a MORE modifier to reduce base attack damage and increase speed.
    if actor.minionData and actor.minionData.damageFixup then
        skillModList:NewMod("Damage", "MORE", -100 * damageFixup, "Damage Fixup", ModFlag.Attack)
        skillModList:NewMod("Speed", "MORE",  100 * damageFixup, "Damage Fixup", ModFlag.Attack)
    end
    -- Rust: skill_mod_db is currently initialized as ModDb::new() (empty).
    -- It must be initialized as a child/overlay of actor.mod_db so that queries
    -- fall through to the actor's global modDB. The Rust ModDb currently does
    -- not have a parent-chain mechanism — this needs to be added.
```

```lua
    -- ── Disable check (lines 487–498) ─────────────────────────────────────
    if skillModList:Flag(skillCfg, "DisableSkill")
       and not skillModList:Flag(skillCfg, "EnableSkill") then
        skillFlags.disable = true
        activeSkill.disableReason = "Skills of this type are disabled"
    end
    -- Note the curse application exemption (Gruthkul's Pelt special case):
    -- item-granted curse skills with noSupports + triggered are exempt from DisableSkill.
    -- This is a rare edge case; the basic disable check is what matters.

    if skillFlags.disable then
        wipeTable(skillFlags)
        skillFlags.disable = true
        -- Short circuit: disabled skills don't get mod lists built.
        return
    end
```

```lua
    -- ── Support gem modifiers (lines 500–531) ────────────────────────────
    for _, skillEffect in pairs(activeSkill.effectList) do
        if skillEffect.grantedEffect.support then
            -- Merge stat-scaled support modifiers into skillModList
            calcs.mergeSkillInstanceMods(env, skillModList, skillEffect)

            local level = skillEffect.grantedEffect.levels[skillEffect.level]
            -- manaMultiplier: increases mana cost of the supported skill
            if level.manaMultiplier then
                skillModList:NewMod("SupportManaMultiplier", "MORE",
                    level.manaMultiplier, skillEffect.grantedEffect.modSource)
            end
            -- manaReservationPercent: overrides the supported skill's reservation
            -- (e.g. Blasphemy Support: sets skillData.manaReservationPercent = 35)
            if level.manaReservationPercent then
                activeSkill.skillData.manaReservationPercent = level.manaReservationPercent
            end
            -- isTrigger: the support triggers the active skill
            if skillEffect.grantedEffect.isTrigger then
                if activeSkill.triggeredBy then
                    -- Multiple triggers → disable skill
                    skillFlags.disable = true
                    activeSkill.disableReason = "This skill is supported by more than one trigger"
                else
                    activeSkill.triggeredBy = skillEffect
                end
            end
        end
    end
    -- Rust: This entire block is missing. skill_mod_db has no parent-chain,
    -- and no support mods are merged into it. triggeredBy is always None.
    -- manaReservationPercent is never populated in skillData.
```

```lua
    -- ── Active gem modifiers (lines 549–552) ──────────────────────────────
    -- Add the level-scaled mods from the active gem itself.
    calcs.mergeSkillInstanceMods(env, skillModList, activeEffect, extraStats)
    activeEffect.grantedEffectLevel = activeGrantedEffect.levels[activeEffect.level]
```

```lua
    -- ── Level data extraction (lines 554–579) ────────────────────────────
    local level = activeEffect.grantedEffectLevel
    activeSkill.skillData.CritChance = level.critChance
    -- damageMultiplier: "x% more Base Attack Damage" (e.g. Heavy Strike at L20 has +44%)
    if level.damageMultiplier then
        skillModList:NewMod("Damage", "MORE", level.damageMultiplier,
            activeEffect.grantedEffect.modSource, ModFlag.Attack)
    end
    if level.attackTime   then activeSkill.skillData.attackTime = level.attackTime end
    if level.attackSpeedMultiplier then
        activeSkill.skillData.attackSpeedMultiplier = level.attackSpeedMultiplier
    end
    if level.cooldown     then activeSkill.skillData.cooldown = level.cooldown end
    if level.storedUses   then activeSkill.skillData.storedUses = level.storedUses end
    if level.vaalStoredUses then
        -- vaalStoredUses adds to existing storedUses (Lua: `or 0 + n` → always n if nil)
        -- WARNING: This Lua expression has a precedence bug: `a or 0 + b` = `a or (0+b)`.
        -- When skillData.storedUses is nil, result is just level.vaalStoredUses (correct).
        -- When skillData.storedUses is set, result is skillData.storedUses (NOT added).
        -- Reproduce the buggy behavior exactly.
        activeSkill.skillData.storedUses = activeSkill.skillData.storedUses or 0 + level.vaalStoredUses
    end
    if level.soulPreventionDuration then
        activeSkill.skillData.soulPreventionDuration = level.soulPreventionDuration
    end
    -- Rust: All of these need to go into skill_data HashMap.
    -- Most importantly: CritChance drives base_crit_chance in OFF-03.
    -- attackTime replaces the gem-data cast_time for attacks (milliseconds in data).
```

```lua
    -- ── ExtraSkillMod (lines 583–587) ────────────────────────────────────
    -- SkillData mods from the modDB that should be added directly to the skill mod list.
    activeSkill.extraSkillModList = { }
    for _, value in ipairs(skillModList:List(activeSkill.skillCfg, "ExtraSkillMod")) do
        skillModList:AddMod(value.mod)
        t_insert(activeSkill.extraSkillModList, value.mod)
    end
```

```lua
    -- ── SkillData from modDB (lines 634–640) ─────────────────────────────
    -- Extract all SkillData mods from both the global modDB and the skillModList.
    for _, value in ipairs(env.modDB:List(activeSkill.skillCfg, "SkillData")) do
        activeSkill.skillData[value.key] = value.value
    end
    for _, value in ipairs(skillModList:List(activeSkill.skillCfg, "SkillData")) do
        activeSkill.skillData[value.key] = value.value
    end
    -- These populate manaReservationPercent (from the active gem's own data),
    -- hasReservation flag, and many other skill-specific properties.
    -- They come from the gem's statMap entries in grantedEffect.
```

```lua
    -- ── GlobalEffect separation (lines 772–842) ──────────────────────────
    -- Mods tagged with GlobalEffect (buffs, auras, curses) are removed from
    -- skillModList and placed into activeSkill.buffList.
    -- Each buff entry has: type (Buff/Aura/Curse/etc.), name, modList.
    -- These are later applied to the env in CalcPerform.
    local i = 1
    while skillModList[i] do
        local effectType, effectName, effectTag
        for _, tag in ipairs(skillModList[i]) do
            if tag.type == "GlobalEffect" then
                effectType = tag.effectType
                effectName = tag.effectName or activeGrantedEffect.name
                break
            end
        end
        if effectType then
            -- Find or create buff entry
            local buff = findOrCreateBuff(activeSkill.buffList, effectType, effectName)
            -- Merge this mod into the buff's modList (BASE/INC types sum)
            mergeIntoBuff(buff.modList, skillModList[i])
            t_remove(skillModList, i)  -- ← extracted from skillModList
        else
            i = i + 1
        end
    end
    if activeSkill.buffList[1] then
        t_insert(env.auxSkillList, activeSkill)  -- add to auxiliary skill list
    end
    -- Rust: This entire GlobalEffect separation is missing.
    -- Buff extraction is required for auras/curses (PERF-06).
    -- For SETUP-02 itself, it's enough to stub this as "remove GlobalEffect mods
    -- from skill_mod_db" without building the full buff infrastructure.
```

---

### `mergeSkillInstanceMods` (CalcActiveSkill.lua:52–78)

```lua
-- Merges level-scaled mods from a gem's statMap into the given modList.
-- This is how "at gem level 20, Fireball does X fire damage" becomes actual mods.
function calcs.mergeSkillInstanceMods(env, modList, skillEffect, extraStats)
    calcLib.validateGemLevel(skillEffect)
    local grantedEffect = skillEffect.grantedEffect
    -- Compute the actual stat values for this level/quality combination
    local stats = calcLib.buildSkillInstanceStats(skillEffect, grantedEffect)
    -- stats is a {statName → value} table, e.g. {fire_damage = 250}
    for stat, statValue in pairs(stats) do
        local map = grantedEffect.statMap[stat]
        -- statMap translates game stat names to modifier entries
        -- map can be: a single mod, a group of mods, or nil (unmapped stat)
        if map then
            for _, modOrGroup in ipairs(map) do
                if modOrGroup.name then
                    -- Single mod: apply directly with computed value
                    mergeLevelMod(modList, modOrGroup, map.value or statValue * mult / div + base)
                else
                    -- Group of mods: apply each with group-level scaling
                    for _, mod in ipairs(modOrGroup) do
                        mergeLevelMod(modList, mod, groupValue)
                    end
                end
            end
        end
    end
    modList:AddList(grantedEffect.baseMods)  -- level-independent base mods
end
-- Rust: This function is the heart of "gem level data → modifiers" translation.
-- calcLib.buildSkillInstanceStats computes stats interpolating between gem levels.
-- The statMap is part of the GrantedEffect data loaded from game files.
-- Currently the Rust active_skill.rs uses a simplified level-data approach that
-- reads pre-computed level rows (phys_min, fire_min, etc.) instead of going through
-- the statMap translation pipeline. This is a structural difference.
-- For SETUP-02, the minimum viable approach is to use level row data directly
-- and treat statMap translation as a future refinement.
```

---

### `canGrantedEffectSupportActiveSkill` (CalcTools.lua:85–144)

```lua
function calcLib.canGrantedEffectSupportActiveSkill(grantedEffect, activeSkill)
    -- 1. Hard disqualifiers
    if grantedEffect.unsupported then return false end
    if activeSkill.activeEffect.grantedEffect.cannotBeSupported then return false end
    -- 2. Support-gems-only: must be socketed gem (not item-granted)
    if grantedEffect.supportGemsOnly and not activeSkill.activeEffect.gemData then
        return false
    end
    -- 3. Item-granted support cannot support item-granted active skill
    if grantedEffect.fromItem and grantedEffect.support
       and activeSkill.activeEffect.grantedEffect.fromItem then
        return false
    end
    -- 4. effectiveSkillTypes: use summonSkill's types for minion skill checking
    local effectiveSkillTypes = activeSkill.summonSkill
        and activeSkill.summonSkill.skillTypes
        or activeSkill.skillTypes
    -- 5. excludeSkillTypes: if type expression matches → reject
    if grantedEffect.excludeSkillTypes[1]
       and doesTypeExpressionMatch(grantedEffect.excludeSkillTypes, effectiveSkillTypes) then
        return false
    end
    -- 6. Trigger support cannot support a hostile minion skill
    if grantedEffect.isTrigger and activeSkill.actor.enemy.player ~= activeSkill.actor then
        return false
    end
    -- 7. Weapon type restriction (for Wisps Support / Varunastra)
    if grantedEffect.weaponTypes then
        -- Check activeSkill's weapon types vs support's weapon types
        -- Varunastra (countsAsAll1H) grants all one-handed melee types
        -- → typeMatch must succeed or reject
    end
    -- 8. requireSkillTypes: must match type expression
    return not grantedEffect.requireSkillTypes[1]
        or doesTypeExpressionMatch(grantedEffect.requireSkillTypes,
                                   effectiveSkillTypes, effectiveMinionTypes)
end
-- Rust: The current can_support() in active_skill.rs handles only require/exclude
-- as string lists with eq_ignore_ascii_case. Missing:
--   - unsupported / cannotBeSupported guards
--   - supportGemsOnly guard
--   - fromItem collision guard
--   - summonSkill type delegation
--   - weaponTypes check
--   - Type expressions (doesTypeExpressionMatch can handle AND/OR/NOT logic)
```

---

## Existing Rust Code

### `crates/pob-calc/src/calc/active_skill.rs` (full file)

**What exists:**
- `can_support()`: checks `require_skill_types` and `exclude_skill_types` as simple
  string lists with `eq_ignore_ascii_case`. Missing the 6 guards listed above.
- `build_skill_cfg()`: builds a `SkillCfg` from `ActiveSkill` fields. Sets `flags`
  and `keyword_flags` from `is_attack`, `is_spell`, `is_melee`. Misses weapon type
  flags, `ModFlags::CAST`, and the full SkillType → KeywordFlag mapping.
- `set_skill_conditions()`: sets `UsingAttack`, `UsingSpell`, `UsingMelee`,
  `DualWielding`, `UsingShield` on the player modDB. This is correct but incomplete.
- `run()`: resolves one active skill from `build.skill_sets[active_skill_set]` into
  `env.player.main_skill`. Does NOT populate `env.player.active_skill_list`.

**What's missing (high-level):**
1. The entire two-phase support collection loop (Phase 4 above). Currently just
   takes explicit gems from the socket group without ExtraSupport, cross-linked,
   or item-granted skill groups.
2. Iteration over all socket groups to build `active_skill_list` (Phase 5 above).
   `active_skill_list` is always an empty `Vec`.
3. `build_active_skill_mod_list()` equivalent — no support mods are merged into
   `skill_mod_db`, no `manaReservationPercent` is populated, no `triggeredBy`.
4. The `skill_mod_db` parent-chain mechanism. Currently it's an empty `ModDb::new()`.
5. `addBestSupport` deduplication logic (awakened vs. base gem dedup by `plusVersionOf`).
6. The SkillType → KeywordFlag mapping for `skillTypes` (integer keys).
7. GlobalEffect buff extraction from `skillModList`.

### `crates/pob-calc/src/calc/env.rs` (relevant parts)

- `Actor::active_skill_list: Vec<ActiveSkill>` exists (line 106) but is **never populated**.
- `Actor::main_skill: Option<ActiveSkill>` is populated by the current `active_skill.rs::run()`.

### `crates/pob-calc/src/build/types.rs` (relevant parts)

- `ActiveSkill` struct (line 129): has `skill_mod_db`, `skill_types: Vec<String>`,
  `skill_flags: HashMap<String, bool>`, `skill_cfg: Option<SkillCfg>`,
  `support_list: Vec<SupportEffect>`, `triggered_by: Option<String>`.
  **Missing:** `skill_data: HashMap<String, f64>`, `buff_list`, `skill_part: Option<u32>`,
  `weapon1_flags`, `weapon2_flags`, `weapon1_cfg`, `weapon2_cfg`, `no_supports`,
  `disable_reason`, `slot_name` (exists but as `Option<String>` — correct).
- `SupportEffect` struct (line 493): has `skill_id`, `level`, `quality`, `gem_data: Option<String>`.
  **Missing:** `granted_effect` reference, `is_supporting` tracking, `superseded: bool`.
- `Gem` struct (line 111): has `enable_global1`, `enable_global2` — correct.
  **Missing:** `no_supports: bool`, `granted_effect: Option<...>` for item-granted skills.
- `Skill` struct (line 103): has `slot`, `enabled`, `main_active_skill`, `gems`.
  **Missing:** `source: Option<String>` (for item-granted skill groups), `no_supports: bool`,
  `slot_enabled: bool`.

---

## What Needs to Change

The following is an ordered implementation plan. Items 1–6 are the MVP to get
`active_skill_list` populated and downstream chunks unblocked.

### 1. Add `skill_data` to `ActiveSkill`
```rust
// In build/types.rs
pub struct ActiveSkill {
    // … existing fields …
    pub skill_data: HashMap<String, f64>,  // replaces individual fields
    pub skill_part: Option<u32>,           // for multi-part skills
    pub buff_list: Vec<BuffEntry>,         // GlobalEffect entries extracted from skill_mod_db
    pub weapon1_flags: u32,               // ModFlags bit-OR for main-hand attack
    pub weapon2_flags: u32,               // ModFlags bit-OR for off-hand attack
    pub no_supports: bool,                 // item-granted skills that block supports
    pub disable_reason: Option<String>,    // "This skill requires a Shield" etc.
}
```

The existing individual fields (`base_crit_chance`, `cast_time`, `attack_speed_base`,
`base_damage`) can stay as convenience values populated from `skill_data` entries.

### 2. Add `source` and `no_supports` to `Skill`
```rust
// In build/types.rs
pub struct Skill {
    // … existing fields …
    pub source: Option<String>,  // "Item:Heretic's Veil" etc. — item-granted groups
    pub no_supports: bool,        // prevents support gems from applying
    pub slot_enabled: bool,       // whether the weapon set is active
}
```

### 3. Implement `addBestSupport` dedup logic in Rust
In `active_skill.rs`, implement:
```rust
fn add_best_support(effect: SupportEffect, list: &mut Vec<SupportEffect>, gems: &GemsMap)
```
Rules:
- If same `granted_effect` id already in list: keep higher level/quality.
- If new effect's `plus_version_of` == existing effect's `id`: replace (awakened wins).
- If existing effect's `plus_version_of` == new effect's `id`: skip new (already have awakened).
- Otherwise: append.

This requires adding `plus_version_of: Option<String>` to `GemData`.

### 4. Implement the two-phase support collection (Phase 4 of CalcSetup)

In `active_skill.rs::run()`, before creating active skills:

```rust
// For each enabled socket group:
//   For each enabled gem in the group that is a support:
//     Create a SupportEffect
//     Call add_best_support into the group's support list
//
// Also handle ExtraSupport mods from item modDB:
//   Query mod_db.list_cfg(slot_cfg, "ExtraSupport") → item-implicit supports
//   (e.g. Heretic's Veil Blasphemy at level 22)
//   addBestSupport these into the slot's support list
```

### 5. Implement the active skill creation loop (Phase 5 of CalcSetup)

In `active_skill.rs::run()`, iterate all socket groups and for each non-support,
non-disabled gem:
- Build `activeEffect` with level, quality, `enable_global1/2` gating
- Collect `appliedSupportList` from Phase 4 output
- Call `create_active_skill(activeEffect, appliedSupportList, …)`
- Append to `env.player.active_skill_list`
- For the `main_socket_group`, set `env.player.main_skill`

### 6. Implement `create_active_skill` Rust equivalent

Port `calcs.createActiveSkill` (CalcActiveSkill.lua:82–161):
- Copy `skill_types` from gem data (as `HashMap<u32, bool>` or string-keyed set)
- Set `skill_flags` from gem base flags
- Set `skillFlags.hit` from `Attack | Damage | Projectile` type membership
- Two-pass support type propagation (fixed-point loop for `addSkillTypes`)
- Collect `effectList` of compatible supports from `appliedSupportList`

### 7. Implement `build_active_skill_mod_list` Rust equivalent

Port `calcs.buildActiveSkillModList` (CalcActiveSkill.lua:227–843):

7a. **Mode flags** → set `skill_flags.buffs/combat/effective` from `env.mode_*`.

7b. **skillModFlags** → compute `ModFlags` bitfield from skill flags (attack, spell,
   cast, melee, projectile, area, weapon type). Crucially add `ModFlags::CAST` for
   all non-attack skills.

7c. **skillKeywordFlags** → compute `KeywordFlags` from `skill_types` integer keys
   mapping to `KeywordFlag.*` constants.

7d. **`skill_cfg` construction** → replace current `build_skill_cfg()` with the
   full construction from line 446. Include weapon type flags in `cfg.flags`.

7e. **`skill_mod_db` parent chain** → `skill_mod_db` must "inherit" from `actor.mod_db`
   so queries fall through. Options:
   - Add a parent `Option<&ModDb>` reference to `ModDb`.
   - Or copy actor's mods into skill_mod_db at construction time.
   The Lua uses `new("ModList", actor.modDB)` which is a chained lookup.
   The minimum viable approach is to copy all of `actor.mod_db` into `skill_mod_db`
   at construction and add skill-specific mods on top.

7f. **Support mod merging** → for each support in `effectList`:
   - Merge level-scaled mods via `merge_skill_instance_mods`
   - Apply `manaMultiplier` as `SupportManaMultiplier MORE` mod
   - Populate `skill_data["manaReservationPercent"]` from `level.manaReservationPercent`
   - Detect `isTrigger` and set `triggered_by`

7g. **Active gem mods** → merge the active gem's level-scaled mods via
   `merge_skill_instance_mods`. Populate `skill_data["CritChance"]`, `attackTime`,
   `attackSpeedMultiplier`, `cooldown`, `storedUses`, etc.

7h. **SkillData from modDB** → query `mod_db.list(skill_cfg, "SkillData")` and
   `skill_mod_db.list(skill_cfg, "SkillData")`, insert into `skill_data`.

7i. **GlobalEffect extraction** → scan `skill_mod_db` for mods tagged `GlobalEffect`,
   remove them and place in `buff_list`. (May stub for now; needed fully by PERF-06.)

### 8. Update PERF-04 to iterate `active_skill_list`

After SETUP-02 is complete, the `accumulate_skill_reservations` function in
`perform.rs` must be rewritten to iterate `env.player.active_skill_list` instead
of raw gem XML data. It must use `skill.skill_data.get("manaReservationPercent")`
(set by Blasphemy Support) and `skill.triggered_by.is_some()` (triggered skills
don't reserve).

---

## Critical Gaps Summary

| Gap | Impact | Lua location |
|-----|--------|--------------|
| `active_skill_list` never populated | Blocks PERF-04, OFF-*, TRIG-* | CalcSetup.lua:1657 |
| `skill_mod_db` has no parent chain | All skill-scoped mod queries wrong | CalcActiveSkill.lua:474 |
| Support mods not merged | `manaReservationPercent` always missing | CalcActiveSkill.lua:500–530 |
| `triggeredBy` never set | Triggered skills counted as reserving | CalcActiveSkill.lua:512–519 |
| `ExtraSupport` mods ignored | Heretic's Veil Blasphemy not applied | CalcSetup.lua:1471–1491 |
| `addBestSupport` dedup missing | Duplicate supports, wrong levels | CalcSetup.lua:320–349 |
| `skillTypes` integer keys not used | SkillType→KeywordFlag mapping incomplete | CalcActiveSkill.lua:363–421 |
| `ModFlags::CAST` not set | Non-attack non-spell mods don't match | CalcActiveSkill.lua:348 |
| `addSkillTypes` pass 1+2 loop missing | Blasphemy does not propagate HasReservation | CalcActiveSkill.lua:106–158 |
| `skill_data["CritChance"]` not populated | OFF-03 uses wrong base crit | CalcActiveSkill.lua:556 |
| `skill_data["attackTime"]` not populated | OFF-04 uses wrong cast time | CalcActiveSkill.lua:561 |
| GlobalEffect buff extraction missing | Aura/curse mods not separated | CalcActiveSkill.lua:772–842 |

## Lua Gotcha Quick Reference (SETUP-02 specific)

| Lua pattern | Rust translation | Notes |
|-------------|------------------|-------|
| `skillTypes[SkillType.Attack]` | `skill_types.contains("Attack")` | SkillType is a numeric enum in Lua; in data pipeline it maps to string names |
| `bor(a, b)` | `a \| b` | `bit.bor` = bitwise OR |
| `band(a, b)` | `a & b` | `bit.band` = bitwise AND |
| `bnot(a)` | `!a` | `bit.bnot` = bitwise NOT |
| `copyTable(t, true)` | `t.clone()` | `true` = shallow copy |
| `wipeTable(skillFlags)` | `skill_flags.clear()` | Clears all keys |
| `modList:NewMod("X", "MORE", v, src, flag)` | `skill_mod_db.add_mod(...)` | Creates and adds in one call |
| `t_remove(list, i)` | `list.remove(i - 1)` | 1-based Lua vs 0-based Rust |
| `level.manaReservationPercent` | `level.mana_reservation_percent: Option<f64>` | Must be in gem level data struct |
| `a or 0 + b` | `if a.is_some() { a } else { b }` | Precedence bug: `+` binds tighter than `or`. Replicate exact behavior. |
