use mlua::prelude::*;

/// Creates a Lua environment pre-configured for loading PoB data files.
///
/// Sets up:
/// - `package.path` to include the PoB src directory
/// - Stub tables for `SkillType`, `ModFlag`, `KeywordFlag`, `ModType`
/// - Stub helper functions `mod()`, `flag()`, `skill()`
pub fn create_pob_lua_env(pob_src_dir: &str) -> mlua::Result<Lua> {
    let lua = Lua::new();

    // Set package.path so require() can find PoB modules
    {
        let package: LuaTable = lua.globals().get("package")?;
        let current_path: String = package.get("path")?;
        let new_path = format!(
            "{}/?.lua;{}/?/init.lua;{}",
            pob_src_dir, pob_src_dir, current_path
        );
        package.set("path", new_path)?;
    }

    // Stub SkillType enum with auto-incrementing IDs.
    // When Global.lua is loaded later it may override these values.
    lua.load(
        r#"
        SkillType = setmetatable({}, {
            __index = function(t, k)
                local next_id = (rawget(t, "__next") or 1)
                rawset(t, k, next_id)
                rawset(t, "__next", next_id + 1)
                return next_id
            end
        })
        "#,
    )
    .exec()?;

    // Stub ModFlag table
    lua.load(
        r#"
        ModFlag = setmetatable({}, {
            __index = function(t, k)
                local next_id = (rawget(t, "__next") or 1)
                rawset(t, k, next_id)
                rawset(t, "__next", next_id + 1)
                return next_id
            end
        })
        "#,
    )
    .exec()?;

    // Stub KeywordFlag table
    lua.load(
        r#"
        KeywordFlag = setmetatable({}, {
            __index = function(t, k)
                local next_id = (rawget(t, "__next") or 1)
                rawset(t, k, next_id)
                rawset(t, "__next", next_id + 1)
                return next_id
            end
        })
        "#,
    )
    .exec()?;

    // Stub ModType table (e.g. ModType.MOD, ModType.MORE, etc.)
    lua.load(
        r#"
        ModType = setmetatable({}, {
            __index = function(t, k)
                local next_id = (rawget(t, "__next") or 1)
                rawset(t, k, next_id)
                rawset(t, "__next", next_id + 1)
                return next_id
            end
        })
        "#,
    )
    .exec()?;

    // Stub helper functions that PoB data files use
    lua.load(
        r#"
        function mod(...)
            return { type = "mod", args = {...} }
        end

        function flag(...)
            return { type = "flag", args = {...} }
        end

        function skill(...)
            return { type = "skill", args = {...} }
        end
        "#,
    )
    .exec()?;

    Ok(lua)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_env_does_not_panic() {
        // Use a dummy path — we just verify the Lua env initialises
        let lua = create_pob_lua_env("/nonexistent").unwrap();
        // SkillType auto-assigns IDs
        let val: i64 = lua.load("return SkillType.Attack").eval().unwrap();
        assert!(val > 0);
    }
}
