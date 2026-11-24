#!/usr/bin/env bash
# Review round two's two remaining in-game phases (evidence only: 0.2.13 is
# published, so this script proves the delivered package and changes nothing).
#
#   MODE=full    SELFTEST=0  a full drive consumes no disk            -> EXIT:0
#   MODE=full    SELFTEST=1  inverted: it must have consumed one      -> EXIT:1
#   MODE=refused SELFTEST=0  a refused placement consumes nothing and
#                            keeps the payload                         -> EXIT:0
#   MODE=refused SELFTEST=1  inverted: it must have consumed the item  -> EXIT:1
#
# The game is the one the launcher built from the delivered package (GAME/CTL/
# RES/E2E_TMP/SOCK come from `launcher-game.sh env refinedstorage`), so the
# transcript names a package whose bytes are the published ones.
set -euo pipefail
export LC_ALL=C

PORT=$(cd "$(dirname "$0")/.." && pwd)
CTL=${CTL:?set CTL (eval "$(ports/tools/launcher-game.sh env refinedstorage)")}
GAME=${GAME:?set GAME}
RES=${RES:?set RES}
TMP=${E2E_TMP:-/tmp/opencode}
SOCK=${SOCK:-$TMP/refinedstorage-e2e.sock}
MODE=${MODE:-full}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
WORLD=RefinedStorageReview2

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }
case "$MODE" in full|refused) ;; *) echo "MODE must be full or refused" >&2; exit 2 ;; esac

# The delivered bytes this transcript belongs to.
hmod=$PORT/refinedstorage.hmod
[ -f "$hmod" ] || { echo "no delivered package at $hmod" >&2; exit 1; }
echo "package: $(sha256sum "$hmod" | cut -d' ' -f1) $(stat -c %s "$hmod") refinedstorage.hmod"

DRIVE="46 64 22"
DRIVE_X=46; DRIVE_Y=64; DRIVE_Z=22
MACHINE="48 64 22"
MACHINE_X=48

mkdir -p "$TMP" "$(dirname "$SOCK")"
SCRATCH=$(mktemp -d "$TMP/refinedstorage-review2.XXXXXX")
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl_raw() { "$CTL" --socket "$SOCK" "$@"; }
# The same one-retry wrapper the port's e2e uses: a single control request on a
# loaded box can answer nothing, and a dead menu refuses every blocking command.
ctl() {
    local out
    if out=$(ctl_raw "$@"); then printf '%s\n' "$out"; return 0; fi
    case "$out" in
        *screen*) ctl_raw close >/dev/null 2>&1 || true ;;
    esac
    sleep 1
    if out=$(ctl_raw "$@"); then printf '%s\n' "$out"; return 0; fi
    printf '%s\n' "$out"
    return 1
}
fail() { echo "FAIL: $*" >&2; ctl logs --lines 40 >&2 || true; exit 1; }
slots() { ctl container "$1" "$2" "$3" | jq -c '[.slots[] | select(. != null)]'; }
held_json() { ctl inventory | jq -c '.held // {}'; }
# Item entities near a point: the refused click must not spawn one.
items_near() { # items_near X Y Z
    ctl entities 4 item | jq -r --argjson x "$1" --argjson y "$2" --argjson z "$3" \
        '[.entities[] | select(.kind == "item")
          | select((.position[0]-$x) < 2 and (.position[0]-$x) > -2
                   and (.position[1]-$y) < 2 and (.position[1]-$y) > -2
                   and (.position[2]-$z) < 2 and (.position[2]-$z) > -2)]
         | length'
}
cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then echo "kept scratch: $SCRATCH" >&2; else rm -rf "$SCRATCH" 2>/dev/null || true; fi
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() {
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" launch --game "$GAME" --world "$WORLD" --seed 20261105 \
        --creative --offscreen --wait --resources "$RES" >/dev/null
    ctl wait-ready 120
    ctl wait 40
    ctl command "execute in minecraft:the_end run tp @s 40.5 64 3.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5
        if [ "$(ctl block 40 63 0 | jq -r '.loaded // false')" = true ]; then loaded=true; break; fi
    done
    [ "$loaded" = true ] || fail "the End arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill 32 63 -8 56 63 30 minecraft:stone" >/dev/null
    ctl command "execute in minecraft:the_end run fill 32 64 -8 56 74 30 minecraft:air" >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=item]" >/dev/null
    ctl wait 20
    ctl command "execute in minecraft:the_end run tp @s 44.5 64 23.5" >/dev/null
    ctl wait 10
}

echo "== launch ($MODE) =="
launch

if [ "$MODE" = full ]; then
    echo "== a full drive must consume no disk =="
    # Nine disks written into the drive's nine slots in one harness write (the
    # engine decodes `Items` into the mod's slots; slice 11's leg does the same).
    ctl command "execute in minecraft:the_end run setblock $DRIVE_X $DRIVE_Y $DRIVE_Z refinedstorage:disk_drive" >/dev/null
    ctl wait 8
    ctl command "data merge block $DRIVE_X $DRIVE_Y $DRIVE_Z {Items:[{Slot:0b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:1b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:2b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:3b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:4b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:5b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:6b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:7b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:8b,id:\"refinedstorage:1k_storage_disk\",count:1}]}" >/dev/null
    ctl wait 10
    drive_before=$(slots "$DRIVE_X" "$DRIVE_Y" "$DRIVE_Z")
    echo "  the drive before the click: $(jq -c 'length' <<<"$drive_before") disks"
    [ "$(jq 'length' <<<"$drive_before")" = 9 ] \
        || fail "the drive holds $(jq 'length' <<<"$drive_before") disks, expected 9: $drive_before"
    ctl command "gamemode creative" >/dev/null
    ctl command "clear @s" >/dev/null
    ctl give refinedstorage:1k_storage_disk 1 >/dev/null
    ctl wait 6
    ctl select refinedstorage:1k_storage_disk >/dev/null
    ctl wait 4
    selected=$(ctl inventory | jq -r '.selected')
    hand_before=$(held_json)
    echo "  the acting hand (main hand, hotbar slot $selected, the hand ctl interact clicks with) before: $hand_before"
    [ "$(jq -r '.count // 0' <<<"$hand_before")" = 1 ] \
        || fail "the tenth disk is not in the acting hand: $hand_before"
    items_before=$(items_near "$DRIVE_X" "$DRIVE_Y" "$DRIVE_Z")
    echo "  item entities near the drive before the click: $items_before"
    ctl command "execute in minecraft:the_end run tp @s 44.5 64 23.5" >/dev/null
    ctl wait 6
    click=$(ctl interact "$DRIVE_X" "$DRIVE_Y" "$DRIVE_Z" 2>&1 || true)
    ctl wait 10
    drive_after=$(slots "$DRIVE_X" "$DRIVE_Y" "$DRIVE_Z")
    hand_after=$(held_json)
    items_after=$(items_near "$DRIVE_X" "$DRIVE_Y" "$DRIVE_Z")
    echo "  the click: $(jq -c '{changed, inventory_change, screen}' <<<"$(tail -1 <<<"$click")" 2>/dev/null || echo "$click")"
    echo "  the drive after the click: $(jq -c 'length' <<<"$drive_after") disks"
    echo "  the acting hand after the click: $hand_after"
    echo "  item entities near the drive after the click: $items_after"
    [ "$(jq 'length' <<<"$drive_after")" = 9 ] \
        || fail "the full drive took the tenth disk: it now holds $(jq 'length' <<<"$drive_after")"
    [ "$(jq -r '.count // 0' <<<"$hand_after")" = 1 ] \
        || fail "the refused insertion changed the hand: $hand_before -> $hand_after"
    [ "$items_after" = "$items_before" ] \
        || fail "the refused insertion spawned an item entity: $items_before -> $items_after"
    if [ "$SELFTEST" = 1 ]; then
        echo "(selftest: asserting the full drive took the tenth disk)"
        [ "$(jq -r '.count // 0' <<<"$hand_after")" = 0 ] \
            || fail "the full drive refused the tenth disk and the hand still holds it ($hand_after): the refusal is the fix, so this inverted assertion fails as it must"
    fi
    # The differential: the same click with a free slot in the same drive takes
    # exactly one from the same hand.
    ctl command "data merge block $DRIVE_X $DRIVE_Y $DRIVE_Z {Items:[{Slot:0b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:1b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:2b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:3b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:4b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:5b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:6b,id:\"refinedstorage:1k_storage_disk\",count:1},{Slot:7b,id:\"refinedstorage:1k_storage_disk\",count:1}]}" >/dev/null
    ctl wait 10
    free_before=$(slots "$DRIVE_X" "$DRIVE_Y" "$DRIVE_Z")
    echo "  the same drive with one free slot: $(jq -c 'length' <<<"$free_before") disks"
    [ "$(jq 'length' <<<"$free_before")" = 8 ] \
        || fail "the free-slot setup left $(jq 'length' <<<"$free_before") disks, expected 8"
    ctl interact "$DRIVE_X" "$DRIVE_Y" "$DRIVE_Z" >/dev/null 2>&1 || true
    ctl wait 10
    drive_free=$(slots "$DRIVE_X" "$DRIVE_Y" "$DRIVE_Z")
    hand_free=$(held_json)
    echo "  after the same click with a free slot: the drive holds $(jq -c 'length' <<<"$drive_free") disks, the acting hand: $hand_free"
    [ "$(jq 'length' <<<"$drive_free")" = 9 ] \
        || fail "the click with a free slot did not fill it: $(jq -c 'length' <<<"$drive_free") disks"
    [ "$(jq -r '.count // 0' <<<"$hand_free")" = 0 ] \
        || fail "the click with a free slot did not consume the hand's disk: $hand_free"
    echo "PASS: a full drive refused a tenth disk (nine still in the drive, one still in the acting hand, no item entity spawned), while the same click with a free slot took exactly that disk (nine in the drive, an empty hand)"
else
    echo "== a refused placement must consume nothing =="
    # A payload machine: filled, broken in survival, so its item carries six slots.
    ctl command "execute in minecraft:the_end run kill @e[type=item]" >/dev/null 2>&1 || true
    ctl command "execute in minecraft:the_end run setblock $MACHINE_X 64 22 refinedstorage:importer[facing=east]" >/dev/null
    ctl wait 8
    for _ in 1 2 3 4; do ctl container "$MACHINE_X" 64 22 refinedstorage:speed_upgrade 1 >/dev/null; done
    ctl container "$MACHINE_X" 64 22 refinedstorage:stack_upgrade 1 >/dev/null
    ctl container "$MACHINE_X" 64 22 minecraft:dirt 1 >/dev/null
    ctl wait 6
    before_slots=$(slots "$MACHINE_X" 64 22)
    echo "  the machine before the break: $(jq -c . <<<"$before_slots")"
    [ "$(jq 'length' <<<"$before_slots")" = 6 ] \
        || fail "the machine did not take its six slot items: $before_slots"
    ctl command "gamemode survival" >/dev/null
    ctl give minecraft:diamond_pickaxe 1 >/dev/null
    ctl wait 4
    ctl select minecraft:diamond_pickaxe >/dev/null
    ctl command "execute in minecraft:the_end run tp @s 48.5 64 23.5" >/dev/null
    ctl wait 6
    broken=$(ctl break "$MACHINE_X" 64 22) || fail "cannot break the machine: $broken"
    echo "  the machine's break: $(jq -c '.drops' <<<"$broken")"
    ctl command "execute in minecraft:the_end run tp @s 48.5 64 22.5" >/dev/null
    ctl wait 20
    ctl command "execute in minecraft:the_end run tp @s 48.5 64 23.5" >/dev/null
    ctl wait 4
    ctl select refinedstorage:importer >/dev/null || fail "the payload item is not in the hotbar"
    ctl wait 2
    selected=$(ctl inventory | jq -r '.selected')
    payload_before=$(held_json)
    echo "  the acting hand (main hand, hotbar slot $selected) holds the payload unit: $payload_before"
    [ "$(jq -r '.count // 0' <<<"$payload_before")" = 1 ] || fail "the payload item is not in the acting hand: $payload_before"
    # The payload itself, read on the server: the client's mirror does not carry
    # this item's components (measured in slice 13).
    PLAYER=$(ctl status 2>/dev/null | jq -r '.server.name // empty' 2>/dev/null || true)
    [ -n "$PLAYER" ] || fail "cannot name the local player"
    payload_nbt() {
        ctl command "data get entity $PLAYER Inventory[{id:\"refinedstorage:importer\"}]" \
            | jq -r '.chat[]?' | head -1
    }
    nbt_before=$(payload_nbt)
    echo "  the payload on the server before any refusal: $(tr -d '\n' <<<"$nbt_before" | cut -c1-200)"
    grep -q 'RefinedStorageContents' <<<"$nbt_before" \
        || fail "the payload item carries no RefinedStorageContents on the server: $nbt_before"

    # Refusal 1: the target position already holds a block (the client refuses the
    # placement before any click: "holds …; break it first").
    ctl command "execute in minecraft:the_end run setblock 50 64 22 minecraft:stone" >/dev/null
    ctl wait 6
    ctl command "execute in minecraft:the_end run tp @s 48.5 64 23.5" >/dev/null
    ctl wait 4
    occupied=$(ctl place 50 64 22 --item refinedstorage:importer 2>&1 || true)
    ctl wait 6
    echo "  the placement onto an occupied position: $(tr -d '\n' <<<"$occupied" | cut -c1-160)"
    ctl close >/dev/null 2>&1 || true
    held_occupied=$(held_json)
    nbt_occupied=$(payload_nbt)
    echo "  the acting hand after the occupied-target refusal: $held_occupied"
    [ "$(jq -r '.count // 0' <<<"$held_occupied")" = 1 ] \
        || fail "the occupied-target refusal changed the hand: $payload_before -> $held_occupied"
    echo "  the payload on the server after the occupied-target refusal: $(tr -d '\n' <<<"$nbt_occupied" | cut -c1-160)"
    grep -q 'RefinedStorageContents' <<<"$nbt_occupied" \
        || fail "the occupied-target refusal lost the payload: $nbt_occupied"
    [ "$(ctl block 50 64 22 | jq -r '.state')" = "minecraft:stone" ] \
        || fail "the occupied position changed: $(ctl block 50 64 22 | jq -c .)"

    # Refusal 2: the server's own answer. In adventure a player may not place a
    # block, so the click on the floor with this item is refused server-side and
    # the placement hook never runs.
    ctl command "gamemode adventure" >/dev/null
    ctl wait 6
    ctl command "execute in minecraft:the_end run tp @s 48.5 64 23.5" >/dev/null
    ctl wait 4
    ctl interact 48 63 22 >/dev/null 2>&1 || true
    ctl wait 10
    placed_state=$(ctl block 48 64 22 | jq -r '.state')
    held_adventure=$(held_json)
    nbt_adventure=$(payload_nbt)
    echo "  the block above the clicked floor after the adventure click: $placed_state"
    echo "  the acting hand after the adventure refusal: $held_adventure"
    [ "$placed_state" = "minecraft:air" ] \
        || fail "the adventure click placed a block: $placed_state"
    [ "$(jq -r '.count // 0' <<<"$held_adventure")" = 1 ] \
        || fail "the adventure refusal changed the hand: $payload_before -> $held_adventure"
    echo "  the payload on the server after the adventure refusal: $(tr -d '\n' <<<"$nbt_adventure" | cut -c1-160)"
    grep -q 'RefinedStorageContents' <<<"$nbt_adventure" \
        || fail "the adventure refusal lost the payload: $nbt_adventure"
    if [ "$SELFTEST" = 1 ]; then
        echo "(selftest: asserting the refused placement consumed the item)"
        [ "$(jq -r '.count // 0' <<<"$held_adventure")" = 0 ] \
            || fail "both refused placements left the item in the hand ($held_adventure) with its payload intact ($(tr -d '\n' <<<"$nbt_adventure" | cut -c1-120)): the refusal is the fix, so this inverted assertion fails as it must"
    fi
    # The differential: the same placement on open ground consumes one and
    # restores the six slots in the machine it makes.
    ctl command "gamemode survival" >/dev/null
    ctl wait 6
    ctl command "execute in minecraft:the_end run tp @s 48.5 64 23.5" >/dev/null
    ctl wait 4
    ctl place 48 64 22 --item refinedstorage:importer >/dev/null 2>&1 \
        || fail "cannot place the payload item on open ground"
    ctl close >/dev/null 2>&1 || true
    ctl wait 10
    held_open=$(held_json)
    restored=$(slots 48 64 22)
    echo "  after the same placement on open ground: the hand: $held_open"
    echo "  the machine it made: $(jq -c . <<<"$restored")"
    [ "$(jq -r '.count // 0' <<<"$held_open")" = 0 ] \
        || fail "the placement on open ground did not consume the item: $held_open"
    [ "$(jq -S -c . <<<"$restored")" = "$(jq -S -c . <<<"$before_slots")" ] \
        || fail "the machine placed on open ground lists $restored, expected $before_slots"
    echo "PASS: a placement refused for an occupied target and one the server refused in adventure both left the acting hand's single payload unit untouched (count 1, RefinedStorageContents on the server), while the same placement on open ground consumed it and restored all six slots"
fi
