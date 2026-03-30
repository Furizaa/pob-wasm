//! Internal AST types for the ModParser code generator.

/// A raw Lua pattern string, before translation to Rust regex.
#[derive(Debug, Clone)]
pub struct LuaPattern(pub String);

/// Form types from formList (INC, RED, MORE, LESS, BASE, FLAG, etc.)
#[derive(Debug, Clone, PartialEq)]
pub enum FormType {
    Inc,
    Red,
    More,
    Less,
    Base,
    Gain,
    Lose,
    Grants,
    Removes,
    Chance,
    Flag,
    TotalCost,
    BaseCost,
    Pen,
    RegenFlat,
    RegenPercent,
    DegenFlat,
    DegenPercent,
    Degen,
    Dmg,
    DmgAttacks,
    DmgSpells,
    DmgBoth,
    Override,
    Doubled,
}

/// A parsed entry from formList.
#[derive(Debug, Clone)]
pub struct FormEntry {
    pub pattern: LuaPattern,
    pub form: FormType,
}

/// A parsed entry from modNameList.
#[derive(Debug, Clone)]
pub struct ModNameEntry {
    pub key: String,
    pub names: Vec<String>,
    pub tags: Vec<LuaTag>,
}

/// A tag extracted from Lua source, not yet translated to Rust.
#[derive(Debug, Clone)]
pub struct LuaTag {
    pub tag_type: String,
    pub fields: Vec<(String, String)>,
}

/// A parsed entry from modFlagList.
#[derive(Debug, Clone)]
pub struct ModFlagEntry {
    pub key: String,
    pub flags: Vec<String>,
    pub keyword_flags: Vec<String>,
    pub tags: Vec<LuaTag>,
}

/// A parsed entry from preFlagList.
#[derive(Debug, Clone)]
pub struct PreFlagEntry {
    pub pattern: LuaPattern,
    pub flags: Vec<String>,
    pub keyword_flags: Vec<String>,
    pub tags: Vec<LuaTag>,
    pub add_to_minion: bool,
    pub add_to_skill: bool,
    pub add_to_aura: bool,
    pub new_aura: bool,
    pub apply_to_enemy: bool,
    pub func_body: Option<String>,
}

/// A parsed entry from modTagList.
#[derive(Debug, Clone)]
pub struct ModTagEntry {
    pub pattern: LuaPattern,
    pub tags: Vec<LuaTag>,
    pub func_body: Option<String>,
}

/// Classification result for a specialModList entry.
#[derive(Debug, Clone)]
pub enum SpecialModTemplate {
    StaticMods(Vec<LuaModCall>),
    SimpleFn(Vec<LuaModCall>),
    HelperCall {
        helper: String,
        args: Vec<String>,
    },
    DamageConversion {
        stat_prefix: String,
        capture_index: usize,
    },
    DamageGainAs {
        stat_prefix: String,
        capture_index: usize,
    },
    NumericScaling {
        mod_call: LuaModCall,
        factor: f64,
    },
    EnemyModifier(Vec<LuaModCall>),
    MinionModifier(Vec<LuaModCall>),
    ManualRequired {
        lua_body: String,
        line_number: usize,
    },
}

/// A parsed mod() or flag() call from Lua source.
#[derive(Debug, Clone)]
pub struct LuaModCall {
    pub name: String,
    pub mod_type: String,
    pub value: String,
    pub flags: Option<String>,
    pub keyword_flags: Option<String>,
    pub tags: Vec<LuaTag>,
    pub dynamic_name: bool,
}

/// A parsed specialModList entry.
#[derive(Debug, Clone)]
pub struct SpecialModEntry {
    pub pattern: LuaPattern,
    pub template: SpecialModTemplate,
    pub line_number: usize,
}

/// All parsed data from ModParser.lua.
#[derive(Debug)]
pub struct ParsedModParser {
    pub forms: Vec<FormEntry>,
    pub mod_names: Vec<ModNameEntry>,
    pub mod_flags: Vec<ModFlagEntry>,
    pub pre_flags: Vec<PreFlagEntry>,
    pub mod_tags: Vec<ModTagEntry>,
    pub special_mods: Vec<SpecialModEntry>,
}
