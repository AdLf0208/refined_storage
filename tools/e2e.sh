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
# Slice 6 (the upgrade items):
#  - a range upgrade in the transmitter's own four upgrade slots raises its
#    range from 16 to 24 blocks: the same 17.0-block use the wireless-grid
#    phase refuses is accepted, and the item opens its view naming 24 blocks;
#  - the transmitter's menu opens with four upgrade slots holding the upgrade,
#    and a diamond shift-clicked from the inventory does not enter them;
#  - a creative range upgrade makes the range unlimited (a use 70.5 blocks out
#    is accepted, beyond the 48-block maximum of four range upgrades) and still
#    refuses another dimension.
#
# Slice 7 (the importer and the exporter):
#  - the importer works on the source block its `facing` names;
#  - with no upgrade it moves one item per nine ticks (1..5 in a 20-tick
#    window), a stack upgrade moves 64 in one pass, and four speed upgrades
#    make the same 20 ticks move a stack (work interval 9, 7, 5, 3, 1);
#  - /refinedstorage info names each machine, its upgrade counts, that interval
#    and its filter entries;
#  - an empty filter imports everything and a filter entry blocks what it names
#    (upstream's BLOCK mode);
#  - the exporter pushes only what its filter names into the chest it faces and
#    an exporter with no filter moves nothing;
#  - the 13-slot menu opens, an upgrade slot refuses a diamond, and both machines
#    break and drop themselves.
#
# SELFTEST=1 flips the wireless range assertion (a use 17 blocks out must be
# accepted, so the run FAILS); SELFTEST=offline flips the older "offline lists
# nothing" assertion the same way; SELFTEST=grid flips the wireless grid's empty
# view; SELFTEST=networks flips the two-network assertion (one grid must hide the
# other network's stock, so the run FAILS now that the key is the network);
# SELFTEST=range asserts the 17-block use is still refused with the range
# upgrade in; SELFTEST=creative asserts the creative upgrade left the finite
# range; SELFTEST=speed asserts the un-upgraded importer moved a stack in
# 20 ticks and SELFTEST=stack asserts the stack upgrade left the un-upgraded
# count, so both fail on the working importer; SELFTEST=energy asserts a use
# with no energy in the network is accepted, so the run fails on the gate;
# SELFTEST=itemenergy asserts a deposit at 0 item FE went through, so the run
# fails on the grid's own gate; SELFTEST=drops asserts the re-placed importer
# came back empty, so the run fails once the slots travel on the item;
# SELFTEST=diskcap asserts a 4k disk accepted the 4,097th item, so the run fails
# now that a disk enforces the capacity its item names; SELFTEST=upgrade asserts
# the source disk's own part was not returned by the in-place upgrade, so the run
# fails now that it is.
#
# Slice 8 (the controller's energy model):
#  - the controller's buffer is 1,000 FE, its `energy_type` is upstream's four
#    levels at 40 %, 30 % and 1 % (0 / 50 / 350 / 500 FE read back), and a
#    transmitter's charge is exactly 16 FE a tick measured against the server
#    tick counter the info line prints;
#  - with no energy the transmitter is inactive and the same use is refused
#    `not enough energy: 0 of 16 FE`, before the range is asked (17 blocks out
#    with no energy is refused for energy, with energy it names the distance);
#  - a grid and an importer on a 0-FE network do no work and are dark, while
#    their disks keep what they hold; charged, both work again.
#
# Slice 9 (the wireless grid's own item buffer):
#  - the grid item holds its own 1,000 FE and pays exactly 5 for the open, 5 for
#    a deposit and 5 for a withdrawal (read against the server tick its readout
#    prints; ticks alone cost nothing);
#  - at 0 item FE the same deposit is refused `not enough item energy: 0 of 5 FE`
#    while the network stays online and still prints its own energy line, and a
#    use at 0 opens nothing; charged, both work again.
#
# Slice 10 (a broken machine's own slots travel on its item):
#  - an importer filled with four speed upgrades, one stack upgrade and a dirt
#    filter entry is broken in survival: its item drops with every slot and the
#    upgrade and filter items do not spill onto the ground beside it;
#  - the item is picked up and placed back at the same position, and the
#    machine's 13 slots list all six where they were;
#  - a transmitter's four range upgrades travel the same way.
#
# Slice 11 (the other disk sizes):
#  - a 4k disk right-clicked onto a drive reports 0/4096 items, a saved component
#    of 5,000 items is cut to 4,096 (not the 1k default), and the 4,097th item
#    deposited through the grid is refused — net_items does not move over a
#    20-tick window and the item stays in the inventory;
#  - a 16k disk holds 16,384 and a 64k disk 65,536, each named by the drive's own
#    info line;
#  - a full 64k disk refuses a 1-item deposit while the creative disk — given,
#    since it has no recipe upstream — takes the same deposit and keeps ingesting
#    past 65,536, its readout saying `unlimited`;
#  - a 4k disk keeps its 4,096 items across `save` + relaunch, beside the 1k
#    disk's 3 gold on one network.
#
# Slice 12 (the in-place disk upgrade):
#  - a 4k disk holding 4,096 cobblestone plus a 16k storage part make a 16k disk
#    in a crafting table (the package's own `refinedstorage:storage_disk_upgrade`
#    kind): the result carries the 4k disk's contents and the destination's
#    16,384-item capacity, the 4k disk is consumed, and the source disk's **own**
#    4k part is returned into the cell the disk left;
#  - the crafted disk inserted into the drive reads `4096/16384 items` and the
#    network totals 4,096.
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

# The package this leg belongs to. The leg runs a game the launcher compiled from
# this same tree, so the line names the *delivered* bytes, and the archive is
# checked against the tree's sources first: a package built before a doc or source
# edit must fail here rather than pass as "the delivery" (`verify-sdk-package.py`
# asks the same question of the archive).
package_line() {
    local hmod=$PORT/refinedstorage.hmod sha bytes drift
    if [ ! -f "$hmod" ]; then
        echo "package: absent ($hmod)"
        return 1
    fi
    sha=$(sha256sum "$hmod" | cut -d' ' -f1)
    bytes=$(stat -c %s "$hmod")
    drift=$(python3 - "$hmod" "$PORT" <<'PY'
import pathlib
import sys
import zipfile
hmod, port = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
with zipfile.ZipFile(hmod) as archive:
    for name in ("horizon-mod.toml", "src/lib.rs", "src/support.rs", "src/runtime.rs"):
        if archive.read(name) != (port / name).read_bytes():
            print(name)
PY
)
    echo "package: $sha $bytes refinedstorage.hmod"
    if [ -n "$drift" ]; then
        echo "package: the archive's $drift differs from the tree — rebuild before this leg"
        return 1
    fi
}
package_line || { echo "FAIL: the delivered package is not the tree's bytes" >&2; exit 1; }

mkdir -p "$TMP"
SCRATCH=$(mktemp -d "$TMP/refinedstorage-home.XXXXXX")
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl_raw() { "$CTL" --socket "$SOCK" "$@"; }
# Every control call is retried once. On a loaded box a single request
# occasionally answers nothing (the socket read returns empty while the game is
# fine), and with `set -e` that aborted a leg several phases in with no message.
# A refused command whose answer names an open (or dead) screen — a menu whose
# block was broken lives on, and the engine refuses every blocking command while
# it is up — is retried after closing it. A retry does not weaken an assertion:
# a wrong answer is still compared, and a genuinely broken command fails both
# attempts and aborts.
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
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }

# The non-null slots of the container at a position, as JSON.
slots() { ctl container "$1" "$2" "$3" | jq -c '[.slots[] | select(. != null)]'; }
# The same with the index every item sits in: a carried payload must come back
# to the slot it left, which the order-only list above cannot prove.
slots_indexed() { ctl container "$1" "$2" "$3" | jq -c '[.slots | to_entries[] | select(.value != null) | {slot: .key} + .value]'; }
# The open menu's container slots (the grid's view, the drive's disks).
view() { ctl screen | jq -c '[.slots[] | select(.slot >= 300 and .item != null)]'; }
count_of() { # count_of ITEM
    ctl inventory | jq -r --arg item "$1" \
        '[((.hotbar // [])[] | select(.item == $item) | .count), ((.main // [])[] | select(.item == $item) | .count)] | add // 0'
}
# Use the held item in the air: `ctl interact` leaves the crosshair on a block,
# and a use on the transmitter would bind again instead of opening the grid.
# Slice 9: the wireless grid is an item with its own 1,000 FE buffer, charged
# 5 FE per open, and it starts empty — so a use charges it first, as a player's
# charger would. `use_held_raw` skips that charge (the phase that proves an empty
# item opens nothing).
use_held() { charge_grid 1000; ctl look 0 -90 >/dev/null; ctl wait 2; ctl use-item; }
use_held_raw() { ctl look 0 -90 >/dev/null; ctl wait 2; ctl use-item; }
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
# The first empty player-inventory slot (0-35 in a container menu).
empty_inv_slot() {
    ctl screen | jq -r '[.slots[] | select(.slot < 36 and .item == null)][0].slot // empty'
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
# The transmitter's own info line (a second chat line, after the network's):
# `expect_info` reads the network line only.
expect_range() { # expect_range X Y Z TEXT
    local line
    line=$(reply "refinedstorage info $1 $2 $3" | grep -m1 'Wireless transmitter' || true)
    echo "  info($1,$2,$3) range: $line"
    case "$line" in
        *"$4"*) ;;
        *) fail "expected the transmitter's range line to say '$4', got '$line'" ;;
    esac
}
# A machine's own info line (a second chat line after the network's): its
# upgrade counts, the work interval they set, and its filter entries.
expect_machine() { # expect_machine X Y Z TEXT
    local line
    line=$(reply "refinedstorage info $1 $2 $3" | grep -m1 -E '^(Importer|Exporter)' || true)
    echo "  info($1,$2,$3) machine: $line"
    case "$line" in
        *"$4"*) ;;
        *) fail "expected the machine's info line to say '$4', got '$line'" ;;
    esac
}
# Open a block's menu, asking again when a click did not register: a right
# click is a press/release pair, and on a loaded box one occasionally reaches
# the server without opening the menu (measured while other legs ran here). The
# assertion is the slot count that follows, never the retry.
open_menu() { # open_menu X Y Z EXPECTED_CONTAINER_SLOTS
    local tries got
    for tries in 1 2 3; do
        ctl interact "$1" "$2" "$3" >/dev/null
        ctl wait 8
        got=$(ctl screen 2>/dev/null | jq -r '[ (.slots // [])[] | select(.slot >= 300)] | length' 2>/dev/null || echo 0)
        [ "${got:-0}" = "$4" ] && return 0
        ctl close >/dev/null 2>&1 || true
        ctl wait 4
    done
    fail "the menu at $1,$2,$3 has ${got:-0} container slots, expected $4: $(ctl screen | jq -c '{screen,slots:((.slots // [])|length)}')"
}
# How many items `/refinedstorage info` totals for a network.
net_items() { # net_items X Y Z
    info "$1" "$2" "$3" | sed -n 's/.*disks, \([0-9]*\) items.*/\1/p'
}
# A drive's own info line (a second chat line after the network's): every disk in
# it and the room its item gives it (`… 4096/4096 items`, and `… 65536/unlimited` for
# the creative variant, which reports no ceiling at all).
expect_drive() { # expect_drive X Y Z TEXT
    local line
    line=$(reply "refinedstorage info $1 $2 $3" | grep -m1 'Disk drive' || true)
    echo "  info($1,$2,$3) drive: $line"
    case "$line" in
        *"$4"*) ;;
        *) fail "expected the drive's info line to say '$4', got '$line'" ;;
    esac
}
# Fill a drive's first slot with a disk whose saved component holds COUNT
# cobblestone — the shape a disk carries between worlds (its item's own
# `refinedstorage:storage` component: `{capacity, stacks}`). CAPACITY is what the
# component records; 0 is the creative disk's unlimited marker.
fill_drive() { # fill_drive X Y Z ITEM CAPACITY COUNT
    ctl command "data merge block $1 $2 $3 {Items:[{Slot:0b,id:\"$4\",count:1,components:{\"refinedstorage:storage\":{capacity:$5,stacks:[{id:\"minecraft:cobblestone\",count:$6}]}}}]}" >/dev/null
    ctl wait 8
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

# Charge a network's buffer through the controller's own energy capability, the
# way an FE cable would: the harness has no generator at this pin, so the
# command is the source (upstream's controller is filled by its neighbours).
# `charge DIM X Y Z [amount]` names any block of the network, in that dimension.
# A network pays per tick (a grid 10, a drive 10 plus 4 a disk, a transmitter
# 16 plus 8 a range upgrade) and per work pass (a machine 2 plus its upgrades),
# exactly as upstream's `doWork` does, so a long leg charges before it reads.
charge() { # charge DIM X Y Z [amount]
    say "execute in $1 run refinedstorage energy $2 $3 $4 ${5:-1000}"
    ctl wait 4
}
# The same, asserted: the energy phase prints the reply and checks it names the
# buffer, so the charge command itself is evidence there.
charge_ok() { # charge_ok DIM X Y Z [amount]
    expect_reply "execute in $1 run refinedstorage energy $2 $3 $4 ${5:-1000}" "FE"
}
# The wireless grid's own buffer (slice 9), the item-side twin of the two above:
# upstream's grid item is an `ItemEnergyStorage` of 1,000 FE, and the harness has
# no charger at this pin.
charge_grid() { # charge_grid [amount]
    say "refinedstorage charge ${1:-1000}"
    ctl wait 4
}
# The held grid's buffer as `/refinedstorage grid` prints it: STORED CAPACITY
# TICK (`Wireless grid energy: <stored>/<capacity> FE, t=<server tick>`).
grid_energy() {
    reply "refinedstorage grid" \
        | sed -n 's/.*energy: \([0-9]*\)\/\([0-9]*\) FE, t=\([0-9]*\)$/\1 \2 \3/p'
}
# The network the current phase works on, and a top-up for it: every node pays
# its reference charge per tick (a grid 10, a drive 10 + 4 a disk, a
# transmitter 16 + 8 a range upgrade) and a machine per work pass, so a
# 1,000 FE buffer lasts ordinary leg lengths. The harness has no generator, so
# this stands in for one; a phase with no network set charges nothing.
NET=""
net() { # net DIM X Y Z
    NET="$1 $2 $3 $4"
}
charge_net() {
    # shellcheck disable=SC2086 # the four words are the network's coordinates
    [ -n "$NET" ] && charge $NET
}
# The network's energy as `/refinedstorage info` prints it: STORED CAPACITY TICK.
energy_of() { # energy_of X Y Z -> "stored capacity tick"
    info "$1" "$2" "$3" | sed -n 's/.*energy \([0-9]*\)\/\([0-9]*\) FE, drawing [0-9]*\/tick.* t=\([0-9]*\)$/\1 \2 \3/p'
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
net minecraft:overworld 40 64 0
# The player starts in the overworld; the network there is built and ticked now,
# then the chunks are force-loaded so a use from the End can still find it.
ctl command "execute in minecraft:overworld run tp @s 41.5 64 2.5"
charge_net
ctl wait 40
ctl command "execute in minecraft:overworld run fill 32 63 -8 56 63 24 minecraft:stone"
ctl command "execute in minecraft:overworld run fill 32 64 -8 56 74 24 minecraft:air"
ctl wait 10
ctl command "execute in minecraft:overworld run setblock 40 64 0 refinedstorage:controller"
ctl command "execute in minecraft:overworld run setblock 41 64 0 refinedstorage:cable"
ctl command "execute in minecraft:overworld run setblock 41 64 1 refinedstorage:wireless_transmitter[facing=north]"
ctl command "execute in minecraft:overworld run forceload add 32 0 48 8"
ctl wait 12
charge minecraft:overworld 40 64 0
expect_state 41 64 1 'facing=north'
expect_state 41 64 1 'active=true'
charge_net
expect_info 41 64 1 "online (3 nodes, 1 controller, 0 drive, 0 grid, 1 transmitter)"
# Back to the End: everything after this reads the End.
ctl command "execute in minecraft:the_end run tp @s 40.5 64 3.5"
charge_net
ctl wait 20

echo "== the network is built: controller, cable, disk drive, grid =="
net minecraft:the_end 40 64 0
ctl command "execute in minecraft:the_end run setblock 40 64 0 refinedstorage:controller"
ctl command "execute in minecraft:the_end run setblock 41 64 0 refinedstorage:cable"
ctl command "execute in minecraft:the_end run setblock 42 64 0 refinedstorage:disk_drive"
ctl command "execute in minecraft:the_end run setblock 43 64 0 refinedstorage:grid"
ctl wait 12
charge minecraft:the_end 40 64 0
expect_state 41 64 0 'west=true'
expect_state 41 64 0 'east=true'
expect_state 41 64 0 'north=false'
expect_state 43 64 0 'active=true'
charge_net
expect_info 43 64 0 "online (4 nodes, 1 controller, 1 drive, 1 grid); 0 disks, 0 items in 0 resources"

echo "== the wireless transmitter joins through its facing side only =="
net minecraft:the_end 52 64 4
ctl command "execute in minecraft:the_end run setblock 52 64 4 refinedstorage:controller"
ctl command "execute in minecraft:the_end run setblock 53 64 4 refinedstorage:cable"
ctl command "execute in minecraft:the_end run setblock 54 64 4 refinedstorage:wireless_transmitter[facing=east]"
ctl wait 12
# Facing away from the cable (east, nothing there): its own offline component.
expect_state 54 64 4 'facing=east'
expect_state 54 64 4 'active=false'
charge_net
expect_info 54 64 4 "offline (1 nodes, 0 controller, 0 drive, 0 grid, 1 transmitter)"
# Turned to face the cable (west): the controller's network, so active.
ctl command "execute in minecraft:the_end run setblock 54 64 4 refinedstorage:wireless_transmitter[facing=west]"
ctl wait 12
charge minecraft:the_end 52 64 4
expect_state 54 64 4 'facing=west'
expect_state 54 64 4 'active=true'
charge_net
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
charge minecraft:the_end 52 64 4
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
charge minecraft:the_end 52 64 4
expect_state 54 64 4 'active=true'
# A position that holds no transmitter is not a wireless use at all.
expect_reply "refinedstorage wireless 52 64 4 minecraft:the_end" "No wireless transmitter there."

echo "== a use from another dimension is refused, and only the dimension differs =="
# The overworld network sits at the same coordinates as this End spot, so the
# two commands below differ in the dimension the use happens in and nothing else.
ctl command "execute in minecraft:the_end run tp @s 41.5 64 2.5"
ctl wait 6
charge minecraft:overworld 40 64 0
expect_reply "refinedstorage wireless 41 64 1 minecraft:overworld" "refused (different dimension)"
expect_reply "execute in minecraft:overworld run refinedstorage wireless 41 64 1 minecraft:overworld" "accepted (1.6 of 16 blocks"

echo "== the item places against the clicked face and turns to face it =="
# The advancement before the item is obtained: the player's saved progress has no
# entry (the pre-check ran right after the arena).
# Upstream's `DefaultDirectionType.FACE_CLICKED` answers `clickedFace.getOpposite()`
# (toward the block the machine was placed on), which is what the slice-7 fix
# applies; the machine below is the same click, read back as `south`.
ctl command "execute in minecraft:the_end run setblock 55 64 6 minecraft:stone"
ctl command "execute in minecraft:the_end run tp @s 56.5 64 5.5"
ctl wait 6
ctl give refinedstorage:wireless_transmitter 1
ctl wait 4
ctl select refinedstorage:wireless_transmitter
ctl wait 4
charge_net
ctl interact 55 64 6 north
ctl wait 8
expect_state 55 64 5 'refinedstorage:wireless_transmitter'
expect_state 55 64 5 'facing=south'
charge_net
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
net minecraft:the_end 40 64 0
charge minecraft:the_end 40 64 0
ctl command "execute in minecraft:the_end run tp @s 42.5 64 2.5"
ctl wait 4
charge_net
open_menu 43 64 0 54
charge_net
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
charge_net
ctl interact 42 64 0
ctl wait 8
[ "$(count_of refinedstorage:1k_storage_disk)" = 0 ] \
    || fail "the disk stayed in the hand: $(ctl inventory | jq -c '[.hotbar[] | select(.item == "refinedstorage:1k_storage_disk")]')"
slots 42 64 0 | jq -e '[.[] | select(.item == "refinedstorage:1k_storage_disk")] | length == 1' >/dev/null \
    || fail "the drive does not hold the disk: $(slots 42 64 0)"
echo "  drive: $(slots 42 64 0)"
charge_net
expect_info 43 64 0 "1 disks, 0 items in 0 resources"

echo "== items deposited through the grid's menu are on the network =="
charge minecraft:the_end 40 64 0
ctl give minecraft:diamond 5
ctl give minecraft:gold_ingot 3
ctl wait 4
ctl command "execute in minecraft:the_end run tp @s 42.5 64 2.5"
ctl wait 4
charge_net
ctl interact 43 64 0
ctl wait 8
diamond_slot=$(inv_slot minecraft:diamond)
[ -n "$diamond_slot" ] || fail "no diamond stack in the menu's player inventory: $(ctl screen | jq -c '[.slots[] | select(.item != null)]')"
charge_net
ctl click-slot "$diamond_slot"
ctl wait 2
charge_net
ctl click-slot 300
ctl wait 8
gold_slot=$(inv_slot minecraft:gold_ingot)
[ -n "$gold_slot" ] || fail "no gold stack in the menu's player inventory"
charge_net
ctl click-slot "$gold_slot"
ctl wait 2
charge_net
ctl click-slot 301
ctl wait 8
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3},{"slot":301,"item":"minecraft:diamond","count":5}]'
[ "$(count_of minecraft:diamond)" = 0 ] || fail "5 diamonds are still in the inventory"
charge_net
expect_info 43 64 0 "1 disks, 8 items in 2 resources"

echo "== the search filters the grid's listing =="
charge minecraft:the_end 40 64 0
say "refinedstorage filter 43 64 0 gold"
ctl wait 8
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
say "refinedstorage filter 43 64 0 diamond"
ctl wait 8
charge_net
expect_slots '[{"slot":300,"item":"minecraft:diamond","count":5}]'
ctl messages 6 | jq -r '.chat[]' | grep -q 'Grid search: diamond' \
    || fail "no search feedback in chat"
say "refinedstorage filter 43 64 0"
ctl wait 8
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3},{"slot":301,"item":"minecraft:diamond","count":5}]'

echo "== a shift-click on the diamond view withdraws it into the inventory =="
charge minecraft:the_end 40 64 0
charge_net
ctl click-slot 301 shift
ctl wait 8
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
[ "$(count_of minecraft:diamond)" = 5 ] \
    || fail "the withdrawn diamonds are not in the inventory: $(ctl inventory | jq -c '[.hotbar // [], .main // []]')"
charge_net
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
charge_net
expect_info 43 64 0 "offline (3 nodes, 0 controller, 1 drive, 1 grid)"
ctl command "gamemode creative"
ctl wait 4
charge_net
ctl interact 43 64 0
ctl wait 8
if [ "$SELFTEST" = offline ]; then
    echo "(selftest: asserting the offline grid still lists the gold)"
    charge_net
    expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
else
    charge_net
    expect_slots '[]'
fi
ctl close
ctl wait 4
# Put the controller back: the gold comes back, so it lived in the disk.
ctl command "execute in minecraft:the_end run setblock 40 64 0 refinedstorage:controller"
ctl wait 12
charge minecraft:the_end 40 64 0
echo "  info after the restore: $(info 43 64 0)"
expect_state 43 64 0 'active=true'
charge_net
expect_info 43 64 0 "online (4 nodes, 1 controller, 1 drive, 1 grid); 1 disks, 3 items in 1 resources"
charge_net
ctl interact 43 64 0
ctl wait 8
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4

echo "== two grids on one network never offer the same items twice =="
charge minecraft:the_end 40 64 0
# A second grid joins the same component (on top of the drive). The first grid's
# view has already promised the 3 gold, so the second shows nothing: with the old
# per-grid view both showed the last 3 gold and both players could take them.
ctl command "execute in minecraft:the_end run setblock 42 65 0 refinedstorage:grid"
ctl wait 12
expect_state 42 65 0 'active=true'
charge_net
ctl interact 42 65 0
ctl wait 8
charge_net
expect_slots '[]'
ctl close
ctl wait 4
# The first grid still shows the gold: the stock was not consumed, only promised.
echo "  info after the second grid: $(info 43 64 0)"
charge minecraft:the_end 40 64 0
charge_net
ctl interact 43 64 0
ctl wait 8
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4
# Breaking the first grid releases its promise; the second then shows the gold.
ctl command "execute in minecraft:the_end run setblock 43 64 0 minecraft:air"
ctl wait 12
charge_net
ctl interact 42 65 0
ctl wait 8
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4
ctl command "execute in minecraft:the_end run setblock 43 64 0 refinedstorage:grid"
ctl command "execute in minecraft:the_end run setblock 42 65 0 minecraft:air"
ctl wait 12
charge_net
expect_info 43 64 0 "online (4 nodes, 1 controller, 1 drive, 1 grid)"

echo "== an online network with no disk drive lists nothing, beside it =="
net minecraft:the_end 48 64 0
ctl command "execute in minecraft:the_end run setblock 48 64 0 refinedstorage:controller"
ctl command "execute in minecraft:the_end run setblock 49 64 0 refinedstorage:cable"
ctl command "execute in minecraft:the_end run setblock 50 64 0 refinedstorage:grid"
ctl wait 12
charge minecraft:the_end 48 64 0
expect_state 50 64 0 'active=true'
charge_net
expect_info 50 64 0 "online (3 nodes, 1 controller, 0 drive, 1 grid); 0 disks, 0 items in 0 resources"
ctl command "execute in minecraft:the_end run tp @s 49.5 64 2.5"
ctl wait 4
charge_net
ctl interact 50 64 0
ctl wait 8
charge_net
expect_slots '[]'
ctl close
ctl wait 4
# The first network still lists its gold.
charge minecraft:the_end 40 64 0
ctl command "execute in minecraft:the_end run tp @s 42.5 64 2.5"
ctl wait 4
charge_net
ctl interact 43 64 0
ctl wait 8
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4

echo "== two networks in one dimension each show their own stock =="
net minecraft:the_end 48 64 0
charge minecraft:the_end 40 64 0
charge minecraft:the_end 48 64 0
# Network A (40..43,64,0) holds 3 gold. Network B (48..50,64,0) gets a drive of
# its own and the same 3 gold: each grid must list its own 3. Keyed by dimension
# alone, A's promise hid B's stock (and then B's promise hid A's), so one grid
# reported 0 while the disk under it held 3.
charge minecraft:the_end 48 64 0
ctl command "execute in minecraft:the_end run setblock 51 64 0 refinedstorage:disk_drive"
ctl wait 10
charge_net
expect_info 51 64 0 "online (4 nodes, 1 controller, 1 drive, 1 grid); 0 disks, 0 items in 0 resources"
ctl give refinedstorage:1k_storage_disk 1 >/dev/null
ctl wait 4
ctl select refinedstorage:1k_storage_disk >/dev/null
ctl wait 4
ctl command "execute in minecraft:the_end run tp @s 51.5 64 2.5"
ctl wait 4
charge_net
ctl interact 51 64 0
ctl wait 8
[ "$(count_of refinedstorage:1k_storage_disk)" = 0 ] \
    || fail "network B's disk stayed in the hand"
charge_net
expect_info 51 64 0 "1 disks, 0 items in 0 resources"
ctl close
ctl wait 4
# 3 gold deposited through network B's own grid.
charge minecraft:the_end 48 64 0
ctl give minecraft:gold_ingot 3 >/dev/null
ctl wait 4
ctl command "execute in minecraft:the_end run tp @s 49.5 64 2.5"
ctl wait 4
charge_net
ctl interact 50 64 0
ctl wait 8
gold_slot=$(inv_slot minecraft:gold_ingot)
[ -n "$gold_slot" ] || fail "no gold stack in network B's menu inventory"
charge_net
ctl click-slot "$gold_slot" shift
ctl wait 8
charge_net
expect_info 51 64 0 "1 disks, 3 items in 1 resources"
if [ "$SELFTEST" = networks ]; then
    echo "(selftest: asserting network B's grid hides network A's stock)"
    charge_net
    expect_slots '[]'
fi
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4
# The differential, with B's promise now live: A's grid still lists its own 3.
charge minecraft:the_end 40 64 0
ctl command "execute in minecraft:the_end run tp @s 42.5 64 2.5"
ctl wait 4
charge_net
ctl interact 43 64 0
ctl wait 8
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4
# And B's grid again, with both promises live: still its own 3.
charge minecraft:the_end 48 64 0
ctl command "execute in minecraft:the_end run tp @s 49.5 64 2.5"
ctl wait 4
charge_net
ctl interact 50 64 0
ctl wait 8
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4
# The stock was promised, not consumed: each disk still holds its own 3.
charge minecraft:the_end 40 64 0
charge minecraft:the_end 48 64 0
charge_net
expect_info 43 64 0 "1 disks, 3 items in 1 resources"
charge_net
expect_info 51 64 0 "1 disks, 3 items in 1 resources"

echo "== the controller's energy model: its buffer, its levels, and the gate =="
net minecraft:the_end 44 64 20
# Slice 8. Upstream's controller is the network's energy provider
# (`ControllerNetworkNode`, `CONTROLLER_CAPACITY` 1000) and its nodes work only
# when `stored >= getEnergyUsage()` (`calculateActive`), paying that charge in
# `doWork`. The reference's rates: a grid 10, a drive 10 plus 4 a disk, a
# transmitter 16 plus 8 a range upgrade, a machine 2 a work pass plus its
# upgrades. Two networks here: E1 (44..46,64,20) is controller + cable + a
# transmitter facing its cable, so its only charge is 16 FE a tick; E2 (36,64,20)
# is a lone controller, so a charge set on it stays put for the level readout.
ctl command "execute in minecraft:the_end run setblock 44 64 20 refinedstorage:controller"
ctl command "execute in minecraft:the_end run setblock 45 64 20 refinedstorage:cable"
ctl command "execute in minecraft:the_end run setblock 46 64 20 refinedstorage:wireless_transmitter[facing=west]"
ctl command "execute in minecraft:the_end run setblock 36 64 20 refinedstorage:controller"
ctl command "execute in minecraft:the_end run tp @s 46.5 64 22.5"
ctl wait 12
charge minecraft:the_end 44 64 20 1000
expect_reply "refinedstorage info 44 64 20" "drawing 16/tick"
# The charge is exact and is measured against the server tick counter the line
# prints, not against a wall clock: over `t2 - t1` ticks the buffer falls by
# 16 × that many FE. The window starts **full**: each harness call waits a few
# ticks and a loaded box stretches that, so a 500 FE top-up once ran dry inside
# the window (196 -> 4 over 16 ticks, a false red on 2026-11-22); 1,000 FE at
# 16 a tick covers ~60 ticks, and a node that cannot pay stops at the floor.
read -r s1 _ t1 <<<"$(energy_of 44 64 20)"
ctl wait 5
read -r s2 _ t2 <<<"$(energy_of 44 64 20)"
echo "  energy: $s1 -> $s2 FE over $((t2 - t1)) ticks"
[ $((s2)) -eq $((s1 - 16 * (t2 - t1))) ] \
    || fail "the transmitter's charge was $((s1 - s2)) FE over $((t2 - t1)) ticks, expected 16 a tick"
# Upstream's four energy levels, at its own thresholds (>= 40 %, >= 30 %,
# >= 1 %, else off) on a network that draws nothing.
for pair in "0 off" "50 nearly_off" "350 nearly_on" "500 on"; do
    set -- $pair
    charge minecraft:the_end 36 64 20 "$1"
    ctl wait 4
    expect_state 36 64 20 "energy_type=$2"
done
# No energy: the transmitter cannot pay its 16 FE, so it is inactive and the
# same use at 2.5 blocks is refused for energy — not for range.
charge minecraft:the_end 44 64 20 0
ctl wait 2
expect_state 46 64 20 'active=false'
if [ "$SELFTEST" = energy ]; then
    echo "(selftest: asserting a use with no energy is accepted)"
    expect_reply "refinedstorage wireless 46 64 20 minecraft:the_end" "accepted ("
else
    expect_reply "refinedstorage wireless 46 64 20 minecraft:the_end" "refused (not enough energy: 0 of 16 FE)"
fi
# The order of the checks: 17 blocks out with no energy is refused for energy
# (the node's activeness comes before the range), and the same 17 blocks with
# energy is refused naming the distance — so the energy refusal is not the range
# refusal in disguise.
ctl tp 46 64 37
ctl wait 4
charge minecraft:the_end 44 64 20 0
expect_reply "refinedstorage wireless 46 64 20 minecraft:the_end" "refused (not enough energy: 0 of 16 FE)"
charge minecraft:the_end 44 64 20 500
expect_reply "refinedstorage wireless 46 64 20 minecraft:the_end" "refused (out of range"
# With energy and inside the range the same use is accepted.
ctl tp 46.5 64 18.5
ctl wait 4
charge minecraft:the_end 44 64 20 500
expect_reply "refinedstorage wireless 46 64 20 minecraft:the_end" "accepted ("
# The grid's gate, on network A (which holds the 3 gold): at 0 FE it is dark and
# lists nothing while the disk keeps the gold; charged, it lists it again.
charge minecraft:the_end 40 64 0 0
ctl wait 6
expect_state 43 64 0 'active=false'
ctl command "execute in minecraft:the_end run tp @s 42.5 64 2.5"
ctl wait 4
charge_net
ctl interact 43 64 0
ctl wait 8
charge_net
expect_slots '[]'
ctl close
ctl wait 4
charge minecraft:the_end 40 64 0
ctl wait 6
expect_state 43 64 0 'active=true'
charge_net
ctl interact 43 64 0
ctl wait 8
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3}]'
ctl close
ctl wait 4

echo "== a stick is not a storage disk: the drive takes nothing =="
net minecraft:the_end 40 64 0
charge minecraft:the_end 40 64 0
ctl give minecraft:stick 1
ctl wait 4
ctl select minecraft:stick
ctl wait 4
ctl command "execute in minecraft:the_end run tp @s 41.5 64 2.5"
ctl wait 4
charge_net
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
charge_net
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
net minecraft:the_end 48 64 8
# A network of its own (48..50,64,8 with the drive at 49,64,9) inside the arena
# floor, so the player never falls out of the world while a leg waits; no block
# grid promises its stock, so the item's view must come from the disks itself.
ctl command "execute in minecraft:the_end run setblock 48 64 8 refinedstorage:controller"
ctl command "execute in minecraft:the_end run setblock 49 64 8 refinedstorage:cable"
ctl command "execute in minecraft:the_end run setblock 50 64 8 refinedstorage:wireless_transmitter[facing=west]"
ctl command "execute in minecraft:the_end run setblock 49 64 9 refinedstorage:disk_drive"
ctl command "execute in minecraft:the_end run tp @s 50.5 64 10.5"
ctl wait 14
charge minecraft:the_end 48 64 8
expect_state 50 64 8 'facing=west'
expect_state 50 64 8 'active=true'
charge_net
expect_info 49 64 9 "online (4 nodes, 1 controller, 1 drive, 0 grid, 1 transmitter); 0 disks, 0 items in 0 resources"

# The disk goes into the drive; the five diamonds are deposited through the
# item's own menu below.
ctl give refinedstorage:1k_storage_disk 1 >/dev/null
ctl wait 4
ctl select refinedstorage:1k_storage_disk >/dev/null
ctl wait 4
ctl command "execute in minecraft:the_end run tp @s 49.5 64 10.5"
ctl wait 4
charge minecraft:the_end 48 64 8
charge_net
ctl interact 49 64 9
ctl wait 8
[ "$(count_of refinedstorage:1k_storage_disk)" = 0 ] \
    || fail "the wireless network's disk stayed in the hand"
charge_net
expect_info 49 64 9 "1 disks, 0 items in 0 resources"

# The item is unbound until it is used on a transmitter: the use says so.
ctl give refinedstorage:wireless_grid 1 >/dev/null
ctl wait 4
ctl select refinedstorage:wireless_grid >/dev/null
ctl wait 4
expect_reply "refinedstorage bound" "The wireless grid is not bound."
ctl command "execute in minecraft:the_end run tp @s 50.5 64 10.5"
ctl wait 4
charge minecraft:the_end 48 64 8
charge_net
use_on_block 50 64 8 south
ctl wait 6
# The binding is the item's own component: the dimension and the block it was
# made on, read back from the stack (upstream's `GlobalPos`).
expect_reply "refinedstorage bound" "Wireless grid bound to 50,64,8 in minecraft:the_end."
# Binding is the item's use, which cancels the click: the transmitter's own
# menu (slice 6) does not open behind it.
ctl screen | jq -e '.screen != "container"' >/dev/null \
    || fail "binding the grid opened a menu: $(ctl screen | jq -c '{screen}')"

# A use inside the range opens the 9x6 view of that network.
ctl give minecraft:diamond 5 >/dev/null
ctl wait 4
charge minecraft:the_end 48 64 8
charge_net
use_held
ctl wait 10
ctl screen | jq -e '.screen == "container"' >/dev/null \
    || fail "the wireless grid did not open a container menu: $(ctl screen | jq -c '{screen,slots:(.slots|length)}')"
ctl screen | jq -e '[.slots[] | select(.slot >= 300)] | length == 54' >/dev/null \
    || fail "the wireless grid's menu does not have 54 container slots: $(ctl screen | jq -c '[.slots[] | select(.slot >= 300)] | length')"
if [ "$SELFTEST" = grid ]; then
    echo "(selftest: asserting the wireless grid's empty view lists a diamond)"
    charge_net
    expect_slots '[{"slot":300,"item":"minecraft:diamond","count":5}]'
fi
charge_net
expect_slots '[]'
# The menu's title names the item (upstream's `ContentNames.WIRELESS_GRID`).
ctl screen | jq -r '.title // .name // empty' | grep -qi 'wireless grid' \
    || echo "  (note: the screen reply names no title: $(ctl screen | jq -c '{title,name}'))"

# A shift-click on a player's inventory slot deposits that stack on the network.
charge minecraft:the_end 48 64 8
diamond_slot=$(inv_slot minecraft:diamond)
[ -n "$diamond_slot" ] || fail "no diamond stack in the wireless grid's player inventory"
charge_net
ctl click-slot "$diamond_slot" shift
ctl wait 10
charge_net
expect_slots '[{"slot":300,"item":"minecraft:diamond","count":5}]'
[ "$(count_of minecraft:diamond)" = 0 ] || fail "the deposited diamonds are still in the inventory"
charge_net
expect_info 49 64 9 "1 disks, 5 items in 1 resources"

# A shift-click on the view withdraws the stack into the inventory.
charge minecraft:the_end 48 64 8
charge_net
ctl click-slot 300 shift
ctl wait 10
charge_net
expect_slots '[]'
[ "$(count_of minecraft:diamond)" = 5 ] \
    || fail "the withdrawn diamonds are not in the inventory: $(ctl inventory | jq -c '[.hotbar // [], .main // []]')"
charge_net
expect_info 49 64 9 "1 disks, 0 items in 0 resources"
ctl close
ctl wait 4

# The range is the transmitter's: 17 blocks out is refused, naming the distance.
# The floor runs x 32..56, so both spots stay on it.
ctl tp 33 64 8
ctl wait 6
charge minecraft:the_end 48 64 8
echo "  bound in range 17: $(reply "refinedstorage bound")"
charge_net
use_held
ctl wait 6
refusal=$(ctl messages 8 | jq -r '.chat[]' | grep -m1 'Wireless grid .*: refused' || true)
echo "  use 17 blocks out: $refusal"
case "$refusal" in
    *"refused (out of range: 17.0 > 16 blocks)"*) ;;
    *) fail "a use 17 blocks out was not refused with its distance: '$refusal'" ;;
esac
ctl screen | jq -e '.screen != "container"' >/dev/null \
    || fail "a refused use opened the grid's menu"
# The differential: inside the range is accepted and opens the view. The spot is
# 15.5 blocks out rather than exactly 16.0: a `ctl tp` keeps the player's
# velocity, and a teleport landing on a moving body drifts by up to half a block
# before the ground friction stops it (measured: 16.4 after `tp 34 64 8`). The
# exact 16.0 boundary is the command's assertion above, which reads the same
# player position without a fresh teleport.
ctl tp 34.5 64 8
ctl wait 8
charge minecraft:the_end 48 64 8
charge_net
use_held
ctl wait 10
ctl screen | jq -e '.screen == "container"' >/dev/null \
    || fail "a use exactly 16 blocks out did not open the view: $(ctl screen | jq -c '{screen}')"
charge_net
expect_slots '[]'
ctl close
ctl wait 4

# The dimension is part of the binding: the same item, the same binding and the
# same player, refused from the overworld and accepted in the End. Only the
# dimension the use happens in differs, which is the differential.
ctl command "execute in minecraft:overworld run tp @s 41.5 64 2.5"
ctl wait 10
charge minecraft:the_end 48 64 8
charge_net
use_held
ctl wait 6
refusal=$(ctl messages 8 | jq -r '.chat[]' | grep -m1 'Wireless grid .*: refused' || true)
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
charge minecraft:the_end 48 64 8
charge_net
use_held
ctl wait 10
ctl screen | jq -e '.screen == "container"' >/dev/null \
    || fail "the same use in the bound dimension did not open the view: $(ctl screen | jq -c '{screen}')"
charge_net
expect_slots '[]'
ctl close
ctl wait 4

echo "== the wireless grid's own item buffer: 1,000 FE, 5 a use =="
net minecraft:the_end 48 64 8
# Slice 9. The grid is an item with its own 1,000 FE buffer (upstream
# `WIRELESS_GRID_CAPACITY`), charged 5 FE per open, insert and extract
# (`WIRELESS_GRID_OPEN`/`_INSERT`/`_EXTRACT`). It starts empty, so it is charged
# the way a charger would: `/refinedstorage charge`, the item-side twin of
# `/refinedstorage energy` for the network's buffer.
ctl command "execute in minecraft:the_end run tp @s 50.5 64 10.5"
ctl wait 6
charge minecraft:the_end 48 64 8
charge_net
expect_reply "refinedstorage grid" "/1000 FE, t="
expect_reply "refinedstorage charge 1000" "1000/1000 FE"
read -r g1 _ t1 <<<"$(grid_energy)"
echo "  item energy: $g1/1000 FE at t=$t1"
[ "$g1" = 1000 ] || fail "the charged grid reads $g1 FE, expected 1000"
# Ticks alone cost nothing: unlike the network's nodes, an item's buffer is not
# drained per tick — its charges are per operation. (The network beside it does
# fall over the same window; that is the differential.)
charge minecraft:the_end 48 64 8
ctl wait 6
read -r g2 _ t2 <<<"$(grid_energy)"
echo "  item energy: $g2/1000 FE at t=$t2 ($((t2 - t1)) ticks later)"
[ "$t2" -gt "$t1" ] || fail "the tick counter did not advance: $t1 -> $t2"
[ "$g2" = "$g1" ] \
    || fail "the item's buffer fell $((g1 - g2)) FE over $((t2 - t1)) ticks without a use"
# The open costs exactly 5.
charge_net
use_held_raw
ctl wait 10
ctl screen | jq -e '.screen == "container"' >/dev/null \
    || fail "a charged wireless grid did not open its view"
read -r g3 _ t3 <<<"$(grid_energy)"
echo "  item energy: $g3/1000 FE at t=$t3 after the open"
[ $((g1 - g3)) -eq 5 ] || fail "opening the grid took $((g1 - g3)) FE, expected 5"
# The deposit costs 5 (the phase above withdrew 5 diamonds into the inventory).
charge minecraft:the_end 48 64 8
diamond_slot=$(inv_slot minecraft:diamond)
[ -n "$diamond_slot" ] || fail "no diamond stack in the wireless grid's player inventory"
charge_net
ctl click-slot "$diamond_slot" shift
ctl wait 10
read -r g4 _ t4 <<<"$(grid_energy)"
echo "  item energy: $g4/1000 FE at t=$t4 after the deposit"
[ $((g3 - g4)) -eq 5 ] || fail "the deposit took $((g3 - g4)) FE of the item's buffer, expected 5"
charge_net
expect_slots '[{"slot":300,"item":"minecraft:diamond","count":5}]'
# The withdrawal costs 5.
charge minecraft:the_end 48 64 8
charge_net
ctl click-slot 300 shift
ctl wait 10
read -r g5 _ t5 <<<"$(grid_energy)"
echo "  item energy: $g5/1000 FE at t=$t5 after the withdrawal"
[ $((g4 - g5)) -eq 5 ] || fail "the withdrawal took $((g4 - g5)) FE of the item's buffer, expected 5"
charge_net
expect_slots '[]'
[ "$(count_of minecraft:diamond)" = 5 ] \
    || fail "the withdrawn diamonds are not in the inventory: $(ctl inventory | jq -c '[.hotbar // [], .main // []]')"
# 0 item FE: the same deposit is refused for the item's energy, while the
# network stays online and draws from its own buffer.
expect_reply "refinedstorage charge 0" "0/1000 FE"
charge minecraft:the_end 48 64 8
charge_net
expect_info 49 64 9 "online (4 nodes, 1 controller, 1 drive, 0 grid, 1 transmitter)"
network_before=$(net_items 49 64 9)
diamond_slot=$(inv_slot minecraft:diamond)
charge_net
ctl click-slot "$diamond_slot" shift
ctl wait 8
refusal=$(ctl messages 8 | jq -r '.chat[]' | grep -m1 'refused (not enough item energy' || true)
echo "  deposit at 0 item FE: $refusal"
case "$refusal" in
    *"not enough item energy: 0 of 5 FE"*) ;;
    *) fail "a deposit at 0 item FE was not refused for the item's energy: '$refusal'" ;;
esac
if [ "$SELFTEST" = itemenergy ]; then
    echo "(selftest: asserting the deposit at 0 item FE went through)"
    [ "$(count_of minecraft:diamond)" = 0 ] \
        || fail "expected the 0-FE deposit to have moved the stack, it is still in the inventory"
fi
[ "$(count_of minecraft:diamond)" = 5 ] || fail "the refused deposit still left the inventory"
[ "$(net_items 49 64 9)" = "$network_before" ] \
    || fail "the network's item count changed on a refused deposit: $network_before -> $(net_items 49 64 9)"
# The network's own buffer is untouched by the item's emptiness: it is online and
# its info line still prints the network's energy.
charge minecraft:the_end 48 64 8
expect_info 49 64 9 "FE, drawing"
# A use at 0 item FE opens nothing and names the item's buffer; the same use,
# charged, opens (the differential for the open's charge).
ctl close
ctl wait 4
expect_reply "refinedstorage charge 0" "0/1000 FE"
charge minecraft:the_end 48 64 8
charge_net
use_held_raw
ctl wait 6
refusal=$(ctl messages 8 | jq -r '.chat[]' | grep -m1 'refused (not enough item energy' || true)
echo "  open at 0 item FE: $refusal"
case "$refusal" in
    *"not enough item energy: 0 of 5 FE"*) ;;
    *) fail "a use at 0 item FE was not refused for the item's energy: '$refusal'" ;;
esac
ctl screen | jq -e '.screen != "container"' >/dev/null \
    || fail "a use at 0 item FE opened the grid's menu"
expect_reply "refinedstorage charge 1000" "1000/1000 FE"
charge_net
use_held_raw
ctl wait 10
ctl screen | jq -e '.screen == "container"' >/dev/null \
    || fail "the same use with the item charged did not open the view"
read -r g6 _ _ <<<"$(grid_energy)"
echo "  item energy: $g6/1000 FE after the charged open"
[ $((1000 - g6)) -eq 5 ] || fail "the open after a full charge took $((1000 - g6)) FE, expected 5"
ctl close
ctl wait 4

echo "== the range upgrade: the transmitter's own slot raises its range =="
net minecraft:the_end 48 64 8
# Upstream's transmitter has a four-slot upgrade container behind its menu
# (`WirelessTransmitterBlockEntity`), and its range is the base 16 plus 8 per
# range upgrade (`WirelessTransmitterRangeModifier`, `rangeUpgradeRange` 8,
# maxAmount 4). The upgrade goes into that slot and is read back before the
# rule is asked.
ctl command "execute in minecraft:the_end run tp @s 50.5 64 10.5"
ctl wait 6
charge minecraft:the_end 48 64 8
ctl container 50 64 8 refinedstorage:range_upgrade 1 >/dev/null
ctl wait 4
upgrade_slots=$(slots 50 64 8)
echo "  transmitter slots: $upgrade_slots"
echo "$upgrade_slots" | jq -e 'length == 1 and .[0].item == "refinedstorage:range_upgrade" and .[0].count == 1' >/dev/null \
    || fail "the transmitter's upgrade slot does not hold the range upgrade: $upgrade_slots"
charge_net
expect_range 50 64 8 "range 24 blocks, active"

# The differential: the same 17.0-block use the phase above refused with 16
# blocks is accepted with the upgrade, and names the raised range.
ctl tp 33 64 8
ctl wait 6
charge minecraft:the_end 48 64 8
if [ "$SELFTEST" = range ]; then
    echo "(selftest: asserting a use 17 blocks out is still refused with the range upgrade in)"
    expect_reply "refinedstorage wireless 50 64 8 minecraft:the_end" "refused (out of range: 17.0 > 16 blocks"
else
    expect_reply "refinedstorage wireless 50 64 8 minecraft:the_end" "accepted (17.0 of 24 blocks"
fi
# And the wireless grid item itself opens the view from the same spot.
ctl select refinedstorage:wireless_grid >/dev/null
ctl wait 4
charge minecraft:the_end 48 64 8
charge_net
use_held
ctl wait 10
opened=$(ctl messages 8 | jq -r '.chat[]' | grep -m1 'Wireless grid .*: opened' || true)
echo "  use 17 blocks out with the range upgrade: $opened"
case "$opened" in
    *"opened (17.0 of 24 blocks"*) ;;
    *) fail "the wireless grid did not open 17.0 blocks out with the range upgrade: '$opened'" ;;
esac
ctl screen | jq -e '.screen == "container"' >/dev/null \
    || fail "the raised range did not open the grid's view: $(ctl screen | jq -c '{screen}')"
charge_net
expect_slots '[]'
ctl close
ctl wait 4

echo "== the transmitter's menu: four upgrade slots, and only upgrades fit =="
# An empty hand: the right click is the block's, which opens the transmitter's
# menu (its container's menu type, API 85).
ctl command "execute in minecraft:the_end run tp @s 50.5 64 10.5"
ctl command "clear @s" >/dev/null
ctl wait 4
charge minecraft:the_end 48 64 8
charge_net
open_menu 50 64 8 4
menu_slots=$(ctl screen | jq '[.slots[] | select(.slot >= 300)] | length')
echo "  transmitter menu slots: $menu_slots"
[ "$menu_slots" = 4 ] || fail "the transmitter's menu has $menu_slots container slots, expected 4"
ctl screen | jq -e '[.slots[] | select(.slot >= 300 and .item != null) | .item] == ["refinedstorage:range_upgrade"]' >/dev/null \
    || fail "the transmitter's menu does not show the range upgrade: $(ctl screen | jq -c '[.slots[] | select(.slot >= 300 and .item != null)]')"
# Only an upgrade fits: a diamond shift-clicked from the inventory is refused
# (upstream `UpgradeContainer.canPlaceItem`) and stays in the inventory.
ctl give minecraft:diamond 1 >/dev/null
ctl wait 4
diamond_slot=$(inv_slot minecraft:diamond)
[ -n "$diamond_slot" ] || fail "no diamond stack in the transmitter menu's inventory"
charge_net
ctl click-slot "$diamond_slot" shift
ctl wait 8
ctl screen | jq -e '[.slots[] | select(.slot >= 300 and .item == "minecraft:diamond")] == []' >/dev/null \
    || fail "a diamond entered the transmitter's upgrade slot"
[ "$(count_of minecraft:diamond)" = 1 ] || fail "the refused diamond left the inventory"
charge_net
expect_range 50 64 8 "range 24 blocks, active"
ctl close
ctl wait 4

echo "== the creative range upgrade: unlimited, and still one dimension =="
ctl command "execute in minecraft:the_end run tp @s 50.5 64 10.5"
ctl wait 4
charge minecraft:the_end 48 64 8
ctl container 50 64 8 refinedstorage:creative_range_upgrade 1 >/dev/null
ctl wait 4
echo "  transmitter slots: $(slots 50 64 8)"
if [ "$SELFTEST" = creative ]; then
    echo "(selftest: asserting the creative upgrade leaves the finite range)"
    charge_net
    expect_range 50 64 8 "range 48 blocks, active"
fi
charge_net
expect_range 50 64 8 "range unlimited, active"
# A platform 70.5 blocks out: beyond the 48-block maximum of four range
# upgrades, so only the creative upgrade accepts it.
ctl command "execute in minecraft:the_end run fill 118 63 -2 122 63 2 minecraft:stone"
ctl tp 120 64 0
ctl wait 8
charge minecraft:the_end 48 64 8
expect_reply "refinedstorage wireless 50 64 8 minecraft:the_end" "accepted (70.5 of unlimited"
# The creative upgrade gives no cross-dimension access: the dimension is
# checked first, as it was without it.
expect_reply "execute in minecraft:overworld run refinedstorage wireless 50 64 8 minecraft:the_end" "refused (different dimension)"
ctl command "execute in minecraft:the_end run tp @s 50.5 64 10.5"
ctl wait 6

echo "== the importer: the work interval, the quota and the filter =="
net minecraft:the_end 40 64 16
# Slice 7. Upstream's importer (`ImporterNetworkNode`) reads the inventory its
# `direction` points at; the work ticker is `UpgradeContainer`'s (9 ticks, two
# fewer per speed upgrade), and one pass moves 1 item, or 64 with a stack
# upgrade (`ItemHandlerImporterTransferStrategyFactory`).
# The network at z=16: controller 40 - cable 41 - drive 42 - importer 43 -
# source 44. The source is a hopper: a chest's own use opens the chest's menu
# instead of placing a block against it, and a hopper's five slots are an
# ordinary `ITEMS` handler. The importer's `facing` is what makes the transfer
# work at all, so the legs below prove it (a machine facing the wrong neighbour
# moves nothing).
ctl command "execute in minecraft:the_end run setblock 40 64 16 refinedstorage:controller"
ctl command "execute in minecraft:the_end run setblock 41 64 16 refinedstorage:cable"
ctl command "execute in minecraft:the_end run setblock 42 64 16 refinedstorage:disk_drive"
ctl command "execute in minecraft:the_end run setblock 43 64 16 refinedstorage:importer[facing=east]"
ctl command "execute in minecraft:the_end run setblock 44 64 16 minecraft:hopper[enabled=true,facing=down]"
ctl wait 10
charge minecraft:the_end 40 64 16
expect_state 43 64 16 'facing=east'
charge_net
expect_info 42 64 16 "online (4 nodes, 1 controller, 1 drive, 0 grid, 1 importer)"
charge_net
expect_machine 43 64 16 "0 speed upgrades, 0 stack upgrades, work interval 9 ticks, 0 filters"
# The two blocks' items are real items: given, read back, and used below.
ctl give refinedstorage:importer 1 >/dev/null
ctl give refinedstorage:exporter 1 >/dev/null
ctl wait 4
[ "$(count_of refinedstorage:importer)" = 1 ] || fail "the importer item was not given"
[ "$(count_of refinedstorage:exporter)" = 1 ] || fail "the exporter item was not given"
echo "  items given: importer $(count_of refinedstorage:importer), exporter $(count_of refinedstorage:exporter)"
ctl command "clear @s" >/dev/null
ctl wait 2
# A disk, then 64 stone in the source (hopper) the importer faces.
ctl give refinedstorage:1k_storage_disk 1 >/dev/null
ctl wait 4
ctl select refinedstorage:1k_storage_disk >/dev/null
ctl command "execute in minecraft:the_end run tp @s 41.5 64 16.5" >/dev/null
ctl wait 4
charge_net
ctl interact 42 64 16
ctl wait 8
# The network's charge as the info line names it: the drive's 10 plus 4 for its
# one disk, and the importer's 2 a work pass (upstream's `DefaultEnergyUsage`).
charge_net
expect_info 42 64 16 "1 disks, 0 items in 0 resources; energy "
charge_net
expect_info 42 64 16 "drawing 14/tick + 2 per work pass"
ctl container 44 64 16 minecraft:stone 64 >/dev/null
ctl wait 4
charge minecraft:the_end 40 64 16
echo "  importer source: $(slots 44 64 16)"
# A machine its network cannot pay does no pass: at 0 FE the same 20 ticks move
# nothing, while the driver and the source are unchanged. Charged, the same
# window moves again — the differential is the charge and nothing else.
saved_net=$NET
NET="" # no top-up during the starved window: the window *is* the 0 FE state
charge minecraft:the_end 40 64 16 0
starved_base=$(net_items 42 64 16)
ctl wait 20
starved=$(net_items 42 64 16)
NET=$saved_net
echo "  after 20 ticks at 0 FE: ${starved:-0} items on the network (was ${starved_base:-0})"
[ "${starved:-0}" = "${starved_base:-0}" ] \
    || fail "an importer its network cannot pay moved $((starved - starved_base)) items"
charge minecraft:the_end 40 64 16
# The un-upgraded machine moves one item per nine ticks: a 20-tick window sees
# the first pass and one or two more, never a stack.
charge_net
ctl wait 20
slow=$(net_items 42 64 16)
echo "  after 20 ticks without an upgrade: $slow items on the network, source $(slots 44 64 16)"
if [ "$SELFTEST" = speed ]; then
    echo "(selftest: asserting the un-upgraded importer moved more than 15 items in 20 ticks)"
    slow=64
fi
[ "${slow:-0}" -ge 1 ] && [ "${slow:-0}" -le 5 ] \
    || fail "the un-upgraded importer moved $slow items in 20 ticks, expected 1 to 5"
# The stack upgrade: one pass moves 64 instead of 1, so the source empties in a
# single pass and the network holds all 64.
ctl give refinedstorage:stack_upgrade 1 >/dev/null
ctl wait 4
ctl command "execute in minecraft:the_end run tp @s 43.5 64 17.5" >/dev/null
ctl wait 4
charge_net
open_menu 43 64 16 13
up=$(inv_slot refinedstorage:stack_upgrade)
[ -n "$up" ] || fail "no stack upgrade in the importer menu's inventory"
charge_net
ctl click-slot "$up"
ctl wait 2
charge_net
ctl click-slot 309
ctl wait 8
ctl close
ctl wait 4
upgrade_slots=$(slots 43 64 16)
echo "  importer slots: $upgrade_slots"
echo "$upgrade_slots" | jq -e '[.[] | select(.item == "refinedstorage:stack_upgrade")] | length == 1' >/dev/null \
    || fail "the importer's upgrade slot does not hold the stack upgrade: $upgrade_slots"
charge_net
expect_machine 43 64 16 "1 stack upgrade, work interval 9 ticks"
charge minecraft:the_end 40 64 16
charge_net
ctl wait 20
fast=$(net_items 42 64 16)
echo "  after the stack upgrade: $fast items on the network, source $(slots 44 64 16)"
if [ "$SELFTEST" = stack ]; then
    echo "(selftest: asserting the stack upgrade left the un-upgraded count)"
    [ "${fast:-0}" = 5 ] || fail "with a stack upgrade the importer moved to $fast items, expected 5"
fi
[ "${fast:-0}" = 64 ] || fail "the importer with a stack upgrade moved to $fast items, expected 64"
echo "$(slots 44 64 16)" | jq -e 'length == 0' >/dev/null \
    || fail "the importer with a stack upgrade left $(slots 44 64 16) in its source"

echo "== the importer: four speed upgrades shorten the interval to one tick =="
# Take the stack upgrade back out and put four speed upgrades in: upstream's
# container is four slots, one item each, and `notifyListener` takes two ticks
# off the interval per speed upgrade (9, 7, 5, 3, 1).
ctl command "execute in minecraft:the_end run tp @s 43.5 64 17.5" >/dev/null
ctl wait 4
charge_net
ctl interact 43 64 16
ctl wait 8
stack_slot=$(ctl screen | jq -r '[.slots[] | select(.slot >= 309 and .slot <= 312 and .item == "refinedstorage:stack_upgrade")][0].slot // empty')
[ -n "$stack_slot" ] || fail "the stack upgrade is not in the importer's upgrade row"
charge_net
ctl click-slot "$stack_slot"
ctl wait 2
free=$(empty_inv_slot)
[ -n "$free" ] || fail "no empty inventory slot to put the stack upgrade back into"
charge_net
ctl click-slot "$free"
ctl wait 4
ctl close
ctl wait 4
ctl command "clear @s" >/dev/null
ctl wait 2
# One speed upgrade at a time, each read back from the machine's own slots.
for n in 1 2 3 4; do
    ctl give refinedstorage:speed_upgrade 1 >/dev/null
    ctl wait 3
    ctl command "execute in minecraft:the_end run tp @s 43.5 64 17.5" >/dev/null
    charge_net
    ctl interact 43 64 16
    ctl wait 8
    speed_slot=$(inv_slot refinedstorage:speed_upgrade)
    [ -n "$speed_slot" ] || fail "no speed upgrade in the importer menu's inventory"
    charge_net
    ctl click-slot "$speed_slot"
    ctl wait 2
    charge_net
    ctl click-slot $((308 + n))
    ctl wait 8
    ctl close
    ctl wait 4
done
echo "  importer slots: $(slots 43 64 16)"
[ "$(slots 43 64 16 | jq '[.[] | select(.item == "refinedstorage:speed_upgrade")] | length')" = 4 ] \
    || fail "the importer does not hold four speed upgrades: $(slots 43 64 16)"
charge_net
expect_machine 43 64 16 "4 speed upgrades, 0 stack upgrades, work interval 1 tick"
# The differential: with four speed upgrades the same 20-tick window moves a
# stack's worth (one per tick), not one per nine ticks.
ctl container 44 64 16 minecraft:dirt 64 >/dev/null
ctl wait 4
charge minecraft:the_end 40 64 16
charge_net
ctl wait 20
rapid=$(net_items 42 64 16)
echo "  after 20 ticks with four speed upgrades: $rapid items on the network"
# One pass per tick: about 20 more items, far past the 1 to 5 the same window
# moved without them (and the differential is that exact comparison).
[ "${rapid:-0}" -ge 79 ] || fail "four speed upgrades moved to $rapid items in 20 ticks, expected at least 79"
charge minecraft:the_end 40 64 16
charge_net
ctl wait 60
charge_net
expect_info 42 64 16 "128 items in 2 resources"

echo "== the importer's filter blocks the resource it names =="
# Upstream's `Filter` is in BLOCK mode: an empty filter blocks nothing (which
# is why the source above was imported whole), and a listed resource is the one
# that is not imported. Dirt is named, so the stone goes and the dirt stays.
ctl container 43 64 16 minecraft:dirt 1 >/dev/null
ctl wait 4
charge minecraft:the_end 40 64 16
ctl container 44 64 16 minecraft:stone 5 >/dev/null
ctl container 44 64 16 minecraft:dirt 5 >/dev/null
ctl wait 4
echo "  importer filter: $(slots 43 64 16)"
charge_net
expect_machine 43 64 16 "1 filter"
charge_net
ctl wait 30
echo "  source after the filtered pass: $(slots 44 64 16)"
[ "$(slots 44 64 16 | jq '[.[] | select(.item == "minecraft:stone")] | length')" = 0 ] \
    || fail "the filtered importer did not import the stone: $(slots 44 64 16)"
[ "$(slots 44 64 16 | jq '[.[] | select(.item == "minecraft:dirt")][0].count // 0')" = 5 ] \
    || fail "the filtered importer imported the blocked dirt: $(slots 44 64 16)"

echo "== the exporter: the resources its filter names leave the network =="
net minecraft:the_end 40 64 16
# The exporter at 41,64,17 faces south into the chest at 41,64,18 and joins the
# network through the cable above it. Upstream's exporter works one filter entry
# at a time (`ExporterNetworkNode`), so an empty filter exports nothing.
ctl command "execute in minecraft:the_end run setblock 41 64 17 refinedstorage:exporter[facing=south]" >/dev/null
ctl command "execute in minecraft:the_end run setblock 41 64 18 minecraft:chest" >/dev/null
ctl wait 12
charge_net
expect_info 41 64 17 "online (5 nodes, 1 controller, 1 drive, 0 grid, 1 importer, 1 exporter)"
charge_net
expect_machine 41 64 17 "0 filters"
charge minecraft:the_end 40 64 16
before=$(net_items 41 64 17)
charge_net
ctl wait 20
after=$(net_items 41 64 17)
echo "  exporter with no filter: $before -> $after items"
[ "$before" = "$after" ] || fail "an exporter without a filter moved items: $before -> $after"
# One gold ingot in the filter, four gold on the network: only the gold leaves.
ctl container 41 64 17 minecraft:gold_ingot 1 >/dev/null
ctl wait 4
charge_net
expect_machine 41 64 17 "1 filter"
# The gold goes on the network the same way everything else did: into the
# hopper the importer faces (its filter blocks dirt only). The exporter has its
# filter already, so it drains the gold as fast as the importer brings it: what
# proves the pass is the *destination* gaining four and the network returning to
# where it was, not a spike on the network.
gold_base=$(net_items 41 64 17)
charge minecraft:the_end 40 64 16
ctl container 44 64 16 minecraft:gold_ingot 4 >/dev/null
charge_net
ctl wait 60
echo "  exporter destination: $(slots 41 64 18)"
[ "$(slots 41 64 18 | jq '[.[] | select(.item == "minecraft:gold_ingot")][0].count // 0')" = 4 ] \
    || fail "the exporter did not move the 4 gold: $(slots 41 64 18)"
# The filter is what left: the stone and the dirt are still on the network.
echo "  exporter network after the pass: $(net_items 41 64 17) items (was $gold_base)"
[ "$(net_items 41 64 17)" = "$gold_base" ] || fail "the exporter moved more than its filter named"

echo "== a machine's upgrade slot refuses what is not an upgrade =="
ctl command "execute in minecraft:the_end run tp @s 43.5 64 17.5" >/dev/null
ctl command "clear @s" >/dev/null
ctl wait 4
charge_net
open_menu 43 64 16 13
ctl give minecraft:diamond 1 >/dev/null
ctl wait 4
diamond_slot=$(inv_slot minecraft:diamond)
[ -n "$diamond_slot" ] || fail "no diamond in the machine menu's inventory"
charge_net
ctl click-slot "$diamond_slot"
ctl wait 2
charge_net
ctl click-slot 309
ctl wait 6
ctl screen | jq -e '[.slots[] | select(.slot >= 300 and .slot <= 312 and .item == "minecraft:diamond")] == []' >/dev/null \
    || fail "a diamond entered a machine's upgrade slot"
free=$(empty_inv_slot)
[ -n "$free" ] || fail "no empty inventory slot to put the refused diamond back into"
charge_net
ctl click-slot "$free"
ctl wait 4
ctl close
ctl wait 4
[ "$(count_of minecraft:diamond)" = 1 ] || fail "the refused diamond left the menu"
ctl command "clear @s" >/dev/null
ctl wait 4

echo "== both machine blocks drop themselves =="
# The exporter's own shape is the cable core plus its three steps on the face
# it works through; with the destination chest removed the ray from the south
# reaches the core.
ctl command "execute in minecraft:the_end run setblock 41 64 18 minecraft:air" >/dev/null
ctl command "gamemode survival" >/dev/null
ctl give minecraft:diamond_pickaxe 1 >/dev/null
ctl wait 4
ctl select minecraft:diamond_pickaxe >/dev/null
ctl command "execute in minecraft:the_end run tp @s 41.5 64 19.5" >/dev/null
ctl wait 6
broken=$(ctl break 41 64 17) || fail "cannot break the exporter: $broken"
echo "  exporter drops: $(echo "$broken" | jq -c '.drops')"
[ "$(echo "$broken" | jq -r '.drops["refinedstorage:exporter"] // 0')" -ge 1 ] \
    || fail "the exporter did not drop itself: $(echo "$broken" | jq -c '.drops')"
# The importer: the same core, from the south, inside reach.
ctl command "execute in minecraft:the_end run tp @s 43.5 64 17.5" >/dev/null
ctl wait 6
broken=$(ctl break 43 64 16) || fail "cannot break the importer: $broken"
echo "  importer drops: $(echo "$broken" | jq -c '.drops')"
[ "$(echo "$broken" | jq -r '.drops["refinedstorage:importer"] // 0')" -ge 1 ] \
    || fail "the importer did not drop itself: $(echo "$broken" | jq -c '.drops')"
ctl command "gamemode creative" >/dev/null
ctl command "clear @s" >/dev/null
ctl wait 4

echo "== a broken machine carries its own slots on its item =="
# Slice 10, from two gap lines: slice 6's "the transmitter's upgrade drops when
# the block is broken | not ported — the game's mod block entity drops an empty
# item", and slice 7's deviation 5 (a machine's upgrades and filter items are
# lost the same way). The engine spills a mod block entity's declared container
# on removal and drops the block's item empty — `canonical()` answers None for a
# mod type id, so `minecraft:block_entity_data` is never merged back in on
# placement. The port takes the slots *before* the removal and puts a fresh
# payload on the drop, so the item carries them and nothing spills; a survival
# break, a pickup and a re-place must find every slot where it was, and the
# drops must not list the items a second time (the duplication a whole-compound
# copy caused in another port's review).
net minecraft:the_end 40 64 16
ctl command "execute in minecraft:the_end run kill @e[type=item]" >/dev/null 2>&1 || true
# The hopper the slice-7 importer read is not needed here, and an empty
# neighbour keeps the machine's own work from moving anything mid-phase.
ctl command "execute in minecraft:the_end run setblock 44 64 16 minecraft:air" >/dev/null
ctl command "gamemode creative" >/dev/null
ctl command "clear @s" >/dev/null
ctl wait 6
# The slice-7 phase above broke this importer; put a fresh one back.
ctl command "execute in minecraft:the_end run setblock 43 64 16 refinedstorage:importer[facing=east]" >/dev/null
ctl wait 8
# Its 13 slots, filled directly: four speed upgrades (slots 0-3), one stack
# upgrade (4) and a dirt filter entry (5). `ctl container` fills the first empty
# slot one item at a time, so each call lands where the assertion expects.
for _ in 1 2 3 4; do ctl container 43 64 16 refinedstorage:speed_upgrade 1 >/dev/null; done
ctl container 43 64 16 refinedstorage:stack_upgrade 1 >/dev/null
ctl container 43 64 16 minecraft:dirt 1 >/dev/null
ctl wait 6
machine_before=$(slots_indexed 43 64 16)
echo "  importer before the break: $machine_before"
[ "$(echo "$machine_before" | jq 'length')" = 6 ] \
    || fail "the importer did not take the six slot items: $machine_before"
ctl command "gamemode survival" >/dev/null
ctl give minecraft:diamond_pickaxe 1 >/dev/null
ctl wait 4
ctl select minecraft:diamond_pickaxe >/dev/null
ctl command "execute in minecraft:the_end run tp @s 43.5 64 17.5" >/dev/null
ctl wait 6
broken=$(ctl break 43 64 16) || fail "cannot break the importer: $broken"
echo "  importer drops: $(echo "$broken" | jq -c '.drops')"
[ "$(echo "$broken" | jq -r '.drops["refinedstorage:importer"] // 0')" -ge 1 ] \
    || fail "the importer did not drop itself: $(echo "$broken" | jq -c '.drops')"
# None of the six is on the ground: the item is the only copy.
for spilled in refinedstorage:speed_upgrade refinedstorage:stack_upgrade minecraft:dirt; do
    [ "$(echo "$broken" | jq -r --arg i "$spilled" '.drops[$i] // 0')" = 0 ] \
        || fail "the importer's $spilled spilled as well as travelling on the item: $(echo "$broken" | jq -c '.drops')"
done
# The drop sits at the block's centre; the player steps onto it, then aside to
# place it back at the same position.
ctl command "execute in minecraft:the_end run tp @s 43.5 64 16.5" >/dev/null
ctl wait 20
[ "$(count_of refinedstorage:importer)" = 1 ] \
    || fail "the importer item was not picked up: $(ctl inventory | jq -c '.hotbar')"
ctl command "execute in minecraft:the_end run tp @s 43.5 64 17.5" >/dev/null
ctl wait 4
ctl place 43 64 16 --item refinedstorage:importer \
    || fail "cannot place the importer back at 43 64 16"
# `ctl place` right-clicks, and a click that lands on the machine's own face
# opens its menu: close it before the next blocking command (the engine refuses
# one while a screen is up).
ctl close >/dev/null 2>&1 || true
ctl wait 10
expect_state 43 64 16 'refinedstorage:importer'
machine_after=$(slots_indexed 43 64 16)
echo "  importer after the re-place: $machine_after"
if [ "$SELFTEST" = drops ]; then
    echo "(selftest: asserting the re-placed importer came back empty)"
    [ "$(echo "$machine_after" | jq 'length')" = 0 ] \
        || fail "the re-placed importer carried its slots back ($machine_after); the empty drop it used to have is gone"
fi
[ "$(jq -S -c . <<<"$machine_after")" = "$(jq -S -c . <<<"$machine_before")" ] \
    || fail "the re-placed importer's slots are $machine_after, expected $machine_before"
# A creative placement keeps the stack, so the payload must come off the item:
# the same item placed twice must not restore the six slots twice. Break the
# machine in survival (the payload lands on its item), pick that item up in
# creative, place it once (restores), remove the block, and place the *same*
# item again — the second machine must be empty.
ctl command "gamemode survival" >/dev/null
ctl give minecraft:diamond_pickaxe 1 >/dev/null
ctl wait 4
ctl select minecraft:diamond_pickaxe >/dev/null
ctl command "execute in minecraft:the_end run tp @s 43.5 64 17.5" >/dev/null
ctl wait 6
broken=$(ctl break 43 64 16) || fail "cannot break the re-placed importer: $broken"
[ "$(echo "$broken" | jq -r '.drops["refinedstorage:importer"] // 0')" -ge 1 ] \
    || fail "the re-placed importer did not drop itself: $(echo "$broken" | jq -c '.drops')"
ctl command "gamemode creative" >/dev/null
ctl command "execute in minecraft:the_end run tp @s 43.5 64 16.5" >/dev/null
ctl wait 20
[ "$(count_of refinedstorage:importer)" = 1 ] \
    || fail "the importer item was not picked up in creative: $(ctl inventory | jq -c '.hotbar')"
ctl command "execute in minecraft:the_end run tp @s 43.5 64 17.5" >/dev/null
ctl wait 4
ctl place 43 64 16 --item refinedstorage:importer \
    || fail "cannot place the importer in creative"
ctl close >/dev/null 2>&1 || true
ctl wait 10
creative_restored=$(slots_indexed 43 64 16)
echo "  importer after a creative re-place: $creative_restored"
[ "$(jq -S -c . <<<"$creative_restored")" = "$(jq -S -c . <<<"$machine_before")" ] \
    || fail "a creative placement did not restore the slots: $creative_restored"
# Remove the block (whatever it held is spilled, and cleared) and place the
# same, now stripped, item again: a payload still on it would restore twice.
ctl command "execute in minecraft:the_end run setblock 43 64 16 minecraft:air" >/dev/null
ctl wait 6
ctl command "execute in minecraft:the_end run kill @e[type=item]" >/dev/null 2>&1 || true
ctl place 43 64 16 --item refinedstorage:importer \
    || fail "cannot place the stripped importer again"
ctl close >/dev/null 2>&1 || true
ctl wait 10
stripped=$(slots_indexed 43 64 16)
echo "  importer after placing the stripped item again: $stripped"
[ "$(echo "$stripped" | jq 'length')" = 0 ] \
    || fail "the same item restored its slots a second time ($stripped): the payload was not taken off the stack"
ctl command "gamemode creative" >/dev/null
ctl command "clear @s" >/dev/null
ctl wait 4

echo "== a broken transmitter carries its four upgrade slots =="
# The same rule for the transmitter (slice 6's gap line): its own four slots
# travel on its item too. The transmitter itself is left standing on the floor
# so the drop and the re-place use the position the slice-3 phase broke from.
ctl command "execute in minecraft:the_end run kill @e[type=item]" >/dev/null 2>&1 || true
ctl command "execute in minecraft:the_end run setblock 55 64 5 refinedstorage:wireless_transmitter[facing=north]" >/dev/null
ctl wait 8
for _ in 1 2 3 4; do ctl container 55 64 5 refinedstorage:range_upgrade 1 >/dev/null; done
ctl wait 4
transmitter_before=$(slots_indexed 55 64 5)
echo "  transmitter before the break: $transmitter_before"
[ "$(echo "$transmitter_before" | jq 'length')" = 4 ] \
    || fail "the transmitter did not take four range upgrades: $transmitter_before"
ctl command "gamemode survival" >/dev/null
ctl give minecraft:diamond_pickaxe 1 >/dev/null
ctl wait 4
ctl select minecraft:diamond_pickaxe >/dev/null
ctl command "execute in minecraft:the_end run tp @s 55.5 64 7.5" >/dev/null
ctl wait 6
broken=$(ctl break 55 64 5) || fail "cannot break the transmitter: $broken"
echo "  transmitter drops: $(echo "$broken" | jq -c '.drops')"
[ "$(echo "$broken" | jq -r '.drops["refinedstorage:wireless_transmitter"] // 0')" -ge 1 ] \
    || fail "the transmitter did not drop itself: $(echo "$broken" | jq -c '.drops')"
[ "$(echo "$broken" | jq -r '.drops["refinedstorage:range_upgrade"] // 0')" = 0 ] \
    || fail "the transmitter's range upgrades spilled as well: $(echo "$broken" | jq -c '.drops')"
# Stand *on* the drop, as the importer phase does: a block's item spawns at the
# block's own centre, and one block away is outside the pickup reach.
ctl command "execute in minecraft:the_end run tp @s 55.5 64 5.5" >/dev/null
ctl wait 20
[ "$(count_of refinedstorage:wireless_transmitter)" = 1 ] \
    || fail "the transmitter item was not picked up: $(ctl inventory | jq -c '.hotbar')"
ctl command "execute in minecraft:the_end run tp @s 55.5 64 6.5" >/dev/null
ctl wait 4
ctl place 55 64 5 --item refinedstorage:wireless_transmitter \
    || fail "cannot place the transmitter back at 55 64 5"
ctl close >/dev/null 2>&1 || true
ctl wait 10
expect_state 55 64 5 'refinedstorage:wireless_transmitter'
transmitter_after=$(slots_indexed 55 64 5)
echo "  transmitter after the re-place: $transmitter_after"
[ "$(jq -S -c . <<<"$transmitter_after")" = "$(jq -S -c . <<<"$transmitter_before")" ] \
    || fail "the re-placed transmitter's slots are $transmitter_after, expected $transmitter_before"
ctl command "gamemode creative" >/dev/null
ctl command "clear @s" >/dev/null
ctl wait 4

# The advancement needs the grid and a transmitter in one `inventory_changed`
# criterion (upstream `advancement/wireless.json`): giving both completes it,
# while the transmitter-only phase above left it unobtained.
ctl command "execute in minecraft:the_end run tp @s 50.5 64 10.5"
ctl give refinedstorage:wireless_grid 1 >/dev/null
ctl give refinedstorage:wireless_transmitter 1 >/dev/null
charge_net
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

echo "== the other disk sizes: the capacity is the disk's own, and the creative disk has none =="
# Slice 11, from the port's own gap list ("the other disk sizes
# (4k/16k/64k/creative)", "Disk capacity is fixed at 1k and the disk is one
# item"). A second network at z=12 carries one drive, so a disk's own capacity is
# the only room the network has: the 4k refuses the 4,097th item while the 64k
# takes 65,536, and the creative disk — upstream's `ItemStorageVariant.CREATIVE`,
# capacity `null`, a plain storage with no `LimitedStorage` around it — keeps
# ingesting past the 64k ceiling. A disk is filled by writing the component it
# saves (`/data merge block`), which is the shape a disk carries between worlds.
net minecraft:the_end 40 64 12
ctl command "execute in minecraft:the_end run tp @s 41.5 64 13.5"
ctl command "execute in minecraft:the_end run setblock 40 64 12 refinedstorage:controller"
ctl command "execute in minecraft:the_end run setblock 41 64 12 refinedstorage:cable"
ctl command "execute in minecraft:the_end run setblock 42 64 12 refinedstorage:disk_drive"
ctl command "execute in minecraft:the_end run setblock 43 64 12 refinedstorage:grid"
ctl wait 12
charge minecraft:the_end 40 64 12
expect_info 43 64 12 "online (4 nodes, 1 controller, 1 drive, 1 grid); 0 disks, 0 items in 0 resources"

# Insert a 4k disk the way a player does: right-click it onto the drive.
ctl give refinedstorage:4k_storage_disk 1
ctl wait 4
ctl select refinedstorage:4k_storage_disk
ctl wait 4
charge_net
ctl interact 42 64 12
ctl wait 8
[ "$(count_of refinedstorage:4k_storage_disk)" = 0 ] \
    || fail "the 4k disk stayed in the hand: $(ctl inventory | jq -c '[.hotbar[] | select(.item == "refinedstorage:4k_storage_disk")]')"
slots 42 64 12 | jq -e '[.[] | select(.item == "refinedstorage:4k_storage_disk")] | length == 1' >/dev/null \
    || fail "the drive does not hold the 4k disk: $(slots 42 64 12)"
echo "  drive: $(slots 42 64 12)"
charge_net
expect_info 42 64 12 "1 disks, 0 items in 0 resources"
expect_drive 42 64 12 "1 disk, capacity slot 0: refinedstorage:4k_storage_disk 0/4096 items"

# Fill the disk by writing the component it saves. 5,000 saved items is 904 more
# than a 4k disk holds: the disk is cut to 4,096 (the 1k default cut it to 1,024
# before this slice, which is what the counterfactual asserts).
fill_drive 42 64 12 refinedstorage:4k_storage_disk 4096 5000
charge_net
expect_info 42 64 12 "1 disks, 4096 items in 1 resources"
expect_drive 42 64 12 "slot 0: refinedstorage:4k_storage_disk 4096/4096 items"

# The 4,097th item, deposited through the grid's own menu. The disk is full, so
# the deposit is refused: the network total does not move over a 20-tick window,
# and the item is not destroyed — the click leaves it in the grid's own slot,
# clickable back.
ctl command "execute in minecraft:the_end run tp @s 42.5 64 14.5"
ctl wait 4
charge_net
ctl interact 43 64 12
ctl wait 8
ctl give minecraft:dirt 1
ctl wait 4
dirt_slot=$(inv_slot minecraft:dirt)
[ -n "$dirt_slot" ] || fail "no dirt in the menu's player inventory: $(ctl screen | jq -c '[.slots[] | select(.item != null)]')"
charge_net
ctl click-slot "$dirt_slot"
ctl wait 2
charge_net
ctl click-slot 340
ctl wait 8
before=$(net_items 42 64 12)
charge_net
ctl wait 20
charge_net
after=$(net_items 42 64 12)
echo "  the 4k disk after a 1-item deposit: $before -> $after items"
if [ "$SELFTEST" = diskcap ]; then
    echo "(selftest: asserting the 4k disk accepted the 4,097th item)"
    [ "$after" = 4097 ] || fail "expected the 4k disk to have accepted the 4,097th item (net_items $after); a disk that enforces its capacity cannot fail this way"
else
    [ "$after" = 4096 ] || fail "a 4k disk holding 4,096 items took the 4,097th (net_items $before -> $after)"
fi
# The refused item comes back: click the slot the click put it in, put it in the
# inventory, and it is still the player's. A refusal that ate the item fails here.
charge_net
ctl click-slot 340
ctl wait 2
refused_back=$(empty_inv_slot)
[ -n "$refused_back" ] || fail "no empty inventory slot for the refused item"
charge_net
ctl click-slot "$refused_back"
ctl wait 4
[ "$(count_of minecraft:dirt)" = 1 ] \
    || fail "the refused dirt was destroyed instead of staying with the player: $(ctl inventory | jq -c '[.hotbar // [], .main // []]')"
ctl close
ctl wait 4
ctl command "clear @s" >/dev/null
ctl command "execute in minecraft:the_end run kill @e[type=item]" >/dev/null 2>&1 || true
ctl wait 4

# The same drive, a 16k disk: 16,384 items, and the readout says so.
fill_drive 42 64 12 refinedstorage:16k_storage_disk 16384 16384
charge_net
expect_info 42 64 12 "1 disks, 16384 items in 1 resources"
expect_drive 42 64 12 "slot 0: refinedstorage:16k_storage_disk 16384/16384 items"

# A 64k disk: 65,536 items, and the same deposit the creative disk takes below is
# refused here.
fill_drive 42 64 12 refinedstorage:64k_storage_disk 65536 65536
charge_net
expect_info 42 64 12 "1 disks, 65536 items in 1 resources"
expect_drive 42 64 12 "slot 0: refinedstorage:64k_storage_disk 65536/65536 items"
charge_net
ctl interact 43 64 12
ctl wait 8
ctl give minecraft:dirt 1
ctl wait 4
dirt_slot=$(inv_slot minecraft:dirt)
[ -n "$dirt_slot" ] || fail "no dirt in the grid's player inventory"
charge_net
ctl click-slot "$dirt_slot"
ctl wait 2
charge_net
ctl click-slot 340
ctl wait 8
charge_net
[ "$(net_items 42 64 12)" = 65536 ] \
    || fail "a full 64k disk took the 1-item deposit (net_items $(net_items 42 64 12))"
echo "  the 64k disk refused the deposit: $(net_items 42 64 12) items"
# Take the refused item back so the free view slot is free again for the next
# disk (the grid keeps the player's own stack in the slot it was put in).
charge_net
ctl click-slot 340
ctl wait 2
refused_back=$(empty_inv_slot)
[ -n "$refused_back" ] || fail "no empty inventory slot for the refused item"
charge_net
ctl click-slot "$refused_back"
ctl wait 4
[ "$(count_of minecraft:dirt)" = 1 ] \
    || fail "the refused dirt was destroyed instead of staying with the player"
ctl close
ctl wait 4
ctl command "clear @s" >/dev/null
ctl wait 2

# The creative disk: give it (it is creative-tab only upstream — no recipe), swap
# it into the drive, and make the *same* deposit the 64k disk refused. Upstream's
# creative variant carries no capacity and is not a `LimitedStorage`, so nothing
# refuses the insertion and the total passes the 64k ceiling.
ctl give refinedstorage:creative_storage_disk 1
ctl wait 4
[ "$(count_of refinedstorage:creative_storage_disk)" = 1 ] \
    || fail "the creative disk is not a giveable item: $(ctl inventory | jq -c '[.hotbar // [], .main // []]')"
fill_drive 42 64 12 refinedstorage:creative_storage_disk 0 65536
charge_net
expect_info 42 64 12 "1 disks, 65536 items in 1 resources"
expect_drive 42 64 12 "slot 0: refinedstorage:creative_storage_disk 65536/unlimited"
ctl give minecraft:dirt 1
ctl wait 4
charge_net
ctl interact 43 64 12
ctl wait 8
dirt_slot=$(inv_slot minecraft:dirt)
[ -n "$dirt_slot" ] || fail "no dirt in the grid's player inventory"
charge_net
ctl click-slot "$dirt_slot"
ctl wait 2
charge_net
ctl click-slot 340
ctl wait 8
charge_net
accepted=$(net_items 42 64 12)
echo "  the creative disk after the same 1-item deposit: $accepted items"
[ "$accepted" = 65537 ] \
    || fail "the creative disk did not take an item past the 64k ceiling (net_items $accepted)"
[ "$(count_of minecraft:dirt)" = 0 ] \
    || fail "the creative disk's accepted deposit is still in the inventory: $(ctl inventory | jq -c '[.hotbar // [], .main // []]')"
ctl close
ctl wait 4
ctl command "clear @s" >/dev/null
ctl wait 2

echo "== the in-place disk upgrade: a smaller disk and the bigger part make the bigger disk =="
# Slice 12, from the port's own gap list ("the in-place upgrade is not carried",
# upstream's `StorageContainerUpgradeRecipe`, a mod crafting kind). A 4k disk
# holding 4,096 cobblestone plus a 16k storage part become a 16k disk: the
# contents come across, the source disk's own 4k part comes back in the cell it
# left, and the drive then reads 4096/16384 — the *destination item's* room, not
# the source's.
net minecraft:the_end 40 64 12
# The drive starts empty, so the network total below is the upgraded disk alone.
ctl command "data merge block 42 64 12 {Items:[]}"
ctl wait 8
charge_net
expect_info 42 64 12 "0 disks, 0 items in 0 resources"
# The filled disk waits in a vanilla chest: a block's `Items` decode a component
# carrying stack, and the chest's menu is how a player takes it out.
ctl command "execute in minecraft:the_end run tp @s 46.5 64 15.5"
ctl command "execute in minecraft:the_end run setblock 46 64 14 minecraft:chest"
ctl wait 6
ctl command "data merge block 46 64 14 {Items:[{Slot:0b,id:\"refinedstorage:4k_storage_disk\",count:1,components:{\"refinedstorage:storage\":{capacity:4096,stacks:[{id:\"minecraft:cobblestone\",count:4096}]}}}]}"
ctl wait 6
charge_net
open_menu 46 64 14 27
disk_slot=$(ctl screen | jq -r '[.slots[] | select(.slot >= 300 and .item == "refinedstorage:4k_storage_disk")][0].slot // empty')
[ -n "$disk_slot" ] || fail "the chest does not hold the filled 4k disk: $(ctl screen | jq -c '[.slots[] | select(.item != null)]')"
ctl click-slot "$disk_slot"
ctl wait 2
free_slot=$(empty_inv_slot)
[ -n "$free_slot" ] || fail "no empty inventory slot for the filled disk"
ctl click-slot "$free_slot"
ctl wait 4
held=$(ctl screen | jq -c '[.slots[] | select(.slot < 36 and .item == "refinedstorage:4k_storage_disk")]')
echo "  the filled 4k disk taken out of the chest: $held"
[ "$(echo "$held" | jq 'length')" = 1 ] \
    || fail "the filled 4k disk is not in the inventory: $held"
ctl close
ctl wait 4
# The crafting table: the 4k disk in one cell, a 16k storage part in another.
ctl command "execute in minecraft:the_end run setblock 44 64 14 minecraft:crafting_table"
ctl wait 6
# A crafting table's grid is 100-108 with the result at 200 (not a container's
# 300+), so it opens with recipe_check's own ask: wait for the result slot.
for _ in 1 2 3; do
    ctl interact 44 64 14 >/dev/null
    ctl wait 8
    if ctl screen 2>/dev/null | jq -e '[.slots[] | select(.slot == 200)] | length > 0' >/dev/null 2>&1; then
        break
    fi
    ctl close >/dev/null 2>&1 || true
    ctl wait 4
done
ctl screen | jq -e '[.slots[] | select(.slot == 200)] | length > 0' >/dev/null \
    || fail "the crafting table at 44,64,14 did not open (no result slot): $(ctl screen | jq -c '{screen,slots:((.slots // [])|length)}')"
disk_slot=$(inv_slot refinedstorage:4k_storage_disk)
[ -n "$disk_slot" ] || fail "the 4k disk is not in the crafting table's menu: $(ctl screen | jq -c '[.slots[] | select(.item != null)]')"
ctl click-slot "$disk_slot"
ctl wait 2
ctl click-slot 100
ctl wait 2
ctl give refinedstorage:16k_storage_part 1
ctl wait 4
part_slot=$(inv_slot refinedstorage:16k_storage_part)
[ -n "$part_slot" ] || fail "no 16k storage part in the menu's player inventory"
ctl click-slot "$part_slot"
ctl wait 2
ctl click-slot 101
ctl wait 4
made=$(ctl screen | jq -c '.slots[] | select(.slot == 200 and .item != null)')
echo "  the upgrade recipe's result: $made"
case "$made" in
    *'"item":"refinedstorage:16k_storage_disk"'*) ;;
    *) fail "a 4k disk and a 16k storage part did not make a 16k disk: $made" ;;
esac
case "$made" in
    *capacity:16384*) ;;
    *) fail "the crafted 16k disk does not record its own 16,384-item capacity: $made" ;;
esac
case "$made" in
    *cobblestone*) ;;
    *) fail "the crafted 16k disk did not receive the 4k disk's contents: $made" ;;
esac
case "$made" in
    *count:4096*) ;;
    *) fail "the crafted 16k disk does not hold the 4,096 cobblestone: $made" ;;
esac
# The take: shift-click the result. The source disk and the part are consumed and
# the source disk's *own* 4k part comes back into the cell the disk left.
ctl click-slot 200 shift
ctl wait 4
grid_after=$(ctl screen | jq -c '[.slots[] | select(.slot >= 100 and .slot <= 108 and .item != null)]')
echo "  the crafting grid after the take: $grid_after"
if [ "$SELFTEST" = upgrade ]; then
    echo "(selftest: asserting the source disk's 4k part was not returned)"
    case "$grid_after" in
        *4k_storage_part*) fail "expected the source 4k part not to be returned; this build returns it" ;;
    esac
else
    case "$grid_after" in
        *'"item":"refinedstorage:4k_storage_part"'*) ;;
        *) fail "the source disk's own 4k part was not returned into its cell: $grid_after" ;;
    esac
    case "$grid_after" in
        *4k_storage_disk*|*16k_storage_part*) fail "the upgrade left an ingredient in the grid: $grid_after" ;;
    esac
fi
[ "$(ctl screen | jq '[.slots[] | select(.slot < 36 and .item == "refinedstorage:4k_storage_disk")] | length')" = 0 ] \
    || fail "the 4k disk survived the upgrade: $(ctl screen | jq -c '[.slots[] | select(.item != null)]')"
upgraded=$(ctl screen | jq -c '[.slots[] | select(.slot < 36 and .item == "refinedstorage:16k_storage_disk")]')
echo "  the crafted 16k disk in the inventory: $upgraded"
[ "$(echo "$upgraded" | jq 'length')" = 1 ] \
    || fail "the crafted 16k disk is not in the inventory: $upgraded"
ctl close
ctl wait 4
# Into the drive through its own menu: the drive reads the disk's own capacity.
charge_net
open_menu 42 64 12 9
disk_slot=$(inv_slot refinedstorage:16k_storage_disk)
[ -n "$disk_slot" ] || fail "the crafted 16k disk is not in the drive's menu: $(ctl screen | jq -c '[.slots[] | select(.item != null)]')"
ctl click-slot "$disk_slot"
ctl wait 2
ctl click-slot 300
ctl wait 8
ctl close
ctl wait 4
charge_net
slots 42 64 12 | jq -e '[.[] | select(.item == "refinedstorage:16k_storage_disk")] | length == 1' >/dev/null \
    || fail "the drive does not hold the crafted 16k disk: $(slots 42 64 12)"
expect_info 42 64 12 "1 disks, 4096 items in 1 resources"
expect_drive 42 64 12 "slot 0: refinedstorage:16k_storage_disk 4096/16384 items"
ctl command "clear @s" >/dev/null
ctl command "execute in minecraft:the_end run kill @e[type=item]" >/dev/null 2>&1 || true
ctl wait 4

# A 4k disk keeps its 4,096 items across a save and a relaunch, at the capacity
# its item names. It joins the slice-1 drive, so the relaunch below reads two
# disks of two sizes on one network (the aggregation the network already did).
net minecraft:the_end 40 64 0
ctl command "data modify block 42 64 0 Items append value {Slot:1b,id:\"refinedstorage:4k_storage_disk\",count:1,components:{\"refinedstorage:storage\":{capacity:4096,stacks:[{id:\"minecraft:cobblestone\",count:4096}]}}}"
ctl wait 8
charge_net
expect_info 43 64 0 "2 disks, 4099 items in 2 resources"
expect_drive 42 64 0 "slot 1: refinedstorage:4k_storage_disk 4096/4096 items"

echo "== the disk keeps its contents across a relaunch =="
net minecraft:the_end 40 64 0
ctl command "execute in minecraft:the_end run tp @s 42.5 64 2.5"
ctl save
ctl wait 10
ctl quit
sleep 3
launch
charge_net
ctl wait 20
charge minecraft:the_end 40 64 0
charge_net
expect_info 43 64 0 "online (4 nodes, 1 controller, 1 drive, 1 grid); 2 disks, 4099 items in 2 resources"
charge_net
expect_drive 42 64 0 "slot 1: refinedstorage:4k_storage_disk 4096/4096 items"
charge_net
ctl interact 43 64 0
ctl wait 10
charge_net
expect_slots '[{"slot":300,"item":"minecraft:gold_ingot","count":3},{"slot":301,"item":"minecraft:cobblestone","count":64}]'
ctl close
ctl wait 4

echo "PASS: the wireless transmitter joined its network through the facing side only (offline facing away, active facing the cable), /refinedstorage info named it and its range 16 blocks, a use 2.5 blocks away and one exactly 16 blocks away were accepted, 17 blocks away was refused naming the distance, an offline transmitter refused and a position without one answered so, a use from the End against the overworld transmitter was refused as a different dimension while the same command under execute in was accepted, the item placed against the clicked face and the block dropped itself, every carried recipe crafts in game with its declared output and count (38 now, each with its wrong-ingredient control), the machine casing places, reads back and drops itself, the wireless grid is unbound until it is used on a transmitter and then names it (dimension and position in its own component), a use inside the range opens the network's 9x6 view with 54 slots, a shift-click in the player inventory deposits 5 diamonds on the network and a shift-click on the view withdraws them back, 17 blocks out is refused naming the distance while 16 blocks out opens the view, the same item and binding is refused as a different dimension from the overworld and accepted in the End, the refinedstorage:wireless advancement is not granted by a transmitter alone and is granted by the grid and a transmitter together, two networks in one dimension each listed their own 3 gold while the other network held 3 too, two grids on one network never offered the same items twice, the cable follows its neighbours and the grid is active with a controller, the disk went into the drive and the grid listed what was deposited through its menu, the search filtered it, a shift-click withdrew the diamonds, breaking the controller took the network offline and putting it back restored the disk's gold, a network without a drive listed nothing, a stick did not enter the drive, a vanilla barrel is not a network block, a range upgrade in the transmitter's own upgrade slot raised its range to 24 blocks so the same 17.0-block use was accepted and the wireless grid opened its view naming 24 blocks, the transmitter's four-slot menu showed the upgrade and refused a diamond, a creative range upgrade gave unlimited range (70.5 blocks out accepted, another dimension still refused), the importer turned on its network moved 3 items in 20 ticks without an upgrade and emptied its source in one pass with a stack upgrade (64 in 20 ticks), four speed upgrades made the same 20 ticks move a stack and /refinedstorage info named the upgrade counts and the work interval (9 ticks down to 1), its filter blocked the resource it names while an empty filter imported everything, the exporter moved exactly the 4 gold its filter names into the chest it faces and an exporter with no filter moved nothing, a machine menu showed its 13 slots and its upgrade row refused a diamond, both machine items were given and read back, both machine blocks dropped themselves, the wireless grid's own 1,000 FE buffer fell by exactly 5 on the open, 5 on a deposit and 5 on a withdrawal with no drift over ticks, and at 0 item FE the same deposit was refused for the item's energy while the network stayed online and still printed its own energy line, a survival break of the importer carried its four speed upgrades, its stack upgrade and its dirt filter entry on the item with none of them spilled beside it and the re-placed machine listed every slot where it was, a broken transmitter's four range upgrades travelled the same way, a 4k disk cut a saved 5,000-item component to its own 4,096 and refused the 4,097th item deposited through the grid while its total stayed 4,096 over a 20-tick window, a 16k disk held 16,384 and a 64k disk 65,536, the creative disk took the same 1-item deposit the full 64k disk refused and kept ingesting past 65,536 with its readout saying unlimited, a 4k disk holding 4,096 cobblestone and a 16k storage part made a 16k disk in a crafting table with the 4k disk's contents and the destination's 16,384-item room, the 4k disk was consumed and its own 4k part came back in the cell it left, the crafted disk read 4096/16384 items in the drive and the network totalled 4,096, and the 1k disk's 3 gold and a 4k disk's 4,096 items both kept their contents across a relaunch"
# The cleanup trap prints EXIT:<status> as the last line.
