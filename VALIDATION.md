# Validation

The port's own record is [notes.md](notes.md): the behaviour, the helper→hook
table of the SDK migration, the deviations, the gap list, and the verification
tables (the retired `.hnmod` ladder and the SDK-format one).

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
cargo horizon-mod build            # -> target/horizon-mod/refinedstorage.hmod
cp target/horizon-mod/refinedstorage.hmod ./refinedstorage.hmod
cargo horizon-mod check refinedstorage.hmod
cargo horizon-mod fits refinedstorage.hmod
```

The in-game leg (`tools/e2e.sh`) needs a game the **launcher** built with this
package (`horizon-launcher native-build ports/refinedstorage --instance <instance>
--trust-native-code`); this machine has no `horizon-launcher`, so that leg is the
next machine's step and is recorded as a gap in `notes.md`.
