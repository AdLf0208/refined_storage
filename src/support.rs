//! The game glue of the item-network core: block entities, the disk cache, the
//! grid's menu and the commands.
//!
//! In the SDK layout this is an ordinary module of the mod crate. The rules
//! themselves live in [`crate::runtime`], which has no `mod_api` types; this
//! module only reads the game, calls the runtime and writes the result back.
//!
//! The game gives every block entity its own slots, so the grid cannot write a
//! drive's slots. The port keeps a per-disk cache here: a drive's tick mirrors
//! its disk items into the cache (or writes a change back), and the grid's tick
//! aggregates the cache of every drive in its network and reconciles the menu
//! slots the player changed. A drive is at most one tick behind a grid change,
//! and the disk's item component is the truth whenever the drive is loaded and
//! not mid-write.
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use mod_api::item::containers::ModBlockEntityFields;
use mod_api::{
    self, BlockDropsEvent, BlockEntityEvent, BlockEntityItemError, BlockEntityPart,
    BlockEntityTickEvent, BlockEntityTicksEvent, BlockKey, BlockPart, BlockState, Command,
    CommandArgumentValue, CommandInvocation, CommandSource, Component, Compound, CraftingGrid,
    CraftingRecipeCodec, CraftingRecipeKind, ENERGY, Flow, GameMode, Hand, Host, HostSide,
    HostTransaction, ITEM_ENERGY, ITEMS, InteractionResult, ItemDrop, ItemKey, ItemPart, ItemPlace,
    ItemResource, ItemStack, LevelHost, LevelRef, LevelSide, MenuClickedEvent, MenuClosedEvent,
    MenuContainer, MenuPart, MenuRef, MenuReplace, MenuSlotBacking, MenuSlotRulesEvent,
    ModBlockEntityView, ModContainer, ModEnergyBuffer, OnBlockPlacedByEvent,
    OnDestroyedByPlayerEvent, PlayerRef, RegisterCommandsEvent, Registrar, RegistrationContext,
    RightClickBlockEvent, RunModCommandEvent, Screen, ScreenEvent, ServerHost, ServerTickEvent,
    ServerTickPhase, StateForPlacementEvent, Tag, Text, UseItemEvent, UseOnEvent, WorldPhase,
};

use crate::runtime::{
    self, Binding, CAPACITY_1K, Cursor, DRIVE_SLOTS, Delta, Destination, Disk, DiskCapacity,
    EnergyType, Face, Fit, GRID_SLOTS, Kind, Node, Place, Plan, Promises, Refusal, Region, Stored,
    ViewKey, WirelessUse, disk_capacity, extract, insert, network, plan_click, view_with,
};

/// The five content ids of the slice, exactly as the content declares them.
const CONTROLLER: &str = "refinedstorage:controller";
const CABLE: &str = "refinedstorage:cable";
const DISK_DRIVE: &str = "refinedstorage:disk_drive";
const GRID: &str = "refinedstorage:grid";
const DISK: &str = "refinedstorage:1k_storage_disk";
/// The other disk sizes (slice 11): upstream's `ItemStorageVariant` — 4k, 16k,
/// 64k, and the creative disk, whose variant carries no capacity at all.
const DISK_4K: &str = "refinedstorage:4k_storage_disk";
const DISK_16K: &str = "refinedstorage:16k_storage_disk";
const DISK_64K: &str = "refinedstorage:64k_storage_disk";
const DISK_CREATIVE: &str = "refinedstorage:creative_storage_disk";
/// The wireless transmitter (slice 3): a network node that joins through its
/// facing side and answers whether a player may use its network.
const TRANSMITTER: &str = "refinedstorage:wireless_transmitter";
/// The wireless grid (slice 4): a network-bound item that opens its network's
/// 9×6 grid view from anywhere inside a transmitter's range.
const WIRELESS_GRID: &str = "refinedstorage:wireless_grid";
/// The importer and exporter (slice 7): the network's two transfer blocks,
/// each working on the neighbour its `facing` points at.
const IMPORTER: &str = "refinedstorage:importer";
const EXPORTER: &str = "refinedstorage:exporter";
/// The external storage (slice 14): a network node that exposes the inventory
/// it faces through the same `ITEMS` capability the transfer machines use.
const EXTERNAL_STORAGE: &str = "refinedstorage:external_storage";
/// The interface (slice 15): nine intermediate storage slots exposed as `ITEMS`
/// on every side, into which the network exports its configured resources.
const INTERFACE: &str = "refinedstorage:interface";
/// The range upgrade (slice 6): +8 blocks each, at most four per transmitter
/// (upstream `RangeUpgradeItem`, registered for `WIRELESS_TRANSMITTER`).
const RANGE_UPGRADE: &str = "refinedstorage:range_upgrade";
/// The creative range upgrade (slice 6): unlimited range, creative-only, at
/// most one per transmitter (upstream `RangeUpgradeItem`, creative).
const CREATIVE_RANGE_UPGRADE: &str = "refinedstorage:creative_range_upgrade";
/// The speed upgrade (slice 7): two ticks off a machine's work interval, at
/// most four (upstream `SimpleUpgradeItem::speedUpgrade`).
const SPEED_UPGRADE: &str = "refinedstorage:speed_upgrade";
/// The stack upgrade (slice 7): one work pass moves 64 instead of 1, at most
/// one (upstream `SimpleUpgradeItem::stackUpgrade`).
const STACK_UPGRADE: &str = "refinedstorage:stack_upgrade";
/// The block-entity type this mod declares for the four network blocks.
const BE_TYPE: &str = "refinedstorage:network";
/// The item component that carries a disk's contents.
const COMPONENT: &str = "refinedstorage:storage";
/// The item component that names the transmitter a wireless grid is bound to
/// (upstream's `refinedstorage:network_location`, a `GlobalPos`).
const NETWORK_LOCATION: &str = "refinedstorage:network_location";
/// The item component a wireless grid keeps its own energy in: upstream's
/// `ItemEnergyStorage` on the grid item, an int under one component id.
const ENERGY_COMPONENT: &str = "refinedstorage:energy";

/// The title the wireless grid's menu shows (upstream's `ContentNames.WIRELESS_GRID`).
const WIRELESS_GRID_TITLE: &str = "Wireless Grid";
/// The wireless grid's menu has six rows, the block grid's 9×6 view.
const WIRELESS_GRID_ROWS: u8 = 6;

/// The drive's menu shows one row of nine (upstream has eight; the game's
/// menu types only show 9/18/27/45/54 slots, and a menu is how a player moves
/// disks in and out).
const DRIVE_MENU: &str = "minecraft:generic_9x1";
const GRID_MENU: &str = "minecraft:generic_9x6";
/// The transmitter's upgrade slots: upstream's `UpgradeContainer` default size
/// is 4, the container its `WirelessTransmitterBlockEntity` builds.
const TRANSMITTER_SLOTS: usize = runtime::TRANSMITTER_SLOTS;
/// The transmitter's menu: upstream's `WirelessTransmitterContainerMenu` shows
/// the four upgrade slots beside the player's inventory. The pin's vanilla
/// menus have no four-slot type, so the package declares its own (API 85).
const TRANSMITTER_MENU: &str = "refinedstorage:wireless_transmitter";
/// A machine's filter entries: upstream's `ResourceContainerImpl.createForFilter`
/// makes a nine-slot container (`AbstractSimpleFilterContainerMenu.addSlots`
/// puts them first).
const FILTER_SLOTS: usize = runtime::MACHINE_FILTERS;
/// A machine's upgrade slots: upstream's `UpgradeContainer` default size (4).
const MACHINE_UPGRADES: usize = runtime::MACHINE_UPGRADES;
/// A machine's menu slots: the nine filter entries, then the four upgrades
/// (upstream's `ImporterContainerMenu`/`ExporterContainerMenu` order).
const MACHINE_SLOTS: usize = FILTER_SLOTS + MACHINE_UPGRADES;
/// The importer's and exporter's menus (the package's own content menu types).
const IMPORTER_MENU: &str = "refinedstorage:importer";
const EXPORTER_MENU: &str = "refinedstorage:exporter";
/// The interface's filter row and buffer row are eighteen slots, which the
/// pin's generic 9x2 menu shows (upstream's `InterfaceContainerMenu` has the
/// same two rows of nine).
const INTERFACE_MENU: &str = "minecraft:generic_9x2";
/// The interface's buffer slots in its block container: the first nine are the
/// filter entries (the row an external hopper must not reach), the next nine
/// the exported resources the whole `ITEMS` container exposes.
const INTERFACE_FILTER_SLOTS: usize = runtime::INTERFACE_SLOTS;
const INTERFACE_SLOTS: usize = runtime::INTERFACE_SLOTS * 2;

/// The saved compound key the grid's view marks live under.
const VIEW_KEY: &str = "RefinedStorageView";

/// The saved compound key a broken machine's slots travel under on its item
/// (slice 10): the item's `minecraft:block_entity_data` carries them, and the
/// key is this port's own, never vanilla's `Items` — the engine spills a mod
/// container's `Items` on removal, so carrying them again would duplicate them.
const CONTENTS_KEY: &str = "RefinedStorageContents";

/// Vanilla's block-entity data component, the one `BlockItem` applies on
/// placement (for a mod type id the engine's `canonical()` answers `None`, so
/// this port's `on_block_placed_by` restores the slots itself).
const BLOCK_ENTITY_DATA: &str = "minecraft:block_entity_data";

/// How long a broken machine's pending carry waits for its own drop, in server
/// ticks. The break's `block_drops` is the same call, so one tick would do; a
/// small window keeps an op's queued break from inheriting nothing.
const CARRY_TTL: u64 = 4;

/// The saved compound key a controller's buffer lives under: the key the game's
/// `block_energy` handler keeps the amount in (`ModEnergyBuffer`: "the amount is
/// not here: it lives in the entity's saved compound, under `energy`").
const ENERGY_KEY: &str = "energy";

/// A disk cache key: the dimension, the drive's position and its slot.
type DiskKey = (String, [i32; 3], usize);

/// The registry keys the glue compares loaded blocks against. `ItemKey` and
/// `BlockKey` only name themselves through a host, so the port asks for them
/// once, at registration, and compares the handles afterwards.
#[derive(Clone, Copy, Debug)]
struct Keys {
    controller: BlockKey,
    cable: BlockKey,
    disk_drive: BlockKey,
    grid: BlockKey,
    transmitter: BlockKey,
    importer: BlockKey,
    exporter: BlockKey,
    external_storage: BlockKey,
    interface: BlockKey,
    /// The five storage disk items, upstream's `ItemStorageVariant`. Which one a
    /// stack is decides its capacity (`runtime::disk_capacity`); a drive takes
    /// any of them.
    disks: [ItemKey; 5],
    wireless_grid: ItemKey,
    range_upgrade: ItemKey,
    creative_range_upgrade: ItemKey,
    speed_upgrade: ItemKey,
    stack_upgrade: ItemKey,
    /// The three blocks whose own slots travel on their item (slice 10).
    importer_item: ItemKey,
    exporter_item: ItemKey,
    transmitter_item: ItemKey,
}

impl Keys {
    /// The network kind of a loaded block, none when it is not one of the
    /// seven.
    fn kind_of(self, block: BlockKey) -> Option<Kind> {
        [
            (self.controller, Kind::Controller),
            (self.cable, Kind::Cable),
            (self.disk_drive, Kind::DiskDrive),
            (self.grid, Kind::Grid),
            (self.transmitter, Kind::WirelessTransmitter),
            (self.importer, Kind::Importer),
            (self.exporter, Kind::Exporter),
            (self.external_storage, Kind::ExternalStorage),
            (self.interface, Kind::Interface),
        ]
        .into_iter()
        .find_map(|(key, kind)| (key == block).then_some(kind))
    }

    /// The kind whose item `item` is, for the three that carry their slots.
    fn kind_of_item(self, item: ItemKey) -> Option<Kind> {
        [
            (self.importer_item, Kind::Importer),
            (self.exporter_item, Kind::Exporter),
            (self.transmitter_item, Kind::WirelessTransmitter),
        ]
        .into_iter()
        .find_map(|(key, kind)| (key == item).then_some(kind))
    }

    /// The item the block of `kind` drops as.
    fn item_of(self, kind: Kind) -> Option<ItemKey> {
        match kind {
            Kind::Importer => Some(self.importer_item),
            Kind::Exporter => Some(self.exporter_item),
            Kind::WirelessTransmitter => Some(self.transmitter_item),
            Kind::Controller
            | Kind::Cable
            | Kind::DiskDrive
            | Kind::Grid
            | Kind::ExternalStorage
            | Kind::Interface => None,
        }
    }
}

static KEYS: OnceLock<Keys> = OnceLock::new();

fn keys() -> Keys {
    *KEYS.get().expect("registration runs before every hook")
}

// --- registration ----------------------------------------------------------

/// Declare the block-entity type, attach the network Part to it, declare the
/// drive's and the grid's block containers, and request the disk item.
pub(crate) fn register(registrar: &mut dyn Registrar, _cx: &RegistrationContext) {
    let keys = Keys {
        controller: registrar.block_key(CONTROLLER),
        cable: registrar.block_key(CABLE),
        disk_drive: registrar.block_key(DISK_DRIVE),
        grid: registrar.block_key(GRID),
        transmitter: registrar.block_key(TRANSMITTER),
        importer: registrar.block_key(IMPORTER),
        exporter: registrar.block_key(EXPORTER),
        external_storage: registrar.block_key(EXTERNAL_STORAGE),
        interface: registrar.block_key(INTERFACE),
        disks: [
            registrar.item_key(DISK),
            registrar.item_key(DISK_4K),
            registrar.item_key(DISK_16K),
            registrar.item_key(DISK_64K),
            registrar.item_key(DISK_CREATIVE),
        ],
        wireless_grid: registrar.item_key(WIRELESS_GRID),
        range_upgrade: registrar.item_key(RANGE_UPGRADE),
        creative_range_upgrade: registrar.item_key(CREATIVE_RANGE_UPGRADE),
        speed_upgrade: registrar.item_key(SPEED_UPGRADE),
        stack_upgrade: registrar.item_key(STACK_UPGRADE),
        importer_item: registrar.item_key(IMPORTER),
        exporter_item: registrar.item_key(EXPORTER),
        transmitter_item: registrar.item_key(TRANSMITTER),
    };
    let _ = KEYS.set(keys);
    registrar.item_part(keys.wireless_grid, WirelessGrid);
    // The three blocks that keep their own slots on their item: a placed
    // machine takes back what its item carried (`BlockItemExtension
    // .onBlockPlacedBy`, here because the engine merges `block_entity_data`
    // only through `canonical()`, which answers `None` for a mod type id).
    for item in [
        keys.importer_item,
        keys.exporter_item,
        keys.transmitter_item,
    ] {
        registrar.item_part(item, CarriedContents);
    }
    // The wireless grid's own energy buffer (slice 9): upstream's item is an
    // `ItemEnergyStorage` of `WIRELESS_GRID_CAPACITY`, charged 5 FE per open,
    // insert and extract. The buffer is the item's `refinedstorage:energy`
    // component, answered through the item energy capability (NeoForge's
    // `Capabilities.Energy.ITEM`), so an FE charger fills the same buffer the
    // port's own charges drain.
    registrar.item_capability(&ITEM_ENERGY, WIRELESS_GRID, |_, place| {
        Some(Box::new(GridBattery(place.clone())))
    });
    registrar.menu_part(TRANSMITTER_MENU, Upgrades);
    registrar.menu_part(IMPORTER_MENU, MachineSlots);
    registrar.menu_part(EXPORTER_MENU, MachineSlots);
    // The in-place disk upgrade's crafting kind (slice 12): a smaller disk plus
    // the bigger tier's storage part. Registering it here reads the three
    // `refinedstorage:storage_disk_upgrade` files as this mod's recipes instead
    // of refusing them as an unknown type — the ids a grid may name are
    // resolved once, because a craft-time match takes no host.
    let mut upgrade_keys = Vec::with_capacity(runtime::STORAGE_VARIANTS.len() * 2);
    for variant in runtime::STORAGE_VARIANTS {
        upgrade_keys.push((variant.disk, registrar.item_key(variant.disk)));
        upgrade_keys.push((variant.part, registrar.item_key(variant.part)));
    }
    registrar.register_crafting_recipe_serializer(UPGRADE_KIND, UpgradeKind { keys: upgrade_keys });
    let kind = registrar
        .block_entity_type(BE_TYPE)
        .blocks(&[
            CONTROLLER,
            CABLE,
            DISK_DRIVE,
            GRID,
            TRANSMITTER,
            IMPORTER,
            EXPORTER,
            EXTERNAL_STORAGE,
            INTERFACE,
        ])
        .build();
    registrar
        .block_entity_part(&kind, Network)
        .expect("one Part per block-entity type");
    for (block, slots, menu) in [
        (keys.disk_drive, DRIVE_SLOTS, DRIVE_MENU),
        (keys.grid, GRID_SLOTS, GRID_MENU),
    ] {
        let all: Vec<usize> = (0..slots).collect();
        // `..Default::default()` keeps a field a later game build adds from
        // breaking this literal (`cargo horizon-mod check` asks for it).
        #[expect(
            clippy::needless_update,
            reason = "the update is for a field a later game build may add"
        )]
        let container = ModContainer {
            slots,
            // No hopper reaches a grid's view or a drive's disk slots:
            // neither is a container upstream.
            faces: [
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ],
            // The menu's clicks may put items into every slot (a deposit into
            // the grid, a disk into the drive) and take them out.
            inputs: all.clone(),
            outputs: all,
            menu: Some(menu.to_owned()),
            ..ModContainer::default()
        };
        registrar.block_container(&block, container);
    }
    // The machines' filter entries and upgrade slots (upstream's nine-slot
    // filter container then its four-slot upgrade container).
    let machine_slots: Vec<usize> = (0..MACHINE_SLOTS).collect();
    for (block, menu) in [
        (keys.importer, IMPORTER_MENU),
        (keys.exporter, EXPORTER_MENU),
    ] {
        #[expect(
            clippy::needless_update,
            reason = "the update is for a field a later game build may add"
        )]
        let container = ModContainer {
            slots: MACHINE_SLOTS,
            // No hopper reaches a machine's filter or upgrade slots upstream:
            // they are the menu's, and the machine's own work is its tick.
            faces: [
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ],
            inputs: machine_slots.clone(),
            outputs: machine_slots.clone(),
            menu: Some(menu.to_owned()),
            ..ModContainer::default()
        };
        registrar.block_container(&block, container);
    }
    // The interface (slice 15): nine filter entries, then nine intermediate
    // storage slots. Upstream exposes only the exported-resources container as
    // `ITEMS` (`ModInitializer` registers an `InvWrapper` over
    // `getExportedResourcesAsContainer()` on every side, `side` ignored), so a
    // hopper reaches the buffer row on all six faces and never the filter row.
    // The artefact has no `facing` state (`InterfaceBlock extends
    // AbstractBaseBlock`), which the port records as `blocked(no-interface-facing)`.
    let all: Vec<usize> = (0..INTERFACE_SLOTS).collect();
    let buffer: Vec<usize> = (INTERFACE_FILTER_SLOTS..INTERFACE_SLOTS).collect();
    #[expect(
        clippy::needless_update,
        reason = "the update is for a field a later game build may add"
    )]
    let container = ModContainer {
        slots: INTERFACE_SLOTS,
        faces: [
            buffer.clone(),
            buffer.clone(),
            buffer.clone(),
            buffer.clone(),
            buffer.clone(),
            buffer.clone(),
        ],
        inputs: all.clone(),
        outputs: all,
        menu: Some(INTERFACE_MENU.to_owned()),
        ..ModContainer::default()
    };
    registrar.block_container(&keys.interface, container);
    // The transmitter's four upgrade slots (upstream `UpgradeContainer`,
    // `WirelessTransmitterBlockEntity`) and the menu that shows them.
    let upgrades: Vec<usize> = (0..TRANSMITTER_SLOTS).collect();
    #[expect(
        clippy::needless_update,
        reason = "the update is for a field a later game build may add"
    )]
    let container = ModContainer {
        slots: TRANSMITTER_SLOTS,
        // No hopper reaches a transmitter's upgrade slots upstream: they are
        // the menu's, not a container's faces.
        faces: [
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ],
        inputs: upgrades.clone(),
        outputs: upgrades,
        menu: Some(TRANSMITTER_MENU.to_owned()),
        ..ModContainer::default()
    };
    registrar.block_container(&keys.transmitter, container);

    // The controller's energy buffer (slice 8): the game's own FE handler on the
    // block (`Registrar::block_energy`, NeoForge's `SimpleEnergyHandler`), whose
    // amount lives in the block entity's compound under `energy`, so an FE
    // cable fills the same buffer the network draws from. Upstream's
    // `ControllerBlockEntity` is a `TransferableBlockEntityEnergy` with
    // `CONTROLLER_CAPACITY` 1000 and the same six faces.
    #[expect(
        clippy::needless_update,
        reason = "the update is for a field a later game build may add"
    )]
    let buffer = ModEnergyBuffer {
        capacity: runtime::CONTROLLER_CAPACITY as i32,
        max_insert: runtime::CONTROLLER_CAPACITY as i32,
        max_extract: runtime::CONTROLLER_CAPACITY as i32,
        insert_faces: [true; 6],
        extract_faces: [true; 6],
        ..ModEnergyBuffer::default()
    };
    registrar.block_energy(&keys.controller, buffer);
}

/// The transmitter's menu Part: its four upgrade slots take an upgrade item
/// and nothing else, one per slot, at most four range upgrades and one
/// creative one (upstream `UpgradeContainer.canPlaceItem` and the
/// `WIRELESS_TRANSMITTER` registrations: range `maxAmount` 4, creative 1).
///
/// Scoped to the menu's own container (`MenuContainer::Main`): the player
/// inventory's slots keep vanilla's rules, so a refusal here cannot cancel a
/// click on the player's side of the menu.
struct Upgrades;

#[mod_api::horizon_part]
impl MenuPart for Upgrades {
    fn menu_slot_rules(&self, _host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
        if event.container != MenuContainer::Main {
            return;
        }
        let keys = keys();
        let item = event.stack.item();
        let range = item == keys.range_upgrade;
        let creative = item == keys.creative_range_upgrade;
        if !range && !creative {
            event.may_place = false;
            return;
        }
        // One upgrade per slot, as `UpgradeContainer.getMaxStackSize` answers 1.
        event.max_stack = 1;
        let held = |what: ItemKey| {
            (0..TRANSMITTER_SLOTS)
                .filter(|slot| {
                    event
                        .context
                        .slot(MenuContainer::Main, *slot)
                        .is_some_and(|stack| stack.item() == what)
                })
                .count() as u32
        };
        if range && held(keys.range_upgrade) >= runtime::MAX_RANGE_UPGRADES {
            event.may_place = false;
        }
        if creative && held(keys.creative_range_upgrade) >= 1 {
            event.may_place = false;
        }
    }
}

/// An importer's or exporter's menu slots: the nine filter entries (any item,
/// one per slot) and the four upgrade slots (a speed or stack upgrade, one per
/// slot, at most four speed and one stack — upstream's
/// `UpgradeContainer.canPlaceItem` and its `IMPORTER`/`EXPORTER`
/// registrations).
///
/// Scoped to the menu's own container (`MenuContainer::Main`) and to the two
/// slot ranges, so a refusal on one slot cannot cancel a click on another: the
/// player inventory keeps vanilla's rules, and a diamond the upgrade slots
/// refuse still lands in a filter slot if that is where the click went.
struct MachineSlots;

#[mod_api::horizon_part]
impl MenuPart for MachineSlots {
    fn menu_slot_rules(&self, _host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
        if event.container != MenuContainer::Main {
            return;
        }
        if event.slot < FILTER_SLOTS {
            // A filter entry names one resource with one item.
            event.max_stack = 1;
            return;
        }
        if event.slot >= MACHINE_SLOTS {
            event.may_place = false;
            return;
        }
        let keys = keys();
        let item = event.stack.item();
        let speed = item == keys.speed_upgrade;
        let stack = item == keys.stack_upgrade;
        if !speed && !stack {
            event.may_place = false;
            return;
        }
        // One upgrade per slot, as `UpgradeContainer.getMaxStackSize` answers 1.
        event.max_stack = 1;
        let held = |what: ItemKey| {
            (FILTER_SLOTS..MACHINE_SLOTS)
                .filter(|slot| {
                    event
                        .context
                        .slot(MenuContainer::Main, *slot)
                        .is_some_and(|held| held.item() == what)
                })
                .count() as u32
        };
        if speed && held(keys.speed_upgrade) >= runtime::MAX_SPEED_UPGRADES {
            event.may_place = false;
        }
        if stack && held(keys.stack_upgrade) >= runtime::MAX_STACK_UPGRADES {
            event.may_place = false;
        }
    }
}

/// The disk's, the wireless grid's and the battery's item components
/// (`component_types`, before content loads).
pub(crate) fn register_component_types(event: &mut mod_api::ComponentTypesEvent<'_>) {
    event.types.push(COMPONENT.to_owned());
    event.types.push(NETWORK_LOCATION.to_owned());
    event.types.push(ENERGY_COMPONENT.to_owned());
}

// --- one item as the network sees it --------------------------------------

/// One item on the network: the stack plus its canonical resource key,
/// resolved once where a host is at hand. Two stacks are one resource when
/// their item id, damage and components all agree.
#[derive(Clone, Debug, PartialEq)]
struct Net {
    stack: ItemStack,
    key: String,
}

impl Net {
    fn of(host: &dyn Host, stack: &ItemStack) -> Self {
        Net {
            key: resource_key(host, stack),
            stack: stack.clone(),
        }
    }

    fn stack(&self) -> &ItemStack {
        &self.stack
    }

    fn owned(self) -> ItemStack {
        self.stack
    }
}

impl Stored for Net {
    fn key(&self) -> String {
        self.key.clone()
    }

    fn count(&self) -> u32 {
        self.stack.count()
    }

    /// The same resource with another count. A stack cannot hold zero items
    /// (`ItemStack::set_count(0)` is an error and leaves the count alone), so a
    /// count of 0 is raised to 1 here: "nothing left over" is an `Option`, as
    /// [`runtime::insert`] answers it, never a clamped stack.
    fn with_count(&self, count: u32) -> Self {
        let mut stack = self.stack.clone();
        let _ = stack.set_count(count.max(1));
        Net {
            stack,
            key: self.key.clone(),
        }
    }
}

/// The identity of a resource: its item id, its damage and its components.
/// The component patch is an ordered `Compound`, so its debug form is stable
/// inside one session, which is all the in-memory network needs.
fn resource_key(host: &dyn Host, stack: &ItemStack) -> String {
    let id = stack.item().id(host);
    match stack.components() {
        Some(patch) => format!("{id}#{}#{:?}", stack.damage(), patch.as_nbt()),
        None => format!("{id}#{}#", stack.damage()),
    }
}

/// The five disk items in a drive's slots (upstream's `DiskInventory.isValid`
/// accepts every `StorageDiskItem`).
fn is_disk(stack: &ItemStack) -> bool {
    keys().disks.contains(&stack.item())
}

// --- the disk's item component --------------------------------------------

/// What a disk's item component saves (slice 11): its capacity and the stacks
/// it holds.
struct SavedDisk {
    /// The capacity the item recorded. A component saved before slice 11 is a
    /// bare list of stacks and has none, so the disk's own item decides.
    capacity: Option<DiskCapacity>,
    stacks: Vec<Compound>,
}

/// The component that carries a storage disk's contents and capacity:
/// `{capacity, stacks}` in `refinedstorage:storage`, the compound shape, or —
/// for a disk saved by 0.2.10 — the bare list of stacks this port wrote then.
struct StorageComponent;

impl Component for StorageComponent {
    type Value = SavedDisk;

    fn id(&self) -> &str {
        COMPONENT
    }

    fn decode(&self, value: &Tag) -> Option<SavedDisk> {
        match value {
            Tag::List(entries) => Some(SavedDisk {
                capacity: None,
                stacks: compounds(entries),
            }),
            Tag::Compound(compound) => Some(SavedDisk {
                capacity: field(compound, "capacity").and_then(saved_capacity),
                stacks: match field(compound, "stacks") {
                    Some(Tag::List(entries)) => compounds(entries),
                    _ => Vec::new(),
                },
            }),
            _ => None,
        }
    }

    fn encode(&self, value: SavedDisk) -> Tag {
        let capacity = value.capacity.map_or(0, DiskCapacity::saved);
        Tag::Compound(Compound::from([
            ("capacity".into(), Tag::Long(capacity as i64)),
            (
                "stacks".into(),
                Tag::List(value.stacks.into_iter().map(Tag::Compound).collect()),
            ),
        ]))
    }
}

/// The compounds of a saved list, skipping anything else.
fn compounds(entries: &[Tag]) -> Vec<Compound> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Tag::Compound(compound) => Some(compound.clone()),
            _ => None,
        })
        .collect()
}

/// A saved capacity number: the item count, or the creative disk's `0` marker.
fn saved_capacity(tag: &Tag) -> Option<DiskCapacity> {
    let saved = match tag {
        Tag::Byte(byte) => u64::try_from(*byte).ok()?,
        Tag::Short(short) => u64::try_from(*short).ok()?,
        Tag::Int(int) => u64::try_from(*int).ok()?,
        Tag::Long(long) => u64::try_from(*long).ok()?,
        _ => return None,
    };
    Some(DiskCapacity::from_saved(saved))
}

/// A disk item's capacity and contents. The capacity is the item's own
/// (upstream's `ItemStorageVariant`, which is what decides a disk's size), cut
/// to what the component records when it records less; an item with no
/// component is an empty disk (a freshly made or creative-given one).
fn read_disk(host: &dyn Host, stack: &ItemStack) -> Disk<Net> {
    let item = disk_capacity(&stack.item().id(host)).unwrap_or(DiskCapacity::Items(CAPACITY_1K));
    let saved = stack.get(&StorageComponent);
    let capacity = match &saved {
        Some(saved) => match saved.capacity {
            Some(said) => item.clamp(said),
            None => item,
        },
        None => item,
    };
    let mut disk = match capacity {
        DiskCapacity::Items(items) => Disk::new(items),
        DiskCapacity::Unlimited => Disk::unlimited(),
    };
    if let Some(saved) = saved {
        let stored: Vec<Net> = saved
            .stacks
            .iter()
            .filter_map(|entry| ItemStack::from_nbt(host, entry))
            .map(|stack| Net::of(host, &stack))
            .collect();
        disk.set_entries(stored);
    }
    disk
}

/// Write a disk's contents and capacity into its item component (an empty disk
/// loses the component rather than keeping an empty list; its item still names
/// its capacity).
fn write_disk(host: &dyn Host, stack: &mut ItemStack, disk: &Disk<Net>) {
    if disk.entries().is_empty() {
        stack.remove(&StorageComponent);
        return;
    }
    let entries: Vec<Compound> = disk
        .entries()
        .iter()
        .filter_map(|entry| entry.stack().to_nbt(host))
        .collect();
    let _ = stack.set(
        &StorageComponent,
        SavedDisk {
            capacity: Some(disk.capacity()),
            stacks: entries,
        },
    );
}

// --- the in-place disk upgrade (slice 12) ----------------------------------

/// The `refinedstorage:storage_disk_upgrade` crafting kind: upstream's
/// `StorageContainerUpgradeRecipe` — a smaller disk plus the bigger tier's
/// storage part becomes the bigger disk, the contents come across, and the
/// source disk's own storage part is returned.
const UPGRADE_KIND: &str = "storage_disk_upgrade";

/// The upgrade recipe as it crosses the API boundary: only the destination
/// variant, which is all upstream's serializer keeps (`{"type": …, "to": …}`).
#[derive(Clone, Debug, PartialEq, Eq, mod_api::Wire)]
struct UpgradeValue {
    /// The variant name the file's `to` field carries (`"4k"`, `"16k"`, `"64k"`).
    to: String,
}

/// The registered `storage_disk_upgrade` kind, with the item keys a matching
/// grid may name: a craft-time match takes no host, so the ids are resolved once
/// at registration and compared afterwards.
struct UpgradeKind {
    /// Every disk and storage part an upgrade may name, in variant order.
    keys: Vec<(&'static str, ItemKey)>,
}

impl UpgradeKind {
    /// The key of one of the ids an upgrade may name.
    fn key(&self, id: &str) -> Option<ItemKey> {
        self.keys
            .iter()
            .find(|(name, _)| *name == id)
            .map(|(_, key)| *key)
    }

    /// The id of the item a grid cell holds, `None` when it is not one of the
    /// kind's own items.
    fn id_of(&self, key: ItemKey) -> Option<&'static str> {
        self.keys
            .iter()
            .find(|(_, known)| *known == key)
            .map(|(name, _)| *name)
    }

    /// The grid's cells as ids. An empty cell stays `None`; a cell holding an
    /// item the kind does not name ends the whole answer, so a third item can
    /// never look like an empty cell to the rule.
    fn grid_ids(&self, grid: &CraftingGrid) -> Option<Vec<Option<&'static str>>> {
        let mut ids = Vec::with_capacity(grid.cells.len());
        for cell in &grid.cells {
            match cell {
                None => ids.push(None),
                Some(stack) => ids.push(Some(self.id_of(stack.item())?)),
            }
        }
        Some(ids)
    }
}

impl CraftingRecipeKind for UpgradeKind {
    type Value = UpgradeValue;

    fn codec(&self) -> Option<CraftingRecipeCodec<Self::Value>> {
        Some(CraftingRecipeCodec::wire())
    }

    /// The file is the serializer's own shape: a `to` variant and nothing else
    /// (the ingredients are derived from it, as upstream derives them).
    fn decode(&self, json: &str) -> Result<Self::Value, String> {
        use mod_api::serde_json::Value;

        let value: Value =
            mod_api::serde_json::from_str(json).map_err(|error| error.to_string())?;
        if let Some(kind) = value.get("type").and_then(Value::as_str)
            && kind != format!("refinedstorage:{UPGRADE_KIND}")
        {
            return Err(format!("{kind} is not a storage disk upgrade"));
        }
        let to = value
            .get("to")
            .and_then(Value::as_str)
            .ok_or("a storage disk upgrade needs a \"to\" variant")?;
        if runtime::disk_of_variant(to).is_none() {
            return Err(format!("{to} is not a storage variant"));
        }
        Ok(UpgradeValue { to: to.to_owned() })
    }

    /// The client reads the same object back; the variant is one of five known
    /// bare names, so the literal needs no escaping.
    fn write(&self, value: &Self::Value) -> Result<String, String> {
        if runtime::disk_of_variant(&value.to).is_none() {
            return Err(format!("{} is not a storage variant", value.to));
        }
        Ok(format!("{{\"to\":\"{}\"}}", value.to))
    }

    fn matches(&self, value: &Self::Value, grid: CraftingGrid) -> bool {
        let Some(to_disk) = runtime::disk_of_variant(&value.to) else {
            return false;
        };
        let Some(ids) = self.grid_ids(&grid) else {
            return false;
        };
        runtime::upgrade_cells(&ids, to_disk).is_some()
    }

    fn assemble(&self, value: &Self::Value, grid: CraftingGrid) -> Option<ItemStack> {
        let to_disk = runtime::disk_of_variant(&value.to)?;
        let ids = self.grid_ids(&grid)?;
        let cells = runtime::upgrade_cells(&ids, to_disk)?;
        let source = grid.cells.get(cells.disk)?.as_ref()?;
        let mut out = ItemStack::new(self.key(to_disk)?, 1)?;
        carry_storage(source, &mut out, runtime::disk_capacity(to_disk)?);
        Some(out)
    }

    /// The source disk's own part goes back into the cell the disk left; every
    /// other cell is empty (the destination part is consumed by the craft).
    fn remaining_items(
        &self,
        value: &Self::Value,
        grid: CraftingGrid,
    ) -> Option<Vec<Option<ItemStack>>> {
        let to_disk = runtime::disk_of_variant(&value.to)?;
        let ids = self.grid_ids(&grid)?;
        let cells = runtime::upgrade_cells(&ids, to_disk)?;
        let source = ids.get(cells.disk).copied().flatten()?;
        let key = self.key(runtime::storage_part_of_disk(source)?)?;
        let mut remaining = vec![None; grid.cells.len()];
        remaining[cells.disk] = Some(ItemStack::new(key, 1)?);
        Some(remaining)
    }
}

/// Carry a source disk's saved contents onto the upgraded disk, recording the
/// destination's room: upstream's `transferTo` reuses the storage and the
/// destination item decides the capacity (its tooltip prints
/// `transferring ? capacity : …`). The saved stacks are compounds, so they move
/// verbatim instead of being decoded and re-created.
fn carry_storage(source: &ItemStack, to: &mut ItemStack, capacity: DiskCapacity) {
    let Some(saved) = source.get(&StorageComponent) else {
        return;
    };
    if saved.stacks.is_empty() {
        return;
    }
    let _ = to.set(
        &StorageComponent,
        SavedDisk {
            capacity: Some(capacity),
            stacks: saved.stacks,
        },
    );
}

// --- a wireless grid's binding --------------------------------------------

/// The component that names the transmitter a wireless grid is bound to:
/// upstream's `GlobalPos` — a dimension id and a block position — in the
/// `refinedstorage:network_location` component.
struct NetworkLocation;

impl Component for NetworkLocation {
    type Value = Binding;

    fn id(&self) -> &str {
        NETWORK_LOCATION
    }

    fn decode(&self, value: &Tag) -> Option<Binding> {
        let Tag::Compound(compound) = value else {
            return None;
        };
        let Some(Tag::String(dimension)) = field(compound, "dimension") else {
            return None;
        };
        let Some(Tag::IntArray(pos)) = field(compound, "pos") else {
            return None;
        };
        let [x, y, z] = pos.as_slice() else {
            return None;
        };
        Some(Binding::new(dimension.to_string_lossy(), [*x, *y, *z]))
    }

    fn encode(&self, value: Binding) -> Tag {
        Tag::Compound(Compound::from([
            ("dimension".into(), Tag::String(value.dimension().into())),
            ("pos".into(), Tag::IntArray(value.pos().to_vec())),
        ]))
    }
}

/// The transmitter a wireless grid item is bound to, from its component.
fn binding_of(stack: &ItemStack) -> Option<Binding> {
    stack.get(&NetworkLocation)
}

/// `stack` with its binding component set to `binding`.
fn with_binding(mut stack: ItemStack, binding: &Binding) -> ItemStack {
    let _ = stack.set(&NetworkLocation, binding.clone());
    stack
}

/// Bind the held wireless grid to the transmitter at `pos`: its dimension and
/// position in the item's own component (`NetworkItemHelperImpl.bind`), in one
/// transaction, and tell the player. False when the hand or the player is gone.
fn bind_grid(
    host: &mut dyn ServerHost,
    uuid: u128,
    hand: Hand,
    pos: [i32; 3],
    stack: &ItemStack,
) -> bool {
    let Some(player) = host.player(uuid) else {
        return false;
    };
    let Some(slot) = held_slot(host, &player, hand) else {
        return false;
    };
    let binding = Binding::new(host.dimension().to_owned(), pos);
    let bound = with_binding(stack.clone(), &binding);
    let wrote = host
        .transaction(|host, _tx| player.set_inventory_slot(host, slot, Some(bound)))
        .is_ok();
    if wrote {
        tell(
            host,
            uuid,
            &format!("Wireless grid bound to {}.", binding_name(&binding)),
        );
    }
    wrote
}

// --- the wireless grid's own buffer ---------------------------------------

/// The wireless grid's charge, in its own `refinedstorage:energy` component
/// (upstream's `ItemEnergyStorage`): a grid carrying none reads empty.
struct Battery;

impl Component for Battery {
    type Value = u32;

    fn id(&self) -> &str {
        ENERGY_COMPONENT
    }

    fn decode(&self, value: &Tag) -> Option<u32> {
        match value {
            Tag::Int(held) => u32::try_from(*held).ok(),
            _ => None,
        }
    }

    fn encode(&self, value: u32) -> Tag {
        Tag::Int(i32::try_from(value).unwrap_or(i32::MAX))
    }
}

/// What the stack holds: its charge, or nothing when it carries none.
fn battery_of(stack: &ItemStack) -> u32 {
    stack.get(&Battery).unwrap_or(0)
}

/// `stack` with its charge set to `energy`.
fn with_battery(mut stack: ItemStack, energy: u32) -> ItemStack {
    let _ = stack.set(&Battery, energy);
    stack
}

/// The grid's own buffer as the item energy capability answers it (NeoForge's
/// `Capabilities.Energy.ITEM`, the port's fifth capability), so an FE charger
/// fills the same component the port's own charges drain.
struct GridBattery(ItemPlace);

impl mod_api::EnergyHandler for GridBattery {
    fn amount(&self, host: &mut dyn ServerHost) -> u32 {
        self.0.stack(host).map_or(0, |stack| battery_of(&stack))
    }

    fn capacity(&self, _: &mut dyn ServerHost) -> u32 {
        runtime::WIRELESS_GRID_CAPACITY
    }

    fn insert(&self, host: &mut dyn ServerHost, tx: &mod_api::Transaction, amount: u32) -> u32 {
        let held = self.amount(host);
        let went = amount.min(runtime::WIRELESS_GRID_CAPACITY.saturating_sub(held));
        match went {
            0 => 0,
            went if store_battery(host, tx, &self.0, held + went) => went,
            _ => 0,
        }
    }

    fn extract(&self, host: &mut dyn ServerHost, tx: &mod_api::Transaction, amount: u32) -> u32 {
        let held = self.amount(host);
        let came = amount.min(held);
        match came {
            0 => 0,
            came if store_battery(host, tx, &self.0, held - came) => came,
            _ => 0,
        }
    }
}

/// Put `energy` into the stack at `place`, in `tx`: whether it went.
///
/// A **held** grid is rewritten through the player's own slot, as a binding is
/// (`bind_grid`). An exchange of the hand is not interchangeable with it: in
/// creative a hand's extract gives without losing the stack, so `exchange`
/// leaves the old, uncharged grid in the hand and puts the charged one in the
/// inventory — measured in this slice's first leg, and the reason the SDK's
/// stock `ItemComponentEnergy` cannot charge a held grid. Every other place is
/// a container slot, whose exchange is the right write.
fn store_battery(
    host: &mut dyn ServerHost,
    tx: &mod_api::Transaction,
    place: &ItemPlace,
    energy: u32,
) -> bool {
    let Some(stack) = place.stack(host) else {
        return false;
    };
    let changed = with_battery(stack, energy);
    match place {
        ItemPlace::PlayerHand { player, hand } => {
            let (uuid, hand) = (*player, *hand);
            let Some(player) = host.player(uuid) else {
                return false;
            };
            let Some(slot) = held_slot(host, &player, hand) else {
                return false;
            };
            host.transaction(|host, _tx| {
                player.set_inventory_slot(host, slot, Some(changed.clone()))
            })
            .is_ok()
        }
        _ => place.exchange(host, tx, &ItemResource::of(&changed), 1) == 1,
    }
}

/// The grid's own buffer, read through the item energy capability the same way
/// a write goes: `(stored, capacity)`, or none where the held item is not a
/// wireless grid (the port answers `ITEM_ENERGY` for that item only).
fn grid_energy(host: &mut dyn ServerHost, uuid: u128, hand: Hand) -> Option<(u32, u32)> {
    let place = ItemPlace::PlayerHand { player: uuid, hand };
    let handler = place.capability(host, &ITEM_ENERGY)?;
    Some((handler.amount(host), handler.capacity(host)))
}

/// Pay `used` from the held wireless grid's own buffer, inside a transaction
/// through the item's capability: how much is left, or the refusal that names
/// the item's energy. A grid whose buffer cannot pay does the use not at all,
/// which is upstream's gate on an item's storage (`stored >= getEnergyUsage()`).
fn pay_grid(
    host: &mut dyn ServerHost,
    uuid: u128,
    hand: Hand,
    used: WirelessUse,
) -> Result<u32, Refusal> {
    let cost = used.cost();
    let place = ItemPlace::PlayerHand { player: uuid, hand };
    let Some(handler) = place.capability(host, &ITEM_ENERGY) else {
        // Not a wireless grid (or no buffer): nothing to pay from.
        return Err(Refusal::NoItemEnergy { stored: 0, cost });
    };
    let stored = handler.amount(host);
    let left = runtime::charge_item(stored, used)?;
    let took = host
        .transaction(|host, tx| {
            Ok::<u32, mod_api::PlayerDataRefusal>(handler.extract(host, tx, cost))
        })
        .unwrap_or(0);
    if took == cost {
        Ok(left)
    } else {
        // The item could not be rewritten (a stack of one in a read-only place):
        // the use does not happen either, so the gate and the charge agree.
        Err(Refusal::NoItemEnergy { stored, cost })
    }
}

// --- the wireless grid item ------------------------------------------------

/// The wireless grid: bind it on a transmitter, use it to open that network's
/// grid view from anywhere inside the transmitter's range.
struct WirelessGrid;

#[mod_api::horizon_part]
impl ItemPart for WirelessGrid {
    /// Using the grid on a wireless transmitter stores the transmitter's
    /// dimension and position in the item (`NetworkItemHelperImpl.bind`); on
    /// any other block the click is left alone.
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        if event.stack.item() != keys().wireless_grid {
            return None;
        }
        let HostSide::Server(host) = host else {
            // The client predicts the swing; only the server writes the item.
            return Some(InteractionResult::Success);
        };
        if kind_at(host, event.pos) != Some(Kind::WirelessTransmitter) {
            return None;
        }
        if bind_grid(host, event.player, event.hand, event.pos, event.stack) {
            return Some(InteractionResult::Success);
        }
        Some(InteractionResult::Fail)
    }

    /// Using the grid opens the network its transmitter exposes, or names the
    /// reason it cannot (`NetworkItemContextImpl.resolveNetwork` +
    /// `NetworkItemPlayerValidator.isValid`).
    fn use_item(
        &self,
        host: HostSide<'_>,
        event: &mut UseItemEvent<'_>,
    ) -> Option<InteractionResult> {
        if event.stack.item() != keys().wireless_grid {
            return None;
        }
        let HostSide::Server(host) = host else {
            return Some(InteractionResult::Success);
        };
        open_wireless_grid(host, event.player, event.stack, event.hand);
        Some(InteractionResult::Success)
    }
}

/// The player's inventory slot the item of `hand` lives in: the hotbar slot
/// they hold for the main hand, the off-hand slot (40) for the other.
fn held_slot(host: &dyn Host, player: &mod_api::PlayerRef, hand: Hand) -> Option<u32> {
    match hand {
        Hand::Main => Some(player.selected_slot(host)? as u32),
        Hand::Off => Some(40),
    }
}

/// The resolve a wireless grid's use makes, and what it does with it: the
/// bound transmitter's network, or the reason the use is refused. A successful
/// resolve then pays the item's own 5 FE for the open (slice 9): the grid is an
/// item with a buffer of its own, and an empty one opens nothing.
fn open_wireless_grid(host: &mut dyn ServerHost, uuid: u128, stack: &ItemStack, hand: Hand) {
    let Some(binding) = binding_of(stack) else {
        tell(
            host,
            uuid,
            "The wireless grid is not bound: use it on a wireless transmitter first.",
        );
        return;
    };
    let Some(player) = host.player(uuid) else {
        return;
    };
    let Some(from) = player.entity().position(host) else {
        return;
    };
    let here = host.dimension().to_owned();
    // Upstream's `resolveNetwork`: the bound dimension's server level, the
    // chunk loaded, and the block entity there a network target.
    let Some(level) = host.level(binding.dimension()) else {
        tell(
            host,
            uuid,
            &format!(
                "Wireless grid {}: no level {} is open.",
                binding_name(&binding),
                binding.dimension()
            ),
        );
        return;
    };
    if node_at(host, level, binding.pos()).map(|node| node.kind) != Some(Kind::WirelessTransmitter)
    {
        tell(
            host,
            uuid,
            &format!(
                "Wireless grid {}: no wireless transmitter there any more.",
                binding_name(&binding)
            ),
        );
        return;
    }
    let net = network_in(host, level, binding.pos());
    let online = net.online();
    let distance = runtime::distance(from, binding.pos());
    let range = range_at(host, level, binding.pos());
    let fit = fit_at(host, level, binding.pos(), TRANSMITTER_SLOTS);
    let charge = runtime::usage(Kind::WirelessTransmitter, fit, 0);
    let energy = runtime::Energy::new(stored_in(host, level, &net), charge);
    match runtime::admit(online, here == binding.dimension(), energy, distance, range) {
        Ok(()) => match pay_grid(host, uuid, hand, WirelessUse::Open) {
            Ok(_) => show_wireless_grid(host, uuid, &binding, level, distance, range, hand),
            Err(refusal) => tell(
                host,
                uuid,
                &format!(
                    "Wireless grid {}: refused ({refusal}).",
                    binding_name(&binding)
                ),
            ),
        },
        Err(refusal) => tell(
            host,
            uuid,
            &format!(
                "Wireless grid {}: refused ({refusal}).",
                binding_name(&binding)
            ),
        ),
    }
}

/// `x,y,z in dimension`, as the wireless grid's lines spell a binding.
fn binding_name(binding: &Binding) -> String {
    format!(
        "{},{},{} in {}",
        binding.pos()[0],
        binding.pos()[1],
        binding.pos()[2],
        binding.dimension()
    )
}

/// Open the 9×6 grid view of `net` as a chest menu in place of whatever the
/// player had open, and remember it so its clicks can move items.
fn show_wireless_grid(
    host: &mut dyn ServerHost,
    uuid: u128,
    binding: &Binding,
    level: LevelRef,
    distance: f64,
    range: u32,
    hand: Hand,
) {
    // Open empty and let the refresh fill it: one place computes the view.
    let items: Vec<Option<ItemStack>> = vec![None; GRID_SLOTS];
    let opened = host.open_menu(
        uuid,
        WIRELESS_GRID_ROWS,
        Text::literal(WIRELESS_GRID_TITLE),
        items.clone(),
        MenuReplace::InPlace,
    );
    let Ok(Some(window)) = opened else {
        tell(host, uuid, "The wireless grid could not open a menu.");
        return;
    };
    with_core(|core| {
        core.menus.insert(
            (uuid, window),
            OpenGrid {
                binding: binding.clone(),
                view: items,
                hand,
            },
        );
    });
    refresh_menu(host, uuid, window, binding, level);
    tell(
        host,
        uuid,
        &format!(
            "Wireless grid {}: opened ({distance:.1} of {}).",
            binding_name(binding),
            runtime::range_label(range)
        ),
    );
}

/// The stacks a menu shows, from a view: one menu slot per entry, in order.
fn view_items(entries: &[Net]) -> Vec<Option<ItemStack>> {
    let mut items: Vec<Option<ItemStack>> = vec![None; GRID_SLOTS];
    for (slot, entry) in items.iter_mut().zip(entries.iter()) {
        *slot = Some(entry.stack().clone());
    }
    items
}

/// What a view promises the network's other views, by resource key and count.
fn promises_of(host: &dyn Host, items: &[Option<ItemStack>]) -> Vec<(String, u32)> {
    items
        .iter()
        .flatten()
        .map(|stack| (resource_key(host, stack), stack.count()))
        .collect()
}

// --- a wireless grid's live view ------------------------------------------

/// One disk of a network read where it lies: its drive and slot, the disk item
/// and the contents read from that item's component.
///
/// A wireless grid reads its network's disks **live** rather than through the
/// tick-maintained cache: its transmitter can be in a level that is not in
/// place, whose drives do not tick, and the item's own view must never be a
/// tick behind the disk it withdraws from.
struct LiveDisk {
    drive: [i32; 3],
    slot: usize,
    item: ItemStack,
    disk: Disk<Net>,
}

/// Every storage disk of `net`, read from the drives' own slots in `level`.
fn live_disks(host: &dyn Host, level: LevelRef, net: &runtime::Network) -> Vec<LiveDisk> {
    let mut out = Vec::new();
    for drive in net.of_kind(Kind::DiskDrive) {
        let Some(entity) = host.block_entity(level, drive) else {
            continue;
        };
        for slot in 0..DRIVE_SLOTS {
            let Ok(Some(item)) = entity.item_slot(host, slot as u32) else {
                continue;
            };
            if !is_disk(&item) {
                continue;
            }
            let disk = read_disk(host, &item);
            out.push(LiveDisk {
                drive,
                slot,
                item,
                disk,
            });
        }
    }
    out
}

/// What a set of live disks holds, as the runtime's network functions take it.
fn disks_of(live: &[LiveDisk]) -> Vec<Disk<Net>> {
    live.iter().map(|disk| disk.disk.clone()).collect()
}

/// Write every disk whose contents changed back into its item, and that item
/// into the drive's slot, in one transaction. The drive's own tick then re-reads
/// the item (its cache is not dirty), so the block grid and this item agree.
fn write_back(host: &mut dyn ServerHost, level: LevelRef, live: &[LiveDisk], disks: &[Disk<Net>]) {
    let _ = host.transaction(|host, tx| {
        write_back_in(host, tx, level, live, disks);
        Ok::<(), mod_api::BlockEntityItemError>(())
    });
}

/// [`write_back`] inside a transaction the caller already opened: a machine's
/// work pass moves items out of a neighbour and puts the disks back in one go,
/// so a failure between the two cannot lose them.
fn write_back_in(
    host: &mut dyn ServerHost,
    tx: &mod_api::Transaction,
    level: LevelRef,
    live: &[LiveDisk],
    disks: &[Disk<Net>],
) {
    for (disk, entries) in live.iter().zip(disks.iter()) {
        if &disk.disk == entries {
            continue;
        }
        let mut item = disk.item.clone();
        write_disk(host, &mut item, entries);
        let Some(entity) = host.block_entity(level, disk.drive) else {
            continue;
        };
        let _ = entity.set_item_slot(host, tx, disk.slot as u32, Some(item));
    }
}

// --- the external storage (slice 14) ---------------------------------------

/// A network's external storage, read where it lies: the block, the container
/// it faces, and that container's contents as one unlimited network storage.
/// The inventory is read live on every use (upstream's external-storage node
/// watches its source for changes), so a `/data merge` or a player's hand edit
/// shows up on the next tick without a cache to invalidate.
struct LiveExternal {
    target: [i32; 3],
    side: Face,
    disk: Disk<Net>,
}

/// The `facing` state of a block in `level` (the level-aware form of
/// [`facing_at`]: a wireless grid may read a network in another dimension).
fn facing_in(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<Face> {
    level
        .block_property(host, pos, "facing")
        .as_deref()
        .and_then(Face::from_property)
}

/// Every external storage of `net`, read from the container each one faces.
fn live_external(
    host: &mut dyn ServerHost,
    level: LevelRef,
    net: &runtime::Network,
) -> Vec<LiveExternal> {
    let mut out = Vec::new();
    for pos in net.of_kind(Kind::ExternalStorage) {
        let Some(face) = facing_in(host, level, pos) else {
            continue;
        };
        let target = neighbour(pos, face);
        let side = face.opposite();
        let Some(handler) = level.capability(host, &ITEMS, target, Some(direction(side))) else {
            continue;
        };
        let mut entries = Vec::new();
        for slot in 0..handler.size(host) {
            let Some(resource) = handler.resource(host, slot) else {
                continue;
            };
            let amount = handler.amount(host, slot);
            if amount > 0
                && let Some(stack) = resource.to_stack(amount)
            {
                entries.push(Net::of(host, &stack));
            }
        }
        let external = runtime::External::of(entries);
        out.push(LiveExternal {
            target,
            side,
            disk: external.disk().clone(),
        });
    }
    out
}

/// The storages the network functions take. External storages come **first**:
/// the port has no insertion-priority surface, so an empty disk in a drive
/// would otherwise shadow the chest forever and the block would be inert.
fn external_disks(live: &[LiveExternal]) -> Vec<Disk<Net>> {
    live.iter().map(|storage| storage.disk.clone()).collect()
}

/// The storages the network functions take, external first (see above).
fn network_disks(live: &[LiveDisk], external: &[LiveExternal]) -> Vec<Disk<Net>> {
    external_disks(external)
        .into_iter()
        .chain(disks_of(live))
        .collect()
}

/// Write every external storage whose contents changed back into the container
/// it faces, inside the caller's transaction. The whole container *is* the
/// exposed storage, so the write replaces its contents with the resulting
/// disk's entries — never the pre-change snapshot (that would undo the move)
/// and never a raw `Items` list on top of the engine's own spill.
fn write_back_external_in(
    host: &mut dyn ServerHost,
    tx: &mod_api::Transaction,
    level: LevelRef,
    live: &[LiveExternal],
    disks: &[Disk<Net>],
) {
    for (storage, disk) in live.iter().zip(disks.iter()) {
        if &storage.disk == disk {
            continue;
        }
        let Some(handler) =
            level.capability(host, &ITEMS, storage.target, Some(direction(storage.side)))
        else {
            continue;
        };
        for slot in 0..handler.size(host) {
            let Some(resource) = handler.resource(host, slot) else {
                continue;
            };
            let amount = handler.amount(host, slot);
            if amount > 0 {
                let _ = handler.extract(host, tx, slot, &resource, amount);
            }
        }
        for entry in disk.entries() {
            let resource = ItemResource::of(entry.stack());
            let _ = handler.insert_any(host, tx, &resource, entry.count());
        }
    }
}

/// The promises every other view of `net` makes, so this view never shows what
/// one of them already shows. A view of another network in the same dimension
/// is not one of them: it holds its own stock.
fn promises_elsewhere(net: &runtime::Network, dimension: &str, place: Place) -> Vec<(String, u32)> {
    let key = ViewKey::new(dimension, net.anchor(), place);
    with_core(|core| core.promises.promised_elsewhere(&key))
}

/// A wireless grid's open menu: its binding, what it currently shows and the
/// hand the grid is in (its own energy is paid from that item).
#[derive(Clone, Debug)]
struct OpenGrid {
    binding: Binding,
    view: Vec<Option<ItemStack>>,
    hand: Hand,
}

/// A click in a wireless grid's menu: what the click asks for, applied to the
/// live disks and the player's own slots.
pub(crate) fn on_menu_clicked(host: &mut dyn ServerHost, event: &MenuClickedEvent) {
    let uuid = event.uuid;
    let window = event.window;
    let Some(open) = with_core(|core| core.menus.get(&(uuid, window)).cloned()) else {
        return;
    };
    let Some(menu) = host.menu(uuid, window) else {
        return;
    };
    let Ok(slot) = usize::try_from(event.slot) else {
        return;
    };
    let (region, view_slot) = match menu.slot_backing(host, slot) {
        Some(MenuSlotBacking::Native(index)) => (Region::View, Some(index)),
        Some(MenuSlotBacking::Player(_)) => (Region::Inventory, None),
        _ => return,
    };
    let binding = open.binding.clone();
    let Some(level) = host.level(binding.dimension()) else {
        return;
    };
    let net = network_in(host, level, binding.pos());
    let live = live_disks(host, level, &net);
    let external = live_external(host, level, &net);
    let external_count = external.len();
    let mut disks = network_disks(&live, &external);
    let carried = menu.carried(host).flatten();
    let slot_has = match view_slot {
        Some(index) => open.view.get(index).is_some_and(Option::is_some),
        None => menu.slot(host, slot).flatten().is_some(),
    };
    let cursor = cursor_shape(host, &carried, view_slot, &open.view);
    let plan = plan_click(
        region,
        slot_has,
        cursor,
        u8::from(event.button == 1),
        event.mode == 1,
    );
    // A click that moves items into or out of the network pays the grid's own
    // 5 FE first (upstream's `WIRELESS_GRID_INSERT`/`_EXTRACT`): an empty grid
    // moves nothing, and says why, while the network keeps its own buffer.
    let used = match plan {
        Plan::Withdraw { .. } => Some(WirelessUse::Extract),
        Plan::Deposit { .. } => Some(WirelessUse::Insert),
        _ => None,
    };
    if let Some(used) = used
        && let Err(refusal) = pay_grid(host, uuid, open.hand, used)
    {
        tell(
            host,
            uuid,
            &format!(
                "Wireless grid {}: refused ({refusal}).",
                binding_name(&binding)
            ),
        );
        return;
    }
    let click = Click {
        player: uuid,
        menu,
        region,
        slot,
        view_slot,
        carried: carried.as_ref(),
    };
    let moved = apply_plan(host, &click, &plan, &mut disks);
    if moved {
        write_back(host, level, &live, &disks[external_count..]);
        let _ = host.transaction(|host, tx| {
            write_back_external_in(host, tx, level, &external, &disks[..external_count]);
            Ok::<(), mod_api::PlayerDataRefusal>(())
        });
    }
    refresh_menu(host, uuid, window, &binding, level);
}

/// What the cursor holds, seen against the resource the click concerns: the
/// stack the clicked view slot shows, or nothing for a slot that shows none.
fn cursor_shape(
    host: &dyn Host,
    carried: &Option<ItemStack>,
    view_slot: Option<usize>,
    view: &[Option<ItemStack>],
) -> Cursor {
    let Some(carried) = carried else {
        return Cursor::Empty;
    };
    let Some(slot_stack) = view_slot
        .and_then(|index| view.get(index))
        .and_then(|slot| slot.as_ref())
    else {
        // An empty view slot takes any cursor as a deposit.
        return Cursor::Other;
    };
    if resource_key(host, carried) != resource_key(host, slot_stack) {
        return Cursor::Other;
    }
    let max = host.max_stack_size(slot_stack.item()).unwrap_or(64);
    Cursor::Same {
        room: max.saturating_sub(slot_stack.count()),
    }
}

/// One click in a wireless grid's menu, as the plan needs it.
struct Click<'a> {
    /// The player who clicked: `menu_clicked` swaps no acting player in.
    player: u128,
    menu: MenuRef,
    /// Which part of the menu it landed on.
    region: Region,
    /// The menu's slot index.
    slot: usize,
    /// The view index, when it landed on one of the view's own slots.
    view_slot: Option<usize>,
    /// What the player carries.
    carried: Option<&'a ItemStack>,
}

/// Do what `plan` says inside one transaction; true when items moved.
fn apply_plan(
    host: &mut dyn ServerHost,
    click: &Click<'_>,
    plan: &Plan,
    disks: &mut [Disk<Net>],
) -> bool {
    let Click {
        player,
        menu,
        region,
        slot,
        view_slot,
        carried,
    } = *click;
    match *plan {
        Plan::Nothing => false,
        Plan::Withdraw { mode, to } => {
            let Some(index) = view_slot else {
                return false;
            };
            let Some(stack) = menu.slot(host, index).flatten() else {
                return false;
            };
            let max = host.max_stack_size(stack.item()).unwrap_or(64);
            let want = runtime::extract_amount(stack.count(), max, mode);
            let key = resource_key(host, &stack);
            match to {
                Destination::Cursor => {
                    let room = match carried {
                        None => max,
                        Some(cursor) if resource_key(host, cursor) == key => {
                            max.saturating_sub(cursor.count())
                        }
                        Some(_) => 0,
                    };
                    let amount = want.min(room);
                    if amount == 0 {
                        return false;
                    }
                    let source = ItemPlace::MenuSlot { menu, slot: index };
                    moved(host, |host, tx| {
                        move_between(host, tx, &source, &menu.cursor(), amount)
                    })
                }
                Destination::Inventory => moved(host, |host, tx| {
                    withdraw_to_inventory(host, tx, disks, &key, want, player)
                }),
            }
        }
        Plan::Deposit { mode } => {
            // A quick-moved inventory slot deposits *that* stack, where a view
            // slot deposits the cursor's.
            let source = if region == Region::Inventory {
                ItemPlace::MenuSlot { menu, slot }
            } else {
                menu.cursor()
            };
            let Some(held) = source.stack(host) else {
                return false;
            };
            let max = host.max_stack_size(held.item()).unwrap_or(64);
            let amount = runtime::insert_amount(max, mode).min(held.count());
            if amount == 0 {
                return false;
            }
            moved(host, |host, tx| {
                deposit_from(host, tx, &source, amount, disks)
            })
        }
        Plan::Inventory(how) => match how {
            runtime::CursorMove::Nothing => false,
            runtime::CursorMove::Take { half } => {
                let amount = match (half, carried) {
                    (false, _) => u32::MAX,
                    (true, Some(cursor)) => cursor.count().div_ceil(2),
                    (true, None) => 1,
                };
                let target = ItemPlace::MenuSlot { menu, slot };
                moved(host, |host, tx| {
                    move_between(host, tx, &target, &menu.cursor(), amount)
                })
            }
            runtime::CursorMove::Place { one } => {
                let Some(cursor) = carried else {
                    return false;
                };
                let amount = if one { 1 } else { cursor.count() };
                let target = ItemPlace::MenuSlot { menu, slot };
                moved(host, |host, tx| {
                    move_between(host, tx, &menu.cursor(), &target, amount)
                })
            }
            runtime::CursorMove::Merge { one } => {
                let Some(cursor) = carried else {
                    return false;
                };
                let room = if one {
                    1
                } else {
                    let max = host.max_stack_size(cursor.item()).unwrap_or(64);
                    max.saturating_sub(
                        menu.slot(host, slot)
                            .flatten()
                            .map_or(0, |held| held.count()),
                    )
                };
                if room == 0 {
                    return false;
                }
                let target = ItemPlace::MenuSlot { menu, slot };
                moved(host, |host, tx| {
                    move_between(host, tx, &menu.cursor(), &target, room)
                })
            }
            runtime::CursorMove::Swap => {
                let target = ItemPlace::MenuSlot { menu, slot };
                moved(host, |host, tx| {
                    swap_between(host, tx, &menu.cursor(), &target)
                })
            }
        },
    }
}

/// Run `f` in one transaction and answer whether any item moved.
fn moved(
    host: &mut dyn ServerHost,
    f: impl FnOnce(&mut dyn ServerHost, &mod_api::Transaction) -> u32,
) -> bool {
    host.transaction(|host, tx| Ok::<u32, mod_api::PlayerDataRefusal>(f(host, tx)))
        .is_ok_and(|amount| amount > 0)
}

/// Move up to `amount` of the source's resource from `from` to `to`, inside
/// `tx`; how much moved.
fn move_between(
    host: &mut dyn ServerHost,
    tx: &mod_api::Transaction,
    from: &ItemPlace,
    to: &ItemPlace,
    amount: u32,
) -> u32 {
    if amount == 0 {
        return 0;
    }
    let Some(resource) = from.resource(host) else {
        return 0;
    };
    let got = from.extract(host, tx, &resource, amount);
    if got == 0 {
        return 0;
    }
    let went = to.insert(host, tx, &resource, got);
    if went < got {
        let _ = from.insert(host, tx, &resource, got - went);
    }
    went
}

/// Exchange what the cursor holds with what a slot holds, inside `tx`.
fn swap_between(
    host: &mut dyn ServerHost,
    tx: &mod_api::Transaction,
    cursor: &ItemPlace,
    slot: &ItemPlace,
) -> u32 {
    let Some(slot_resource) = slot.resource(host) else {
        return 0;
    };
    let from_slot = slot.extract(host, tx, &slot_resource, u32::MAX);
    if from_slot == 0 {
        return 0;
    }
    let cursor_resource = cursor.resource(host);
    let from_cursor = match &cursor_resource {
        Some(resource) => cursor.extract(host, tx, resource, u32::MAX),
        None => 0,
    };
    let mut back = 0;
    if let Some(resource) = &cursor_resource {
        back = slot.insert(host, tx, resource, from_cursor);
    }
    let into_cursor = cursor.insert(host, tx, &slot_resource, from_slot);
    if back < from_cursor
        && let Some(resource) = &cursor_resource
    {
        let _ = cursor.insert(host, tx, resource, from_cursor - back);
    }
    into_cursor
}

/// Take `want` of `key` off the network and put it in the player's inventory,
/// inside `tx`; how much went. What the inventory cannot take goes back onto
/// the network, as upstream's `TransferHelper.transfer` does.
fn withdraw_to_inventory(
    host: &mut dyn ServerHost,
    tx: &mod_api::Transaction,
    disks: &mut [Disk<Net>],
    key: &str,
    want: u32,
    player: u128,
) -> u32 {
    let Some(taken) = extract(disks, key, want) else {
        return 0;
    };
    let resource = ItemResource::of(taken.stack());
    let Some(player) = host.player(player) else {
        return 0;
    };
    let went = player
        .entity()
        .capability(host, &mod_api::ENTITY_ITEMS, ())
        .map_or(0, |handler| {
            handler.insert_any(host, tx, &resource, taken.count())
        });
    if went < taken.count() {
        let _ = insert(disks, taken.with_count(taken.count() - went));
    }
    went
}

/// Take `amount` of what `from` holds and put it on the network, inside `tx`;
/// how much went. What the network cannot take stays where it was.
fn deposit_from(
    host: &mut dyn ServerHost,
    tx: &mod_api::Transaction,
    from: &ItemPlace,
    amount: u32,
    disks: &mut [Disk<Net>],
) -> u32 {
    let Some(resource) = from.resource(host) else {
        return 0;
    };
    let got = from.extract(host, tx, &resource, amount);
    if got == 0 {
        return 0;
    }
    let Some(stack) = resource.to_stack(got) else {
        let _ = from.insert(host, tx, &resource, got);
        return 0;
    };
    let Some(left) = insert(disks, Net::of(host, &stack)) else {
        return got;
    };
    let left_count = left.count();
    if left_count > 0 {
        let _ = from.insert(host, tx, &resource, left_count);
    }
    got - left_count
}

/// Rewrite a wireless grid's menu contents from the network, and remember the
/// promise it makes. Nothing is sent when the view did not change.
fn refresh_menu(
    host: &mut dyn ServerHost,
    uuid: u128,
    window: i32,
    binding: &Binding,
    level: LevelRef,
) {
    let net = network_in(host, level, binding.pos());
    let live = live_disks(host, level, &net);
    let external = live_external(host, level, &net);
    let disks = network_disks(&live, &external);
    let held = promises_elsewhere(
        &net,
        binding.dimension(),
        Place::Wireless {
            player: uuid,
            window,
        },
    );
    let entries = if net.online() {
        view_with(&disks, &held, "", GRID_SLOTS, |stack| {
            host.max_stack_size(stack.stack().item()).unwrap_or(64)
        })
    } else {
        Vec::new()
    };
    let items = view_items(&entries);
    let unchanged = with_core(|core| {
        core.menus
            .get(&(uuid, window))
            .is_some_and(|open| open.view == items)
    });
    if unchanged {
        return;
    }
    let _ = host.menu_set_items(uuid, window, items.clone());
    let promised = promises_of(host, &items);
    let key = ViewKey::new(
        binding.dimension(),
        net.anchor(),
        Place::Wireless {
            player: uuid,
            window,
        },
    );
    with_core(|core| {
        if let Some(open) = core.menus.get_mut(&(uuid, window)) {
            open.view = items;
        }
        core.promises.record(key, promised);
    });
}

/// A wireless grid's menu closed: forget it and release its promise.
pub(crate) fn on_menu_closed(event: &MenuClosedEvent) {
    forget_menu(event.uuid, event.window);
}

/// Forget one wireless grid menu and the promise it made.
fn forget_menu(uuid: u128, window: i32) {
    with_core(|core| {
        core.menus.remove(&(uuid, window));
        core.promises.release(Place::Wireless {
            player: uuid,
            window,
        });
    });
}

/// The server tick: keep every open wireless grid's view current, and forget
/// the menus whose player or window is gone.
pub(crate) fn on_server_tick(host: &mut dyn ServerHost, event: &ServerTickEvent<'_>) {
    if !matches!(event.phase, ServerTickPhase::ServerTickStart { .. }) {
        return;
    }
    with_core(|core| core.ticks = core.ticks.wrapping_add(1));
    // A carry nobody collected is stale: the break that made it printed no
    // drop, so the next break at that position must not inherit it.
    with_core(|core| {
        if let Some(carried) = &core.carried
            && core.ticks.wrapping_sub(carried.tick) > CARRY_TTL
        {
            core.carried = None;
        }
    });
    let open: Vec<(u128, i32, Binding)> = with_core(|core| {
        core.menus
            .iter()
            .map(|((uuid, window), grid)| (*uuid, *window, grid.binding.clone()))
            .collect()
    });
    for (uuid, window, binding) in open {
        if host.menu(uuid, window).is_none() {
            forget_menu(uuid, window);
            continue;
        }
        let Some(level) = host.level(binding.dimension()) else {
            continue;
        };
        refresh_menu(host, uuid, window, &binding, level);
    }
}

// --- blocks ----------------------------------------------------------------

/// The network kind of the loaded block at `pos`, none when it is not loaded
/// or not a network block.
fn kind_at(host: &dyn Host, pos: [i32; 3]) -> Option<Kind> {
    let level = host.level(host.dimension())?;
    if !level.is_loaded(host, pos) {
        return None;
    }
    let block = level.block(host, pos)?;
    keys().kind_of(block)
}

/// The node at `pos` in `level`: its kind, and for a wireless transmitter the
/// face its `facing` state points at (upstream joins the network through that
/// side only).
fn node_at(host: &dyn Host, level: mod_api::LevelRef, pos: [i32; 3]) -> Option<Node> {
    if !level.is_loaded(host, pos) {
        return None;
    }
    let block = level.block(host, pos)?;
    let kind = keys().kind_of(block)?;
    if kind != Kind::WirelessTransmitter {
        return Some(Node::plain(kind));
    }
    let face = level
        .block_property(host, pos, "facing")
        .as_deref()
        .and_then(Face::from_property)?;
    Some(Node::facing(kind, face))
}

/// The network the block at `start` belongs to in `level`, read from the
/// loaded level (`level_in_place` is false for a parked level, whose blocks
/// can still be read, so the same read answers in another dimension).
fn network_in(host: &dyn Host, level: mod_api::LevelRef, start: [i32; 3]) -> runtime::Network {
    network(start, |pos| node_at(host, level, pos))
}

/// The upgrades a network block holds in its own slots, as the energy model
/// counts them (`Fit`): the transmitter's range and creative range upgrades,
/// a machine's speed and stack upgrades. One reader for both, so the charge
/// and the range can never disagree about what a slot holds.
fn fit_at(host: &dyn Host, level: LevelRef, pos: [i32; 3], slots: usize) -> Fit {
    let Some(entity) = host.block_entity(level, pos) else {
        return Fit::default();
    };
    let keys = keys();
    let mut fit = Fit::default();
    for slot in 0..slots as u32 {
        let Ok(Some(stack)) = entity.item_slot(host, slot) else {
            continue;
        };
        let item = stack.item();
        if item == keys.range_upgrade {
            fit.range += 1;
        }
        if item == keys.creative_range_upgrade {
            fit.creative_range = true;
        }
        if item == keys.speed_upgrade {
            fit.speed += 1;
        }
        if item == keys.stack_upgrade {
            fit.stack += 1;
        }
    }
    fit
}

/// A wireless transmitter's range, read from its four upgrade slots: the base
/// 16 blocks plus 8 per range upgrade, or unlimited with a creative range
/// upgrade (upstream's composite range modifier, which reads the same
/// container the menu edits).
fn range_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> u32 {
    let fit = fit_at(host, level, pos, TRANSMITTER_SLOTS);
    runtime::upgraded_range(fit.range, fit.creative_range)
}

/// How many storage disks a drive holds (upstream charges
/// `DISK_DRIVE_PER_DISK` for each).
fn disks_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> u32 {
    let Some(entity) = host.block_entity(level, pos) else {
        return 0;
    };
    (0..DRIVE_SLOTS as u32)
        .filter(|slot| {
            entity
                .item_slot(host, *slot)
                .ok()
                .flatten()
                .is_some_and(|held| is_disk(&held))
        })
        .count() as u32
}

/// What one controller holds, asked of the game's own `block_energy` handler —
/// the same handler a drain goes through, so the gate and the charge cannot
/// disagree about the buffer. (The handler keeps the amount in the saved
/// compound under `energy`, but reading that compound directly while another
/// node writes through the capability lags the write, and a charged network read
/// as empty.)
fn controller_stored(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> u32 {
    level
        .capability(host, &ENERGY, pos, None)
        .map_or(0, |handler| handler.amount(host))
}

/// The network's buffer: what all its controllers hold together (upstream's
/// `EnergyNetworkComponentImpl.getStored` sums its providers).
fn stored_in(host: &mut dyn ServerHost, level: LevelRef, net: &runtime::Network) -> u32 {
    let mut held = Vec::new();
    for pos in net.of_kind(Kind::Controller) {
        held.push(controller_stored(host, level, pos));
    }
    runtime::bank(held)
}

/// The network's capacity: one controller's buffer for each controller
/// (`CONTROLLER_CAPACITY` × controllers).
fn capacity_in(net: &runtime::Network) -> u32 {
    runtime::CONTROLLER_CAPACITY * net.count(Kind::Controller) as u32
}

/// Take a node's charge from the network's controllers, in order, through the
/// game's energy capability so the write is journaled in the transaction and an
/// FE cable sees the same buffer. Returns how much was taken.
fn draw_from(
    host: &mut dyn ServerHost,
    level: LevelRef,
    net: &runtime::Network,
    amount: u32,
) -> u32 {
    let mut left = amount;
    let mut taken = 0;
    for pos in net.of_kind(Kind::Controller) {
        if left == 0 {
            break;
        }
        let Some(handler) = level.capability(host, &ENERGY, pos, None) else {
            continue;
        };
        let got = host
            .transaction(|host, tx| {
                Ok::<u32, mod_api::PlayerDataRefusal>(handler.extract(host, tx, left))
            })
            .unwrap_or(0);
        taken += got;
        left = left.saturating_sub(got);
    }
    taken
}

/// One node's work gate: upstream's `calculateActive` (`stored >= usage`) and,
/// when it passes, its `doWork` charge. A node that cannot pay does no work and
/// nothing is taken — the network refuses the work, it does not run it on debt.
fn pay(host: &mut dyn ServerHost, level: LevelRef, net: &runtime::Network, usage: u32) -> bool {
    if !runtime::Energy::new(stored_in(host, level, net), usage).energized() {
        return false;
    }
    draw_from(host, level, net, usage) == usage
}

/// A node's charge, as the two places that gate and that read the network's
/// draw both compute it.
fn charge_of(host: &dyn Host, level: LevelRef, kind: Kind, pos: [i32; 3]) -> u32 {
    let fit = match kind {
        Kind::WirelessTransmitter => fit_at(host, level, pos, TRANSMITTER_SLOTS),
        Kind::Importer | Kind::Exporter => fit_at(host, level, pos, MACHINE_SLOTS),
        _ => Fit::default(),
    };
    let disks = if kind == Kind::DiskDrive {
        disks_at(host, level, pos)
    } else {
        0
    };
    runtime::usage(kind, fit, disks)
}

/// The network the block at `start` belongs to, in the level in place.
fn network_at(host: &dyn Host, start: [i32; 3]) -> runtime::Network {
    let Some(level) = host.level(host.dimension()) else {
        return runtime::Network::default();
    };
    network_in(host, level, start)
}

// --- the Part: the four network blocks ------------------------------------

/// The Part of the mod's one block-entity type: all four network blocks share
/// it, and it distinguishes them by the block at the position.
struct Network;

#[mod_api::horizon_part]
impl BlockPart for Network {
    /// A directional block faces the block it was placed on (upstream's
    /// `DefaultDirectionType.FACE_CLICKED`: `clickedFace.getOpposite()`), so
    /// the transmitter joins through that side and a machine's source is the
    /// block it was clicked onto.
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        let block = level.state_block(event.state)?;
        if !matches!(
            Kind::from_block(block),
            Some(
                Kind::WirelessTransmitter | Kind::Importer | Kind::Exporter | Kind::ExternalStorage,
            )
        ) {
            return None;
        }
        let face = Face::from_offset(event.context.face.normal())?.opposite();
        Some(Some(level.block_state_id(&format!(
            "{block}[facing={}]",
            face.property()
        ))?))
    }

    /// A player removes one of this mod's machines: the completion gate before
    /// anything of the break changes the world (slice 13). `will_harvest` is the
    /// platform's own answer to "will this break hand an item over" — never in
    /// creative, false for a tool the block refuses — so the slots are taken
    /// only when there will be an item to carry them. Every other break leaves
    /// the container untouched, and the engine's ordinary spill is then the
    /// only copy: the port neither clears slots it cannot hand back nor puts a
    /// payload beside a spill.
    fn on_destroyed_by_player(
        &self,
        host: &mut dyn ServerHost,
        event: &mut OnDestroyedByPlayerEvent<'_>,
    ) -> Option<bool> {
        let pos = event.pos;
        let kind = kind_at(host, pos)?;
        if !kind.carries_contents() {
            return None;
        }
        if runtime::break_plan(event.will_harvest) == runtime::BreakPlan::Carry {
            take_contents(host, pos, kind);
        }
        None
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Network {
    /// Every network block gets a block entity with empty saved data.
    fn block_entity(
        &self,
        _level: LevelSide<'_>,
        _event: &mut BlockEntityEvent<'_>,
    ) -> Option<Compound> {
        Some(Compound::new())
    }

    /// The controller (its `energy_type`), the cable (its connections), the
    /// drive (its disks), the grid (its view), the transmitter (its `active`
    /// state) and the two machines (their work tick) tick.
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = matches!(
            Kind::from_block(event.block),
            Some(
                Kind::Controller
                    | Kind::Cable
                    | Kind::DiskDrive
                    | Kind::Grid
                    | Kind::WirelessTransmitter
                    | Kind::Importer
                    | Kind::Exporter
                    | Kind::ExternalStorage
                    | Kind::Interface
            )
        );
    }

    /// One server tick of a network block (`native_block_entity_tick`).
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        match kind_at(host, pos) {
            Some(Kind::Controller) => controller_tick(host, pos, &mut event.entity),
            Some(Kind::Cable) => cable_tick(host, pos),
            Some(Kind::DiskDrive) => drive_tick(host, pos, &mut event.entity),
            Some(Kind::Grid) => grid_tick(host, pos, &mut event.entity),
            Some(Kind::WirelessTransmitter) => transmitter_tick(host, pos),
            Some(kind @ (Kind::Importer | Kind::Exporter)) => {
                machine_tick(host, pos, kind, &mut event.entity);
            }
            Some(Kind::ExternalStorage) => external_storage_tick(host, pos),
            Some(Kind::Interface) => interface_tick(host, pos, &mut event.entity),
            None => {}
        }
    }

    /// The grid's 9×6 and the drive's 9-slot screen layouts: a mod block's
    /// menu shows on a Horizon client through this layout only.
    fn screen(&self, _host: &dyn Host, event: &mut ScreenEvent<'_>) -> Option<Screen> {
        match Kind::from_block(event.block)? {
            Kind::Grid => {
                let slots = (0..GRID_SLOTS)
                    .map(|index| {
                        let (column, row) = (index % 9, index / 9);
                        Some([8.0 + 18.0 * column as f32, 18.0 + 18.0 * row as f32])
                    })
                    .collect();
                Some(Screen {
                    width: 176.0,
                    height: 222.0,
                    slots,
                    inventory_y: 140.0,
                    inventory_x: 8.0,
                    own_labels: false,
                    own_background: false,
                    ..Screen::default()
                })
            }
            Kind::DiskDrive => {
                let slots = (0..DRIVE_SLOTS)
                    .map(|index| Some([8.0 + 18.0 * index as f32, 16.0]))
                    .collect();
                Some(Screen {
                    width: 176.0,
                    height: 133.0,
                    slots,
                    inventory_y: 84.0,
                    inventory_x: 8.0,
                    own_labels: false,
                    own_background: false,
                    ..Screen::default()
                })
            }
            Kind::WirelessTransmitter => {
                // Upstream's `WirelessTransmitterScreen`: four upgrade slots in
                // the top row, the player's inventory below.
                let slots = (0..TRANSMITTER_SLOTS)
                    .map(|index| Some([8.0 + 18.0 * index as f32, 16.0]))
                    .collect();
                Some(Screen {
                    width: 176.0,
                    height: 133.0,
                    slots,
                    inventory_y: 84.0,
                    inventory_x: 8.0,
                    own_labels: false,
                    own_background: false,
                    ..Screen::default()
                })
            }
            Kind::Importer | Kind::Exporter => {
                // Upstream's `ImporterScreen`/`ExporterScreen`: the nine filter
                // entries in a row, the four upgrade slots under them (upstream
                // puts the upgrades in a right-hand column; the pin's 176-wide
                // panel keeps them on their own row), the inventory below.
                let mut slots: Vec<Option<[f32; 2]>> = (0..FILTER_SLOTS)
                    .map(|index| Some([8.0 + 18.0 * index as f32, 16.0]))
                    .collect();
                slots.extend(
                    (0..MACHINE_UPGRADES).map(|index| Some([8.0 + 18.0 * index as f32, 40.0])),
                );
                Some(Screen {
                    width: 176.0,
                    height: 133.0,
                    slots,
                    inventory_y: 84.0,
                    inventory_x: 8.0,
                    own_labels: false,
                    own_background: false,
                    ..Screen::default()
                })
            }
            Kind::Interface => {
                // Upstream's `InterfaceScreen`: the nine configured resources
                // in the top row, the nine intermediate slots below them.
                let mut slots: Vec<Option<[f32; 2]>> = (0..runtime::INTERFACE_SLOTS)
                    .map(|index| Some([8.0 + 18.0 * index as f32, 16.0]))
                    .collect();
                slots.extend(
                    (0..runtime::INTERFACE_SLOTS)
                        .map(|index| Some([8.0 + 18.0 * index as f32, 40.0])),
                );
                Some(Screen {
                    width: 176.0,
                    height: 133.0,
                    slots,
                    inventory_y: 84.0,
                    inventory_x: 8.0,
                    own_labels: false,
                    own_background: false,
                    ..Screen::default()
                })
            }
            Kind::Controller | Kind::Cable | Kind::ExternalStorage => None,
        }
    }
}

// --- the ticks -------------------------------------------------------------

/// A controller's tick: keep its buffer inside the declared capacity and say
/// which of upstream's four energy levels it is at, in the block's
/// `energy_type` state (`ControllerBlockEntityTicker.updateEnergyTypeInLevel`).
/// The buffer itself is the game's `block_energy` handler's; this only clamps
/// and reads it.
fn controller_tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    let capacity = runtime::CONTROLLER_CAPACITY;
    let held = match field(&entity.nbt, ENERGY_KEY) {
        Some(Tag::Int(held)) => u32::try_from(*held).unwrap_or(0),
        _ => 0,
    };
    // Clamp only. A block entity is out of the world during its own tick, so the
    // capability cannot answer for it here and its payload is the only reading;
    // writing the amount back from a snapshot would undo another node's drain
    // from the same tick, so nothing is written unless the buffer is over its
    // capacity (which the handler itself refuses to do).
    let stored = held.min(capacity);
    if stored != held {
        entity.nbt.insert(
            ENERGY_KEY.into(),
            Tag::Int(i32::try_from(stored).unwrap_or(0)),
        );
    }
    let energy_type = EnergyType::of(stored, capacity);
    let key = (host.dimension().to_owned(), pos);
    let previous = with_core(|core| core.energy.insert(key, energy_type));
    if previous != Some(energy_type) {
        host.set_block_property(pos, "energy_type", energy_type.property());
    }
}

/// A cable's six connection bits, from the blocks beside it (upstream's
/// `computeConnections`: a neighbour with a network node accepts the
/// connection, and every network block of the slice has one).
fn cable_tick(host: &mut dyn ServerHost, pos: [i32; 3]) {
    let mut bits = 0u8;
    for (index, at) in runtime::neighbours(pos).into_iter().enumerate() {
        if kind_at(host, at).is_some() {
            bits |= 1 << index;
        }
    }
    let key = (host.dimension().to_owned(), pos);
    let previous = with_core(|core| core.connections.get(&key).copied());
    if previous == Some(bits) {
        return;
    }
    // The declared state properties, in `neighbours`' order.
    const NAMES: [&str; 6] = ["north", "east", "south", "west", "up", "down"];
    for (index, name) in NAMES.into_iter().enumerate() {
        let value = if bits & (1 << index) != 0 {
            "true"
        } else {
            "false"
        };
        host.set_block_property(pos, name, value);
    }
    with_core(|core| core.connections.insert(key, bits));
}

/// A drive's tick: pay its charge (10 FE plus 4 per disk), then mirror its disk
/// items into the cache, or write a change the grid made back into the disk's
/// component. A drive its network cannot pay does no work: the disks are not
/// mirrored, so the grids of that network list nothing (upstream's inactive
/// node).
fn drive_tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    let net = network_at(host, pos);
    let Some(level) = host.level(host.dimension()) else {
        return;
    };
    let cost = charge_of(host, level, Kind::DiskDrive, pos);
    if !pay(host, level, &net, cost) {
        return;
    }
    let dimension = host.dimension().to_owned();
    for slot in 0..DRIVE_SLOTS {
        let key = (dimension.clone(), pos, slot);
        let disk = entity
            .items
            .get(slot)
            .and_then(|held| held.as_ref())
            .filter(|held| is_disk(held))
            .cloned();
        let Some(mut item) = disk else {
            with_core(|core| {
                core.disks.remove(&key);
                core.dirty.remove(&key);
            });
            continue;
        };
        let dirty = with_core(|core| core.dirty.remove(&key));
        if dirty {
            let cached = with_core(|core| core.disks.get(&key).cloned());
            if let Some(cached) = cached {
                write_disk(host, &mut item, &cached);
                if let Some(slot_item) = entity.items.get_mut(slot) {
                    *slot_item = Some(item);
                }
            }
        } else {
            let cached = read_disk(host, &item);
            with_core(|core| core.disks.insert(key, cached));
        }
    }
}

/// The view slots a grid last put in its menu, and which of them hold a stack
/// the network refused (the player's own until they empty it).
struct ViewState {
    slots: Vec<Option<Net>>,
    owned: Vec<bool>,
    initialised: bool,
}

/// One saved field by its key. A compound is keyed by Java's lossless UTF-16
/// strings, whose type the API does not export (and a second SDK path dependency
/// is what the launcher refuses), so the lookup spells the key the way Java does
/// and compares the stored units. The same shape as the API's own `Tag::get`.
fn field<'a>(data: &'a Compound, name: &str) -> Option<&'a Tag> {
    let units: Vec<u16> = name.encode_utf16().collect();
    data.iter()
        .find(|(key, _)| key.0 == units)
        .map(|(_, tag)| tag)
}

fn view_state(host: &dyn Host, entity: &ModBlockEntityFields) -> ViewState {
    let mut state = ViewState {
        slots: vec![None; GRID_SLOTS],
        owned: vec![false; GRID_SLOTS],
        initialised: false,
    };
    let Some(Tag::List(marks)) = field(&entity.nbt, VIEW_KEY) else {
        return state;
    };
    state.initialised = true;
    for (index, mark) in marks.iter().enumerate().take(GRID_SLOTS) {
        let Tag::Compound(mark) = mark else {
            continue;
        };
        state.owned[index] = matches!(field(mark, "owned"), Some(Tag::Byte(1)));
        let Some(Tag::Compound(item)) = field(mark, "item") else {
            continue;
        };
        state.slots[index] = ItemStack::from_nbt(host, item)
            .filter(|stack| stack.count() > 0)
            .map(|stack| Net::of(host, &stack));
    }
    state
}

fn save_view(
    host: &dyn Host,
    entity: &mut ModBlockEntityFields,
    slots: &[Option<Net>],
    owned: &[bool],
) {
    let marks: Vec<Tag> = (0..GRID_SLOTS)
        .map(|index| {
            let mut mark = Compound::new();
            if owned.get(index).copied().unwrap_or(false) {
                mark.insert("owned".into(), Tag::Byte(1));
            }
            if let Some(item) = slots
                .get(index)
                .and_then(|slot| slot.as_ref())
                .and_then(|stack| stack.stack().to_nbt(host))
            {
                mark.insert("item".into(), Tag::Compound(item));
            }
            Tag::Compound(mark)
        })
        .collect();
    entity.nbt.insert(VIEW_KEY.into(), Tag::List(marks));
}

/// A grid's tick: reconcile what the player did to the menu's slots with the
/// network, then show the network's resources again.
fn grid_tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    let dimension = host.dimension().to_owned();
    let net = network_at(host, pos);
    let online = net.online();
    // The node works only when the network can pay its grid charge: a grid on a
    // 0-FE network is inactive and lists nothing, while the disks keep what they
    // hold (upstream's `calculateActive` + `doWork`).
    let level = host.level(host.dimension());
    let mut works = false;
    if online && let Some(level) = level {
        works = pay(
            host,
            level,
            &net,
            runtime::usage(Kind::Grid, Fit::default(), 0),
        );
    }
    let drives: Vec<[i32; 3]> = net.of_kind(Kind::DiskDrive).collect();
    let keys: Vec<DiskKey> = drive_keys(&dimension, &drives);
    // Only the slots the drives already mirror, in the same order, so a
    // change written back lands in the disk it came from.
    let (used, drive_disks): (Vec<DiskKey>, Vec<Disk<Net>>) = with_core(|core| {
        keys.iter()
            .filter(|key| core.disks.contains_key(*key))
            .map(|key| (key.clone(), core.disks[key].clone()))
            .unzip()
    });
    // External storages are read live and come first, so a free disk slot
    // cannot shadow the inventory the block exposes (no priority surface).
    let external = level.map_or_else(Vec::new, |level| live_external(host, level, &net));
    let external_count = external.len();
    let mut disks: Vec<Disk<Net>> = external
        .iter()
        .map(|storage| storage.disk.clone())
        .chain(drive_disks)
        .collect();

    let mut state = view_state(host, entity);
    let after: Vec<Option<Net>> = (0..GRID_SLOTS)
        .map(|index| {
            entity
                .items
                .get(index)
                .and_then(|slot| slot.as_ref())
                .filter(|stack| stack.count() > 0)
                .map(|stack| Net::of(host, stack))
        })
        .collect();
    // What the slots hold once this tick's deltas are applied. A refused
    // deposit leaves only its *remainder* there: the slot must be rebuilt from
    // this, never from `after`, or the player keeps the refused items too.
    let mut current = after.clone();
    if state.initialised {
        for delta in runtime::deltas(&state.slots, &after, &state.owned) {
            match delta {
                Delta::Withdraw { slot, stack } => {
                    // The player took it out of the menu already; take it off
                    // the network. `view_with` promised this grid at most what
                    // the network held, so no other grid can have taken it.
                    extract(&mut disks, &stack.key(), stack.count());
                    current[slot] = None;
                }
                Delta::Deposit { slot, stack } => {
                    // `None` means the network took every item; only a real
                    // remainder stays in the slot as the player's own. (The
                    // answer is an `Option`, not a zero-count stack: a stack
                    // cannot hold zero items, and a clamped one reads as a
                    // refused deposit — the 1-item ghost this fix removes.)
                    let left = if works {
                        insert(&mut disks, stack)
                    } else {
                        Some(stack)
                    };
                    match left {
                        Some(left) => {
                            state.owned[slot] = true;
                            if let Some(slot_item) = entity.items.get_mut(slot) {
                                *slot_item = Some(left.stack().clone());
                            }
                            current[slot] = Some(left);
                        }
                        None => current[slot] = None,
                    }
                }
            }
        }
    }
    // Any slot the player emptied stops being theirs.
    for (index, flag) in state.owned.iter_mut().enumerate() {
        if *flag && after[index].is_none() {
            *flag = false;
        }
    }

    // Write the network's changes back into the drives' caches. Only a disk the
    // view actually changed is marked dirty: marking every disk on every tick
    // made the drive rewrite each disk item each tick, and a change made to a
    // disk's component from outside (a `/data merge`, another tool) was replaced
    // by the stale cache one tick later.
    with_core(|core| {
        for (key, disk) in used.iter().zip(disks[external_count..].iter()) {
            let changed = core.disks.get(key) != Some(disk);
            if changed {
                core.disks.insert(key.clone(), disk.clone());
                core.dirty.insert(key.clone());
            }
        }
    });
    // The external storages' changes go back to the containers they face in
    // their own transaction; the drive caches above are in-memory only.
    if external_count > 0
        && let Some(level) = level
    {
        let _ = host.transaction(|host, tx| {
            write_back_external_in(host, tx, level, &external, &disks[..external_count]);
            Ok::<(), mod_api::BlockEntityItemError>(())
        });
    }

    // Rebuild the view: the player's own stacks stay, the rest shows what the
    // network still has once every *other* grid's promise is set aside, in the
    // grid's sort order.
    let filter = with_core(|core| {
        core.filters
            .get(&(dimension.clone(), pos))
            .cloned()
            .unwrap_or_default()
    });
    let held = with_core(|core| {
        let key = ViewKey::new(&dimension, net.anchor(), Place::Grid(pos));
        // Forget the promises of grids that are no longer there: a broken grid
        // must not keep hiding the network's stock from the others. The block
        // is the truth, not the cache (a replaced block can leave its entity
        // behind for a tick). A wireless grid's menu is forgotten by its own
        // close, and by the tick when its window is gone.
        core.promises.retain(|key| {
            key.dimension != dimension
                || match key.place {
                    Place::Grid(grid) => grid == pos || kind_at(host, grid) == Some(Kind::Grid),
                    Place::Wireless { .. } => true,
                }
        });
        core.promises.promised_elsewhere(&key)
    });
    let entries = if works {
        view_with(&disks, &held, &filter, GRID_SLOTS, |stack| {
            host.max_stack_size(stack.stack().item()).unwrap_or(64)
        })
    } else {
        Vec::new()
    };
    let mut slots: Vec<Option<ItemStack>> = vec![None; GRID_SLOTS];
    let mut owned = vec![false; GRID_SLOTS];
    let mut entries = entries.into_iter();
    for index in 0..GRID_SLOTS {
        if state.owned[index]
            && let Some(keep) = current[index].clone()
        {
            slots[index] = Some(keep.owned());
            owned[index] = true;
            continue;
        }
        slots[index] = entries.next().map(Net::owned);
    }
    // Promise the next grid only what this one does not show.
    let promised = slots
        .iter()
        .flatten()
        .map(|stack| (resource_key(host, stack), stack.count()))
        .collect::<Vec<_>>();
    with_core(|core| {
        core.promises.record(
            ViewKey::new(&dimension, net.anchor(), Place::Grid(pos)),
            promised,
        );
    });
    entity.items = slots;
    let rebuilt: Vec<Option<Net>> = entity
        .items
        .iter()
        .map(|slot| slot.as_ref().map(|stack| Net::of(host, stack)))
        .collect();
    save_view(host, entity, &rebuilt, &owned);

    // The block state says whether the grid works: an offline or unpowered grid
    // is dark.
    let previous = with_core(|core| core.active.get(&(dimension.clone(), pos)).copied());
    if previous != Some(works) {
        host.set_block_property(pos, "active", if works { "true" } else { "false" });
        with_core(|core| core.active.insert((dimension, pos), works));
    }
}

/// An external storage's tick (slice 14): pay its 6 FE/tick while its network
/// is online, as upstream's `doWork` does. The faced inventory itself is read
/// live by whichever grid or machine is using the network.
fn external_storage_tick(host: &mut dyn ServerHost, pos: [i32; 3]) {
    let net = network_at(host, pos);
    if net.online()
        && let Some(level) = host.level(host.dimension())
    {
        pay(
            host,
            level,
            &net,
            runtime::usage(Kind::ExternalStorage, Fit::default(), 0),
        );
    }
}

/// An interface's tick (slice 15): pay its 4 FE/tick while the network is
/// online, set `active` from that gate, then run one work pass per slot.
///
/// Upstream's `InterfaceNetworkNode.doWork` fills the nine intermediate slots
/// from the network with what the filter names (up to the 64-item item export
/// limit), returns what the filter no longer names, and inserts what an
/// external block put into an unconfigured slot — all instantly, with no
/// upgrade interval. The buffer itself is the `ITEMS` container a hopper sees,
/// read here from the block entity's own nine buffer slots.
fn interface_tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
    let net = network_at(host, pos);
    let mut active = false;
    if net.online()
        && let Some(level) = host.level(host.dimension())
    {
        active = pay(
            host,
            level,
            &net,
            runtime::usage(Kind::Interface, Fit::default(), 0),
        );
    }
    let key = (host.dimension().to_owned(), pos);
    let previous = with_core(|core| core.active.get(&key).copied());
    if previous != Some(active) {
        host.set_block_property(pos, "active", if active { "true" } else { "false" });
        with_core(|core| core.active.insert(key, active));
    }
    if !active {
        return;
    }
    let Some(level) = host.level(host.dimension()) else {
        return;
    };

    // The network's storages, without this block's own buffer: extracting from
    // the buffer would just move items from a slot back into itself.
    let live = live_disks(host, level, &net);
    let external = live_external(host, level, &net);
    let external_count = external.len();
    let mut disks = network_disks(&live, &external);

    let wanted: Vec<Option<String>> = (0..runtime::INTERFACE_SLOTS)
        .map(|slot| {
            entity.items[slot]
                .as_ref()
                .map(|stack| resource_key(host, stack))
        })
        .collect();
    for (slot, want) in wanted.iter().enumerate() {
        let buffer = INTERFACE_FILTER_SLOTS + slot;
        let exported = entity.items[buffer]
            .as_ref()
            .map(|stack| (resource_key(host, stack), stack.count()));
        let available = want
            .as_deref()
            .map_or(0, |key| runtime::amount(&disks, key) as u32);
        let plan = runtime::interface_plan(
            want.as_deref(),
            exported.as_ref().map(|(key, count)| (key.as_str(), *count)),
            available,
            runtime::INTERFACE_EXPORT_LIMIT,
        );
        match plan {
            runtime::InterfaceStep::Nothing => {}
            runtime::InterfaceStep::Extract { key, amount } => {
                let Some(got) = runtime::extract(&mut disks, &key, amount) else {
                    continue;
                };
                let taken = got.count();
                entity.items[buffer] = Some(match exported {
                    Some((_, held)) => {
                        let mut stack = got.owned();
                        let _ = stack.set_count(held + taken);
                        stack
                    }
                    None => got.owned(),
                });
            }
            runtime::InterfaceStep::Return { key: _, amount: _ } => {
                let Some(stack) = entity.items[buffer].clone() else {
                    continue;
                };
                let put = Net::of(host, &stack);
                entity.items[buffer] = runtime::insert(&mut disks, put).map(Net::owned);
            }
        }
    }

    let _ = host.transaction(|host, tx| {
        write_back_in(host, tx, level, &live, &disks[external_count..]);
        write_back_external_in(host, tx, level, &external, &disks[..external_count]);
        Ok::<(), mod_api::PlayerDataRefusal>(())
    });
}

/// A transmitter's tick: `active` is whether the network it joins has a
/// controller **and can pay the transmitter's charge** (16 FE a tick plus 8 per
/// range upgrade). It joins through its facing side only, so a transmitter
/// placed with its face away from a cable is offline and answers no use; one
/// whose network cannot pay is inactive and answers no use either (upstream's
/// `stored >= getEnergyUsage()`).
fn transmitter_tick(host: &mut dyn ServerHost, pos: [i32; 3]) {
    let net = network_at(host, pos);
    let mut active = false;
    if net.online()
        && let Some(level) = host.level(host.dimension())
    {
        let cost = charge_of(host, level, Kind::WirelessTransmitter, pos);
        active = pay(host, level, &net, cost);
    }
    let key = (host.dimension().to_owned(), pos);
    let previous = with_core(|core| core.active.get(&key).copied());
    if previous == Some(active) {
        return;
    }
    host.set_block_property(pos, "active", if active { "true" } else { "false" });
    with_core(|core| core.active.insert(key, active));
}

// --- the two transfer machines (slice 7) -----------------------------------

/// The working face of a machine: its `facing` state (upstream's `direction`),
/// the side whose neighbour it reads or writes.
fn facing_at(host: &dyn Host, pos: [i32; 3]) -> Option<Face> {
    let level = host.level(host.dimension())?;
    level
        .block_property(host, pos, "facing")
        .as_deref()
        .and_then(Face::from_property)
}

/// The block one step out of `pos` through `face`.
fn neighbour(pos: [i32; 3], face: Face) -> [i32; 3] {
    let step = face.offset();
    [pos[0] + step[0], pos[1] + step[1], pos[2] + step[2]]
}

/// The pin's direction for one of the port's faces (the side a machine asks a
/// neighbour's capability through).
fn direction(face: Face) -> mod_api::Direction {
    match face {
        Face::North => mod_api::Direction::North,
        Face::East => mod_api::Direction::East,
        Face::South => mod_api::Direction::South,
        Face::West => mod_api::Direction::West,
        Face::Up => mod_api::Direction::Up,
        Face::Down => mod_api::Direction::Down,
    }
}

/// What a machine's own slots hold: how many speed and stack upgrades, and the
/// resource keys its filter entries name (an empty list is upstream's empty
/// `BLOCK` filter, which blocks nothing).
fn machine_slots(host: &dyn Host, items: &[Option<ItemStack>]) -> (u32, u32, Vec<String>) {
    let keys = keys();
    let mut speed = 0;
    let mut stacks = 0;
    for held in items
        .iter()
        .skip(FILTER_SLOTS)
        .take(MACHINE_UPGRADES)
        .flatten()
    {
        if held.item() == keys.speed_upgrade {
            speed += 1;
        }
        if held.item() == keys.stack_upgrade {
            stacks += 1;
        }
    }
    let filter = items
        .iter()
        .take(FILTER_SLOTS)
        .flatten()
        .map(|held| resource_key(host, held))
        .collect();
    (speed, stacks, filter)
}

/// An importer's or exporter's tick: every [`runtime::work_interval`] server
/// ticks one work pass moves items between the neighbour the machine faces and
/// the network's disks, all in one transaction.
fn machine_tick(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    kind: Kind,
    entity: &mut ModBlockEntityView<'_>,
) {
    let Some(face) = facing_at(host, pos) else {
        return;
    };
    let (speed, stacks, filter) = machine_slots(host, &entity.items);
    let interval = runtime::work_interval(speed);
    // Upstream's throttled ticker counts every tick, online or not.
    let key = (host.dimension().to_owned(), pos);
    let due = with_core(|core| {
        let ticks = core.work.entry(key).or_insert(0);
        let due = runtime::work_due(*ticks, interval);
        *ticks = ticks.wrapping_add(1);
        due
    });
    if !due {
        return;
    }
    let net = network_at(host, pos);
    if !net.online() {
        return;
    }
    let Some(level) = host.level(host.dimension()) else {
        return;
    };
    // The pass's charge, upstream's `stored >= getEnergyUsage()` gate and its
    // `doWork` draw: 2 FE plus 4 per speed upgrade and 16 per stack upgrade.
    let fit = Fit {
        speed,
        stack: stacks,
        ..Fit::default()
    };
    if !pay(host, level, &net, runtime::usage(kind, fit, 0)) {
        return;
    }
    let target = neighbour(pos, face);
    let live = live_disks(host, level, &net);
    let external = live_external(host, level, &net);
    let external_count = external.len();
    let mut disks = network_disks(&live, &external);
    let work = Work {
        level,
        target,
        side: face.opposite(),
        filter: &filter,
        quota: runtime::transfer_quota(stacks),
    };
    let _ = host.transaction(|host, tx| {
        match kind {
            Kind::Importer => importer_pass(host, tx, &work, &mut disks),
            Kind::Exporter => exporter_pass(host, tx, &work, &mut disks),
            _ => 0,
        };
        write_back_in(host, tx, level, &live, &disks[external_count..]);
        write_back_external_in(host, tx, level, &external, &disks[..external_count]);
        Ok::<(), mod_api::PlayerDataRefusal>(())
    });
}

/// One machine work pass's inputs: the neighbour it works on (its position and
/// the side of that block the machine reaches it through), the filter entries
/// and how much one pass may move.
struct Work<'a> {
    level: LevelRef,
    target: [i32; 3],
    side: Face,
    filter: &'a [String],
    quota: u32,
}

/// One importer work pass: take up to `quota` of the first resource the filter
/// allows out of the neighbour's container and put it on the network
/// (`ImporterTransferStrategyImpl`); what the network cannot take stays in the
/// source.
fn importer_pass(
    host: &mut dyn ServerHost,
    tx: &mod_api::Transaction,
    work: &Work<'_>,
    disks: &mut [Disk<Net>],
) -> u32 {
    let Some(handler) =
        work.level
            .capability(host, &ITEMS, work.target, Some(direction(work.side)))
    else {
        return 0;
    };
    let mut source: Vec<Option<(String, u32)>> = Vec::new();
    for slot in 0..handler.size(host) {
        let entry = handler.resource(host, slot).map(|resource| {
            let key = resource
                .to_stack(1)
                .map_or_else(String::new, |stack| resource_key(host, &stack));
            (key, handler.amount(host, slot))
        });
        source.push(entry);
    }
    let Some((slot, _, amount)) = runtime::import_plan(&source, work.filter, work.quota) else {
        return 0;
    };
    let Some(resource) = handler.resource(host, slot) else {
        return 0;
    };
    let taken = handler.extract(host, tx, slot, &resource, amount);
    if taken == 0 {
        return 0;
    }
    let Some(stack) = resource.to_stack(taken) else {
        let _ = handler.insert(host, tx, slot, &resource, taken);
        return 0;
    };
    let Some(left) = insert(disks, Net::of(host, &stack)) else {
        return taken;
    };
    let left_count = left.count();
    if left_count > 0 {
        let _ = handler.insert(host, tx, slot, &resource, left_count);
    }
    taken - left_count
}

/// One exporter work pass: push the first filter entry the network holds into
/// the neighbour's container (`ExporterTransferStrategyImpl`), telling each
/// filter entry in turn; what the destination refuses goes back on the
/// network.
fn exporter_pass(
    host: &mut dyn ServerHost,
    tx: &mod_api::Transaction,
    work: &Work<'_>,
    disks: &mut [Disk<Net>],
) -> u32 {
    let Some(handler) =
        work.level
            .capability(host, &ITEMS, work.target, Some(direction(work.side)))
    else {
        return 0;
    };
    let held: Vec<(String, u64)> = runtime::aggregate(disks)
        .iter()
        .map(|entry| (entry.key(), u64::from(entry.count())))
        .collect();
    let Some((key, amount)) = runtime::export_plan(work.filter, &held, work.quota) else {
        return 0;
    };
    let Some(taken) = extract(disks, &key, amount) else {
        return 0;
    };
    let resource = ItemResource::of(taken.stack());
    let went = handler.insert_any(host, tx, &resource, taken.count());
    if went < taken.count() {
        let _ = insert(disks, taken.with_count(taken.count() - went));
    }
    went
}

/// Every cache key a set of drives can hold.
fn drive_keys(dimension: &str, drives: &[[i32; 3]]) -> Vec<DiskKey> {
    drives
        .iter()
        .flat_map(|drive| (0..DRIVE_SLOTS).map(move |slot| (dimension.to_owned(), *drive, slot)))
        .collect()
}

// --- a broken machine's slots on its item (slice 10) -----------------------

/// A player is breaking a block: when it is a machine that owns its slots, take
/// them out of the block entity **before** the removal, so the engine's own
/// container spill finds nothing, and hold them for this break's `block_drops`.
///
/// The clear is verified: if any slot still holds something afterwards (a
/// refused write), nothing is recorded and the engine's spill is the fallback —
/// a machine's items are never both carried and spilled.
/// Empty the machine's own slots into the pending carry, in one transaction,
/// and record them only when the read-back says they are gone. Called from
/// `on_destroyed_by_player`, once the break is known to hand an item over
/// (`runtime::break_plan`), so a break that hands nothing over keeps its
/// container and the engine's own spill carries it.
fn take_contents(host: &mut dyn ServerHost, pos: [i32; 3], kind: Kind) {
    let Some(level) = host.level(host.dimension()) else {
        return;
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return;
    };
    let count = kind.own_slots() as u32;
    let held: Vec<Option<ItemStack>> = (0..count)
        .map(|slot| entity.item_slot(&*host, slot).ok().flatten())
        .collect();
    let Some(carried) = runtime::carry(&held) else {
        return;
    };
    let mut slots = Vec::with_capacity(carried.len());
    let mut stacks = Vec::with_capacity(carried.len());
    for (slot, stack) in carried {
        let Some(mut entry) = stack.to_nbt(&*host) else {
            return;
        };
        let Ok(byte) = i8::try_from(slot) else {
            return;
        };
        entry.insert("Slot".into(), Tag::Byte(byte));
        slots.push((slot, entry));
        stacks.push((slot, stack));
    }
    let cleared = host
        .transaction(|host, tx| {
            for slot in 0..count {
                entity.set_item_slot(host, tx, slot, None)?;
            }
            Ok::<(), BlockEntityItemError>(())
        })
        .is_ok();
    let emptied = (0..count).all(|slot| matches!(entity.item_slot(&*host, slot), Ok(None)));
    if !cleared || !emptied {
        // The write was refused: the slots stay where they were and the engine's
        // container spill carries them, as it did before this slice.
        return;
    }
    with_core(|core| {
        core.carried = Some(Carried {
            dimension: host.dimension().to_owned(),
            pos,
            tick: core.ticks,
            slots,
            stacks,
        });
    });
}

/// The block's own drop: put the pending carry on it as a **fresh** payload —
/// this port's `RefinedStorageContents` under vanilla's `block_entity_data` —
/// when the item is there. The pre-removal snapshot is not copied: its `Items`
/// list is what the engine has already spilled, and carrying vanilla's `Items`
/// would hand the player both.
///
/// The slots were taken at a break the platform said would hand an item over
/// (`on_destroyed_by_player`'s `will_harvest`). Should that item still not be in
/// the list — another mod emptied the loot, a table lost its pool — the taken
/// slots go back on the ground as the items they were: a break either transfers
/// them or drops them, never neither.
pub(crate) fn on_block_drops(level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) -> Flow {
    let Some(block) = level.state_block(event.state) else {
        return Flow::Continue;
    };
    let Some(kind) = Kind::from_block(block) else {
        return Flow::Continue;
    };
    if !kind.carries_contents() {
        return Flow::Continue;
    }
    let dimension = event.dimension.key().id().to_owned();
    let pos = [event.pos.x, event.pos.y, event.pos.z];
    let Some(carried) = with_core(|core| {
        core.carried
            .as_ref()
            .filter(|carried| carried.dimension == dimension && carried.pos == pos)
            .map(|carried| (carried.slots.clone(), carried.stacks.clone()))
    }) else {
        return Flow::Continue;
    };
    let Some(item) = keys().item_of(kind) else {
        return Flow::Continue;
    };
    let (slots, stacks) = carried;
    with_core(|core| core.carried = None);
    let restored = runtime::restore(&slots, kind.own_slots());
    if restored.is_empty() {
        return Flow::Continue;
    }
    let present = event.drops.iter().any(|drop| drop.stack.item() == item);
    match runtime::carry_plan(present) {
        runtime::CarryPlan::OnTheItem => {
            let list = restored
                .into_iter()
                .map(|(_, entry)| Tag::Compound(entry))
                .collect();
            let data = Tag::Compound(Compound::from([
                ("id".into(), Tag::String(BE_TYPE.into())),
                (CONTENTS_KEY.into(), Tag::List(list)),
            ]));
            let mut patch = Compound::new();
            patch.insert(BLOCK_ENTITY_DATA.into(), data);
            for drop in event.drops.iter_mut() {
                if drop.stack.item() == item {
                    let _ = drop.stack.apply_components(&patch);
                }
            }
        }
        runtime::CarryPlan::BackOnTheGround => {
            // The engine's own spill position and delay (`Block.popResource`:
            // the block's centre, ten ticks before pickup), so a player who
            // breaks the machine finds what it held where vanilla would put it.
            let at = [
                f64::from(pos[0]) + 0.5,
                f64::from(pos[1]) + 0.5,
                f64::from(pos[2]) + 0.5,
            ];
            for (_, stack) in runtime::restore(&stacks, kind.own_slots()) {
                event.drops.push(ItemDrop::new(stack, at, [0.0; 3], 10));
            }
        }
    }
    Flow::Continue
}

/// The three machines' item Part: a placed machine takes back the slots its
/// item carried (upstream's `BlockEntityWithDrops` in reverse —
/// `applyImplicitComponents`), because the engine applies
/// `minecraft:block_entity_data` only through `canonical()`, which knows
/// vanilla's types alone.
struct CarriedContents;

#[mod_api::horizon_part]
impl ItemPart for CarriedContents {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        let Some(kind) = keys().kind_of_item(event.stack.item()) else {
            return;
        };
        let Some(carried) = contents_of(event.stack) else {
            return;
        };
        let decoded: Vec<(u32, ItemStack)> = carried
            .iter()
            .filter_map(|(slot, entry)| Some((*slot, ItemStack::from_nbt(&*host, entry)?)))
            .collect();
        let stored = runtime::restore(&decoded, kind.own_slots());
        if stored.is_empty() {
            return;
        }
        let Some(level) = event
            .placer
            .and_then(|player| player.level(&*host))
            .or_else(|| host.level(host.dimension()))
        else {
            return;
        };
        let pos = event.pos;
        let Some(entity) = host.block_entity(level, pos) else {
            return;
        };
        let wrote = host
            .transaction(|host, tx| {
                for (slot, stack) in &stored {
                    entity.set_item_slot(host, tx, *slot, Some(stack.clone()))?;
                }
                Ok::<(), BlockEntityItemError>(())
            })
            .is_ok();
        if !wrote {
            return;
        }
        // The payload is the *placed unit's*, and `event.stack` is the stack
        // before the engine takes that unit off (`onBlockPlacedBy` runs before
        // stack consumption). Survival therefore needs no write at all: the
        // engine takes the one unit and every unit it did not place keeps its
        // own payload. Creative keeps the stack, so a payload that has now been
        // delivered comes off — but only when the placed unit was the whole
        // stack: with two units in hand the second is a machine of its own, and
        // stripping it would destroy its contents.
        let creative =
            event.placer.and_then(|player| player.game_mode(&*host)) == Some(GameMode::Creative);
        if runtime::placed_hand(creative, event.stack.count()) == runtime::PlacedHand::StripPlaced {
            strip_contents(host, event.placer, event.stack);
        }
    }
}

/// The slots a placed item carries: each entry of `RefinedStorageContents`, with
/// the slot it names. A payload naming another type, or no payload at all, is
/// nothing.
fn contents_of(stack: &ItemStack) -> Option<Vec<(u32, Compound)>> {
    let patch = stack.components()?.as_nbt();
    let Some(Tag::Compound(data)) = field(patch, BLOCK_ENTITY_DATA) else {
        return None;
    };
    match field(data, "id") {
        Some(Tag::String(id)) if id.to_string_lossy() == BE_TYPE => {}
        _ => return None,
    }
    let Some(Tag::List(entries)) = field(data, CONTENTS_KEY) else {
        return None;
    };
    Some(
        entries
            .iter()
            .filter_map(|entry| {
                let Tag::Compound(entry) = entry else {
                    return None;
                };
                let Some(Tag::Byte(slot)) = field(entry, "Slot") else {
                    return None;
                };
                Some((u32::from(u8::try_from(*slot).ok()?), entry.clone()))
            })
            .collect(),
    )
}

/// Take the contents off the stack that was placed, so a creative placement
/// cannot restore them again. The whole `minecraft:block_entity_data` goes when
/// this port's list is all it held.
///
/// The hand is **found**, never assumed: `on_block_placed_by` does not name it,
/// and a write into the main hand for a placement from the off hand would
/// destroy that hand's item and duplicate this one. Only called for a placement
/// whose payload is spent (`runtime::PlacedHand::StripPlaced`).
fn strip_contents(host: &mut dyn ServerHost, placer: Option<PlayerRef>, stack: &ItemStack) {
    let Some(player) = placer else {
        return;
    };
    let Some(slot) = hand_holding(host, &player, stack) else {
        return;
    };
    let Some(patch) = stack.components() else {
        return;
    };
    let mut patch = patch.as_nbt().clone();
    let Some(Tag::Compound(data)) = field(&patch, BLOCK_ENTITY_DATA) else {
        return;
    };
    let kept = without_key(data, CONTENTS_KEY);
    if kept.iter().all(|(key, _)| key.to_string_lossy() == "id") {
        // This port's list was all the component held: the whole component goes,
        // and an empty patch is the right answer (it clears the stack's
        // components) — not a reason to leave the payload on it.
        patch = without_key(&patch, BLOCK_ENTITY_DATA);
    } else {
        patch.insert(BLOCK_ENTITY_DATA.into(), Tag::Compound(kept));
    }
    let mut stripped = stack.clone();
    if stripped.set_components(patch).is_err() || stripped == *stack {
        return;
    }
    let _ =
        host.transaction(|host, _tx| player.set_inventory_slot(host, slot, Some(stripped.clone())));
}

/// The inventory slot of the hand holding `stack`: the hotbar slot the player
/// holds, else the off-hand slot (40). `None` when neither hand holds it — a
/// dispenser's placement, or a copy that no longer matches.
fn hand_holding(host: &mut dyn ServerHost, player: &PlayerRef, stack: &ItemStack) -> Option<u32> {
    Hand::BOTH.into_iter().find_map(|hand| {
        let slot = held_slot(&*host, player, hand)?;
        let here = ItemPlace::PlayerHand {
            player: player.uuid(),
            hand,
        }
        .stack(host)?;
        (here == *stack).then_some(slot)
    })
}

/// A compound without `name`, in the order it had.
fn without_key(data: &Compound, name: &str) -> Compound {
    data.iter()
        .filter(|(key, _)| key.to_string_lossy() != name)
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

// --- the port's cross-hook state ------------------------------------------

/// Everything in the cache is derived from loaded blocks and their block
/// entities, so a new world drops it.
#[derive(Default)]
struct Core {
    /// Each loaded drive slot that holds a storage disk.
    disks: HashMap<DiskKey, Disk<Net>>,
    /// Disk caches a grid changed and their drive has not written back yet.
    dirty: HashSet<DiskKey>,
    /// Each grid's transient search, by dimension and position (upstream's
    /// search box is the client's; a mod's screen has no text field yet, so
    /// `/refinedstorage filter` sets it).
    filters: HashMap<(String, [i32; 3]), String>,
    /// The connection bits each cable's state was last written with.
    connections: HashMap<(String, [i32; 3]), u8>,
    /// The `active` state each grid was last written with.
    active: HashMap<(String, [i32; 3]), bool>,
    /// The server ticks each machine has seen, so its work falls every
    /// `work_interval` of them (upstream's throttled ticker).
    work: HashMap<(String, [i32; 3]), u64>,
    /// What each open view currently shows: the network stock it has already
    /// promised its players (`view_with`). Two views of one network never show
    /// the same items, so neither can hand them out twice, whether they are a
    /// grid block or a wireless grid's menu; two networks keep separate stock.
    promises: Promises,
    /// The wireless grids whose menu is open: the player and window id, the
    /// transmitter it is bound to and what it last showed.
    menus: HashMap<(u128, i32), OpenGrid>,
    /// The energy level each controller's `energy_type` state was last written
    /// with.
    energy: HashMap<(String, [i32; 3]), EnergyType>,
    /// The server ticks this world has seen, counted on `server_tick`: the tick
    /// counter the energy readout prints, so a fall over N ticks can be checked
    /// against N ticks rather than against a wall clock.
    ticks: u64,
    /// The slots a machine broken this tick held, waiting for that break's own
    /// `block_drops` (slice 10). One entry: two player breaks cannot interleave
    /// inside one `destroyBlock`.
    carried: Option<Carried>,
}

/// What a broken machine's drop carries: where it broke, which tick, and the
/// slots it held — as the item NBT `block_drops` puts on the drop, and as the
/// stacks its fallback spawns (that hook has no game host to convert one into
/// the other).
struct Carried {
    dimension: String,
    pos: [i32; 3],
    tick: u64,
    slots: Vec<(u32, Compound)>,
    stacks: Vec<(u32, ItemStack)>,
}

static CORE: Mutex<Option<Core>> = Mutex::new(None);

fn with_core<R>(f: impl FnOnce(&mut Core) -> R) -> R {
    let mut guard = CORE
        .lock()
        .expect("the disk cache lock is held for one call");
    f(guard.get_or_insert_with(Core::default))
}

/// A world opened: start its cache empty.
pub(crate) fn reset() {
    let mut guard = CORE
        .lock()
        .expect("the disk cache lock is held for one call");
    *guard = Some(Core::default());
}

/// A world closed: drop everything derived from it.
pub(crate) fn on_world_lifecycle(phase: WorldPhase) {
    if matches!(
        phase,
        WorldPhase::WorldOpenFailed | WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed
    ) {
        let mut guard = CORE
            .lock()
            .expect("the disk cache lock is held for one call");
        *guard = None;
    }
}

// --- player interaction ----------------------------------------------------

/// A storage disk right-clicked onto a drive goes into its first free slot;
/// every other click is left to the block (the menu opens).
pub(crate) fn on_right_click_block(
    host: HostSide<'_>,
    event: &mut RightClickBlockEvent<'_>,
) -> Flow {
    let HostSide::Server(host) = host else {
        return Flow::Continue;
    };
    let pos = event.click.pos;
    // A wireless grid right-clicked onto a transmitter binds it. The block's
    // own use would open the transmitter's upgrade menu first (the game runs
    // the block's `use_item_on` before the item's `use_on`), so the binding is
    // the click's, as the disk onto a drive is.
    if kind_at(host, pos) == Some(Kind::WirelessTransmitter)
        && let Some(held) = event.click.stack
        && held.item() == keys().wireless_grid
    {
        bind_grid(host, event.click.player, event.click.hand, pos, held);
        return Flow::Cancel;
    }
    if kind_at(host, pos) != Some(Kind::DiskDrive) {
        return Flow::Continue;
    }
    let Some(held) = event.click.stack else {
        return Flow::Continue;
    };
    if !is_disk(held) {
        return Flow::Continue;
    }
    let mut one = held.clone();
    let _ = one.set_count(1);
    match insert_disk(host, pos, one, event.click.player, event.click.hand, held) {
        DiskInsertion::Inserted => tell(host, event.click.player, "Storage disk inserted."),
        DiskInsertion::Full => tell(host, event.click.player, "The disk drive is full."),
        DiskInsertion::Refused => tell(
            host,
            event.click.player,
            "The disk did not go in: the drive and the hand could not be written together.",
        ),
    }
    // Either way the click is the drive's: the menu must not open behind a
    // disk that was placed.
    Flow::Cancel
}

/// What a disk click did; the insertion either commits both writes or neither.
enum DiskInsertion {
    Inserted,
    /// Every drive slot held a disk.
    Full,
    /// A write was refused (a stale drive, a dead player, a slot the hand no
    /// longer has): the transaction aborted, so neither side moved.
    Refused,
}

/// Why a disk insertion wrote nothing. The transaction rolls back on either,
/// which is the point: a disk in the drive with nothing taken from the hand
/// duplicates the disk, and a decremented hand with no disk in the drive
/// destroys one.
#[derive(Debug)]
enum DiskRefusal {
    /// Every drive slot held a disk.
    NoSlot,
    /// A write was refused (a stale drive, a dead player, a hand that no longer
    /// has the slot): the transaction aborted, so neither side moved.
    WriteRefused,
}

/// Put `one` in the drive's first free slot **and** take it from the hand that
/// clicked, in one transaction (slice 13): both writes commit or neither does.
/// The hand is the click's own — a disk in the off hand is taken from the off
/// hand, not from whatever the main hand happens to hold.
fn insert_disk(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    one: ItemStack,
    player: u128,
    hand: Hand,
    held: &ItemStack,
) -> DiskInsertion {
    let Some(level) = host.level(host.dimension()) else {
        return DiskInsertion::Refused;
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return DiskInsertion::Refused;
    };
    let Some(actor) = host.player(player) else {
        return DiskInsertion::Refused;
    };
    let Some(hand_slot) = held_slot(&*host, &actor, hand) else {
        return DiskInsertion::Refused;
    };
    // The stack the hand keeps: one disk fewer, or an empty hand.
    let left = match runtime::after_taking_one(held.count()) {
        Some(count) => {
            let mut rest = held.clone();
            let _ = rest.set_count(count);
            Some(rest)
        }
        // Nothing held: nothing to take, so nothing is inserted either.
        None if held.count() == 0 => return DiskInsertion::Refused,
        None => None,
    };
    let wrote = host.transaction(|host, tx| {
        let slot = (0..DRIVE_SLOTS)
            .find(|slot| matches!(entity.item_slot(&*host, *slot as u32), Ok(None)))
            .ok_or(DiskRefusal::NoSlot)?;
        entity
            .set_item_slot(host, tx, slot as u32, Some(one.clone()))
            .map_err(|_| DiskRefusal::WriteRefused)?;
        actor
            .set_inventory_slot(host, hand_slot, left.clone())
            .map_err(|_| DiskRefusal::WriteRefused)?;
        Ok(())
    });
    match wrote {
        Ok(()) => DiskInsertion::Inserted,
        Err(DiskRefusal::NoSlot) => DiskInsertion::Full,
        Err(_) => DiskInsertion::Refused,
    }
}

fn tell(host: &mut dyn ServerHost, uuid: u128, line: &str) {
    if let Some(player) = host.player(uuid) {
        let _ = player.tell(host, Text::literal(line));
    }
}

// --- commands --------------------------------------------------------------

/// `/refinedstorage info|filter|wireless` (`register_commands`).
pub(crate) fn on_register_commands(event: &mut RegisterCommandsEvent<'_>) {
    event
        .commands
        .push(Command::new("refinedstorage info <pos:block_pos>"));
    event.commands.push(Command::new(
        "refinedstorage filter <pos:block_pos> [text:text]",
    ));
    // The use a wireless grid will make: does this player, here, reach the
    // network this transmitter exposes? (Upstream asks the same question through
    // `NetworkItemPlayerValidator`; with no wireless grid in the package yet,
    // this is how the rule is exercised.)
    event.commands.push(Command::new(
        "refinedstorage wireless <pos:block_pos> <dimension:dimension>",
    ));
    // The binding a wireless grid carries, read back from the item component
    // (upstream shows it in the item's tooltip; a mod item's tooltip is a
    // client hook this port does not subscribe, so the readout is a command,
    // like `/refinedstorage info`).
    event.commands.push(Command::new("refinedstorage bound"));
    // The network's buffer, filled through the capability an FE cable uses.
    // Upstream's controller is filled by its neighbours; the harness has no FE
    // generator, so this command is the source, as `/refinedstorage info` is
    // the screen (a recorded harness/behaviour addition). It writes a buffer,
    // so it is an operator command (slice 13): the level is declared here and
    // re-checked in the body (`runtime::may_write_energy`).
    event.commands.push(Command::op(
        "refinedstorage energy <pos:block_pos> <amount:integer>",
        runtime::ENERGY_LEVEL,
    ));
    // The wireless grid's own buffer, read back from the item's component, and
    // filled through the item's energy capability the way an FE charger would
    // (upstream's grid item is an `ItemEnergyStorage`; the harness has no
    // charger at this pin). Two recorded harness/behaviour additions, twins of
    // `/refinedstorage energy` for the network's buffer — and the same operator
    // gate, because it writes the item's buffer.
    event.commands.push(Command::new("refinedstorage grid"));
    event.commands.push(Command::op(
        "refinedstorage charge <amount:integer>",
        runtime::ENERGY_LEVEL,
    ));
}

/// The `/refinedstorage` commands' bodies (`run_mod_command`).
pub(crate) fn on_run_mod_command(
    host: &mut dyn ServerHost,
    event: &mut RunModCommandEvent<'_>,
) -> Flow {
    let [root, sub] = event.command.literals.as_slice() else {
        return Flow::Continue;
    };
    if root != "refinedstorage" {
        return Flow::Continue;
    }
    if sub == "bound" {
        bound(host, event.source);
        return Flow::Cancel;
    }
    if sub == "grid" {
        grid_buffer(host, event.source);
        return Flow::Cancel;
    }
    if sub == "charge" {
        // Both energy writes are operator actions: declared on the command
        // (`Command::op`) and asked again here, so neither the gate nor its
        // refusal rests on the dispatcher alone.
        if !runtime::may_write_energy(event.source.permission().level()) {
            feedback(
                host,
                "Only an operator may charge a wireless grid (permission level 2).",
            );
            return Flow::Cancel;
        }
        let Some(CommandArgumentValue::Integer(amount)) = event.command.argument("amount") else {
            feedback(host, "Usage: /refinedstorage charge <amount>");
            return Flow::Cancel;
        };
        charge_grid(host, event.source, u32::try_from(*amount).unwrap_or(0));
        return Flow::Cancel;
    }
    if sub == "energy" {
        if !runtime::may_write_energy(event.source.permission().level()) {
            feedback(
                host,
                "Only an operator may charge a network (permission level 2).",
            );
            return Flow::Cancel;
        }
        let Some(CommandArgumentValue::BlockPos(pos)) = event.command.argument("pos") else {
            feedback(host, "Usage: /refinedstorage energy <x> <y> <z> <amount>");
            return Flow::Cancel;
        };
        let Some(CommandArgumentValue::Integer(amount)) = event.command.argument("amount") else {
            feedback(host, "Usage: /refinedstorage energy <x> <y> <z> <amount>");
            return Flow::Cancel;
        };
        charge(host, *pos, u32::try_from(*amount).unwrap_or(0));
        return Flow::Cancel;
    }
    let Some(CommandArgumentValue::BlockPos(pos)) = event.command.argument("pos") else {
        feedback(
            host,
            "Usage: /refinedstorage <info|filter|wireless> <x> <y> <z> [text|dimension]",
        );
        return Flow::Cancel;
    };
    let pos = *pos;
    match sub.as_str() {
        "info" => {
            info(host, pos);
            Flow::Cancel
        }
        "filter" => {
            let filter = match event.command.argument("text") {
                Some(CommandArgumentValue::String(text)) => text.clone(),
                _ => String::new(),
            };
            filter_at(host, pos, &filter);
            Flow::Cancel
        }
        "wireless" => {
            let Some(CommandArgumentValue::Dimension(level)) =
                event.command.argument("dimension").cloned()
            else {
                feedback(
                    host,
                    "Usage: /refinedstorage wireless <x> <y> <z> <dimension>",
                );
                return Flow::Cancel;
            };
            wireless(host, event.source, event.command, pos, level);
            Flow::Cancel
        }
        _ => Flow::Continue,
    }
}

fn feedback(host: &mut dyn ServerHost, line: &str) {
    let _ = host.commands().feedback(Text::literal(line));
}

/// The body of `/refinedstorage bound`: the transmitter the acting player's
/// wireless grid is bound to, read back from the item's own component.
fn bound(host: &mut dyn ServerHost, source: &CommandSource) {
    let player = match source {
        CommandSource::Player { player, .. } => player,
        _ => {
            feedback(host, "Only a player holds a wireless grid.");
            return;
        }
    };
    let held = ItemPlace::PlayerHand {
        player: player.uuid(),
        hand: Hand::Main,
    };
    let Some(stack) = held.stack(host) else {
        feedback(host, "No item in the main hand.");
        return;
    };
    if stack.item() != keys().wireless_grid {
        feedback(host, "The item in the main hand is not a wireless grid.");
        return;
    }
    match binding_of(&stack) {
        Some(binding) => feedback(
            host,
            &format!("Wireless grid bound to {}.", binding_name(&binding)),
        ),
        None => feedback(host, "The wireless grid is not bound."),
    }
}

/// The body of `/refinedstorage grid`: the held wireless grid's own buffer,
/// read from the item's `refinedstorage:energy` component through the same
/// capability a charge goes through, with the server tick the reading was taken
/// at (so a fall can be checked against ticks, not a wall clock).
fn grid_buffer(host: &mut dyn ServerHost, source: &CommandSource) {
    let Some(uuid) = player_of(host, source, "Only a player holds a wireless grid.") else {
        return;
    };
    let place = ItemPlace::PlayerHand {
        player: uuid,
        hand: Hand::Main,
    };
    let Some(stack) = place.stack(host) else {
        feedback(host, "No item in the main hand.");
        return;
    };
    if stack.item() != keys().wireless_grid {
        feedback(host, "The item in the main hand is not a wireless grid.");
        return;
    }
    let Some((stored, capacity)) = grid_energy(host, uuid, Hand::Main) else {
        feedback(host, "The wireless grid has no energy buffer.");
        return;
    };
    let ticks = with_core(|core| core.ticks);
    feedback(
        host,
        &format!("Wireless grid energy: {stored}/{capacity} FE, t={ticks}"),
    );
}

/// The body of `/refinedstorage charge <amount>`: bring the held wireless
/// grid's own buffer to `amount` FE through the item energy capability — an
/// insert the way a charger fills it, an extract the way a use drains it.
///
/// The held item must **be** the wireless grid before anything is written
/// (slice 13): a charge asked of another item answers so instead of filling
/// whatever buffer another mod's item happens to answer with.
fn charge_grid(host: &mut dyn ServerHost, source: &CommandSource, amount: u32) {
    let Some(uuid) = player_of(host, source, "Only a player holds a wireless grid.") else {
        return;
    };
    let place = ItemPlace::PlayerHand {
        player: uuid,
        hand: Hand::Main,
    };
    let Some(stack) = place.stack(host) else {
        feedback(host, "No item in the main hand.");
        return;
    };
    if stack.item() != keys().wireless_grid {
        feedback(host, "The item in the main hand is not a wireless grid.");
        return;
    }
    let Some(handler) = place.capability(host, &ITEM_ENERGY) else {
        feedback(host, "The item in the main hand is not a wireless grid.");
        return;
    };
    let before = handler.amount(host);
    let capacity = handler.capacity(host);
    let target = amount.min(capacity);
    let insert = target.saturating_sub(before);
    let extract = before.saturating_sub(target);
    let _ = host
        .transaction(|host, tx| {
            let put = if insert > 0 {
                handler.insert(host, tx, insert)
            } else {
                0
            };
            let got = if extract > 0 {
                handler.extract(host, tx, extract)
            } else {
                0
            };
            Ok::<(u32, u32), mod_api::PlayerDataRefusal>((put, got))
        })
        .unwrap_or((0, 0));
    let after = handler.amount(host);
    feedback(
        host,
        &format!("Wireless grid energy: {after}/{capacity} FE (was {before})"),
    );
}

/// The acting player a command's body needs, or a feedback line saying why it
/// has none.
fn player_of(host: &mut dyn ServerHost, source: &CommandSource, why_not: &str) -> Option<u128> {
    match source {
        CommandSource::Player { player, .. } => Some(player.uuid()),
        _ => {
            feedback(host, why_not);
            None
        }
    }
}

fn info(host: &mut dyn ServerHost, pos: [i32; 3]) {
    if kind_at(host, pos).is_none() {
        feedback(host, "No Refined Storage network block there.");
        return;
    }
    let net = network_at(host, pos);
    let dimension = host.dimension().to_owned();
    let level = host.level(host.dimension());
    let drives: Vec<[i32; 3]> = net.of_kind(Kind::DiskDrive).collect();
    let (disks, items, resources) = {
        let keys: Vec<DiskKey> = drive_keys(&dimension, &drives);
        let mut all: Vec<Disk<Net>> = with_core(|core| {
            keys.iter()
                .filter_map(|key| core.disks.get(key).cloned())
                .collect()
        });
        let disks = all.len();
        // An external storage is network storage too: its faced container's
        // stock is counted in the totals, while `disks` stays the drive's own
        // census number.
        if let Some(level) = level {
            all.extend(external_disks(&live_external(host, level, &net)));
        }
        let items: u64 = all.iter().map(Disk::total).sum();
        let resources = runtime::aggregate(&all).len();
        (disks, items, resources)
    };
    let online = net.online();
    // The buffer and what the network charges: the control readout a machine
    // menu would show as a bar, and the number the energy assertions compare.
    // `drawing` is what the always-on nodes take per tick (grid 10, drive 10 +
    // 4 a disk, transmitter 16 + 8 a range upgrade); `per work pass` is what the
    // two machines take once every `work_interval` ticks.
    let mut per_tick = 0;
    let mut per_pass = 0;
    if let Some(level) = level {
        for &(kind, at) in &net.nodes {
            let charge = charge_of(host, level, kind, at);
            match kind {
                Kind::Importer | Kind::Exporter => per_pass += charge,
                _ => per_tick += charge,
            }
        }
    }
    let stored = level.map_or(0, |level| stored_in(host, level, &net));
    let capacity = capacity_in(&net);
    let ticks = with_core(|core| core.ticks);
    let mut census = format!(
        "{} nodes, {} controller, {} drive, {} grid",
        net.nodes.len(),
        net.count(Kind::Controller),
        drives.len(),
        net.count(Kind::Grid),
    );
    for (kind, name) in [
        (Kind::WirelessTransmitter, "transmitter"),
        (Kind::Importer, "importer"),
        (Kind::Exporter, "exporter"),
        (Kind::ExternalStorage, "external storage"),
        (Kind::Interface, "interface"),
    ] {
        let count = net.count(kind);
        if count > 0 {
            census.push_str(&format!(", {count} {name}"));
        }
    }
    feedback(
        host,
        &format!(
            "Refined Storage network at {},{},{}: {state} ({census}); {} disks, {items} items in {resources} resources; energy {stored}/{capacity} FE, drawing {per_tick}/tick{pass}, t={ticks}",
            pos[0],
            pos[1],
            pos[2],
            disks,
            state = if online { "online" } else { "offline" },
            pass = if per_pass > 0 {
                format!(" + {per_pass} per work pass")
            } else {
                String::new()
            },
        ),
    );
    if kind_at(host, pos) == Some(Kind::WirelessTransmitter) {
        let range = host
            .level(host.dimension())
            .map_or(runtime::WIRELESS_RANGE, |level| range_at(host, level, pos));
        feedback(
            host,
            &format!(
                "Wireless transmitter at {},{},{}: range {}, {}",
                pos[0],
                pos[1],
                pos[2],
                runtime::range_label(range),
                if online { "active" } else { "inactive" },
            ),
        );
    }
    if kind_at(host, pos) == Some(Kind::DiskDrive) {
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Some(entity) = host.block_entity(level, pos) else {
            return;
        };
        let mut found: Vec<String> = Vec::new();
        for slot in 0..DRIVE_SLOTS as u32 {
            let Some(item) = entity.item_slot(host, slot).ok().flatten() else {
                continue;
            };
            if !is_disk(&item) {
                continue;
            }
            let disk = read_disk(host, &item);
            found.push(format!(
                "slot {slot}: {} {}/{}",
                item.item().id(host),
                disk.total(),
                disk.capacity().label()
            ));
        }
        feedback(
            host,
            &format!(
                "Disk drive at {},{},{}: {} disk{}, {}{}",
                pos[0],
                pos[1],
                pos[2],
                found.len(),
                if found.len() == 1 { "" } else { "s" },
                if found.is_empty() {
                    "empty"
                } else {
                    "capacity "
                },
                found.join(", "),
            ),
        );
    }
    if kind_at(host, pos) == Some(Kind::ExternalStorage) {
        let Some(level) = level else {
            return;
        };
        let facing = facing_in(host, level, pos)
            .map_or_else(|| "none".to_owned(), |face| face.property().to_owned());
        let resources = live_external(host, level, &net)
            .first()
            .map_or(0, |storage| storage.disk.entries().len());
        feedback(
            host,
            &format!(
                "External storage at {},{},{}: facing {}, {} resources",
                pos[0], pos[1], pos[2], facing, resources
            ),
        );
    }
    if kind_at(host, pos) == Some(Kind::Interface) {
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Some(entity) = host.block_entity(level, pos) else {
            return;
        };
        let filters: Vec<String> = (0..INTERFACE_FILTER_SLOTS as u32)
            .filter_map(|slot| entity.item_slot(host, slot).ok().flatten())
            .map(|stack| stack.item().id(host).to_owned())
            .collect();
        let exported: Vec<String> = (INTERFACE_FILTER_SLOTS as u32..INTERFACE_SLOTS as u32)
            .filter_map(|slot| entity.item_slot(host, slot).ok().flatten())
            .map(|stack| format!("{}:{}", stack.item().id(host), stack.count()))
            .collect();
        feedback(
            host,
            &format!(
                "Interface at {},{},{}: {} filter entries, {} exported [{}]",
                pos[0],
                pos[1],
                pos[2],
                filters.len(),
                exported.len(),
                exported.join(", ")
            ),
        );
    }
    if let Some(Kind::Importer | Kind::Exporter) = kind_at(host, pos) {
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Some(entity) = host.block_entity(level, pos) else {
            return;
        };
        let items: Vec<Option<ItemStack>> = (0..MACHINE_SLOTS as u32)
            .map(|slot| entity.item_slot(host, slot).ok().flatten())
            .collect();
        let (speed, stacks, filter) = machine_slots(host, &items);
        let name = kind_at(host, pos)
            .and_then(Kind::machine_name)
            .unwrap_or("Machine");
        feedback(
            host,
            &runtime::machine_readout(name, pos, speed, stacks, filter.len()),
        );
    }
}

/// The body of `/refinedstorage wireless <pos> <dimension>`: the admission a
/// wireless grid asks of a transmitter before it answers the player's use —
/// the same dimension, an online network, and within the transmitter's range
/// (`NetworkItemPlayerValidator.isValid`).
fn wireless(
    host: &mut dyn ServerHost,
    source: &CommandSource,
    command: &CommandInvocation,
    pos: [i32; 3],
    level: LevelRef,
) {
    if node_at(host, level, pos).map(|node| node.kind) != Some(Kind::WirelessTransmitter) {
        feedback(host, "No wireless transmitter there.");
        return;
    }
    let net = network_in(host, level, pos);
    let online = net.online();
    // The transmitter's own charge (16 FE plus 8 a range upgrade) against what
    // the network holds: upstream's `node.isActive()` step, before the range.
    let fit = fit_at(host, level, pos, TRANSMITTER_SLOTS);
    let charge = runtime::usage(Kind::WirelessTransmitter, fit, 0);
    let energy = runtime::Energy::new(stored_in(host, level, &net), charge);
    // Upstream hands the validator the *player's* coordinates; the command's
    // own dimension is the dimension the use happens in (`execute in` moves it),
    // and a player's real position is used for the distance so a rewritten
    // execution position cannot fake a range pass.
    let from = match source {
        CommandSource::Player { player, .. } => player.entity().position(host),
        _ => Some(command.position),
    };
    let Some(from) = from else {
        feedback(host, "No position for this use.");
        return;
    };
    let distance = runtime::distance(from, pos);
    let range = range_at(host, level, pos);
    match runtime::admit(
        online,
        command.dimension == level.dimension(),
        energy,
        distance,
        range,
    ) {
        Ok(()) => feedback(
            host,
            &format!(
                "Wireless access at {},{},{}: accepted ({distance:.1} of {}, {} network)",
                pos[0],
                pos[1],
                pos[2],
                runtime::range_label(range),
                if online { "online" } else { "offline" },
            ),
        ),
        Err(refusal) => feedback(
            host,
            &format!(
                "Wireless access at {},{},{}: refused ({refusal})",
                pos[0], pos[1], pos[2]
            ),
        ),
    }
}

fn filter_at(host: &mut dyn ServerHost, pos: [i32; 3], filter: &str) {
    if kind_at(host, pos) != Some(Kind::Grid) {
        feedback(host, "No grid there.");
        return;
    }
    let dimension = host.dimension().to_owned();
    with_core(|core| {
        core.filters
            .retain(|(dim, at), _| !(dim == &dimension && *at == pos));
        if !filter.is_empty() {
            core.filters.insert((dimension, pos), filter.to_owned());
        }
    });
    if filter.is_empty() {
        feedback(host, "Grid search cleared.");
    } else {
        feedback(host, &format!("Grid search: {filter}"));
    }
}

/// `/refinedstorage energy <pos> <amount>`: bring the network's buffer to
/// `amount` FE through the controller's declared energy capability — an insert
/// the way an FE cable fills it, an extract the way a consumer takes it. The
/// harness has no generator at this pin, so the command is the source; it
/// writes only what the game's own handler would.
fn charge(host: &mut dyn ServerHost, pos: [i32; 3], amount: u32) {
    let net = network_at(host, pos);
    if net.count(Kind::Controller) == 0 {
        feedback(host, "No controller on the network: nothing to charge.");
        return;
    }
    let Some(level) = host.level(host.dimension()) else {
        return;
    };
    let before = stored_in(host, level, &net);
    let capacity = capacity_in(&net);
    let target = amount.min(capacity);
    let mut insert = target.saturating_sub(before);
    let mut extract = before.saturating_sub(target);
    for at in net.of_kind(Kind::Controller) {
        if insert == 0 && extract == 0 {
            break;
        }
        let Some(handler) = level.capability(host, &ENERGY, at, None) else {
            continue;
        };
        let (put, got) = host
            .transaction(|host, tx| {
                let put = if insert > 0 {
                    handler.insert(host, tx, insert)
                } else {
                    0
                };
                let got = if extract > 0 {
                    handler.extract(host, tx, extract)
                } else {
                    0
                };
                Ok::<(u32, u32), mod_api::PlayerDataRefusal>((put, got))
            })
            .unwrap_or((0, 0));
        insert = insert.saturating_sub(put);
        extract = extract.saturating_sub(got);
    }
    let after = stored_in(host, level, &net);
    feedback(
        host,
        &format!(
            "Network energy at {},{},{}: {after}/{capacity} FE (was {before})",
            pos[0], pos[1], pos[2]
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::field;
    use mod_api::{Compound, Tag};

    /// The saved grid view is read back through [`field`], the lookup that
    /// replaced the SDK's `JavaString` (and with it the second path dependency
    /// the launcher refused): a mark written with `KEY.into()` is found again,
    /// and its own keys are found in the nested compound.
    #[test]
    fn a_saved_view_mark_is_found_by_its_java_key() {
        let mut mark = Compound::new();
        mark.insert("owned".into(), Tag::Byte(1));
        mark.insert("item".into(), Tag::Compound(Compound::new()));
        let mut nbt = Compound::new();
        nbt.insert(
            "RefinedStorageView".into(),
            Tag::List(vec![Tag::Compound(mark)]),
        );

        let Some(Tag::List(marks)) = field(&nbt, "RefinedStorageView") else {
            panic!("the saved view was not found: {nbt:?}");
        };
        let Tag::Compound(mark) = &marks[0] else {
            panic!("the mark is not a compound: {marks:?}");
        };
        assert_eq!(field(mark, "owned"), Some(&Tag::Byte(1)), "the owned mark");
        assert!(
            matches!(field(mark, "item"), Some(Tag::Compound(item)) if item.is_empty()),
            "the item mark: {:?}",
            field(mark, "item")
        );
        assert_eq!(field(mark, "Owned"), None, "the lookup is case-sensitive");
        assert_eq!(field(&nbt, "StorageView"), None, "it is a whole-key match");
    }
}
