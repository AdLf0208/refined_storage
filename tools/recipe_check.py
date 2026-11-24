#!/usr/bin/env python3
"""Craft every recipe this package carries, in a real game.

Slice 2's in-game proof (AGENTS.md "Parity rule"): each recipe is crafted the way
it declares, its output id and count are asserted, and the ingredients are shown
to be consumed. The recipe table is read from the package's own
`data/refinedstorage/recipe/*.json`, so a carried recipe cannot be dead data.

Called by tools/e2e.sh with the open game's control socket:

    recipe_check.py SOCK CTL TABLE_X TABLE_Y TABLE_Z

Shaped and shapeless recipes run in a crafting table (`screen` slots 100-108
row-major, result 200); smelting recipes run in a furnace placed one block east
of the table (`container` inserts into the input slot, then the fuel slot).

Assertions, per recipe:
  1. the result slot holds exactly the declared id and count;
  2. a shift-click on the result leaves the 3x3 grid empty (the ingredients were
     consumed) and the crafted stack in the player's inventory;
  3. counterfactuals: the same grid with one ingredient replaced by a non-ingredient
     yields no result (shaped, shapeless), and a furnace with a non-ingredient
     smelts nothing — so the assertions can fail.

`refinedstorage:storage_disk_upgrade` (slice 12) is a **mod crafting kind**, not a
vanilla one: its file names only the destination variant (`to`), and the two
ingredients are derived from it — a *smaller* disk and that tier's storage part.
Its branch crafts the previous tier's disk with this tier's part, asserts the
output, asserts that the source disk's **own** part comes back in the disk's cell
after the take (upstream's `getRemainingItems`), and runs two controls: the same
tier's disk (not smaller) and the source tier's own part (no recipe names it as a
destination).

Exits 1 on the first mismatch, printing `FAIL: …` (the e2e's trap prints EXIT:1).
"""
import json
import pathlib
import subprocess
import sys

GRID = [100 + i for i in range(9)]  # row-major: 100,101,102 = the top row
RESULT = 200

# The tag -> a real item that carries it, taken from the package's own tag files
# when they have a plain entry; this fallback covers a tag whose only values are
# other tags.
TAG_FALLBACK = {
    "c:glass_blocks": "minecraft:glass",
    "c:stones": "minecraft:stone",
    "c:chests": "minecraft:chest",
    "c:strings": "minecraft:string",
    "c:slime_balls": "minecraft:slime_ball",
    "c:dusts/redstone": "minecraft:redstone",
    "c:dusts/glowstone": "minecraft:glowstone_dust",
    "c:gems/quartz": "minecraft:quartz",
    "c:gems/diamond": "minecraft:diamond",
    "c:ingots/iron": "minecraft:iron_ingot",
    "c:ingots/gold": "minecraft:gold_ingot",
    "c:ingots/copper": "minecraft:copper_ingot",
    "c:silicon": "refinedstorage:silicon",
    # This package's own tags (slice 4's wireless grid tests #refinedstorage:grids).
    "refinedstorage:grids": "refinedstorage:grid",
}

# A vanilla item that is NOT in the tag above, for the tag counterfactual: with
# one of these in place of the tag's item the recipe must not match.
TAG_IMPOSTOR = {
    "c:stones": "minecraft:dirt",
    "c:glass_blocks": "minecraft:dirt",
    "c:chests": "minecraft:dirt",
    "c:strings": "minecraft:dirt",
    "c:slime_balls": "minecraft:dirt",
    "c:dusts/redstone": "minecraft:dirt",
    "c:dusts/glowstone": "minecraft:dirt",
    "c:gems/quartz": "minecraft:dirt",
    "c:gems/diamond": "minecraft:dirt",
    "c:ingots/iron": "minecraft:dirt",
    "c:ingots/gold": "minecraft:dirt",
    "c:ingots/copper": "minecraft:dirt",
    "c:silicon": "minecraft:dirt",
    "refinedstorage:grids": "minecraft:dirt",
}


class Ctl:
    def __init__(self, sock, binary):
        self.sock = sock
        self.binary = binary

    def __call__(self, *args):
        out = subprocess.run(
            [self.binary, "--socket", self.sock, *map(str, args)],
            text=True,
            capture_output=True,
        )
        try:
            return json.loads(out.stdout)
        except json.JSONDecodeError:
            raise SystemExit(f"ctl {' '.join(map(str, args))}: {out.stdout}{out.stderr}")

    def wait(self, ticks=2):
        self("wait", ticks)

    def slots(self):
        screen = self("screen")
        if "slots" not in screen:
            raise SystemExit(f"no menu is open: {screen}")
        return screen["slots"]


def load_tags(data):
    """Every `#tag` in the package's data, mapped to one concrete item id.

    Any namespace: `c:`, `minecraft:` and this package's own (`refinedstorage:`)
    tags alike, so a recipe testing one of the mod's own tags resolves here.
    """
    tags = {}
    for namespace in sorted(p.name for p in data.iterdir() if (p / "tags" / "item").is_dir()):
        root = data / namespace / "tags" / "item"
        for path in sorted(root.rglob("*.json")):
            tag = f"{namespace}:" + path.relative_to(root).with_suffix("").as_posix()
            values = json.loads(path.read_text())["values"]
            plain = [v for v in values if not v.startswith("#")]
            if plain:
                tags[tag] = plain[0]
    return tags


def ingredient_id(spec, tags):
    """A recipe's ingredient as (`item-id`, `#tag or None`)."""
    if isinstance(spec, str):
        if spec.startswith("#"):
            tag = spec[1:]
            return (tags.get(tag) or TAG_FALLBACK[tag], tag)
        return (spec, None)
    if "item" in spec:
        return (spec["item"], None)
    tag = spec["tag"]
    return (tags.get(tag) or TAG_FALLBACK[tag], tag)


def main():
    if len(sys.argv) < 6:
        raise SystemExit(__doc__)
    sock, ctl_path, tx, ty, tz = sys.argv[1], sys.argv[2], *map(int, sys.argv[3:6])
    fx, fy, fz = tx + 2, ty, tz  # the furnace, two blocks east of the table
    here = pathlib.Path(__file__).resolve().parent.parent
    data = here / "data"
    recipes = sorted((data / "refinedstorage" / "recipe").rglob("*.json"))
    if not recipes:
        raise SystemExit(f"no recipes under {data / 'refinedstorage' / 'recipe'}")
    tags = load_tags(data)
    ctl = Ctl(sock, ctl_path)
    failures = []

    def fail(message):
        failures.append(message)
        print(f"FAIL: {message}", file=sys.stderr)

    def give(item, count=1):
        ctl("command", "gamemode creative @s")
        ctl("give", item, count)

    def find(item):
        for slot in ctl.slots():
            if slot.get("item") == item:
                return slot["slot"]
        return None

    def open_table():
        """Open the table's menu, asking again when a click did not register.

        A right click on the block is a press/release pair; on a loaded box one
        occasionally reaches the server without opening the menu (measured
        while another leg ran on the same machine), and the run then died on a
        missing menu rather than on a recipe. Three asks, then the failure.
        """
        opened = None
        for _ in range(6):
            opened = ctl("interact", tx, ty, tz)
            ctl.wait(3)
            try:
                if any(slot["slot"] == RESULT for slot in ctl.slots()):
                    return
            except SystemExit:
                pass
            ctl.wait(4)
        raise SystemExit(f"the crafting table did not open (no result slot): {opened}")

    def reset_table():
        """An empty table, an empty inventory, and its menu open."""
        try:
            ctl("close")
        except SystemExit:
            pass
        ctl.wait(2)
        ctl("setblock", tx, ty, tz, "minecraft:air")
        ctl("setblock", tx, ty, tz, "minecraft:crafting_table")
        ctl.wait(2)
        ctl("command", f"tp @s {tx - 0.5} {ty} {tz + 1.5}")
        ctl.wait(2)
        ctl("command", "clear @s")
        ctl.wait(1)
        open_table()

    def grid():
        return [s for s in ctl.slots() if s["slot"] in GRID]

    def result():
        stack = [s for s in ctl.slots() if s["slot"] == RESULT]
        return (stack[0]["item"], stack[0]["count"]) if stack and stack[0].get("item") else (None, 0)

    def place(item, cells):
        """Right-click one unit of ITEM into each cell; the rest goes back."""
        slot = find(item)
        if slot is None:
            fail(f"no {item} in the inventory to place")
            return
        ctl("click-slot", slot)
        ctl.wait(1)
        for cell in cells:
            ctl("click-slot", cell, "right")
            ctl.wait(1)
        ctl("click-slot", slot)
        ctl.wait(2)

    def count(item):
        """How many of ITEM the player carries (not the grid's or the result's)."""
        return sum(
            s.get("count", 0)
            for s in ctl.slots()
            if s.get("item") == item and s["slot"] not in GRID and s["slot"] != RESULT
        )

    def craft(label, cells_per_item, want):
        """GIVE the ingredients, PLACE them in cells_per_item, assert the result."""
        reset_table()
        for item, cells in cells_per_item:
            give(item, len(cells))
            ctl.wait(2)
            place(item, cells)
        got_item, got_count = result()
        print(f"  {label} -> {got_item} x{got_count}")
        if (got_item, got_count) != (want[0], want[1]):
            fail(f"{label}: expected {want[0]} x{want[1]}, got {got_item} x{got_count}")
            return
        # Take it: a shift-click on the result moves it to the inventory and,
        # because the grid held exactly one recipe's worth, empties the grid.
        ctl("click-slot", RESULT, "shift")
        ctl.wait(3)
        leftovers = [s for s in grid() if s.get("item")]
        if leftovers:
            fail(f"{label}: the grid still holds {json.dumps(leftovers)} after crafting (ingredients were not consumed)")
        if count(want[0]) < want[1]:
            fail(f"{label}: the crafted {want[0]} is not in the inventory ({count(want[0])} of {want[1]})")

    def expect_empty(label):
        nonlocal controls
        controls += 1
        got_item, got_count = result()
        print(f"  {label} (control) -> {got_item} x{got_count}")
        if got_item is not None:
            fail(f"{label} control: the wrong ingredient still crafted {got_item} x{got_count}")

    crafted = 0
    controls = 0
    for path in recipes:
        recipe = json.loads(path.read_text())
        kind = recipe["type"]
        name = path.stem
        if kind == "refinedstorage:storage_disk_upgrade":
            # A mod kind's file names only `to`; its result is that disk.
            out = f"refinedstorage:{recipe['to']}_storage_disk"
            out_count = 1
        else:
            out = recipe["result"]["id"]
            out_count = recipe["result"].get("count", 1)
        print(f"recipe {name} ({kind})")
        if kind == "minecraft:crafting_shaped":
            cells_per_item = {}
            for row, line in enumerate(recipe["pattern"]):
                for col, key in enumerate(line):
                    if key == " ":
                        continue
                    item, tag = ingredient_id(recipe["key"][key], tags)
                    cells_per_item.setdefault((item, tag), []).append(GRID[row * 3 + col])
            craft(
                name,
                [(item, cells) for (item, _), cells in cells_per_item.items()],
                (out, out_count),
            )
            crafted += 1
            # Counterfactual: replace the first tag ingredient with a non-member.
            # The tag is read from the resolved ingredient (the pin's recipe shape
            # spells it as a "#tag" string), not from the jar's `{"tag": …}` dict.
            tag = next((t for (_, t) in cells_per_item if t), None)
            if tag:
                impostor = TAG_IMPOSTOR.get(tag)
                cells = [c for (_, t), cs in cells_per_item.items() if t == tag for c in cs]
                if impostor and cells:
                    reset_table()
                    for (item, t), cs in cells_per_item.items():
                        give(impostor if t == tag else item, len(cs))
                        ctl.wait(2)
                        place(impostor if t == tag else item, cs)
                    expect_empty(f"{name} with {impostor} for #{tag}")
        elif kind == "minecraft:crafting_shapeless":
            cells_per_item = []
            for index, spec in enumerate(recipe["ingredients"]):
                item, tag = ingredient_id(spec, tags)
                cells_per_item.append(((item, tag), [GRID[index]]))
            craft(
                name,
                [(item, cells) for (item, _), cells in cells_per_item],
                (out, out_count),
            )
            crafted += 1
            # Counterfactual: the first ingredient doubled instead of the second.
            if len(recipe["ingredients"]) >= 2:
                first, _ = ingredient_id(recipe["ingredients"][0], tags)
                second, second_tag = ingredient_id(recipe["ingredients"][1], tags)
                impostor = TAG_IMPOSTOR.get(second_tag) if second_tag else "minecraft:dirt"
                if impostor and impostor != second:
                    reset_table()
                    give(first, 2)
                    give(impostor, 1)
                    ctl.wait(2)
                    place(first, [GRID[0]])
                    place(impostor, [GRID[1]])
                    expect_empty(f"{name} with {impostor} for {second}")
        elif kind == "refinedstorage:storage_disk_upgrade":
            # A mod crafting kind: the file names only `to`, and upstream derives
            # the pair from it — a smaller disk (any finite variant below `to`,
            # the previous tier being one member) and `to`'s storage part.
            tiers = ["1k", "4k", "16k", "64k"]
            index = tiers.index(recipe["to"])
            source = f"refinedstorage:{tiers[index - 1]}_storage_disk"
            source_part = f"refinedstorage:{tiers[index - 1]}_storage_part"
            part = f"refinedstorage:{recipe['to']}_storage_part"
            reset_table()
            give(source, 1)
            ctl.wait(2)
            place(source, [GRID[0]])
            give(part, 1)
            ctl.wait(2)
            place(part, [GRID[1]])
            got_item, got_count = result()
            print(f"  {name} -> {got_item} x{got_count} (from {source} + {part})")
            if (got_item, got_count) != (out, out_count):
                fail(f"{name}: expected {out} x{out_count}, got {got_item} x{got_count}")
                continue
            made = next(s for s in ctl.slots() if s["slot"] == RESULT)
            print(f"  {name} result components: {made.get('components')}")
            ctl("click-slot", RESULT, "shift")
            ctl.wait(3)
            # The take consumes both ingredients and returns the *source* disk's own
            # part into the cell it left: that is the rule this kind exists for.
            carried = [(s["item"], s["count"]) for s in grid() if s.get("item")]
            print(f"  {name} grid after the take: {carried}")
            if carried != [(source_part, 1)]:
                fail(
                    f"{name}: the grid after the craft holds {carried}, expected only "
                    f"{source_part} x1 (the source disk's own part)"
                )
            if count(out) < out_count:
                fail(f"{name}: the crafted {out} is not in the inventory ({count(out)} of {out_count})")
            crafted += 1
            # Controls: the destination's own size is not smaller, and the source
            # tier's own part names no recipe for that disk (a smaller disk plus a
            # smaller part is not an upgrade — and the other tiers' parts would be
            # their own recipes, so they are not counterfactuals).
            controls_here = [
                (
                    f"{name} with its own tier's disk",
                    f"refinedstorage:{recipe['to']}_storage_disk",
                    part,
                ),
                (
                    f"{name} with the source tier's own part",
                    source,
                    source_part,
                ),
            ]
            for label, control_disk, control_part in controls_here:
                reset_table()
                give(control_disk, 1)
                ctl.wait(2)
                place(control_disk, [GRID[0]])
                give(control_part, 1)
                ctl.wait(2)
                place(control_part, [GRID[1]])
                expect_empty(label)
        elif kind == "minecraft:smelting":
            item, tag = ingredient_id(recipe["ingredient"], tags)
            # A fresh furnace: input in slot 0, fuel in slot 1, 210 ticks to cook.
            ctl("close")
            ctl("setblock", fx, fy, fz, "minecraft:air")
            ctl("setblock", fx, fy, fz, "minecraft:furnace")
            ctl.wait(3)
            ctl("container", fx, fy, fz, item, 1)
            ctl.wait(2)
            ctl("container", fx, fy, fz, "minecraft:coal", 1)
            ctl.wait(230)
            slots = [s if s is not None else {} for s in ctl("container", fx, fy, fz)["slots"]]
            got_item = slots[2].get("item")
            got_count = slots[2].get("count", 0)
            print(f"  {name} -> {got_item} x{got_count}")
            if (got_item, got_count) != (out, out_count):
                fail(f"{name}: expected {out} x{out_count} from the furnace, got {got_item} x{got_count}")
            if slots[0].get("item") is not None:
                fail(f"{name}: the furnace still holds its input {slots[0]}")
            crafted += 1
            # Counterfactual: a non-ingredient does not smelt.
            ctl("setblock", fx, fy, fz, "minecraft:air")
            ctl("setblock", fx, fy, fz, "minecraft:furnace")
            ctl.wait(3)
            ctl("container", fx, fy, fz, "minecraft:dirt", 1)
            ctl.wait(2)
            ctl("container", fx, fy, fz, "minecraft:coal", 1)
            ctl.wait(230)
            slots = [s if s is not None else {} for s in ctl("container", fx, fy, fz)["slots"]]
            print(f"  {name} (control: dirt) -> {slots[2].get('item')}")
            controls += 1
            if slots[2].get("item") is not None:
                fail(f"{name} control: a furnace smelted {slots[2]['item']} out of dirt")
        else:
            fail(f"{name}: unsupported recipe type {kind} (the driver would silently skip it)")

    ctl("close")
    ctl("command", "clear @s")

    # The tag that selects one of several stones: the recipe must accept a
    # different member of the tag it names (#c:stones), not only the first.
    print("tag differential: machine_casing from cobblestone (#c:stones)")
    reset_table()
    give("refinedstorage:quartz_enriched_iron", 8)
    give("minecraft:cobblestone", 1)
    ctl.wait(3)
    place("refinedstorage:quartz_enriched_iron", [100, 101, 102, 103, 105, 106, 107, 108])
    place("minecraft:cobblestone", [104])
    got_item, got_count = result()
    print(f"  machine_casing from cobblestone -> {got_item} x{got_count}")
    if (got_item, got_count) != ("refinedstorage:machine_casing", 1):
        fail(f"#c:stones does not take cobblestone: got {got_item} x{got_count}")
    ctl("click-slot", RESULT, "shift")
    ctl.wait(3)
    ctl("close")

    if failures:
        print(f"EXIT:1 ({len(failures)} failed)", file=sys.stderr)
        raise SystemExit(1)
    print(
        f"recipe PASS: {len(recipes)} of {len(recipes)} carried recipes craft in game with their declared "
        f"output and count, their ingredients are consumed, {controls} wrong-ingredient controls "
        f"leave the result empty, and #c:stones takes cobblestone as well as stone"
    )


if __name__ == "__main__":
    main()
