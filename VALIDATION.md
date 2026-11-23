# Validation

The port's own record is [notes.md](notes.md): the behaviour, the helper→hook
table of the SDK migration, the deviations, the gap list, and the verification
tables (the retired `.hnmod` ladder, the SDK-format one, slice 2's red→green
recipe proof, slice 3's wireless-transmitter red→green and in-game leg, slice
4's wireless-grid one, slice 5's network-keyed promise table, slice 6's upgrade
items and the raised range, slice 7's importer/exporter pair with the speed and stack
upgrades, slice 9's wireless-grid item buffer, slice 10's block drops that carry
the broken machine's own slots, slice 11's five disk sizes, slice 12's in-place disk upgrade, and
slice 13's review-round-two repairs: the placing hand and the payload on unplaced units, the
insertion and its consumption in one transaction, the break's completion gate, and the two
operator-only energy writes).

Quick reproduction (a build slot is required; see the campaign's AGENTS.md):

```sh
cd ports/refinedstorage
rustfmt --edition 2024 --check src/lib.rs src/support.rs src/runtime.rs
cargo test
cargo clippy --all-targets -- -D warnings

# the SDK gate (format + manifest + code, at the pinned build)
../tools/build-slot.sh run --job sdk-refinedstorage --port ports/refinedstorage \
    --step check -- bash -c 'cd ports/refinedstorage && cargo horizon-mod check'

# the delivered package
rm -f Cargo.lock && cargo horizon-mod build   # -> target/horizon-mod/refinedstorage.hmod
cp target/horizon-mod/refinedstorage.hmod ./refinedstorage.hmod
cargo horizon-mod check refinedstorage.hmod
cargo horizon-mod fits refinedstorage.hmod --build 0a875da   # "made for" (the pin: tautology)
cargo horizon-mod fits refinedstorage.hmod --build 0b14f8a   # "fits" (a different build: the proof)
# Both fits runs print the file's sha256 first (sdk-fits-0.2.6.log). The GitHub releases
# list answers 403 (unauthenticated rate limit) while other legs on the box fetch too;
# retry the second run after the limit resets.
```

The two `check`/`build`/`fits` steps need `BUILD_LOCAL=1` on the slot runner:
the second build machine has no `cargo-horizon-mod`.

Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`. Delivered package (0.2.12):
`refinedstorage.hmod`, **221,841 B**, SHA-256
`92858557af7dcf5bd4c88ce4f43a62fc666605c099bc373cfa8a4baa390cc21f`, 260 entries, no packed
`Cargo.lock`; a copy of the delivered bytes is at
`ports/_shared/evidence/refinedstorage/refinedstorage.hmod`, which is the file both `fits`
runs used.

The in-game leg (`tools/e2e.sh`) needs a game the **launcher** built with this
package:

```sh
ports/tools/launcher-game.sh build refinedstorage --rebuild   # "Compiling refinedstorage (1 of 1)"
eval "$(ports/tools/launcher-game.sh env refinedstorage)"
ports/tools/build-slot.sh run --job W1-refinedstorage --port ports/refinedstorage \
    --step e2e -- bash ports/refinedstorage/tools/e2e.sh          # PASS + EXIT:0
GAME=$CONTROL_GAME … tools/e2e.sh                                 # the pre-package control: EXIT:1
SELFTEST=1 … tools/e2e.sh                                         # flipped command range: EXIT:1
SELFTEST=grid … tools/e2e.sh                                      # flipped grid view: EXIT:1
SELFTEST=offline … tools/e2e.sh                                   # flipped offline-grid assertion: EXIT:1
SELFTEST=networks … tools/e2e.sh                                  # flipped two-network assertion: EXIT:1
SELFTEST=range … tools/e2e.sh                                     # flipped raised-range assertion: EXIT:1
SELFTEST=creative … tools/e2e.sh                                  # flipped creative-range assertion: EXIT:1
SELFTEST=speed … tools/e2e.sh                                     # flipped importer interval: EXIT:1
SELFTEST=stack … tools/e2e.sh                                     # flipped importer quota: EXIT:1
SELFTEST=itemenergy … tools/e2e.sh                                # flipped item-energy gate: EXIT:1
SELFTEST=drops … tools/e2e.sh                                     # flipped block-drop carry: EXIT:1
SELFTEST=diskcap … tools/e2e.sh                                   # flipped 4k capacity (accepted the 4,097th): EXIT:1
SELFTEST=upgrade … tools/e2e.sh                                   # flipped: the source part was NOT returned: EXIT:1
SELFTEST=payload … tools/e2e.sh                                   # flipped: the unplaced unit lost its payload: EXIT:1
SELFTEST=break … tools/e2e.sh                                     # flipped: a creative break spilled nothing: EXIT:1
python3 ../tools/check-e2e-logs.py refinedstorage-e2e --strict
```

`tools/recipe_check.py` is the recipe family's driver: it reads this package's own
`data/refinedstorage/recipe/*.json` (38 recipes, including slice 3's `wireless_transmitter`,
slice 4's `wireless_grid`, slice 6's `upgrade` and `range_upgrade`, slice 7's `importer`,
`exporter`, `speed_upgrade` and `stack_upgrade`, and slice 11's `4k/16k/64k_storage_part` and
`4k/16k/64k_storage_disk`, and slice 12's three `{4k,16k,64k}_storage_disk_upgrade` files, the
package's own crafting kind) and crafts every one of them in a crafting table or a furnace,
asserting the output id and count, that the ingredients were consumed, and that a
wrong-ingredient control yields nothing — including one control per shaped tag ingredient
(slice 4 fixed the driver's tag lookup). Recorded in `notes.md` §"Slice 2", §"Slice 3" and
§"Slice 4" (the latter with the red run against the previous package).

Slice 3's in-game assertions (each with its differential) are listed in `notes.md` §"Slice 3":
the transmitter's facing-only connection, `/refinedstorage info` naming it and its range, the
range boundary at 16.0 vs 17.0, an offline transmitter, a position with no transmitter, the
cross-dimension use refused while the same command under `execute in` is accepted, the item
placing against the clicked face, the block dropping itself, and the `refinedstorage:wireless`
advancement (now granted by the grid **and** a transmitter, slice 4).

Slice 4's assertions are listed in `notes.md` §"Slice 4": the binding read back from the item's
own component, the unbound refusal, a use in range opening the 9×6 view with the network's
stock, a shift-click deposit and a shift-click withdrawal through the menu, 17 blocks out
refused naming the distance while 16 opens, the same item and binding refused as a different
dimension from the overworld and accepted in the End, and the advancement's two halves.

Slice 5's assertion is `notes.md` §"Slice 5": two networks in one dimension, each holding 3 gold,
each grid listing its own 3 (`SELFTEST=networks` asserts the old, wrong answer and fails, so the
assertion is fail-capable). Its first run was against the previous package (0.2.4), where one grid
answered `[]` while its disk held 3.

Slice 6's assertions are `notes.md` §"Slice 6": a range upgrade inserted into the transmitter's own
four upgrade slots raises its range to 24 blocks (the same 17.0-block use the slice-4 phase refuses
is accepted, and the wireless grid opens its view naming 24 blocks), the transmitter's menu shows the
four slots and refuses a diamond, and a creative range upgrade gives unlimited range (70.5 blocks out
accepted, another dimension still refused). `SELFTEST=range` and `SELFTEST=creative` invert the two
new assertions and exit 1.

Slice 7's assertions are `notes.md` §"Slice 7": the importer moves 3 items in 20 ticks with no
upgrade, empties its source in one pass (64) with a stack upgrade, and moves ~20 in 20 ticks with
four speed upgrades while `/refinedstorage info` names the counts and the interval (9 ticks down to
1); its filter blocks the one resource it names while an empty filter imports everything; the
exporter moves exactly the four gold its filter names into the chest it faces and a filterless
exporter moves nothing; a machine menu shows 13 slots and refuses a diamond in its upgrade row; and
the exporter block drops itself. `SELFTEST=speed` and `SELFTEST=stack` invert the two new
differentials and exit 1.

Slice 8's assertions are `notes.md` §"Slice 8": an isolated controller + cable + transmitter at 500
FE falls by exactly 16 × the server ticks the `info` line's counter shows; a lone controller reads
`off`/`nearly_off`/`nearly_on`/`on` at 0/50/350/500 FE (upstream's 1 %, 30 % and 40 % thresholds);
at 0 FE the transmitter is inactive and the same use is refused `not enough energy: 0 of 16 FE`
both inside and 17 blocks outside the range, while with energy the same 17 blocks name the distance
and 2.5 blocks are accepted; a grid at 0 FE is `active=false` and lists nothing while its disk keeps
the 3 gold (charged, it lists them); and an importer at 0 FE moves nothing in a 20-tick window that
moves 3 items once charged. `SELFTEST=energy` inverts the no-energy refusal and exits 1; the
slice-5 promise rule's `SELFTEST=networks` is re-run and still fails on purpose.

Slice 9's assertions are `notes.md` §"Slice 9": the grid item's own 1,000 FE buffer read twice 17
ticks apart does not move while the network beside it drains 30 FE a tick; the open, a deposit and a
withdrawal each cost exactly 5; at 0 item FE the same deposit is refused `not enough item energy: 0 of
5 FE` while the network stays online with its own energy line, and a use at 0 opens nothing while the
same use charged opens for 5. `SELFTEST=itemenergy` inverts the 0-FE deposit and exits 1. The
slice-8 rate window starts full (1,000 FE) after it ran dry under load and produced a false red.

Slice 10's assertions are `notes.md` §"Slice 10": an importer filled with four speed upgrades, one
stack upgrade and a dirt filter entry is broken in survival, and its drop lists only the block item —
none of the six spills beside it; the item is picked up and placed back at the same position, and
`ctl container` lists all six at their own indices; a second break with the item picked up **in
creative** restores once, is stripped, and placing the same item again yields an empty machine; a
transmitter's four range upgrades travel the same way and none spills. `SELFTEST=drops` asserts the
old, empty re-placed machine and exits 1. The control (the same Nightly with no package) fails at the
first phase, so the leg is not passing on the engine alone.

Slice 11's assertions are `notes.md` §"Slice 11": a 4k disk is written with a 5,000-item component
and reads back 4,096 (the pre-slice build cut every disk to 1,024), the 4,097th item deposited
through the grid's menu is refused with `net_items` flat over a 20-tick window, the refused item comes
back to the player, a 16k disk holds 16,384 and a 64k disk 65,536 (each named by the drive's own
info line), a full 64k disk refuses the same 1-item deposit the creative disk accepts (65,536 →
65,537, readout `65536/unlimited`), and a 4k disk keeps its 4,096 items across `save` + relaunch
beside the 1k disk's 3 gold (`2 disks, 4099 items in 2 resources`). `SELFTEST=diskcap` inverts the
capacity refusal and exits 1. The four earlier selftests (`networks`, `energy`, `itemenergy`,
`drops`) were re-run on this package and each still exits 1 on its own assertion.

Slice 12's assertions are `notes.md` §"Slice 12": a 4k disk holding 4,096 cobblestone (taken out of a
chest, its own component read back) plus a `16k_storage_part` make a `16k_storage_disk` in a crafting
table — the result's component reads `capacity:16384` with the source's `count:4096` cobblestone — and
after the shift-click the source disk is gone while **its own `4k_storage_part`** is back in the cell
the disk left. The crafted disk inserted into the drive's own menu slot reads `4096/16384 items` and
the network totals 4,096. `SELFTEST=upgrade` asserts the source part was not returned and exits 1.
The recipe driver crafts all three `storage_disk_upgrade` files (38 of 38 carried recipes, 36
wrong-ingredient controls — including, per upgrade file, its own tier's disk and the source tier's own
part, each yielding nothing). The five earlier selftests (`networks`, `energy`, `itemenergy`, `drops`,
`diskcap`) were re-run on this package and each still exits 1 on its own assertion; the 62-log gate is
green.
