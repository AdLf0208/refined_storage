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
    Component, Compound, Flow, Hand, Host, HostSide, HostTransaction, InteractionResult, ItemKey,
    ItemPart, ItemPlace, ItemResource, ItemStack, LevelHost, LevelRef, LevelSide, MenuClickedEvent,
    MenuClosedEvent, MenuRef, MenuReplace, MenuSlotBacking, ModBlockEntityView, ModContainer,
    RegisterCommandsEvent, Registrar, RegistrationContext, RightClickBlockEvent,
    RunModCommandEvent, Screen, ScreenEvent, ServerHost, ServerTickEvent, ServerTickPhase,
    StateForPlacementEvent, Tag, Text, UseItemEvent, UseOnEvent, WorldPhase,
};

use crate::runtime::{
    self, Binding, CAPACITY_1K, Cursor, DRIVE_SLOTS, Delta, Destination, Disk, Face, GRID_SLOTS,
    Kind, Node, Place, Plan, Promises, Region, Stored, ViewKey, WIRELESS_RANGE, extract, insert,
    network, plan_click, view_with,
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
/// The wireless grid (slice 4): a network-bound item that opens its network's
/// 9×6 grid view from anywhere inside a transmitter's range.
const WIRELESS_GRID: &str = "refinedstorage:wireless_grid";
/// The block-entity type this mod declares for the four network blocks.
const BE_TYPE: &str = "refinedstorage:network";
/// The item component that carries a disk's contents.
const COMPONENT: &str = "refinedstorage:storage";
/// The item component that names the transmitter a wireless grid is bound to
/// (upstream's `refinedstorage:network_location`, a `GlobalPos`).
const NETWORK_LOCATION: &str = "refinedstorage:network_location";

/// The title the wireless grid's menu shows (upstream's `ContentNames.WIRELESS_GRID`).
const WIRELESS_GRID_TITLE: &str = "Wireless Grid";
/// The wireless grid's menu has six rows, the block grid's 9×6 view.
const WIRELESS_GRID_ROWS: u8 = 6;

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
    wireless_grid: ItemKey,
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
        wireless_grid: registrar.item_key(WIRELESS_GRID),
    };
    let _ = KEYS.set(keys);
    registrar.item_part(keys.wireless_grid, WirelessGrid);
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

/// The disk's item component and the wireless grid's binding (`component_types`,
/// before content loads).
pub(crate) fn register_component_types(event: &mut mod_api::ComponentTypesEvent<'_>) {
    event.types.push(COMPONENT.to_owned());
    event.types.push(NETWORK_LOCATION.to_owned());
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
        let Some(player) = host.player(event.player) else {
            return Some(InteractionResult::Fail);
        };
        let Some(slot) = held_slot(host, &player, event.hand) else {
            return Some(InteractionResult::Fail);
        };
        let binding = Binding::new(host.dimension().to_owned(), event.pos);
        let bound = with_binding(event.stack.clone(), &binding);
        let wrote = host
            .transaction(|host, _tx| player.set_inventory_slot(host, slot, Some(bound)))
            .is_ok();
        if wrote {
            tell(
                host,
                event.player,
                &format!("Wireless grid bound to {}.", binding_name(&binding)),
            );
        }
        Some(InteractionResult::Success)
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
        open_wireless_grid(host, event.player, event.stack);
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
/// bound transmitter's network, or the reason the use is refused.
fn open_wireless_grid(host: &mut dyn ServerHost, uuid: u128, stack: &ItemStack) {
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
    match runtime::admit(
        online,
        here == binding.dimension(),
        distance,
        WIRELESS_RANGE,
    ) {
        Ok(()) => show_wireless_grid(host, uuid, &binding, level, distance),
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
            },
        );
    });
    refresh_menu(host, uuid, window, binding, level);
    tell(
        host,
        uuid,
        &format!(
            "Wireless grid {}: opened ({distance:.1} of {WIRELESS_RANGE} blocks).",
            binding_name(binding)
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
    let mut writes = Vec::new();
    for (disk, entries) in live.iter().zip(disks.iter()) {
        if &disk.disk == entries {
            continue;
        }
        let mut item = disk.item.clone();
        write_disk(host, &mut item, entries);
        writes.push((disk.drive, disk.slot, item));
    }
    if writes.is_empty() {
        return;
    }
    let _ = host.transaction(|host, tx| {
        for (drive, slot, item) in &writes {
            let Some(entity) = host.block_entity(level, *drive) else {
                continue;
            };
            entity.set_item_slot(host, tx, *slot as u32, Some(item.clone()))?;
        }
        Ok::<(), mod_api::BlockEntityItemError>(())
    });
}

/// The promises every other view of `net` makes, so this view never shows what
/// one of them already shows. A view of another network in the same dimension
/// is not one of them: it holds its own stock.
fn promises_elsewhere(net: &runtime::Network, dimension: &str, place: Place) -> Vec<(String, u32)> {
    let key = ViewKey::new(dimension, net.anchor(), place);
    with_core(|core| core.promises.promised_elsewhere(&key))
}

/// A wireless grid's open menu: its binding and what it currently shows.
#[derive(Clone, Debug)]
struct OpenGrid {
    binding: Binding,
    view: Vec<Option<ItemStack>>,
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
    let mut disks = disks_of(&live);
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
        write_back(host, level, &live, &disks);
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
    let disks = disks_of(&live);
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
    /// What each open view currently shows: the network stock it has already
    /// promised its players (`view_with`). Two views of one network never show
    /// the same items, so neither can hand them out twice, whether they are a
    /// grid block or a wireless grid's menu; two networks keep separate stock.
    promises: Promises,
    /// The wireless grids whose menu is open: the player and window id, the
    /// transmitter it is bound to and what it last showed.
    menus: HashMap<(u128, i32), OpenGrid>,
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
    // The binding a wireless grid carries, read back from the item component
    // (upstream shows it in the item's tooltip; a mod item's tooltip is a
    // client hook this port does not subscribe, so the readout is a command,
    // like `/refinedstorage info`).
    event.commands.push(Command::new("refinedstorage bound"));
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
