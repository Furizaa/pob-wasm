#!/usr/bin/env luajit
-- gen_oracle.lua: Run a POB build XML through the POB engine (headless)
-- and output a CalculationResult JSON to stdout.
-- Usage: cd third-party/PathOfBuilding/src && luajit ../../../scripts/gen_oracle.lua <abs-path-to-build.xml>
-- Requires: LuaJIT, and the PathOfBuilding submodule initialized.
--
-- NOTE: Must be run from third-party/PathOfBuilding/src/ because POB uses
-- relative paths throughout (dofile("Launch.lua"), LoadModule("Modules/..."), etc.)
-- Use scripts/run_oracle.sh which handles the directory change automatically.

local xml_path = arg[1]
if not xml_path then
    io.stderr:write("Usage: luajit gen_oracle.lua <build.xml>\n")
    os.exit(1)
end

-- Locate repo root relative to cwd (we assume cwd = third-party/PathOfBuilding/src)
local pob_dir = "."
local runtime_lua_dir = "../runtime/lua"

-- Add runtime lua libs to path (include sha1 subdirectory pattern)
package.path = pob_dir .. "/?.lua;" 
    .. runtime_lua_dir .. "/?.lua;"
    .. runtime_lua_dir .. "/?/init.lua;"
    .. package.path

-- Stub out C modules that are not available in plain LuaJIT
-- lua-utf8: used only for number formatting (thousands separator), safe to stub
package.preload['lua-utf8'] = function()
    local utf8 = {}
    utf8.reverse = string.reverse
    utf8.gsub = string.gsub
    utf8.find = string.find
    utf8.sub = string.sub
    utf8.len = string.len
    utf8.char = string.char
    utf8.byte = string.byte
    return utf8
end

-- lcurl: used for update checking, not needed for calc
package.preload['lcurl'] = function()
    return { easy = function() return {} end }
end
package.preload['lcurl.safe'] = package.preload['lcurl']

-- lzip: used for build import compression
package.preload['lzip'] = function()
    return {
        inflate = function(data) return data end,
        deflate = function(data) return data end,
    }
end

-- Pre-define GetVirtualScreenSize before HeadlessWrapper loads Launch.lua
-- HeadlessWrapper defines GetScreenSize() but not GetVirtualScreenSize()
-- and Launch.lua:394 calls GetVirtualScreenSize() in DrawPopup (called during OnFrame).
-- We define it now so it's available when HeadlessWrapper calls runCallback("OnFrame").
function GetVirtualScreenSize()
    -- HeadlessWrapper.lua:48 defines GetScreenSize() as returning 1920, 1080
    return 1920, 1080
end

-- Redirect ConPrintf to stderr so stdout remains clean JSON.
-- We define this before loading HeadlessWrapper because HeadlessWrapper defines
-- ConPrintf as print() (stdout), and the startup sequence (Launch.lua, OnInit,
-- OnFrame) emits messages like "Loading main script...", "Uniques loaded", etc.
-- HeadlessWrapper.lua will overwrite this with its own version (also print-based),
-- so we override it again after dofile.
function ConPrintf(fmt, ...)
    io.stderr:write(string.format(fmt, ...) .. "\n")
end

-- Also redirect Lua's print() to stderr for any other stray output
local _print = print
print = function(...)
    local args = {...}
    local parts = {}
    for i = 1, select("#", ...) do
        parts[i] = tostring(args[i])
    end
    io.stderr:write(table.concat(parts, "\t") .. "\n")
end

-- Bootstrap POB's headless wrapper (defines globals: loadBuildFromXML, build, etc.)
dofile(pob_dir .. "/HeadlessWrapper.lua")

-- Re-override ConPrintf since HeadlessWrapper redefines it as print()
function ConPrintf(fmt, ...)
    io.stderr:write(string.format(fmt, ...) .. "\n")
end

-- Simple JSON serializer (no external deps needed for basic types)
local function to_json(val)
    local t = type(val)
    if t == "nil" then return "null"
    elseif t == "boolean" then return tostring(val)
    elseif t == "number" then
        if val ~= val then return "null" end -- NaN
        if val == math.huge or val == -math.huge then return "null" end
        return string.format("%.10g", val)
    elseif t == "string" then
        return '"' .. val:gsub('\\','\\\\'):gsub('"','\\"'):gsub('\n','\\n') .. '"'
    elseif t == "table" then
        local is_array = #val > 0
        if is_array then
            local parts = {}
            for _, v in ipairs(val) do
                table.insert(parts, to_json(v))
            end
            return "[" .. table.concat(parts, ",") .. "]"
        else
            local parts = {}
            for k, v in pairs(val) do
                if type(k) == "string" or type(k) == "number" then
                    table.insert(parts, '"' .. tostring(k) .. '":' .. to_json(v))
                end
            end
            table.sort(parts)
            return "{" .. table.concat(parts, ",") .. "}"
        end
    end
    return "null"
end

-- Read the build XML
local f = io.open(xml_path, "r")
if not f then
    io.stderr:write("Cannot open: " .. xml_path .. "\n")
    os.exit(1)
end
local xml_content = f:read("*a")
f:close()

-- Use HeadlessWrapper's loadBuildFromXML to properly initialize the build.
-- This calls mainObject.main:SetMode("BUILD", ...) + runCallback("OnFrame"),
-- which triggers full build initialization including CalcsTab:BuildOutput().
-- After this, the global `build` is set (HeadlessWrapper.lua:201).
loadBuildFromXML(xml_content, "oracle_build")

-- build is now the global set by HeadlessWrapper (line 201):
--   build = mainObject.main.modes["BUILD"]
-- build.calcsTab.mainEnv has the full calculation environment
-- build.calcsTab.mainOutput is env.player.output

local mainEnv = build.calcsTab.mainEnv
local mainOutput = build.calcsTab.mainOutput

-- Collect output (filter out non-serializable values)
local output = {}
for k, v in pairs(mainOutput) do
    local t = type(v)
    if t == "number" or t == "boolean" or t == "string" then
        output[k] = v
    end
end

-- Collect breakdown (only text lines for now)
local breakdown = {}
if mainEnv.player.breakdown then
    for k, v in pairs(mainEnv.player.breakdown) do
        if type(v) == "table" then
            local bd = {}
            local lines = {}
            for _, line in ipairs(v) do
                if type(line) == "string" then
                    table.insert(lines, line)
                end
            end
            if #lines > 0 then bd.lines = lines end
            if next(bd) then breakdown[k] = bd end
        end
    end
end

local result = { output = output, breakdown = breakdown }
io.write(to_json(result))
io.write("\n")
