# Validation

The port's own record is [notes.md](notes.md): the behaviour, the helper→hook
table of the SDK migration, the deviations, the gap list, and the verification
tables (the retired `.hnmod` ladder, the SDK-format one, slice 2's red→green
recipe proof, slice 3's wireless-transmitter red→green and in-game leg, slice
4's wireless-grid one, slice 5's network-keyed promise table, slice 6's upgrade
items and the raised range, and slice 7's importer/exporter pair with the speed and stack
upgrades).

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
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`. Delivered package (0.2.7):
`refinedstorage.hmod`, **179,081 B**, SHA-256
`3223f32ae80773769dd4a5b3ac1dc350477d7608778131223a6ba43e93bb6bb9`, 235 entries, no packed
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
python3 ../tools/check-e2e-logs.py refinedstorage-e2e --strict
```

`tools/recipe_check.py` is the recipe family's driver: it reads this package's own
`data/refinedstorage/recipe/*.json` (29 recipes, including slice 3's `wireless_transmitter`,
slice 4's `wireless_grid`, slice 6's `upgrade` and `range_upgrade`, and slice 7's `importer`,
`exporter`, `speed_upgrade` and `stack_upgrade`) and crafts every one of them in a crafting table or a furnace,
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
