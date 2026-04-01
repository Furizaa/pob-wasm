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

        // SETUP-05: Cluster jewel subgraph generation. Cluster jewels generate
        // dynamic sub-tree nodes (large/medium/small) with notables and small
        // passives. Without this, builds using cluster jewels will fail parity
        // checks in every downstream chunk. No direct output fields — verified
        // by re-running PERF-01-attributes which should then pass 30/30 builds.
        "SETUP-05-cluster-jewels" => &[],

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
            "ManaReservedP",
            "LifeReservedP",
        ],

        "PERF-05-buffs" => &[
            "FortifyStacks",
            "FortifyEffect",
            "AilmentWarcryEffect",
            "ActiveTotemLimit",
            "ActiveMineLimit",
            "ActiveTrapLimit",
            "ActiveBrandLimit",
            "ActiveGolemLimit",
            "BannerStage",
        ],

        "PERF-06-aura-curse" => &[
            // Aura-related output fields are module-internal; curses affect enemy.
            // Most aura/curse effects show up in other chunks' fields (resistances, damage, etc.)
            // Placeholder — refined during field-to-Lua mapping.
        ],

        "PERF-07-regen-recharge-leech" => &[
            "LifeRegen",
            "LifeRegenPercent",
            "ManaRegen",
            "ManaRegenPercent",
            "EnergyShieldRegen",
            "EnergyShieldRegenPercent",
            "LifeDegen",
            "LifeDegenRate",
            "NetLifeRegen",
            "NetManaRegen",
            "NetEnergyShieldRegen",
            "LifeLeechRate",
            "ManaLeechRate",
            "EnergyShieldLeechRate",
            "MaxLifeLeechRate",
            "MaxManaLeechRate",
            "MaxEnergyShieldLeechRate",
            "MaxLifeLeechRatePercent",
            "MaxManaLeechRatePercent",
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
            "LifeRecoveryRate",
            "ManaRecoveryRate",
            "EnergyShieldRecoveryRate",
            "LifeRecoveryRateTotal",
            "ManaRecoveryRateTotal",
            "EnergyShieldRecharge",
            "EnergyShieldRechargeDelay",
            "EnergyShieldRechargeRecovery",
            "WardRecharge",
            "WardRechargeDelay",
        ],

        "PERF-08-action-speed-conditions" => &[
            "ActionSpeedMod",
            "MovementSpeedMod",
            "MovementSpeed",
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
            "FireResistOver",
            "ColdResistOver",
            "LightningResistOver",
            "ChaosResistOver",
        ],

        "DEF-02-armour-evasion-es-ward" => &[
            "Armour",
            "ArmourDefense",
            "Evasion",
            "EvasionDefense",
            "EnergyShieldOnBody Armour",
            "ArmourOnBody Armour",
            "ArmourOnHelmet",
            "ArmourOnGloves",
            "ArmourOnBoots",
            "ArmourOnWeapon 1",
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
            // Recovery rates as computed in CalcDefence's pool-based recovery
            // These overlap with PERF-07 fields but are the final computed values
            // after defence-specific adjustments.
        ],

        "DEF-06-ehp" => &[
            "AverageEvadeChance",
            "AverageNotHitChance",
            "AverageBlockChance",
            "AverageSpellBlockChance",
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

        _ => return None,
    })
}

/// Returns all known chunk IDs in dependency order.
pub fn all_chunk_ids() -> &'static [&'static str] {
    &[
        "SETUP-05-cluster-jewels",
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
