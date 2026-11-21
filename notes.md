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

Status: **slice 2 — the crafting chain (0.2.2)** — the network core of slice 1 plus the
Refined Storage crafting chain it needs (14 items and the `machine_casing` block) and the
21 recipes that reach the five shipped blocks/items, every one of them crafted in game (see
`## Slice 2` at the end). Everything else is the gap list below, not a claim.

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
   security manager, wireless transmitter/grid, portable grid, crafting/pattern
   grids, autocrafting, fluid storage and disks, the other disk sizes (4k/16k/
   64k/creative), storage blocks, upgrades, the config file, and the coloured
   variants of every block (the slice has the default colour only).
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
| the rest of the jar (crafters, exporters/importers, external storage, interfaces, relays, security, wireless, fluid storage, the other disk sizes, coloured variants, upgrades) | **not ported** — slice-1 gap 2, unchanged |

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
