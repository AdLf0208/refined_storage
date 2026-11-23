# Refined Storage — port notes

Ported from **Refined Storage 2.0.9** (`refinedmods/refinedstorage2`, **MIT**),
tag `v2.0.9`, commit `0dec63affe8147702b020843c22748483ef18c01` (2026-06-07).
Minecraft Java 1.21.1 NeoForge; the ATM10 build 8.4 jar is
`refinedstorage-neoforge-2.0.9.jar` (SHA-1 `59d8e734…`, SHA-256 `e9c758dd…`;
provenance in `ports/_shared/carry-work/refinedstorage/SOURCE.md`).

**LOCAL-ONLY** until the maintainer approves publishing (ARCHITECTURE.md §8).
MIT allows derivatives; the approval is still required.

**Class + tier:** `content` / **deep** (`ports/ledger.csv`; provenance
`ports/_shared/source-map/atm10-sources.csv` — `refinedmods/refinedstorage2`).
The slice's shape is **machines**, per `horizon/docs/modpacks.md` §"What packs
need, and what runs today": block entities, containers, menus and a screen for
the network; no entities, renderers, dimensions or new vanilla hooks.

**Licence rank 0** (MIT, read from the jar's `LICENSE.md` and `mods.toml`).
Ported now because it is the **#1 core base** of the core-first queue: 13
pending ATM10 mods depend on it (`ports/tools/port_queue.py`).

Status: **slice 13 — the review round two repairs (0.2.13)** — the network core of slice 1, the
crafting chain of slice 2, the finite-range, same-dimension transmitter of slice 3, the item that
asks its admission (slice 4), the promise table keyed by network (slice 5), the transmitter's four
upgrade slots with the range and creative range upgrades (slice 6), the two transfer machines with
the speed and stack upgrades (slice 7), the network's energy model (slice 8), the grid item's
own 1,000 FE buffer (slice 9), the block drops that carry a broken machine's slots (slice 10), the
other disk sizes — 4k (4,096), 16k (16,384), 64k (65,536) and the creative disk, whose capacity
lives in the disk item's own component beside its contents (the creative one has none, upstream's
`null`, so it never refuses an item) (slice 11) — the in-place upgrade: a smaller disk plus the
bigger tier's storage part become that disk in one crafting-table step, the contents come across
with the destination's room, and the source disk's own part comes back (slice 12) — and now the
review's three blockers: one unit's payload per placed machine (the other units of a stack keep
theirs), one transaction for a disk's insertion *and* its consumption from the clicking hand, the
slots taken only at the break's completion gate (so a break that hands no item over leaves the
engine's spill alone), and both energy commands operator-only with a charge that names the wireless
grid (slice 13). Everything else is the gap list below, not a claim.

> The sections from `## Behaviour` to `### Verification after the repairs`
> describe the **retired `.hnmod`** layout. They are kept as history: the
> behaviour, the two review repairs and the in-game evidence they record still
> hold, and `src/runtime.rs` is byte-identical to the file they describe. The
> file names, the hook table and the packaging commands in them are **not** the
> current shape; the migration section at the end is.

## Behaviour (slice 1)

- **Network** (`runtime::network`): the connected component of the four network
  blocks over face-adjacent positions, capped at 512 blocks. A component with a
  controller is **online**; without one nothing works (upstream: the controller
  is the network's root and every node is inactive without it). The grid's
  `active` block state and the `/refinedstorage info` line both read it.
- **Cable connections** (upstream `computeConnections`): the block declares six
  boolean state properties (`north/east/south/west/up/down`), and the cable's
  tick writes each one from whether its neighbour is a network block. That is
  the pin's stand-in for upstream's code-baked cable model, and it makes the
  network visible to `ctl block`.
- **Disk Drive**: nine slots (upstream has eight; the pin's menus only show
  9/18/27/45/54 slots and a menu is how disks move in and out) with the
  `minecraft:generic_9x1` menu; a storage disk right-clicked onto it goes into
  the first free slot and leaves the hand. The drive has no hopper faces.
- **1k Storage Disk**: 1,024 items, stack size 1, one entry per resource (up to
  256 resources per disk), contents in the `refinedstorage:storage` item
  component (SNBT of each stored stack, so components survive), read back and
  cut to capacity on load. The contents travel with the disk item.
- **Grid**: a 9x6 menu (`minecraft:generic_9x6`) whose 54 slots are a *view* of
  the network's resources, sorted by amount ascending then id (upstream's
  default quantity sort), each entry capped to one stack. The grid's tick
  reconciles the slots the player changed: a decrease withdraws from the
  network, an increase deposits into it (all-or-nothing per stack; a deposit the
  network cannot take stays in the slot as the player's own), then the view is
  rebuilt. A player-owned slot (a refused deposit) is marked in the BE's saved
  `RefinedStorageView` and is never taken off the network.
- **Search**: `/refinedstorage filter <x> <y> <z> [text]` sets a case-insensitive
  id substring for that grid; the view applies it. Upstream's search box is the
  client's, and a mod menu on this pin has no text field, so the command is the
  slice's search (a recorded harness/behaviour addition, like
  `/enderstorage info`).
- **Commands** (`register_commands`/`perform_command`): `/refinedstorage info
  <pos>` prints the network's state, node census, disks and totals;
  `/refinedstorage filter <pos> [text]` sets or clears the search.

## Hooks

| Hook (API) | Helper | What it does |
|---|---|---|
| `component_types` (65) | `component_types.rs` | Registers `refinedstorage:storage`. |
| `block_entity` (15) | `block_entity.rs` | Creates the block entity of the four blocks. |
| `block_entity_ticks` (15) | `ticks.rs` | The cable, drive and grid tick; the controller does not. |
| `block_entity_container` (16) | `container.rs` | The drive's 9 slots (`generic_9x1`) and the grid's 54 (`generic_9x6`), no hopper faces. |
| `block_entity_tick` (17) | `tick.rs` | Cable connections, the drive's disk mirror, the grid's view and `active`. |
| `screen` (19) | `screen.rs` | The grid's 9x6 and the drive's 9-slot screen layouts (a mod menu shows on a Horizon client only through this). |
| `right_click_block` (21) | `right_click.rs` | A storage disk onto a drive goes in; every other click is the block's (the menu opens). |
| `world_lifecycle` (v1) | `world.rs` | A new world drops the disk cache; a closed one drops everything. |
| `register_commands` (8) | `register.rs` | `/refinedstorage info|filter`. |
| `perform_command` (8) | `perform.rs` | Their bodies. |

10 injections, 2 modules. Design notes that cost time to learn:

- A mod container menu's slots are **invisible to the game and to `ctl`** unless
  the mod declares a `native_screen` layout (API 19): the window still opens and
  `screen` says `container`, but `Game::slots` lists only the player inventory.
- The game gives every block entity its own slots, so the grid cannot write a
  drive's slots. The port keeps a per-disk cache in the module: a drive's tick
  mirrors its disk items into the cache (or writes a change back), and the
  grid's tick aggregates every drive in its component and reconciles the menu
  slots. The disk's item component is the truth whenever the drive is loaded and
  not mid-write (at most one tick behind).
- `native_set_block_property` keeps the block entity (a property change does not
  change the block), so the grid sets `active` from its own tick after it writes
  its slots.

## Upstream source analysis

See [`FINDINGS.md`](FINDINGS.md): the repository, commit, files read (the
network container/graph/controller, the cable, the drive and disk variant, the
grid menu/view/query/sorting, the storage component) and the behaviours
extracted. Where the rewrite differs is in FINDINGS §"Where this port differs"
and in the gaps below.

## Gaps / deviations (slice 1)

1. **No energy.** The controller is the network's on/off root but consumes and
   stores nothing; its `energy_type` state has two values (`off`/`on`) where
   upstream has four energy levels. No FE hook is needed or used.
2. **The rest of the mod does not exist**: crafters, exporters/importers,
   external storage, interfaces, relays, network transmitters/receivers,
   security manager, ~~wireless transmitter~~ (**done in slice 3, 0.2.3**; the
   block, its item, the facing-only connection rule, the same-dimension/16-block
   admission, its recipe and the `refinedstorage:wireless` advancement),
   ~~the wireless grid~~ (**done in slice 4, 0.2.4**), portable grid, crafting/pattern
   grids, autocrafting, fluid storage and disks, ~~the other disk sizes (4k/16k/
   64k/creative)~~ (**done in slice 11, 0.2.11**: all five disk items, each
   capacity saved in the disk's own component, the creative disk unlimited and
   reporting no ceiling, the three larger disks and the storage parts they are
   built from crafting from the 1k chain's own materials), storage blocks, ~~the range and creative range upgrades~~
   (**done in slice 6, 0.2.6**: the transmitter's four upgrade slots, the base
   `upgrade` item and its recipe, +8 blocks per range upgrade up to four, the
   creative one unlimited but never cross-dimension),
   ~~exporters/importers~~ (**done in slice 7, 0.2.7**: both blocks, their nine
   filter entries and four upgrade slots, their menus, the 9-tick work tick and
   the `speed_upgrade`/`stack_upgrade` items that change it), the **six inert
   upgrades** (`regulator`, `autocrafting`, `silk_touch`, `fortune_1/2/3` —
   `blocked(no-regulator-configuration)` / `blocked(no-autocrafting-system)`:
   shipped with the machines, but each still has no configuration surface or no
   crafting queue to act on, so shipping them would be inert; `silk_touch`/
   `fortune_1..3` stay `blocked(no-destructor)` and also need the
   `refinedstorage:upgrade_with_enchanted_book` recipe type, which the pin has no
   content-side recipe-type registry for), the config file, and the coloured
   variants of every block (the slice has the default colour only — the
   transmitter is the plain variant, and its tag is ready for the dyed ones).
3. ~~**No recipes.** Every recipe of the five items needs Refined Storage's own
   chain (`quartz_enriched_iron`, `machine_casing`, processors, cores,
   `1k_storage_part`, `storage_housing`) and NeoForge `#c:` tags; the items come
   from creative until that chain is ported.~~ **Resolved in slice 2 (0.2.2):**
   the chain and all 21 recipes are carried and proven in game — see
   `## Slice 2` at the end. The `#c:` tags the recipes test are declared by this
   package (vanilla members), not inherited from NeoForge's tag data.
4. **Disk capacity is fixed at 1k** and the disk is one item; the storage
   reference is a component of this port's own shape, not upstream's UUID into a
   world-level repository.
5. **Grid limits**: 54 entries (no scrolling), no view type/autocraftable
   filter, no sorting buttons (quantity ascending fixed), no last-modified
   tracking, no craft/autocraft button, no side buttons, no search box (the
   command instead), and the plain 9x6 panel rather than upstream's grid
   texture.
6. **Search** is a plain case-insensitive id substring: upstream's query
   language (`@mod`, `#tag`, `$tooltip`, operators) is not carried.
7. **Deposits and withdrawals are per menu slot**, all-or-nothing per stack:
   upstream's extraction/insertion strategies (half stack, cursor, one item,
   insertion hints) are not modelled; a refused deposit stays in the slot and is
   marked as the player's.
8. **The drive has 9 slots, no configuration** (access mode, void excess, fuzzy
   filter) and no per-slot item filter: the menu accepts any item, and a
   non-disk item in a disk slot is ignored (it does not become storage).
9. **No cable colour variants**, no waterlogging, and a cable's outline is its
   core only (the arms do not extend the outline). The cable's and drive's
   models are plain models: upstream's are baked in code (disks on the drive's
   front, a cable whose arms follow its connections), so the drive shows no
   animated disks.
10. **One tick of exposure**: a disk pulled out of a drive in the same tick as a
    grid change can lose that change (the drive writes the disk's component on
    its next tick); a chunk unload between them does the same.
11. **No comparator/deletion semantics** for the drive or grid (upstream's
    storage nodes answer redstone through the storage configuration).

## Verification

Pin **`1d63e0a5a37a7a708eb0b32b49bf0dfeb28ed8a9`**, mixins
**`5a64af938ec86b00d53d16a408aabc0038fb3e57`** (schema 6); checkouts
`ports/_shared/pins/horizon-1d63e0a` / `horizon-mixins-5a64af9`. Prepared 26.3
root `/home/adlf/Desktop/horizon/.local/horizon-client/26.3`. Every build ran
through `ports/tools/build-slot.sh` with `CARGO_INCREMENTAL=0`, never a bare
`cargo`, never `--target-dir`; the e2e ran in one slot invocation and used the
pin's shared `horizon-ctl`.

| Step | Command | Result |
|---|---|---|
| runtime tests (red proof first) | `cargo test --locked --offline` | **28 passed**, 0 failed; the tests were seen failing twice before they passed (two real bugs) and the suite was mutated red (`Network::online` forced true → 2 failures) before the final run |
| runtime clippy | `cargo clippy --all-targets --locked --offline -- -D warnings` | pass, **0 warnings** |
| format | `cargo fmt --check` | pass |
| stage | `python3 tools/horizon.py stage --horizon …/pins/horizon-1d63e0a` | `stage_changed: true`, revision `1d63e0a5…`, **10 injections, 2 modules** |
| check | `… check …` | `cargo_exit: 0`, **0 warnings** |
| embedded tests | `… test …` (`--lib game::refinedstorage::tests`) | `28 passed; 0 failed; 1240 filtered out` |
| fast build | `… build …` | `cargo_exit: 0`, 0 warnings, binary `…/target/fast/horizon-client` (110,464,192 B) |
| e2e | one slot `e2e` invocation, `tools/e2e.sh` | **PASS**, `EXIT:0` |
| fail proof | `SELFTEST=1 tools/e2e.sh` | **FAIL** at the flipped offline assertion (see below) |
| content `.hmod` | `cargo-horizon-mod build` + `check` | `content/refinedstorage.hmod`, **39,156 B**, SHA-256 `4f20a9df3d7c41b60048542468d1b2756897e366a09790cd8183631ae96bb691`, **58 entries**, content only |
| native `.hnmod` | `python3 tools/package.py` | `target/refinedstorage-0.1.0.hnmod`, **20,597 B**, SHA-256 `cca7da8c545a387f8e811d5826952b3c85b66471d52d0a985eae58fc6c7acefb`, 14 entries |
| content load check | `tools/hmod-load.sh` (shared unmodded game, `--dev-mod refinedstorage.hmod`) | the controller places and the disk is given; **without** `--dev-mod` the same ids are unknown (negative control), `EXIT:0` |

Evidence: `ports/_shared/evidence/refinedstorage/`
(`runtime-test.log`, `runtime-red-proof.log`, `horizon-check.log`,
`horizon-test.log`, `horizon-build.log`, `e2e.log`, `e2e-selftest.log`,
`hmod-build.log` (build and check), `hmod-load.log`, `hnmod.log`).

### In-game scenario (`tools/e2e.sh`)

The End arena away from the exit podium (network at 40..43,64,0), `--offscreen`,
creative. Quoted from `e2e.log`:

```
block(41,64,0): refinedstorage:cable[down=false,east=true,north=false,south=false,up=false,west=true]
block(43,64,0): refinedstorage:grid[active=true,facing=north,open=false]
info(43,64,0): Refined Storage network at 43,64,0: online (4 nodes, 1 controller, 1 drive, 1 grid); 0 disks, 0 items in 0 resources
grid view: []
drive: [{"count":1,"item":"refinedstorage:1k_storage_disk"}]
info(43,64,0): … 1 disks, 0 items in 0 resources
grid view: [{"count":3,"item":"minecraft:gold_ingot","slot":300},{"count":5,"item":"minecraft:diamond","slot":301}]
info(43,64,0): … 1 disks, 8 items in 2 resources
grid view: [{"count":3,"item":"minecraft:gold_ingot","slot":300}]      (search "gold")
grid view: [{"count":5,"item":"minecraft:diamond","slot":300}]         (search "diamond")
grid view: […both…]                                                    (search cleared)
grid view: [{"count":3,"item":"minecraft:gold_ingot","slot":300}]      (after the shift-click)
info(43,64,0): … 1 disks, 3 items in 1 resources
block(43,64,0): refinedstorage:grid[active=false,...]                  (controller broken)
info(43,64,0): … offline (3 nodes, 0 controller, 1 drive, 1 grid); 1 disks, 3 items in 1 resources
grid view: []                                                          (offline lists nothing)
block(43,64,0): refinedstorage:grid[active=true,...]                   (controller back)
grid view: [{"count":3,"item":"minecraft:gold_ingot","slot":300}]      (the disk kept it)
block(50,64,0): refinedstorage:grid[active=true,...]                   (second network)
info(50,64,0): … online (3 nodes, 1 controller, 0 drive, 1 grid); 0 disks, 0 items in 0 resources
grid view: []                                                          (no drive)
grid view: [{"count":3,"item":"minecraft:gold_ingot","slot":300}]      (the first grid still does)
drive after a stick: [{"count":1,"item":"refinedstorage:1k_storage_disk"}]
info(56,64,0): No Refined Storage network block there.                 (vanilla barrel)
… relaunch: info 1 disks, 3 items in 1 resources; grid view [gold 3]
PASS: … EXIT:0
```

The controls are inside the scenario: **no controller** (the network goes
offline, `active=false`, the grid lists nothing while the disk still holds the
gold, and restoring the controller lists it again), **no drive** (an online
network beside the first one lists nothing) and **wrong item** (a stick does not
enter the drive; a vanilla barrel is not a network block). `SELFTEST=1` flips
the offline assertion and the run fails, so the scenario can fail:

```
(selftest: asserting the offline grid still lists the gold)
FAIL: the grid's view is [], expected [{"count":3,"item":"minecraft:gold_ingot","slot":300}]
```

### Harness facts learned here

- A mod container's slots are listed (and clickable) only with a `native_screen`
  layout; without one the menu opens but `ctl screen` shows only the player
  inventory, and `click-slot 300+` has nothing to click.
- For a `minecraft:generic_9xN` mod menu the *client* holds a chest block
  entity, so the slots are `300+` and the player's are `0..35` (unlike a mod
  menu type, which needs the mod's own screen).
- `ctl container` reads the block entity's slots and inserts into the first
  empty one; `ctl interact` right-clicks the block (and our hook cancels it for
  a disk onto a drive).
- Killing every entity in the End kills the dragon, whose exit podium (an
  `end_portal` at 0,63,0) then shows the end credits to a player standing in it;
  the arena is built at x=40 and only item entities are killed.

## Embedded content — one `.hnmod` deliverable (2026-11-10)

The content half now travels **inside** the native bundle (the AE2 pattern, proved
on `charginggadgets`): the built `content/refinedstorage.hmod` is carried at
`mixins/refinedstorage/data/refinedstorage-content.hmod`; `embedded_content.rs` writes it to
`.horizon-cache/` at start and appends it to the dev list from one `content_root`
HEAD injection (`cancellable = true`; bindings `root`, `enabled`, `dev`). One
installed `.hnmod` installs both halves, so the split content+code blocker is
gone. `tools/package.py` packs `data/` and refuses a copy that differs from
`content/refinedstorage.hmod`.

Verification (2026-11-10, pin `1d63e0a`, Minecraft 26.3):
- **ladder**: `cargo fmt --check`, `clippy --all-targets -- -D warnings` (0
  warnings), `cargo test` **28 passed / 0 failed**, `horizon.py stage / check / test /
  build` all `cargo_exit: 0` (embedded tests included).
- **artifacts**: `content/refinedstorage.hmod` **39,156 B**, SHA-256 `4f20a9df3d7c41b60048542468d1b2756897e366a09790cd8183631ae96bb691`,
  **58** entries; `refinedstorage.hnmod` **52,582 B**, SHA-256 `de6f5be1fde05e7616409582ce62bc73210b9d321e2b0a3528b79609b5cfa306`, **16**
  entries (rebuilt locally from these sources with `tools/package.py`; a second
  run is byte-identical and `horizon-mod check --kind native-mod` passes);
  `mixins/refinedstorage/data/refinedstorage-content.hmod` is byte-identical to the built `.hmod`.
- **e2e with no `--dev-mod`** (`EMBEDDED=1`): `PASS` + `EXIT:0`; the run printed
  `embedded content: 39156 B, identical to content/refinedstorage.hmod`.
- **control** (`e2e-control.log`, `.horizon-cache` made unusable): the sample id
  is unknown, `PASS` + `EXIT:0` - the content really comes from the bundle.
- **fail leg** (`SELFTEST=1`): `FAIL` + `EXIT:1`.
- `check-e2e-logs.py refinedstorage --strict` green (all logs end `EXIT:<n>`);
  `upstream_ready.py refinedstorage` -> `READY`, exit 0.

The compile ladder ran on the second build machine; the in-game legs ran locally.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

### Mode B + AE2 embedding (2026-11-10) — one listable artifact, re-verified

Pin unchanged (`1d63e0a5…` / `5a64af93…`, schema 6, `game.rs`
`64813cf4…f80f`). The session rebuilt the port from its current sources and
applied the **AE2 embedding pattern** so one `.hnmod` installs the content and
the code (the registry keeps one file per project): `mixins/refinedstorage/data/refinedstorage-content.hmod`
is the built `content/refinedstorage.hmod` byte for byte, `mixins/refinedstorage/embedded_content.rs`
compiles it in with `native_data!`, and the `embedded-content` `[[injection]]`
(`target = "horizon_client::game::mods::content_root"`, `kind = "HEAD"`,
`cancellable = true`, bindings `root`/`enabled`/`dev`) writes it to the game's
`.horizon-cache` and appends it to the dev-mod list. `tools/package.py` refuses
an embedded copy that differs from the built hmod.

All compile/e2e steps ran through `ports/tools/build-slot.sh` with
`CARGO_INCREMENTAL=0`; the ladder is the shared `tools/ladder-embed.sh`
(hmod build → embed → hnmod → fmt → clippy → test → stage/check/test/build),
evidence `ports/_shared/evidence/refinedstorage/ladder-*`.

| Step | Result |
|---|---|
| `cargo fmt --all --check` / clippy `-D warnings` | exit 0, **0 warnings** |
| `cargo test --locked --offline` | **28 passed**, 0 failed |
| `horizon.py stage/check/test/build` | `cargo_exit: 0` in every step |
| `content/refinedstorage.hmod` | **39,156 B**, `4f20a9df…b691`, **58 entries**, "no code: content only" |
| `refinedstorage.hnmod` (single artifact, carries the content) | **52,582 B**, `de6f5be1…a306`, **16 entries** |
| e2e (with `EMBEDDED=1`, the shipped shape) | default **PASS + `EXIT:0`** / selftest **FAIL + `EXIT:1`** |
| `check-e2e-logs.py refinedstorage --strict` | green (`logs=… ok=…`, exit 0) |
| `upstream_ready.py --copyleft refinedstorage` | **READY**, exit 0 |

Fail-capability: the selftest/negative legs flip a real assertion and exit
non-zero; the embedded shape is the one the artifact ships (no `--dev-mod`), and
the e2e checks the bundle wrote its content out (or the loader reports
`content from 1 mod(s)` when the game consumes the file).

## Review repairs (PR #608)

1. Staging is the orchestrator's (horizon-mods#686).
2. *"The original-project link is relative … The description also has broken
   bold markers and truncated sentences (ending 'That is' and 'and the
   coloured')"* — the entry links
   `https://github.com/refinedmods/refinedstorage2`, credits Refined Mods, and
   the description is rewritten complete.
3. **[P1] refused deposits duplicated the virtual stack** — a refused deposit
   wrote only its remainder to the slot but the rebuild copied the
   pre-reconciliation snapshot, so the player kept the displayed items *and* the
   disk kept the originals. The rebuild now uses the post-reconciliation slot
   content (`current`), never `after`.
4. **[P1] two grids could withdraw the same backing items** — each grid's view
   now sets aside what the other open grids already show
   (`runtime::view_with`, a per-grid reservation table in the support module),
   so the total shown across grids never exceeds the disks' stock; the
   regression test is `a_second_grid_cannot_show_what_the_first_already_shows`.
   A drive removed between a view and a click remains a recorded gap (the pin
   has no per-click transaction hook for a mod's container menu).
5. **[P2] notices** — `mixins/refinedstorage/LICENSE` and `NOTICE.md` travel
   inside the `.hnmod`, with Refined Mods' MIT notice added to the repository
   `LICENSE`, and both sit at the embedded content archive's root.

### Verification after the repairs

- Artifact: `refinedstorage.hnmod`, 56,436 B, SHA-256 `0b66dd7db04b42b59f77fd97d6293ea1a6b702b8d1a55628881c958845105c25` (rebuilt twice, identical bytes; `tools/package.py`; the embedded content equals `content/refinedstorage.hmod`).
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --locked --offline`: clean, 29 passed; the embedded run (`python3 tools/horizon.py test --horizon ports/_shared/pins/horizon-1d63e0a`) is 29 passed, 0 failed (`target/horizon-test.log`).
- `build-slot.sh e2e --job … --port ports/refinedstorage -- tools/e2e.sh`: PASS, log `ports/_shared/evidence/refinedstorage/e2e-repair.log` ends `EXIT:0` (`check-e2e-logs.py refinedstorage --strict` exit 0).
- `python3 ports/tools/upstream_ready.py refinedstorage` and `check-skills.py --notes ports/refinedstorage`: exit 0.

## Migration to the SDK format (2026-11-18) — version 0.2.0

Horizon retired the `.hnmod` container with the mod-API merge (#16941, launcher
≥ 0.2.70; the registry closed #869 for it, #17992). This port is now **one flat
`.hmod`**: `horizon-mod.toml` + `Cargo.toml` + `src/` + `assets/` + `data/`,
built by the launcher against the build `[native]` pins.

- **SDK:** nightly `0b14f8acfd97fa2c750e42795249dcc981178ca4` (2026-11-18),
  mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, launcher 0.2.78,
  `cargo-horizon-mod` installed. `.horizon/sdk/` is fetched per clone and never
  packed.
- **Class + tier unchanged:** `content` / `deep` with the `machines` shape
  (`ports/ledger.csv`, `horizon/docs/modpacks.md`); this migration changes the
  package format, not the slice.
- **`src/runtime.rs` is the old `mixins/refinedstorage/runtime.rs`, byte for
  byte** (SHA-256 `938bd3d657282b24faec86d5c32d3c1e5cbc665c9af4dd99f14cce8d0fd455f3`,
  1,001 lines, **29 tests**). It is the campaign's rule made literal: the logic
  and its two review-repair tests did not change while the glue was rewritten.

### File map

| retired `.hnmod` layout | flat SDK package |
|---|---|
| `mixins/refinedstorage/runtime.rs` | `src/runtime.rs` (byte-identical) |
| `mixins/refinedstorage/support.rs` (injected module) | `src/support.rs` (ordinary module) |
| `mixins/refinedstorage/{block_entity,ticks,tick,container,screen,right_click,world,register,perform,component_types}.rs` (one helper per injection) | methods of the one `impl Hooks` in `src/lib.rs` + the Part in `src/support.rs` (table below) |
| `mixins/refinedstorage/embedded_content.rs` + `data/refinedstorage-content.hmod` (the AE2 trick that wrote the content out) | **gone**: the flat package carries content and code together |
| `mixins/refinedstorage/horizon-mixin.toml` (10 injections, 2 modules) | no plan: `horizon-mod.toml` + the `Hooks` impl (13 hooks subscribed) |
| `horizon-native.json` | `horizon-mod.toml` `[native]` (`game`/`horizon`/`mixins`) |
| `content/horizon-mod.toml` + `content/assets` + `content/data` | one root `horizon-mod.toml` + `assets/` + `data/` |
| `content/refinedstorage.hmod`, `refinedstorage.hnmod` (two files, one project) | `refinedstorage.hmod` (one file) |
| `compatibility.json`, `rust-toolchain.toml`, `tools/{horizon.py,package.py,ladder-embed.sh,hmod-load.sh}` | not part of the format; removed (the retired files and their hashes are in `ports/_shared/evidence/refinedstorage/retired-hnmod-files.txt`) |

### Helper → hook table (the whole glue, mapped)

| Old helper (old API) | New form (SDK 0.2) | Verdict |
|---|---|---|
| `component_types.rs` — `component_types` (65) | `Hooks::component_types(_cx, &mut ComponentTypesEvent)` pushes the name into `event.types` | **kept** |
| `block_entity.rs` — `block_entity` (15) | `BlockEntityPart::block_entity(level, &BlockEntityEvent) -> Option<Compound>` | **kept** (replace contract; returns the empty saved data) |
| `ticks.rs` — `block_entity_ticks` (15) | `BlockEntityPart::block_entity_ticks(&dyn Host, &mut BlockEntityTicksEvent)` sets `event.ticks` | **kept** (cable, drive and grid; not the controller) |
| `container.rs` — `block_entity_container` (16) | **no hook at this pin**: `Registrar::block_container(&BlockKey, ModContainer)` in `register` (slots, six faces, inputs, outputs, the vanilla menu id) | **kept, moved to registration** |
| `tick.rs` — `block_entity_tick` (17) | `BlockEntityPart::block_entity_tick(&mut dyn ServerHost, &mut BlockEntityTickEvent)`; `event.entity` is the saved compound + item slots + data slots | **kept** (one Part for all four blocks) |
| `screen.rs` — `screen` (19) | `BlockEntityPart::screen(&dyn Host, &mut ScreenEvent) -> Option<Screen>` | **kept** (9×6 grid, 9-slot drive) |
| `right_click.rs` — `right_click_block` (21) | `Hooks::right_click_block(HostSide, &mut RightClickBlockEvent) -> Flow`; the held stack is `event.click.stack`, the insert runs in `host.transaction` | **kept** |
| `world.rs` — `world_lifecycle` (v1, `WorldOpenReady`/close) | `Hooks::server_started` (the doc says it fires where `WorldOpenReady` did) for the open, `Hooks::world_lifecycle` (v2) for `WorldOpenFailed`/`WorldCloseFinal`/`WorldCloseFailed` | **kept across two hooks** |
| `register.rs` — `register_commands` (8) | `Hooks::register_commands(&mut dyn ServerHost, &mut RegisterCommandsEvent)` pushes `Command`s | **kept** (v2) |
| `perform.rs` — `perform_command` (8) | `Hooks::run_mod_command(...) -> Flow`; arguments are typed (`BlockPos`, `String`) instead of split words | **kept** (v2) |
| `embedded_content.rs` + `content_root` HEAD injection | — | **retired by the format** (one package holds both halves; not a gap) |
| `support.rs` state/disk cache/grid reconcile/commands | `src/support.rs`, same functions with `ServerHost`/`LevelRef` instead of `Game` | **kept** |

The two **review repairs** live in `runtime.rs` and are untouched:

1. the grid rebuilds its slots from the **post-reconciliation** content
   (`current`), never the pre-state snapshot, so a refused deposit cannot
   duplicate a virtual stack (`a_slot_the_network_could_not_take_is_the_player_s`,
   `a_grid_view_round_trip_conserves_items`);
2. each grid's view sets aside what the other grids already show
   (`runtime::view_with` + the per-grid reservation table in `support`), so two
   grids never withdraw the same backing items
   (`a_second_grid_cannot_show_what_the_first_already_shows`).

### Manifests

- `horizon-mod.toml` — `id = "refinedstorage"` (the namespace), `version =
  "0.2.0"` (the port's version, first SDK release), `api = "0.2"`, `side =
  "both"` (the grid and drive draw a screen), `capabilities = ["registries"]`
  (five blocks + one item), `[native]` `game = "26.3"`,
  `horizon = "0b14f8ac…"`, `mixins = "e59da909…"`, plus the licence metadata
  (`license`, `homepage`, `issues`; measured to survive `check`, `build`,
  `check <file>` and `fits` and to be packed — verify the packaged
  `horizon-mod.toml` still carries `license` after editing the manifest).
  Cargo's package name/version are not the identity.
- `Cargo.toml` — **`horizon-mod-api` alone**. The migration had added a second
  path dependency (`horizon-core`, for `JavaString`) and that is what made the
  launcher's `native-build` fail; see
  [`## Build fix and the first in-game leg`](#build-fix-and-the-first-in-game-leg-2026-11-19--version-021).
  Saved-compound keys are read with `support::field` and written with
  `KEY.into()`.

### Build, check, package

```sh
ports/tools/build-slot.sh run --job sdk-refinedstorage --port ports/refinedstorage \
    --step check -- bash -c 'cd ports/refinedstorage && cargo horizon-mod check'
cd ports/refinedstorage && cargo horizon-mod build          # -> target/horizon-mod/refinedstorage.hmod
cargo horizon-mod check refinedstorage.hmod                 # validates the package
cargo horizon-mod fits refinedstorage.hmod                  # does it still fit the pin
```

### Verification (this migration)

| Step | Command | Result |
|---|---|---|
| runtime tests (byte-identical file) | `cargo test` | **29 passed**, 0 failed |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, **0 warnings** |
| formatting | `rustfmt --edition 2024 --check src/lib.rs src/support.rs src/runtime.rs` | clean (see trap 2: not `cargo fmt --all`) |
| SDK check (code + manifest) | `cargo horizon-mod check` | exit 0 — *"Refined Storage uses 352 items of the API (5 Part kinds, 1 associated item, 1 capability, 79 fields or variants, 13 hooks, 15 hook arguments, 2 macros, 114 methods, 25 traits, 97 types). Made for 0b14f8a."* |
| package | `cargo horizon-mod build` | `refinedstorage.hmod`, **73,129 B**, SHA-256 `7267e6438204b3b7dc006c7955c096bcb30b41967bc1ce5e65735ea939d4823f`, **67 entries**; a second build is byte-identical |
| package check | `cargo horizon-mod check refinedstorage.hmod` | exit 0 — *"refinedstorage 0.2.0 (native-mod, format native-1/0b14f8ac…/e59da909…, for 26.3) 73129 bytes, SHA-256 7267e643…, unsigned"* |
| fits | `cargo horizon-mod fits refinedstorage.hmod` | exit 0 — *"Refined Storage: 0b14f8a  made for"* |
| in-game e2e | `tools/e2e.sh` | **not run on this box** at that time: there was no `horizon-launcher` binary here, so no launcher-built game existed to load the package. **Superseded 2026-11-19** — the launcher build and the in-game leg are green; see the last section. |

Evidence: `ports/_shared/evidence/refinedstorage/` — `sdk-test.log` (29 tests),
`sdk-lint.log` (`rustfmt` + `clippy -D warnings`), `sdk-package.log` (the SDK
check, the build, the package check and `fits`), and `retired-hnmod-files.txt`
(the old layout's files and hashes, kept because the retired sources are gone).

### Gaps (this migration)

1. **~~The in-game e2e has not run at this pin.~~ Resolved 2026-11-19:** the
   launcher build now succeeds and `launcher-game.sh run refinedstorage` is
   `PASS` + `EXIT:0` with a control leg at `EXIT:1` (the section at the end).
2. **`ports/tools/upstream_ready.py refinedstorage` still reports
   `blocked`** because it checks the retired layout (`content/*.hmod`,
   `*.hnmod`, `compatibility.json`). It is a campaign tool, outside this port's
   write ownership; the SDK gate it should ask is `cargo horizon-mod check`
   plus `cargo horizon-mod fits` (both green here).
3. **`block_entity_container` has no hook equivalent.** The container is a
   registration-time declaration; a port that needs per-state containers (a
   menu whose slot count changes with a block property) has no path at this
   pin. This port does not need one.

### Traps for the next large port

1. **The retired tree must be gone before `check` runs.** With `mixins/` still
   in the port, `cargo horizon-mod check` fails with
   `unexpected package path: mixins/refinedstorage/runtime.rs` — the tool walks
   the directory and refuses paths the format does not have. Copy
   `runtime.rs` out first (byte-identical!), then remove the old tree.
2. **`cargo fmt --all` reaches into `.horizon/sdk`** and reformats the vendored
   crate's `build.rs` (measured here). Format only the port's own files:
   `rustfmt --edition 2024 --check src/lib.rs src/support.rs src/runtime.rs`.
3. **`LevelRef` has no `block_entity`.** The read is a host method:
   `host.block_entity(level, pos) -> Option<BlockEntityRef>`; the handle then
   gives `item_slot`/`set_item_slot` (the write takes `&Transaction`, so wrap it
   in `host.transaction`).
4. **`WorldOpenReady` is gone** from `WorldLifecycleEvent` (v2 carries
   `WorldOpenFailed`, `WorldCloseFinal`, `WorldCloseFailed`). Use
   `Hooks::server_started` for "a world opened"; a port that only listens to
   `world_lifecycle` never resets its caches.
5. **`JavaString` is not re-exported, and a second path dependency is not the
   answer**: the launcher supplies the API as an `--extern` and has no
   `horizon_core`, so `horizon-core = { path = … }` fails the build with
   `error: No such file or directory (os error 2)`. Read keys with
   `support::field` (UTF-16 comparison, the same shape as `Tag::get(&str)`) and
   write them with `KEY.into()`.
6. **Registry keys name themselves only through a host** (`ItemKey::id(host)`,
   `block_id` is `pub(crate)`). Keep the `BlockKey`/`ItemKey` handles in a
   `OnceLock` set in `register` and compare handles; `Kind::from_block(&str)`
   is only for hooks that hand you a block-id string (`block_entity_ticks`,
   `screen`).
7. **One Part per block-entity *type*, not per block.** The four network blocks
   share one type; the Part tells them apart by the block at `event.pos`
   (`host.block_entity(level, pos)`'s `kind` is the same for all four). Declaring
   four types would also give four ids the content does not name.
8. **`register` takes `&self`**, so the Part and the commands read the keys from
   a `OnceLock`, not from a field of `Mod`.
9. **The tick's writes are real now.** The old pin reverted a block entity's
   writes inside its own tick; at this pin `block_entity_tick` keeps the
   payload (`Changed`), so the cable's state properties and the grid's mirrored
   slots are written from the tick as before — but a `Unchanged` reply would
   silently discard them.
10. **`ModContainer` and `Screen` literals should end with `..Default::default()`**:
    the check prints a compatibility note when they are built field by field.
11. **The `.hmod` lives in the port root**, not `target/` (`cargo horizon-mod
    build` writes `target/horizon-mod/`): copy it out after every build. The
    build is deterministic (zip mtimes are fixed), so the SHA-256 is stable.

### Changelog for the resubmission (supersedes #869)

> Supersedes #869, closed because Horizon retired `.hnmod` (#17992). Changes:
> flat SDK package with `api = "0.2"` and `[native]` pinned to `0b14f8a`;
> hooks rewritten as `impl mod_api::Hooks` methods against typed hosts/events
> (11 old helpers mapped: 10 kept, 1 retired by the format itself, 0 faked;
> `block_entity_container` became a registration-time `block_container`);
> `runtime.rs` and its tests unchanged (29 tests, including the two review
> repairs: a refused deposit no longer duplicates a virtual stack, and two grids
> no longer withdraw the same backing items).

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

## Build fix and the first in-game leg (2026-11-19) — version 0.2.1

The package had **never been built by the launcher** and had **never run in a game**
at this pin. Both now hold. Two defects, one found by the build, one by the game.

### Defect 1 — the launcher could not build the package (fixed)

`Cargo.toml` carried a second SDK path dependency, `horizon-core`, added for
`JavaString` (a compound's key type). The launcher's `native-build` removes the
API's path dependency and supplies `mod_api` as an `--extern`; it supplies **no
`horizon_core`**, so the build died before compiling:

```
$ ports/tools/launcher-game.sh build refinedstorage --rebuild
Compiling the mods against the prebuilt game
error: No such file or directory (os error 2)
launcher-game.sh: horizon-launcher native-build failed for refinedstorage 10
```

`ports/rftoolsstorage` hit the identical defect and its fix is the reference
(measured independently in
`ports/_shared/evidence/refinedtypes-e2e/launcher-externs.md`: the build plan's
externs are `horizon_client, horizon_mod_host, horizon_mod_package, mod_api,
mod_points, mod_vocab`). The dependency is gone; saved-compound keys are read
through `support::field` (spell the key in UTF-16, compare the stored units —
the same shape as `Tag::get(&str)`, which the API re-exports but a `Compound`
cannot be asked directly because `BTreeMap::get` wants the key type) and written
with `KEY.into()`. The read path is pinned by
`support::tests::a_saved_view_mark_is_found_by_its_java_key`, whose red proof was
a mutated lookup (first entry instead of the named one → the test failed).

### Defect 2 — a deposit the disk took left a 1-item ghost (fixed)

The first in-game run failed at the deposit assertion:

```
FAIL: the grid's view is [{"count":1,"item":"minecraft:diamond","slot":300},
                          {"count":1,"item":"minecraft:gold_ingot","slot":301},
                          {"count":3,"item":"minecraft:gold_ingot","slot":302}],
      expected [{"count":3,"item":"minecraft:gold_ingot","slot":300},
                {"count":5,"item":"minecraft:diamond","slot":301}]
```

Root cause, traced in game with a temporary tick trace stored in the grid's saved
compound and read back through `/refinedstorage info`:
`runtime::insert` answered the remainder as a **stack**, and `Net::with_count`
clamped its count with `.max(1)` — `ItemStack::set_count(0)` is an error, so a
zero-count stack cannot exist. A deposit the disk took **entirely** therefore
came back as a **1-item stack**, the tick read that as a refused remainder
(`taken.count() > 0`), marked the slot `owned`, wrote 1 item back into the menu
and rebuilt the view from the network — so the grid showed the deposit **twice**
(1 owned + the whole stack in the view), the same duplication class review
repair #1 removed for refused deposits. The trace of the failing tick:

```
[i1 dcap1024 tot5 free1019 n1 b0/0/0 a5/0/0 o1/0/0 c1/0/0 D0:->1 ]
   ^ the disk already held the 5      ^ 5 placed   ^ marked owned   ^ "1 left over"
```

The fix makes "nothing left over" unrepresentable as a stack instead of clamping
it:

- `Disk::insert` answers **how many items it took** (`u32`), not a stack;
- `runtime::insert` answers `Option<T>`: `None` when the network took every
  item, `Some(left)` with `left.count() >= 1` otherwise;
- the grid's deposit path takes the `Option` (`Some` = the player keeps the
  remainder, `None` = the slot is rebuilt from the view).

The regression test was written first and seen red —
`a_stack_the_disk_took_entirely_leaves_nothing_over` uses a `Stored` double whose
`with_count` clamps like `ItemStack` (the real constraint) and asserted
`left.count() == 0`; it failed with `left: 1`. `Net::with_count` keeps the clamp
and now says why: a stack cannot hold zero items, so callers ask `insert` for the
count.

**`src/runtime.rs` is no longer byte-identical** to the migration's copy
(`938bd3d6…` → `79b72b93…`): the `insert` contract changed and one test was
added. Everything else in the file (the network, the disks, the view, the two
review repairs) is untouched and its tests still pass.

### Ladder and the in-game leg

Pin `0b14f8acfd97fa2c750e42795249dcc981178ca4`, mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, launcher 0.2.78, game 26.3.
Every cargo step ran through `ports/tools/build-slot.sh` with
`CARGO_INCREMENTAL=0`; the in-game leg through
`ports/tools/launcher-game.sh run refinedstorage` (one slot lease, `e2e-guard`
active, never `E2E_GUARD=0`).

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `cargo test --offline` | **31 passed**, 0 failed |
| the new test, before the fix | same | **FAILED** (`left: 1`, `right: 0`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | exit 0, **byte-identical**, **72,215 B**, SHA-256 `7be4a4152b9d5ecab139b8c4f9ae87208788f4e9f6842c9d167fb4316830fd38`, **66 entries**, **no `Cargo.lock`** |
| package check | `cargo horizon-mod check refinedstorage.hmod` | exit 0 — *"refinedstorage 0.2.1 (native-mod, format native-1/0b14f8a…/e59da90…, for 26.3) 72215 bytes, SHA-256 7be4a415…"* |
| fits | `cargo horizon-mod fits refinedstorage.hmod --build 0b14f8a` | exit 0 — *"Refined Storage: 0b14f8a made for"* |
| **launcher build** (never passed before) | `launcher-game.sh build refinedstorage --rebuild` | exit **0** — it compiled (`Compiling the mods against the prebuilt game / Compiling refinedstorage (1 of 1) / Your mods are built`) and installed `native-0b14f8a-34c3ea44c166`; the recorded run of the same package then answers *"Up to date (no build needed)"* and re-exports that build, and `diff -r ports/refinedstorage/src <installed>/horizon-native-mods/refinedstorage/src` is empty (the manifest and `Cargo.toml` compare equal too), so the installed build is this source |
| **in-game e2e** | `launcher-game.sh run refinedstorage` | **PASS** + **`E2E EXIT:0`** |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME … tools/e2e.sh` | **FAIL** + `EXIT:1` — *"the block at 41,64,0 is minecraft:air, expected west=true"* |
| selftest (modded game, flipped assertion) | `SELFTEST=1 … tools/e2e.sh` | **FAIL** + `EXIT:1` |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | `logs=3 ok=3 missing-exit=0 bad-exit=0`, exit 0 |

The scenario is the one this file has described since slice 1 (the End arena, the
network at 40..43,64,0): cable connections, `active`, the 9×6 grid menu, a disk
into the drive, deposits and a shift-click withdrawal through the menu, the
search, the controller-out/in control, the second-grid reservation, the
drive-less network, the stick and the barrel controls, and the disk's contents
across a relaunch. It had never executed at this pin; it does now, and every
assertion in it is green.

Evidence: `ports/_shared/evidence/refinedstorage/` (`launcher-build-before.log`
— the reproduced defect and `EXIT:10`; `sdk-test.log`, `sdk-clippy.log`,
`sdk-package.log`, `sdk-check-fits.log`) and
`ports/_shared/evidence/refinedstorage-e2e/` (`launcher-build.log`,
`e2e-launcher.log`, `e2e-launcher-control.log`, `e2e-launcher-selftest.log`).

### What is still not done

- `ports/tools/upstream_ready.py refinedstorage` still reads the retired layout
  (gap 2 above); the SDK gate is `cargo horizon-mod check` + `fits`, both green.
- The gap list from slice 1 stands, except recipes (slice 2): no energy, no
  crafters/exporters/importers, one disk size, 54 grid entries, command-driven
  search.

Skills: superpowers/using-superpowers,systematic-debugging,test-driven-development,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

## Slice 2 — the crafting chain, every recipe proven in game (2026-11-21) — version 0.2.2

Gap line this slice answers — §"Gaps / deviations (slice 1)" item 3: *"**No recipes.** Every
recipe of the five items needs Refined Storage's own chain (`quartz_enriched_iron`,
`machine_casing`, processors, cores, `1k_storage_part`, `storage_housing`) and NeoForge `#c:`
tags; the items come from creative until that chain is ported."* It is also the first of the
*"What is still not done"* gaps, and the parity rule's sharpest case: a carried recipe that is
never crafted is dead data, and until this slice the five shipped blocks existed only from
creative.

Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`**, mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, SDK `0a875da`, launcher 0.2.92, game 26.3
(0.2.1 was built at `0b14f8a` and migrated here). Class `content` / tier **deep**, shape
**machines**, unchanged: recipes are data-tier *content*, so this slice adds a family below the
tier and cannot exceed it. No new pin API and no third-party mod: every ingredient is vanilla
or a chain item this package declares.

`src/runtime.rs` is **untouched** (`79b72b93…`, byte-identical in the package): this is a
mechanical carry from the jar, so there is no new runtime rule to unit-test. The red proof is
therefore the in-game one below, against the previous package.

### What was carried (from `refinedstorage-neoforge-2.0.9.jar`, MIT, tag v2.0.9)

| family | carried |
|---|---|
| items | 14: `quartz_enriched_iron`, `quartz_enriched_copper`, `silicon`, `processor_binding`, `raw_/basic_/improved_/advanced_processor` (raw + cooked, 6), `construction_core`, `destruction_core`, `1k_storage_part`, `storage_housing` — `minecraft:paper` template, own texture, model, item definition, lang, creative tab |
| block | `machine_casing` (a block upstream): `minecraft:stone` template, cube-column model from the jar, own textures, loot table, pickaxe tag, tab |
| recipes | 21: the 15 chain recipes and the 6 that reach the shipped things (`controller`, `cable` ×12, `disk_drive`, `grid`, `1k_storage_disk`, the shapeless `1k_storage_disk_from_storage_housing` alternative) |
| tags | 14 `c:` tags the recipes test, with vanilla members; plus upstream's own `c:ingots` (both enriched ingots) and `c:silicon` contributions, as upstream writes them |

The jar's 1.21.1 NeoForge ingredients (`{"item": …}` / `{"tag": …}`) are written in the pin's
shape (`"id"` / `"#tag"`); smelting keeps upstream's `experience` and 200-tick time.

### Red proof — the same driver against the previous package (0.2.1 at `0b14f8a`)

`tools/recipe_check.py` reads this package's **own** `data/refinedstorage/recipe/*.json`, so a
recipe that is not crafted fails the run. Against 0.2.1 the chain does not exist, and the
first recipe of the alphabetical run fails in the game:

```
recipe 1k_storage_disk (minecraft:crafting_shaped)
  1k_storage_disk -> None x0
…
FAIL: no refinedstorage:1k_storage_part in the inventory to place
FAIL: no refinedstorage:quartz_enriched_iron in the inventory to place
FAIL: 1k_storage_disk: expected refinedstorage:1k_storage_disk x1, got None x0
```

62 assertions failed, `EXIT:1` (`ports/_shared/evidence/refinedstorage-e2e/e2e-red-recipes-0b14f8a.log`).
Before the driver existed the same gap was `no recipes under 0.2.1`'s recipe directory
(`e2e-red-recipes-0b14f8a-nodata.log`), and the first full-repo attempt exposed two driver
defects of my own (a tag read as an item id; a `jq` field on a string) — kept as
`red-recipes-driver-bug-intermediate.txt`, not as exit evidence.

### Green — the in-game leg (one slot lease, `launcher-game.sh` build at `0a875da`)

```
== the crafting chain: every carried recipe crafts in game ==
recipe PASS: 21 of 21 carried recipes craft in game with their declared output and count,
             their ingredients are consumed, 10 wrong-ingredient controls leave the result
             empty, and #c:stones takes cobblestone as well as stone
  machine casing drops: {"refinedstorage:machine_casing":1}
  refinedstorage:no_such_item refused, refinedstorage:no_such_block did not place (minecraft:air)
…
PASS: every carried recipe crafts in game … and the disk kept its 3 gold across a relaunch
EXIT:0
```

The driver asserts, per recipe: the result slot's **id and count**, that a shift-click leaves
the 3×3 grid empty (the ingredients were consumed) and the crafted stack in the inventory, and
a counterfactual (one ingredient replaced by a non-ingredient → no result). Every control is
per-recipe, plus the `#c:stones` differential below, so the assertions can fail — and did,
twice, before the content was right (the tag's missing cobblestone; the loot table's 26.3
shape).

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `cargo test --offline` | **31 passed**, 0 failed (`sdk-test.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean |
| SDK check | `cargo horizon-mod check` | *"…354 items of the API (… 13 hooks …). Made for 0a875da."* |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical**, **116,902 B**, SHA-256 `3c26f5a4986847a1090f9582a65cd0341ee6e60efaad30687e86c678d99d1ad4`, **165 entries**, no `Cargo.lock` |
| package check | `cargo horizon-mod check refinedstorage.hmod` | `0.2.2 (native-mod, format native-1/0a875da…/e59da909…, for 26.3) 116902 bytes` |
| fits (pin) | `… fits refinedstorage.hmod --build 0a875da` | *"0a875da made for"* (a tautology on its own) |
| fits (other build) | `… fits refinedstorage.hmod --build 0b14f8a` | *"0b14f8a fits"* — the one that proves something |
| launcher build | `launcher-game.sh build refinedstorage --rebuild` | exit 0, `native-0a875da-0e04cf451395` |
| in-game leg | `build-slot.sh run --step e2e -- bash tools/e2e.sh` | **PASS** + **`EXIT:0`** |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME …` | **FAIL** + `EXIT:1` — *"the block at 41,64,0 is minecraft:air, expected west=true"* |
| selftest (flipped assertion) | `SELFTEST=1 …` | **FAIL** + `EXIT:1` — the offline grid lists the gold |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | `logs=6 ok=6 missing-exit=0 bad-exit=0` |
| package gate | `verify-sdk-package.py refinedstorage` | **1/1** (runtime byte-identical, 165 entries, no `Cargo.lock`, 31 tests, 0 warnings, skills PASS) |

### Coverage (parity rule) — this slice's families

| family | state |
|---|---|
| 14 chain items | **tested** — given, crafted and taken; each is an ingredient of the recipe that declares it |
| `machine_casing` block | **tested** — crafted, placed, read back (`refinedstorage:machine_casing`), broken in survival, **drops itself** |
| 15 chain recipes (12 shaped/shapeless + 3 smelting) | **tested** — `recipe_check.py`, output id + count + ingredients consumed; each has a wrong-ingredient control |
| 6 shipped-item recipes | **tested** — `controller` 1, `cable` 12, `disk_drive` 1, `grid` 1, `1k_storage_disk` 1 (shaped and from the housing) |
| the `c:` tags | **tested** — every tag-consuming recipe crafts, and `#c:stones` makes the casing from **cobblestone**, not only from its first entry (stone) |
| unknown ids | **tested** — `give refinedstorage:no_such_item` refused; `setblock … refinedstorage:no_such_block` leaves air |
| the rest of the jar (crafters, exporters/importers, external storage, interfaces, relays, security, fluid storage, the other disk sizes, coloured variants, the eight inert upgrades) | **not ported** — slice-1 gap 2, unchanged (the range and creative range upgrades are slice 6) |

### Deviations and traps

1. **The package declares the `c:` tags it references.** Upstream inherits NeoForge's common
   tag data; a lone package in a game without it would leave the recipes unresolvable. The
   values are the vanilla minimum per tag (`c:glass_blocks` = glass + tinted glass, `c:stones` =
   stone/cobblestone/granite/diorite/andesite/deepslate/cobbled deepslate/tuff/calcite), and
   every file is `"replace": false`, so a pack that also ships them merges additively. A modded
   glass that is not vanilla will not satisfy `c:glass_blocks` unless its own pack adds it.
2. **26.3 loot tables use a singular `condition` on the pool**, not 1.21.1's `conditions` list:
   the first build of the casing's table used the list and the block broke **empty**
   (`drops: {}`), while the port's older tables (already singular) dropped. Fixed; recorded
   here because the same trap will bite the next carried block.
3. **`machine_casing` is carried as a block** (as upstream), not an item: it places, drops
   itself, and is in `#minecraft:mineable/pickaxe`.
4. **Upstream's `c:ingots` contribution is kept as upstream writes it**: both
   `quartz_enriched_iron` and `quartz_enriched_copper` are ingots in that tag.
5. The chain is what makes the five blocks reachable in survival; it does not make the disk's
   contents world-level storage, and it does not add energy — both remain slice-1 gaps.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

Evidence: `ports/_shared/evidence/refinedstorage/` — `plan-slice2-recipes.md`, `sdk-test.log`,
`sdk-clippy.log`, `sdk-check-fits.log`, `sdk-package.log` (older), and
`ports/_shared/evidence/refinedstorage-e2e/` — `e2e-red-recipes-0b14f8a.log` (red),
`e2e-red-recipes-0b14f8a-nodata.log` (the family absent), `e2e-launcher-0a875da.log` (green),
`e2e-launcher-control.log`, `e2e-launcher-selftest.log`, `launcher-build-0a875da.log`.

## Slice 3 — the wireless transmitter, and the rule an add-on relaxes (2026-11-21) — version 0.2.3

Gap line this slice answers — §"Gaps / deviations (slice 1)" item 2, verbatim: *"The rest of the
mod does not exist: … network transmitters/receivers, security manager, **wireless
transmitter/grid**, portable grid, …"*. It is the surface
`ports/interdimensionalwirelesstransmitter` binds to: that mod stopped the same day at
`blocked(no-parent-addon-surface)` because its whole content is **four lines** relaxing this
block's rule (the coordinates never read, so any dimension and any distance), its 49 parent
classes have no counterpart here, and its main recipe's base item (`refinedstorage:wireless_transmitter`)
was unregistered. This slice registers that id and the rule it relaxes.

Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`**, mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, SDK `0a875da`, launcher 0.2.92, game 26.3. Class
`content` / tier **deep**, shape **machines**, unchanged: one block, one block entity and a rule
are below the tier. No new pin API and no third-party mod: every ingredient is vanilla or a chain
item slice 2 already carries.

### What the jar says (the artefact wins over the brief's prose)

| upstream (`refinedstorage2` v2.0.9, MIT) | what this slice carries |
|---|---|
| `WirelessTransmitterBlock` (`AbstractActiveColoredDirectionalBlock`, `DefaultDirectionType.FACE_CLICKED`) | states **`direction` (6) + `active` (2)**; one 4×4×10 nub per face (`box(6,0,6,10,10,10)` for down, rotated for the rest) |
| `WirelessTransmitterConnectionStrategy` | a network node that joins **through its facing side only**: `addOutgoingConnections` → `relative(direction)`; `canAcceptIncomingConnection` → `incomingDirection == myDirection` |
| `WirelessTransmitterNetworkNodeContainer.isValid(PlayerCoordinates)` | dimension equal → `node.isActive()` → `distance(pos, playerPosition) <= getRange()` |
| `DefaultEnergyUsage` | `WIRELESS_TRANSMITTER_BASE_RANGE = 16`, `WIRELESS_TRANSMITTER = 16 FE/t` |
| `data/refinedstorage/recipe/wireless_transmitter.json` | `EPE/EME/EAE`: `E = quartz_enriched_iron`, `P = #c:ender_pearls`, `M = machine_casing`, `A = advanced_processor` |
| `data/refinedstorage/advancement/wireless.json` | a child of `refinedstorage:root`; criteria `wireless_grid` **and** a member of `#refinedstorage:wireless_transmitters` |
| `data/refinedstorage/tags/item/{wireless_transmitters,controllers}.json` | the tag the advancement tests, and the tag its parent's criteria tests |

**The brief's "the transmitter block with its six connection faces (reuse the parent's existing
cable/nub shapes)" is not what the jar has.** The jar's blockstate declares `active` and
`direction` only — no six boolean connection states — and its model is a torch-like cross
(`inactive`/`active`), not the cable's nubs; the connection rule is *one* face, not six. The
artefact wins (AGENTS.md: *"when a brief and the artefact disagree, the artefact wins"*), so the
port carries the jar's shape and its one-face rule. The `shapes` are upstream's own six voxel
boxes, selected by the `facing` property.

### What was implemented

- **runtime.rs** (pure, tests first): `Kind::WirelessTransmitter`; `Face` (offset, opposite,
  property value, from a property value or a unit offset); `Node { kind, face }` and the
  connection rule in `network()` (every block joins on all six sides, a transmitter through its
  facing side only, and only when the neighbour on that side is the one asking);
  `WIRELESS_RANGE = 16`, `distance(from, block)` and `admit(online, same_dimension, distance,
  range)` with `Refusal` naming the reason in upstream's check order (dimension, then activeness,
  then range, boundary inclusive).
- **support.rs** (glue): the transmitter is a block of the mod's one block-entity type; its tick
  writes `active` from its component's online state; `BlockPart::state_for_placement` sets
  `facing` from the clicked face (upstream's `FACE_CLICKED`, and it removes the end-rod template's
  chaining quirk); `network_at`/`network_in` read a level by handle, so the same rule answers in
  another dimension; `/refinedstorage info` names a transmitter and its range;
  `/refinedstorage wireless <pos> <dimension>` asks the admission and prints either
  `accepted (<d> of 16 blocks, online network)` or `refused (<reason>)`.
- **Content** (from the jar, MIT): the block declaration (end-rod template: `facing`, plus our
  `active`, six shapes), blockstate (12 variants), the two models and the item model, the three
  textures, the item, the loot table, the pickaxe tag, the lang keys, the creative tab, the
  recipe, `data/c/tags/item/ender_pearls.json`, the two item tags, and the `root` + `wireless`
  advancements.

### Upstream defects fixed

**`shapes` boxes are block units, not pixels — the port's cable carried pixels.** The block
declaration's `shapes` values are `[min_x, min_y, min_z, max_x, max_y, max_z]` **in block units**
(`horizon/docs/modpacks.md`), while the jar's voxel shapes and the port's cable declaration used
pixels (`[6,6,6,10,10,10]`). Measured: a vanilla outline row is `[[0.0,0.0,0.0,1.0,1.0,1.0]]` or
`[[0.4375,0.8125,0.4375,0.5625,1.0,0.5625]]`, never 6..10. The consequence is not cosmetic: the
cable's selection/collision shape sat **outside** its own cell, so the block could not be hit by
a ray — `ctl break` on any small block failed with *"no face of block … is visible from here:
minecraft:stone at … is in the way"* (measured while writing this slice's loot assertion). Fixed
in both declarations by dividing every box by 16 (`cable` → `0.375..0.625`; the transmitter → its
six nubs); the cable is now hittable and the transmitter breaks and drops itself. Nothing else
read the cable's shape, and no earlier assertion depended on it.

### Deviations (recorded, not defects)

1. **Template `minecraft:end_rod`.** The pin's block declaration has no `direction` placement
   rule of its own, so the template supplies `facing` (six values) and the placement hook sets it
   from the clicked face; the shape is selected by that template property. The jar's `direction`
   property name and its 16 dyed variants are not carried (the port has no colour variants for any
   block).
2. **`root` is carried too, because `wireless` needs a parent.** Upstream's tree has 28
   advancements; this package ships the two that reach the transmitter. `root`'s icon is upstream's
   `creative_controller` (unported) → this package's `controller`, its background is vanilla's
   stone panel instead of the jar's `textures/gui/advancements.png`, and its criteria test this
   package's own `#refinedstorage:controllers` tag.
3. **`wireless`'s icon and criteria are this package's, not the wireless grid's.** The jar's icon is
   `refinedstorage:wireless_grid` (next slice) and its criteria need both a grid and a transmitter;
   as carried, the advancement could never be obtained. It tests
   `#refinedstorage:wireless_transmitters` (which holds the transmitter until the dyed variants
   exist) and draws the transmitter. The tag is load-bearing: it is the reason the criterion fires.
4. **No network energy.** Parent gap 1: the controller consumes and stores nothing, so the
   transmitter's 16 FE/t is not charged. The admission rule is **independent of energy** — the
   node's activeness is the component's controller rule — so the rule is provable without it (this
   is the state `interdimensionalwirelesstransmitter` needs; its own 100 FE/t stays a gap).
5. **`/refinedstorage wireless` is a harness/behaviour addition, like `/refinedstorage info` and
   `/refinedstorage filter`.** Upstream asks that question from the wireless grid item through
   `NetworkItemPlayerValidator`; with no grid in this package, the command is how the *same*
   function is exercised. It takes the transmitter's position and dimension and answers for the
   acting player, so a cross-dimension use is nameable.
6. **The transmitter has no menu.** Upstream opens a 0-container-slot menu that shows the range; the
   pin's mod menus take 1–256 slots (`horizon/docs/mods.md`), so the range is in
   `/refinedstorage info` instead (`Wireless transmitter at x,y,z: range 16 blocks, active`).

### Red proof — the rule as tests, before the glue

Written first against stubs (`links` answered `true` for every pair, `admit` answered `Ok(())`),
then run: **5 failed of 39** (`ports/_shared/evidence/refinedstorage/runtime-red-slice3-transmitter.log`):

```
---- runtime::tests::a_transmitter_joins_the_network_through_its_facing_side_only stdout ----
assertion `left == right` failed: a transmitter that faces nothing is alone
  left: [(WirelessTransmitter, [2, 64, 0]), (Cable, [1, 64, 0]), (Controller, [0, 64, 0])]
 right: [(WirelessTransmitter, [2, 64, 0])]
---- runtime::tests::a_use_in_another_dimension_is_refused_as_such_even_when_offline stdout ----
assertion `left == right` failed: upstream checks the dimension before the node's activeness
  left: Ok(())
 right: Err(DifferentDimension)
---- runtime::tests::an_offline_transmitter_refuses_a_use_within_its_range stdout ----
  left: Ok(())
 right: Err(Offline)
---- runtime::tests::a_use_beyond_the_range_is_refused_naming_the_distance stdout ----
  left: Ok(())
 right: Err(OutOfRange { distance: 16.5, range: 16 })
---- runtime::tests::the_range_boundary_itself_is_accepted stdout ----
assertion failed: admit(true, true, 16.000_1, WIRELESS_RANGE).is_err()
test result: FAILED. 34 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out
```

One test of my own was wrong and the run said so: two transmitters "facing each other" were
written facing away (`facing=east` on the west one), and the fill answered 3 nodes instead of 4 —
the test was rewritten so each faces the other (`a_transmitter_takes_the_side_it_faces_when_the_neighbour_is_the_one_asking`).
Green after the two rules: **39 passed, 0 failed** (`sdk-test.log`).

### Green — the ladder and the in-game leg (one launcher build at `0a875da`)

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `cargo test --offline` | **39 passed**, 0 failed (`sdk-test.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean |
| SDK check | `cargo horizon-mod check` | *"…404 items of the API (… 14 hooks …). Made for 0a875da."* |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical**, **129,923 B**, SHA-256 `a36d3a5d7378cfc3283558fb382daa59e9337c2f939d73be363000e3d8ac02da`, **182 entries**, no `Cargo.lock` |
| package check | `cargo horizon-mod check refinedstorage.hmod` | `0.2.3 (native-mod, format native-1/0a875da…/e59da909…, for 26.3) 129923 bytes` (`sdk-check.log`) |
| fits (pin) | `… fits refinedstorage.hmod --build 0a875da` | *"0a875da made for"* |
| fits (other build) | `… fits refinedstorage.hmod --build 0b14f8a` | *"0b14f8a fits"* (`sdk-check-fits.log`) |
| launcher build | `launcher-game.sh build refinedstorage --rebuild` | exit 0, **`Compiling refinedstorage (1 of 1)`**, `native-0a875da-4f02f2698214` |
| in-game leg | `build-slot.sh run --step e2e -- bash tools/e2e.sh` | **PASS** + **`EXIT:0`** (`e2e-launcher-slice3.log`) |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME …` | **FAIL** + `EXIT:1` — *"the block at 41,64,1 is minecraft:air, expected facing=north"* |
| selftest (flipped range) | `SELFTEST=1 …` | **FAIL** + `EXIT:1` — *"expected … 'accepted (17.0 of 16 blocks', got '… refused (out of range: 17.0 > 16 blocks)'"* |
| selftest (flipped offline) | `SELFTEST=offline …` | **FAIL** + `EXIT:1` — *"the grid's view is [], expected [3 gold]"* |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | `logs=10 ok=10 missing-exit=0 bad-exit=0` |
| package gate | `verify-sdk-package.py refinedstorage` | **1/1** |

The scenario (quoted from `e2e-launcher-slice3.log`):

```
  block(41,64,1): refinedstorage:wireless_transmitter[active=true,facing=north]      (overworld)
  info(41,64,1): … online (3 nodes, 1 controller, 0 drive, 0 grid, 1 transmitter); 0 disks, …
  block(54,64,4): refinedstorage:wireless_transmitter[active=false,facing=east]      (facing away: alone)
  info(54,64,4): … offline (1 nodes, 0 controller, 0 drive, 0 grid, 1 transmitter); …
  block(54,64,4): refinedstorage:wireless_transmitter[active=true,facing=west]       (facing the cable)
  info(54,64,4): … online (3 nodes, 1 controller, 0 drive, 0 grid, 1 transmitter); …
  info(54,64,4): Wireless transmitter at 54,64,4: range 16 blocks, active
  info(53,64,4): Refined Storage network at 53,64,4: … (no range line on a cable)
  /refinedstorage wireless 54 64 4 minecraft:the_end -> … accepted (2.5 of 16 blocks, online network)
  /refinedstorage wireless 54 64 4 minecraft:the_end -> … accepted (16.0 of 16 blocks, online network)
  /refinedstorage wireless 54 64 4 minecraft:the_end -> … refused (out of range: 17.0 > 16 blocks)
  /refinedstorage wireless 54 64 4 minecraft:the_end -> … refused (the network is offline)
  /refinedstorage wireless 52 64 4 minecraft:the_end -> No wireless transmitter there.
  /refinedstorage wireless 41 64 1 minecraft:overworld -> … refused (different dimension)
  /execute in minecraft:overworld run refinedstorage wireless 41 64 1 minecraft:overworld -> … accepted (1.6 of 16 blocks, online network)
  block(55,64,5): refinedstorage:wireless_transmitter[active=false,facing=north]     (placed by the item)
  advancement after the give: {"criteria":{"wireless_transmitter_in_inventory":"…"},"done":true}
  transmitter drops: {"refinedstorage:wireless_transmitter":1}
```

Every assertion has a differential or a counterfactual: the same position offline vs online (the
facing rule), a cable's info without a range line, the boundary at 16.0 vs 17.0, an offline
transmitter, a position with no transmitter, and — for the dimension — **the same command, the
same arguments and the same player coordinates**, refused as `different dimension` from the End and
accepted under `execute in minecraft:overworld` (the overworld network sits at the same
coordinates, force-loaded with `/forceload`, so only the use's dimension differs). The advancement
is proven twice: the game announced `[Wireless]` and the player's saved progress carries
`refinedstorage:wireless: done=true` (and `"absent"` before the item was obtained).

### Harness facts learned here

- `ctl break` aims at the block's **centre** and needs a face of the block's *own* shape to be
  visible; a 4×4×10 nub is grazed by a ray from 1.6 blocks above it (`1.62` eye height), and a
  shape declared in pixels is outside the cell entirely (the defect above). Stand level with the
  block, on the side its shape faces.
- `ctl tp` sets exact doubles; `tp @s <ints>` **centres** x/z on the block (`54` → `54.5`), so a
  distance that must be exactly 16 is only reachable with `ctl tp`.
- `/forceload add` keeps a dimension's chunks loaded while the player is in another one, which is
  what makes a cross-dimension read of a *ticking* network possible.
- A command's own chat lines are best isolated with `events --since <seq>` (`last` from a previous
  `events` call): a count of "the last N lines" matches earlier phases (`botanypots`'s false
  proof), and a player-chat marker never reaches `messages .chat`.
- The player's advancements file is `saves/<world>/players/advancements/<uuid>.json` under the
  game's **launch directory** (the script's `$SCRATCH`), not `HORIZON_HOME`; `ctl save` writes it,
  and the game writes two copies (client and server player).

### Coverage (parity rule) — every family this slice ships

| family | state |
|---|---|
| the `wireless_transmitter` block | **tested** — placed by `setblock` and by the item, read back with its `facing`/`active`, broken in survival, **drops itself** |
| its six shapes and the `facing` placement rule | **tested** — `interact … north` places `facing=north`; the shape is what makes the block breakable (the units defect above) |
| its connection rule (facing side only) | **tested** — the same block at 54,64,4 is `active=false` facing away and `active=true` facing the cable, and the census is 1 node vs 3 |
| the range and same-dimension admission | **tested** — 2.5 / 16.0 accepted, 17.0 refused with the distance, offline refused, no transmitter refused, cross-dimension refused with `execute in` as the differential |
| the `wireless_transmitter` item | **tested** — given, selected, placed against the clicked face; `refinedstorage:no_such_item` still refused by the legacy phase |
| its recipe | **tested** — `recipe_check.py` crafts it in a table (output id + count, ingredients consumed) with a wrong-ingredient control (`#c:ender_pearls` → dirt) |
| the `root` and `wireless` advancements | **tested** — `refinedstorage:wireless` absent before the give, `done=true` after it, and the `[Wireless]` announcement in chat |
| the `wireless_transmitters` / `controllers` item tags | **tested** — the advancement criterion fires through `#refinedstorage:wireless_transmitters`; `root`'s through `#refinedstorage:controllers` |
| its loot table | **tested** — the break above |
| network energy (upstream 16 FE/t) | **not ported** — parent gap 1 (no FE on the network); the rule is energy-independent and proven without it |
| the transmitter's menu/screen (range readout) | **not ported** — the pin has no 0-container-slot mod menu; `/refinedstorage info` prints the range |
| the wireless grid, the portable grid, the dyed transmitters | **not ported** — the next slices (see the report's `## Next slice`) |
| the `interdimensionalwirelesstransmitter` add-on | **not ported here** — its own dispatch; this slice is the surface it binds to (its `isValid` override, its recipe's base item, its `parent: refinedstorage:wireless`) |

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

Evidence: `ports/_shared/evidence/refinedstorage/` — `plan-slice3-transmitter.md`,
`runtime-red-slice3-transmitter.log`, `sdk-test.log`, `sdk-clippy.log`, `sdk-check.log`,
`sdk-check-fits.log`, `refinedstorage.hmod` (the delivered bytes) — and
`ports/_shared/evidence/refinedstorage-e2e/` — `e2e-launcher-slice3.log` (green),
`e2e-launcher-slice3-control.log`, `e2e-launcher-slice3-selftest.log`,
`e2e-launcher-slice3-selftest-offline.log`.

## Slice 4 — the wireless grid, the item that uses the rule (2026-11-21) — version 0.2.4

Gap line this slice answers — §"Gaps / deviations (slice 1)" item 2, as slice 3 left it: *"The rest of
the mod does not exist: … ~~wireless transmitter~~ (**done in slice 3**) — … the wireless grid, portable
grid, …"*, and slice 3's own coverage row *"the wireless grid, the portable grid, the dyed transmitters →
**not ported** — the next slices"*. It is the consumer of the admission slice 3 built: the transmitter
answers *whether* a network-bound item may be used; this item is what asks.

Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`**, mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, SDK `0a875da`, launcher 0.2.92, game 26.3. Class `content`
/ tier **deep**, shape **machines**, unchanged: one item, one item component, one menu surface and one
click rule are below the tier. No new pin API and no third-party mod.

### What the jar says (read from `refinedmods/refinedstorage2` tag v2.0.9, MIT)

The clone (commit `0dec63affe8147702b020843c22748483ef18c01`) was read for behaviour only; it is deleted
after this record. Files read: `common/grid/WirelessGridItem.java`, `WirelessGrid.java`,
`WirelessGridData.java`, `WirelessGridContainerMenu.java`, `screen/AbstractGridScreen.java`,
`support/network/item/{NetworkItemContextImpl,NetworkItemHelperImpl}.java`,
`api/support/network/item/{NetworkItemContext,NetworkItemPlayerValidator}.java`,
`api/network/impl/node/grid/GridOperationsImpl.java`, `api/network/node/grid/{GridExtractMode,GridInsertMode}.java`,
`neoforge/grid/strategy/{ItemGridExtractionStrategy,ItemGridInsertionStrategy}.java`, and the jar's
`data/refinedstorage/recipe/wireless_grid.json`, `tags/item/grids.json`, `advancement/wireless.json`,
`lang/en_us.json`, `textures/item/wireless_grid/{inactive,active}.png`, `models/item/wireless_grid*.json`.

| upstream | what this slice carries |
|---|---|
| `NetworkItemHelperImpl.bind` — a `GlobalPos` (dimension + block) in the `refinedstorage:network_location` component, written by `useOn` on a `NetworkItemTargetBlockEntity`, `SUCCESS` | the same component, read and written through a `Component` impl; the item binds on a wireless transmitter and answers *"Wireless grid bound to x,y,z in dimension."* |
| `NetworkItemContextImpl.resolveNetwork` — the bound dimension's level, the chunk loaded, the block entity a network target, then `NetworkItemPlayerValidator.isValid` over the network's validators | `host.level(<bound id>)` + `host.block_entity(level, pos)` + `node_at`/`network_in`, then slice 3's `runtime::admit(online, same_dimension, distance, range)` |
| `use` always consumes; an invalid network opens an inert, empty grid | a refusal names the reason and opens nothing (recorded deviation 2) |
| `GridOperationsImpl.extract` — amount = `min(total, max_stack)`, `ENTIRE` that, `HALF` half (1 stays 1), `SINGLE` 1 | `runtime::extract_amount` and `runtime::insert_amount`, unit-tested |
| `AbstractGridScreen` — a left click withdraws to the cursor, a right click half, a shift-click to the inventory; a carried stack over the area deposits (`ENTIRE` left, `SINGLE` right); a shift-click in the inventory deposits that stack | `runtime::plan_click` + `runtime::inventory_click`, unit-tested; the menu is a native chest menu, so the mod moves everything |
| `recipe/wireless_grid.json` — `EPE/EGE/EAE`, `E` = `quartz_enriched_iron`, `P` = `#c:ender_pearls`, `G` = `#refinedstorage:grids`, `A` = `advanced_processor` | the same recipe in the pin's recipe shape, with `#refinedstorage:grids` declared (its only member here is `refinedstorage:grid`) |
| `advancement/wireless.json` — one `inventory_changed` criterion needing **both** the wireless grid **and** a member of `#refinedstorage:wireless_transmitters` | restored exactly; this closes slice 3's deviation 3 (which could only test the transmitter) |

### The menu half: probed first, and it reconciles

The pin's `ModContainer` is block-shaped, so a menu opened *from an item* cannot use it. What the pin
does have is `ServerHost::open_menu(uuid, rows, title, items, replace)` — a chest menu opened in place of
the player's, whose slots are `MenuSlotBacking::Native(index)` and whose clicks reach `Hooks::menu_clicked`
with **nothing moved** (`crates/horizon-mod-vocab/src/hooks/notes.rs`, "Server menus"). So the mod owns
every movement: `ItemPlace::MenuSlot` / `MenuCursor` writes in one transaction, `Hooks::menu_closed` to
forget the menu, and a `server_tick` refresh that rewrites the view with `menu_set_items` when it changed.

The probe was the first in-game leg of this slice, and it found two defects of the glue (both fixed, both
invisible to `check`/`fits`/unit tests):

1. a shift-click on a **player inventory** slot deposited the *cursor* instead of that slot's stack (the
   plan's `Deposit` covers both, and the source place is the clicked slot for one and the cursor for the
   other);
2. a shift-click on a **view** slot withdrew nothing: `menu_clicked` swaps no acting player in, so
   `host.players().acting_player()` is `None` there and the inventory insert found no player. The click
   carries its player (`event.uuid`).

### Upstream defects fixed

**The `wireless` advancement's criteria (slice 3's deviation 3, now closed).** Slice 3 carried the
advancement while the wireless grid did not exist, so it tested only `#refinedstorage:wireless_transmitters`
and drew the transmitter. Upstream's `advancement/wireless.json` needs **both** the grid and a transmitter
in one `inventory_changed` criterion; the port now carries that, and the in-game leg proves both halves:
a transmitter alone leaves it unobtained, the grid and a transmitter together grant it (`done=true` and the
`[Wireless]` announcement). A user now gets the advancement where upstream gives it, not on picking up a
transmitter.

**The recipe driver silently skipped every shaped tag counterfactual** (`tools/recipe_check.py`, a harness
defect, not the mod's): it looked for the jar's `{"tag": …}` ingredient object, while this package's
recipes spell a tag as a `"#tag"` string, so no shaped recipe with a tag ingredient had a wrong-ingredient
control at all (slice 2's ten controls were the shapeless ones and `#c:stones`). The driver now reads the
tag from the resolved ingredient, and every shaped tag recipe has its control — 22 recipes, 14 of them with
a tag control. Its tag loader also reads every namespace under `data/*/tags/item`, so a recipe testing
this package's own tag resolves.

### Deviations (recorded, not defects)

1. **The item is one model, the inactive texture.** Upstream's item model switches on the
   `refinedstorage:network_bound_active` item-model property; the pin's `register_item_model_property` is
   not a stamped hook, so the port carries `textures/item/wireless_grid/inactive.png` as the item's model and
   leaves `active.png` out rather than shipping an unreachable texture.
2. **A refused use opens nothing; upstream opens an inert, empty grid.** `WirelessGrid.isGridActive()` is
   false out of range, offline or unbound, and upstream still opens the screen. The port tells the player
   the parsed reason instead (`Wireless grid 50,64,8 in minecraft:the_end: refused (out of range: 17.0 > 16 blocks).`)
   and opens no menu — the same sentence shape as `/refinedstorage wireless`, so the rule has one voice.
   A use on an unbound grid says so; a use whose transmitter is gone says so.
3. **The binding is read back with a command, not a tooltip.** Upstream shows `Bound to x, y, z.` in the
   item's tooltip (`item.refinedstorage.network_item.bound_to`); a mod item's tooltip is the client
   `tooltip` ItemPart hook, which this port does not subscribe, so `/refinedstorage bound` prints the
   component's dimension and position (a harness/behaviour addition, like `/refinedstorage info`).
4. **Click modes.** Pickup (0) and quick-move (1) are carried with upstream's meanings; quick-craft,
   clone, throw, quick-craft-all and pickup-all (2–6) move nothing (they are not modelled). Drags and
   scrolling are not modelled either.
5. **No energy on the item.** Upstream's wireless grid drains FE per open; the parent has no FE at all
   (gap 1), and the admission rule is energy-independent, as slice 3 proved.
6. **The menu is a chest menu.** 54 slots, no scrolling, no sorting buttons, no search box, no side
   buttons, and the plain panel rather than upstream's grid texture — the same limits the block grid
   records (gap 5). The shift-click path is the deposit/withdraw route the leg proves.
7. **The view reads the disks live**, not through the tick cache: the transmitter can be in a level that is
   not in place (whose drives do not tick), and the item's view must not be a tick behind the disk it
   withdraws from. Writes go straight into the drive's disk item in one transaction, and the drive's own
   tick re-reads the item.

### Gap this slice leaves (recorded, not fixed)

**The promise table is keyed by dimension, not by network.** `view_with` is fed every other view's promise
in the same dimension, so two *different* networks in one dimension holding the same resource hide it from
each other (never a duplication, always an over-exclusion). The key should be the view's network — its
controller position (upstream's root) — not the dimension; that is a follow-up slice, and the block grid
has had the same coarseness since slice 1's review repair #4. The in-game leg is unaffected (its networks
hold different resources).

### Red proof — the rules as tests, before the glue

Written first against stubs (`describes` true, `extract_amount` total, `insert_amount` 1, `plan_click`
and `inventory_click` `Nothing`), then run: **14 failed of 54**
(`ports/_shared/evidence/refinedstorage/runtime-red-slice4-wireless-grid.log`):

```
---- runtime::tests::a_right_click_on_a_view_slot_withdraws_half stdout ----
assertion `left == right` failed
  left: Nothing
 right: Withdraw { mode: Half, to: Cursor }
---- runtime::tests::a_single_extraction_takes_one stdout ----
assertion `left == right` failed
  left: 5
 right: 1
---- runtime::tests::an_entire_extraction_is_capped_by_the_stack_size stdout ----
assertion `left == right` failed
  left: 200
 right: 64
---- runtime::tests::a_shift_click_in_the_inventory_deposits_that_stack stdout ----
assertion `left == right` failed
  left: Nothing
 right: Deposit { mode: Entire }
---- runtime::tests::an_plain_inventory_click_is_the_cursor_rules stdout ----
assertion `left == right` failed
  left: Nothing
 right: Inventory(Take { half: false })
test result: FAILED. 40 passed; 14 failed; 0 ignored; 0 measured; 0 filtered out
```

Green after the rules: **54 passed, 0 failed** (`sdk-test.log`).

### Green — the ladder and the in-game leg (one launcher build at `0a875da`)

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `cargo test --offline` | **54 passed**, 0 failed (`sdk-test.log`, package sha printed) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean |
| SDK check | `cargo horizon-mod check` | *"…535 items of the API (… 19 hooks …). Made for 0a875da."* (`sdk-check.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical**, **146,938 B**, SHA-256 `31aa1a0d2c28623a66e6dbb30612798cf8629d7069b78f55cf8d3341543067ae`, **189 entries**, no `Cargo.lock` |
| package check | `cargo horizon-mod check refinedstorage.hmod` | `0.2.4 (native-mod, format native-1/0a875da…/e59da909…, for 26.3) 146938 bytes` |
| fits (pin) | `… fits refinedstorage.hmod --build 0a875da` | *"0a875da made for"* |
| fits (other build) | `… fits refinedstorage.hmod --build 0b14f8a` | *"0b14f8a fits"* (`sdk-check-fits-0.2.4.log`) |
| launcher build | `launcher-game.sh build refinedstorage --rebuild` | exit 0, **`Compiling refinedstorage (1 of 1)`**, `native-0a875da-dd7dca8f05e8` (`launcher-build-0.2.4.log`) |
| in-game leg | `build-slot.sh run --step e2e -- bash tools/e2e.sh` | **PASS** + **`EXIT:0`** (`e2e-launcher-slice4.log`) |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME …` | **FAIL** + `EXIT:1` — *"the block at 41,64,1 is minecraft:air, expected facing=north"* |
| selftest (flipped grid view) | `SELFTEST=grid …` | **FAIL** + `EXIT:1` — *"the grid's view is [], expected [{"count":5,"item":"minecraft:diamond","slot":300}]"* |
| selftest (flipped command range) | `SELFTEST=1 …` | **FAIL** + `EXIT:1` — *"expected … 'accepted (17.0 of 16 blocks', got … 'refused (out of range: 17.0 > 16 blocks)'"* |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | `logs=14 ok=14 missing-exit=0 bad-exit=0` |
| package gate | `verify-sdk-package.py refinedstorage` | **1/1** |

The scenario (quoted from `e2e-launcher-slice4.log`; the network is 48..50,64,8 with the drive at
49,64,9, inside the arena floor):

```
  block(50,64,8): refinedstorage:wireless_transmitter[active=true,facing=west]
  info(49,64,9): Refined Storage network at 49,64,9: online (4 nodes, 1 controller, 1 drive, 0 grid, 1 transmitter); 1 disks, 0 items in 0 resources
  /refinedstorage bound -> The wireless grid is not bound.
  /refinedstorage bound -> Wireless grid bound to 50,64,8 in minecraft:the_end.
  grid view: []                                                    (the view opens, empty)
  grid view: [{"count":5,"item":"minecraft:diamond","slot":300}]   (after a shift-click on the inventory slot)
  info(49,64,9): … 1 disks, 5 items in 1 resources
  grid view: []                                                    (after a shift-click on the view)
  info(49,64,9): … 1 disks, 0 items in 0 resources
  use 17 blocks out: Wireless grid 50,64,8 in minecraft:the_end: refused (out of range: 17.0 > 16 blocks).
  use of the End transmitter from the overworld: Wireless grid 50,64,8 in minecraft:the_end: refused (different dimension).
  advancement after the grid and a transmitter: {"criteria":{"wireless_grid_in_inventory":"…"},"done":true}
```

Every assertion has its differential or counterfactual: the unbound readout before the bind; an empty
view before the deposit and after the withdrawal (the same slots, both directions); 17.0 refused vs 16.0
accepted (the boundary); the same item, the same binding and the same player refused from the overworld
and accepted in the End (the dimension); the advancement absent after a transmitter alone and present
after the grid with one; the `SELFTEST=grid` run inverts the view assertion and exits 1.

### Harness facts learned here

- **A use-item click hits whatever the crosshair is on.** After `ctl interact`/`look-at`, the crosshair is
  on a block, so `ctl use-item` runs the *block* path and the grid binds again instead of opening. Look at
  the sky (`ctl look 0 -90`) before an air use.
- **The transmitter's nub is one face's shape, not six.** With `facing=north` the declared shape is the
  north nub (`[0.375, 0.375, 0.0, 0.625, 0.625, 0.625]`), so a ray from the south never reaches it and
  `ctl interact 41 64 1 south` answers *"no face … is visible: minecraft:stone at [41,63,1] is in the way"*.
  Bind by aiming the crosshair with `ctl look-at block X Y Z <face>` from the side the nub is on.
- **Keep every teleport inside the arena floor.** The End arena is stone at y=63 over x 32..56, z −8..24;
  a `ctl tp` outside it drops the player into the void, and a distance assertion then measures from y≈59
  (17.0 became 17.7 before the coordinates were moved onto the floor).
- `ctl click-slot N shift` on a *native* menu sends click mode 1 (quick move) for the player's slots and
  the view's alike; the mod's `menu_clicked` sees the true menu index, so `slot_backing` — not a slot-number
  threshold — is what tells the view from the inventory.

### Coverage (parity rule) — every family this slice ships

| family | state |
|---|---|
| the `wireless_grid` item | **tested** — given, selected, read back (`/refinedstorage bound`), used; `refinedstorage:no_such_item` still refused by the legacy phase |
| its `refinedstorage:network_location` component | **tested** — unbound before the bind, `50,64,8 in minecraft:the_end` after it; an unknown id refused; the unit test pins the round trip and the dimension's part in it |
| binding on a transmitter | **tested** — a use on the transmitter writes it, and the transmitter opens no menu of its own |
| the admission gate (dimension, online, range) | **tested** — 2.5 opens, 17.0 refused naming the distance, 16.0 opens, another dimension refused while the binding's dimension opens |
| the 9×6 view from the item | **tested** — `ctl screen` says `container` with 54 slots, empty on an empty network, and it lists what the network holds |
| deposit through the menu | **tested** — a shift-click on an inventory slot puts 5 diamonds on the network (the view and `/refinedstorage info` agree, the inventory empties) |
| withdrawal through the menu | **tested** — a shift-click on the view returns them (the view empties, the inventory gains 5, the disk holds 0) |
| its recipe | **tested** — `recipe_check.py` crafts it (output id + count, ingredients consumed) with a wrong-ingredient control on `#refinedstorage:grids` |
| the `#refinedstorage:grids` tag | **tested** — the recipe's control replaces its member with dirt and crafts nothing |
| the `refinedstorage:wireless` advancement | **tested** — absent after a transmitter alone, `done=true` after the grid with a transmitter, with the `[Wireless]` announcement |
| its textures/model | **carried** — one model (the inactive texture); the active variant needs an item-model property hook the pin does not stamp (deviation 1) |
| cursor pick-up / place / merge / swap in the menu | **unit-tested, not in game** — `inventory_click`'s table; the leg's two paths are shift-clicks |
| drag, throw, quick-craft, scrolling, search, sorting | **not ported** — deviation 4 and gap 6 |
| network energy (upstream's wireless grid drains FE) | **not ported** — parent gap 1 |
| the portable grid, the crafting/pattern grids, the dyed grids | **not ported** — the next slices |

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

Evidence: `ports/_shared/evidence/refinedstorage/` — `plan-slice4-wireless-grid.md`,
`runtime-red-slice4-wireless-grid.log`, `sdk-test.log`, `sdk-clippy.log`, `sdk-check.log`,
`sdk-check-fits-0.2.4.log`, `refinedstorage.hmod` (the delivered bytes) — and
`ports/_shared/evidence/refinedstorage-e2e/` — `e2e-launcher-slice4.log` (green),
`e2e-launcher-slice4-control.log`, `e2e-launcher-slice4-selftest.log`,
`e2e-launcher-slice4-selftest-grid.log`, and `launcher-build-0.2.4.log` (`Compiling refinedstorage
(1 of 1)`, `native-0a875da-dd7dca8f05e8` — the build all four legs ran on).

## Slice 5 — the promise table keyed by network: two networks in one dimension (2026-11-21) — version 0.2.5

Defect this slice fixes, quoted from slice 4's own record: *"**The promise table is keyed by
dimension, not by network.** … two *different* networks in one dimension holding the same resource
hide it from each other (never a duplication, always an over-exclusion) … the key should be the
view's network — its controller position (upstream's root) … the block grid has had the same
coarseness since slice 1's review repair #4."*

Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`**, mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, SDK `0a875da`, launcher 0.2.92, game 26.3. Class
`content` / tier **deep**, shape **machines**, unchanged. No new pin API, no third-party mod, no
content change: this slice is the keying rule, its tests and its in-game differential.

### The rule, and where it lives now

The rule was inline in two places in `support.rs` (the block grid's `grid_tick` filter and
`promises_elsewhere` for the wireless grid), both keyed `(dimension, Place)`. It is now one pure,
unit-tested type in `runtime.rs`:

- `runtime::Place` — where a view sits (`Grid(pos)` or `Wireless { player, window }`), moved out of
  `support.rs` so the table's identity is part of the pure model.
- `runtime::ViewKey { dimension, network, place }` — `network` is
  `runtime::Network::anchor()`, the network's **root**: the controller's position (upstream's
  `AbstractNetworkNode.getRoot`), the smallest one when a flood fill found more than one so the
  answer does not depend on where the fill started; `None` for a controller-less network, whose view
  is empty and therefore promises nothing.
- `runtime::Promises` — `record`, `promised_elsewhere`, `retain`, `release`. `promised_elsewhere`
  sets aside only the promises of views with **the same dimension, the same network and another
  place**, so two networks in one dimension can no longer hide each other's stock.
- `Core::reserved` became `Core::promises: Promises`; both grids build one `ViewKey` and call the
  same function (`grid_tick`, `refresh_menu`), so the block grid and the wireless grid share one
  code path.

### Port defect fixed (not upstream's — this one put the port *out* of step with upstream)

Upstream binds a network-bound item to its root and every grid reads its own network's storage, so
two networks in one dimension never saw each other. The port's promise table (added in the 2026-11-08
review to stop two grids of *one* network offering the same items twice) keyed only by dimension, so
a second network's grid listed `[]` while its own disk held the 3 items. A player with two networks in
one base lost sight of one network's stock on both grids (an over-exclusion, never a duplication: the
`view_with` cap could only ever subtract). The port now keys by the root, which is upstream's
behaviour; nothing else about the promise changes (same-network views still never overlap, and a
broken grid still releases its promise).

### Red proof — the rule as a unit test, before the keying change

The rule was first moved into `runtime.rs` **preserving today's keying** (a pure refactor), and the
two-network test was written against it. `cargo test --offline`:
**55 passed, 1 failed** (`ports/_shared/evidence/refinedstorage/runtime-red-slice5-promises.log`):

```
---- runtime::tests::two_networks_in_one_dimension_do_not_hide_each_others_stock stdout ----

thread '…' panicked at src/runtime.rs:1746:9:
assertion `left == right` failed: a grid of another network in the same dimension shows its own 3 gold
  left: [("minecraft:gold_ingot", 3)]
 right: []
test result: FAILED. 55 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

Green after the key gained the network: **57 passed, 0 failed** (`sdk-test.log`), the two new tests
being the rule above plus `a_networks_root_is_its_controller_wherever_the_fill_started` (the same
three-block network filled from either end has one root; a lone cable has none).

### Red proof — in game, against the previous package (0.2.4 at `31aa1a0d…`)

The new phase was run first against **0.2.4** (the export the slice-4 build left,
`native-0a875da-dd7dca8f05e8`): network A (40..43,64,0) holds 3 gold; network B (48..50,64,0) is
given a drive at 51,64,0, a disk and the same 3 gold, deposited through B's own grid. The disk says
it has them; the grid does not show them (`ports/_shared/evidence/refinedstorage-e2e/e2e-slice5-red-0.2.4.log`):

```
  info(51,64,0): Refined Storage network at 51,64,0: online (4 nodes, 1 controller, 1 drive, 1 grid); 1 disks, 3 items in 1 resources
FAIL: the grid's view is [], expected [{"count":3,"item":"minecraft:gold_ingot","slot":300}]
EXIT:1
```

That is the reported symptom exactly: B's own stock hidden by A's promise of 3 gold.

### Green — the ladder and the in-game leg (one launcher build at `0a875da`)

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `cargo test --offline` | **57 passed**, 0 failed (`sdk-test.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean (`FMT_EXIT:0`) |
| SDK check | `cargo horizon-mod check` | *"…532 items of the API (… 19 hooks …). Made for 0a875da."* (`sdk-check.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical** to the previous build (`cmp`), **148,725 B**, SHA-256 `0a5d894c67cec3483b3ec6955f4ed2d42eba150c5bbdf365a20360d67e8547fd`, **189 entries**, no `Cargo.lock` (`sdk-build-0.2.5.log`) |
| package check | `cargo horizon-mod check refinedstorage.hmod` | `0.2.5 (native-mod, format native-1/0a875da…/e59da909…, for 26.3) 148725 bytes` |
| fits (pin) | `… fits refinedstorage.hmod --build 0a875da` | *"0a875da made for"* (`FITS_PIN_EXIT:0`) |
| fits (other build) | `… fits refinedstorage.hmod --build 0b14f8a` | *"0b14f8a fits"* (`FITS_OTHER_EXIT:0`) |
| launcher build | `launcher-game.sh build refinedstorage --rebuild` | exit 0, **`Compiling refinedstorage (1 of 1)`**, `native-0a875da-174d06351fe0` (`launcher-build-0.2.5.log`) |
| in-game leg | `launcher-game.sh run refinedstorage` | **PASS** + **`EXIT:0`** (`e2e-slice5-green.log`) |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME …` | **FAIL** + `EXIT:1` — *"the block at 41,64,1 is minecraft:air, expected facing=north"* |
| selftest (flipped two-network rule) | `SELFTEST=networks …` | **FAIL** + `EXIT:1` — *"the grid's view is [{count 3 gold}], expected []"* |
| selftest (flipped grid view) | `SELFTEST=grid …` | **FAIL** + `EXIT:1` — *"the grid's view is [], expected [{"count":5,"item":"minecraft:diamond","slot":300}]"* |
| selftest (flipped command range) | `SELFTEST=1 …` | **FAIL** + `EXIT:1` — *"expected … 'accepted (17.0 of 16 blocks', got … 'refused (out of range: 17.0 > 16 blocks)'"* |
| selftest (flipped offline view) | `SELFTEST=offline …` | **FAIL** + `EXIT:1` — *"the grid's view is [], expected [{count 3 gold}]"* |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | `logs=21 ok=21 missing-exit=0 bad-exit=0` |
| package gate | `verify-sdk-package.py refinedstorage` | **1/1** |

Both `fits` runs used the delivered bytes (`sdk-fits-0.2.5.log`, first lines: port root and evidence
copy both `0a5d894c…`).

The two-network phase, quoted from `e2e-slice5-green.log` (A's grid at 43,64,0 and B's at 50,64,0,
both promises live):

```
  info(51,64,0): … online (4 nodes, 1 controller, 1 drive, 1 grid); 1 disks, 3 items in 1 resources
  grid view: [{"count":3,"item":"minecraft:gold_ingot","slot":300}]   (B's grid, A holding 3 too)
  grid view: [{"count":3,"item":"minecraft:gold_ingot","slot":300}]   (A's grid, B's promise live)
  grid view: [{"count":3,"item":"minecraft:gold_ingot","slot":300}]   (B's grid again)
  info(43,64,0): … 1 disks, 3 items in 1 resources
  info(51,64,0): … 1 disks, 3 items in 1 resources
```

The differential is the phase itself: the same two networks, the same 3 gold each, on the **previous
package** one grid answered `[]` (red log above) and on this one both answer 3. The counterfactual is
`SELFTEST=networks`, which asserts the old behaviour and fails on the fix — so the assertion is not
vacuous. The same-network rule it must not break is the pre-existing phase *"two grids on one
network never offer the same items twice"*, green in the same log.

### Coverage — what this slice changes

| family | state |
|---|---|
| the promise table's key | **tested** — unit: `two_networks_in_one_dimension_do_not_hide_each_others_stock`, `two_grids_of_one_network_never_show_the_same_items`; in game: the two-network phase (two grids, both 3) |
| the network's root | **tested** — unit: `a_networks_root_is_its_controller_wherever_the_fill_started`; in game: every `info` line names the network the grid at that position resolved to |
| two grids of one network never overlap | **tested** — the slice-1 phase, still green |
| a wireless grid of another network | **not in this leg** — the wireless grid's key is the same `Promises` call (`refresh_menu`), unit-covered by the same rule; its in-game leg still binds one network only (slice 4) |
| everything slice 1–4 shipped | **unchanged** — same asserts, same logs, all green |

### Harness facts learned here

- **A promise outlives the menu.** A block grid records what it shows on every tick, whether or not a
  player has its menu open, so the differential needs the *other* grid to have promised first — this
  phase opens A's grid (asserting 3), then B's, and reads each again after both promises are live.
- **A drive joins through any side** (only the transmitter faces one side), so a second network's
  drive can hang off its grid at 51,64,0 with no extra cable.
- `inv_slot` + `ctl click-slot N shift` deposits a whole stack through *any* grid's menu; the same
  two lines prove network B's own stock arrived before the view is read.

Skills: superpowers/using-superpowers,systematic-debugging,test-driven-development,verification-before-completion; BeautifulRust/rust-best-practices

Evidence: `ports/_shared/evidence/refinedstorage/` — `runtime-red-slice5-promises.log`,
`sdk-test.log`, `sdk-clippy.log`, `sdk-check.log`, `sdk-build-0.2.5.log`,
`sdk-check-fits-0.2.5.log`, `sdk-fits-0.2.5.log`, `refinedstorage.hmod` (the delivered bytes) — and
`ports/_shared/evidence/refinedstorage-e2e/` — `e2e-slice5-green.log` (green),
`e2e-slice5-red-0.2.4.log` (the previous package, red), `e2e-slice5-control.log`,
`e2e-slice5-selftest.log`, `e2e-slice5-selftest-grid.log`, `e2e-slice5-selftest-offline.log`,
`e2e-slice5-selftest-networks.log`, and `launcher-build-0.2.5.log` (`Compiling refinedstorage
(1 of 1)`, `native-0a875da-174d06351fe0` — the build every leg ran on).

## Slice 6 — the upgrade items, and the one that is not inert (2026-11-21) — version 0.2.6

Gap line this slice answers — §"Gaps / deviations (slice 1)" item 2, as slices 2–5 left it: *"The rest
of the mod does not exist: … **upgrades**, … the config file …"*, and slice 2's coverage row *"the
rest of the jar (…, coloured variants, **upgrades**) → not ported"*. It is also the family the slice-4
report named as the next slice, with the selection rule the `mifa` dispatch had just cost the campaign:
of upstream's eleven upgrade items, only the one whose consumer exists in this port is non-inert.

Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`**, mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, SDK `0a875da`, launcher 0.2.92, game 26.3. Class
`content` / tier **deep**, shape **machines**, unchanged: three items, a block container, a content
menu type, a menu Part and one pure range rule are below the tier. No new pin API and no third-party
mod: every ingredient is vanilla or a chain item slice 2 already carries.

### The jar's eleven upgrade items, and which are non-inert here

Read from `refinedmods/refinedstorage2` v2.0.9 (MIT), commit
`0dec63affe8147702b020843c22748483ef18c01` — the clone was read for behaviour only and is deleted
after this record. Files read: `upgrade/{UpgradeContainer,UpgradeDestinations,UpgradeRegistryImpl,
RangeUpgradeItem,SimpleUpgradeItem}.java`, `networking/{Base,Composite,RangeUpgrade,CreativeRangeUpgrade}
WirelessTransmitterRangeModifier.java`, `networking/{WirelessTransmitterBlockEntity,
WirelessTransmitterNetworkNodeContainer,WirelessTransmitterContainerMenu}.java`,
`common/content/DefaultEnergyUsage.java`, the `AbstractModInitializer` upgrade registrations, and the
jar's `recipe/*upgrade.json`, `models/item/*upgrade.json`, `textures/item/*upgrade.png`,
`lang/en_us.json`.

| item | upstream registers it for | this port has the surface? |
|---|---|---|
| `upgrade` | a plain `Item`, the base ingredient of the upgrade recipes | **yes** — `range_upgrade`'s recipe consumes it |
| `range_upgrade` | `WIRELESS_TRANSMITTER` (`maxAmount` 4) | **yes** — the transmitter ships (slice 3) |
| `creative_range_upgrade` | `WIRELESS_TRANSMITTER` (`maxAmount` 1) | **yes** — creative-only, no recipe |
| `speed_upgrade` | importer, exporter, destructor, constructor, disk interface, autocrafter | **no** |
| `stack_upgrade` | importer, exporter, constructor, disk interface | **no** |
| `regulator_upgrade` | importer, exporter | **no** |
| `autocrafting_upgrade` | importer, constructor, interface | **no** |
| `silk_touch_upgrade` | destructor | **no** |
| `fortune_1/2/3_upgrade` | destructor | **no** |

The eight inert ones are **not shipped** (the `mifa` rule: an item no shipped block reads is not a
port). Their `blocked(<symbol>)` reasons are in gap 2 above. Their recipes would also be
unresolvable: `silk_touch` and `fortune_1..3` are `refinedstorage:upgrade_with_enchanted_book`
recipes, and the pin has **no content-side recipe-type registry** (custom recipe types are planned for
machines in `horizon/docs/modpacks.md` §"What packs need"), so those four recipes stay
`blocked(no-recipe-type: refinedstorage:upgrade_with_enchanted_book)` even if the destructor existed.

### The rule (upstream, read rather than guessed)

`WirelessTransmitterBlockEntity.getRange()` calls
`CompositeWirelessTransmitterRangeModifier.modifyRange(upgradeContainer, 0)`:

1. `BaseWirelessTransmitterRangeModifier` → `baseRange` (`WIRELESS_TRANSMITTER_BASE_RANGE` = 16);
2. `RangeUpgradeWirelessTransmitterRangeModifier` → `range + amount(range_upgrade) *
   rangeUpgradeRange`, and `DefaultEnergyUsage.RANGE_UPGRADE_RANGE` = **8**;
3. `CreativeRangeUpgradeWirelessTransmitterRangeModifier` (priority `Integer.MAX_VALUE`) →
   `Integer.MAX_VALUE` when the creative upgrade is present.

The container is `UpgradeContainer`'s default size **4**, and its `canPlaceItem` refuses a placement
past the registration's `maxAmount` (4 range, 1 creative). So the port's rule is
`upgraded_range(range_upgrades, creative)` = `16 + 8 * min(4, count)`, or `i32::MAX` with a creative
upgrade; `admit` keeps upstream's check order (dimension, online, range), so the creative upgrade
never crosses a dimension.

### What was implemented

- **runtime.rs** (tests first): `RANGE_PER_UPGRADE` (8), `MAX_RANGE_UPGRADES` (4), `CREATIVE_RANGE`
  (`i32::MAX`), `upgraded_range(count, creative)` and `range_label(range)` ("24 blocks"/"unlimited").
- **support.rs**: the transmitter's four upgrade slots (`Registrar::block_container`); a content menu
  type `refinedstorage:wireless_transmitter` (`data/refinedstorage/horizon/menu/`, API 85,
  `{"slots": 4}`) with a `Screen` layout and a `MenuPart`; `range_at(host, level, pos)` reading the
  four slots; the three range readers (the wireless grid's `use_item`, `/refinedstorage wireless`,
  `/refinedstorage info`) now call it.
- **The menu Part's `menu_slot_rules`** (API 130) is upstream's `canPlaceItem`/`getMaxStackSize`:
  only a range or creative range upgrade, one per slot, at most four range and one creative. It is
  scoped to `MenuContainer::Main`, so the player inventory keeps vanilla's rules — a refusal on the
  menu's slot cannot cancel a deposit from the player's side.
- **The binding moved to `right_click_block`.** The transmitter's menu needs the block's own use to
  open, which the game only posts for a container *class*, so its template became
  `minecraft:barrel` (as the drive's already was) and the blockstate gained `open=false|true` (24
  variants). That changed the order of a click: the block's `use_item_on` now runs **before** the
  item's `use_on`, so a click with a wireless grid opened the menu instead of binding. The binding is
  now done in `right_click_block` (`Flow::Cancel`, the same shape as a disk onto a drive) and the
  item's `use_on` calls the same `bind_grid` helper for the paths that do not run the hook.
- **Content** (from the jar, MIT): the `upgrade`, `range_upgrade` and `creative_range_upgrade` items
  (paper template, own textures/models/item definitions/lang, the creative tab), the `upgrade` and
  `range_upgrade` recipes in the pin's `"id"`/`"#tag"` shape, and the transmitter's
  `open` blockstate variants.

### Upstream defects fixed

None found in the upgrade items themselves. One **port** defect was found and fixed by the slice:
the transmitter's block could not open its own menu at all while its template was `minecraft:end_rod`
(the game posts `PlayerEffect::OpenMenu` only for a container class,
`horizon-world/src/level/interact.rs:414-437`), so the four upgrade slots would have been reachable
only through `ctl container`. The template change above fixes it; the nub stays the block's shape and
collision through `shapes`, and the barrel's `open` state is carried.

### Red proof — the rules as tests, before the glue

`upgraded_range` first answered the base range and `range_label` always "N blocks"; run:
**5 failed of 64** (`ports/_shared/evidence/refinedstorage/runtime-red-slice6-upgrades.log`):

```
---- runtime::tests::each_range_upgrade_adds_eight_blocks stdout ----
assertion `left == right` failed
  left: 16
 right: 24
---- runtime::tests::a_raised_range_accepts_what_the_base_range_refused stdout ----
assertion `left == right` failed
  left: Err(OutOfRange { distance: 17.0, range: 16 })
 right: Ok(())
---- runtime::tests::a_creative_range_upgrade_reaches_any_finite_distance stdout ----
assertion `left == right` failed
  left: 16
 right: 2147483647
test result: FAILED. 59 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out
```

Green after the rule: **64 passed, 0 failed** (`sdk-test-0.2.6.log`). The five new tests are
`a_transmitter_without_upgrades_keeps_the_base_range`, `each_range_upgrade_adds_eight_blocks`,
`a_fifth_range_upgrade_is_not_counted`, `a_creative_range_upgrade_reaches_any_finite_distance`,
`a_creative_range_upgrade_still_refuses_another_dimension`,
`a_raised_range_accepts_what_the_base_range_refused` and
`a_range_is_named_in_blocks_unless_it_is_the_creative_one` — seven, of which five were red.

### Green — the ladder and the in-game leg (one launcher build at `0a875da`)

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `cargo test --offline` | **64 passed**, 0 failed (`sdk-test-0.2.6.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy-0.2.6.log`) |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean (`sdk-fmt-0.2.6.log`, `FMT_EXIT:0`) |
| SDK check | `cargo horizon-mod check` | *"…561 items of the API (… 20 hooks …). Made for 0a875da."* (`sdk-check-0.2.6.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical** (`cmp`), **158,835 B**, SHA-256 `5aa7faa89f85979a6347dc23b5982f9b993c895f0e3f8efdda34c8adab729743`, **204 entries**, no `Cargo.lock` (`sdk-build-0.2.6-{1,2}.log`) |
| package check | `cargo horizon-mod check refinedstorage.hmod` | `0.2.6 (native-mod, format native-1/0a875da…/e59da909…, for 26.3) 158835 bytes` |
| fits (pin) | `… fits refinedstorage.hmod --build 0a875da` | *"0a875da made for"*, `FITS_PIN_EXIT:0` |
| fits (other build) | `… fits refinedstorage.hmod --build 0b14f8a` | *"0b14f8a fits"*, `FITS_OTHER_EXIT:0` (`sdk-fits-0.2.6.log`, sha `5aa7faa8…` printed before each run) — the run first answered the GitHub releases list **403** (the unauthenticated rate limit, exhausted by the box's parallel legs) and was repeated after the reset; `8f83e5c` (a later cached nightly) also answers *"8f83e5c fits"* |
| launcher build | `launcher-game.sh build refinedstorage --rebuild` | exit 0, **`Compiling refinedstorage (1 of 1)`**, `native-0a875da-3f241726424e` (`launcher-build-0.2.6.log`) |
| in-game leg | `build-slot.sh run --step e2e -- bash tools/e2e.sh` | **PASS** + **`EXIT:0`** (`e2e-slice6-green.log`) |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME …` | **FAIL** + `EXIT:1` — *"the block at 41,64,1 is minecraft:air, expected facing=north"* |
| selftest (flipped command range) | `SELFTEST=1 …` | **FAIL** + `EXIT:1` — *"expected … 'accepted (17.0 of 16 blocks', got … 'refused (out of range: 17.0 > 16 blocks)'"* |
| selftest (flipped upgraded range) | `SELFTEST=range …` | **FAIL** + `EXIT:1` — *"expected … 'refused (out of range: 17.0 > 16 blocks', got … 'accepted (17.0 of 24 blocks, online network)'"* |
| selftest (flipped creative range) | `SELFTEST=creative …` | **FAIL** + `EXIT:1` — *"expected the transmitter's range line to say 'range 48 blocks, active', got '… range unlimited, active'"* |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | green |
| package gate | `verify-sdk-package.py refinedstorage` | **1/1** |

The scenario (quoted from `e2e-slice6-green.log`; the transmitter at 50,64,8 faces its cable west,
inside the arena floor):

```
  use 17 blocks out: Wireless grid 50,64,8 in minecraft:the_end: refused (out of range: 17.0 > 16 blocks).
  transmitter slots: [{"count":1,"item":"refinedstorage:range_upgrade"}]
  info(50,64,8) range: Wireless transmitter at 50,64,8: range 24 blocks, active
  /refinedstorage wireless 50 64 8 minecraft:the_end -> Wireless access at 50,64,8: accepted (17.0 of 24 blocks, online network)
  use 17 blocks out with the range upgrade: Wireless grid 50,64,8 in minecraft:the_end: opened (17.0 of 24 blocks).
  transmitter menu slots: 4
  info(50,64,8) range: Wireless transmitter at 50,64,8: range 24 blocks, active
  transmitter slots: [{"count":1,"item":"refinedstorage:range_upgrade"},{"count":1,"item":"refinedstorage:creative_range_upgrade"}]
  info(50,64,8) range: Wireless transmitter at 50,64,8: range unlimited, active
  /refinedstorage wireless 50 64 8 minecraft:the_end -> Wireless access at 50,64,8: accepted (70.5 of unlimited, online network)
  /execute in minecraft:overworld run refinedstorage wireless 50 64 8 minecraft:the_end -> Wireless access at 50,64,8: refused (different dimension)
```

Every assertion has its differential or counterfactual: the same 17.0-block use is refused in the
slice-4 phase (un-upgraded) and accepted after the upgrade; 24 is the named raised range (16 + 8);
70.5 blocks is beyond the 48-block maximum of four range upgrades, so only the creative upgrade can
accept it, and the same command from the overworld is still refused; a diamond shift-clicked into the
menu stays in the inventory; `SELFTEST=range` and `SELFTEST=creative` assert the old behaviour and
exit 1.

### Coverage (parity rule) — every family this slice ships

| family | state |
|---|---|
| `refinedstorage:range_upgrade` | **tested** — crafted by `recipe_check.py` (output id + count, ingredients consumed, wrong-ingredient control on `#c:ender_pearls`), given, inserted into the transmitter's slot, read back, and its rule proven at 17.0 / 24.0 blocks |
| `refinedstorage:creative_range_upgrade` | **tested** — given (creative-only, no recipe), inserted beside the range upgrade, `range unlimited`, 70.5 blocks accepted, another dimension refused; `SELFTEST=creative` inverts it |
| `refinedstorage:upgrade` (base) | **tested** — crafted (output id + count, `#c:glass_blocks` control) and consumed by the range upgrade's recipe |
| the transmitter's four upgrade slots | **tested** — `ctl container` insert/read back, and the menu shows the same four slots |
| the transmitter's menu (`refinedstorage:wireless_transmitter`, 4 slots) | **tested** — a right click with an empty hand opens it (`screen: container`, 4 container slots, the range upgrade in slot 0); the barrel template is what posts the open |
| the slot rules (only upgrades, 1 per slot) | **tested** — a diamond shift-clicked from the inventory does not enter the slots and stays in the inventory |
| the `range_label` readout | **tested** — `range 24 blocks` and `range unlimited` in the info line, `of 24 blocks`/`of unlimited` in both the command's and the item's replies |
| `speed`, `stack`, `regulator`, `autocrafting`, `silk_touch`, `fortune_1/2/3` | **not ported** — `blocked(no-importer/exporter/destructor/crafting-grid)`; the four enchanted-book recipes also `blocked(no-recipe-type: refinedstorage:upgrade_with_enchanted_book)` |
| network energy (upstream charges 8 FE/t per range upgrade) | **not ported** — parent gap 1 (no FE at all); the range rule is energy-independent |
| the transmitter's upgrade drops when the block is broken | **not ported** — the game's mod block entity drops an empty item (ARCHITECTURE §3's `block_drops` route is the fix, a later slice) |
| the dyed upgrade items / dyed transmitters | **not ported** — no colour variants in this port |

### Deviations and traps (this slice)

1. **The base `upgrade` item is shipped, the other eight are not.** The base item is not inert: the
   range upgrade's recipe consumes it. The eight are, so they are not in the package.
2. **The transmitter's template is `minecraft:barrel`.** It is what makes the block's use open the
   menu (container classes only); the shape/collision stay the nub via `shapes`, the `open` state is
   carried in the blockstate, and the drive already uses the same template.
3. **The binding moved to `right_click_block`.** With a container class the game runs the block's
   `use_item_on` before the item's `use_on`, so a wireless grid clicked onto a transmitter opened the
   menu instead of binding. The hook cancels the click and binds, exactly as a disk onto a drive; the
   item's `use_on` keeps the same helper for the prediction path.
4. **The upgrade slot count is the menu's four.** A range upgrade past four cannot be placed (the
   slot rule), and `upgraded_range` caps a stray count at four anyway, so the 48-block maximum holds
   even if a save is edited.
5. **Harness flake fixed in `tools/recipe_check.py`.** On a loaded box a right click on the crafting
   table occasionally reached the server without opening the menu, and the driver died on "no menu is
   open" rather than on a recipe. `reset_table` now asks up to three times and only then fails; the
   same run passed all 24 recipes and 22 controls afterwards. (A first attempt had also read the
   upgrade slot's JSON with a fixed key order — `jq` prints `count` before `item` — and compared
   objects; the assertion now uses a `jq -e` predicate.)

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

Evidence: `ports/_shared/evidence/refinedstorage/` — `plan-slice6-upgrades.md`,
`runtime-red-slice6-upgrades.log`, `sdk-test-0.2.6.log`, `sdk-clippy-0.2.6.log`, `sdk-fmt-0.2.6.log`,
`sdk-check-0.2.6.log`, `sdk-build-0.2.6-{1,2}.log`, `sdk-check-fits-0.2.6.log`, `sdk-fits-0.2.6.log`,
`refinedstorage.hmod` (the delivered bytes) — and `ports/_shared/evidence/refinedstorage-e2e/` —
`e2e-slice6-green.log` (green), `e2e-slice6-control.log`, `e2e-slice6-selftest.log`,
`e2e-slice6-selftest-range.log`, `e2e-slice6-selftest-creative.log`, and `launcher-build-0.2.6.log`
(`Compiling refinedstorage (1 of 1)`, `native-0a875da-3f241726424e` — the build every leg ran on).

## Slice 7 — the importer and the exporter, and the two upgrades they make real (2026-11-22) — version 0.2.7

Gap line this slice answers — §"Gaps / deviations (slice 1)" item 2 as slice 6 left it: *"the
**eight inert upgrades** (`speed`, `stack`, `regulator`, `autocrafting`, `silk_touch`,
`fortune_1/2/3` — `blocked(no-importer/exporter/destructor/crafting-grid)`)"*, and slice 2's
coverage row *"the rest of the jar (crafters, **exporters/importers**, …) → not ported"*. Slice 6's
report named this pair next, because it is the family that turns two of the eight refusals into
real items.

Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`**, mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, SDK `0a875da`, launcher 0.2.92, game 26.3. Class
`content` / tier **deep**, shape **machines**, unchanged: two block entities, two containers, two
menus, one work rule and two items are below the tier. No new pin API beyond the block capability
the thing needs, and no third-party mod: every ingredient is vanilla or a chain item slice 2 carries.

### What the jar says (read, not guessed)

`refinedmods/refinedstorage2` tag `v2.0.9`, commit
`0dec63affe8147702b020843c22748483ef18c01`. Files read: `common/importer/
{AbstractImporterBlockEntity,ImporterBlock,ImporterTransferQuotaProvider,ImporterContainerMenu}.java`,
`common/exporter/{AbstractExporterBlockEntity,ExporterBlock,ExporterTransferQuotaProvider,
ExporterContainerMenu}.java`, `network/impl/node/importer/{ImporterNetworkNode,
ImporterTransferStrategyImpl,CompositeImporterTransferStrategy,ImporterSource}.java`,
`network/impl/node/exporter/{ExporterNetworkNode,ExporterTransferStrategyImpl}.java`,
`neoforge/importer/{ItemHandlerImporterTransferStrategyFactory,ItemHandlerImporterSource}.java`,
`neoforge/exporter/ItemHandlerExporterTransferStrategyFactory.java`,
`common/upgrade/{UpgradeContainer,UpgradeDestinations}.java`, `common/AbstractModInitializer.java`
(`registerUpgradeMappings`), `common/support/SchedulingModeContainer.java`,
`resource/filter/{Filter,FilterMode}.java`, `support/direction/DefaultDirectionType.java`,
`support/{AbstractDirectionalBlock,AbstractDirectionalCableBlock}.java`,
`support/network/SimpleConnectionStrategy.java`, and the jar's `blockstates/{importer,exporter}.json`,
`models/{block,item}/{importer,exporter}*.json`, `recipe/{importer,exporter,speed_upgrade,
stack_upgrade}.json`, `tags/item/{importers,exporters}.json`, `lang/en_us.json` and
`textures/block/cable/part_border.png`.

| upstream | what this slice carries |
|---|---|
| `AbstractDirectionalCableBlock` + `SimpleConnectionStrategy` | a cable-like node that joins on **all six** sides, with a working face from `FACE_CLICKED` |
| `ImporterNetworkNode.doWork` | per work tick: the neighbour at the working face, the first resource the filter allows, up to the quota |
| `ImporterTransferQuotaProvider` | quota **1**, or **64** with a stack upgrade |
| `ExporterNetworkNode` + `DefaultSchedulingMode` | one task per filter entry, the first that moves anything wins |
| `ExporterTransferQuotaProvider` | quota 1, or 64 with a stack upgrade |
| `UpgradeContainer` | **4** slots, one item each, work tick rate **9**, `notifyListener` → `9 - 2 × speed`; IMPORTER = speed ×4, stack ×1, regulator ×4; EXPORTER = the same + autocrafting ×1 |
| `ImporterContainerMenu`/`ExporterContainerMenu` | nine filter slots, then the four upgrade slots (13), the player's 36 after |
| `Filter` | `BLOCK` by default: an **empty** filter blocks nothing, a listed resource is the one blocked |

### What was implemented

- **runtime.rs** (tests first): `Kind::Importer`/`Kind::Exporter` (+ `machine_name`),
  `WORK_TICK_RATE` (9), `SPEED_TICK_REDUCTION` (2), `MAX_SPEED_UPGRADES` (4),
  `MAX_STACK_UPGRADES` (1), `BASE_TRANSFER` (1), `STACK_TRANSFER` (64), `work_interval`,
  `transfer_quota`, `work_due`, `allows` (the BLOCK filter), `import_plan`, `export_plan` and
  `machine_readout`.
- **support.rs**: the two blocks in the one block-entity type; each a 13-slot container
  (nine filter entries + four upgrades) with its own content menu type (`{"slots": 13}`) and a
  `Screen` row for the filter entries and the upgrades; a `MachineSlots` menu Part (a filter slot
  takes any item, one per slot; an upgrade slot only a speed or stack upgrade, one per slot, at most
  four speed and one stack, scoped to `MenuContainer::Main` and to its own slot range); the work tick
  (`machine_tick`) with the counter in `Core::work`; `importer_pass`/`exporter_pass` moving items
  through the neighbour's `ITEMS` capability and the network's disks in **one transaction**
  (`write_back_in` is the old `write_back` without its own transaction); the `facing` reader; the
  machine line in `/refinedstorage info` (and the census gained importer/exporter counts).
- **Content** (from the jar, MIT): the two blocks (barrel template — the block's use opens its
  menu — with the cathode-ray shapes of their own models, in block units), their multipart
  blockstates (the cable core plus the machine's three steps, rotated per `facing`), the block and
  item models (the jar's, including its `item/<name>/base.json`), `part_border.png`, the two items,
  loot tables, the pickaxe tag, the `importers`/`exporters` item tags, the `speed_upgrade` and
  `stack_upgrade` items with their models, textures and recipes, the `importer`/`exporter` recipes,
  the lang keys and the creative tab entries.

### Defects fixed

**The port's directional placement was mirrored (found here, fixed here).** `state_for_placement`
wrote the *clicked face* into `facing`, while upstream's `DefaultDirectionType.FACE_CLICKED`
answers `clickedFace.getOpposite()` (`AbstractDirectionalBlock.getStateForPlacement` →
`getDirection(clickedFace, …)`). The port's `facing` is the side a node joins through and a machine
works through, so the two conventions meant a transmitter placed on a cable faced **away** from it
(offline), and the machines this slice adds would have read the block behind them. Provoked by the
slice's own need, proved from the artefact, fixed with `.opposite()` for all three directional
blocks; the slice-3 placement assertion now reads `facing=south` for a click on a block's north
face (the same statement, pointing at the clicked block). A user notices that a wireless
transmitter placed against a cable now joins it instead of staying dark, and an importer placed
against a chest now reads the chest.

**The four new recipes were first written in the jar's ingredient shape.** 1.21.1 spells an
ingredient `{"item": …}` / `{"tag": …}`; the pin spells it `"id"` / `"#tag"` (this file, §"Slice 2").
As written, the game loaded no result for `importer`, `exporter`, `speed_upgrade` or
`stack_upgrade` — `recipe_check.py` failed all four *and* their controls. Fixed in the data; the
run above crafts all four.

### Deviations (recorded, not defects)

1. **Nine real item slots are the filter, not nine ghost resource slots.** Upstream's filter
   container holds *representations*; the pin's mod menus have item slots only, so a filter entry
   is one real item held in a slot (and saved with the block entity). One entry per slot, as
   upstream's `getMaxStackSize` 1.
2. **No filter mode, fuzzy mode, redstone mode or scheduling mode.** The importer is upstream's
   default `BLOCK` filter, the exporter is its default scheduling mode (filter order); the toggles
   are GUI buttons the pin's mod screens have no widget for.
3. **No exporting indicators.** Upstream's exporter menu shows per-filter results
   (`ExportingIndicator`); the port's menu shows the slots only.
4. **No energy.** Parent gap 1: the controller consumes and stores nothing, so the machines' FE/t
   (and the upgrades' extra usage) is not charged. The transfer rules are energy-independent.
5. **The machines' upgrade slots drop nothing when the block is broken** — the same gap slice 6
   recorded for the transmitter (the game's mod block entity drops an empty item;
   `ARCHITECTURE.md` §3's `block_drops` route is the fix, a later slice). A filter item held in a
   filter slot is lost the same way.
6. **The importer/exporter item is one model** (the jar's own `item/<name>/base.json`), not a
   dyed variant set; no coloured variants exist in this port.
7. **`regulator` and `autocrafting` are still not shipped.** Their *consumer* now exists, but each
   needs a surface this port does not have: the regulator reads its amount and resource from the
   upgrade item's own menu (`RegulatorUpgradeItem.use` opens a `RegulatorUpgradeContainerMenu`;
   `RegulatorUpgradeState.EMPTY` is amount 1 with **no resource**, so an unconfigured regulator
   changes nothing), and the autocrafting upgrade needs a crafting-task queue
   (`MissingResourcesListeningExporterTransferStrategy` + `scheduleAutocrafting`). Shipping either
   now would be the `mifa` case: an item whose every read is zero. Ledger rows stay
   `blocked(no-regulator-configuration)` / `blocked(no-autocrafting-system)`.

### Red proof — the rules as tests, before the glue

Written first against stubs (`work_interval` always 9, `transfer_quota` always 1, `work_due`
always true, `allows` always true, `import_plan` ignoring filter and quota, `export_plan` always
`None`, a readout without upgrades), then run: **11 failed of 77**
(`ports/_shared/evidence/refinedstorage/runtime-red-slice7-machines.log`):

```
---- runtime::tests::each_speed_upgrade_shortens_the_work_interval_by_two stdout ----
assertion `left == right` failed
  left: 9
 right: 7
---- runtime::tests::an_empty_filter_imports_everything stdout ----
assertion `left == right` failed: an empty BLOCK filter blocks nothing
  left: Some((0, "minecraft:stone", 5))
 right: Some((0, "minecraft:stone", 1))
---- runtime::tests::an_exporter_exports_only_what_its_filter_names stdout ----
assertion `left == right` failed
  left: None
 right: Some(("minecraft:gold_ingot", 1))
---- runtime::tests::the_first_filter_entry_with_stock_wins stdout ----
assertion `left == right` failed: the first entry with stock is the pass's task
  left: None
 right: Some(("minecraft:gold_ingot", 1))
test result: FAILED. 66 passed; 11 failed; 0 ignored; 0 measured; 0 filtered out
```

Green after the rules: **77 passed, 0 failed** (`sdk-test-0.2.7.log`). The thirteen new tests are
`kind_maps_the_four_blocks_and_nothing_else` (extended to seven), `a_machine_without_speed_upgrades_works_every_nine_ticks`,
`each_speed_upgrade_shortens_the_work_interval_by_two`, `a_fifth_speed_upgrade_is_not_counted`,
`a_work_pass_falls_on_every_interval_th_tick_starting_with_the_first`,
`a_stack_upgrade_multiplies_one_pass_by_a_stack`, `an_empty_filter_imports_everything`,
`a_filter_imports_everything_but_what_it_names`,
`an_import_takes_the_quota_and_no_more_than_the_slot_holds`,
`an_exporter_exports_only_what_its_filter_names`, `an_exporter_without_a_filter_exports_nothing`,
`an_export_takes_the_quota_and_no_more_than_the_network_holds`,
`the_first_filter_entry_with_stock_wins` and `a_machine_readout_names_its_upgrades_and_interval`.

### Green — the ladder and the in-game leg (one launcher build at `0a875da`)

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `cargo test --offline` | **77 passed**, 0 failed (`sdk-test-0.2.7.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy-0.2.7.log`) |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean, `FMT_EXIT:0` (`sdk-fmt-0.2.7.log`) |
| SDK check | `cargo horizon-mod check` | *"…566 items of the API (… 20 hooks …). Made for 0a875da."* (`sdk-check-0.2.7.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical** (`cmp`), **179,081 B**, SHA-256 `3223f32ae80773769dd4a5b3ac1dc350477d7608778131223a6ba43e93bb6bb9`, **235 entries**, no `Cargo.lock` (`sdk-build-0.2.7-{1,2}.log`) |
| package check | `cargo horizon-mod check refinedstorage.hmod` | `0.2.7 (native-mod, format native-1/0a875da…/e59da909…, for 26.3) 179081 bytes` |
| fits (pin) | `… fits refinedstorage.hmod --build 0a875da` | *"0a875da made for"*, `FITS_PIN_EXIT:0` |
| fits (other build) | `… fits refinedstorage.hmod --build 0b14f8a` | *"0b14f8a fits"*, `FITS_OTHER_EXIT:0` (`sdk-check-fits-0.2.7.log`) |
| launcher build | `launcher-game.sh build refinedstorage --rebuild` | exit 0, **`Compiling refinedstorage (1 of 1)`**, `native-0a875da-3a543f18b508` (`launcher-build-0.2.7.log`) |
| in-game leg | `build-slot.sh run --step e2e -- bash tools/e2e.sh` | **PASS** + **`EXIT:0`** (`e2e-slice7-green.log`) |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME …` | **FAIL** + **`EXIT:1`** — *"the block at 41,64,1 is minecraft:air, expected facing=north"* |
| selftest (flipped interval) | `SELFTEST=speed …` | **FAIL** + **`EXIT:1`** — *"the un-upgraded importer moved 64 items in 20 ticks, expected 1 to 5"* |
| selftest (flipped quota) | `SELFTEST=stack …` | **FAIL** + **`EXIT:1`** — *"with a stack upgrade the importer moved to 64 items, expected 5"* |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | `logs=31 ok=31 missing-exit=0 bad-exit=0` |
| package gate | `verify-sdk-package.py refinedstorage` | **1/1** |

The scenario (quoted from `e2e-slice7-green.log`; the machines sit at z=16, on the arena floor,
with the source a hopper at 44,64,16 and the exporter's destination a chest at 41,64,18):

```
  block(43,64,16): refinedstorage:importer[facing=east,open=false]
  info(42,64,16): … online (4 nodes, 1 controller, 1 drive, 0 grid, 1 importer); 0 disks, …
  info(43,64,16) machine: Importer at 43,64,16: 0 speed upgrades, 0 stack upgrades, work interval 9 ticks, 0 filters
  items given: importer 1, exporter 1
  after 20 ticks without an upgrade: 3 items on the network, source [{"count":61,"item":"minecraft:stone"}]
  importer slots: [{"count":1,"item":"refinedstorage:stack_upgrade"}]
  info(43,64,16) machine: Importer at 43,64,16: 0 speed upgrades, 1 stack upgrade, work interval 9 ticks, 0 filters
  after the stack upgrade: 64 items on the network, source []
  importer slots: [4 × refinedstorage:speed_upgrade]
  info(43,64,16) machine: Importer at 43,64,16: 4 speed upgrades, 0 stack upgrades, work interval 1 tick, 0 filters
  after 20 ticks with four speed upgrades: 88 items on the network
  info(42,64,16): … 128 items in 2 resources
  importer filter: [{"count":1,"item":"minecraft:dirt"}, 4 × speed_upgrade]
  source after the filtered pass: [{"count":5,"item":"minecraft:dirt"}]
  info(41,64,17) machine: Exporter at 41,64,17: 0 speed upgrades, 0 stack upgrades, work interval 9 ticks, 1 filter
  exporter with no filter: 133 -> 133 items
  exporter destination: [{"count":4,"item":"minecraft:gold_ingot"}]
  exporter network after the pass: 133 items (was 133)
  exporter drops: {"minecraft:gold_ingot":1,"refinedstorage:exporter":1}
  importer drops: {"refinedstorage:importer":1}
```

Every assertion has its differential or counterfactual: the **same** importer and source are read
after the same 20 ticks with no upgrade (3), with a stack upgrade (64 in one pass, source empty)
and with four speed upgrades (88, then the whole source); the filter leg has the stone leave and
the dirt stay in the same chest; the exporter has a no-filter pass move nothing before the gold
pass moves exactly four; the machine menu shows 13 slots and its upgrade row refuses a diamond
while a filter slot took the dirt; and `SELFTEST=speed` / `SELFTEST=stack` assert the other side of
the two rules, so a machine that ignored its upgrades fails the run.

### Harness facts learned here

- **A container block's own use wins over placing a block against it.** Right-clicking a chest or
  a hopper with a block item in hand opens *its* menu (the reply's `screen: container`), so an
  importer cannot be placed by clicking one. The machines are placed with `setblock`, and the
  directional placement rule is proven on a plain stone (the shared `state_for_placement` hook), or
  by a machine actually working on the neighbour its `facing` names.
- **Creative placement keeps the block in the hand** (`hotbar[0].count == 1` after placing), so
  "the item left the hand" is not a placement assertion in creative; a *mod* consumption (a disk
  into a drive) is a different path.
- **A `ctl tp` keeps the player's velocity**, so a spot chosen for an exact distance drifts (a
  16.0-block use was measured at 16.4 after `tp 34 64 8`). The exact boundary is the command's
  assertion; the item's in-range leg stands 15.5 blocks out.
- **A block capability is `LevelRef::capability(host, &ITEMS, pos, side)`** — the game's own
  provider is the hopper-rules container handler, so a vanilla chest or hopper needs no extra
  registration, and the side passed is the source's face toward the machine
  (`direction.getOpposite()` upstream).
- **The exporter drains as fast as the importer fills.** With a filter already set, a spike on the
  network is not observable: assert the destination's count and the network's return instead.

### Coverage (parity rule) — every family this slice ships

| family | state |
|---|---|
| the `importer` block | **tested** — placed, read back with `facing`, its network census line, and **broken for itself** (`drops: {refinedstorage:importer: 1}`) |
| the `exporter` block | **tested** — placed, read back, and **broken for itself** (`drops: {refinedstorage:exporter: 1}`) |
| the importer's transfer tick | **tested** — 1 item per 9 ticks (3 in 20), 64 with a stack upgrade (one pass), ~20 in 20 with four speed upgrades |
| the exporter's transfer tick | **tested** — 1 item per 9 ticks, exactly the four gold its filter names |
| the work interval (`9 - 2 × speed`) | **tested** — unit tests (9/7/5/3/1, a fifth uncounted) and the in-game counts; `SELFTEST=speed` inverts it |
| the transfer quota (1 / 64) | **tested** — unit tests and the in-game one-pass 64; `SELFTEST=stack` inverts it |
| the filter (nine entries, BLOCK mode) | **tested** — an empty filter imports everything, a named resource stays in the source, the exporter exports only what it names, a no-filter exporter does nothing |
| the 13-slot menu | **tested** — `ctl screen` (13 container slots), the upgrade row visible, an upgrade placed and read back from the block's own slots |
| the slot rules | **tested** — a diamond refused by the upgrade row (and the filter row accepting a filter item) |
| `refinedstorage:speed_upgrade` | **tested** — crafted by `recipe_check.py` (output + count, ingredients consumed, wrong-ingredient control), given, placed in a machine's slot, its rule proven |
| `refinedstorage:stack_upgrade` | **tested** — the same, and its recipe consumes a speed upgrade |
| the `importer`/`exporter` recipes | **tested** — crafted (output id + count, ingredients consumed) with a wrong-ingredient control on `refinedstorage:destruction_core`/`construction_core` |
| the `importers`/`exporters` item tags | **carried** — declared with their plain member, ready for the dyed variants |
| the block models, blockstates, textures and lang | **carried** — the jar's own models (the cable core plus the machine's steps), `part_border.png`, the two item textures |
| `regulator_upgrade` | **not ported** — `blocked(no-regulator-configuration)`: its amount/resource live in the item's own menu, which the port has no surface for; unconfigured it changes nothing |
| `autocrafting_upgrade` | **not ported** — `blocked(no-autocrafting-system)`: it starts an autocrafting task for a missing resource |
| `silk_touch`, `fortune_1/2/3` | **not ported** — `blocked(no-destructor)`, and the enchanted-book recipe type |
| filter mode / fuzzy / redstone / scheduling mode toggles, exporting indicators | **not ported** — deviation 2 and 3 |
| the upgrades/filter items dropping when a machine is broken | **not ported** — deviation 5 |
| network energy (upstream 5 FE/t per machine + per upgrade) | **not ported** — parent gap 1 |

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

Evidence: `ports/_shared/evidence/refinedstorage/` — `plan-slice7-importer-exporter.md`,
`runtime-red-slice7-machines.log`, `sdk-test-0.2.7.log`, `sdk-clippy-0.2.7.log`,
`sdk-fmt-0.2.7.log`, `sdk-check-0.2.7.log`, `sdk-build-0.2.7-{1,2}.log`,
`sdk-check-fits-0.2.7.log`, `refinedstorage.hmod` (the delivered bytes) — and
`ports/_shared/evidence/refinedstorage-e2e/` — `e2e-slice7-green.log` (green),
`e2e-slice7-control.log`, `e2e-slice7-selftest-speed.log`, `e2e-slice7-selftest-stack.log`,
`e2e-slice7-selftest-networks.log` (the slice-5 rule's counterfactual, still failing on purpose), and
`launcher-build-0.2.7.log` (`Compiling refinedstorage (1 of 1)`, `native-0a875da-3a543f18b508` —
the build every leg ran on).

## Slice 8 — the controller's energy model (2026-11-22) — version 0.2.8

Gap line this slice answers — §"Gaps / deviations (slice 1)" item 1, verbatim: *"**No energy.** The
controller is the network's on/off root but consumes and stores nothing; its `energy_type` state has
two values (`off`/`on`) where upstream has four energy levels. No FE hook is needed or used."* Every
later slice cites it: the six refused upgrades, the wireless grid's per-open drain, the transmitter's
16 FE/t and both machines' per-pass cost all wait on the network having a buffer at all. It is the
family the slice-7 report named next for that reason.

Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`**, mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, SDK `0a875da`, launcher 0.2.92, game 26.3. Class
`content` / tier **deep**, shape **machines**, unchanged: one buffer, one pure cost rule, one gate and
one consumer are below the tier. No third-party mod, and the only pin API this slice adds is the
**block energy capability** (`Registrar::block_energy`, `ENERGY`/`EnergyHandler`).

### The reference (read from `refinedmods/refinedstorage2` v2.0.9, MIT, commit `0dec63af`)

| upstream | value | where |
|---|---|---|
| `CONTROLLER_CAPACITY` | 1000 FE | `common/content/DefaultEnergyUsage.java` |
| `CABLE` / `CONTROLLER` usage | 0 / 0 | same, and `ControllerNetworkNode.getEnergyUsage` |
| `GRID` | 10 | same |
| `DISK_DRIVE` + `DISK_DRIVE_PER_DISK` | 10 + 4 | same |
| `IMPORTER` / `EXPORTER` | 2 / 2 (a work pass) | same |
| `WIRELESS_TRANSMITTER` | 16 | same |
| `RANGE_UPGRADE` / `SPEED_UPGRADE` / `STACK_UPGRADE` | 8 / 4 / 16 | same |
| the level thresholds | ≥0.4 `on`, ≥0.3 `nearly_on`, ≥0.01 `nearly_off`, else `off` | `api/network/impl/node/controller/ControllerNetworkNode.getState` |
| the gate | a node works iff `stored >= getEnergyUsage()` | `common/support/network/AbstractBaseNetworkNodeContainerBlockEntity.calculateActive` |
| the draw | an active node's `doWork` extracts its usage | `api/network/impl/node/AbstractNetworkNode.doWork` |
| the admission order | dimension → `node.isActive()` → range | `common/networking/WirelessTransmitterNetworkNodeContainer.isValid` |

The controller is active whatever its own (0) usage, so a network with a controller is **online**
structurally — the port's `Network::online()` keeps that meaning — and the new rule is per node:
without energy its nodes are inert. That is why the wireless refusal distinguishes `Offline` (no
controller) from `NoEnergy` (a controller whose buffer cannot pay).

### What was implemented

- **runtime.rs** (tests first): `CONTROLLER_CAPACITY` and every reference rate as a named constant;
  `Fit` (an upgrade census) and `usage(kind, fit, disks)`; `Energy { stored, usage }` with
  `energized()`/`draw()`; `EnergyType::{Off, NearlyOff, NearlyOn, On}` with `of(stored, capacity)`
  (integer thresholds, so 400/300/10 of 1000 are exact) and `property()`; `bank` (a network's
  controllers summed); and `Refusal::NoEnergy { stored, usage }` inside `admit`, in upstream's order.
- **support.rs**: the controller's buffer declared through `registrar.block_energy(…, ModEnergyBuffer
  { capacity 1000, max_insert 1000, max_extract 1000, all six faces })` — the game's own handler, whose
  amount lives in the entity's compound under `energy`, so an FE cable and this network share one
  buffer; the controller's tick writes `energy_type` from the stored/capacity ratio; `pay` gates every
  node and takes its charge through `ENERGY`/`EnergyHandler::extract` in a transaction (grid 10 a tick,
  drive 10 + 4 a disk, transmitter 16 + 8 a range upgrade, a machine 2 per work pass + 4 a speed and
  16 a stack upgrade); `/refinedstorage info` reads `energy <stored>/<capacity> FE, drawing <n>/tick[ +
  <m> per work pass], t=<server tick>`; `/refinedstorage energy <pos> <amount>` fills or drains the
  buffer through the same capability.
- **Content** (from the jar, MIT): the controller's `energy_type` now carries upstream's four values
  with the jar's `nearly` parent model, the two `nearly_*` models, the four `cutouts/nearly_*` textures
  and their `.mcmeta` animations, and its default state is `off` (`AbstractControllerBlock`).

### Port defects fixed while building it (not upstream's)

1. **`entity.data()` is not a live read for another block entity.** The first cut read a controller's
   amount from its saved compound; while one node writes through the capability another node's read
   came back up to hundreds of FE behind, so a charged network looked empty (a grid went dark with 82
   FE in the bank). Reads now go through the same handler as the writes (`ENERGY`,
   `EnergyHandler::amount`), so the gate and the charge cannot disagree.
2. **The controller's own tick must not write its amount back.** A block entity is out of the world
   during its own tick (`native_block_entity_tick`: "it reads as a `Mod` with an empty id"), so the
   capability cannot answer for it and its payload is the only reading; materialising the key or
   clamping from that snapshot could undo another node's drain from the same tick. The tick now writes
   `energy_type` only, and clamps solely when the buffer is over capacity.

### Deviations (recorded, not defects)

1. **`/refinedstorage energy` is a harness/behaviour addition**, like `/refinedstorage info`,
   `/refinedstorage filter`, `/refinedstorage wireless` and `/refinedstorage bound`. Upstream's
   controller is filled by its neighbours' FE cables; the harness has no generator at this pin, so the
   command is the source and it goes through the *same* capability an FE cable would use.
2. **The wireless grid's own item buffer is not ported.** Upstream gives the item 1000 FE and charges 5
   per open/insert/extract (`WIRELESS_GRID_CAPACITY`, `WIRELESS_GRID_OPEN`); that is the *item* energy
   capability (`item_energy_buffer`), a separate family from the network's, and it waits for a slice
   that also carries the portable grid. The network-side gate it depends on (`stored > 0`) is covered
   by the transmitter's charge. **(Done in slice 9, 0.2.9:** the 1,000 FE buffer and the three 5 FE
   charges, through the item energy capability; §"Slice 9".)
3. **No `isEnergyRequired` switch and no creative controller.** The pin has no config API at this tier
   (`blocked(no-config-api)`), so an empty network is always gated; the creative controller
   (`InfiniteEnergyHandler`, no recipe) is not shipped.
4. **No controller menu / energy bar.** The buffer is read by `/refinedstorage info`; a gauge is a
   server-side screen data value the pin's had no widget for at this tier (the same gap the grid's
   sorting buttons record).

### Red proof — the rules as tests, before the implementation

Written against stubs (`usage` always 0, `energized` always true, `EnergyType::of` always `On`, the
admission ignoring energy), then run: **9 failed of 89**
(`ports/_shared/evidence/refinedstorage/runtime-red-slice8-energy.log`):

```
---- runtime::tests::a_transmitter_charges_sixteen_fe_a_tick stdout ----
assertion `left == right` failed
  left: 0
 right: 16
---- runtime::tests::no_energy_refuses_a_use_before_the_range_is_asked stdout ----
  left: Err(OutOfRange { distance: 100.0, range: 16 })
 right: Err(NoEnergy { stored: 0, usage: 16 })
---- runtime::tests::the_energy_type_is_the_four_upstream_levels stdout ----
  left: On
 right: NearlyOn
---- runtime::tests::a_tick_takes_exactly_the_charge_and_never_goes_negative stdout ----
thread '…' panicked at src/runtime.rs:1286:34: attempt to subtract with overflow
test result: FAILED. 80 passed; 9 failed; 0 ignored; 0 measured; 0 filtered out
```

Green after the rules: **89 passed, 0 failed** (`sdk-test.log`). The thirteen new tests cover the
reference rates (transmitter 16 + 8 a range upgrade and the creative one's 0, drive 10 + 4 a disk,
grid 10, machines 2 + their upgrades with the caps), the gate (`draw` is `None` below the charge and
never negative), the four levels and their property strings, and the admission order.

### Green — the ladder and the in-game leg (one launcher build at `0a875da`)

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `cargo test --offline` | **89 passed**, 0 failed (`sdk-test.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean, `FMT_EXIT:0` (`sdk-fmt.log`) |
| SDK check | `cargo horizon-mod check` | *"…582 items of the API (5 Part kinds, 1 associated item, **4 capabilitys**, 20 hooks …). Made for 0a875da."* |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical** (`cmp`), **193,579 B**, SHA-256 `e755815241995fc2d4dc6123c6feaae24a0130aa6999b5d21d4548b16ecff72a`, **244 entries**, no `Cargo.lock` (`sdk-build-0.2.8-{1,2}.log`) |
| package check | `cargo horizon-mod check refinedstorage.hmod` | `0.2.8 (native-mod, format native-1/0a875da…/e59da909…, for 26.3) 193579 bytes` |
| fits (pin) | `… fits refinedstorage.hmod --build 0a875da` | *"0a875da made for"* |
| fits (other build) | `… fits refinedstorage.hmod --build 0b14f8a` | *"0b14f8a fits"* (`sdk-check-fits-0.2.8.log`; the GitHub releases list answered 403 first — the unauthenticated limit — and the run was repeated after it reset) |
| launcher build | `launcher-game.sh build refinedstorage --rebuild` | exit 0, **`Compiling refinedstorage (1 of 1)`**, `native-0a875da-0f48bca7346a` |
| in-game leg | `build-slot.sh run --step e2e -- bash tools/e2e.sh` | **PASS** + **`EXIT:0`** (`e2e-slice8-energy.log`) |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME …` | **FAIL** + `EXIT:1` — *"the block at 41,64,1 is minecraft:air, expected facing=north"* |
| selftest (existing range rule) | `SELFTEST=1 …` | **FAIL** + `EXIT:1` |
| selftest (the new gate) | `SELFTEST=energy …` | **FAIL** + `EXIT:1` — *"expected … 'accepted (', got '… refused (not enough energy: 0 of 16 FE)'"* |
| selftest (the 0.2.5 promise rule) | `SELFTEST=networks …` | **FAIL** + `EXIT:1` — the two-network rule is unchanged |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | see below |
| package gate | `verify-sdk-package.py refinedstorage` | **1/1** |

The energy phase, quoted from `e2e-slice8-energy.log`:

```
  energy: 308 -> 116 FE over 12 ticks                              (exactly 16 a tick)
  block(36,64,20): refinedstorage:controller[energy_type=off]      (0 FE)
  block(36,64,20): refinedstorage:controller[energy_type=nearly_off]  (50 FE)
  block(36,64,20): refinedstorage:controller[energy_type=nearly_on]   (350 FE)
  block(36,64,20): refinedstorage:controller[energy_type=on]          (500 FE)
  block(46,64,20): refinedstorage:wireless_transmitter[active=false,facing=west,open=false]
  /refinedstorage wireless 46 64 20 minecraft:the_end -> … refused (not enough energy: 0 of 16 FE)
  /refinedstorage wireless 46 64 20 minecraft:the_end -> … refused (not enough energy: 0 of 16 FE)   (17 blocks out: energy before range)
  /refinedstorage wireless 46 64 20 minecraft:the_end -> … refused (out of range: 17.7 > 16 blocks)   (charged: the range speaks)
  /refinedstorage wireless 46 64 20 minecraft:the_end -> … accepted (1.6 of 16 blocks, online network)
  grid view: []                                                    (network A at 0 FE)
  grid view: [{"count":3,"item":"minecraft:gold_ingot","slot":300}] (charged again)
  after 20 ticks at 0 FE: 1 items on the network (was 1)            (the importer does no pass)
  after 20 ticks without an upgrade: 4 items on the network, source [59 stone]   (charged: it does)
  info(42,64,16): … energy 940/1000 FE, drawing 14/tick + 2 per work pass, t=8104
```

Every assertion has its differential or counterfactual: the same 12 ticks measure exactly 16 FE each
(against the tick counter the line prints, not a sleep); the four levels are read at the reference
thresholds on a network that draws nothing; the same use is refused *for energy* at 0 FE and refused
*for range* at the same distance with energy, then accepted inside the range; the same grid lists
nothing at 0 FE and its 3 gold when charged; the same importer moves nothing in 20 ticks at 0 FE and
3 items in the same window when charged (1 → 4). `SELFTEST=energy` inverts the no-energy refusal and
exits 1.

### Harness facts learned here

- **A 1,000 FE buffer is small.** A grid (10) + a drive with a disk (14) drains it in ~42 ticks, so a
  scenario that keeps a network working must feed it like a player's generators do. The e2e's
  `charge_net` (called before a menu click, a view read and a long wait) is that generator; a leg that
  asserts the *absence* of energy clears it around the window (`NET=""`).
- **`ctl tp` keeps velocity**: a spot chosen for an exact distance drifts (a 17.0-block use measured
  17.7 here). The exact boundary belongs to the command's assertion, which reads the player's own
  position; the in-range leg asserts only that it is accepted.
- The energy phase's exact measurement reads the buffer and the printed tick together
  (`energy_of` splits `energy <stored>/<capacity> FE … t=<n>`), so the assertion is
  `stored₁ − stored₂ = 16 × (t₂ − t₁)` — independent of how long a `ctl` call took.

### Coverage (parity rule) — every family this slice ships

| family | state |
|---|---|
| the controller's buffer (1000 FE) | **tested** — `/refinedstorage energy` fills it through the capability (`Network energy at … 1000/1000 FE (was 0)`), `info` reads it back, and it drains as nodes pay |
| per-block charge, read from the reference | **tested** — the transmitter's 16/tick measured exactly over N ticks (12 ticks → 192 FE), the drive's 14 (10 + 4 for its disk) and the importer's 2 a work pass printed in `info`, and unit tests for every constant and the upgrade additions |
| the four `energy_type` levels | **tested** — 0 / 50 / 350 / 500 FE read back as `off`, `nearly_off`, `nearly_on`, `on` at upstream's thresholds |
| the gate (no work without energy) | **tested** — the grid is `active=false` and lists nothing at 0 FE (its disk keeps the gold), the importer moves nothing in 20 ticks at 0 FE, and the transmitter refuses a use for energy |
| the admission order | **tested** — 0 FE at 2.5 blocks and at 17 blocks both refuse `not enough energy`; with energy the same 17 blocks refuse `out of range` and 2.5 blocks are accepted |
| the block energy capability | **tested** — every charge and every drain goes through it (the charge replies `1000/1000 FE (was …)`, i.e. the game's handler wrote the buffer) |
| the wireless grid's own item buffer (1000 FE, 5/open) | **not ported** — item energy (`item_energy_buffer`) is a separate family; deviation 2 |
| `isEnergyRequired` config switch | **blocked(no-config-api)** — the pin has no config surface at this tier |
| the creative controller (`InfiniteEnergyHandler`) | **not ported** — no creative controller block in the port (no recipe upstream either) |
| the controller's menu / energy gauge | **not ported** — a screen data value the pin's mod screens have no widget for at this tier; `/refinedstorage info` reads it |
| everything slices 1–7 shipped | **unchanged** — the same assertions, the transmitter's facing rule, the range and creative upgrades, the promise table and both machines are still green in the same log |

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

Evidence: `ports/_shared/evidence/refinedstorage/` — `plan-slice8-energy.md`,
`runtime-red-slice8-energy.log`, `sdk-test.log`, `sdk-clippy.log`, `sdk-fmt.log`,
`sdk-build-0.2.8-{1,2}.log`, `sdk-build-0.2.8-cmp.log`, `sdk-check-fits-0.2.8.log`,
`refinedstorage.hmod` (the delivered bytes) — and `ports/_shared/evidence/refinedstorage-e2e/` —
`e2e-slice8-energy.log` (green), `e2e-slice8-control.log`, `e2e-slice8-selftest.log`,
`e2e-slice8-selftest-energy.log`, `e2e-slice8-selftest-networks.log`.

## Slice 9 — the wireless grid's own item buffer (2026-11-22) — version 0.2.9

Gap line this slice answers — §"Slice 8" → "Coverage (parity rule)", verbatim: *"the wireless grid's own
item buffer (1000 FE, 5/open) | **not ported** — item energy (`item_energy_buffer`) is a separate
family; deviation 2"*, and deviation 2 itself: *"Upstream gives the item 1000 FE and charges 5 per
open/insert/extract (`WIRELESS_GRID_CAPACITY`, `WIRELESS_GRID_OPEN`)."* The slice-8 report named it next
for that reason: it is the item half of the energy model slice 8 built for the network.

Pin **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`**, mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, SDK `0a875da`, launcher 0.2.92, game 26.3. Class
`content` / tier **deep**, shape **machines**, unchanged: one item capability, one item component,
one pure cost rule and one gate on the open and on each moving click are below the tier. No new
content id and no third-party mod.

### The reference (upstream `refinedmods/refinedstorage2` v2.0.9, MIT, `DefaultEnergyUsage`)

| constant | value |
|---|---|
| `WIRELESS_GRID_CAPACITY` | 1000 FE |
| `WIRELESS_GRID_OPEN` | 5 FE |
| `WIRELESS_GRID_INSERT` | 5 FE |
| `WIRELESS_GRID_EXTRACT` | 5 FE |

Recorded by slice 8's read of the same file (§"Slice 8", the `WIRELESS_TRANSMITTER` row and deviation 2);
this slice implements it, and the brief asserts the same four values.

### What was implemented

- **runtime.rs** (tests first): `WIRELESS_GRID_CAPACITY`, `WIRELESS_GRID_OPEN`/`_INSERT`/`_EXTRACT`;
  `WirelessUse { Open, Insert, Extract }` with `cost()`; `charge_item(stored, use)` — the buffer after
  paying, or `Refusal::NoItemEnergy { stored, cost }`, whose `Display` is **`not enough item energy:
  {stored} of {cost} FE`**, deliberately distinct from the network's `not enough energy: …`.
- **support.rs**: the grid's charge lives in the item's own `refinedstorage:energy` component (a
  `Component` impl, as the binding is); the item energy capability (`mod_api::ITEM_ENERGY`, NeoForge's
  `Capabilities.Energy.ITEM`, the port's **fifth** capability) is registered for
  `refinedstorage:wireless_grid` with `GridBattery`, whose writes go through the player's own slot;
  `pay_grid` is the gate plus the charge, in one transaction through the capability; the open pays
  `WirelessUse::Open` before the menu is opened, and a menu click that moves items pays
  `Insert`/`Extract` **before** it moves anything (a click that moves nothing is free); two commands,
  `/refinedstorage grid` (the readout, with the server tick) and `/refinedstorage charge <amount>`
  (the item-side twin of `/refinedstorage energy`).
- **Content**: none — the buffer is a data component of an item that already ships.

### Port defect fixed (found by this slice's first in-game leg)

**A held item cannot be charged through `ItemPlace::exchange` in creative.** The first cut used the
SDK's stock `ItemComponentEnergy` (NeoForge's `ItemAccessEnergyHandler`), which rewrites the item with
`place.exchange(host, tx, &changed, 1)`. In creative a hand's *extract* gives without losing the stack,
so the exchange left the old, uncharged grid in the hand and put a charged copy in the inventory: the
command answered `0/1000 FE (was 0)` while the hand's component stayed absent (measured in
`e2e-slice9-green.log`'s first run, `charge 1000` → `Wireless grid energy: 0/1000 FE (was 0)`). The
port now writes the changed stack through the player's own slot (`player.set_inventory_slot`, the path
`bind_grid` has used since slice 4, which the same leg proves), and keeps `exchange` only for places
that are container slots. A user notices that charging a held grid works in creative and survival
alike, and that no second grid appears.

### Harness defect fixed while proving it (not the mod's)

**The slice-8 rate window could run dry.** `== the controller's energy model ==` topped the transmitter
network up to 500 FE and then measured `stored₁ - stored₂ = 16 × (t₂ - t₁)` over a window whose harness
calls each wait a few ticks; on a loaded box the window stretched past 500/16 ticks and the buffer hit
its floor, so a correct 16 FE/tick read as `192 FE over 16 ticks` and failed (`SELFTEST=itemenergy`'s
first run, 2026-11-22). The window now starts **full** (1,000 FE, ~60 ticks at 16), so the assertion
measures the rate instead of the margin. No behaviour changed.

### Red proof — the rules as tests, before the implementation

Written against stubs (`cost()` answered 0, `charge_item` answered `Ok(stored)`, and the new refusal's
`Display` copied the network's wording), then run: **4 failed of 93**
(`ports/_shared/evidence/refinedstorage/runtime-red-slice9-item-energy.log`):

```
---- runtime::tests::each_wireless_grid_use_costs_five_fe stdout ----

thread 'runtime::tests::each_wireless_grid_use_costs_five_fe' (3756071) panicked at src/runtime.rs:2847:9:
assertion `left == right` failed: a full grid pays 5 to open
  left: Ok(1000)
 right: Ok(995)

---- runtime::tests::the_item_buffer_pays_exactly_and_refuses_below_the_cost stdout ----

thread 'runtime::tests::the_item_buffer_pays_exactly_and_refuses_below_the_cost' (3756089) panicked at src/runtime.rs:2866:9:
assertion `left == right` failed: exactly the cost is enough
  left: Ok(5)
 right: Ok(0)

---- runtime::tests::the_item_refusal_names_the_items_own_energy_not_the_networks stdout ----

thread 'runtime::tests::the_item_refusal_names_the_items_own_energy_not_the_networks' (3756090) panicked at src/runtime.rs:2893:9:
assertion `left == right` failed
  left: "not enough energy: 0 of 5 FE"
 right: "not enough item energy: 0 of 5 FE"

---- runtime::tests::the_wireless_grids_buffer_is_the_reference_capacity_and_costs stdout ----

thread 'runtime::tests::the_wireless_grids_buffer_is_the_reference_capacity_and_costs' (3756096) panicked at src/runtime.rs:2903:9:
assertion `left == right` failed: WIRELESS_GRID_OPEN
  left: 0
 right: 5

test result: FAILED. 89 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out
```

Green after the rules: **93 passed, 0 failed** (`sdk-test.log`). The four new tests are
`each_wireless_grid_use_costs_five_fe`, `the_item_buffer_pays_exactly_and_refuses_below_the_cost`,
`the_item_refusal_names_the_items_own_energy_not_the_networks` and
`the_wireless_grids_buffer_is_the_reference_capacity_and_costs`.

`src/runtime.rs` gains this slice's rule and its four tests (its byte-identical copy of the migration's
file ended at slice 3; slices 4–9 each added a rule with its tests). Everything it already carried —
the network, the disks, the view, the promise table, the transfer rules, the network energy model —
is untouched and still green, and the package carries the current file (`verify-sdk-package.py`:
`runtime byte-identical`).

### Green — the ladder and the in-game leg (one launcher build at `0a875da`)

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `cargo test --offline` | **93 passed**, 0 failed (`sdk-test.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean, `FMT_EXIT:0` (`sdk-fmt.log`) |
| SDK check | `cargo horizon-mod check` | *"…585 items of the API (5 Part kinds, 1 associated item, **5 capabilitys**, 20 hooks …). Made for 0a875da."* (`sdk-check-fits-0.2.9.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical** (`cmp`), **197,142 B**, SHA-256 `397bb8c0cd23c708b80e0b2b253f85280c89dfbc6a5d991d34d0e98b505d2014`, **244 entries**, no `Cargo.lock` (`sdk-build-0.2.9-{1,2}.log`, `sdk-build-0.2.9-cmp.log`) |
| package check | `cargo horizon-mod check refinedstorage.hmod` | `0.2.9 (native-mod, format native-1/0a875da…/e59da909…, for 26.3) 197142 bytes` |
| fits (pin) | `… fits refinedstorage.hmod --build 0a875da` | *"0a875da  made for"*, `FITS_PIN_EXIT:0` |
| fits (other build) | `… fits refinedstorage.hmod --build 0b14f8a` | *"0b14f8a  fits"*, `FITS_OTHER_EXIT:0` (both after `sha256sum refinedstorage.hmod` printed `397bb8c0…`, `sdk-check-fits-0.2.9.log`) |
| launcher build | `launcher-game.sh build refinedstorage --rebuild` | exit 0, **`Compiling refinedstorage (1 of 1)`**, `native-0a875da-a3aab5050286` (`launcher-build-0.2.9.log`) |
| in-game leg | `launcher-game.sh run refinedstorage` | **PASS** + **`EXIT:0`** (`e2e-slice9-green.log`) |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME …` | **FAIL** + **`EXIT:1`** — *"the block at 41,64,1 is minecraft:air, expected facing=north"* |
| selftest (flipped item gate) | `SELFTEST=itemenergy …` | **FAIL** + **`EXIT:1`** — *"expected the 0-FE deposit to have moved the stack, it is still in the inventory"* |
| selftest (the slice-8 network gate) | `SELFTEST=energy …` | **FAIL** + **`EXIT:1`** — *"… got 'Wireless access at 46,64,20: refused (not enough energy: 0 of 16 FE)'"* |
| selftest (the slice-5 promise rule) | `SELFTEST=networks …` | **FAIL** + **`EXIT:1`** — *"the grid's view is [{count 3 gold}], expected []"* |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | `logs=41 ok=41 missing-exit=0 bad-exit=0` |
| package gate | `verify-sdk-package.py refinedstorage` | **1/1** |

The new phase, quoted from `e2e-slice9-green.log` (the slice-4/5 network at 48..50,64,8, the drive at
49,64,9, the player at 50.5,64,10.5 inside the transmitter's range):

```
== the wireless grid's own item buffer: 1,000 FE, 5 a use ==
  /refinedstorage grid -> Wireless grid energy: 995/1000 FE, t=7600
  /refinedstorage charge 1000 -> Wireless grid energy: 1000/1000 FE (was 995)
  item energy: 1000/1000 FE at t=7614
  item energy: 1000/1000 FE at t=7631 (17 ticks later)
  item energy: 995/1000 FE at t=7660 after the open
  item energy: 990/1000 FE at t=7689 after the deposit
  grid view: [{"count":5,"item":"minecraft:diamond","slot":300}]
  item energy: 985/1000 FE at t=7723 after the withdrawal
  grid view: []
  /refinedstorage charge 0 -> Wireless grid energy: 0/1000 FE (was 985)
  info(49,64,9): Refined Storage network at 49,64,9: online (4 nodes, 1 controller, 1 drive, 0 grid, 1 transmitter); 1 disks, 0 items in 0 resources; energy 844/1000 FE, drawing 30/tick, t=7754
  deposit at 0 item FE: Wireless grid 50,64,8 in minecraft:the_end: refused (not enough item energy: 0 of 5 FE).
  info(49,64,9): Refined Storage network at 49,64,9: online (4 nodes, 1 controller, 1 drive, 0 grid, 1 transmitter); 1 disks, 0 items in 0 resources; energy 792/1000 FE, drawing 30/tick, t=7798
  /refinedstorage charge 0 -> Wireless grid energy: 0/1000 FE (was 0)
  open at 0 item FE: Wireless grid 50,64,8 in minecraft:the_end: refused (not enough item energy: 0 of 5 FE).
  /refinedstorage charge 1000 -> Wireless grid energy: 1000/1000 FE (was 0)
  item energy: 995/1000 FE after the charged open
```

Every assertion has its differential or counterfactual: the buffer is read twice **17 ticks apart with
no use and does not move** (the network beside it falls 30 FE a tick over the same window — the
per-tick charge is the network's, not the item's); the open, the deposit and the withdrawal each cost
exactly 5; at 0 item FE the *same* deposit is refused for the item's energy while the network's info
line still answers `online … energy 844/1000 FE, drawing 30/tick` (two buffers, two sentences); the
same use at 0 FE opens no menu, and the same use after `charge 1000` opens and costs 5;
`SELFTEST=itemenergy` asserts the 0-FE deposit went through and fails.

### Harness facts learned here

- **A fresh wireless grid is empty.** The component is absent until charged, so any leg that uses the
  grid must charge it first: `use_held` now does (`charge_grid 1000`), and the phases that use the
  vendored `refinedstorage wireless` command (the rule without the item) are unaffected.
- **The network beside a slow assertion drains.** `reply`/`info` each wait ~4 ticks, so a 1,000 FE
  network loses ~30 FE a tick while a phase reads its readouts. Every action in this phase tops the
  network up immediately before it (`charge_net`); the first version of the phase let it reach 2 FE and
  the use was refused *for the network*, which is exactly the confusion the two-word distinction
  (`not enough energy` vs `not enough item energy`) exists to expose.
- **`ctl select <item>` while a container menu is open is not a no-op** (the probe's reply lost its
  `screen` field); a phase that reselects the held grid must close the menu first.
- `ctl state`'s `held` prints the stack's components as SNBT and is the cheapest way to see that a
  written component landed on the item in the hand
  (`{"refinedstorage:energy":1000,"refinedstorage:network_location":{…}}`).

### Coverage (parity rule) — every family this slice ships

| family | state |
|---|---|
| the grid's own buffer (1,000 FE) | **tested** — `/refinedstorage charge` fills it through the item capability (readout `1000/1000 FE (was 0)`), `/refinedstorage grid` reads it back with the server tick, and it does not drift over ticks |
| the open's 5 FE | **tested** — 1000 → 995 on a use, and the same use after a full charge → 995 |
| the deposit's 5 FE | **tested** — a shift-click on an inventory slot: 995 → 990, the view and `info` showing the 5 diamonds |
| the withdrawal's 5 FE | **tested** — a shift-click on the view: 990 → 985, the view empty and the inventory back to 5 |
| the item's gate | **tested** — at 0 FE the same deposit is refused `not enough item energy: 0 of 5 FE` (the stack stays, the network's count unchanged) and a use opens nothing; `SELFTEST=itemenergy` inverts the deposit |
| the item energy capability | **tested** — every charge, every payment and both commands go through `ITEM_ENERGY`/`EnergyHandler`; the SDK check counts it (`5 capabilitys`) |
| the `refinedstorage:energy` component | **tested** — written on the hand's stack (read back by `ctl state`), and a stack carrying none reads empty |
| `/refinedstorage grid`, `/refinedstorage charge` | **tested** — every readout above is their parsed reply (a recorded harness/behaviour addition, twins of `/refinedstorage energy`) |
| charging the grid in a creative hand | **tested** — the whole leg runs in creative, which is what found the exchange defect |
| a charger block that fills a grid (upstream's charger) | **not ported** — this package has no charger; the capability is registered, so such a block would fill the same component, but no block of this port fills it |
| the portable grid, the crafting/pattern grids | **not ported** — the next slices (see the report's `## Next slice`) |
| the rest of slice-8's list (creative controller, `isEnergyRequired`, the controller's gauge) | **unchanged** — still not ported, §"Slice 8" |

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

Evidence: `ports/_shared/evidence/refinedstorage/` — `plan-slice9-item-energy.md`,
`runtime-red-slice9-item-energy.log`, `sdk-test.log`, `sdk-clippy.log`, `sdk-fmt.log`,
`sdk-build-0.2.9-{1,2}.log`, `sdk-build-0.2.9-cmp.log`, `sdk-check-fits-0.2.9.log`,
`refinedstorage.hmod` (the delivered bytes) — and `ports/_shared/evidence/refinedstorage-e2e/` —
`e2e-slice9-green.log` (green), `e2e-slice9-control.log`, `e2e-slice9-selftest-itemenergy.log`,
`e2e-slice9-selftest-energy.log`, `e2e-slice9-selftest-networks.log`, and
`launcher-build-0.2.9.log` (`Compiling refinedstorage (1 of 1)`, `native-0a875da-a3aab5050286` — the
build every leg ran on).

## Slice 10 — a broken machine carries its own slots (2026-11-23) — version 0.2.10

### The gap lines this slice answers (verbatim)

- §"Slice 6" coverage: *"the transmitter's upgrade drops when the block is broken | **not ported** —
  the game's mod block entity drops an empty item (ARCHITECTURE §3's `block_drops` route is the fix, a
  later slice)"*.
- §"Slice 7" deviation 5: *"The machines' upgrade slots drop nothing when the block is broken … A
  filter item held in a filter slot is lost the same way."*

### What the pin actually does (measured, not assumed)

1. **The engine spills a mod block entity's declared container and drops the block's item empty.**
   `horizon-client/src/game/spill.rs::Pending::record` / `remove_entity` treats a
   `BlockEntity::Mod(m)` with `!m.items.is_empty()` and a declared `mod_container(state)` as
   `PreRemove::Contents`: every non-empty slot becomes an item entity. The block's own drop is built
   from its loot table and never learns the slots, because `block_entity_nbt::canonical()` answers
   `None` for a mod type id, so vanilla's `minecraft:block_entity_data` is never merged into the
   placed block entity.
2. That is visible in this port's own earlier log: `e2e-slice7-green.log:628`
   `exporter drops: {"minecraft:gold_ingot":1,"refinedstorage:exporter":1}` — slice 7's "the filter
   item is lost" was wrong; the filter **spilled**. What was lost is the *machine*: the item that
   dropped carried nothing, so a re-placed machine was empty.
3. **Upstream RS2 v2.0.9** (`BlockEntityWithDrops.getDrops()`,
   `AbstractImporterBlockEntity.getDrops()` → `upgradeContainer.getDrops()`): the four upgrades leave
   with the block item and the ghost filter container drops nothing. This port's filter slots hold
   real items (its deviation 1), so they travel too.

### What was implemented

- **`Hooks::break_block`** (server, before the removal): for an importer, exporter or wireless
  transmitter in a **non-creative** break, read the block entity's own slots, clear them in one
  transaction, and read them back. Only a verified-empty container records a carry; a refused write
  records nothing and the engine's ordinary spill stays the fallback — never both. Creative is skipped
  because a creative break drops no loot at all, so clearing would destroy the contents.
- **`Hooks::block_drops`** (server, level-read host): put a **fresh** payload on the block's own drop
  — `{id: "refinedstorage:network", RefinedStorageContents: [{Slot, id, count, components}]}` under
  vanilla `minecraft:block_entity_data`. It never copies the pre-removal snapshot (its `Items` list is
  what the engine already spilled) and never writes a raw `Items` list: a whole-compound copy is
  exactly the duplication that failed `actuallyadditions` #1557 in review, where the machine's
  inventory rode on the item twice.
- **`ItemPart::on_block_placed_by`** on the three items: read the placed stack's
  `minecraft:block_entity_data`, require its `id` to be this block entity's type, clamp the list to the
  container (`runtime::restore`) and write each slot in one transaction, then strip the payload from
  the held stack so a creative placement cannot restore it a second time. The restore lives here, not
  in the engine's component merge, because `canonical()` answers `None` for a mod type id.
- A pending carry that no drop consumed is pruned on `server_tick` after `CARRY_TTL` (4 ticks), so a
  later break at the same position can never inherit it.

### The rules as tests (red first)

`runtime.rs`: `Kind::carries_contents()` (importer/exporter/transmitter yes; controller, cable, drive,
grid no — a drive's disks spill as items, as upstream drops them), `Kind::own_slots()` (13/13/4/0),
`carry()` (only non-empty slots, each with its index; `None` for an empty container) and `restore()`
(an index at or past the container is dropped, a slot named twice keeps its first entry).

### Red proof — verbatim, before the glue

`cargo test --offline`, 2026-11-22 08:24, `runtime-red-slice10-drops.log`
(`ports/_shared/evidence/refinedstorage/runtime-red-slice10-drops.log`):

```
---- runtime::tests::the_slot_counts_are_what_the_content_registers stdout ----
thread 'runtime::tests::the_slot_counts_are_what_the_content_registers' (1674503) panicked at src/runtime.rs:2970:9:
assertion `left == right` failed: nine filters, four upgrades
  left: 0
 right: 13

failures:
    runtime::tests::a_doctored_carried_list_cannot_reach_past_the_container
    runtime::tests::a_machines_slots_travel_on_its_item_and_a_drives_do_not
    runtime::tests::an_empty_machine_carries_nothing
    runtime::tests::the_slot_counts_are_what_the_content_registers

test result: FAILED. 94 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out
```

The four new tests failed against the stubs and pass after the implementation (98 total).

### Green — the ladder and the in-game leg (one launcher build at `0a875da`)

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `rm -f Cargo.lock && cargo test` | **98 passed**, 0 failed (`sdk-test.log`, 0.2.10) |
| lints | `cargo clippy --all-targets --locked --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean, exit 0 (`sdk-fmt.log`) |
| SDK check | `cargo horizon-mod check` | *"Refined Storage uses 643 items of the API (5 Part kinds, 1 associated item, 5 capabilitys, 188 fields or variants, 23 hooks, 58 hook arguments, 2 macros, 195 methods, 33 traits, 133 types). Made for 0a875da."* (`sdk-check-0.2.10.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical** (`cmp`, `CMP_EXIT:0`), **203,749 B**, SHA-256 `41c9810a2d6c33a032abc935cad389b56ec5330cfb34141847c620cf017ba922`, **244 entries**, no `Cargo.lock` (`sdk-build-0.2.10-{1,2}.log`, `sdk-build-0.2.10-cmp.log`) |
| package check | `cargo horizon-mod check refinedstorage.hmod` | `0.2.10 (native-mod, format native-1/0a875da…/e59da909…, for 26.3) 203749 bytes`, `CHECK_FILE_EXIT:0` |
| fits (pin) | `… fits refinedstorage.hmod --build 0a875da` | `0a875da  made for`, `FITS_PIN_EXIT:0` |
| fits (other build) | `… fits refinedstorage.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_OTHER_EXIT:0` (both in `sdk-check-fits-0.2.10.log`) |
| launcher build | `launcher-game.sh build refinedstorage --rebuild` | exit 0, **`Compiling refinedstorage (1 of 1)`**, `native-0a875da-3bdf09dd1977` (`launcher-build-0.2.10.log`) |
| in-game leg | `build-slot.sh run … --step e2e -- tools/e2e.sh` | **PASS** + **`EXIT:0`** (`e2e-slice10-green.log`) |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME …` | **FAIL** + **`EXIT:1`** — *"the block at 41,64,1 is minecraft:air, expected facing=north"* (`e2e-slice10-control.log`) |
| counterfactual (this slice) | `SELFTEST=drops …` | **FAIL** + **`EXIT:1`** — *"the re-placed importer carried its slots back ([…4 speed…, stack…, dirt]); the empty drop it used to have is gone"* (`e2e-slice10-selftest-drops.log`) |
| selftest (slice-8 network gate) | `SELFTEST=energy …` | **FAIL** + **`EXIT:1`** — *"… got 'Wireless access at 46,64,20: refused (not enough energy: 0 of 16 FE)'"* (`e2e-slice10-selftest-energy.log`) |
| selftest (slice-5 promise rule) | `SELFTEST=networks …` | **FAIL** + **`EXIT:1`** — *"the grid's view is [{"count":3,"item":"minecraft:gold_ingot","slot":300}], expected []"* (`e2e-slice10-selftest-networks.log`) |
| selftest (slice-9 item gate) | `SELFTEST=itemenergy …` | **FAIL** + **`EXIT:1`** — *"expected the 0-FE deposit to have moved the stack, it is still in the inventory"* (`e2e-slice10-selftest-itemenergy.log`) |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | `logs=47 ok=47 missing-exit=0 bad-exit=0` |
| package gate | `verify-sdk-package.py refinedstorage` | **1/1** |

The new phase, quoted from `e2e-slice10-green.log` (the slice-7 machine network at z=16; the importer
at 43,64,16 filled directly, broken in survival, picked up and placed back):

```
== a broken machine carries its own slots on its item ==
  importer before the break: [{"slot":0,...speed_upgrade},{"slot":1,...speed_upgrade},{"slot":2,...speed_upgrade},{"slot":3,...speed_upgrade},{"slot":4,...stack_upgrade},{"slot":5,"item":"minecraft:dirt"}]
  importer drops: {"refinedstorage:importer":1}
  block(43,64,16): refinedstorage:importer[facing=down,open=false]
  importer after the re-place: [{"slot":0,...speed_upgrade},{"slot":1,...speed_upgrade},{"slot":2,...speed_upgrade},{"slot":3,...speed_upgrade},{"slot":4,...stack_upgrade},{"slot":5,"item":"minecraft:dirt"}]
  importer after a creative re-place: [{"slot":0,...speed_upgrade},{"slot":1,...speed_upgrade},{"slot":2,...speed_upgrade},{"slot":3,...speed_upgrade},{"slot":4,...stack_upgrade},{"slot":5,"item":"minecraft:dirt"}]
  importer after placing the stripped item again: []
== a broken transmitter carries its four upgrade slots ==
  transmitter before the break: [{"slot":0,...range_upgrade},{"slot":1,...range_upgrade},{"slot":2,...range_upgrade},{"slot":3,...range_upgrade}]
  transmitter drops: {"refinedstorage:wireless_transmitter":1}
  transmitter after the re-place: [{"slot":0,...range_upgrade},{"slot":1,...range_upgrade},{"slot":2,...range_upgrade},{"slot":3,...range_upgrade}]
```

(`...` abbreviates `"count":1,"item":`; the log has the full objects.) The differentials:

- **The drops list is the non-duplication proof.** Before this slice the same break answered
  `{"minecraft:gold_ingot":1,"refinedstorage:exporter":1}` (`e2e-slice7-green.log:628`); now the break
  answers `{"refinedstorage:importer":1}` — the six slot items are on the item *and* nowhere else. The
  phase asserts `drops[<each of the six>] == 0` item by item, so a payload that also spilled cannot
  pass.
- **The re-place assertion is index-exact.** `slots_indexed` reads `slot` as well as `item`/`count`, so
  a payload that restored the six items into the first free slots (0..5 in a different order) would
  fail: slot 4 must still be the stack upgrade and slot 5 the dirt.
- **The creative round is the anti-duplication proof for a kept stack.** A creative placement keeps
  the item in hand, so the phase breaks the machine again, picks the payload item up **in creative**,
  places it (restores the six), removes the block and places the *same* item a second time — the last
  line is `[]`. Without `strip_contents` the second machine lists the six again, which is the
  duplication that failed `actuallyadditions` #1557 in review.
- **`SELFTEST=drops`** asserts the *old* answer (an empty re-placed machine) and fails on this build,
  which is what makes the assertion fail-capable.
- The control run (the same Nightly with no package) fails at the first phase, so the leg is not
  passing on the engine alone.

### Port defect found by this slice's own leg (fixed here, not upstream's)

The first green run failed at the creative round with
*"the same item restored its slots a second time … the payload was not taken off the stack"*: a
creative placement restored the six slots once, and the item still carried them, so placing it again
restored a second machine. The cause was a guard in `strip_contents` — when this port's list was the
whole `minecraft:block_entity_data` value, stripping it left an **empty** component patch and the
function returned instead of writing the stripped stack. An empty patch is the right answer (it clears
the stack's components); removing that early return fixed it, and the assertion above is the proof
(`importer after placing the stripped item again: []`).

### Deviations and limits (recorded, not hidden)

1. **A creative break carries nothing.** `playerDestroy` is skipped in creative, so no loot is built
   and clearing the slots would destroy them; the engine's own spill still happens there (the
   contents fall out, the item is empty). Upstream drops them in creative too; a creative break is a
   build action, not a player keeping a machine.
2. **Only a player's break carries.** A piston, an explosion, `setblock … destroy` and a mob break go
   through the engine's spill (items on the ground, empty item), exactly as before this slice.
3. **A carry waits four ticks for its drop.** The pending entry is keyed by dimension and position and
   pruned on `server_tick`; a break whose `block_drops` never runs (a cancelled destruction) leaves
   the slots out of the world — the same outcome the engine gave, not a duplication.
4. **The payload is this port's own key.** `RefinedStorageContents` under `minecraft:block_entity_data`
   is a harness-side shape (the pin has no `BlockEntityWithDrops` API and merges no mod block entity
   data); a stack from another port's workspace cannot forge it without naming this block entity's id.
5. **The drive is unchanged.** Its nine disks spill as items, as upstream's `DiskDriveBlockEntity`
   drops them; the item is the drive, not the disks.

### Coverage (parity rule) — every family this slice ships

| family | state |
|---|---|
| the importer's 13 slots on a survival break | **tested** — four speed upgrades, a stack upgrade and a dirt entry are all listed after `ctl break`, at their own indices, and none is in `.drops` |
| the exporter's 13 slots | **tested** — the same `break_block`/`block_drops` route (the kind check names both); the slice-7 phase's exporter filter now travels (its drop answer no longer lists the gold) |
| the transmitter's 4 upgrade slots | **tested** — four range upgrades listed before and after the break, none spilled |
| restore on placement | **tested** — pickup, `ctl place` at the same position, `ctl container` lists every slot where it was |
| the item is not duplicated | **tested** — `.drops` has the block item only; the counterfactual (`SELFTEST=drops`) fails |
| a creative placement cannot restore twice | **tested** — the same payload item picked up and placed in creative restores once and is stripped; placing it again yields an empty machine (`importer after placing the stripped item again: []`) |
| a doctored slot index | **tested** — `restore` drops an index at or past the container (unit test `a_doctored_carried_list_cannot_reach_past_the_container`) |
| a non-player break (piston, explosion, `setblock destroy`) | **not ported** — engine spill, recorded above as deviation 2 |
| a creative break | **not ported** — recorded above as deviation 1 |
| the exporter's creative round | **not tested** — the three items share one Part and one code path, which the importer's round exercises; the exporter's survival break is tested |

### Harness facts learned here

- **`ctl place` right-clicks, and a click that lands on the placed machine opens its menu.** The next
  blocking command then answers `{"error":"the dead screen is open; close it first"}` once the block is
  gone (the menu outlives the block). Every `ctl place` in this phase is followed by
  `ctl close … || true`.
- **`ctl container X Y Z ITEM 1` fills the first *empty* slot, one call one slot** — it does not merge
  stacks, which is what lets the phase put four upgrades in four named slots and assert their indices
  instead of their count.
- **A dropped item is picked up by standing on its block position** (`tp` to the centre, `wait 20` for
  the 10-tick pickup delay), including in creative — a payload item can only be obtained by breaking
  the machine, never by `ctl give`.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

Evidence: `ports/_shared/evidence/refinedstorage/` — `plan-slice10-drops.md`,
`runtime-red-slice10-drops.log`, `sdk-test.log`, `sdk-clippy.log`, `sdk-fmt.log`,
`sdk-build-0.2.10-{1,2}.log`, `sdk-build-0.2.10-cmp.log`, `sdk-check-0.2.10.log`,
`sdk-check-fits-0.2.10.log`, `refinedstorage.hmod` (the delivered bytes) — and
`ports/_shared/evidence/refinedstorage-e2e/` — `e2e-slice10-green.log`, `e2e-slice10-control.log`,
`e2e-slice10-selftest-drops.log`, `e2e-slice10-selftest-networks.log`,
`e2e-slice10-selftest-energy.log`, `e2e-slice10-selftest-itemenergy.log`, and
`launcher-build-0.2.10.log` (`Compiling refinedstorage (1 of 1)`, `native-0a875da-3bdf09dd1977` — the
build every leg ran on).

## Slice 11 — the other storage disks: 4k, 16k, 64k and creative (2026-11-23) — version 0.2.11

### The gap lines this slice answers (verbatim)

- §"Slice 10" next-slice proposal: *"the other disk sizes (4k/16k/64k/creative)"*.
- §"Gaps / deviations (slice 1)" item 2: *"… fluid storage and disks, the other disk sizes (4k/16k/
  64k/creative), storage blocks, …"*, and the 0.2.10 manifest's last line: *"… and the other disk
  sizes are the port's gap list, not this version."*
- The slice's own note in the previous report: *"Disk capacity is fixed at 1k and the disk is one
  item"*.

### The reference (read, not guessed)

Upstream `refinedmods/refinedstorage2` v2.0.9, MIT, commit `0dec63affe8147702b020843c22748483ef18c01`
(`ports/_shared/upstream/refinedstorage/`):

| file | what it says |
|---|---|
| `common/storage/ItemStorageVariant.java` | `ONE_K("1k", 1024)`, `FOUR_K("4k", 4096)`, `SIXTEEN_K("16k", 16384)`, `SIXTY_FOUR_K("64k", 65536)`, `CREATIVE("creative", null)` — **the creative variant's capacity is `null`** |
| `common/storage/storagedisk/ItemStorageDiskItem.java` | `getCapacity()` is the variant's; `createStorage` hands the variant's capacity to `StorageTypes.ITEM.create(capacity, …)` |
| `common/storage/SameTypeStorageType.java` | `data.capacity().map(limited).orElseGet(plain)`: a **null** capacity builds a plain `PlatformStorage` — **no `LimitedStorage`, no `getCapacity()`**, so the creative disk is unbounded and reports no ceiling |
| `common/storage/LimitedPlatformStorage.java` | only the finite variants implement `LimitedStorage.getCapacity()` |
| `api/storage/StorageImpl.java` | `insert` returns the whole amount — **there is no per-operation cap** either |
| `resources/…/recipe/{4k,16k,64k}_storage_disk.json` | `GRG/RPR/EEE` with `P` = that tier's storage part (the 1k disk's own shape) |
| `resources/…/recipe/{4k,16k,64k}_storage_part.json` | `PEP/SRS/PSP`, `P` = basic/improved/advanced processor, `S` = the previous tier's part |
| `resources/…/recipe/*_storage_disk_from_storage_housing.json` | a second, shapeless route (housing + part) |
| `resources/…/recipe/*_storage_disk_upgrade.json` + `StorageContainerUpgradeRecipe` | the in-place upgrade: a smaller disk + the bigger part → the bigger disk, the source part returned, contents transferred |
| `CreativeModeTabItems.java` | the **creative disk is creative-tab only**: no recipe, and filtered out of the part list |

**The creative disk's semantics, as read** (not inferred from its name): unlimited stored items,
**no capacity reported** (it is not a `LimitedStorage`, so `getCapacity()` does not exist for it),
**no per-insertion ceiling**, and **no recipe**. It is the only disk whose capacity upstream leaves
`null`; this port's component stores that as `0`, the one spelling a number can give a `null`.

### What was implemented

- **`runtime::DiskCapacity`** — `Items(u64)` or `Unlimited`, with `room()` (`u64::MAX` for the
  creative disk, so `insert` never refuses an amount), `saved()`/`from_saved()` (0 = the creative
  marker), `clamp()` (a component can never give a disk more room than its item names) and `label()`
  (`"4096 items"` / `"unlimited"`).
- **`runtime::disk_capacity(item_id)`** — the five ids → their capacity, `None` for anything else
  (`storage_housing`, a stick). `CAPACITY_4K/16K/64K` sit beside `CAPACITY_1K`, all derived from it
  the way upstream writes `1024 * 4 * 4 * 4`.
- **`Disk`** carries a `DiskCapacity` (`Disk::new(n)` / `Disk::unlimited()`): `free()`, `insert()`
  and `trim()` honour it, so a saved 4k disk with 5,000 items is cut to 4,096 (before this slice
  every disk was cut to the 1k default), and a creative disk's `free()` is `u64::MAX - total`.
- **`support::StorageComponent`** now saves `{capacity, stacks}` in the **same**
  `refinedstorage:storage` component and still reads a 0.2.10 disk's bare list (no capacity → the
  item's own). `read_disk` takes the capacity from the item id, clamped by what the component says;
  `write_disk` records it.
- **New content, upstream's own ids**: `refinedstorage:{4k,16k,64k,creative}_storage_disk` and the
  three storage parts the disk recipes name. Each disk item declares its capacity as the component's
  **default** (`{"capacity": 4096}`), so a crafted or given disk is self-describing; the creative
  one declares `0`. All seven join the creative tab.
- **Recipes** (upstream's own shapes, adapted only to the pin's `"#tag"` string spelling): the three
  parts and the three disks from their part — six new recipes, each crafted in game.
- **`/refinedstorage info` on a drive** now names every disk and its room:
  `Disk drive at 42,64,12: 1 disk, capacity slot 0: refinedstorage:4k_storage_disk 4096/4096 items`,
  and `… creative_storage_disk 65536/unlimited` (the creative variant reports no ceiling).

### Port defect fixed (found by this slice's own first leg)

`grid_tick` marked **every** disk of its network dirty on every tick it worked, so the drive rewrote
each disk item every tick and any change made to a disk's component from outside (a `/data merge`,
another tool) was replaced by the stale cache one tick later — the first leg's fill vanished and the
four disk readouts stayed `0 items`. It now marks a disk dirty only when the view **changed** it
(`core.disks[key] != disk`), which is also what the module's own contract says ("the disk's item
component is the truth whenever the drive is loaded and not mid-write"). The in-game proof is the
fill itself: the same `/data merge` now reaches 4,096 and stays there.

### Harness repairs made while proving it (the tool, not the mod)

1. **`ctl` retries once**, and closes a screen when the refusal names one. Three consecutive legs
   aborted with no message (a single control request answering nothing under load, or a dead menu
   outliving a broken block), which cost three 15-minute runs. A retry cannot weaken an assertion:
   a wrong answer is still compared, and a genuinely broken command fails twice.
2. **`open_menu X Y Z N`** asks up to three times for a menu, because a right click is a
   press/release pair and occasionally does not open one on a loaded box; the slot-count assertion
   follows it unchanged.
3. **The transmitter's pickup stands on the drop's own position** (`55.5 64 5.5`, as the importer
   phase already did); one block away (`z = 6.5`) was outside the pickup reach and failed once.
4. **A block-grid deposit goes into a free view slot.** A view slot holds the network's own stack,
   and the engine *swaps* a cursor of another resource with it, which the tick reads back as a
   withdrawal — the first attempt "failed to accept" 63 items it had actually withdrawn. Only an
   empty slot is a deposit.

### The rules as tests (red first)

`runtime.rs`: `every_storage_disk_id_names_its_capacity`, `a_4k_disk_takes_four_times_what_a_1k_does`,
`the_creative_disk_has_no_ceiling`, `a_saved_fuller_than_capacity_disk_is_cut_to_its_own_room`,
`the_saved_spelling_marks_the_creative_disk_zero`,
`a_component_cannot_give_a_disk_more_room_than_its_item`, `a_capacity_reads_back_as_its_room`.

### Red proof — verbatim, before the glue

`cargo test`, 2026-11-23 06:0x, `runtime-red-slice11-disks.log`
(`ports/_shared/evidence/refinedstorage/runtime-red-slice11-disks.log`), against stubs that kept the
old behaviour in the new types (every disk a 1k disk, `0` read as 1,024, the component always winning):

```
---- runtime::tests::every_storage_disk_id_names_its_capacity stdout ----
thread 'runtime::tests::every_storage_disk_id_names_its_capacity' (118037) panicked at src/runtime.rs:2184:9:
assertion `left == right` failed
  left: Some(Items(1024))
 right: Some(Items(4096))
---- runtime::tests::the_creative_disk_has_no_ceiling stdout ----
assertion `left == right` failed: unlimited means no room count
  left: 1024
 right: 18446744073709551615
---- runtime::tests::the_saved_spelling_marks_the_creative_disk_zero stdout ----
assertion `left == right` failed: 0 is the only spelling upstream's null has in a component
  left: Items(1024)
 right: Unlimited
---- runtime::tests::a_component_cannot_give_a_disk_more_room_than_its_item stdout ----
assertion `left == right` failed
  left: Items(65536)
 right: Items(4096)
---- runtime::tests::a_capacity_reads_back_as_its_room stdout ----
assertion `left == right` failed
  left: "1024 items"
 right: "unlimited"

failures:
    runtime::tests::a_capacity_reads_back_as_its_room
    runtime::tests::a_component_cannot_give_a_disk_more_room_than_its_item
    runtime::tests::every_storage_disk_id_names_its_capacity
    runtime::tests::the_creative_disk_has_no_ceiling
    runtime::tests::the_saved_spelling_marks_the_creative_disk_zero

test result: FAILED. 100 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out
```

Honest note: two of the seven new tests (`a_4k_disk_takes_four_times_what_a_1k_does`,
`a_saved_fuller_than_capacity_disk_is_cut_to_its_own_room`) were **already green**: `Disk` always
took an arbitrary capacity, and what was missing was the id→capacity mapping, the unlimited variant
and the saved-capacity clamp. They stay as regression guards for the generic capacity.

### Green — the ladder and the in-game leg (one launcher build at `0a875da`)

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `rm -f Cargo.lock && cargo test` | **105 passed**, 0 failed (`sdk-test.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean, exit 0 (`sdk-fmt.log`) |
| SDK check | `cargo horizon-mod check` | *"Refined Storage uses 645 items of the API (…). Made for 0a875da."* (`sdk-check-0.2.11.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical** (`cmp`, `CMP_EXIT:0`), **210,866 B**, SHA-256 `e1c84b64a7804fdbce737ba63ed0b860840c140a43f5da26dbc2262b6c3f9ac4`, **257 entries**, no `Cargo.lock` (`sdk-build-0.2.11.log`) |
| package check | `cargo horizon-mod check refinedstorage.hmod` | `0.2.11 (native-mod, format native-1/0a875da…/e59da909…, for 26.3) 210866 bytes`, `CHECK_FILE_EXIT:0` |
| fits (pin) | `… fits refinedstorage.hmod --build 0a875da` | `0a875da  made for`, `FITS_PIN_EXIT:0` |
| fits (other build) | `… fits refinedstorage.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_OTHER_EXIT:0` (both in `sdk-check-fits-0.2.11.log`) |
| launcher build | `launcher-game.sh build refinedstorage --rebuild` | exit 0, **`Compiling refinedstorage (1 of 1)`**, `native-0a875da-25454b066fed` (`launcher-build-0.2.11.log`) |
| in-game leg | `build-slot.sh run … --step e2e -- tools/e2e.sh` | **PASS** + **`EXIT:0`** (`e2e-slice11-green.log`) |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME …` | **FAIL** + **`EXIT:1`** — *"the block at 41,64,1 is minecraft:air, expected facing=north"* (`e2e-slice11-control.log`) |
| counterfactual (this slice) | `SELFTEST=diskcap …` | **FAIL** + **`EXIT:1`** — *"expected the 4k disk to have accepted the 4,097th item (net_items 4096)"* (`e2e-slice11-selftest-diskcap.log`) |
| selftest (slice-5 promise rule) | `SELFTEST=networks …` | **FAIL** + **`EXIT:1`** — *"the grid's view is [{"count":3,"item":"minecraft:gold_ingot","slot":300}], expected []"* |
| selftest (slice-8 network gate) | `SELFTEST=energy …` | **FAIL** + **`EXIT:1`** — *"… refused (not enough energy: 0 of 16 FE)"* |
| selftest (slice-9 item gate) | `SELFTEST=itemenergy …` | **FAIL** + **`EXIT:1`** — *"expected the 0-FE deposit to have moved the stack, it is still in the inventory"* |
| selftest (slice-10 carry) | `SELFTEST=drops …` | **FAIL** + **`EXIT:1`** — *"the re-placed importer carried its slots back ([…]); the empty drop it used to have is gone"* |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | `logs=54 ok=54 missing-exit=0 bad-exit=0` |
| package gate | `verify-sdk-package.py refinedstorage` | **1/1** |

The new phase, quoted from `e2e-slice11-green.log` (a second network at z=12: controller 40, cable
41, drive 42, grid 43):

```
  info(43,64,12): … 1 disks, 0 items in 0 resources; energy 900/1000 FE, drawing 24/tick, t=10809
  info(42,64,12) drive: Disk drive at 42,64,12: 1 disk, capacity slot 0: refinedstorage:4k_storage_disk 0/4096 items
  info(42,64,12): … 1 disks, 4096 items in 1 resources; …
  info(42,64,12) drive: Disk drive at 42,64,12: 1 disk, capacity slot 0: refinedstorage:4k_storage_disk 4096/4096 items
  the 4k disk after a 1-item deposit: 4096 -> 4096 items
  info(42,64,12) drive: Disk drive at 42,64,12: 1 disk, capacity slot 0: refinedstorage:16k_storage_disk 16384/16384 items
  info(42,64,12) drive: Disk drive at 42,64,12: 1 disk, capacity slot 0: refinedstorage:64k_storage_disk 65536/65536 items
  the 64k disk refused the deposit: 65536 items
  info(42,64,12) drive: Disk drive at 42,64,12: 1 disk, capacity slot 0: refinedstorage:creative_storage_disk 65536/unlimited
  the creative disk after the same 1-item deposit: 65537 items
  info(42,64,0) drive: Disk drive at 42,64,0: 2 disks, capacity slot 0: refinedstorage:1k_storage_disk 3/1024 items, slot 1: refinedstorage:4k_storage_disk 4096/4096 items
```

and after `ctl save` + quit + relaunch (the existing slice-1 phase, extended):

```
  info(43,64,0): … 2 disks, 4099 items in 2 resources; …
  info(42,64,0) drive: Disk drive at 42,64,0: 2 disks, capacity slot 0: refinedstorage:1k_storage_disk 3/1024 items, slot 1: refinedstorage:4k_storage_disk 4096/4096 items
  grid view: [{"count":3,"item":"minecraft:gold_ingot","slot":300},{"count":64,"item":"minecraft:cobblestone","slot":301}]
```

### The assertions and their differentials

- **The fill is the 5,000-item component.** A 4k disk is written with 5,000 saved items and reads
  back 4,096 — the old code cut every disk to 1,024, so this line alone separates the two builds
  (and `SELFTEST=diskcap` asserts the opposite of the refusal, which fails).
- **The refusal is a 1-item deposit through the grid's own menu**, `net_items` read before and after
  a 20-tick window: 4,096 → 4,096 on the 4k and 65,536 → 65,536 on the 64k.
- **The accepted deposit is the same action on the creative disk**: 65,536 → 65,537, past the 64k
  ceiling, with `…/unlimited` in its readout — the pair is the capacity differential.
- **A refusal does not destroy the item**: the dirt left in the grid's own slot is clicked back and
  lands in the inventory (`count_of minecraft:dirt = 1`).
- **Each capacity is named by the drive's own info line**, not only by the total: `4096/4096`,
  `16384/16384`, `65536/65536`, `65536/unlimited`.
- **Across a relaunch** the 1k disk (3 gold) and the 4k disk (4,096 cobblestone) both keep their
  contents on one network: `2 disks, 4099 items in 2 resources`, the drive line naming both
  capacities.

### Recipes (the parity rule)

Six new recipes, each crafted in a real crafting table by `tools/recipe_check.py` (35 recipes now,
each with its wrong-ingredient control): `4k/16k/64k_storage_part` (four tier processors, two
quartz-enriched iron, three of the previous part, one redstone) and `4k/16k/64k_storage_disk` (the
1k disk's own `GRG/RPR/EEE` shape with its own part). The driver reads the package's own JSON, so a
carried recipe cannot be dead data.

### Deviations and gaps (recorded, not hidden)

1. **The from-housing route is not carried for the three larger disks.** Upstream has both a shaped
   disk recipe and a shapeless `housing + part` one; the port carries the shaped one (the 1k disk's
   pair was already carried in slice 2). One route per item, stated rather than duplicated.
2. **The in-place upgrade is not carried.** Upstream's `refinedstorage:storage_disk_upgrade` is a
   custom recipe type (`CraftingRecipeKind` here) that upgrades a smaller disk with the bigger part
   and **returns the smaller disk's own part** while transferring the contents; it is
   `not ported` — the pin can express it (`remaining_items`), but it needs its own decode/match/
   assemble code and its own in-game proof, so it is the next slice rather than half of this one.
3. **The creative disk's tooltip/help text** (`item.refinedstorage.creative_storage_disk.help`) and
   the disks' `fireResistant` property are not carried: the port has no tooltip text for any item and
   no fire-resistance test, so shipping either would be an unproven claim.
4. **The capacity is clamped by the item**, not taken on trust: a component that names 65,536 on a
   1k disk reads as 1,024 (unit test `a_component_cannot_give_a_disk_more_room_than_its_item`).
   Upstream has no component at all here, so this is the port's own safety rule, not a deviation
   from behaviour a player can see.
5. **The drive still shows nine slots** where upstream has eight (slice 1's recorded deviation).

### Coverage (parity rule) — every family this slice ships

| family | state |
|---|---|
| the five disk items | **tested** — each id is given/inserted, read back by `ctl container`, and its capacity read from the drive's info line |
| the 4k disk's capacity | **tested** — 5,000 saved items cut to 4,096; the 4,097th deposit refused with `net_items` flat over 20 ticks |
| the 16k and 64k disks' capacity | **tested** — 16,384 and 65,536 items, each named by its own readout; the 64k refuses the 1-item deposit |
| the creative disk | **tested** — 65,536 stored, the same deposit accepted (65,537), readout `…/unlimited`; item given and read back |
| a refused deposit | **tested** — the item comes back to the player (clicked out of the grid's slot into the inventory) |
| contents across a relaunch | **tested** — save, quit, relaunch: `2 disks, 4099 items`, both capacities named |
| the three storage parts and six recipes | **tested** — `tools/recipe_check.py` crafts all six with their declared output and count, consumes them, and its controls yield nothing |
| the creative disk's recipe | **not ported** — upstream ships none (creative-tab only) |
| the `*_from_storage_housing` duplicates | **not ported** — deviation 1 |
| the in-place disk upgrade | **not ported** this slice — deviation 2, `blocked(recipe-serializer)`; **closed in slice 12 (0.2.12)** |
| fluid disks | **not ported** — the fluid storage system is still a gap (slice 1 item 2) |

### Harness facts learned here

- **`/data merge block` reaches a mod block entity**: the engine decodes `Items` (with components)
  into the mod's slots, and `data get block` shows the tag back. It is how a disk is filled with
  4,096/16,384/65,536 items without thousands of clicks.
- **A view slot is not an empty slot**: a click with a *different* resource swaps with the network's
  stack and reads back as a withdrawal; a deposit must land on a free view slot.
- **A refused deposit stays in the grid's slot as the player's** (the slice-10 "owned slot" rule), so
  the slot must be emptied before the next deposit test.
- **The drive's info line is the per-disk readout**: `Disk drive at X,Y,Z: N disks, capacity slot 0:
  …`, which is what makes a per-disk capacity assertable at all.
- **`ctl` needs a retry on a loaded box**; without one, a leg can abort with no message at all.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

Evidence: `ports/_shared/evidence/refinedstorage/` — `plan-slice11-disks.md`,
`runtime-red-slice11-disks.log`, `sdk-test.log` (105), `sdk-clippy.log`, `sdk-fmt.log`,
`sdk-build-0.2.11.log` (two builds + `cmp`), `sdk-check-0.2.11.log`, `sdk-check-fits-0.2.11.log`,
`refinedstorage.hmod` (the delivered bytes) — and `ports/_shared/evidence/refinedstorage-e2e/` —
`e2e-slice11-green.log`, `e2e-slice11-control.log`, `e2e-slice11-selftest-diskcap.log`,
`e2e-slice11-selftest-{networks,energy,itemenergy,drops}.log`, and `launcher-build-0.2.11.log`
(`Compiling refinedstorage (1 of 1)`, `native-0a875da-25454b066fed` — the build every leg ran on).

## Slice 12 — the in-place disk upgrade (2026-11-23) — version 0.2.12

### The gap line this slice answers (verbatim)

§"Slice 11" deviation 2, which is also the coverage row it left
(`the in-place disk upgrade | not ported — deviation 2, blocked(recipe-serializer) for this slice`):

> **The in-place upgrade is not carried.** Upstream's `refinedstorage:storage_disk_upgrade` is a
> custom recipe type (`CraftingRecipeKind` here) that upgrades a smaller disk with the bigger part
> and **returns the smaller disk's own part** while transferring the contents; it is `not ported` —
> the pin can express it (`remaining_items`), but it needs its own decode/match/assemble code and
> its own in-game proof, so it is the next slice rather than half of this one.

### The route, verified at this pin (not assumed)

`HANDOVER.md` carries the warning *"the grid-kind pair (`CraftingRecipeKind`) crashes at load"*.
At **`0a875da`** that note is about the **machine** case (a recipe type read by a machine needs API
36's `register_recipe_type` + `register_raw_recipe_serializer`); a crafting-table kind is
`minecraft:crafting` and loads normally:

| evidence | what it measures |
|---|---|
| `ports/_shared/sdk/0a875da/crates/horizon-mod-vocab/src/parts/crafting_recipes.rs:162-169` | `register_crafting_recipe_serializer(name, kind)` → `add_recipe_serializer(name, "minecraft:crafting", handler)` — a **3×3 crafting-grid** kind, which is what an in-place disk upgrade is |
| the same file, `:79-105` | the trait: `decode`/`write`/`matches`/`assemble`/`remaining_items`, the last documented as *"One remainder per input cell … Asked once per server take, with the grid before consumption"* |
| `HANDOVER.md` §17b (2026-11-21) | *"A mod CAN register a custom recipe kind at `0a875da`"*; `pocketstorage` 0.2.5 authored its three recipes this way and crafted them in game |
| `HANDOVER.md` §"A machine's own recipe type is API 36" | the load crash is the *machine* registrar (`unregistered recipe serializer`), not this pair |
| **this slice's own in-game run** | `Compiling refinedstorage (1 of 1)`, the three `storage_disk_upgrade` files crafted in a real table, and the take's remainders applied (quotes below) — so the pair is safe here, and the survey's crash claim does not apply |

### The reference (read, not guessed)

Upstream `refinedmods/refinedstorage2` v2.0.9, MIT, commit
`0dec63affe8147702b020843c22748483ef18c01` (`ports/_shared/upstream/refinedstorage/`):

| file | what it says |
|---|---|
| `common/storage/StorageContainerUpgradeRecipeSerializer.java` | the recipe file is `{"type": "refinedstorage:storage_disk_upgrade", "to": "<variant>"}` — the pattern is **derived**, not written |
| `common/storage/StorageContainerUpgradeRecipe.java` | a **shapeless** pair derived from `to`: any *smaller* finite disk (`capacity < to.capacity`, and `Set.of()` when `to` has no capacity), plus `to.getStoragePart()`. `matches` accepts one of each; `assemble` builds `to`'s stack and `transferTo(fromDisk, toDisk)`; `getRemainingItems` puts `from.getVariant().getStoragePart()` in the **source disk's cell** |
| `common/storage/storagedisk/ItemStorageDiskItem.java` | `transferTo` = `helper.markAsToTransfer(from, to)`: the destination **item** decides the new capacity (the tooltip prints `transferring ? capacity : …`), the storage's contents are reused |
| `resources/…/recipe/{4k,16k,64k}_storage_disk_upgrade.json` | three files, each only `to`; there is no creative one |
| `common/storage/ItemStorageVariant.java` | the five variants and their parts (`1k`…`64k`, `creative` with `null` capacity) |

### What was implemented

- **`runtime::StorageVariant` / `STORAGE_VARIANTS`** — the four finite variants (name, disk id, part
  id, capacity); `disk_capacity` now reads the same table, so the capacity has one spelling.
- **`runtime::disk_variant` / `disk_of_variant` / `storage_part_of_disk`** — the id ↔ variant ↔ part
  mapping (the creative disk has no part; a recipe's `to` may still name it, it just never matches).
- **`runtime::upgrade_sources(to_disk)`** — the disks a destination accepts: the finite ones with
  less room; **empty** for the creative disk, mirroring upstream's `Set.of()` for a `null` capacity.
- **`runtime::upgrade_cells(cells, to_disk)`** — the grid rule: exactly one source disk and exactly
  one destination part, every other cell empty.
- **`runtime::upgraded_disk(source, to_disk)`** — the destination disk with **its own item's** room
  and the source's contents; `None` unless the destination is bigger (so the creative disk is never
  a source or a destination by construction).
- **`support::UpgradeKind`** (`impl CraftingRecipeKind`): `decode`/`write` the `{to}` object,
  `matches` through the item keys resolved once at registration (a craft-time match takes no host),
  `assemble` = the destination stack carrying `refinedstorage:storage` with the destination's
  capacity and the source's saved stacks, `remaining_items` = the source disk's own part in its cell.
- **Three recipe files** — upstream's own `{"type":"refinedstorage:storage_disk_upgrade","to":…}`.
- **`tools/recipe_check.py`** knows the kind: each of the three files is crafted in a real table, its
  output and count asserted, the source disk's own part asserted in the disk's cell after the take,
  and each file gets two controls (its own tier's disk — not smaller — and the source tier's own part
  — no recipe names it as a destination).
- **`tools/e2e.sh`** — the in-place upgrade with *contents*: a 4k disk holding 4,096 cobblestone
  taken out of a chest, crafted with a `16k_storage_part` in a table, the result shift-clicked, the
  crafted disk read (id, count, its component) before it goes into the drive that then reports
  `4096/16384 items` and a network total of 4,096; `SELFTEST=upgrade` asserts the source part was
  **not** returned and exits 1.

### The rules as tests (red first)

`runtime.rs`: `every_storage_disk_names_its_own_part`, `an_upgrade_takes_only_the_smaller_disks`,
`an_upgrade_grid_is_exactly_one_disk_and_its_part`,
`an_upgraded_disk_keeps_its_contents_with_the_new_room`,
`an_upgrade_refuses_a_disk_that_is_not_bigger`.

### Red proof — verbatim, before the glue

`ports/_shared/evidence/refinedstorage/runtime-red-slice12-upgrade.log`, against the new API's stubs
(the functions existed and answered "no rule": `None`/empty, so the tests failed on the missing
behaviour rather than on a missing symbol):

```
---- runtime::tests::every_storage_disk_names_its_own_part stdout ----
thread 'runtime::tests::every_storage_disk_names_its_own_part' (288290) panicked at src/runtime.rs:2417:9:
assertion `left == right` failed
  left: None
 right: Some("refinedstorage:1k_storage_part")
---- runtime::tests::an_upgrade_takes_only_the_smaller_disks stdout ----
thread 'runtime::tests::an_upgrade_takes_only_the_smaller_disks' (288282) panicked at src/runtime.rs:2453:9:
assertion `left == right` failed
  left: []
 right: ["refinedstorage:1k_storage_disk"]
---- runtime::tests::an_upgrade_grid_is_exactly_one_disk_and_its_part stdout ----
thread 'runtime::tests::an_upgrade_grid_is_exactly_one_disk_and_its_part' (288280) panicked at src/runtime.rs:2485:9:
assertion `left == right` failed
  left: None
 right: Some(UpgradeGrid { disk: 3, part: 4 })
---- runtime::tests::an_upgraded_disk_keeps_its_contents_with_the_new_room stdout ----
thread 'runtime::tests::an_upgraded_disk_keeps_its_contents_with_the_new_room' (288283) panicked at src/runtime.rs:2533:14:
a 4k disk upgrades into a 16k one

failures:
    runtime::tests::an_upgrade_grid_is_exactly_one_disk_and_its_part
    runtime::tests::an_upgrade_takes_only_the_smaller_disks
    runtime::tests::an_upgraded_disk_keeps_its_contents_with_the_new_room
    runtime::tests::every_storage_disk_names_its_own_part

test result: FAILED. 106 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out
```

Honest note: the fifth test (`an_upgrade_refuses_a_disk_that_is_not_bigger`) was **already green**
against the stubs, which answered `None` to everything. It stays as the regression guard for the
"only a bigger disk" rule.

### A tool defect the first leg found (recorded, fixed)

The recipe driver crashed on the new files with `KeyError: 'result'` — it read `recipe["result"]`
before looking at the kind, and a mod kind's file has no `result` (the destination *is* the recipe).
The result is now derived from `to` for that kind. The first version of the second control was also
wrong: "a 1k disk with a 16k part" **is** the 16k recipe's own pair, and the driver reported a real
16k disk for a control that was meant to prove nothing crafts (the 16k and 4k files both caught it).
The control is now the *source* tier's own part (a 1k disk + a 1k part), which no file names as a
destination. Both were tool bugs, not mod bugs; the two superseded legs' logs were not kept (their
`FAIL:` lines are quoted here and the driver's branch carries the fix), and every kept log is from
the corrected package.

### Green — the ladder and the in-game leg (one launcher build at `0a875da`)

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `rm -f Cargo.lock && cargo test` | **110 passed**, 0 failed (`sdk-test.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean, exit 0 (`sdk-fmt.log`) |
| SDK check | `cargo horizon-mod check` | *"Refined Storage uses 677 items of the API (…). Made for 0a875da."* (`sdk-check-0.2.12.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical** (`cmp`, `CMP_EXIT:0`), **216,906 B**, SHA-256 `2e46885e9759bfa7f88dc6344b99f107cafb9686f25ed8c7696ad6e69599b080`, **260 entries**, no `Cargo.lock` (`sdk-build-0.2.12.log`) |
| package check | `cargo horizon-mod check refinedstorage.hmod` | `0.2.12 (native-mod, format native-1/0a875da…/e59da909…, for 26.3) 216906 bytes`, `CHECK_FILE_EXIT:0` |
| fits (pin) | `… fits refinedstorage.hmod --build 0a875da` | `0a875da  made for`, `FITS_PIN_EXIT:0` |
| fits (other build) | `… fits refinedstorage.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_OTHER_EXIT:0` (both in `sdk-check-fits-0.2.12.log`) |
| recipe types | `python3 ports/tools/check-recipe-types.py refinedstorage` | `refinedstorage: ok` (`recipe-types.log`) |
| launcher build | `launcher-game.sh build refinedstorage --rebuild` | exit 0, **`Compiling refinedstorage (1 of 1)`**, `native-0a875da-e2f5ccd8f3ce` (`launcher-build-0.2.12.log`) |
| in-game leg | `build-slot.sh run … --step e2e -- tools/e2e.sh` | **PASS** + **`EXIT:0`** (`e2e-slice12-green.log`) — `38 of 38 carried recipes`, `36` controls |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME …` | **FAIL** + **`EXIT:1`** — *"the block at 41,64,1 is minecraft:air, expected facing=north"* (`e2e-slice12-control.log`) |
| counterfactual (this slice) | `SELFTEST=upgrade …` | **FAIL** + **`EXIT:1`** — *"expected the source 4k part not to be returned; this build returns it"* (`e2e-slice12-selftest-upgrade.log`) |
| selftest (slice-5 promise rule) | `SELFTEST=networks …` | **FAIL** + **`EXIT:1`** — *"the grid's view is [{"count":3,"item":"minecraft:gold_ingot","slot":300}], expected []"* |
| selftest (slice-8 network gate) | `SELFTEST=energy …` | **FAIL** + **`EXIT:1`** — *"Wireless access at 46,64,20: refused (not enough energy: 0 of 16 FE)"* |
| selftest (slice-9 item gate) | `SELFTEST=itemenergy …` | **FAIL** + **`EXIT:1`** — *"expected the 0-FE deposit to have moved the stack, it is still in the inventory"* |
| selftest (slice-10 carry) | `SELFTEST=drops …` | **FAIL** + **`EXIT:1`** — *"the re-placed importer carried its slots back ([…]); the empty drop it used to have is gone"* |
| selftest (slice-11 capacity) | `SELFTEST=diskcap …` | **FAIL** + **`EXIT:1`** — *"expected the 4k disk to have accepted the 4,097th item (net_items 4096)"* |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | `logs=62 ok=62 missing-exit=0 bad-exit=0` |
| package gate | `verify-sdk-package.py refinedstorage` | **1/1** |

### The new phase, quoted from `e2e-slice12-green.log`

```
== the in-place disk upgrade: a smaller disk and the bigger part make the bigger disk ==
  info(42,64,12): … 0 disks, 0 items in 0 resources; energy 900/1000 FE, drawing 20/tick, t=11906
  the filled 4k disk taken out of the chest: [{"components":"{\"refinedstorage:storage\":{capacity:4096,stacks:[{count:4096,id:\"minecraft:cobblestone\"}]}}","count":1,"item":"refinedstorage:4k_storage_disk","slot":0}]
  the upgrade recipe's result: {"components":"{\"refinedstorage:storage\":{capacity:16384L,stacks:[{count:4096,id:\"minecraft:cobblestone\"}]}}","count":1,"item":"refinedstorage:16k_storage_disk","slot":200}
  the crafting grid after the take: [{"count":1,"item":"refinedstorage:4k_storage_part","slot":100}]
  the crafted 16k disk in the inventory: [{"components":"{\"refinedstorage:storage\":{capacity:16384L,stacks:[{count:4096,id:\"minecraft:cobblestone\"}]}}","count":1,"item":"refinedstorage:16k_storage_disk","slot":8}]
  info(42,64,12): … 1 disks, 4096 items in 1 resources; energy 800/1000 FE, drawing 24/tick, t=12134
  info(42,64,12) drive: Disk drive at 42,64,12: 1 disk, capacity slot 0: refinedstorage:16k_storage_disk 4096/16384 items
```

### The three upgrade files, quoted from the recipe driver (same log)

```
recipe 4k_storage_disk_upgrade (refinedstorage:storage_disk_upgrade)
  4k_storage_disk_upgrade -> refinedstorage:4k_storage_disk x1 (from refinedstorage:1k_storage_disk + refinedstorage:4k_storage_part)
  4k_storage_disk_upgrade grid after the take: [('refinedstorage:1k_storage_part', 1)]
  4k_storage_disk_upgrade with its own tier's disk (control) -> None x0
  4k_storage_disk_upgrade with the source tier's own part (control) -> None x0
recipe 16k_storage_disk_upgrade (refinedstorage:storage_disk_upgrade)
  16k_storage_disk_upgrade -> refinedstorage:16k_storage_disk x1 (from refinedstorage:4k_storage_disk + refinedstorage:16k_storage_part)
  16k_storage_disk_upgrade grid after the take: [('refinedstorage:4k_storage_part', 1)]
  … both controls -> None x0
recipe 64k_storage_disk_upgrade (refinedstorage:storage_disk_upgrade)
  64k_storage_disk_upgrade -> refinedstorage:64k_storage_disk x1 (from refinedstorage:16k_storage_disk + refinedstorage:64k_storage_part)
  64k_storage_disk_upgrade grid after the take: [('refinedstorage:16k_storage_part', 1)]
  … both controls -> None x0
```

### The assertions and their differentials

- **The crafted disk is the destination, with the destination's room.** The result slot's own
  component reads `capacity:16384` while the source said 4,096 — a build that kept the source's
  capacity would read 4096/4096, and the drive's line is the second witness.
- **The contents came across.** The same component carries `count:4096` cobblestone, which the source
  disk's component held.
- **The source part comes back.** After the take the *crafting grid* holds
  `refinedstorage:4k_storage_part` in slot 100 — the cell the disk left — and no disk; the
  counterfactual (`SELFTEST=upgrade`) asserts the opposite and fails.
- **Only a bigger disk upgrades.** Each of the three files has two controls that craft nothing (its
  own tier's disk; the source tier's own part), so the match rule is exercised in both directions.
- **The drive reads the disk's own capacity**: `4096/16384 items`, and the network totals 4,096 —
  the upgraded disk is a real 16k disk, not a 4k disk with a bigger label.

### Deviations and gaps (recorded, not hidden)

1. **The match is strict where upstream's is lenient.** `StorageContainerUpgradeRecipe.matches`
   returns true as soon as it has seen one valid disk and one part, ignoring other grid cells; this
   port requires every other cell to be empty. A shapeless recipe of two ingredients accepts no
   third item, and upstream's leniency would let a player park a stack in the grid while upgrading.
   A user notices nothing legitimate: the same pair in the same table crafts the same disk.
2. **The creative disk is never a destination or a source** — upstream's `Set.of()` for a `null`
   capacity; there is no `creative_storage_disk_upgrade` file, and the unit test pins the empty
   source list.
3. **The from-housing duplicate route** for the three larger disks is still not carried (slice 11,
   deviation 1). Unchanged.
4. **The recipe book does not show the upgrade** (a mod crafting kind has no placement info in this
   package) — the same `blocked(PlacementInfo)` state `pocketstorage` records for its kind. The
   recipe crafts regardless.
5. The upgraded disk's **component records the destination's capacity** rather than upstream's
   storage-object reuse: upstream's disk contents live behind a UUID in a world-level repository,
   this port's live in the item. The behaviour a player sees is the same (contents kept, new room).

### Coverage (parity rule) — every family this slice ships

| family | state |
|---|---|
| the three `*_storage_disk_upgrade` files | **tested** — each crafts in a real table with its destination's own pair, output and count asserted, ingredients consumed |
| the source disk's part returned | **tested** — the returned part is asserted in the disk's own cell after the take; `SELFTEST=upgrade` fails on a build that does not return it |
| the contents transfer | **tested** — the phase's 4k disk (4,096 cobblestone) makes a 16k disk whose component carries the stacks |
| the destination's capacity | **tested** — `capacity:16384` in the crafted component and `4096/16384 items` from the drive, then 4,096 as the network total |
| a same-tier disk | **tested** — driver control crafts nothing (all three files) |
| a smaller disk with its own part | **tested** — driver control crafts nothing (all three files) |
| only a bigger disk is accepted | **tested** — unit test `an_upgrade_refuses_a_disk_that_is_not_bigger` |
| the creative disk as a destination | **tested** — unit test `an_upgrade_takes_only_the_smaller_disks` (empty) and `upgrade_cells` returns `None` |
| the recipe book entry | `blocked(PlacementInfo)` — a mod kind has no placement info; the recipe crafts |
| fluid disks' upgrades | **not ported** — the fluid storage system is still a gap (slice 1 item 2) |

### Harness facts learned here

- **A crafting table's menu is not a container's**: its grid is `100-108` with the result at `200`,
  so `open_menu X Y Z N` (which counts `300+`) never opens one; the phase waits for the result slot
  instead, as `recipe_check.py` does.
- **A chest hands a component-carrying stack to a player**: `/data merge block … {Items:[…]}`
  decodes `components`, and clicking the chest's slot into the inventory works with the components
  intact (the disk's 4,096 cobblestone survived the trip).
- **A mod `generic_9xN` menu accepts a click on `300+`**: the crafted disk clicked into the drive's
  own menu slot 300 landed in the drive and the drive read it — the first time this port has clicked
  a *mod* container's slot rather than only reading it.
- **The engine applies a mod kind's `remaining_items` on the take**: the returned part appeared in
  the cell the disk left, which is the platform fact the pin's docs describe and this port now has
  evidence for.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

Evidence: `ports/_shared/evidence/refinedstorage/` — `plan-slice12-upgrade.md`,
`runtime-red-slice12-upgrade.log`, `sdk-test.log` (110), `sdk-clippy.log`, `sdk-fmt.log`,
`sdk-build-0.2.12.log` (two builds + `cmp`), `sdk-check-0.2.12.log`, `sdk-check-fits-0.2.12.log`,
`recipe-types.log`, `refinedstorage.hmod` (the delivered bytes) — and
`ports/_shared/evidence/refinedstorage-e2e/` — `e2e-slice12-green.log`, `e2e-slice12-control.log`,
`e2e-slice12-selftest-upgrade.log`,
`e2e-slice12-selftest-{networks,energy,itemenergy,drops,diskcap}.log`, and
`launcher-build-0.2.12.log` (`Compiling refinedstorage (1 of 1)`, `native-0a875da-e2f5ccd8f3ce` —
the build every leg ran on).

## Slice 13 — review round two: the hand, the completion gate and the operator (2026-11-23) — version 0.2.13

### The three blockers this slice answers (verbatim from the review)

1. *"**Placement and insertion can lose or duplicate items.** Update the hand that placed the
   machine and preserve the payload on unplaced units in a stack. For drive insertion, commit the
   disk insertion and the consumption from the acting hand together."*
2. *"**An interrupted break can discard upgrades or filters.** Leave the normal container spill in
   place unless the completed break transfers those contents to an emitted machine item."*
3. *"**Energy writes need authorization.** Require operator permission for both mutating commands,
   and verify the held item is the Refined Storage wireless grid before applying a charge."*

### What 0.2.12 did, measured rather than assumed

- **Placement.** `CarriedContents::on_block_placed_by` ran `strip_contents`, which wrote the
  **pre-shrink** stack (`on_block_placed_by` runs *before stack consumption*, so `event.stack` is
  the stack the engine is about to take one from) with its payload removed into the **main hand**
  (`Hand::Main`) whatever hand placed the block. A survival placement of one unit of a stack of two
  therefore left the other unit with no payload — its contents destroyed — and a placement from the
  off hand rewrote the main hand's item instead of the off hand's (that item destroyed, the
  off-hand machine duplicated).
- **Insertion.** `on_right_click_block` inserted the disk in one transaction and took it from the
  hand in a *second* write, from `player.selected_slot` rather than the click's own `hand`. A refused
  hand write left the disk in the drive **and** in the hand; a click from the off hand decremented
  the main hand's stack instead.
- **Break.** `Hooks::break_block` cleared the machine's slots *before the break was known to
  complete*. A break that then did not hand an item over — cancelled by another listener, a loot
  list that lost the machine item, a creative break with a loot path — had already emptied the
  container: the carry was pruned after `CARRY_TTL` and nothing spilled, so both mechanisms skipped.
- **Energy.** `/refinedstorage energy` and `/refinedstorage charge` were level-0 commands, and
  `charge_grid` asked the item energy capability without naming the item it was filling.

### What was implemented

- **`runtime::placed_hand(creative, count)`** — the placement rule: survival leaves the hand alone
  (the engine takes the placed unit; the units it did not place keep their own payload); creative,
  which keeps the stack, strips the spent payload only when the placed unit was the whole stack
  (`StripPlaced`), and leaves it when other units still carry theirs (`UnplacedKeep`).
- **`CarriedContents::on_block_placed_by`** reads that rule; **`strip_contents`** finds the hand
  whose stack **is** the placed stack (`hand_holding`: hotbar slot, then the off hand at 40) instead
  of assuming the main hand.
- **`runtime::after_taking_one(count)`** — the count a hand keeps after one item leaves it, or
  `None` for the last one. `insert_disk` now does the drive slot write **and** the hand's decrement
  in **one transaction** (`DiskRefusal` aborts both), consuming from `event.click.hand`; the click
  says which of "inserted / full / did not go in" it was.
- **`runtime::break_plan(drops_loot)`** — the block's own slots are taken only at the *completion
  gate*, `BlockPart::on_destroyed_by_player`, whose `will_harvest` is the platform's "will this break
  hand an item over" (never in creative, false for a tool the block refuses). `Hooks::break_block`
  is gone from the port: every other break leaves the container untouched and the engine's ordinary
  spill (`PreRemove::Contents`) is the only copy.
- **`runtime::carry_plan(item_in_drops)`** — `block_drops` puts the payload on the machine item when
  it is in the list, and otherwise pushes the taken slots back as their own drops (`ItemDrop` at the
  block's centre, vanilla's ten-tick pickup delay), so a break either transfers the contents or drops
  them, never neither and never both.
- **`runtime::may_write_energy(level)`** with `ENERGY_LEVEL = 2`: both mutating commands are
  `Command::op(…, 2)` **and** re-check `event.source.permission()` in their bodies. `charge_grid`
  requires `stack.item() == keys().wireless_grid` before it asks for the capability.

### The rules as tests (red first)

`runtime.rs`: `a_placement_keeps_the_payload_on_the_units_it_did_not_place`,
`one_disk_costs_one_item_from_the_hand_that_held_it`,
`a_break_carries_its_slots_only_when_it_hands_over_an_item`,
`a_carry_with_no_item_to_ride_falls_back_to_the_ground`, `only_an_operator_may_write_energy`.

### Red proof — verbatim, before the glue

`ports/_shared/evidence/refinedstorage/runtime-red-slice13-review2.log`, against stubs that kept the
0.2.12 answers (`StripPlaced` always, `Some(count)` for the hand, `Carry` always, `OnTheItem` always,
`may_write_energy` always true):

```
---- runtime::tests::a_break_carries_its_slots_only_when_it_hands_over_an_item stdout ----
thread 'runtime::tests::a_break_carries_its_slots_only_when_it_hands_over_an_item' (374049) panicked at src/runtime.rs:3735:9:
assertion `left == right` failed: a break that drops nothing leaves the engine's spill alone
  left: Carry
 right: Spill
---- runtime::tests::a_carry_with_no_item_to_ride_falls_back_to_the_ground stdout ----
thread 'runtime::tests::a_carry_with_no_item_to_ride_falls_back_to_the_ground' (374052) panicked at src/runtime.rs:3745:9:
assertion `left == right` failed: no machine item to carry them: they drop, they are not discarded
  left: OnTheItem
 right: BackOnTheGround
---- runtime::tests::a_placement_keeps_the_payload_on_the_units_it_did_not_place stdout ----
thread 'runtime::tests::a_placement_keeps_the_payload_on_the_units_it_did_not_place' (374083) panicked at src/runtime.rs:3689:9:
assertion `left == right` failed: survival takes the placed unit; the second unit keeps its own payload
  left: StripPlaced
 right: EngineConsumes
---- runtime::tests::one_disk_costs_one_item_from_the_hand_that_held_it stdout ----
thread 'runtime::tests::one_disk_costs_one_item_from_the_hand_that_held_it' (374136) panicked at src/runtime.rs:3714:9:
assertion `left == right` failed: the last disk leaves an empty hand
  left: Some(1)
 right: None
---- runtime::tests::only_an_operator_may_write_energy stdout ----
thread 'runtime::tests::only_an_operator_may_write_energy' (374138) panicked at src/runtime.rs:3755:9:
every player may not charge

test result: FAILED. 110 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

### Green — the ladder and the in-game leg (one launcher build at `0a875da`)

| Step | Command | Result |
|---|---|---|
| runtime + glue tests | `rm -f Cargo.lock && cargo test` | **115 passed**, 0 failed (`sdk-test.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| formatting | `rustfmt --edition 2024 --check src/{lib,support,runtime}.rs` | clean, exit 0 (`sdk-fmt.log`) |
| SDK check | `cargo horizon-mod check` | *"Refined Storage uses 680 items of the API (…). Made for 0a875da."* (`sdk-check-0.2.13.log`) |
| package | `rm -f Cargo.lock && cargo horizon-mod build` ×2 | **byte-identical** (`cmp`, `CMP_EXIT:0`), **221,841 B**, SHA-256 `92858557af7dcf5bd4c88ce4f43a62fc666605c099bc373cfa8a4baa390cc21f`, **260 entries**, no `Cargo.lock` (`sdk-build-0.2.13-{1,2}.log`) |
| package check | `cargo horizon-mod check refinedstorage.hmod` | `0.2.13 (native-mod, format native-1/0a875da…/e59da909…, for 26.3) 221841 bytes`, `CHECK_FILE_EXIT:0` |
| fits (pin) | `… fits refinedstorage.hmod --build 0a875da` | `0a875da  made for`, `FITS_PIN_EXIT:0` |
| fits (other build) | `… fits refinedstorage.hmod --build 0b14f8a` | `0b14f8a  fits`, `FITS_OTHER_EXIT:0` (both in `sdk-check-fits-0.2.13.log`) |
| launcher build | `launcher-game.sh build refinedstorage --rebuild` | exit 0, **`Compiling refinedstorage (1 of 1)`**, `native-0a875da-7befb00f6e88` (`launcher-build-0.2.13.log`) |
| in-game leg | `build-slot.sh run … --step e2e -- tools/e2e.sh` | **PASS** + **`EXIT:0`** (`e2e-slice13-green.log`) |
| control (same Nightly, no package) | `GAME=$CONTROL_GAME …` | **FAIL** + **`EXIT:1`** — *"the block at 41,64,1 is minecraft:air, expected facing=north"* (`e2e-slice13-control.log`) |
| counterfactual (this slice's placement) | `SELFTEST=payload …` | **FAIL** + **`EXIT:1`** — *"the second machine came back with […6 slots…]: the hand kept the unplaced unit's payload, so 0.2.12's strip would have destroyed it"* (`e2e-slice13-selftest-payload.log`) |
| counterfactual (this slice's break) | `SELFTEST=break …` | **FAIL** + **`EXIT:1`** — *"the creative break spilled its four speed upgrades ({"refinedstorage:speed_upgrade":6,"refinedstorage:stack_upgrade":1}): the engine's spill is intact, so this inverted assertion fails as it must"* (`e2e-slice13-selftest-break.log`) |
| selftest (slice-7 stack upgrade, re-run) | `SELFTEST=stack …` | **FAIL** + **`EXIT:1`** — *"with a stack upgrade the importer moved to 64 items, expected 5"* (`e2e-slice13-selftest-stack-upgrade.log`) |
| selftest (slice-10 carry) | `SELFTEST=drops …` | **FAIL** + **`EXIT:1`** — *"the re-placed importer carried its slots back ([…6…]); the empty drop it used to have is gone"* (`e2e-slice13-selftest-drops.log`) |
| log gate | `check-e2e-logs.py refinedstorage-e2e --strict` | `logs=68 ok=68 missing-exit=0 bad-exit=0` |
| package gate | `verify-sdk-package.py refinedstorage` | **1/1** |

### The new phases, quoted from `e2e-slice13-green.log`

```
== placing one of a stack of two keeps the second unit's payload ==
  the machine that will ride a stack of two: [{"slot":0,"count":1,"item":"refinedstorage:speed_upgrade"},…,{"slot":5,"count":1,"item":"minecraft:dirt"}]
  the hand before the placement: {"count":2,"item":"refinedstorage:importer"}
  the hand after placing one of the two: {"count":1,"item":"refinedstorage:importer"}
  the machine placed from the stack: [{"slot":0,…speed_upgrade},…,{"slot":5,…dirt}]
  the second machine, from the same payload: [{"slot":0,…speed_upgrade},…,{"slot":5,…dirt}]
== a creative break spills its slots and carries nothing ==
  the machine before the creative break: [{"slot":0,…speed_upgrade},{"slot":4,…stack_upgrade}]
  creative break drops: {"refinedstorage:speed_upgrade":4,"refinedstorage:stack_upgrade":1}
  creative break on the ground: [{"count":1,…speed_upgrade},{"count":1,…speed_upgrade},{"count":1,…speed_upgrade},{"count":1,…speed_upgrade},{"count":1,…stack_upgrade}]
== the energy writes need an operator, and the charge names the grid ==
  the owner's item buffer: 1000/1000 FE
  LAN port 46339 (guest commands off: a guest joins at level 0)
  guest connect: {"ok":true}
  the level-0 guest runs /refinedstorage info: No Refined Storage network block there.
  the level-0 guest runs /refinedstorage charge 1000 -> Incorrect argument for command
  the level-0 guest runs /refinedstorage energy 48 64 8 1000 -> Incorrect argument for command
  the owner's item buffer after the guest leg: 1000/1000 FE
  /refinedstorage charge 1000 -> The item in the main hand is not a wireless grid.
  the hand after charging a stick: {"count":1,"item":"minecraft:stick"}
```

(`…` abbreviates `"count":1,"item":`; the log has the full objects. The two machines' slot lists are
identical and index-exact, from `slots_indexed`.)

### The assertions and their differentials

- **The stack of two is the reviewer's own case.** Two identically filled importers are broken in
  survival; their items merge into one stack of two; placing **one** leaves **one** item, and that
  item is proven to still carry its payload by the *second* machine listing all six slots at their
  own indices. On 0.2.12 the second machine comes back empty (that is what `SELFTEST=payload`
  asserts, and it fails).
- **The client's inventory mirror does not show this item's components** (measured: a payload item
  reads as a bare `refinedstorage:importer` while its re-placement restores every slot), so the
  proof is behavioural — a placement that restores the six slots could not have come from a unit
  without them — rather than a string match on the components.
- **The creative break is the "does not complete" half.** The same machine, the same five slot
  items, in creative: the engine's spill puts all five on the ground, no machine item is emitted
  (creative drops no loot) and the player holds none. `SELFTEST=break` asserts the opposite
  (spilled nothing) and fails.
- **The two mechanisms cannot both skip or both fire**: a survival break reports
  `drops: {"refinedstorage:importer":1}` with `items_left_on_ground: 0` (payload only, checked by
  six `drops[<slot item>] == 0` assertions in the slice-10 phase, still green), while the creative
  break reports the five items and no machine.
- **The operator gate has its own differential.** The non-operator this harness can actually produce
  is a **LAN guest at level 0** (`ctl lan open 0` + a second game instance on `$GSOCK`): the same
  guest runs this mod's level-0 `/refinedstorage info` first, and only then are its refusals of
  `/refinedstorage charge` and `/refinedstorage energy` evidence. Both are refused
  (`Incorrect argument for command`, the level-2 nodes are not in a level-0 tree) and the owner's
  item buffer is unchanged (1000/1000) across the guest leg. The owner's own charge works in the
  same phase, and a stick in the main hand is refused by name (`The item in the main hand is not a
  wireless grid.`) with no `refinedstorage:energy` component written.
- **`/deop` is not the lever** in this harness: a single-player world's owner keeps permission 4
  (`owner_permission(allow_commands)`), so a de-opped owner still charged — measured on this build
  (`Wireless grid energy: 1000/1000 FE (was 995)` after the deop), which is exactly the false
  positive another port's notes warn about (the refusal they once matched was the failed `/deop`'s
  own). The LAN guest is the route that works.

### Deviations and limits (recorded, not hidden)

1. **A creative placement of a multi-unit stack keeps the payload.** With two units in the hand the
   engine consumes none, and only one of them has spent its payload; stripping the stack would
   destroy the other's. The single-unit creative placement is still stripped (the slice-10
   anti-duplication rule), which is the case the harness can place.
2. **The fallback drop is not reachable in game.** A player's break of this block always emits its
   own item (the loot table is unconditional and `requires_tool` is false), so `block_drops`'s
   "no machine item in the list" branch is a guard for a loot list another listener empties; it is
   covered by the unit test `a_carry_with_no_item_to_ride_falls_back_to_the_ground`, not by a leg.
3. **`/refinedstorage energy` and `/refinedstorage charge` are operator commands** (level 2). They
   are the harness's stand-ins for an FE cable and a charger, as recorded since slice 8/9; the
   trade-off is that a world without cheats cannot refill a buffer at all, which is the same
   situation as before (the commands existed and any player could run them).
4. **The drive still shows nine slots** (slice 1's deviation), and the insertion still requires the
   click's hand to hold the disk: a click with an empty hand is the block's own.

### Coverage (parity rule) — every family this slice ships

| family | state |
|---|---|
| placement from a stack of two (survival) | **tested** — one item placed, one left; both machines list the six slots where they were |
| placement from a single unit (survival and creative) | **tested** — slice 10's phase: survival re-place restores, creative re-place restores once and strips |
| the hand that placed (off hand) | **not reachable by the harness** — `ctl` places from the hotbar only; the code finds the hand (`hand_holding`) instead of assuming the main one, and the rule it feeds is unit-tested |
| a disk into the drive (one transaction) | **tested** — the slice-1 and slice-7 phases insert disks and read them back from the drive; a refused click leaves both sides alone (the retry prints the refusal) |
| the disk consumer's hand (off hand) | **not reachable by the harness** — `ctl interact` clicks with the main hand; the code consumes from `event.click.hand` |
| a survival break carrying its slots | **tested** — slice 10's phase (four speed, a stack, a dirt entry on the item; nothing spilled) |
| a break that hands no item over (creative) | **tested** — the five slot items spill, no machine item is emitted, the player holds none |
| a break that hands no item over (cancelled / no loot) | `blocked(harness:no-cancelled-break)` — the completion gate is `on_destroyed_by_player`'s `will_harvest`, unit-tested; no ctl step can cancel a break |
| a carry with no item to ride | **not reachable in game** (deviation 2) — unit-tested |
| operator-only energy writes (charge) | **tested** — a level-0 LAN guest is refused, the owner's charge works |
| operator-only energy writes (network) | **tested** — the same guest is refused `/refinedstorage energy` and the network buffer never reaches 1000 |
| the charge names the item | **tested** — a stick is refused by name and gains no energy component |
| every earlier family (slices 1–12) | **tested** — the same leg, unchanged, all green |

### Harness facts learned here

- **A single-player world's owner cannot be de-opped into a lower permission**: `registries.permission`
  is the owner's level from Allow Commands, so `/deop` succeeds and level-2 commands still run. The
  LAN guest (`ctl lan open 0`, a second game on `$GSOCK`, `gctl connect`) is the only level-0 source,
  and it needs its own level-0 differential before its refusals mean anything.
- **A level-0 guest's refusal of a level-2 mod command reads `Incorrect argument for command`** (the
  node is not in its tree), not "unknown command" — grep for both.
- **`ctl state` carries no player name** (`ctl status` does: `.server.name`), and its reply is
  shorter while a screen is up.
- **The client's inventory mirror does not show a mod item's components** for this port's payload
  items; assert the behaviour (a re-placement) instead.
- **`ctl look-at block X Y Z FACE` can fail** ("no face is visible") where `ctl break`'s own aiming
  succeeds; it is a hint, so it is called with `|| true`.
- **A machine that neighbours a full block is a coin flip to aim from one side** (the importer's west
  face is coplanar with the drive at 42,64,16 — `ctl break` reported "disk_drive is in the way"); aim
  a visible face first, or break from the other side.
- **The break's `.drops` window covers three blocks**, so a stray item entity from an earlier phase
  joins the count (a creative break once reported five speed upgrades for four). Kill item entities
  before a phase whose drop *sums* are asserted, and assert `>=` on a per-item count.
- **Two payload items with identical contents merge into one stack** on pickup, which is what makes
  the stack-of-two case reachable at all.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

Evidence: `ports/_shared/evidence/refinedstorage/` — `plan-slice13-review2.md`,
`runtime-red-slice13-review2.log`, `sdk-test.log` (115), `sdk-clippy.log`, `sdk-fmt.log`,
`sdk-build-0.2.13-{1,2}.log` (two builds + `cmp`), `sdk-check-0.2.13.log`,
`sdk-check-fits-0.2.13.log`, `refinedstorage.hmod` (the delivered bytes) — and
`ports/_shared/evidence/refinedstorage-e2e/` — `e2e-slice13-green.log`, `e2e-slice13-control.log`,
`e2e-slice13-selftest-{payload,break,stack-upgrade,drops}.log`, and `launcher-build-0.2.13.log`
(`Compiling refinedstorage (1 of 1)`, `native-0a875da-7befb00f6e88` — the build every leg ran on).
