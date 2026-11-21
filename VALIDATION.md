# Validation

The port's own record is [notes.md](notes.md): the behaviour, the helper→hook
table of the SDK migration, the deviations, the gap list, and the verification
tables (the retired `.hnmod` ladder, the SDK-format one, slice 2's red→green
recipe proof, and slice 3's wireless-transmitter red→green and in-game leg).

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
```

Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`. Delivered package (0.2.3):
`refinedstorage.hmod`, **129,923 B**, SHA-256
`a36d3a5d7378cfc3283558fb382daa59e9337c2f939d73be363000e3d8ac02da`, 182 entries, no packed
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
SELFTEST=1 … tools/e2e.sh                                         # flipped range assertion: EXIT:1
SELFTEST=offline … tools/e2e.sh                                   # flipped offline-grid assertion: EXIT:1
python3 ../tools/check-e2e-logs.py refinedstorage-e2e --strict
```

`tools/recipe_check.py` is the recipe family's driver: it reads this package's own
`data/refinedstorage/recipe/*.json` (including slice 3's `wireless_transmitter`) and crafts
every one of them in a crafting table or a furnace, asserting the output id and count, that the
ingredients were consumed, and that a wrong-ingredient control yields nothing. Recorded in
`notes.md` §"Slice 2" (red run against the previous package) and §"Slice 3".

Slice 3's in-game assertions (each with its differential) are listed in `notes.md` §"Slice 3":
the transmitter's facing-only connection, `/refinedstorage info` naming it and its range, the
range boundary at 16.0 vs 17.0, an offline transmitter, a position with no transmitter, the
cross-dimension use refused while the same command under `execute in` is accepted, the item
placing against the clicked face, the block dropping itself, and the `refinedstorage:wireless`
advancement absent before the item is obtained and `done=true` after it.
