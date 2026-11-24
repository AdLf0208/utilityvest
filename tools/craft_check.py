#!/usr/bin/env python3
"""Craft all five carried vest recipes in a real 26.3 crafting table.

Called by tools/e2e.sh with the open game's control socket. Slots 100..108 are the
3x3 grid (row-major) and 200 the result, as the campaign's other craft checks use.
Each recipe is placed with right-clicks (one item per cell), the parsed result id
and count are asserted, the stack is clicked into the inventory and its count is
read back. A control with the wrong centre item must craft nothing.
"""
import json
import subprocess
import sys

# (tier, ingredient item id, centre item id) — the leather vest is the base and
# the other four are the upstream "upgrade" recipes chained through the tiers.
CASES = [
    ("leather", "minecraft:leather", "minecraft:chest"),
    ("iron", "minecraft:iron_ingot", "utilityvest:leather_utility_vest"),
    ("gold", "minecraft:gold_ingot", "utilityvest:iron_utility_vest"),
    ("diamond", "minecraft:diamond", "utilityvest:gold_utility_vest"),
    ("netherite", "minecraft:netherite_ingot", "utilityvest:diamond_utility_vest"),
]


def main():
    sock, ctl, x, y, z = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]

    def run(*args):
        out = subprocess.run([ctl, "--socket", sock, *args], text=True, capture_output=True)
        try:
            return json.loads(out.stdout)
        except json.JSONDecodeError:
            raise SystemExit(f"ctl {' '.join(args)}: {out.stdout}{out.stderr}")

    def wait(ticks=4):
        run("wait", str(ticks))

    def slots():
        screen = run("screen")
        if "slots" not in screen:
            raise SystemExit(f"no menu is open: {screen}")
        return screen["slots"]

    def find(item):
        for slot in slots():
            if slot.get("item") == item:
                return slot["slot"]
        raise SystemExit(f"missing ingredient {item} in the inventory")

    def place(item, targets):
        slot = find(item)
        run("click-slot", str(slot))
        wait(1)
        for target in targets:
            run("click-slot", str(target), "right")
            wait(1)
        run("click-slot", str(slot))
        wait(2)

    def result():
        for slot in slots():
            if slot["slot"] == 200:
                return slot.get("item"), slot.get("count", 0)
        return None, 0

    def setup(*items):
        try:
            run("close")
        except SystemExit:
            pass
        wait(5)
        run("setblock", x, y, z, "minecraft:air")
        run("setblock", x, y, z, "minecraft:crafting_table")
        wait(2)
        run("command", f"tp @s {float(x) - 0.5} {y} {float(z) + 0.5}")
        wait(3)
        run("command", "clear @s")
        for item, count in items:
            run("give", item, str(count))
        wait(3)
        wait()
        print("interact:", run("interact", x, y, z))
        wait(5)

    failures = []
    for index, (tier, ingredient, centre) in enumerate(CASES):
        item_id = f"utilityvest:{tier}_utility_vest"
        label = f"recipe {tier}"
        if tier == "leather":
            # pattern "a a"/"aba"/"ccc", a=c=leather, b=chest
            setup((ingredient, 8), (centre, 1))
            place(ingredient, ["100", "102", "103", "105", "106", "107", "108"])
            place(centre, ["104"])
        else:
            # pattern " a "/"aba"/" a ", a=ingredient, b=previous vest
            setup((ingredient, 4), (centre, 1))
            place(ingredient, ["101", "103", "105", "107"])
            place(centre, ["104"])
        got_item, got_count = result()
        print(f"{label} -> {got_item} x{got_count}")
        if got_item != item_id or got_count != 1:
            failures.append(f"{label}: expected {item_id} x1, got {got_item} x{got_count}")
            continue
        run("click-slot", "200")
        wait(5)
        run("click-slot", "9")
        wait(3)
        made = run("inventory")["counts"].get(item_id, 0)
        print(f"  {item_id} reached inventory: {made}")
        if made < 1:
            failures.append(f"{label}: the crafted item did not reach the inventory")

    # Control: the upgrade pattern with the wrong centre item crafts nothing.
    setup(("minecraft:iron_ingot", 4), ("minecraft:stick", 1))
    place("minecraft:iron_ingot", ["101", "103", "105", "107"])
    place("minecraft:stick", ["104"])
    got_item, got_count = result()
    print("control (stick centre) ->", got_item, got_count)
    if got_item is not None:
        failures.append(f"control crafted {got_item} x{got_count}")

    if failures:
        print("FAIL: " + "; ".join(failures))
        return 1
    print("PASS: all five vest recipes crafted with their outputs consumed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
