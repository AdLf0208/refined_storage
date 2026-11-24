# Refined Storage for Horizon (slice 14)

A clean-room Rust port of **Refined Storage 2.0.9**
(`refinedmods/refinedstorage2`, MIT), scoped to the **item network core**:

- **Controller**: a network is online when its connected component holds one, with
  a 1,000 FE buffer, four energy levels, and a per-block charge (a network that
  cannot pay does no work and says why);
- **Cable**: carries the network between blocks, with its six connection states;
- **Disk Drive**: nine slots of storage disks, opened with a right click;
- **Storage Disks**: 1k (1,024 items), 4k, 16k, 64k and the creative disk, whose
  capacity travels in the disk item's own component beside its contents (the
  creative one has no ceiling and never refuses an item);
- **Grid**: a 9x6 menu that searches, withdraws and deposits across the network;
- **Wireless Transmitter**: joins a network through its facing side and answers
  whether a player may use that network — same dimension, online, within 16 blocks;
- **Wireless Grid**: binds a transmitter's dimension and position into the item, then
  opens that network's 9x6 view from anywhere inside the transmitter's range, paying
  from its own 1,000 FE buffer;
- **Importer / Exporter**: the two transfer machines, each with nine filter entries
  and four upgrade slots, working on the neighbour its facing names;
- **External Storage**: exposes the inventory it faces to the network — a grid lists
  the chest's stock alongside the disks, deposits through the grid land in that chest,
  and breaking the block or removing the chest drops only that stock from the view;
- **Upgrades**: range (+8 blocks each, up to four) and creative range for the
  transmitter; speed (two ticks off the work interval, up to four) and stack
  (64 items per pass) for the machines;
- **The in-place disk upgrade**: a smaller disk and the next tier's storage part make
  that bigger disk in one crafting-table step — the contents come across with the
  destination's room, and the source disk's own part is returned.

The crafting chain that reaches them (quartz enriched iron and copper, silicon,
processors, the machine casing, cores, the storage parts and housing) is carried
too, with a recipe for every block and item, the item tags, and the
`refinedstorage:root` / `refinedstorage:wireless` advancements. A broken importer,
exporter or transmitter carries its own slots on its dropped item and restores them
when placed again — one unit's payload per placed machine, so placing one of a
stack of two leaves the other unit's payload intact, and a break that hands no
item over (creative, a cancelled break) leaves the container to the engine's own
spill instead of emptying it. The two energy commands (`/refinedstorage energy`,
`/refinedstorage charge`) are operator commands, and a charge names the item it
fills: the wireless grid, or nothing.

The rest of the mod (crafters, interfaces, relays, fluids, the portable grid,
security, autocrafting, the coloured variants and the upgrades whose blocks are not
ported) is the planned gap list in [notes.md](notes.md).

One flat package, the SDK layout: `horizon-mod.toml` is the only manifest,
`src/` is the code (compiled by the launcher against the pinned game build),
`assets/` and `data/` are the blocks, the items, the recipes and the tags with their
models, textures, loot tables and `horizon/` declarations.

- `src/runtime.rs` — the pure, unit-tested model (network, disks, the upgrade rule, the grid view).
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
