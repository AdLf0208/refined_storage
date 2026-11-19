# Working on this native Horizon mod (SDK format)

This port is **one flat `.hmod`** in the SDK layout (mod API 0.2, 2026-11-18):
`horizon-mod.toml` is the only manifest, `src/` is the code the launcher compiles
against the pinned game build, `assets/` and `data/` are the carried content.
`mixins/`, `horizon-mixin.toml`, `horizon-native.json`, per-hook helper files,
`compatibility.json` and `rust-toolchain.toml` were retired with the `.hnmod`
container — do not recreate them.

- `src/runtime.rs` — the pure model; **keep it byte-identical** to what the
  migration carried and keep its tests green (`cargo test`).
- `src/support.rs` — the game glue (block entities, the disk cache, the menus,
  the commands). No business rules here: they belong in `runtime.rs` with a test.
- `src/lib.rs` — the one `#[mod_api::horizon_mod] impl Hooks`.

Validation (see `VALIDATION.md`; every build/test step goes through the
campaign's `ports/tools/build-slot.sh`, never a bare `cargo` outside it):

```sh
rustfmt --edition 2024 --check src/lib.rs src/support.rs src/runtime.rs
cargo test
cargo clippy --all-targets -- -D warnings
cargo horizon-mod check                       # the SDK gate (format + manifest + code)
cargo horizon-mod build                       # -> target/horizon-mod/refinedstorage.hmod
cargo horizon-mod check refinedstorage.hmod
cargo horizon-mod fits refinedstorage.hmod
```

Do **not** run `cargo fmt --all`: it reaches into the vendored `.horizon/sdk`
and rewrites the SDK's own sources. The delivered `refinedstorage.hmod` lives in
the port root, never in `target/`.

Keep the `.horizon/sdk` checkout out of the package (it is fetched per clone) and
keep the two review repairs in `runtime.rs` (post-state grid rebuild; per-grid
reservations through `view_with`) — `notes.md` §Migration records them.
