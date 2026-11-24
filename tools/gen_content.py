#!/usr/bin/env python3
"""Generate the Utility Vest 26.3 content tree from the cached 1.3.1 jar.

Jar (`ports/_shared/licence-cache/utilityvest-1.3.1.jar`, LGPL-3.0-only by Satherov):
five wearable vest items, their models and textures, three lang files, five recipes, five
recipe advancements, the `utilityvest:utility_vest` tag and a `curios:body` tag.

The four `utilityvest:upgrade` recipes use a NeoForge custom serializer that copies the old
vest's id and contents into the upgrade. The SDK has no such serializer, so the port carries
them as plain `minecraft:crafting_shaped` recipes (same shape and ingredients, concrete vanilla
item ids in place of the `#c:` tags). This is recorded as a deviation in `notes.md`. The
`curios:body` tag is not carried: the pin has no Curios, and the vanilla body equipment slot is
written directly by `/utilityvest wear` (`support.rs`).

Usage: python3 tools/gen_content.py            # reads the cached jar, writes the package root
       python3 tools/gen_content.py --check    # verify the tree matches what this script writes
"""

from __future__ import annotations

import argparse
import json
import zipfile
from pathlib import Path

PORT = Path(__file__).resolve().parent.parent
JAR = PORT.parent / "_shared" / "licence-cache" / "utilityvest-1.3.1.jar"

TIERS = ["leather", "iron", "gold", "diamond", "netherite"]
# The shaped recipe of each tier: (pattern, key letter -> vanilla item id).
# The leather recipe is the base; the other four are the upstream "upgrade" recipes.
RECIPES: dict[str, tuple[list[str], dict[str, str]]] = {
    "leather": (
        ["a a", "aba", "ccc"],
        {"a": "minecraft:leather", "b": "minecraft:chest", "c": "minecraft:leather"},
    ),
    "iron": (
        [" a ", "aba", " a "],
        {"a": "minecraft:iron_ingot", "b": "utilityvest:leather_utility_vest"},
    ),
    "gold": (
        [" a ", "aba", " a "],
        {"a": "minecraft:gold_ingot", "b": "utilityvest:iron_utility_vest"},
    ),
    "diamond": (
        [" a ", "aba", " a "],
        {"a": "minecraft:diamond", "b": "utilityvest:gold_utility_vest"},
    ),
    "netherite": (
        [" a ", "aba", " a "],
        {"a": "minecraft:netherite_ingot", "b": "utilityvest:diamond_utility_vest"},
    ),
}


def generated_files() -> dict[str, bytes]:
    """The exact bytes this generator writes, keyed by path relative to the port root."""
    files: dict[str, bytes] = {}
    with zipfile.ZipFile(JAR) as jar:
        # The five item textures and models are carried verbatim (LGPL, no ARR asset split).
        for tier in TIERS:
            name = f"{tier}_utility_vest"
            for prefix, suffix in [("assets/utilityvest/textures/item/", ".png"),
                                   ("assets/utilityvest/models/item/", ".json")]:
                files[prefix + name + suffix] = jar.read(prefix + name + suffix)
        for lang in ("en_us", "ja_jp", "pt_br"):
            files[f"assets/utilityvest/lang/{lang}.json"] = jar.read(
                f"assets/utilityvest/lang/{lang}.json"
            )
        files["data/utilityvest/tags/item/utility_vest.json"] = jar.read(
            "data/utilityvest/tags/item/utility_vest.json"
        )
        for tier in TIERS:
            files[f"data/utilityvest/advancement/recipes/misc/{tier}_utility_vest.json"] = jar.read(
                f"data/utilityvest/advancement/recipes/misc/{tier}_utility_vest.json"
            )

    for tier in TIERS:
        name = f"{tier}_utility_vest"
        item_id = f"utilityvest:{name}"
        # 26.3 item definition -> the model carried from the jar.
        files[f"assets/utilityvest/items/{name}.json"] = json.dumps(
            {"model": {"model": item_id, "type": "minecraft:model"}},
            indent=2, sort_keys=True,
        ).encode() + b"\n"
        # Content item declaration: a one-slot wearable template from vanilla.
        files[f"data/utilityvest/horizon/item/{name}.json"] = json.dumps(
            {
                "template": "minecraft:leather_chestplate",
                "max_stack_size": 1,
                "components": {"minecraft:item_model": item_id},
            },
            indent=2, sort_keys=True,
        ).encode() + b"\n"
        pattern, key = RECIPES[tier]
        files[f"data/utilityvest/recipe/{name}.json"] = json.dumps(
            {
                "type": "minecraft:crafting_shaped",
                "category": "misc",
                "pattern": pattern,
                "key": key,
                "result": {"id": item_id, "count": 1},
            },
            indent=2, sort_keys=True,
        ).encode() + b"\n"
    return files


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="verify the tree matches")
    args = parser.parse_args()
    files = generated_files()
    bad: list[str] = []
    for relative, content in files.items():
        path = PORT / relative
        if args.check:
            if not path.is_file() or path.read_bytes() != content:
                bad.append(relative)
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
    if args.check:
        if bad:
            raise SystemExit("content out of date: " + ", ".join(bad))
        print(f"content check: {len(files)} files match")
    else:
        print(f"wrote {len(files)} content files from {JAR.name}")


if __name__ == "__main__":
    main()
