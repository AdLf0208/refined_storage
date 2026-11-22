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

Status: **slice 7 — the importer and the exporter (0.2.7)** — the network core of slice 1, the
crafting chain of slice 2, the finite-range, same-dimension transmitter of slice 3, the item that
asks its admission (slice 4), the promise table keyed by network (slice 5), the transmitter's four
upgrade slots with the range and creative range upgrades (slice 6), and now the two transfer
machines: a filter, four upgrade slots, a work tick and a menu each, with the speed and stack
upgrades they make real. Everything else is the gap list below, not a claim.

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
   grids, autocrafting, fluid storage and disks, the other disk sizes (4k/16k/
   64k/creative), storage blocks, ~~the range and creative range upgrades~~
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
