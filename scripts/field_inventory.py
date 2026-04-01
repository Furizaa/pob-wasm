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
