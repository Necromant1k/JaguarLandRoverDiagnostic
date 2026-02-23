#!/usr/bin/env python3
"""Parse CCF_DATA_X260_201600.exml-decrypted and generate a CCF decode table JSON."""

import xml.etree.ElementTree as ET
import json
import sys

CCF_FILE = "/Users/andrei/JLR_SDD/JLR/SDD/Xml/CCF_DATA_X260_201600.exml-decrypted"

def clean_name(group_name):
    """Convert GROUP_CCF_EUCD_DOORS -> Doors"""
    n = group_name.replace("GROUP_CCF_EUCD_", "").replace("GROUP_EUCD_CCF_", "").replace("GROUP_CCF_", "")
    return n.replace("_", " ").title()

def get_tm_text(elem):
    """Get text from <tm> element — use id attribute as fallback."""
    if elem is None:
        return ""
    return (elem.text or elem.get("id", "")).strip()

def parse():
    tree = ET.parse(CCF_FILE)
    root = tree.getroot()

    # Find the block with CCF data
    result = {}

    for group in root.iter("group"):
        start = group.get("start")
        if start is None:
            continue
        option_id = int(start)
        group_name = group.get("name", "")

        # Get human-readable title
        title_tm = group.find(".//title/tm")
        title = get_tm_text(title_tm) if title_tm is not None else clean_name(group_name)
        if not title or title.startswith("@"):
            title = clean_name(group_name)

        # Get parameters and their values
        values = {}
        for param in group.iter("parameter"):
            for option in param.iter("option"):
                val = option.get("value", "")
                if not val:
                    continue
                # Normalize value to int
                try:
                    val_int = int(val, 16) if val.startswith("0x") else int(val)
                except ValueError:
                    continue
                tm = option.find("tm")
                label = get_tm_text(tm)
                if not label or label.startswith("@"):
                    label = option.get("name", val)
                values[val_int] = label

        result[option_id] = {
            "id": option_id,
            "name": title,
            "group": group_name,
            "values": values,
        }

    return result

if __name__ == "__main__":
    table = parse()
    print(f"Parsed {len(table)} CCF option definitions", file=sys.stderr)

    # Show our specific option IDs
    our_ids = [1,2,3,4,6,7,8,9,10,11,14,15,16,17,18,19,21,22,23,25,27,29,30,31,32,33,34,35,36,
               65,67,68,69,70,71,72,73,77,79,80,81,82,83,84,86,87,88,89,90,91,92,93,94,95,96,
               97,98,99,100,101,102,105,107,108,109,110,111,112,113,114,116,117,119]

    print(f"\nCCF options on this IMC:", file=sys.stderr)
    for oid in our_ids:
        entry = table.get(oid)
        if entry:
            vals = ", ".join(f"0x{k:02X}={v}" for k,v in list(entry["values"].items())[:4])
            print(f"  [{oid:3d}] {entry['name']:<45} {vals}", file=sys.stderr)
        else:
            print(f"  [{oid:3d}] ??? (not in X260 CCF data)", file=sys.stderr)

    # Output JSON decode table (only our IDs)
    output = {str(oid): table[oid] for oid in our_ids if oid in table}
    print(json.dumps(output, indent=2, ensure_ascii=False))
