# Refined Storage for Horizon (slice 1)

A clean-room Rust port of **Refined Storage 2.0.9**
(`refinedmods/refinedstorage2`, MIT), scoped to the **item network core**:

- **Controller**: a network is online when its connected component holds one;
- **Cable**: carries the network between blocks, with its six connection states;
- **Disk Drive**: nine slots of storage disks, opened with a right click;
- **1k Storage Disk**: 1,024 items, kept in the disk item's own component;
- **Grid**: a 9x6 menu that searches, withdraws and deposits across the network.

The rest of the mod (crafters, exporters/importers, fluids, wireless, security,
autocrafting, the crafting chain) is the planned gap list in
[notes.md](notes.md).

One flat package, the SDK layout: `horizon-mod.toml` is the only manifest,
`src/` is the code (compiled by the launcher against the pinned game build),
`assets/` and `data/` are the five blocks, the disk item and their models,
textures, loot tables and `horizon/` declarations.

- `src/runtime.rs` — the pure, unit-tested model (network, disks, grid view).
- `src/support.rs` — the game glue (block entities, the disk cache, the menus, the commands).
- `FINDINGS.md` — the upstream behaviour analysis.
- `notes.md` — scope, the helper→hook table, deviations, gaps, verification.
- `tools/e2e.sh` — the in-game scenario (needs a launcher-built game).

Local-only until the maintainer approves publishing (ARCHITECTURE.md §8).

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo horizon-mod check
cargo horizon-mod check refinedstorage.hmod
cargo horizon-mod fits refinedstorage.hmod
```
