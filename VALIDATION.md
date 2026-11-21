# Validation

The port's own record is [notes.md](notes.md): the behaviour, the helper→hook
table of the SDK migration, the deviations, the gap list, and the verification
tables (the retired `.hnmod` ladder, the SDK-format one, and slice 2's
red→green recipe proof).

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
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`. Delivered package (0.2.2):
`refinedstorage.hmod`, **116,902 B**, SHA-256
`3c26f5a4986847a1090f9582a65cd0341ee6e60efaad30687e86c678d99d1ad4`, 165 entries, no packed
`Cargo.lock`.

The in-game leg (`tools/e2e.sh`) needs a game the **launcher** built with this
package:

```sh
ports/tools/launcher-game.sh build refinedstorage --rebuild
eval "$(ports/tools/launcher-game.sh env refinedstorage)"
ports/tools/build-slot.sh run --job W1-refinedstorage --port ports/refinedstorage \
    --step e2e -- bash ports/refinedstorage/tools/e2e.sh          # PASS + EXIT:0
GAME=$CONTROL_GAME … tools/e2e.sh                                 # the pre-package control: EXIT:1
SELFTEST=1 … tools/e2e.sh                                         # flipped assertion: EXIT:1
python3 ../tools/check-e2e-logs.py refinedstorage-e2e --strict
```

`tools/recipe_check.py` is the recipe family's driver: it reads this package's own
`data/refinedstorage/recipe/*.json` and crafts every one of them in a crafting table or a
furnace, asserting the output id and count, that the ingredients were consumed, and that a
wrong-ingredient control yields nothing. Recorded in `notes.md` §"Slice 2", with the red run
against the previous package.
