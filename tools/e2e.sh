#!/usr/bin/env bash
# In-game check for Refined Storage slice 1: the item network core.
#
# The End arena (stone floor at y=63, the network at y=64):
#   controller 0,64,0 - cable 1,64,0 - disk drive 2,64,0 - grid 3,64,0
# Every mod command is typed with `ctl chat "/cmd"` because `ctl command` runs
# the console path and bypasses the native `perform_command` hook.
#
# Default run:
#  - the cable's six connection bits follow the blocks beside it, and the grid
#    reports `active=true` while its network has a controller;
#  - the grid opens a 9x6 menu (54 container slots) that is empty with no disk;
#  - a storage disk right-clicked onto the drive goes into it (and leaves the
#    hand), and the network then reports 1 disk;
#  - 5 diamonds and 3 gold deposited *through the grid's menu* are in the
#    network: the grid lists both and `/refinedstorage info` totals 8 items in
#    2 resources;
#  - the search filters the listing (`gold` lists only the gold), and clearing
#    it lists both again;
#  - a shift-click on the diamond view slot withdraws 5 into the inventory and
#    the network totals 3 items in 1 resource;
#  - a broken controller takes the network offline (the grid lists nothing while
#    the disk still holds the gold), and putting it back lists the gold again;
#  - an online network with no disk drive lists nothing while the first grid
#    still lists its gold (the listing comes from the drive's disk);
#  - a stick right-clicked onto the drive is not a disk and does not enter it;
#  - a vanilla barrel is not a network block (and `/refinedstorage info` says
#    so);
#  - the disk keeps its contents across `save` + relaunch.
#
# Slice 3 (the wireless transmitter):
#  - it joins the network through its facing side only: the same block at the
#    same position is offline facing away and active facing the cable;
#  - `/refinedstorage info` on it names it and prints `range 16 blocks`, and a
#    cable's info prints no range;
#  - a use 2.5 blocks away and one exactly 16 blocks away are accepted, 17
#    blocks away is refused `out of range: 17.0 > 16 blocks`, an offline
#    transmitter refuses `the network is offline`, and a position with no
#    transmitter refuses `No wireless transmitter there.`;
#  - a use from the End against the overworld transmitter (same coordinates,
#    force-loaded chunks) is refused `different dimension`, and the same
#    command under `execute in minecraft:overworld` is accepted;
#  - the item places against the clicked face, the block drops itself, and the
#    `refinedstorage:wireless` advancement is granted when the item is obtained
#    (and was absent before).
#
# Slice 5 (the promise table's key):
#  - two networks in one dimension each show their own stock: network A and
#    network B both hold 3 gold, and each grid lists its own 3 (keyed by
#    dimension alone, one of them listed 0 and hid 3 items it did have).
#
# SELFTEST=1 flips the wireless range assertion (a use 17 blocks out must be
# accepted, so the run FAILS); SELFTEST=offline flips the older "offline lists
# nothing" assertion the same way; SELFTEST=grid flips the wireless grid's empty
# view; SELFTEST=networks flips the two-network assertion (one grid must hide the
# other network's stock, so the run FAILS now that the key is the network).
set -euo pipefail
export LC_ALL=C

CLONE=$(cd "$(dirname "$0")/../.." && pwd)
PORT=$(cd "$(dirname "$0")/.." && pwd)
SHARED=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$SHARED/horizon-ctl}
# Since the SDK migration (2026-11-18) the mod is one flat `.hmod` the launcher
# compiles into the game, so GAME must name a game the launcher built with it.
# `ports/tools/launcher-game.sh run refinedstorage` builds, exports and runs this
# script with GAME/CTL/CONTROL_GAME/RES/E2E_TMP/SOCK already exported:
#   GAME=<launcher-built horizon-client>, CONTROL_GAME=<the same Nightly, no package>
# The fallback names the retired `tools/horizon.py build` output, which no longer
# exists, so a hand-run without GAME fails with "missing horizon-client" rather
# than silently running an unmodded game.
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
TMP=${E2E_TMP:-/tmp/opencode}
SOCK=${SOCK:-$TMP/refinedstorage-e2e.sock}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
WORLD=RefinedStorageE2E

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

mkdir -p "$TMP"
SCRATCH=$(mktemp -d "$TMP/refinedstorage-home.XXXXXX")
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }

# The non-null slots of the container at a position, as JSON.
slots() { ctl container "$1" "$2" "$3" | jq -c '[.slots[] | select(. != null)]'; }
# The open menu's container slots (the grid's view, the drive's disks).
view() { ctl screen | jq -c '[.slots[] | select(.slot >= 300 and .item != null)]'; }
count_of() { # count_of ITEM
    ctl inventory | jq -r --arg item "$1" \
        '[((.hotbar // [])[] | select(.item == $item) | .count), ((.main // [])[] | select(.item == $item) | .count)] | add // 0'
}
# Use the held item in the air: `ctl interact` leaves the crosshair on a block,
# and a use on the transmitter would bind again instead of opening the grid.
use_held() { ctl look 0 -90 >/dev/null; ctl wait 2; ctl use-item; }
# Aim level at a block before right-clicking it (`ctl interact` needs a face of
# the block visible from where the player stands; `use_held` looks at the sky).
# Right-click a block with the held item, aiming the crosshair at one of its
# faces: a player binds the grid by right-clicking the transmitter.
use_on_block() { ctl look-at block "$1" "$2" "$3" "$4" >/dev/null; ctl wait 2; ctl use-item; }
# The first player-inventory slot (0-35 in a container menu) holding ITEM.
inv_slot() { # inv_slot ITEM
    ctl screen | jq -r --arg item "$1" \
        '[.slots[] | select(.slot < 36 and .item == $item)][0].slot // empty'
}
expect_slots() { # expect_slots JSON
    local got want
    got=$(view | jq -S -c .)
    want=$(jq -S -c . <<<"$1")
    [ "$got" = "$want" ] || fail "the grid's view is $got, expected $want"
    echo "  grid view: $got"
}
info() { # info X Y Z: the /refinedstorage info line
    say "refinedstorage info $1 $2 $3"
    ctl wait 4 >/dev/null
    ctl messages 6 | jq -r '.chat[]' | grep -m1 -E 'Refined Storage network|No Refined Storage' || true
}
# Only the chat lines a command printed: `events --since <seq>` names the
# sequence from before the command, so an earlier phase's feedback can neither
# satisfy nor break an assertion.
event_seq() { ctl events | jq -r '.last'; }
since_seq() { ctl events --since "$1" | jq -r '.events[] | select(.event == "chat") | .text'; }
# A /refinedstorage command's parsed reply: the lines it printed, oldest first.
reply() { # reply "command text"
    local seq
    seq=$(event_seq)
    say "$1"
    ctl wait 4 >/dev/null
    since_seq "$seq"
}
expect_reply() { # expect_reply "command text" "text the reply must contain"
    local line
    line=$(reply "$1")
    echo "  /$1 -> $line"
    case "$line" in
        *"$2"*) ;;
        *) fail "expected '$1' to answer '$2', got '$line'" ;;
    esac
}
expect_info() { # expect_info X Y Z TEXT
    local line
    line=$(info "$1" "$2" "$3")
    echo "  info($1,$2,$3): $line"
    case "$line" in
        *"$4"*) ;;
        *) fail "expected /refinedstorage info to say '$4', got '$line'" ;;
    esac
}
# The player's saved advancement progress (`players/advancements/<uuid>.json`),
# written by `ctl save`. The game's data folder is the launch directory
# (`$SCRATCH`), not `HORIZON_HOME`.
adv_file() { ls -t "$SCRATCH"/saves/*/players/advancements/*.json 2>/dev/null | head -1; }
block() { ctl block "$1" "$2" "$3" | jq -r '.state'; }
expect_state() { # expect_state X Y Z TEXT
    local got
    got=$(block "$1" "$2" "$3")
    case "$got" in
        *"$4"*) ;;
        *) fail "the block at $1,$2,$3 is $got, expected $4" ;;
    esac
    echo "  block($1,$2,$3): $got"
}

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() {
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" launch \
        --game "$GAME" --world "$WORLD" --seed 20261105 --creative --offscreen --wait \
        --resources "$RES" >/dev/null
    ctl wait-ready 120
    ctl wait 40
}

# The first launch only: a stone floor and a clear volume in The End.
arena() {
    ctl command "execute in minecraft:the_end run tp @s 40.5 64 3.5"
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5
        if [ "$(ctl block 40 63 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the End arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill 32 63 -8 56 63 24 minecraft:stone"
    ctl command "execute in minecraft:the_end run fill 32 64 -8 56 74 24 minecraft:air"
    ctl command "execute in minecraft:the_end run kill @e[type=item]"
    ctl wait 20
}

echo "== launch (The End arena, the flat package) =="
launch
arena

echo "== the wireless advancement starts unobtained =="
ctl save
ctl wait 10
before=$(adv_file)
[ -n "$before" ] || fail "no player advancements file under $SCRATCH/saves"
echo "  advancement before the give: $(jq -c '.["refinedstorage:wireless"] // "absent"' "$before")"
if jq -e 'has("refinedstorage:wireless")' "$before" >/dev/null; then
    fail "the player already had refinedstorage:wireless before obtaining a transmitter"
fi

echo "== the overworld transmitter, for the cross-dimension use =="
# The player starts in the overworld; the network there is built and ticked now,
# then the chunks are force-loaded so a use from the End can still find it.
ctl command "execute in minecraft:overworld run tp @s 41.5 64 2.5"
ctl wait 40
ctl command "execute in minecraft:overworld run fill 32 63 -8 56 63 24 minecraft:stone"
ctl command "execute in minecraft:overworld run fill 32 64 -8 56 74 24 minecraft:air"
ctl wait 10
ctl command "execute in minecraft:overworld run setblock 40 64 0 refinedstorage:controller"
ctl command "execute in minecraft:overworld run setblock 41 64 0 refinedstorage:cable"
ctl command "execute in minecraft:overworld run setblock 41 64 1 refinedstorage:wireless_transmitter[facing=north]"
ctl command "execute in minecraft:overworld run forceload add 32 0 48 8"
ctl wait 12
expect_state 41 64 1 'facing=north'
expect_state 41 64 1 'active=true'
expect_info 41 64 1 "online (3 nodes, 1 controller, 0 drive, 0 grid, 1 transmitter)"
# Back to the End: everything after this reads the End.
ctl command "execute in minecraft:the_end run tp @s 40.5 64 3.5"
ctl wait 20

echo "== the network is built: controller, cable, disk drive, grid =="
ctl command "execute in minecraft:the_end run setblock 40 64 0 refinedstorage:controller"
ctl command "execute in minecraft:the_end run setblock 41 64 0 refinedstorage:cable"
ctl command "execute in minecraft:the_end run setblock 42 64 0 refinedstorage:disk_drive"
ctl command "execute in minecraft:the_end run setblock 43 64 0 refinedstorage:grid"
ctl wait 12
expect_state 41 64 0 'west=true'
expect_state 41 64 0 'east=true'
expect_state 41 64 0 'north=false'
expect_state 43 64 0 'active=true'
expect_info 43 64 0 "online (4 nodes, 1 controller, 1 drive, 1 grid); 0 disks, 0 items in 0 resources"

echo "== the wireless transmitter joins through its facing side only =="
ctl command "execute in minecraft:the_end run setblock 52 64 4 refinedstorage:controller"
ctl command "execute in minecraft:the_end run setblock 53 64 4 refinedstorage:cable"
ctl command "execute in minecraft:the_end run setblock 54 64 4 refinedstorage:wireless_transmitter[facing=east]"
ctl wait 12
# Facing away from the cable (east, nothing there): its own offline component.
expect_state 54 64 4 'facing=east'
expect_state 54 64 4 'active=false'
expect_info 54 64 4 "offline (1 nodes, 0 controller, 0 drive, 0 grid, 1 transmitter)"
# Turned to face the cable (west): the controller's network, so active.
ctl command "execute in minecraft:the_end run setblock 54 64 4 refinedstorage:wireless_transmitter[facing=west]"
ctl wait 12
expect_state 54 64 4 'facing=west'
expect_state 54 64 4 'active=true'
expect_info 54 64 4 "online (3 nodes, 1 controller, 0 drive, 0 grid, 1 transmitter)"

echo "== /refinedstorage info names the transmitter and its range =="
transmitter_lines=$(reply "refinedstorage info 54 64 4")
echo "$transmitter_lines" | sed 's/^/  info(54,64,4): /'
echo "$transmitter_lines" | grep -q 'Wireless transmitter at 54,64,4: range 16 blocks, active' \
    || fail "the transmitter's info line does not name its range: $transmitter_lines"
# The differential: the same command on the cable names no range at all.
cable_lines=$(reply "refinedstorage info 53 64 4")
echo "$cable_lines" | sed 's/^/  info(53,64,4): /'
if echo "$cable_lines" | grep -q 'range'; then
    fail "a cable's info named a range: $cable_lines"
fi

echo "== a use in range is accepted, beyond it refused naming the reason =="
ctl tp 54.5 64 6.5 # exact coordinates: `tp @s` would centre them
ctl wait 6
expect_reply "refinedstorage wireless 54 64 4 minecraft:the_end" "accepted (2.5 of 16 blocks"
# The boundary is inclusive: exactly 16 blocks out (z = 4 + 16), same y, same x.
ctl tp 54 64 20
ctl wait 6
expect_reply "refinedstorage wireless 54 64 4 minecraft:the_end" "accepted (16.0 of 16 blocks"
ctl tp 54 64 21
ctl wait 6
if [ "$SELFTEST" = 1 ]; then
    echo "(selftest: asserting a use 17 blocks out is accepted)"
    expect_reply "refinedstorage wireless 54 64 4 minecraft:the_end" "accepted (17.0 of 16 blocks"
else
    expect_reply "refinedstorage wireless 54 64 4 minecraft:the_end" "refused (out of range: 17.0 > 16 blocks"
fi

echo "== an offline transmitter refuses a use within its range =="
ctl tp 54.5 64 6.5 # exact coordinates: `tp @s` would centre them
ctl command "execute in minecraft:the_end run setblock 54 64 4 refinedstorage:wireless_transmitter[facing=east]"
ctl wait 12
expect_state 54 64 4 'active=false'
expect_reply "refinedstorage wireless 54 64 4 minecraft:the_end" "refused (the network is offline)"
ctl command "execute in minecraft:the_end run setblock 54 64 4 refinedstorage:wireless_transmitter[facing=west]"
ctl wait 12
expect_state 54 64 4 'active=true'
# A position that holds no transmitter is not a wireless use at all.
expect_reply "refinedstorage wireless 52 64 4 minecraft:the_end" "No wireless transmitter there."

echo "== a use from another dimension is refused, and only the dimension differs =="
# The overworld network sits at the same coordinates as this End spot, so the
# two commands below differ in the dimension the use happens in and nothing else.
ctl command "execute in minecraft:the_end run tp @s 41.5 64 2.5"
ctl wait 6
expect_reply "refinedstorage wireless 41 64 1 minecraft:overworld" "refused (different dimension)"
expect_reply "execute in minecraft:overworld run refinedstorage wireless 41 64 1 minecraft:overworld" "accepted (1.6 of 16 blocks"

echo "== the item places with the clicked face and drops itself; the transmitter alone is not the advancement =="
# The advancement before the item is obtained: the player's saved progress has no
# entry (the pre-check ran right after the arena).
ctl command "execute in minecraft:the_end run setblock 55 64 6 minecraft:stone"
ctl command "execute in minecraft:the_end run tp @s 56.5 64 5.5"
ctl wait 6
ctl give refinedstorage:wireless_transmitter 1
ctl wait 4
ctl select refinedstorage:wireless_transmitter
ctl wait 4
ctl interact 55 64 6 north
ctl wait 8
expect_state 55 64 5 'refinedstorage:wireless_transmitter'
expect_state 55 64 5 'facing=north'
ctl wait 20
ctl save
ctl wait 10
after=$(adv_file)
[ -n "$after" ] || fail "no player advancements file under $SCRATCH/saves"
echo "  advancement after the transmitter alone: $(jq -c '.["refinedstorage:wireless"] // "absent"' "$after")"
# Upstream's criteria need the wireless grid *and* a transmitter in the
# inventory (data/refinedstorage/advancement/wireless.json), so a transmitter
# alone must not grant it; the grid half is asserted in the wireless grid
# section below, where the item is obtained.
if jq -e '.["refinedstorage:wireless"].done == true' "$after" >/dev/null; then
    fail "a wireless transmitter alone granted refinedstorage:wireless: $(jq -c '.["refinedstorage:wireless"]' "$after")"
fi

echo "== the transmitter block drops itself =="
# The stone the item was placed against is removed, and the player stands two
# blocks south: `ctl break` aims at the block's centre, and the nub is 4x4x10,
# so a ray from above grazes past it (measured: "no face ... is visible").
ctl command "execute in minecraft:the_end run setblock 55 64 6 minecraft:air"
ctl command "gamemode survival"
ctl give minecraft:diamond_pickaxe 1
ctl wait 4
ctl select minecraft:diamond_pickaxe
ctl command "execute in minecraft:the_end run tp @s 55.5 64 7.5"
ctl wait 6
broken=$(ctl break 55 64 5) || fail "cannot break the transmitter: $broken"
echo "  transmitter drops: $(echo "$broken" | jq -c '.drops')"
[ "$(echo "$broken" | jq -r '.drops["refinedstorage:wireless_transmitter"] // 0')" -ge 1 ] \
    || fail "the transmitter did not drop itself: $(echo "$broken" | jq -c '.drops')"
ctl command "gamemode creative"
ctl command "clear @s"
ctl wait 4

echo "== the grid opens its 9x6 menu, empty with no disk =="
ctl command "execute in minecraft:the_end run tp @s 42.5 64 2.5"
ctl wait 4
ctl interact 43 64 0
ctl wait 8
ctl screen | jq -e '.screen == "container"' >/dev/null \
    || fail "the grid did not open a container menu: $(ctl screen | jq -c '{screen,slots:(.slots|length)}')"
ctl screen | jq -e '[.slots[] | select(.slot >= 300)] | length == 54' >/dev/null \
    || fail "the grid menu does not have 54 container slots: $(ctl screen | jq -c '[.slots[] | select(.slot >= 300)] | length')"
expect_slots '[]'
ctl close
ctl wait 4

echo "== a storage disk right-clicked onto the drive goes in, out of the hand =="
ctl give refinedstorage:1k_storage_disk 1
ctl wait 4
ctl select refinedstorage:1k_storage_disk
ctl wait 4
ctl command "execute in minecraft:the_end run tp @s 41.5 64 2.5"
ctl wait 4
ctl interact 42 64 0
ctl wait 8
[ "$(count_of refinedstorage:1k_storage_disk)" = 0 ] \
    || fail "the disk stayed in the hand: $(ctl inventory | jq -c '[.hotbar[] | select(.item == "refinedstorage:1k_storage_disk")]')"
slots 42 64 0 | jq -e '[.[] | select(.item == "refinedstorage:1k_storage_disk")] | length == 1' >/dev/null \
    || fail "the drive does not hold the disk: $(slots 42 64 0)"
echo "  drive: $(slots 42 64 0)"
expect_info 43 64 0 "1 disks, 0 items in 0 resources"

echo "== items deposited through the grid's menu are on the network =="
ctl give minecraft:diamond 5
ctl give minecraft:gold_ingot 3
ctl wait 4
ctl command "execute in minecraft:the_end run tp @s 42.5 64 2.5"
ctl wait 4
ctl interact 43 64 0
ctl wait 8
diamond_slot=$(inv_slot minecraft:diamond)
[ -n "$diamond_slot" ] || fail "no diamond stack in the menu's player inventory: $(ctl screen | jq -c '[.slots[] | select(.item != null)]')"
ctl click-slot "$diamond_slot"
ctl wait 2
ctl click-slot 300
ctl wait 8
gold_slot=$(inv_slot minecraft:gold_ingot)
[ -n "$gold_slot" ] || fail "no gold stack in the menu's player inventory"
ctl click-slot "$gold_slot"
ctl wait 2
ctl click-slot 301
ctl wait 8
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3},{"slot":301,"item":"minecraft:diamond","count":5}]'
[ "$(count_of minecraft:diamond)" = 0 ] || fail "5 diamonds are still in the inventory"
expect_info 43 64 0 "1 disks, 8 items in 2 resources"

echo "== the search filters the grid's listing =="
say "refinedstorage filter 43 64 0 gold"
ctl wait 8
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
say "refinedstorage filter 43 64 0 diamond"
ctl wait 8
expect_slots '[{"slot":300,"item":"minecraft:diamond","count":5}]'
ctl messages 6 | jq -r '.chat[]' | grep -q 'Grid search: diamond' \
    || fail "no search feedback in chat"
say "refinedstorage filter 43 64 0"
ctl wait 8
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3},{"slot":301,"item":"minecraft:diamond","count":5}]'

echo "== a shift-click on the diamond view withdraws it into the inventory =="
ctl click-slot 301 shift
ctl wait 8
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
[ "$(count_of minecraft:diamond)" = 5 ] \
    || fail "the withdrawn diamonds are not in the inventory: $(ctl inventory | jq -c '[.hotbar // [], .main // []]')"
expect_info 43 64 0 "1 disks, 3 items in 1 resources"
ctl close
ctl wait 4

echo "== a broken controller takes the network offline; the disk keeps the gold =="
ctl command "gamemode survival"
ctl give minecraft:diamond_pickaxe 1
ctl wait 4
ctl select minecraft:diamond_pickaxe
ctl command "execute in minecraft:the_end run tp @s 40.5 64 2.5"
ctl wait 6
ctl break 40 64 0
ctl wait 12
ctl command "clear @s refinedstorage:controller"
ctl command "execute in minecraft:the_end run kill @e[type=item]"
expect_state 43 64 0 'active=false'
expect_info 43 64 0 "offline (3 nodes, 0 controller, 1 drive, 1 grid)"
ctl command "gamemode creative"
ctl wait 4
ctl interact 43 64 0
ctl wait 8
if [ "$SELFTEST" = offline ]; then
    echo "(selftest: asserting the offline grid still lists the gold)"
    expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
else
    expect_slots '[]'
fi
ctl close
ctl wait 4
# Put the controller back: the gold comes back, so it lived in the disk.
ctl command "execute in minecraft:the_end run setblock 40 64 0 refinedstorage:controller"
ctl wait 12
expect_state 43 64 0 'active=true'
expect_info 43 64 0 "online (4 nodes, 1 controller, 1 drive, 1 grid); 1 disks, 3 items in 1 resources"
ctl interact 43 64 0
ctl wait 8
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4

echo "== two grids on one network never offer the same items twice =="
# A second grid joins the same component (on top of the drive). The first grid's
# view has already promised the 3 gold, so the second shows nothing: with the old
# per-grid view both showed the last 3 gold and both players could take them.
ctl command "execute in minecraft:the_end run setblock 42 65 0 refinedstorage:grid"
ctl wait 12
expect_state 42 65 0 'active=true'
ctl interact 42 65 0
ctl wait 8
expect_slots '[]'
ctl close
ctl wait 4
# The first grid still shows the gold: the stock was not consumed, only promised.
ctl interact 43 64 0
ctl wait 8
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4
# Breaking the first grid releases its promise; the second then shows the gold.
ctl command "execute in minecraft:the_end run setblock 43 64 0 minecraft:air"
ctl wait 12
ctl interact 42 65 0
ctl wait 8
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4
ctl command "execute in minecraft:the_end run setblock 43 64 0 refinedstorage:grid"
ctl command "execute in minecraft:the_end run setblock 42 65 0 minecraft:air"
ctl wait 12
expect_info 43 64 0 "online (4 nodes, 1 controller, 1 drive, 1 grid)"

echo "== an online network with no disk drive lists nothing, beside it =="
ctl command "execute in minecraft:the_end run setblock 48 64 0 refinedstorage:controller"
ctl command "execute in minecraft:the_end run setblock 49 64 0 refinedstorage:cable"
ctl command "execute in minecraft:the_end run setblock 50 64 0 refinedstorage:grid"
ctl wait 12
expect_state 50 64 0 'active=true'
expect_info 50 64 0 "online (3 nodes, 1 controller, 0 drive, 1 grid); 0 disks, 0 items in 0 resources"
ctl command "execute in minecraft:the_end run tp @s 49.5 64 2.5"
ctl wait 4
ctl interact 50 64 0
ctl wait 8
expect_slots '[]'
ctl close
ctl wait 4
# The first network still lists its gold.
ctl command "execute in minecraft:the_end run tp @s 42.5 64 2.5"
ctl wait 4
ctl interact 43 64 0
ctl wait 8
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4

echo "== two networks in one dimension each show their own stock =="
# Network A (40..43,64,0) holds 3 gold. Network B (48..50,64,0) gets a drive of
# its own and the same 3 gold: each grid must list its own 3. Keyed by dimension
# alone, A's promise hid B's stock (and then B's promise hid A's), so one grid
# reported 0 while the disk under it held 3.
ctl command "execute in minecraft:the_end run setblock 51 64 0 refinedstorage:disk_drive"
ctl wait 10
expect_info 51 64 0 "online (4 nodes, 1 controller, 1 drive, 1 grid); 0 disks, 0 items in 0 resources"
ctl give refinedstorage:1k_storage_disk 1 >/dev/null
ctl wait 4
ctl select refinedstorage:1k_storage_disk >/dev/null
ctl wait 4
ctl command "execute in minecraft:the_end run tp @s 51.5 64 2.5"
ctl wait 4
ctl interact 51 64 0
ctl wait 8
[ "$(count_of refinedstorage:1k_storage_disk)" = 0 ] \
    || fail "network B's disk stayed in the hand"
expect_info 51 64 0 "1 disks, 0 items in 0 resources"
ctl close
ctl wait 4
# 3 gold deposited through network B's own grid.
ctl give minecraft:gold_ingot 3 >/dev/null
ctl wait 4
ctl command "execute in minecraft:the_end run tp @s 49.5 64 2.5"
ctl wait 4
ctl interact 50 64 0
ctl wait 8
gold_slot=$(inv_slot minecraft:gold_ingot)
[ -n "$gold_slot" ] || fail "no gold stack in network B's menu inventory"
ctl click-slot "$gold_slot" shift
ctl wait 8
expect_info 51 64 0 "1 disks, 3 items in 1 resources"
if [ "$SELFTEST" = networks ]; then
    echo "(selftest: asserting network B's grid hides network A's stock)"
    expect_slots '[]'
fi
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4
# The differential, with B's promise now live: A's grid still lists its own 3.
ctl command "execute in minecraft:the_end run tp @s 42.5 64 2.5"
ctl wait 4
ctl interact 43 64 0
ctl wait 8
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4
# And B's grid again, with both promises live: still its own 3.
ctl command "execute in minecraft:the_end run tp @s 49.5 64 2.5"
ctl wait 4
ctl interact 50 64 0
ctl wait 8
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4
# The stock was promised, not consumed: each disk still holds its own 3.
expect_info 43 64 0 "1 disks, 3 items in 1 resources"
expect_info 51 64 0 "1 disks, 3 items in 1 resources"

echo "== a stick is not a storage disk: the drive takes nothing =="
ctl give minecraft:stick 1
ctl wait 4
ctl select minecraft:stick
ctl wait 4
ctl command "execute in minecraft:the_end run tp @s 41.5 64 2.5"
ctl wait 4
ctl interact 42 64 0
ctl wait 8
[ "$(count_of minecraft:stick)" = 1 ] || fail "the drive took the stick"
slots 42 64 0 | jq -e '[.[] | select(.item == "minecraft:stick")] | length == 0' >/dev/null \
    || fail "the drive holds a stick: $(slots 42 64 0)"
ctl screen | jq -e '.screen == "container"' >/dev/null \
    || fail "the stick click did not open the drive's own menu"
echo "  drive after a stick: $(slots 42 64 0)"
ctl close
ctl wait 4

echo "== a vanilla barrel is not a network block =="
ctl command "execute in minecraft:the_end run setblock 56 64 0 minecraft:barrel"
ctl wait 6
expect_info 56 64 0 "No Refined Storage network block there."

echo "== the crafting chain: every carried recipe crafts in game =="
# Slice 2. The driver reads this package's own data/refinedstorage/recipe/*.json,
# so a carried recipe that is not crafted here fails the run. Shaped/shapeless
# recipes go into a crafting table at 36,64,8, smelting into a furnace at 38,64,8;
# both sit beside the network's arena, on its stone floor.
python3 "$PORT/tools/recipe_check.py" "$SOCK" "$CTL" 36 64 8 || fail "the recipe check failed"
ctl command "clear @s" >/dev/null
# The chain's one block: placed, read back, and broken for itself.
ctl command "execute in minecraft:the_end run setblock 44 64 8 refinedstorage:machine_casing" >/dev/null
ctl wait 6
[ "$(block 44 64 8)" = "refinedstorage:machine_casing" ] \
    || fail "machine_casing did not place: $(block 44 64 8)"
echo "  block(44,64,8): $(block 44 64 8)"
ctl command "gamemode survival" >/dev/null
ctl give minecraft:diamond_pickaxe 1 >/dev/null
ctl wait 4
ctl select minecraft:diamond_pickaxe >/dev/null
ctl command "execute in minecraft:the_end run tp @s 43.5 64 8.5" >/dev/null
ctl wait 6
broken=$(ctl break 44 64 8) || fail "cannot break the machine casing"
echo "  machine casing drops: $(echo "$broken" | jq -c '.drops')"
[ "$(echo "$broken" | jq -r '.drops["refinedstorage:machine_casing"] // 0')" -ge 1 ] \
    || fail "the machine casing did not drop itself: $(echo "$broken" | jq -c '.drops')"
ctl command "gamemode creative" >/dev/null
ctl command "clear @s" >/dev/null
ctl wait 4

echo "== the wireless grid: bind it, open the network's view, move items through it =="
# A network of its own (48..50,64,8 with the drive at 49,64,9) inside the arena
# floor, so the player never falls out of the world while a leg waits; no block
# grid promises its stock, so the item's view must come from the disks itself.
ctl command "execute in minecraft:the_end run setblock 48 64 8 refinedstorage:controller"
ctl command "execute in minecraft:the_end run setblock 49 64 8 refinedstorage:cable"
ctl command "execute in minecraft:the_end run setblock 50 64 8 refinedstorage:wireless_transmitter[facing=west]"
ctl command "execute in minecraft:the_end run setblock 49 64 9 refinedstorage:disk_drive"
ctl command "execute in minecraft:the_end run tp @s 50.5 64 10.5"
ctl wait 14
expect_state 50 64 8 'facing=west'
expect_state 50 64 8 'active=true'
expect_info 49 64 9 "online (4 nodes, 1 controller, 1 drive, 0 grid, 1 transmitter); 0 disks, 0 items in 0 resources"

# The disk goes into the drive; the five diamonds are deposited through the
# item's own menu below.
ctl give refinedstorage:1k_storage_disk 1 >/dev/null
ctl wait 4
ctl select refinedstorage:1k_storage_disk >/dev/null
ctl wait 4
ctl command "execute in minecraft:the_end run tp @s 49.5 64 10.5"
ctl wait 4
ctl interact 49 64 9
ctl wait 8
[ "$(count_of refinedstorage:1k_storage_disk)" = 0 ] \
    || fail "the wireless network's disk stayed in the hand"
expect_info 49 64 9 "1 disks, 0 items in 0 resources"

# The item is unbound until it is used on a transmitter: the use says so.
ctl give refinedstorage:wireless_grid 1 >/dev/null
ctl wait 4
ctl select refinedstorage:wireless_grid >/dev/null
ctl wait 4
expect_reply "refinedstorage bound" "The wireless grid is not bound."
ctl command "execute in minecraft:the_end run tp @s 50.5 64 10.5"
ctl wait 4
use_on_block 50 64 8 south
ctl wait 6
# The binding is the item's own component: the dimension and the block it was
# made on, read back from the stack (upstream's `GlobalPos`).
expect_reply "refinedstorage bound" "Wireless grid bound to 50,64,8 in minecraft:the_end."
# The transmitter has no menu of its own: binding opens none.
ctl screen | jq -e '.screen != "container"' >/dev/null \
    || fail "binding the grid opened a menu: $(ctl screen | jq -c '{screen}')"

# A use inside the range opens the 9x6 view of that network.
ctl give minecraft:diamond 5 >/dev/null
ctl wait 4
use_held
ctl wait 10
ctl screen | jq -e '.screen == "container"' >/dev/null \
    || fail "the wireless grid did not open a container menu: $(ctl screen | jq -c '{screen,slots:(.slots|length)}')"
ctl screen | jq -e '[.slots[] | select(.slot >= 300)] | length == 54' >/dev/null \
    || fail "the wireless grid's menu does not have 54 container slots: $(ctl screen | jq -c '[.slots[] | select(.slot >= 300)] | length')"
if [ "$SELFTEST" = grid ]; then
    echo "(selftest: asserting the wireless grid's empty view lists a diamond)"
    expect_slots '[{"slot":300,"item":"minecraft:diamond","count":5}]'
fi
expect_slots '[]'
# The menu's title names the item (upstream's `ContentNames.WIRELESS_GRID`).
ctl screen | jq -r '.title // .name // empty' | grep -qi 'wireless grid' \
    || echo "  (note: the screen reply names no title: $(ctl screen | jq -c '{title,name}'))"

# A shift-click on a player's inventory slot deposits that stack on the network.
diamond_slot=$(inv_slot minecraft:diamond)
[ -n "$diamond_slot" ] || fail "no diamond stack in the wireless grid's player inventory"
ctl click-slot "$diamond_slot" shift
ctl wait 10
expect_slots '[{"slot":300,"item":"minecraft:diamond","count":5}]'
[ "$(count_of minecraft:diamond)" = 0 ] || fail "the deposited diamonds are still in the inventory"
expect_info 49 64 9 "1 disks, 5 items in 1 resources"

# A shift-click on the view withdraws the stack into the inventory.
ctl click-slot 300 shift
ctl wait 10
expect_slots '[]'
[ "$(count_of minecraft:diamond)" = 5 ] \
    || fail "the withdrawn diamonds are not in the inventory: $(ctl inventory | jq -c '[.hotbar // [], .main // []]')"
expect_info 49 64 9 "1 disks, 0 items in 0 resources"
ctl close
ctl wait 4

# The range is the transmitter's: 17 blocks out is refused, naming the distance.
# The floor runs x 32..56, so both spots stay on it.
ctl tp 33 64 8
ctl wait 6
echo "  bound in range 17: $(reply "refinedstorage bound")"
use_held
ctl wait 6
refusal=$(ctl messages 6 | jq -r '.chat[]' | grep -m1 'Wireless grid' || true)
echo "  use 17 blocks out: $refusal"
case "$refusal" in
    *"refused (out of range: 17.0 > 16 blocks)"*) ;;
    *) fail "a use 17 blocks out was not refused with its distance: '$refusal'" ;;
esac
ctl screen | jq -e '.screen != "container"' >/dev/null \
    || fail "a refused use opened the grid's menu"
# The differential: 16 blocks out is inside the range and opens the view.
ctl tp 34 64 8
ctl wait 6
use_held
ctl wait 10
ctl screen | jq -e '.screen == "container"' >/dev/null \
    || fail "a use exactly 16 blocks out did not open the view: $(ctl screen | jq -c '{screen}')"
expect_slots '[]'
ctl close
ctl wait 4

# The dimension is part of the binding: the same item, the same binding and the
# same player, refused from the overworld and accepted in the End. Only the
# dimension the use happens in differs, which is the differential.
ctl command "execute in minecraft:overworld run tp @s 41.5 64 2.5"
ctl wait 10
use_held
ctl wait 6
refusal=$(ctl messages 6 | jq -r '.chat[]' | grep -m1 'Wireless grid' || true)
echo "  use of the End transmitter from the overworld: $refusal"
case "$refusal" in
    *"refused (different dimension)"*) ;;
    *) fail "a use in another dimension was not refused as such: '$refusal'" ;;
esac
ctl screen | jq -e '.screen != "container"' >/dev/null \
    || fail "a cross-dimension use opened the grid's menu"
# Back in the binding's dimension: the same item opens the view.
ctl command "execute in minecraft:the_end run tp @s 50.5 64 10.5"
ctl wait 10
use_held
ctl wait 10
ctl screen | jq -e '.screen == "container"' >/dev/null \
    || fail "the same use in the bound dimension did not open the view: $(ctl screen | jq -c '{screen}')"
expect_slots '[]'
ctl close
ctl wait 4

# The advancement needs the grid and a transmitter: the grid was obtained
# above, so giving a transmitter completes it.
ctl command "execute in minecraft:the_end run tp @s 50.5 64 10.5"
ctl give refinedstorage:wireless_transmitter 1 >/dev/null
ctl wait 20
ctl save
ctl wait 10
grid_adv=$(adv_file)
[ -n "$grid_adv" ] || fail "no player advancements file under $SCRATCH/saves"
echo "  advancement after the grid and a transmitter: $(jq -c '.["refinedstorage:wireless"] // "absent"' "$grid_adv")"
jq -e '.["refinedstorage:wireless"].done == true' "$grid_adv" >/dev/null \
    || fail "the wireless grid and a transmitter did not grant refinedstorage:wireless: $(jq -c '.["refinedstorage:wireless"] // "absent"' "$grid_adv")"
ctl messages 30 | jq -r '.chat[]' | grep -q 'made the advancement \[Wireless\]' \
    || fail "no '[Wireless]' advancement announcement in chat"
ctl command "clear @s" >/dev/null
ctl wait 4

echo "== an unknown id in this namespace is refused =="
if ctl give refinedstorage:no_such_item 1 >/dev/null 2>&1; then
    fail "give refinedstorage:no_such_item succeeded"
fi
ctl command "execute in minecraft:the_end run setblock 45 64 8 refinedstorage:no_such_block" >/dev/null 2>&1 || true
ctl wait 4
unknown_state=$(block 45 64 8)
case "$unknown_state" in
    refinedstorage:no_such_block*) fail "setblock accepted refinedstorage:no_such_block" ;;
esac
echo "  refinedstorage:no_such_item refused, refinedstorage:no_such_block did not place ($unknown_state)"

echo "== the disk keeps its contents across a relaunch =="
ctl command "execute in minecraft:the_end run tp @s 42.5 64 2.5"
ctl save
ctl wait 10
ctl quit
sleep 3
launch
ctl wait 20
expect_info 43 64 0 "online (4 nodes, 1 controller, 1 drive, 1 grid); 1 disks, 3 items in 1 resources"
ctl interact 43 64 0
ctl wait 10
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4

echo "PASS: the wireless transmitter joined its network through the facing side only (offline facing away, active facing the cable), /refinedstorage info named it and its range 16 blocks, a use 2.5 blocks away and one exactly 16 blocks away were accepted, 17 blocks away was refused naming the distance, an offline transmitter refused and a position without one answered so, a use from the End against the overworld transmitter was refused as a different dimension while the same command under execute in was accepted, the item placed against the clicked face and the block dropped itself, every carried recipe crafts in game with its declared output and count (22 now, each with its wrong-ingredient control), the machine casing places, reads back and drops itself, the wireless grid is unbound until it is used on a transmitter and then names it (dimension and position in its own component), a use inside the range opens the network's 9x6 view with 54 slots, a shift-click in the player inventory deposits 5 diamonds on the network and a shift-click on the view withdraws them back, 17 blocks out is refused naming the distance while 16 blocks out opens the view, the same item and binding is refused as a different dimension from the overworld and accepted in the End, the refinedstorage:wireless advancement is not granted by a transmitter alone and is granted by the grid and a transmitter together, two networks in one dimension each listed their own 3 gold while the other network held 3 too, two grids on one network never offered the same items twice, the cable follows its neighbours and the grid is active with a controller, the disk went into the drive and the grid listed what was deposited through its menu, the search filtered it, a shift-click withdrew the diamonds, breaking the controller took the network offline and putting it back restored the disk's gold, a network without a drive listed nothing, a stick did not enter the drive, a vanilla barrel is not a network block, and the disk kept its 3 gold across a relaunch"
# The cleanup trap prints EXIT:<status> as the last line.
