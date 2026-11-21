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
    self, BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockKey,
    BlockPart, BlockState, Command, CommandArgumentValue, CommandInvocation, CommandSource,
    Component, Compound, Flow, Host, HostSide, HostTransaction, ItemKey, ItemStack, LevelHost,
    LevelRef, LevelSide, ModBlockEntityView, ModContainer, RegisterCommandsEvent, Registrar,
    RegistrationContext, RightClickBlockEvent, RunModCommandEvent, Screen, ScreenEvent, ServerHost,
    StateForPlacementEvent, Tag, Text, WorldPhase,
};

use crate::runtime::{
    self, CAPACITY_1K, DRIVE_SLOTS, Delta, Disk, Face, GRID_SLOTS, Kind, Node, Stored,
    WIRELESS_RANGE, extract, insert, network, view_with,
};

/// The five content ids of the slice, exactly as the content declares them.
const CONTROLLER: &str = "refinedstorage:controller";
const CABLE: &str = "refinedstorage:cable";
const DISK_DRIVE: &str = "refinedstorage:disk_drive";
const GRID: &str = "refinedstorage:grid";
const DISK: &str = "refinedstorage:1k_storage_disk";
/// The wireless transmitter (slice 3): a network node that joins through its
/// facing side and answers whether a player may use its network.
const TRANSMITTER: &str = "refinedstorage:wireless_transmitter";
/// The block-entity type this mod declares for the four network blocks.
const BE_TYPE: &str = "refinedstorage:network";
/// The item component that carries a disk's contents.
const COMPONENT: &str = "refinedstorage:storage";

/// The drive's menu shows one row of nine (upstream has eight; the game's
/// menu types only show 9/18/27/45/54 slots, and a menu is how a player moves
/// disks in and out).
const DRIVE_MENU: &str = "minecraft:generic_9x1";
const GRID_MENU: &str = "minecraft:generic_9x6";

/// The saved compound key the grid's view marks live under.
const VIEW_KEY: &str = "RefinedStorageView";

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
    disk: ItemKey,
}

impl Keys {
    /// The network kind of a loaded block, none when it is not one of the
    /// five.
    fn kind_of(self, block: BlockKey) -> Option<Kind> {
        [
            (self.controller, Kind::Controller),
            (self.cable, Kind::Cable),
            (self.disk_drive, Kind::DiskDrive),
            (self.grid, Kind::Grid),
            (self.transmitter, Kind::WirelessTransmitter),
        ]
        .into_iter()
        .find_map(|(key, kind)| (key == block).then_some(kind))
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
        disk: registrar.item_key(DISK),
    };
    let _ = KEYS.set(keys);
    let kind = registrar
        .block_entity_type(BE_TYPE)
        .blocks(&[CONTROLLER, CABLE, DISK_DRIVE, GRID, TRANSMITTER])
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
}

/// The disk's item component (`component_types`, before content loads).
pub(crate) fn register_component_types(event: &mut mod_api::ComponentTypesEvent<'_>) {
    event.types.push(COMPONENT.to_owned());
}

// --- one item as the network sees it --------------------------------------

/// One item on the network: the stack plus its canonical resource key,
/// resolved once where a host is at hand. Two stacks are one resource when
/// their item id, damage and components all agree.
#[derive(Clone, Debug)]
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

/// The disk items in a drive's slots.
fn is_disk(stack: &ItemStack) -> bool {
    stack.item() == keys().disk
}

// --- the disk's item component --------------------------------------------

/// The component that carries a 1k disk's contents: the saved stacks, one
/// compound each, in the stack's own NBT escape.
struct StorageComponent;

impl Component for StorageComponent {
    type Value = Vec<Compound>;

    fn id(&self) -> &str {
        COMPONENT
    }

    fn decode(&self, value: &Tag) -> Option<Self::Value> {
        let Tag::List(entries) = value else {
            return None;
        };
        let mut stored = Vec::with_capacity(entries.len());
        for entry in entries {
            if let Tag::Compound(compound) = entry {
                stored.push(compound.clone());
            }
        }
        Some(stored)
    }

    fn encode(&self, value: Self::Value) -> Tag {
        Tag::List(value.into_iter().map(Tag::Compound).collect())
    }
}

/// A disk item's contents from its component; an empty disk when it has none
/// (a freshly made or creative-given disk).
fn read_disk(host: &dyn Host, stack: &ItemStack) -> Disk<Net> {
    let mut disk = Disk::new(CAPACITY_1K);
    let Some(entries) = stack.get(&StorageComponent) else {
        return disk;
    };
    let stored: Vec<Net> = entries
        .iter()
        .filter_map(|entry| ItemStack::from_nbt(host, entry))
        .map(|stack| Net::of(host, &stack))
        .collect();
    disk.set_entries(stored);
    disk
}

/// Write a disk's contents into its item component (an empty disk loses the
/// component rather than keeping an empty list).
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
    let _ = stack.set(&StorageComponent, entries);
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
    /// The transmitter faces the face it was placed against (upstream's
    /// `DefaultDirectionType.FACE_CLICKED`).
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        let block = level.state_block(event.state)?;
        if Kind::from_block(block) != Some(Kind::WirelessTransmitter) {
            return None;
        }
        let face = Face::from_offset(event.context.face.normal())?;
        Some(Some(level.block_state_id(&format!(
            "{block}[facing={}]",
            face.property()
        ))?))
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

    /// The cable (its connections), the drive (its disks), the grid (its
    /// view) and the transmitter (its `active` state) tick; the controller
    /// holds no state.
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = matches!(
            Kind::from_block(event.block),
            Some(Kind::Cable | Kind::DiskDrive | Kind::Grid | Kind::WirelessTransmitter)
        );
    }

    /// One server tick of a network block (`native_block_entity_tick`).
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        match kind_at(host, pos) {
            Some(Kind::Cable) => cable_tick(host, pos),
            Some(Kind::DiskDrive) => drive_tick(host, pos, &mut event.entity),
            Some(Kind::Grid) => grid_tick(host, pos, &mut event.entity),
            Some(Kind::WirelessTransmitter) => transmitter_tick(host, pos),
            Some(Kind::Controller) | None => {}
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
            Kind::Controller | Kind::Cable | Kind::WirelessTransmitter => None,
        }
    }
}

// --- the ticks -------------------------------------------------------------

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

/// A drive's tick: mirror its disk items into the cache, or write a change the
/// grid made back into the disk's component.
fn drive_tick(host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
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
    let drives: Vec<[i32; 3]> = net.of_kind(Kind::DiskDrive).collect();
    let keys: Vec<DiskKey> = drive_keys(&dimension, &drives);
    // Only the slots the drives already mirror, in the same order, so a
    // change written back lands in the disk it came from.
    let (used, mut disks): (Vec<DiskKey>, Vec<Disk<Net>>) = with_core(|core| {
        keys.iter()
            .filter(|key| core.disks.contains_key(*key))
            .map(|key| (key.clone(), core.disks[key].clone()))
            .unzip()
    });

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
                    let left = if online {
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

    // Write the network's changes back into the drives' caches.
    with_core(|core| {
        for (key, disk) in used.iter().zip(disks.iter()) {
            core.disks.insert(key.clone(), disk.clone());
            core.dirty.insert(key.clone());
        }
    });

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
        // Forget the promises of grids that are no longer there: a broken grid
        // must not keep hiding the network's stock from the others. The block
        // is the truth, not the cache (a replaced block can leave its entity
        // behind for a tick).
        core.reserved.retain(|(dim, grid), _| {
            dim != &dimension
                || if *grid == pos {
                    true
                } else {
                    kind_at(host, *grid) == Some(Kind::Grid)
                }
        });
        core.reserved
            .iter()
            .filter(|((dim, grid), _)| !(dim == &dimension && *grid == pos))
            .flat_map(|(_, stacks)| stacks.iter().cloned())
            .collect::<Vec<_>>()
    });
    let entries = if online {
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
        core.reserved.insert((dimension.clone(), pos), promised);
    });
    entity.items = slots;
    let rebuilt: Vec<Option<Net>> = entity
        .items
        .iter()
        .map(|slot| slot.as_ref().map(|stack| Net::of(host, stack)))
        .collect();
    save_view(host, entity, &rebuilt, &owned);

    // The block state says whether the network works: an offline grid is dark.
    let previous = with_core(|core| core.active.get(&(dimension.clone(), pos)).copied());
    if previous != Some(online) {
        host.set_block_property(pos, "active", if online { "true" } else { "false" });
        with_core(|core| core.active.insert((dimension, pos), online));
    }
}

/// A transmitter's tick: `active` is whether the network it joins has a
/// controller. It joins through its facing side only, so a transmitter placed
/// with its face away from a cable is offline and answers no use.
fn transmitter_tick(host: &mut dyn ServerHost, pos: [i32; 3]) {
    let online = network_at(host, pos).online();
    let key = (host.dimension().to_owned(), pos);
    let previous = with_core(|core| core.active.get(&key).copied());
    if previous == Some(online) {
        return;
    }
    host.set_block_property(pos, "active", if online { "true" } else { "false" });
    with_core(|core| core.active.insert(key, online));
}

/// Every cache key a set of drives can hold.
fn drive_keys(dimension: &str, drives: &[[i32; 3]]) -> Vec<DiskKey> {
    drives
        .iter()
        .flat_map(|drive| (0..DRIVE_SLOTS).map(move |slot| (dimension.to_owned(), *drive, slot)))
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
    /// What each open grid currently shows: the network stock it has already
    /// promised its players (`view_with`). Two grids never show the same
    /// items, so neither can hand them out twice.
    #[expect(
        clippy::type_complexity,
        reason = "one grid's place to the stacks it shows; a newtype would not read better"
    )]
    reserved: HashMap<(String, [i32; 3]), Vec<(String, u32)>>,
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
    let inserted = insert_disk(host, pos, one);
    if inserted {
        consume_held(host, event.click.player, held);
        tell(host, event.click.player, "Storage disk inserted.");
    } else {
        tell(host, event.click.player, "The disk drive is full.");
    }
    // Either way the click is the drive's: the menu must not open behind a
    // disk that was placed.
    Flow::Cancel
}

/// Put `one` in the drive's first free slot, in one transaction.
fn insert_disk(host: &mut dyn ServerHost, pos: [i32; 3], one: ItemStack) -> bool {
    let Some(level) = host.level(host.dimension()) else {
        return false;
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return false;
    };
    let slot =
        (0..DRIVE_SLOTS).find(|slot| matches!(entity.item_slot(host, *slot as u32), Ok(None)));
    let Some(slot) = slot else {
        return false;
    };
    host.transaction(|host, tx| entity.set_item_slot(host, tx, slot as u32, Some(one)))
        .unwrap_or(false)
}

/// Take one item from the acting player's held stack. A disk is moved into
/// the drive, not used, so creative mode does not keep a free copy.
fn consume_held(host: &mut dyn ServerHost, uuid: u128, held: &ItemStack) {
    let Some(player) = host.player(uuid) else {
        return;
    };
    let Some(slot) = player.selected_slot(host) else {
        return;
    };
    let remaining = held.clone().split(1).remainder;
    let _ = player.set_inventory_slot(host, slot as u32, remaining);
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

fn info(host: &mut dyn ServerHost, pos: [i32; 3]) {
    if kind_at(host, pos).is_none() {
        feedback(host, "No Refined Storage network block there.");
        return;
    }
    let net = network_at(host, pos);
    let dimension = host.dimension().to_owned();
    let drives: Vec<[i32; 3]> = net.of_kind(Kind::DiskDrive).collect();
    let (disks, items, resources) = {
        let keys: Vec<DiskKey> = drive_keys(&dimension, &drives);
        with_core(|core| {
            let found: Vec<Disk<Net>> = keys
                .iter()
                .filter_map(|key| core.disks.get(key).cloned())
                .collect();
            let items: u64 = found.iter().map(Disk::total).sum();
            let resources = runtime::aggregate(&found).len();
            (found.len(), items, resources)
        })
    };
    let online = net.online();
    let mut census = format!(
        "{} nodes, {} controller, {} drive, {} grid",
        net.nodes.len(),
        net.count(Kind::Controller),
        drives.len(),
        net.count(Kind::Grid),
    );
    let transmitters = net.count(Kind::WirelessTransmitter);
    if transmitters > 0 {
        census.push_str(&format!(", {transmitters} transmitter"));
    }
    feedback(
        host,
        &format!(
            "Refined Storage network at {},{},{}: {state} ({census}); {} disks, {items} items in {resources} resources",
            pos[0],
            pos[1],
            pos[2],
            disks,
            state = if online { "online" } else { "offline" },
        ),
    );
    if kind_at(host, pos) == Some(Kind::WirelessTransmitter) {
        feedback(
            host,
            &format!(
                "Wireless transmitter at {},{},{}: range {WIRELESS_RANGE} blocks, {}",
                pos[0],
                pos[1],
                pos[2],
                if online { "active" } else { "inactive" },
            ),
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
    let online = network_in(host, level, pos).online();
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
    match runtime::admit(
        online,
        command.dimension == level.dimension(),
        distance,
        WIRELESS_RANGE,
    ) {
        Ok(()) => feedback(
            host,
            &format!(
                "Wireless access at {},{},{}: accepted ({distance:.1} of {WIRELESS_RANGE} blocks, {} network)",
                pos[0],
                pos[1],
                pos[2],
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
