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

        // SETUP-02: Active skill construction pipeline. Builds active_skill_list
        // from socket groups: creates active skills, matches supports, applies
        // addSkillTypes, builds skillModList. BLOCKER for PERF-04 (reservation),
        // OFF-* (damage), TRIG-* (triggers). No direct output fields — verified
        // by PERF-04 passing 30/30 after restart.
        "SETUP-02-active-skills" => &[],

        // SETUP-05: Cluster jewel subgraph generation. Cluster jewels generate
        // dynamic sub-tree nodes (large/medium/small) with notables and small
        // passives. Without this, builds using cluster jewels will fail parity
        // checks in every downstream chunk. No direct output fields — verified
        // by re-running PERF-01-attributes which should then pass 30/30 builds.
        "SETUP-05-cluster-jewels" => &[],

        // SETUP-06: Timeless jewel node replacement. Timeless jewels (Glorious Vanity,
        // Lethal Pride, Brutal Restraint, Militant Faith, Elegant Hubris) replace
        // passive nodes within their radius using seed-based lookup tables. Without
        // this, realworld_timeless_jewel will fail every downstream chunk. No direct
        // output fields — verified by PERF-01-attributes passing all builds.
        "SETUP-06-timeless-jewels" => &[],

        // SETUP-07: Anointments & granted passives. Items with "Allocates X" grant
        // notable passives. Mod parser must capture the passive name (currently
        // stubbed as TODO). CalcSetup.lua lines 1230-1239 look up the notable and
        // add it to allocNodes. Affects 4 oracle builds. HIGH PRIORITY.
        "SETUP-07-anointments" => &[],

        // SETUP-08: Radius jewel framework. The two-pass system for Thread of Hope,
        // Intuitive Leap, Impossible Escape, passive effect scaling, and
        // PassiveSkillHasNoEffect. CalcSetup.lua lines 113-210. No radius jewel
        // processing exists in Rust. Affects realworld_coc_trigger. HIGH PRIORITY.
        "SETUP-08-radius-jewels" => &[],

        // SETUP-09: Radius jewel framework — full implementation. This chunk
        // implements the complete two-pass radius jewel processing from
        // CalcSetup.lua lines 113-210: PassiveSkillEffect scaling, suppression
        // checks, and the per-jewel-type callback dispatch. Requires
        // nodes_in_radius data in the passive tree JSON. Affects
        // realworld_coc_trigger (Thread of Hope). HIGH PRIORITY.
        // No direct output fields — correctness is verified by downstream chunks
        // (DEF-01-resistances, PERF-01-attributes, etc.) passing for
        // realworld_coc_trigger.
        "SETUP-09-radius-jewels" => &[],

        // SETUP-10 (was SETUP-09): Mastery selections. 3.16+ tree feature. Mastery nodes have
        // stats replaced by player-selected effect. Stored as masteryEffects on
        // <Spec>. XML parser does not parse this. No oracle builds use masteries
        // (all pre-3.16 trees). MEDIUM PRIORITY.
        "SETUP-09-mastery-selections" => &[],

        // SETUP-10: Keystone merging for non-tree grants. Keystones granted by
        // items via "Keystone" list mod need mergeKeystones() lookup in
        // tree.keystoneMap. CalcSetup.lua / ModTools.lua lines 226-237. MEDIUM.
        "SETUP-10-keystone-merging" => &[],

        // SETUP-11: Item condition & multiplier tracking. Item rarity counts,
        // influence multipliers (ShaperItem, ElderItem), socket counts. CalcSetup.lua
        // lines 1132-1210. Needed for "per shaper item" type mods. MEDIUM.
        "SETUP-11-item-conditions" => &[],

        // SETUP-12: Bandit & Pantheon mods. CalcSetup.lua lines 531-553. All oracle
        // builds use bandit=None and pantheon=None. LOW PRIORITY.
        "SETUP-12-bandit-pantheon" => &[],

        // SETUP-13: Buff mode conditions. CalcSetup.lua lines 444-467. Sets
        // mode_buffs/mode_combat/mode_effective. Oracle assumes EFFECTIVE. LOW.
        "SETUP-13-buff-mode" => &[],

        // SETUP-14: Tattoo / hash overrides. Replace allocated passive nodes with
        // alternative effects via <Overrides> XML. No oracle builds. LOW.
        "SETUP-14-tattoo-overrides" => &[],

        // SETUP-15: Forbidden Flesh/Flame. Two matching jewels grant an ascendancy
        // notable. CalcSetup.lua lines 1242-1258. Shares mechanism with SETUP-07.
        // No oracle builds. LOW — combine with SETUP-07 when implementing.
        "SETUP-15-forbidden-flesh-flame" => &[],

        // SETUP-16: Special unique item handling. Necromantic Aegis, Energy Blade,
        // The Iron Mass, Dancing Dervish, Kalandra's Touch, Widowhail, The Adorned.
        // No oracle builds. LOW — add per-unique as needed.
        "SETUP-16-special-uniques" => &[],

        // ── Tier 1: Attributes & Pools (CalcPerform early) ──
        "PERF-01-attributes" => &[
            "Str",
            "Dex",
            "Int",
            "Omni",
            "ReqStr",
            "ReqDex",
            "ReqInt",
            "ReqStrString",
            "ReqDexString",
            "ReqIntString",
            "ReqStrItem",
            "ReqDexItem",
            "ReqIntItem",
        ],

        "PERF-02-life-mana-es" => &[
            "Life",
            "Mana",
            "EnergyShield",
            "Ward",
            "EnergyShieldRecoveryCap",
            "LifeUnreserved",
            "LifeUnreservedPercent",
            "ManaUnreserved",
            "ManaUnreservedPercent",
            "LifeRecoverable",
            "ManaRecoverable",
        ],

        "PERF-03-charges" => &[
            "PowerCharges",
            "PowerChargesMin",
            "PowerChargesMax",
            "FrenzyCharges",
            "FrenzyChargesMin",
            "FrenzyChargesMax",
            "EnduranceCharges",
            "EnduranceChargesMin",
            "EnduranceChargesMax",
            "SiphoningCharges",
            "ChallengerCharges",
            "BlitzCharges",
            "BlitzChargesMax",
            "BrutalCharges",
            "BrutalChargesMax",
            "BrutalChargesMin",
            "AbsorptionCharges",
            "AbsorptionChargesMax",
            "AbsorptionChargesMin",
            "AfflictionCharges",
            "AfflictionChargesMax",
            "AfflictionChargesMin",
            "BloodCharges",
            "BloodChargesMax",
        ],

        "PERF-04-reservation" => &[
            "ManaReserved",
            "ManaReservedPercent",
            "LifeReserved",
            "LifeReservedPercent",
        ],

        "PERF-05-buffs" => &[
            "FortificationStacks",
            "FortificationEffect",
            "AilmentWarcryEffect",
            "ActiveTotemLimit",
            "ActiveMineLimit",
            "ActiveTrapLimit",
            "ActiveBrandLimit",
        ],

        "PERF-06-aura-curse" => &[
            // EnemyCurseLimit: written in perform.rs after curse processing
            "EnemyCurseLimit",
            // Avoidance fields: CurseAvoidChance (CurseImmune flag or AvoidCurse BASE),
            // SilenceAvoidChance (SilenceImmune flag or copy of CurseAvoidChance)
            "CurseAvoidChance",
            "SilenceAvoidChance",
            // Curse/exposure/wither effects on self (CalcDefence.lua:1586-1588)
            "CurseEffectOnSelf",
            "ExposureEffectOnSelf",
            "WitherEffectOnSelf",
            // Debuff expiration rate/modifier
            "DebuffExpirationRate",
            "DebuffExpirationModifier",
            "showDebuffExpirationModifier",
            // Totem resists (written in calcs.resistances, alongside player resists)
            "TotemFireResist",
            "TotemColdResist",
            "TotemLightningResist",
            "TotemChaosResist",
            "TotemFireResistTotal",
            "TotemColdResistTotal",
            "TotemLightningResistTotal",
            "TotemChaosResistTotal",
            "TotemFireResistOverCap",
            "TotemColdResistOverCap",
            "TotemLightningResistOverCap",
            "TotemChaosResistOverCap",
            "MissingTotemFireResist",
            "MissingTotemColdResist",
            "MissingTotemLightningResist",
            "MissingTotemChaosResist",
            // Enemy crit: written in buildDefenceEstimations
            "EnemyCritChance",
            "EnemyCritEffect",
            // Skill-type specific fields (CalcOffence pass, only written for relevant skills)
            "AuraEffectMod",
            "CurseEffectMod",
            // Enemy stun modifiers (CalcOffence pass)
            "EnemyStunThresholdMod",
            "EnemyStunDuration",
            // Enemy regeneration (CalcOffence pass, from enemyDB INC mods)
            "EnemyLifeRegen",
            "EnemyManaRegen",
            "EnemyEnergyShieldRegen",
            // Reservation DPS (CalcOffence pass)
            "ReservationDpsMultiplier",
            "ReservationDPS",
            // Degen and net regen (CalcDefence buildDefenceEstimations)
            // TotalBuildDegen only written when > 0 (absent in oracle when no degens)
            "TotalBuildDegen",
            // NetLifeRegen/NetManaRegen/NetEnergyShieldRegen only written when TotalBuildDegen > 0
            "NetLifeRegen",
            "NetManaRegen",
            "NetEnergyShieldRegen",
        ],

        "PERF-07-regen-recharge-leech" => &[
            "LifeRegen",
            "LifeRegenPercent",
            "ManaRegen",
            "ManaRegenPercent",
            "EnergyShieldRegen",
            "EnergyShieldRegenPercent",
            "LifeDegen",
            // NetLifeRegen/NetManaRegen/NetEnergyShieldRegen are in PERF-06 (they depend on
            // TotalBuildDegen computed in buildDefenceEstimations, which is a PERF-06 concern).
            "LifeLeechRate",
            "ManaLeechRate",
            "EnergyShieldLeechRate",
            "MaxLifeLeechRate",
            "MaxManaLeechRate",
            "MaxEnergyShieldLeechRate",
            "MaxLifeLeechRatePercent",
            "LifeLeechGainRate",
            "ManaLeechGainRate",
            "EnergyShieldLeechGainRate",
            "LifeLeechDuration",
            "ManaLeechDuration",
            "EnergyShieldLeechDuration",
            "LifeLeechInstances",
            "ManaLeechInstances",
            "EnergyShieldLeechInstances",
            "LifeLeechInstantRate",
            "ManaLeechInstantRate",
            "EnergyShieldLeechInstantRate",
            "LifeOnHitRate",
            "ManaOnHitRate",
            "EnergyShieldOnHitRate",
            "LifeRecoveryRateMod",
            "ManaRecoveryRateMod",
            "EnergyShieldRecoveryRateMod",
            "EnergyShieldRecharge",
            "EnergyShieldRechargeDelay",
            "WardRechargeDelay",
        ],

        "PERF-08-action-speed-conditions" => &[
            "ActionSpeedMod",
            "MovementSpeedMod",
            "EffectiveMovementSpeedMod",
        ],

        // ── Tier 4: Defence (CalcDefence) ──
        "DEF-01-resistances" => &[
            "FireResist",
            "FireResistTotal",
            "FireResistOverCap",
            "ColdResist",
            "ColdResistTotal",
            "ColdResistOverCap",
            "LightningResist",
            "LightningResistTotal",
            "LightningResistOverCap",
            "ChaosResist",
            "ChaosResistTotal",
            "ChaosResistOverCap",
        ],

        "DEF-02-armour-evasion-es-ward" => &[
            "Armour",
            "ArmourDefense",
            "Evasion",
            "EnergyShieldOnBody Armour",
            "ArmourOnBody Armour",
            "ArmourOnHelmet",
            "ArmourOnGloves",
            "ArmourOnBoots",
            "ArmourOnWeapon 2",
            "EvasionOnBody Armour",
            "EvasionOnHelmet",
            "EvasionOnGloves",
            "EvasionOnBoots",
            "EnergyShieldOnHelmet",
            "EnergyShieldOnGloves",
            "EnergyShieldOnBoots",
        ],

        "DEF-03-block-suppression" => &[
            "BlockChance",
            "BlockChanceMax",
            "BlockChanceOverCap",
            "SpellBlockChance",
            "SpellBlockChanceMax",
            "SpellBlockChanceOverCap",
            "BlockEffect",
            "BlockDuration",
            "SpellSuppressionChance",
            "SpellSuppressionChanceOverCap",
            "SpellSuppressionEffect",
        ],

        "DEF-04-damage-reduction-avoidance" => &[
            "PhysicalDamageReduction",
            "BasePhysicalDamageReduction",
            "BasePhysicalDamageReductionWhenHit",
            "BaseFireDamageReduction",
            "BaseFireDamageReductionWhenHit",
            "BaseColdDamageReduction",
            "BaseColdDamageReductionWhenHit",
            "BaseLightningDamageReduction",
            "BaseLightningDamageReductionWhenHit",
            "BaseChaosDamageReduction",
            "BaseChaosDamageReductionWhenHit",
            "AttackDodgeChance",
            "AttackDodgeChanceOverCap",
            "SpellDodgeChance",
            "SpellDodgeChanceOverCap",
            "BlindAvoidChance",
            "AvoidPhysicalDamageChance",
            "AvoidFireDamageChance",
            "AvoidColdDamageChance",
            "AvoidLightningDamageChance",
            "AvoidChaosDamageChance",
            "AvoidAllDamageFromHitsChance",
            "AvoidProjectilesChance",
            "BleedAvoidChance",
            "PoisonAvoidChance",
            "IgniteAvoidChance",
            "ShockAvoidChance",
            "FreezeAvoidChance",
            "ChillAvoidChance",
            "ScorchAvoidChance",
            "BrittleAvoidChance",
            "SapAvoidChance",
            "StunAvoidChance",
        ],

        "DEF-05-recovery-in-defence" => &[
            "EnergyShieldRecoveryCap",
            "LifeOnBlock",
            "ManaOnBlock",
            "EnergyShieldOnBlock",
            "EnergyShieldOnSpellBlock",
            "LifeOnSuppress",
            "EnergyShieldOnSuppress",
            "LifeRecoup",
            "ManaRecoup",
            "EnergyShieldRecoup",
            "anyRecoup",
        ],

        "DEF-06-ehp" => &[
            "AverageEvadeChance",
            "AverageNotHitChance",
            "MeleeNotHitChance",
            "ProjectileNotHitChance",
            "SpellNotHitChance",
            "AttackTakenHitMult",
            "SpellTakenHitMult",
            "TotalEHP",
            "PhysicalMaximumHitTaken",
            "FireMaximumHitTaken",
            "ColdMaximumHitTaken",
            "LightningMaximumHitTaken",
            "ChaosMaximumHitTaken",
            "AnyAegis",
            "AnyBypass",
            "AnyGuard",
            "AnySpecificMindOverMatter",
            "AnyTakenReflect",
            "sharedAegis",
            "sharedElementalAegis",
            "sharedGuardAbsorbRate",
            "sharedMindOverMatter",
            "sharedMoMHitPool",
            "sharedManaEffectiveLife",
            "totalEnemyDamage",
            "totalEnemyDamageIn",
            "totalTakenDamage",
            "totalTakenHit",
            "enemySkillTime",
            "enemyBlockChance",
            "noSplitEvade",
            "ehpSectionAnySpecificTypes",
            "specificTypeAvoidance",
            "preventedLifeLoss",
            "preventedLifeLossBelowHalf",
            "preventedLifeLossTotal",
        ],

        // ── Tier 5: Offence (CalcOffence) ──
        "OFF-01-base-damage" => &[
            "AverageDamage",
            "AverageBurstDamage",
            "AverageBurstHits",
            "MainHand.AverageDamage",
            "OffHand.AverageDamage",
        ],

        "OFF-02-conversion" => &[
            // Conversion fields are intermediate — they manifest in per-type damage fields.
            // Placeholder — refined during field-to-Lua mapping.
        ],

        "OFF-03-crit-hit" => &[
            "CritChance",
            "CritMultiplier",
            "CritEffect",
            "CritDegenMultiplier",
            "AccuracyHitChance",
            "MainHand.CritChance",
            "MainHand.CritMultiplier",
            "OffHand.CritChance",
            "OffHand.CritMultiplier",
            "MeleeNotHitChance",
            "ProjectileNotHitChance",
        ],

        "OFF-04-speed-dps" => &[
            "Speed",
            "HitSpeed",
            "HitTime",
            "TotalDPS",
            "TotalDot",
            "MainHand.Speed",
            "MainHand.HitSpeed",
            "OffHand.Speed",
            "OffHand.HitSpeed",
            "AreaOfEffectMod",
            "AreaOfEffectRadius",
            "AreaOfEffectRadiusMetres",
        ],

        "OFF-05-ailments" => &[
            "IgniteChance",
            "IgniteDPS",
            "IgniteDamage",
            "IgniteDuration",
            "IgniteEffMult",
            "BleedChance",
            "BleedDPS",
            "BleedDamage",
            "BleedDuration",
            "BleedEffMult",
            "BleedStackPotential",
            "BleedStacks",
            "BleedStacksMax",
            "BleedRollAverage",
            "PoisonChance",
            "PoisonDPS",
            "PoisonDamage",
            "PoisonDuration",
            "PoisonStacks",
            "PoisonStacksMax",
        ],

        "OFF-06-dot-impale" => &[
            "TotalDot",
            "ImpaleDPS",
            "ImpaleHit",
            "ImpaleModifier",
            "ImpaleStacks",
            "ImpaleStacksMax",
            "impaleStoredHitAvg",
        ],

        "OFF-07-combined-dps" => &[
            "CombinedDPS",
            "CombinedAvg",
            "WithBleedDPS",
            "WithIgniteDPS",
            "WithPoisonDPS",
            "CullingMultiplier",
            "FullDPS",
            "FullDotDPS",
            "WithDotDPS",
        ],

        // ── Tier 6: Triggers & Mirages ──
        "TRIG-01-trigger-rates" => &["TriggerRate", "TriggerTime", "ServerTriggerRate"],

        "TRIG-02-totem-trap-mine" => &[
            "TotemPlacementSpeed",
            "TotemPlacementTime",
            "TotemLife",
            "TrapThrowSpeed",
            "TrapThrowTime",
            "TrapCooldown",
            "MineLayingSpeed",
            "MineLayingTime",
        ],

        "MIR-01-mirages" => &["MirageDPS", "MirageCount"],

        // ── Tier 7: Aggregation ──
        "AGG-01-full-dps" => &["FullDPS", "FullDotDPS"],

        // ── FIX chunks: address gaps found during post-implementation review ──

        // FIX-01: Stat name mismatches in recovery rates and ES recharge.
        // Verified via PERF-07 fields (LifeRecoveryRateMod, ManaRecoveryRateMod,
        // EnergyShieldRecoveryRateMod, EnergyShieldRecharge, EnergyShieldRechargeDelay).
        "FIX-01-stat-name-mismatches" => &[
            "LifeRecoveryRateMod",
            "ManaRecoveryRateMod",
            "EnergyShieldRecoveryRateMod",
            "EnergyShieldRecharge",
            "EnergyShieldRechargeDelay",
        ],

        // FIX-02: Per-slot defence accumulation. Verified via DEF-02 fields.
        "FIX-02-per-slot-defence" => &["Armour", "Evasion", "EnergyShield", "Ward"],

        // FIX-03: Radius jewel per-jewel callbacks. No direct output fields —
        // verified by downstream chunks passing for builds with Thread of Hope etc.
        "FIX-03-radius-jewel-callbacks" => &[],

        // FIX-04: Glorious Vanity normal node LUT. No direct output fields —
        // verified by downstream chunks passing for realworld_timeless_jewel.
        "FIX-04-glorious-vanity-normals" => &[],

        // FIX-05: Tattoo data loading & node replacement. No direct output fields.
        "FIX-05-tattoo-data" => &[],

        // FIX-06: PERF-02 medium gaps. Verified via recovery/recharge/block fields.
        "FIX-06-perf02-medium-gaps" => &[
            "EnergyShieldOnBlock",
            "EnergyShieldOnSpellBlock",
            "EnergyShieldOnSuppress",
            "WardRechargeDelay",
        ],

        // FIX-07: Energy Blade weapon creation. No direct output fields —
        // verified by offence fields for Energy Blade builds.
        "FIX-07-energy-blade" => &[],

        // FIX-08: PERF-02 Mana computation bug. Two builds (phys_melee_slayer and coc_trigger)
        // had Mana ~32 too low because tree-socketed jewel mods (e.g. Watcher's Eye) were not
        // being applied to the player mod_db. Only Mana is verified here.
        //
        // ManaReserved and ManaReservedPercent are intentionally excluded: wand_occultist has a
        // pre-existing PERF-04 Blasphemy/SETUP-02 bug that makes its ManaReserved wrong
        // (expected 1224, got 383 — gap of 841 from missing Blasphemy reservation).
        // That is out of scope for FIX-08 per spec section 5.3.
        //
        // ManaUnreserved / ManaUnreservedPercent are excluded because they depend on the full
        // reservation calculation (PERF-04), which has pre-existing failures: coc_trigger
        // (SupportBloodMagic data pipeline bug), aura_stacker and wand_occultist (Blasphemy/
        // PERF-04 architecture issues). All three are out of scope for FIX-08.
        //
        // The fix verified here: Mana is correct across all 30 builds:
        //   phys_melee_slayer: 672 ✓ (was 640)
        //   coc_trigger:       874 ✓ (was 842)
        "FIX-08-mana-computation" => &["Mana"],

        _ => return None,
    })
}

/// Returns all known chunk IDs in dependency order.
pub fn all_chunk_ids() -> &'static [&'static str] {
    &[
        "SETUP-02-active-skills",
        "SETUP-05-cluster-jewels",
        "SETUP-06-timeless-jewels",
        "SETUP-07-anointments",
        "SETUP-08-radius-jewels",
        "SETUP-09-radius-jewels",
        "SETUP-09-mastery-selections",
        "SETUP-10-keystone-merging",
        "SETUP-11-item-conditions",
        "SETUP-12-bandit-pantheon",
        "SETUP-13-buff-mode",
        "SETUP-14-tattoo-overrides",
        "SETUP-15-forbidden-flesh-flame",
        "SETUP-16-special-uniques",
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
        "FIX-01-stat-name-mismatches",
        "FIX-02-per-slot-defence",
        "FIX-03-radius-jewel-callbacks",
        "FIX-04-glorious-vanity-normals",
        "FIX-05-tattoo-data",
        "FIX-06-perf02-medium-gaps",
        "FIX-07-energy-blade",
        "FIX-08-mana-computation",
    ]
}

/// Returns the names of all 30 realworld oracle builds.
pub fn realworld_build_names() -> Vec<String> {
    let oracle_dir = std::path::Path::new("tests/oracle");
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(oracle_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let fname = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if fname.starts_with("realworld_") && fname.ends_with(".xml") {
                names.push(fname.trim_end_matches(".xml").to_string());
            }
        }
    }
    names.sort();
    names
}
