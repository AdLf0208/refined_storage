# Refined Storage 2.0.9 — upstream analysis for slice 1

Read from `ports/_shared/upstream/refinedstorage` (git clone of
`https://github.com/refinedmods/refinedstorage2`, tag **`v2.0.9`**, commit
`0dec63affe8147702b020843c22748483ef18c01`, 2026-06-07), plus the jar's data
files. **Behaviour only**: this port is a clean-room Rust rewrite, no Java was
copied or translated.

## The network model

- Every network block is a `NetworkNode` with an `InWorldNetworkNodeContainer`
  (`AbstractBaseNetworkNodeContainerBlockEntity`). A container is found at a
  position and side (`NetworkNodeContainerProviderImpl`), and its containers
  answer `canAcceptIncomingConnection`.
- `GraphNetworkComponentImpl` is the connected component: nodes are joined when
  their containers accept each other, which is `ColoredConnectionStrategy` for
  the machines (a directional machine's front does not accept a cable, and a
  cable only links another cable of its own colour or the default grey).
- A network is a graph of `NetworkComponent`s; nodes are **inactive** without a
  root. `AbstractNetworkNode.setActive` gates every node's work, and
  `GraphNetworkComponentImpl`/`NetworkBuilder` set the activeness of each node
  from the graph and the energy component: an online network is one with a
  **Controller** that has energy. `isEnergyRequired()` is the config switch
  (default true); without it the controller is only informational
  (`item.refinedstorage.controller.not_needed_because_energy_is_not_required`).
- The Controller is an energy provider (`ControllerNetworkNode`): its
  `ControllerEnergyType` is a **block state** (`off`, `nearly_off`,
  `nearly_on`, `on`) chosen from the stored/capacity ratio (0.01/0.3/0.4), and
  it is the only node with `getEnergyUsage() == 0`.
- `CableBlock` keeps its six connection booleans in the block entity
  (`CableConnections`, NBT `North/East/…`), recomputed on `updateShape`, on
  place and when the state is set; the shape is `CableShapes.getShape(...)`.
  The cable is waterloggable and is not pathfindable.
- `AbstractCableLikeBlockEntity` computes a connection towards `direction` only
  when a neighbouring container accepts it, besides the block's own
  blacklisted front face.

## The disk drive and the disks

- `AbstractDiskDriveBlockEntity`: **8 disk slots** (`AMOUNT_OF_DISKS = 8`),
  a `StorageNetworkNode(capacity = 8)` and a `StorageConfigurationContainer`
  (access mode, void excess, fuzzy filter). The drive is a
  `BlockEntityWithDrops` and its `DiskInventory` accepts only storage-disk
  items (`isValid`), notifying the node when a slot changes.
- `ItemStorageVariant`: `1k = 1024`, `4k = 4096`, `16k = 16384`, `64k = 65536`,
  `creative = unlimited`. The 1k disk is `stacksTo(1)`, fire resistant, and its
  storage is a world-level `SerializableStorage` referred to by a UUID in the
  item's `refinedstorage:storage_reference` component (the world keeps the
  bytes; a stale reference falls back to an empty disk).
- A disk in a drive contributes its capacity to the network's
  `StorageNetworkComponent`, which is the network's item repository: insertions
  go to one storage with room (never split across two), extractions take from
  each in turn.

## The grid and its view

- `GridBlockEntity` + `GridContainerMenu` + `GridScreen`: a 9x6 resource view
  (54 entries), a search box, sorting (quantity / name / id / last modified,
  default **quantity ascending**), ascending/descending and a view type. The
  grid is a `GridWatcher` on the network's storage and re-sends its resources on
  change (`GridUpdatePacket`), with an `active` boolean in the data (the grid's
  block state also has an `ACTIVE` property, set by the node's activeness).
- The search is `GridQueryParser` over a small query language (`@mod`, `#tag`,
  `$tooltip`, operators), applied server-side; the text comes from the client's
  search box, and the last query is remembered client-side
  (`AbstractGridContainerMenu.lastSearchQuery`).
- Click semantics: a left click extracts one, a right click half, shift a
  whole stack, and the cursor/insertion strategy decides where an insertion
  goes; `GridInsertionHint`s can narrow it. The view entries carry the true
  amount (a long) and the last-modified time.
- `GridBlockEntity` merges/commits pending view updates; the grid is a
  directional `BiDirection` block (24 directions) with a custom screen texture
  (`gui/grid.png`) and side buttons for sorting/view/synchronisation.

## The controller

- `AbstractControllerBlock` is a plain (non-directional) block with the
  `energy_type` enum state; `ControllerBlockEntity` ticks its node, which
  provides energy to the network. `ControllerBlockEntityTicker` also writes the
  block state when the energy state changes (`ACTIVENESS_CHANGE_TICK_RATE`).
- A network with several controllers links them: the graph shares one energy
  component.

## Edge cases that matter

- A **network without a controller** is inactive: every node's `setActive(false)`
  means the grid lists nothing, the drive exposes no storage, and an insertion
  into the grid is refused.
- Two networks with a gap, or a diagonal touch, are two networks.
- A disk's contents survive the disk being moved, because they live in the
  item (a storage reference upstream; a component in this port).
- A drive whose disk is removed loses that capacity; a fuller-than-capacity
  disk (a corrupted component) is cut to capacity on load.

## Where this port differs (all recorded in `notes.md`)

- Energy is not modelled: the controller's presence alone makes the network
  online, and its block state has `off`/`on` only (upstream has four energy
  levels). No FE buffer, no energy upkeep.
- The disk's contents live in the disk item's own component
  (`refinedstorage:storage`), not in a world-level storage repository keyed by
  UUID.
- The grid's view slots are the menu's slots (a proportional view reconciled
  each tick, the functional-storage pattern), not a client-side repository with
  extraction/insertion packets. 54 entries are reachable; no scrolling.
- The search is a plain case-insensitive substring of the item id, set by
  `/refinedstorage filter`, because a mod menu on this pin has no text field
  (a client text box needs `native_mod_screen_open`, which is not in the
  slice).
- The drive has **9** slots (the pin's menus only show 9/18/27/45/54) and no
  configuration (access mode, void excess, filters). The drive and grid models
  are ordinary models, not upstream's code-baked ones, and the grid's screen is
  the plain 9x6 panel (no upstream texture, side buttons or search box).
- Only the four network blocks and the 1k disk exist; the rest of the mod is the
  planned gap list.
