// Refined Storage slice 1 for Horizon: the item network core's pure model.
//
// Ported from Refined Storage 2.0.9 (refinedmods/refinedstorage2, MIT); this is
// a clean-room rewrite from the mod's observed behaviour, its data files and
// its public documentation. Local-only until the maintainer approves
// publishing (ARCHITECTURE.md §8).
//
// This module is the standalone, unit-tested core: the network's connected
// component and its on/off rule, a storage disk's capacity and contents, the
// aggregated view a grid shows, and the reconciliation of the grid's menu
// slots against the network. Everything that needs the game (block entities,
// menus, item components, commands) lives in `support.rs` and is injected as
// `crate::game::refinedstorage_state`.

/// A 1k storage disk's capacity in items (upstream `ItemStorageVariant.ONE_K`).
pub const CAPACITY_1K: u64 = 1024;
/// A 4k storage disk's capacity in items (upstream `ItemStorageVariant.FOUR_K`).
pub const CAPACITY_4K: u64 = CAPACITY_1K * 4;
/// A 16k storage disk's capacity in items (upstream `ItemStorageVariant.SIXTEEN_K`).
pub const CAPACITY_16K: u64 = CAPACITY_1K * 4 * 4;
/// A 64k storage disk's capacity in items (upstream `ItemStorageVariant.SIXTY_FOUR_K`).
pub const CAPACITY_64K: u64 = CAPACITY_1K * 4 * 4 * 4;
/// The grid's menu shows a 9x6 grid of aggregated entries.
pub const GRID_SLOTS: usize = 54;
/// The disk drive's slots (upstream `AMOUNT_OF_DISKS` is 8; the pin's menu
/// types only show 9/18/27/45/54 slots, so the port shows one row of nine and
/// records the difference).
pub const DRIVE_SLOTS: usize = 9;
/// The most blocks one flood fill visits; a network larger than this is cut.
pub const NETWORK_CAP: usize = 512;
/// How far a wireless transmitter reaches, in blocks (upstream
/// `DefaultEnergyUsage.WIRELESS_TRANSMITTER_BASE_RANGE`, the config key
/// `wirelessTransmitter.baseRange`; this port carries no config file yet, so the
/// upstream default is the rule).
pub const WIRELESS_RANGE: u32 = 16;
/// How much range one range upgrade adds, in blocks (upstream
/// `DefaultEnergyUsage.RANGE_UPGRADE_RANGE`, the config key
/// `upgrade.rangeUpgradeRange`).
pub const RANGE_PER_UPGRADE: u32 = 8;
/// The most range upgrades a wireless transmitter takes: upstream registers the
/// range upgrade for `WIRELESS_TRANSMITTER` with `maxAmount` 4, and its
/// `UpgradeContainer` refuses a fifth.
pub const MAX_RANGE_UPGRADES: u32 = 4;
/// The range a creative range upgrade gives: upstream's
/// `CreativeRangeUpgradeWirelessTransmitterRangeModifier` answers
/// `Integer.MAX_VALUE`, which no finite distance exceeds.
pub const CREATIVE_RANGE: u32 = i32::MAX as u32;

/// The network blocks of the port: the slice's four, the wireless transmitter
/// (slice 3) and the two transfer machines (slice 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Controller,
    Cable,
    DiskDrive,
    Grid,
    WirelessTransmitter,
    Importer,
    Exporter,
}

impl Kind {
    /// The block id this kind is, exactly as the content mod declares it.
    pub fn block_id(self) -> &'static str {
        match self {
            Kind::Controller => "refinedstorage:controller",
            Kind::Cable => "refinedstorage:cable",
            Kind::DiskDrive => "refinedstorage:disk_drive",
            Kind::Grid => "refinedstorage:grid",
            Kind::WirelessTransmitter => "refinedstorage:wireless_transmitter",
            Kind::Importer => "refinedstorage:importer",
            Kind::Exporter => "refinedstorage:exporter",
        }
    }

    /// The kind a block id stands for, or none for any other block.
    pub fn from_block(block: &str) -> Option<Kind> {
        [
            Kind::Controller,
            Kind::Cable,
            Kind::DiskDrive,
            Kind::Grid,
            Kind::WirelessTransmitter,
            Kind::Importer,
            Kind::Exporter,
        ]
        .into_iter()
        .find(|kind| kind.block_id() == block)
    }

    /// The human name a machine's info line leads with, for the two kinds that
    /// transfer; `None` for every other block.
    pub fn machine_name(self) -> Option<&'static str> {
        match self {
            Kind::Importer => Some("Importer"),
            Kind::Exporter => Some("Exporter"),
            _ => None,
        }
    }
}

/// A face of a block, in the order [`neighbours`] lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    North,
    East,
    South,
    West,
    Up,
    Down,
}

impl Face {
    /// The block offset one step out of this face.
    pub fn offset(self) -> [i32; 3] {
        match self {
            Face::North => [0, 0, -1],
            Face::East => [1, 0, 0],
            Face::South => [0, 0, 1],
            Face::West => [-1, 0, 0],
            Face::Up => [0, 1, 0],
            Face::Down => [0, -1, 0],
        }
    }

    /// The face across the block.
    pub fn opposite(self) -> Face {
        match self {
            Face::North => Face::South,
            Face::East => Face::West,
            Face::South => Face::North,
            Face::West => Face::East,
            Face::Up => Face::Down,
            Face::Down => Face::Up,
        }
    }

    /// The face a `facing`/`direction` state property value names, none for any
    /// other value.
    pub fn from_property(value: &str) -> Option<Face> {
        [
            Face::North,
            Face::East,
            Face::South,
            Face::West,
            Face::Up,
            Face::Down,
        ]
        .into_iter()
        .find(|face| face.property() == value)
    }

    /// The face a unit block offset names, none for anything else.
    pub fn from_offset(offset: [i32; 3]) -> Option<Face> {
        [
            Face::North,
            Face::East,
            Face::South,
            Face::West,
            Face::Up,
            Face::Down,
        ]
        .into_iter()
        .find(|face| face.offset() == offset)
    }

    /// The state property value this face is written as.
    pub fn property(self) -> &'static str {
        match self {
            Face::North => "north",
            Face::East => "east",
            Face::South => "south",
            Face::West => "west",
            Face::Up => "up",
            Face::Down => "down",
        }
    }
}

/// A block the flood fill reads: its kind, and, for a wireless transmitter, the
/// one face it joins the network through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Node {
    pub kind: Kind,
    /// `Some` for a block that joins through one face only (upstream's
    /// `WirelessTransmitterConnectionStrategy`), `None` for every other block.
    pub face: Option<Face>,
}

impl Node {
    /// A node that joins on all six sides.
    pub fn plain(kind: Kind) -> Self {
        Node { kind, face: None }
    }

    /// A node that joins through `face` only.
    pub fn facing(kind: Kind, face: Face) -> Self {
        Node {
            kind,
            face: Some(face),
        }
    }
}

/// The offset from one position to a face neighbour of it.
fn offset(from: [i32; 3], to: [i32; 3]) -> [i32; 3] {
    [to[0] - from[0], to[1] - from[1], to[2] - from[2]]
}

/// Whether two face-adjacent nodes link: every network block takes every side,
/// except a wireless transmitter, which joins through its facing side only and
/// then only when the neighbour on that side is the one asking (upstream's
/// `canAcceptIncomingConnection`: `incomingDirection == myDirection`).
fn links(from: Node, to: Node, step: [i32; 3]) -> bool {
    let leaves = from.face.is_none_or(|face| face.offset() == step);
    let arrives = to
        .face
        .is_none_or(|face| face.offset() == [-step[0], -step[1], -step[2]]);
    leaves && arrives
}

/// The six face neighbours of a position, in the order
/// `north, east, south, west, up, down`.
pub fn neighbours(pos: [i32; 3]) -> [[i32; 3]; 6] {
    [
        [pos[0], pos[1], pos[2] - 1],
        [pos[0] + 1, pos[1], pos[2]],
        [pos[0], pos[1], pos[2] + 1],
        [pos[0] - 1, pos[1], pos[2]],
        [pos[0], pos[1] + 1, pos[2]],
        [pos[0], pos[1] - 1, pos[2]],
    ]
}

/// One connected network: the blocks reachable from some start block through
/// face-adjacent network blocks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Network {
    pub nodes: Vec<(Kind, [i32; 3])>,
    /// True when the flood fill hit [`NETWORK_CAP`] and the list is cut.
    #[allow(
        dead_code,
        reason = "the runtime tests read it; slice 1 has no oversized network"
    )]
    pub truncated: bool,
}

impl Network {
    /// A network with a controller is online; without one nothing in it works
    /// (upstream: the controller is the network's root, and every node is
    /// inactive without it).
    pub fn online(&self) -> bool {
        self.nodes.iter().any(|(kind, _)| *kind == Kind::Controller)
    }

    /// The block positions of one kind, in visit order.
    pub fn of_kind(&self, kind: Kind) -> impl Iterator<Item = [i32; 3]> + '_ {
        self.nodes
            .iter()
            .filter(move |(k, _)| *k == kind)
            .map(|(_, pos)| *pos)
    }

    /// How many blocks of one kind the network holds.
    pub fn count(&self, kind: Kind) -> usize {
        self.nodes.iter().filter(|(k, _)| *k == kind).count()
    }

    /// The network's root: the controller's position (upstream's
    /// `AbstractNetworkNode.getRoot`), the smallest one when the flood fill
    /// found more than one, so the answer does not depend on where the fill
    /// started. `None` for a network with no controller — such a network is
    /// offline and its views are empty, so nothing is ever promised on it.
    pub fn anchor(&self) -> Option<[i32; 3]> {
        self.of_kind(Kind::Controller).min()
    }
}

/// Flood fill the network `start` belongs to. `at` answers the node at a position
/// from the world (none for a non-network or unloaded block); `start` itself must
/// be a network block.
pub fn network(start: [i32; 3], mut at: impl FnMut([i32; 3]) -> Option<Node>) -> Network {
    let mut out = Network::default();
    if at(start).is_none() {
        return out;
    }
    let mut seen = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    seen.insert(start);
    queue.push_back(start);
    while let Some(pos) = queue.pop_front() {
        let Some(node) = at(pos) else {
            continue;
        };
        if out.nodes.len() >= NETWORK_CAP {
            out.truncated = true;
            break;
        }
        out.nodes.push((node.kind, pos));
        for next in neighbours(pos) {
            if seen.contains(&next) {
                continue;
            }
            let Some(next_node) = at(next) else {
                continue;
            };
            if links(node, next_node, offset(pos, next)) {
                seen.insert(next);
                queue.push_back(next);
            }
        }
    }
    out
}

/// Why a network-bound use through a wireless transmitter was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    /// The user and the transmitter are in different dimensions.
    DifferentDimension,
    /// The transmitter's network has no controller, so every node of it is
    /// inactive.
    Offline,
    /// The transmitter's network cannot pay the transmitter's charge, so the
    /// node is inactive (upstream's `stored >= getEnergyUsage()` gate).
    NoEnergy { stored: u32, usage: u32 },
    /// The wireless grid item's own buffer cannot pay for the use it is being
    /// put to (upstream's `ItemEnergyStorage` on the wireless grid).
    NoItemEnergy { stored: u32, cost: u32 },
    /// The user is further from the transmitter's block than its range.
    OutOfRange { distance: f64, range: u32 },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::DifferentDimension => write!(f, "different dimension"),
            Refusal::Offline => write!(f, "the network is offline"),
            Refusal::NoEnergy { stored, usage } => {
                write!(f, "not enough energy: {stored} of {usage} FE")
            }
            Refusal::NoItemEnergy { stored, cost } => {
                write!(f, "not enough item energy: {stored} of {cost} FE")
            }
            Refusal::OutOfRange { distance, range } => {
                write!(f, "out of range: {distance:.1} > {range} blocks")
            }
        }
    }
}

/// The distance from a player's position to a transmitter's block, as upstream
/// measures it: the square root of three squared coordinate differences, the
/// transmitter at its integer block position and the player at their own.
pub fn distance(from: [f64; 3], block: [i32; 3]) -> f64 {
    let dx = f64::from(block[0]) - from[0];
    let dy = f64::from(block[1]) - from[1];
    let dz = f64::from(block[2]) - from[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Whether a network-bound use may use the network a wireless transmitter
/// exposes. `online` is whether the transmitter's component has a controller
/// (otherwise every node of it is inactive); `same_dimension` whether the user
/// and the transmitter are in one dimension; `energy` what the network holds
/// against the transmitter's own charge; `distance` the value [`distance`]
/// answered; `range` the transmitter's range in blocks.
///
/// The checks run in upstream's order (`NetworkItemPlayerValidator.isValid`):
/// dimension first, then the node's activeness — which is the controller's
/// presence and then the energy gate, upstream's `calculateActive` — then the
/// range, so a use in another dimension is refused as such even when the
/// transmitter is offline too, and a use with no energy is refused as such
/// before the range is even asked. The boundary is inclusive, as upstream's
/// `<=` is.
pub fn admit(
    online: bool,
    same_dimension: bool,
    energy: Energy,
    distance: f64,
    range: u32,
) -> Result<(), Refusal> {
    if !same_dimension {
        return Err(Refusal::DifferentDimension);
    }
    if !online {
        return Err(Refusal::Offline);
    }
    if !energy.energized() {
        return Err(Refusal::NoEnergy {
            stored: energy.stored,
            usage: energy.usage,
        });
    }
    if distance > f64::from(range) {
        return Err(Refusal::OutOfRange { distance, range });
    }
    Ok(())
}

/// The range a wireless transmitter reaches with `range_upgrades` range
/// upgrades in its upgrade slots, and `creative` whether a creative range
/// upgrade is among them.
///
/// Upstream composes three modifiers (`CompositeWirelessTransmitterRangeModifier`):
/// the base range (`WIRELESS_TRANSMITTER_BASE_RANGE`, 16), then
/// `range + amount * rangeUpgradeRange` per range upgrade, then
/// `Integer.MAX_VALUE` when the creative upgrade is present (it has the highest
/// priority, so it wins over any number of range upgrades). A count past
/// [`MAX_RANGE_UPGRADES`] is capped: upstream's container refuses a fifth range
/// upgrade, and a stray stack must not raise the range past it either.
pub fn upgraded_range(range_upgrades: u32, creative: bool) -> u32 {
    if creative {
        return CREATIVE_RANGE;
    }
    WIRELESS_RANGE + RANGE_PER_UPGRADE * range_upgrades.min(MAX_RANGE_UPGRADES)
}

/// How a range reads in the port's messages: in blocks, or `unlimited` for the
/// creative range, which no finite distance exceeds.
pub fn range_label(range: u32) -> String {
    if range >= CREATIVE_RANGE {
        return "unlimited".to_owned();
    }
    format!("{range} blocks")
}

// --- the transfer machines (slice 7) --------------------------------------

/// The ticks one work pass takes with no speed upgrade (upstream
/// `UpgradeContainer.DEFAULT_WORK_TICK_RATE`, 9).
pub const WORK_TICK_RATE: u32 = 9;

/// The ticks each speed upgrade removes from the work interval (upstream
/// `UpgradeContainer.notifyListener`: `defaultWorkTickRate - (amount * 2)`).
pub const SPEED_TICK_REDUCTION: u32 = 2;

/// The most speed upgrades a machine counts (upstream's `IMPORTER`/`EXPORTER`
/// registration: `speedUpgrade`, `maxAmount` 4).
pub const MAX_SPEED_UPGRADES: u32 = 4;

/// The most stack upgrades a machine counts (upstream's registration has no
/// `maxAmount`, so the container's four slots cap it; one is all that changes
/// the quota, and its recipe is one per machine).
pub const MAX_STACK_UPGRADES: u32 = 1;

/// The items one work pass moves with no stack upgrade (upstream's
/// `ItemHandlerImporterTransferStrategyFactory`/`ItemHandlerExporterTransferStrategyFactory`:
/// a single amount of 1).
pub const BASE_TRANSFER: u32 = 1;

/// The items one work pass moves with a stack upgrade (the same factories: a
/// single amount of 64).
pub const STACK_TRANSFER: u32 = 64;

/// The ticks between two work passes with `speed_upgrades` in the machine:
/// 9 at none, 7, 5, 3, and 1 from four on (upstream's throttled ticker, which
/// never counts a fifth).
pub fn work_interval(speed_upgrades: u32) -> u32 {
    WORK_TICK_RATE - SPEED_TICK_REDUCTION * speed_upgrades.min(MAX_SPEED_UPGRADES)
}

/// How many items one work pass may move with `stack_upgrades` in the machine.
pub fn transfer_quota(stack_upgrades: u32) -> u32 {
    if stack_upgrades >= MAX_STACK_UPGRADES {
        STACK_TRANSFER
    } else {
        BASE_TRANSFER
    }
}

/// Whether a work pass falls on this tick: upstream's ticker works when
/// `workTicks++ % workTickRate == 0`, so the first tick it sees works and the
/// rest follow every `interval` ticks.
pub fn work_due(ticks: u64, interval: u32) -> bool {
    ticks.is_multiple_of(u64::from(interval))
}

/// Whether a filter of resource keys lets `key` through (upstream's `Filter`
/// in its default `BLOCK` mode: an empty filter blocks nothing, so everything
/// is allowed, and a listed key is the only thing that is not).
pub fn allows(filter: &[String], key: &str) -> bool {
    !filter.iter().any(|blocked| blocked == key)
}

/// What an importer's work pass takes from its source: the first slot whose
/// resource the filter allows, and up to `quota` of it, capped by the slot's
/// own count. `None` when nothing is allowed or the source is empty.
pub fn import_plan(
    source: &[Option<(String, u32)>],
    filter: &[String],
    quota: u32,
) -> Option<(usize, String, u32)> {
    for (slot, entry) in source.iter().enumerate() {
        let Some((key, count)) = entry else {
            continue;
        };
        if *count == 0 || !allows(filter, key) {
            continue;
        }
        return Some((slot, key.clone(), quota.min(*count)));
    }
    None
}

/// What an exporter's work pass pushes: the first filter entry the network
/// holds, and up to `quota` of it, capped by the network's stock. `None` when
/// the filter is empty (upstream's exporter has one task per filter entry, so
/// no entry means no work) or nothing it names is held.
pub fn export_plan(filter: &[String], held: &[(String, u64)], quota: u32) -> Option<(String, u32)> {
    for key in filter {
        let Some((_, amount)) = held.iter().find(|(resource, _)| resource == key) else {
            continue;
        };
        if *amount == 0 {
            continue;
        }
        let capped = u32::try_from(*amount).unwrap_or(u32::MAX);
        return Some((key.clone(), quota.min(capped)));
    }
    None
}

/// A machine's `/refinedstorage info` line: its name and position, the two
/// upgrade counts, the work interval the speed upgrades set, and how many
/// filter entries it holds.
pub fn machine_readout(
    name: &str,
    pos: [i32; 3],
    speed_upgrades: u32,
    stack_upgrades: u32,
    filters: usize,
) -> String {
    let interval = work_interval(speed_upgrades);
    let plural = |count: usize| if count == 1 { "" } else { "s" };
    format!(
        "{name} at {},{},{}: {speed_upgrades} speed upgrade{}, {stack_upgrades} stack upgrade{}, work interval {interval} tick{}, {filters} filter{}",
        pos[0],
        pos[1],
        pos[2],
        plural(speed_upgrades as usize),
        plural(stack_upgrades as usize),
        plural(interval as usize),
        plural(filters),
    )
}

/// What a stored entry must answer for the network to treat it as one resource.
pub trait Stored: Clone {
    /// The identity of the resource: two entries with the same key are
    /// different amounts of the same thing (the item id and its components).
    fn key(&self) -> String;
    /// How many items the entry holds.
    fn count(&self) -> u32;
    /// The same resource with another count.
    fn with_count(&self, count: u32) -> Self;
}

/// A storage disk's capacity: a finite room in items, or the creative disk's
/// unlimited marker (upstream `ItemStorageVariant.CREATIVE`'s `@Nullable Long
/// capacity` of `null`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskCapacity {
    /// Room for this many items.
    Items(u64),
    /// No ceiling: the creative disk (`creative_storage_disk`).
    Unlimited,
}

impl DiskCapacity {
    /// The items this disk can hold in total (`u64::MAX` for the creative
    /// disk, whose `insert` therefore never refuses an amount).
    pub fn room(self) -> u64 {
        match self {
            DiskCapacity::Items(items) => items,
            // Upstream's creative disk is a plain storage with no `LimitedStorage`
            // around it, so nothing ever refuses an insertion (`StorageImpl.insert`
            // answers the whole amount). A `u64` of room is how this port says
            // "no ceiling" without inventing one.
            DiskCapacity::Unlimited => u64::MAX,
        }
    }

    /// The saved spelling: the item count, or `0` for the creative disk's
    /// unlimited marker (upstream's `null` has no NBT form, and this port's
    /// component stores a number).
    pub fn saved(self) -> u64 {
        match self {
            DiskCapacity::Items(items) => items,
            DiskCapacity::Unlimited => 0,
        }
    }

    /// A saved spelling back into a capacity: `0` is the creative disk.
    pub fn from_saved(saved: u64) -> Self {
        if saved == 0 {
            DiskCapacity::Unlimited
        } else {
            DiskCapacity::Items(saved)
        }
    }

    /// This capacity within `limit`: a disk's item decides how much room it
    /// has, so a component claiming more is cut to it (a doctored or stale
    /// item cannot give a 1k disk a 64k room).
    pub fn clamp(self, limit: Self) -> Self {
        match (self, limit) {
            (DiskCapacity::Items(said), DiskCapacity::Items(item)) => {
                DiskCapacity::Items(said.min(item))
            }
            (DiskCapacity::Items(said), DiskCapacity::Unlimited) => DiskCapacity::Items(said),
            (DiskCapacity::Unlimited, DiskCapacity::Items(item)) => DiskCapacity::Items(item),
            (DiskCapacity::Unlimited, DiskCapacity::Unlimited) => DiskCapacity::Unlimited,
        }
    }

    /// The capacity as a readout: `"4096 items"`, or `"unlimited"`.
    pub fn label(self) -> String {
        match self {
            DiskCapacity::Items(items) => format!("{items} items"),
            DiskCapacity::Unlimited => "unlimited".to_owned(),
        }
    }
}

/// The capacity of the storage disk named by `item` (upstream's
/// `ItemStorageVariant`), `None` for an item that is not a storage disk.
pub fn disk_capacity(item: &str) -> Option<DiskCapacity> {
    if let Some(variant) = disk_variant(item) {
        return Some(variant.capacity);
    }
    // Upstream `ItemStorageVariant.CREATIVE` carries no capacity at all.
    (item == DISK_CREATIVE).then_some(DiskCapacity::Unlimited)
}

/// One storage variant (upstream's `ItemStorageVariant`): the disk item, the
/// storage part it is built from, and the room its item names. The creative
/// disk is not here — upstream gives it neither a capacity nor a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageVariant {
    /// The variant's own name, the spelling a recipe's `to` field uses (`"4k"`).
    pub name: &'static str,
    /// The disk item.
    pub disk: &'static str,
    /// The storage part the disk is built from — and what an upgrade returns.
    pub part: &'static str,
    /// The room the disk's item names.
    pub capacity: DiskCapacity,
}

/// The four finite storage variants, smallest first (upstream's
/// `ItemStorageVariant` declaration order).
pub const STORAGE_VARIANTS: [StorageVariant; 4] = [
    StorageVariant {
        name: "1k",
        disk: "refinedstorage:1k_storage_disk",
        part: "refinedstorage:1k_storage_part",
        capacity: DiskCapacity::Items(CAPACITY_1K),
    },
    StorageVariant {
        name: "4k",
        disk: "refinedstorage:4k_storage_disk",
        part: "refinedstorage:4k_storage_part",
        capacity: DiskCapacity::Items(CAPACITY_4K),
    },
    StorageVariant {
        name: "16k",
        disk: "refinedstorage:16k_storage_disk",
        part: "refinedstorage:16k_storage_part",
        capacity: DiskCapacity::Items(CAPACITY_16K),
    },
    StorageVariant {
        name: "64k",
        disk: "refinedstorage:64k_storage_disk",
        part: "refinedstorage:64k_storage_part",
        capacity: DiskCapacity::Items(CAPACITY_64K),
    },
];

/// The creative disk: the only variant with no capacity and no part.
pub const DISK_CREATIVE: &str = "refinedstorage:creative_storage_disk";

/// The finite variant a disk item is, `None` for anything else (the creative
/// disk included: it has no part and no capacity to upgrade into).
pub fn disk_variant(disk: &str) -> Option<StorageVariant> {
    STORAGE_VARIANTS
        .into_iter()
        .find(|variant| variant.disk == disk)
}

/// The disk item a recipe's `to` variant names (the creative disk included, so
/// a file naming it decodes; it is simply never a destination).
pub fn disk_of_variant(name: &str) -> Option<&'static str> {
    if name == "creative" {
        return Some(DISK_CREATIVE);
    }
    STORAGE_VARIANTS
        .into_iter()
        .find(|variant| variant.name == name)
        .map(|variant| variant.disk)
}

/// The storage part a disk item is built from, `None` for the creative disk and
/// for anything that is not a disk.
pub fn storage_part_of_disk(disk: &str) -> Option<&'static str> {
    disk_variant(disk).map(|variant| variant.part)
}

/// The disks an upgrade into `to_disk` accepts: the finite ones with less room
/// (upstream's `getValidSourceContainers`), empty for the creative disk, whose
/// capacity is `null`.
pub fn upgrade_sources(to_disk: &str) -> Vec<&'static str> {
    let Some(destination) = disk_variant(to_disk) else {
        return Vec::new();
    };
    STORAGE_VARIANTS
        .into_iter()
        .filter(|variant| variant.capacity.room() < destination.capacity.room())
        .map(|variant| variant.disk)
        .collect()
}

/// Where an upgrade grid's two ingredients sit: the source disk's cell and the
/// destination part's cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpgradeGrid {
    /// The cell holding the smaller disk.
    pub disk: usize,
    /// The cell holding the destination tier's storage part.
    pub part: usize,
}

/// The cells of a matching upgrade grid: exactly one source disk and exactly one
/// destination part, every other cell empty. `None` when the grid is not that
/// pair (a third item, two disks, the wrong tier's part, the creative disk).
pub fn upgrade_cells(cells: &[Option<&str>], to_disk: &str) -> Option<UpgradeGrid> {
    let destination = disk_variant(to_disk)?;
    let sources = upgrade_sources(to_disk);
    let mut disk = None;
    let mut part = None;
    for (slot, cell) in cells.iter().enumerate() {
        let Some(id) = cell else { continue };
        if sources.contains(id) {
            if disk.is_some() {
                return None;
            }
            disk = Some(slot);
        } else if *id == destination.part {
            if part.is_some() {
                return None;
            }
            part = Some(slot);
        } else {
            // A shapeless recipe of two ingredients accepts no third one.
            return None;
        }
    }
    Some(UpgradeGrid {
        disk: disk?,
        part: part?,
    })
}

/// `source`'s contents on the disk `to_disk` names, with the destination item's
/// room (upstream's `transferTo`: the new item decides the capacity, the
/// storage's contents are reused). `None` unless that disk is a bigger one.
pub fn upgraded_disk<T: Stored>(source: &Disk<T>, to_disk: &str) -> Option<Disk<T>> {
    let destination = disk_variant(to_disk)?;
    if destination.capacity.room() <= source.capacity().room() {
        return None;
    }
    let mut upgraded = Disk::new(destination.capacity.room());
    upgraded.set_entries(source.entries().to_vec());
    Some(upgraded)
}

/// One storage disk's contents: at most `capacity` items over any number of
/// resources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disk<T> {
    capacity: DiskCapacity,
    entries: Vec<T>,
}

impl<T: Stored> Disk<T> {
    /// An empty disk with `capacity` items of room.
    pub fn new(capacity: u64) -> Self {
        Disk {
            capacity: DiskCapacity::Items(capacity),
            entries: Vec::new(),
        }
    }

    /// An empty creative disk: no ceiling on what it stores.
    pub fn unlimited() -> Self {
        Disk {
            capacity: DiskCapacity::Unlimited,
            entries: Vec::new(),
        }
    }

    /// The disk's capacity.
    pub fn capacity(&self) -> DiskCapacity {
        self.capacity
    }

    /// Everything the disk holds, one entry per resource, in insertion order.
    pub fn entries(&self) -> &[T] {
        &self.entries
    }

    /// Replace the contents (a disk read back from its item component).
    pub fn set_entries(&mut self, entries: Vec<T>) {
        self.entries = entries;
        self.trim();
    }

    /// The items the disk holds.
    pub fn total(&self) -> u64 {
        self.entries
            .iter()
            .map(|entry| u64::from(entry.count()))
            .sum()
    }

    /// The items the disk can still take.
    pub fn free(&self) -> u64 {
        self.capacity.room().saturating_sub(self.total())
    }

    /// How many of `key` the disk holds.
    #[allow(
        dead_code,
        reason = "the runtime tests read it; the grid aggregates instead"
    )]
    pub fn amount(&self, key: &str) -> u64 {
        self.entries
            .iter()
            .filter(|entry| entry.key() == key)
            .map(|entry| u64::from(entry.count()))
            .sum()
    }

    /// Put up to the disk's free space of `stack` in, answering how many items
    /// it took. The amount is a count, never a stack: a stack cannot hold zero
    /// items (`ItemStack::set_count(0)` is an error), so "nothing left over"
    /// has no stack form and must not be built.
    pub fn insert(&mut self, stack: T) -> u32 {
        let count = stack.count();
        if count == 0 {
            return 0;
        }
        let key = stack.key();
        let mut room = self.free();
        let mut left = count;
        let mut taken = 0;
        for entry in &mut self.entries {
            if left == 0 || room == 0 {
                break;
            }
            if entry.key() != key {
                continue;
            }
            let moved = room.min(u64::from(left));
            if moved > 0 {
                *entry = entry.with_count(entry.count() + moved as u32);
                left -= moved as u32;
                room -= moved;
                taken += moved as u32;
            }
        }
        if left > 0 && room > 0 && self.entries.len() < MAX_RESOURCES {
            let moved = room.min(u64::from(left)) as u32;
            self.entries.push(stack.with_count(moved));
            left -= moved;
            taken += moved;
        }
        debug_assert_eq!(taken + left, count, "every item was taken or is left");
        taken
    }

    /// Take up to `want` of `key`, answering the stack taken (none when the
    /// disk does not hold it).
    pub fn extract(&mut self, key: &str, want: u32) -> Option<T> {
        let mut taken: Option<T> = None;
        let mut left = want;
        let mut index = 0;
        while index < self.entries.len() && left > 0 {
            if self.entries[index].key() == key {
                let have = self.entries[index].count();
                let moved = have.min(left);
                let entry = self.entries[index].clone();
                taken = Some(match taken {
                    Some(previous) => previous.with_count(previous.count() + moved),
                    None => entry.with_count(moved),
                });
                left -= moved;
                if moved == have {
                    self.entries.remove(index);
                    continue;
                }
                self.entries[index] = self.entries[index].with_count(have - moved);
            }
            index += 1;
        }
        taken
    }

    /// Empty the disk, answering what it held.
    #[allow(
        dead_code,
        reason = "the port's API; slice 1 empties a disk only by taking items"
    )]
    pub fn drain(&mut self) -> Vec<T> {
        std::mem::take(&mut self.entries)
    }

    /// Drop empty entries and merge entries of one resource; keep capacity.
    fn trim(&mut self) {
        let mut merged: Vec<T> = Vec::new();
        for entry in std::mem::take(&mut self.entries) {
            if entry.count() == 0 {
                continue;
            }
            let key = entry.key();
            match merged.iter_mut().find(|kept| kept.key() == key) {
                Some(kept) => *kept = kept.with_count(kept.count().saturating_add(entry.count())),
                None => merged.push(entry),
            }
        }
        let mut room = self.capacity.room();
        merged.retain_mut(|entry| {
            let kept = u64::from(entry.count()).min(room);
            *entry = entry.with_count(kept as u32);
            room -= kept;
            kept > 0
        });
        self.entries = merged;
    }
}

/// Insert into the first disk that can take the whole stack, answering what it
/// could not take (`None` when it took every item). A disk takes a whole stack
/// or nothing, so a network never spreads one stack over two disks (upstream
/// inserts into one storage at a time).
pub fn insert<T: Stored>(disks: &mut [Disk<T>], stack: T) -> Option<T> {
    let count = stack.count();
    if count == 0 {
        return None;
    }
    let key = stack.key();
    for disk in disks {
        if disk.free() < u64::from(count) || !disk.fits(&key) {
            continue;
        }
        let taken = disk.insert(stack.clone());
        return (taken < count).then(|| stack.with_count(count - taken));
    }
    Some(stack)
}

impl<T: Stored> Disk<T> {
    /// Whether the disk can hold more of `key`: it already does, or it has
    /// room for a new resource (one entry per disk resource, so a disk that
    /// holds `NETWORK` resources does not accept another).
    fn fits(&self, key: &str) -> bool {
        self.entries.iter().any(|entry| entry.key() == key) || self.entries.len() < MAX_RESOURCES
    }
}

/// The most distinct resources one disk tracks (it mirrors the number of
/// entries a menu can show, and keeps a disk's item component small).
pub const MAX_RESOURCES: usize = 256;

/// Take up to `want` of `key` from the disks, answering the stack taken.
pub fn extract<T: Stored>(disks: &mut [Disk<T>], key: &str, want: u32) -> Option<T> {
    let mut left = want;
    let mut taken: Option<T> = None;
    for disk in disks {
        if left == 0 {
            break;
        }
        if let Some(part) = disk.extract(key, left) {
            left -= part.count();
            taken = Some(match taken {
                Some(previous) => previous.with_count(previous.count() + part.count()),
                None => part,
            });
        }
    }
    taken
}

/// How many of `key` the disks hold together.
#[allow(
    dead_code,
    reason = "the runtime tests read it; `info` sums the disks instead"
)]
pub fn amount<T: Stored>(disks: &[Disk<T>], key: &str) -> u64 {
    disks.iter().map(|disk| disk.amount(key)).sum()
}

/// Every resource the disks hold, one entry per resource with its full total.
pub fn aggregate<T: Stored>(disks: &[Disk<T>]) -> Vec<T> {
    let mut out: Vec<T> = Vec::new();
    for disk in disks {
        for entry in disk.entries() {
            let key = entry.key();
            match out.iter_mut().find(|kept| kept.key() == key) {
                Some(kept) => *kept = kept.with_count(kept.count().saturating_add(entry.count())),
                None => out.push(entry.clone()),
            }
        }
    }
    out
}

/// Whether a resource's key matches a grid search: a case-insensitive
/// substring of the item id (`minecraft:diamond` matches `diamond`, `MI`,
/// `minecraft`).
pub fn matches(key: &str, search: &str) -> bool {
    let search = search.trim().to_ascii_lowercase();
    search.is_empty() || key.to_ascii_lowercase().contains(&search)
}

/// The grid's view of the network: one entry per matching resource, sorted by
/// amount ascending and then by identity (upstream's default grid sorting is
/// quantity, ascending), each entry capped to one stack so it fits a menu
/// slot. `max_stack` says how many items one stack of a resource holds in
/// this game (64, 16, 1...).
pub fn view<T: Stored>(
    disks: &[Disk<T>],
    search: &str,
    slots: usize,
    max_stack: impl Fn(&T) -> u32,
) -> Vec<T> {
    view_with(disks, &[], search, slots, max_stack)
}

/// The grid's view of the network once other grids' promises are set aside:
/// [`view`] minus what another grid already shows its players.
///
/// Every grid exposes the *same* network through real menu slots, so without
/// this two grids both show the last ten diamonds and both players take them.
/// A grid passes the stacks the other open grids are showing; the total shown
/// across grids therefore never exceeds what the disks hold.
pub fn view_with<T: Stored>(
    disks: &[Disk<T>],
    held: &[(String, u32)],
    search: &str,
    slots: usize,
    max_stack: impl Fn(&T) -> u32,
) -> Vec<T> {
    let mut entries: Vec<T> = aggregate(disks)
        .into_iter()
        .filter(|entry| matches(&entry.key(), search))
        .filter_map(|entry| {
            let promised: u32 = held
                .iter()
                .filter(|(key, _)| *key == entry.key())
                .map(|(_, count)| *count)
                .sum();
            let left = entry.count().saturating_sub(promised);
            (left > 0).then(|| entry.with_count(left))
        })
        .collect();
    entries.sort_by(|a, b| {
        a.count()
            .cmp(&b.count())
            .then_with(|| a.key().cmp(&b.key()))
    });
    entries.truncate(slots);
    entries
        .into_iter()
        .map(|entry| {
            let cap = max_stack(&entry).max(1);
            entry.with_count(entry.count().min(cap))
        })
        .collect()
}

// --- the promise table -----------------------------------------------------

/// Where a view sits: a block grid's position, or a wireless grid's open menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    /// The grid block at a position.
    Grid([i32; 3]),
    /// A wireless grid's open menu: its player and window id.
    Wireless { player: u128, window: i32 },
}

/// A view's identity in the promise table: the dimension it is in, the network
/// it shows and which view it is.
///
/// The network is part of the key because two *different* networks in one
/// dimension keep separate stock: keying by dimension alone made each hide the
/// other's items (an over-exclusion, never a duplication).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ViewKey {
    /// The dimension's id, e.g. `minecraft:the_end`.
    pub dimension: String,
    /// The network's root ([`Network::anchor`]); `None` for a network with no
    /// controller, whose view is empty and therefore promises nothing.
    pub network: Option<[i32; 3]>,
    /// Which view it is.
    pub place: Place,
}

impl ViewKey {
    /// The key of the view `place`, showing `network` in `dimension`.
    pub fn new(dimension: &str, network: Option<[i32; 3]>, place: Place) -> Self {
        ViewKey {
            dimension: dimension.to_owned(),
            network,
            place,
        }
    }
}

/// What every open view currently shows. Every grid exposes the *same* network
/// through real menu slots, so two views of one network must never show the
/// same items; [`Promises::promised_elsewhere`] is what a view sets aside before
/// it shows its network's stock (`view_with`).
#[derive(Debug, Default)]
pub struct Promises {
    views: std::collections::HashMap<ViewKey, Vec<(String, u32)>>,
}

impl Promises {
    /// Remember what the view `key` shows.
    pub fn record(&mut self, key: ViewKey, promised: Vec<(String, u32)>) {
        self.views.insert(key, promised);
    }

    /// The stacks every *other* view of the same network promises, which this
    /// view must set aside. A view of another network in the same dimension is
    /// not one of them: it holds its own stock.
    pub fn promised_elsewhere(&self, key: &ViewKey) -> Vec<(String, u32)> {
        self.views
            .iter()
            .filter(|(other, _)| {
                other.dimension == key.dimension
                    && other.network == key.network
                    && other.place != key.place
            })
            .flat_map(|(_, stacks)| stacks.iter().cloned())
            .collect()
    }

    /// Forget the views `keep` rejects: a broken grid must not keep hiding the
    /// network's stock from the others.
    pub fn retain(&mut self, mut keep: impl FnMut(&ViewKey) -> bool) {
        self.views.retain(|key, _| keep(key));
    }

    /// Forget the view at `place` wherever it is (a closed menu).
    pub fn release(&mut self, place: Place) {
        self.views.retain(|key, _| key.place != place);
    }
}

/// What a player's click did to one grid slot, as the next tick sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Delta<T> {
    /// The player took this much out of the slot: take it off the network.
    Withdraw { slot: usize, stack: T },
    /// The player put this much into the slot: insert it into the network.
    Deposit { slot: usize, stack: T },
}

/// Compare a grid's slots with what the port last put there and answer the
/// network operations a player's clicks and drags became. Only slots the port
/// owns are compared: a slot it marked `owned` (a stack the network could not
/// take) belongs to the player until they empty it.
pub fn deltas<T: Stored>(
    before: &[Option<T>],
    after: &[Option<T>],
    owned: &[bool],
) -> Vec<Delta<T>> {
    let mut out = Vec::new();
    for index in 0..before.len().max(after.len()) {
        if owned.get(index).copied().unwrap_or(false) {
            continue;
        }
        let old = before.get(index).and_then(|slot| slot.as_ref());
        let new = after.get(index).and_then(|slot| slot.as_ref());
        match (old, new) {
            (None, None) => {}
            (Some(old), None) => out.push(Delta::Withdraw {
                slot: index,
                stack: old.clone(),
            }),
            (None, Some(new)) => out.push(Delta::Deposit {
                slot: index,
                stack: new.clone(),
            }),
            (Some(old), Some(new)) if old.key() == new.key() => match new.count().cmp(&old.count())
            {
                std::cmp::Ordering::Less => out.push(Delta::Withdraw {
                    slot: index,
                    stack: old.with_count(old.count() - new.count()),
                }),
                std::cmp::Ordering::Greater => out.push(Delta::Deposit {
                    slot: index,
                    stack: new.with_count(new.count() - old.count()),
                }),
                std::cmp::Ordering::Equal => {}
            },
            (Some(old), Some(new)) => {
                out.push(Delta::Withdraw {
                    slot: index,
                    stack: old.clone(),
                });
                out.push(Delta::Deposit {
                    slot: index,
                    stack: new.clone(),
                });
            }
        }
    }
    out
}

// --- the network-bound item (slice 4) --------------------------------------

/// The transmitter a wireless grid is bound to: its dimension and its block,
/// upstream's `GlobalPos` in the `refinedstorage:network_location` component.
///
/// The dimension is part of the binding, not just the position: the same
/// coordinates in another dimension are another transmitter, which is why the
/// use has to resolve the level by name before it can read the network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    dimension: String,
    pos: [i32; 3],
}

impl Binding {
    /// A binding to the transmitter block at `pos` in `dimension`.
    pub fn new(dimension: impl Into<String>, pos: [i32; 3]) -> Self {
        Binding {
            dimension: dimension.into(),
            pos,
        }
    }

    /// The bound dimension's id (`minecraft:the_end`).
    pub fn dimension(&self) -> &str {
        &self.dimension
    }

    /// The bound transmitter's block position.
    pub fn pos(&self) -> [i32; 3] {
        self.pos
    }

    /// Whether this binding names the block at `pos` in `dimension`.
    pub fn describes(&self, dimension: &str, pos: [i32; 3]) -> bool {
        self.dimension == dimension && self.pos == pos
    }
}

/// How much of a grid resource a click moves (upstream's `GridExtractMode` and
/// the two of `GridInsertMode` this port needs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtractMode {
    /// The whole resource, capped by its maximum stack size.
    Entire,
    /// Half of that, rounded down.
    Half,
    /// One item.
    Single,
}

/// How many of a stored resource `mode` moves when it is withdrawn from a
/// network with `total` of it; `max_stack` is the resource's maximum stack size.
///
/// Transcribed from upstream's `GridOperationsImpl`: the base amount is
/// `min(total, max_stack)`, `Half` is that halved unless it is one item, and
/// `Single` is one.
pub fn extract_amount(total: u32, max_stack: u32, mode: ExtractMode) -> u32 {
    let available = total.min(max_stack);
    match mode {
        ExtractMode::Entire => available,
        ExtractMode::Half if available <= 1 => available,
        ExtractMode::Half => available / 2,
        ExtractMode::Single => available.min(1),
    }
}

/// How many of a resource a deposit moves: a whole stack in `Entire` mode, one
/// item in `Single` mode (upstream's `GridOperationsImpl.insert`).
pub fn insert_amount(max_stack: u32, mode: ExtractMode) -> u32 {
    match mode {
        ExtractMode::Entire | ExtractMode::Half => max_stack,
        ExtractMode::Single => 1,
    }
}

/// Which part of a grid menu a click landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    /// One of the menu's own slots, which show the network's resources.
    View,
    /// One of the player's own inventory slots below the view.
    Inventory,
}

/// What the cursor (the stack the player carries, upstream's `getCarried`)
/// holds, as far as a click cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    /// Nothing.
    Empty,
    /// The resource the click concerns, with `room` items to spare.
    Same { room: u32 },
    /// Another resource, so it can only be put down.
    Other,
}

/// Where a withdrawal goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    /// Onto the cursor, as a normal click does.
    Cursor,
    /// Straight into the player's inventory, as a shift-click does.
    Inventory,
}

/// What a click on a player's own inventory slot does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorMove {
    /// Take the slot's stack (`half` for a right click: half of it, rounded up).
    Take { half: bool },
    /// Put the cursor's stack into an empty slot (`one` for a right click).
    Place { one: bool },
    /// Top the slot up from the cursor (`one` for a right click).
    Merge { one: bool },
    /// Exchange the cursor's resource with the slot's.
    Swap,
    /// Nothing moves.
    Nothing,
}

/// What a click in a grid menu does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Take resources off the network.
    Withdraw {
        /// How much of the resource.
        mode: ExtractMode,
        /// Where it goes.
        to: Destination,
    },
    /// Put the cursor's resource onto the network.
    Deposit {
        /// How much of it.
        mode: ExtractMode,
    },
    /// The player's own inventory's cursor rules.
    Inventory(CursorMove),
    /// Nothing moves.
    Nothing,
}

/// What a click does in a grid menu whose own slots show the network's
/// resources: `region` where it landed, `slot_has` whether that slot holds
/// anything, `cursor` what the player carries, `button` the mouse button (0
/// left, 1 right) and `shift` whether it is a shift-click (the menu's quick-move
/// click mode).
///
/// Transcribed from upstream's `AbstractGridScreen`: a click on a resource slot
/// withdraws when the cursor can take the resource and deposits when it cannot
/// (or when it is a different resource, or the slot is empty); a shift-click
/// always withdraws to the player's inventory, or deposits the clicked stack
/// from it. A plain click on the player's own inventory is the cursor's.
pub fn plan_click(region: Region, slot_has: bool, cursor: Cursor, button: u8, shift: bool) -> Plan {
    let withdraw_mode = if button == 1 {
        ExtractMode::Half
    } else {
        ExtractMode::Entire
    };
    let deposit_mode = if button == 1 {
        ExtractMode::Single
    } else {
        ExtractMode::Entire
    };
    match region {
        Region::Inventory if shift => {
            if slot_has {
                Plan::Deposit {
                    mode: ExtractMode::Entire,
                }
            } else {
                Plan::Nothing
            }
        }
        Region::Inventory => Plan::Inventory(inventory_click(slot_has, cursor, button)),
        Region::View if shift => {
            if slot_has {
                Plan::Withdraw {
                    mode: ExtractMode::Entire,
                    to: Destination::Inventory,
                }
            } else {
                Plan::Nothing
            }
        }
        Region::View => {
            let takes = match cursor {
                Cursor::Empty => slot_has,
                Cursor::Same { room } => slot_has && room > 0,
                Cursor::Other => false,
            };
            if takes {
                Plan::Withdraw {
                    mode: withdraw_mode,
                    to: Destination::Cursor,
                }
            } else if cursor != Cursor::Empty {
                Plan::Deposit { mode: deposit_mode }
            } else {
                Plan::Nothing
            }
        }
    }
}

/// The cursor's own rules in the player's inventory, as vanilla's left and
/// right clicks: an empty cursor takes, a cursor puts down or merges, another
/// resource swaps.
pub fn inventory_click(slot_has: bool, cursor: Cursor, button: u8) -> CursorMove {
    let right = button == 1;
    match (cursor, slot_has) {
        (Cursor::Empty, true) => CursorMove::Take { half: right },
        (Cursor::Empty, false) => CursorMove::Nothing,
        (Cursor::Same { .. }, true) => CursorMove::Merge { one: right },
        (Cursor::Same { .. }, false) => CursorMove::Place { one: right },
        (Cursor::Other, true) => CursorMove::Swap,
        (Cursor::Other, false) => CursorMove::Place { one: right },
    }
}

// --- the network's energy (slice 8) ---------------------------------------

/// The controller's buffer, in FE (upstream
/// `DefaultEnergyUsage.CONTROLLER_CAPACITY`).
pub const CONTROLLER_CAPACITY: u32 = 1000;

/// What a controller charges: nothing (upstream `ControllerNetworkNode
/// ::getEnergyUsage` is 0; the controller is the network's provider).
pub const CONTROLLER_USAGE: u32 = 0;

/// What a cable charges: nothing (upstream `DefaultEnergyUsage.CABLE`).
pub const CABLE_USAGE: u32 = 0;

/// What a grid charges per tick it works (upstream `DefaultEnergyUsage.GRID`).
pub const GRID_USAGE: u32 = 10;

/// What a disk drive charges per tick, plus [`DISK_DRIVE_PER_DISK_USAGE`] for
/// each disk in it (upstream `DISK_DRIVE` and `DISK_DRIVE_PER_DISK`).
pub const DISK_DRIVE_USAGE: u32 = 10;
pub const DISK_DRIVE_PER_DISK_USAGE: u32 = 4;

/// What each transfer machine charges per work pass (upstream `IMPORTER` and
/// `EXPORTER`).
pub const IMPORTER_USAGE: u32 = 2;
pub const EXPORTER_USAGE: u32 = 2;

/// What a wireless transmitter charges per tick (upstream
/// `WIRELESS_TRANSMITTER`).
pub const TRANSMITTER_USAGE: u32 = 16;

/// What each upgrade adds to the holder's charge (upstream `SPEED_UPGRADE`,
/// `STACK_UPGRADE`, `RANGE_UPGRADE` and `CREATIVE_RANGE_UPGRADE`).
pub const SPEED_UPGRADE_USAGE: u32 = 4;
pub const STACK_UPGRADE_USAGE: u32 = 16;
pub const RANGE_UPGRADE_USAGE: u32 = 8;
pub const CREATIVE_RANGE_UPGRADE_USAGE: u32 = 0;

/// A node's upgrades, as the charge cares about them: the four counts that
/// upstream's `UpgradeContainer.getEnergyUsage` asks each item for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fit {
    pub speed: u32,
    pub stack: u32,
    pub range: u32,
    pub creative_range: bool,
}

/// What one node charges: one tick for the always-on nodes (a grid, a drive, a
/// transmitter, a cable), one work pass for a machine (upstream's throttled
/// ticker calls `doWork` every interval, and `doWork` is where the charge is
/// taken). `disks` is how many storage disks a drive holds.
pub fn usage(kind: Kind, fit: Fit, disks: u32) -> u32 {
    match kind {
        Kind::Controller => CONTROLLER_USAGE,
        Kind::Cable => CABLE_USAGE,
        Kind::Grid => GRID_USAGE,
        Kind::DiskDrive => DISK_DRIVE_USAGE + DISK_DRIVE_PER_DISK_USAGE * disks,
        Kind::WirelessTransmitter => {
            let range = if fit.creative_range {
                CREATIVE_RANGE_UPGRADE_USAGE
            } else {
                RANGE_UPGRADE_USAGE * fit.range.min(MAX_RANGE_UPGRADES)
            };
            TRANSMITTER_USAGE + range
        }
        Kind::Importer => {
            IMPORTER_USAGE
                + SPEED_UPGRADE_USAGE * fit.speed.min(MAX_SPEED_UPGRADES)
                + STACK_UPGRADE_USAGE * fit.stack.min(MAX_STACK_UPGRADES)
        }
        Kind::Exporter => {
            EXPORTER_USAGE
                + SPEED_UPGRADE_USAGE * fit.speed.min(MAX_SPEED_UPGRADES)
                + STACK_UPGRADE_USAGE * fit.stack.min(MAX_STACK_UPGRADES)
        }
    }
}

/// What a network holds and what a node about to work needs, in FE. Upstream's
/// gate is `stored >= getEnergyUsage()` (`calculateActive`), and an active
/// node's `doWork` takes exactly that charge (`AbstractNetworkNode.doWork`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Energy {
    pub stored: u32,
    pub usage: u32,
}

impl Energy {
    pub fn new(stored: u32, usage: u32) -> Self {
        Energy { stored, usage }
    }

    /// Whether the node may work: upstream's `stored >= usage`.
    pub fn energized(self) -> bool {
        self.stored >= self.usage
    }

    /// What the network holds after one charge, or `None` when it cannot pay
    /// it (the node then does no work, and nothing is taken).
    pub fn draw(self) -> Option<u32> {
        self.energized().then(|| self.stored - self.usage)
    }
}

/// The controller's `energy_type`: upstream's four levels
/// (`ControllerEnergyType`), chosen from the stored/capacity ratio by
/// `ControllerNetworkNode.getState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnergyType {
    Off,
    NearlyOff,
    NearlyOn,
    On,
}

impl EnergyType {
    /// The level a buffer is at: `pct >= 0.4` on, `>= 0.3` nearly on, `>= 0.01`
    /// nearly off, else off. The comparisons are integer so the thresholds are
    /// exact (400, 300 and 10 of 1000).
    pub fn of(stored: u32, capacity: u32) -> EnergyType {
        if capacity == 0 {
            return EnergyType::Off;
        }
        let capacity = u64::from(capacity);
        let stored = u64::from(stored).min(capacity);
        if stored * 100 >= capacity * 40 {
            EnergyType::On
        } else if stored * 100 >= capacity * 30 {
            EnergyType::NearlyOn
        } else if stored * 100 >= capacity {
            EnergyType::NearlyOff
        } else {
            EnergyType::Off
        }
    }

    /// The value the block's `energy_type` state carries.
    pub fn property(self) -> &'static str {
        match self {
            EnergyType::Off => "off",
            EnergyType::NearlyOff => "nearly_off",
            EnergyType::NearlyOn => "nearly_on",
            EnergyType::On => "on",
        }
    }
}

/// A network's whole buffer: what each of its controllers holds. A controller
/// with no energy still counts as a controller (the network is structurally
/// online), so the sum, not the count, is what gates every node.
pub fn bank(stored: impl IntoIterator<Item = u32>) -> u32 {
    stored.into_iter().sum()
}

// --- the wireless grid's own buffer (slice 9) ------------------------------

/// What a wireless grid item holds, in FE (upstream
/// `DefaultEnergyUsage.WIRELESS_GRID_CAPACITY`): the item carries its own
/// buffer, charged before it is used, so a portable grid is not the network's
/// battery.
pub const WIRELESS_GRID_CAPACITY: u32 = 1000;

/// What one use of the wireless grid costs its own buffer (upstream
/// `DefaultEnergyUsage.WIRELESS_GRID_OPEN`, `WIRELESS_GRID_INSERT` and
/// `WIRELESS_GRID_EXTRACT`).
pub const WIRELESS_GRID_OPEN: u32 = 5;
pub const WIRELESS_GRID_INSERT: u32 = 5;
pub const WIRELESS_GRID_EXTRACT: u32 = 5;

/// The three things a wireless grid charges its own buffer for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WirelessUse {
    /// Opening the grid's view.
    Open,
    /// A deposit from the player into the network.
    Insert,
    /// A withdrawal from the network to the player.
    Extract,
}

impl WirelessUse {
    /// What this use costs: the reference's three constants.
    pub fn cost(self) -> u32 {
        match self {
            WirelessUse::Open => WIRELESS_GRID_OPEN,
            WirelessUse::Insert => WIRELESS_GRID_INSERT,
            WirelessUse::Extract => WIRELESS_GRID_EXTRACT,
        }
    }
}

/// The wireless grid's own buffer after `used` is paid for, or a refusal that
/// names the *item's* energy (upstream's `ItemEnergyStorage`), so a player can
/// tell an empty grid from an empty network.
pub fn charge_item(stored: u32, used: WirelessUse) -> Result<u32, Refusal> {
    let cost = used.cost();
    Energy::new(stored, cost)
        .draw()
        .ok_or(Refusal::NoItemEnergy { stored, cost })
}

// --- a broken block's own slots on its item (slice 10) ---------------------

/// A machine's filter entries: upstream's `ResourceContainerImpl.createForFilter`
/// makes a nine-slot container (`AbstractSimpleFilterContainerMenu.addSlots`
/// puts them first).
pub const MACHINE_FILTERS: usize = 9;

/// A machine's upgrade slots: upstream's `UpgradeContainer` default size.
pub const MACHINE_UPGRADES: usize = 4;

/// The transmitter's upgrade slots: the same `UpgradeContainer`, which its
/// `WirelessTransmitterBlockEntity` builds with no destination.
pub const TRANSMITTER_SLOTS: usize = MACHINE_UPGRADES;

impl Kind {
    /// Whether this block carries its own slots on its item when it is broken:
    /// the two machines (nine filter entries and four upgrades) and the
    /// transmitter (four upgrades) do. The controller, cable and grid have no
    /// slots, and a drive's nine slots are the player's disks — upstream drops
    /// those as items and keeps them off the drive's item, so the engine's own
    /// container spill stays the drive's route.
    pub fn carries_contents(self) -> bool {
        matches!(
            self,
            Kind::Importer | Kind::Exporter | Kind::WirelessTransmitter
        )
    }

    /// The slots this block's own container has, or zero for a block without
    /// one. A restore is clamped to it, so a doctored item cannot write into a
    /// slot the block does not have.
    pub fn own_slots(self) -> usize {
        match self {
            Kind::Importer | Kind::Exporter => MACHINE_FILTERS + MACHINE_UPGRADES,
            Kind::WirelessTransmitter => TRANSMITTER_SLOTS,
            Kind::Controller | Kind::Cable | Kind::DiskDrive | Kind::Grid => 0,
        }
    }
}

/// What a broken block carries on its item: every slot that held something,
/// with the slot index it held it in. `None` when the container was empty, so
/// a plain machine drops a plain machine item.
pub fn carry<T: Clone>(slots: &[Option<T>]) -> Option<Vec<(u32, T)>> {
    let held: Vec<(u32, T)> = slots
        .iter()
        .enumerate()
        .filter_map(|(slot, stack)| Some((u32::try_from(slot).ok()?, stack.clone()?)))
        .collect();
    (!held.is_empty()).then_some(held)
}

/// The slots a carried list may restore into a container of `capacity` slots:
/// an index at or past it is dropped (the item is a player's data, not the
/// container's definition), and a slot named twice keeps its first entry.
pub fn restore<T: Clone>(carried: &[(u32, T)], capacity: usize) -> Vec<(u32, T)> {
    let mut out: Vec<(u32, T)> = Vec::with_capacity(carried.len().min(capacity));
    for (slot, stack) in carried {
        if *slot as usize >= capacity {
            continue;
        }
        if out.iter().any(|(taken, _)| taken == slot) {
            continue;
        }
        out.push((*slot, stack.clone()));
    }
    out
}

/// What a placement does to the stack in the hand that placed the machine
/// (slice 13). A carried payload belongs to the **unit** it rides on, and a
/// placement spends exactly one unit's payload:
///
/// - in survival the engine takes the placed unit off the stack after the
///   placement hook, so the units it did not place keep their own payload and
///   the hand is not the port's to rewrite;
/// - in creative the engine keeps the stack, so a payload that has been spent
///   comes off — but only when the placed unit was the whole stack. A stack of
///   two is two machines' worth of contents, and stripping the whole stack
///   because one of them was placed destroys the other's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacedHand {
    /// The engine takes the placed unit: leave the rest, payload included.
    EngineConsumes,
    /// The engine keeps the stack and the placed unit was all of it: strip it.
    StripPlaced,
    /// The engine keeps the stack and the other units still carry their own.
    UnplacedKeep,
}

/// What a placement of `count` units does to the hand, `creative` or not.
pub fn placed_hand(creative: bool, count: u32) -> PlacedHand {
    match (creative, count) {
        (false, _) => PlacedHand::EngineConsumes,
        (true, 0 | 1) => PlacedHand::StripPlaced,
        (true, _) => PlacedHand::UnplacedKeep,
    }
}

/// The count a hand keeps after one item leaves it — a disk into a drive — or
/// `None` when that was the last one. The insertion and this decrement are one
/// transaction: a disk that went in without the hand losing one is a duplicate,
/// and a hand that lost one without the disk going in is a loss.
pub fn after_taking_one(count: u32) -> Option<u32> {
    count.checked_sub(1).filter(|left| *left > 0)
}

/// What a break does with a machine's own slots (slice 13). Only a break that
/// hands an item over can carry them; every other break — creative, a tool the
/// block refuses, a destroyed block whose loot is never built — must leave the
/// container alone, because the engine's own spill is then the only copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakPlan {
    /// Take the slots before the removal and put them on the dropped item.
    Carry,
    /// Leave them: the engine spills the container as vanilla does.
    Spill,
}

/// The break plan for a break that will (`true`) or will not drop loot.
pub fn break_plan(drops_loot: bool) -> BreakPlan {
    if drops_loot {
        BreakPlan::Carry
    } else {
        BreakPlan::Spill
    }
}

/// What `block_drops` does with the slots a `BreakPlan::Carry` took: they ride
/// the machine item when the break emitted one, and fall back to the ground as
/// the items they were when it did not. Neither answer loses them, and both
/// together cannot duplicate them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarryPlan {
    /// The machine item is in the drop list: the payload goes on it.
    OnTheItem,
    /// It is not: drop the slots themselves, as the engine's spill would.
    BackOnTheGround,
}

/// The carry plan for a drop list that does (`true`) or does not hold the item.
pub fn carry_plan(item_in_drops: bool) -> CarryPlan {
    if item_in_drops {
        CarryPlan::OnTheItem
    } else {
        CarryPlan::BackOnTheGround
    }
}

/// The operator level both energy writes need: charging a network's buffer or
/// a held item's buffer is an operator action (it stands in for the FE cable
/// and charger this pin's harness has none of).
pub const ENERGY_LEVEL: u8 = 2;

/// Whether a command source at `level` may write energy. Checked in the two
/// commands' bodies as well as declared on them, so the gate does not rest on
/// the dispatcher alone.
pub fn may_write_energy(level: u8) -> bool {
    level >= ENERGY_LEVEL
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test resource: `key` is the item id, counts free-form.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TestItem {
        id: &'static str,
        count: u32,
    }

    impl TestItem {
        fn new(id: &'static str, count: u32) -> Self {
            TestItem { id, count }
        }
    }

    impl Stored for TestItem {
        fn key(&self) -> String {
            self.id.to_owned()
        }
        fn count(&self) -> u32 {
            self.count
        }
        fn with_count(&self, count: u32) -> Self {
            TestItem { id: self.id, count }
        }
    }

    /// One stack of a test item holds 64.
    fn stack_of(_entry: &TestItem) -> u32 {
        64
    }

    fn disk_with(id: &'static str, count: u32) -> Disk<TestItem> {
        let mut disk = Disk::new(CAPACITY_1K);
        assert_eq!(disk.insert(TestItem::new(id, count)), count);
        disk
    }

    /// A full controller's worth of energy against a transmitter's charge: the
    /// power side of every admission that is not about energy.
    fn powered() -> Energy {
        Energy::new(CONTROLLER_CAPACITY, TRANSMITTER_USAGE)
    }

    // --- network -----------------------------------------------------------

    #[test]
    fn kind_maps_the_four_blocks_and_nothing_else() {
        assert_eq!(
            Kind::from_block("refinedstorage:controller"),
            Some(Kind::Controller)
        );
        assert_eq!(Kind::from_block("refinedstorage:cable"), Some(Kind::Cable));
        assert_eq!(
            Kind::from_block("refinedstorage:disk_drive"),
            Some(Kind::DiskDrive)
        );
        assert_eq!(Kind::from_block("refinedstorage:grid"), Some(Kind::Grid));
        assert_eq!(
            Kind::from_block("refinedstorage:wireless_transmitter"),
            Some(Kind::WirelessTransmitter)
        );
        assert_eq!(
            Kind::from_block("refinedstorage:importer"),
            Some(Kind::Importer)
        );
        assert_eq!(
            Kind::from_block("refinedstorage:exporter"),
            Some(Kind::Exporter)
        );
        assert_eq!(Kind::from_block("minecraft:stone"), None);
        assert_eq!(Kind::from_block("refinedstorage:destructor"), None);
        assert_eq!(Kind::Controller.block_id(), "refinedstorage:controller");
        assert_eq!(
            Kind::WirelessTransmitter.block_id(),
            "refinedstorage:wireless_transmitter"
        );
        assert_eq!(Kind::Importer.machine_name(), Some("Importer"));
        assert_eq!(Kind::Exporter.machine_name(), Some("Exporter"));
        assert_eq!(Kind::Cable.machine_name(), None);
    }

    #[test]
    fn a_controller_cable_drive_and_grid_are_one_online_network() {
        let nodes = [
            (Kind::Controller, [0, 64, 0]),
            (Kind::Cable, [1, 64, 0]),
            (Kind::DiskDrive, [2, 64, 0]),
            (Kind::Grid, [3, 64, 0]),
        ];
        let net = network(nodes[1].1, |pos| {
            nodes
                .iter()
                .find(|(_, at)| *at == pos)
                .map(|(kind, _)| Node::plain(*kind))
        });
        assert_eq!(net.nodes.len(), 4);
        assert!(net.online());
        assert_eq!(net.count(Kind::Cable), 1);
        assert_eq!(
            net.of_kind(Kind::DiskDrive).collect::<Vec<_>>(),
            vec![[2, 64, 0]]
        );
    }

    #[test]
    fn a_network_without_a_controller_is_offline() {
        let nodes = [
            (Kind::Cable, [0, 64, 0]),
            (Kind::DiskDrive, [1, 64, 0]),
            (Kind::Grid, [2, 64, 0]),
        ];
        let net = network([0, 64, 0], |pos| {
            nodes
                .iter()
                .find(|(_, at)| *at == pos)
                .map(|(kind, _)| Node::plain(*kind))
        });
        assert_eq!(net.nodes.len(), 3);
        assert!(!net.online(), "no controller means no network");
    }

    #[test]
    fn a_gap_or_a_diagonal_disconnects_a_network() {
        let nodes = [
            (Kind::Controller, [0, 64, 0]),
            (Kind::Grid, [2, 64, 0]),
            (Kind::DiskDrive, [1, 65, 0]),
        ];
        let at = |pos| {
            nodes
                .iter()
                .find(|(_, at)| *at == pos)
                .map(|(kind, _)| Node::plain(*kind))
        };
        let net = network([0, 64, 0], at);
        assert_eq!(net.nodes, vec![(Kind::Controller, [0, 64, 0])]);
        assert!(net.online());
        assert_eq!(
            net.count(Kind::Grid),
            0,
            "one block gap is not a connection"
        );
        let other = network([2, 64, 0], at);
        assert!(!other.online(), "a lone grid is its own offline network");
    }

    #[test]
    fn six_faces_are_the_neighbours() {
        assert_eq!(
            neighbours([0, 0, 0]),
            [
                [0, 0, -1],
                [1, 0, 0],
                [0, 0, 1],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0]
            ]
        );
    }

    #[test]
    fn the_flood_fill_stops_at_the_cap() {
        // A wall of `NETWORK_CAP + 8` cables: the fill reports the cut.
        let at = |pos: [i32; 3]| {
            (pos[1] == 0 && pos[2] == 0 && (0..NETWORK_CAP as i32 + 8).contains(&pos[0]))
                .then_some(Node::plain(Kind::Cable))
        };
        let net = network([0, 0, 0], at);
        assert_eq!(net.nodes.len(), NETWORK_CAP);
        assert!(net.truncated);
    }

    // --- the wireless transmitter ------------------------------------------

    /// A tiny world: the nodes it holds, by position.
    fn world(nodes: &[(Node, [i32; 3])]) -> impl FnMut([i32; 3]) -> Option<Node> + '_ {
        move |pos| {
            nodes
                .iter()
                .find(|(_, at)| *at == pos)
                .map(|(node, _)| *node)
        }
    }

    #[test]
    fn every_face_has_an_offset_an_opposite_and_a_property_name() {
        assert_eq!(Face::North.offset(), [0, 0, -1]);
        assert_eq!(Face::Up.offset(), [0, 1, 0]);
        assert_eq!(Face::Down.opposite(), Face::Up);
        assert_eq!(Face::East.opposite(), Face::West);
        for face in [
            Face::North,
            Face::East,
            Face::South,
            Face::West,
            Face::Up,
            Face::Down,
        ] {
            assert_eq!(face.opposite().opposite(), face);
            assert_eq!(
                Face::from_property(face.property()),
                Some(face),
                "a written property reads back as the same face"
            );
            assert!(neighbours([0, 0, 0]).contains(&face.offset()));
        }
        assert_eq!(Face::from_property("middle"), None);
    }

    #[test]
    fn a_transmitter_joins_the_network_through_its_facing_side_only() {
        // controller 0 - cable 1 - transmitter 2, the cable west of it.
        let nodes = [
            (Node::plain(Kind::Controller), [0, 64, 0]),
            (Node::plain(Kind::Cable), [1, 64, 0]),
        ];
        // Facing the cable: one online network of three.
        let mut facing_cable = nodes.to_vec();
        facing_cable.push((
            Node::facing(Kind::WirelessTransmitter, Face::West),
            [2, 64, 0],
        ));
        let net = network([2, 64, 0], world(&facing_cable));
        assert_eq!(net.nodes.len(), 3);
        assert!(
            net.online(),
            "facing the cable joins the controller's network"
        );

        // Facing up: nothing on its face, so it is its own offline component and
        // the cable's component does not reach it either.
        let mut facing_away = nodes.to_vec();
        facing_away.push((
            Node::facing(Kind::WirelessTransmitter, Face::Up),
            [2, 64, 0],
        ));
        let tx = network([2, 64, 0], world(&facing_away));
        assert_eq!(
            tx.nodes,
            vec![(Kind::WirelessTransmitter, [2, 64, 0])],
            "a transmitter that faces nothing is alone"
        );
        assert!(!tx.online());
        let cable = network([1, 64, 0], world(&facing_away));
        assert_eq!(
            cable.nodes.len(),
            2,
            "the cable does not reach it from behind"
        );
        assert!(cable.online());
    }

    #[test]
    fn a_transmitter_takes_the_side_it_faces_when_the_neighbour_is_the_one_asking() {
        // Two transmitters facing each other: each is the other's facing
        // neighbour, so they are one component of their own — and the cable
        // behind the first (which it faces away from) does not reach either.
        let nodes = [
            (Node::plain(Kind::Controller), [0, 64, 0]),
            (Node::plain(Kind::Cable), [1, 64, 0]),
            (
                Node::facing(Kind::WirelessTransmitter, Face::East),
                [2, 64, 0],
            ),
            (
                Node::facing(Kind::WirelessTransmitter, Face::West),
                [3, 64, 0],
            ),
        ];
        let pair = network([2, 64, 0], world(&nodes));
        assert_eq!(pair.nodes.len(), 2);
        assert_eq!(pair.count(Kind::WirelessTransmitter), 2);
        assert!(!pair.online(), "no controller among them");
        let cable = network([1, 64, 0], world(&nodes));
        assert_eq!(
            cable.nodes.len(),
            2,
            "the cable's network is the controller and itself"
        );
        assert!(cable.online());
    }

    #[test]
    fn a_use_in_another_dimension_is_refused_as_such_even_when_offline() {
        assert_eq!(
            admit(false, false, powered(), 0.0, WIRELESS_RANGE),
            Err(Refusal::DifferentDimension),
            "upstream checks the dimension before the node's activeness"
        );
        assert_eq!(
            admit(true, false, powered(), 1.0, WIRELESS_RANGE),
            Err(Refusal::DifferentDimension)
        );
    }

    #[test]
    fn an_offline_transmitter_refuses_a_use_within_its_range() {
        assert_eq!(
            admit(false, true, powered(), 1.0, WIRELESS_RANGE),
            Err(Refusal::Offline)
        );
    }

    #[test]
    fn a_use_beyond_the_range_is_refused_naming_the_distance() {
        assert_eq!(
            admit(true, true, powered(), 16.5, WIRELESS_RANGE),
            Err(Refusal::OutOfRange {
                distance: 16.5,
                range: WIRELESS_RANGE
            })
        );
        assert_eq!(
            Refusal::OutOfRange {
                distance: 16.5,
                range: WIRELESS_RANGE
            }
            .to_string(),
            "out of range: 16.5 > 16 blocks"
        );
    }

    #[test]
    fn the_range_boundary_itself_is_accepted() {
        assert_eq!(admit(true, true, powered(), 16.0, WIRELESS_RANGE), Ok(()));
        assert!(admit(true, true, powered(), 16.000_1, WIRELESS_RANGE).is_err());
        assert_eq!(admit(true, true, powered(), 0.0, WIRELESS_RANGE), Ok(()));
    }

    #[test]
    fn the_distance_is_measured_from_the_transmitter_block_to_the_user() {
        // 1.5 blocks east, 1.5 blocks south of the block's corner: sqrt(2.25 + 2.25).
        let d = distance([42.5, 64.0, 2.5], [41, 64, 1]);
        assert!((d - 2.121_320_3).abs() < 1e-6, "distance was {d}");
        assert_eq!(distance([41.0, 64.0, 1.0], [41, 64, 1]), 0.0);
    }

    // --- the upgrade items and the range they raise (slice 6) --------------

    #[test]
    fn a_transmitter_without_upgrades_keeps_the_base_range() {
        assert_eq!(upgraded_range(0, false), WIRELESS_RANGE);
    }

    #[test]
    fn each_range_upgrade_adds_eight_blocks() {
        // upstream `RangeUpgradeWirelessTransmitterRangeModifier.modifyRange`:
        // `range + amountOfRangeUpgrades * rangeUpgradeRange`, with 8 per upgrade.
        assert_eq!(upgraded_range(1, false), 24);
        assert_eq!(upgraded_range(2, false), 32);
        assert_eq!(upgraded_range(4, false), 48);
    }

    #[test]
    fn a_fifth_range_upgrade_is_not_counted() {
        // upstream registers the range upgrade for a transmitter with maxAmount 4
        // and its `UpgradeContainer.canPlaceItem` refuses the fifth.
        assert_eq!(upgraded_range(5, false), 48);
        assert_eq!(upgraded_range(40, false), 48);
    }

    #[test]
    fn a_creative_range_upgrade_reaches_any_finite_distance() {
        let range = upgraded_range(0, true);
        assert_eq!(range, CREATIVE_RANGE);
        assert_eq!(admit(true, true, powered(), 1_000_000.0, range), Ok(()));
        assert_eq!(
            upgraded_range(4, true),
            CREATIVE_RANGE,
            "the creative upgrade has the highest priority and wins over the range ones"
        );
    }

    #[test]
    fn a_creative_range_upgrade_still_refuses_another_dimension() {
        // `WirelessTransmitterNetworkNodeContainer.isValid` checks the dimension
        // before the range, so the creative upgrade cannot cross one.
        assert_eq!(
            admit(true, false, powered(), 1.0, upgraded_range(0, true)),
            Err(Refusal::DifferentDimension)
        );
    }

    #[test]
    fn a_raised_range_accepts_what_the_base_range_refused() {
        // the differential the in-game leg uses: the same 17.0-block use.
        assert_eq!(
            admit(true, true, powered(), 17.0, upgraded_range(0, false)),
            Err(Refusal::OutOfRange {
                distance: 17.0,
                range: 16
            })
        );
        assert_eq!(
            admit(true, true, powered(), 17.0, upgraded_range(1, false)),
            Ok(())
        );
        assert_eq!(
            admit(true, true, powered(), 48.0, upgraded_range(4, false)),
            Ok(())
        );
        assert!(admit(true, true, powered(), 48.1, upgraded_range(4, false)).is_err());
    }

    #[test]
    fn a_range_is_named_in_blocks_unless_it_is_the_creative_one() {
        assert_eq!(range_label(upgraded_range(1, false)), "24 blocks");
        assert_eq!(range_label(WIRELESS_RANGE), "16 blocks");
        assert_eq!(range_label(upgraded_range(0, true)), "unlimited");
    }

    // --- the wireless grid's binding and its menu (slice 4) ----------------

    #[test]
    fn a_binding_names_a_dimension_and_a_position() {
        let binding = Binding::new("minecraft:the_end", [54, 64, 4]);
        assert_eq!(binding.dimension(), "minecraft:the_end");
        assert_eq!(binding.pos(), [54, 64, 4]);
        assert!(
            binding.describes("minecraft:the_end", [54, 64, 4]),
            "the binding describes the transmitter it was made on"
        );
        assert!(
            !binding.describes("minecraft:overworld", [54, 64, 4]),
            "the same coordinates in another dimension are another transmitter"
        );
        assert!(
            !binding.describes("minecraft:the_end", [54, 64, 5]),
            "the position is part of the binding"
        );
    }

    #[test]
    fn an_entire_extraction_is_capped_by_the_stack_size() {
        assert_eq!(extract_amount(5, 64, ExtractMode::Entire), 5);
        assert_eq!(extract_amount(200, 64, ExtractMode::Entire), 64);
        assert_eq!(extract_amount(0, 64, ExtractMode::Entire), 0);
    }

    #[test]
    fn a_half_extraction_is_floor_halved_and_one_item_stays_one() {
        // Upstream: `extractableAmount == 1 ? 1 : extractableAmount / 2`.
        assert_eq!(extract_amount(5, 64, ExtractMode::Half), 2);
        assert_eq!(extract_amount(4, 64, ExtractMode::Half), 2);
        assert_eq!(extract_amount(1, 64, ExtractMode::Half), 1);
        assert_eq!(extract_amount(200, 64, ExtractMode::Half), 32);
        assert_eq!(extract_amount(0, 64, ExtractMode::Half), 0);
    }

    #[test]
    fn a_single_extraction_takes_one() {
        assert_eq!(extract_amount(5, 64, ExtractMode::Single), 1);
        assert_eq!(extract_amount(0, 64, ExtractMode::Single), 0);
    }

    #[test]
    fn a_deposit_moves_a_whole_stack_or_one_item() {
        assert_eq!(insert_amount(64, ExtractMode::Entire), 64);
        assert_eq!(insert_amount(64, ExtractMode::Single), 1);
        assert_eq!(insert_amount(16, ExtractMode::Entire), 16);
    }

    #[test]
    fn a_left_click_on_a_view_slot_withdraws_the_whole_stack_to_the_cursor() {
        assert_eq!(
            plan_click(Region::View, true, Cursor::Empty, 0, false),
            Plan::Withdraw {
                mode: ExtractMode::Entire,
                to: Destination::Cursor
            }
        );
    }

    #[test]
    fn a_right_click_on_a_view_slot_withdraws_half() {
        assert_eq!(
            plan_click(Region::View, true, Cursor::Empty, 1, false),
            Plan::Withdraw {
                mode: ExtractMode::Half,
                to: Destination::Cursor
            }
        );
    }

    #[test]
    fn a_shift_click_on_a_view_slot_withdraws_to_the_inventory() {
        assert_eq!(
            plan_click(Region::View, true, Cursor::Empty, 0, true),
            Plan::Withdraw {
                mode: ExtractMode::Entire,
                to: Destination::Inventory
            }
        );
        // The cursor is ignored: a quick move always goes to the inventory.
        assert_eq!(
            plan_click(Region::View, true, Cursor::Same { room: 10 }, 1, true),
            Plan::Withdraw {
                mode: ExtractMode::Entire,
                to: Destination::Inventory
            }
        );
    }

    #[test]
    fn a_cursor_that_can_take_the_resource_withdraws_into_it() {
        assert_eq!(
            plan_click(Region::View, true, Cursor::Same { room: 10 }, 0, false),
            Plan::Withdraw {
                mode: ExtractMode::Entire,
                to: Destination::Cursor
            }
        );
    }

    #[test]
    fn a_cursor_with_another_resource_deposits_instead() {
        assert_eq!(
            plan_click(Region::View, true, Cursor::Other, 0, false),
            Plan::Deposit {
                mode: ExtractMode::Entire
            }
        );
        assert_eq!(
            plan_click(Region::View, true, Cursor::Other, 1, false),
            Plan::Deposit {
                mode: ExtractMode::Single
            }
        );
    }

    #[test]
    fn a_cursor_that_is_full_of_the_resource_deposits_instead() {
        assert_eq!(
            plan_click(Region::View, true, Cursor::Same { room: 0 }, 0, false),
            Plan::Deposit {
                mode: ExtractMode::Entire
            }
        );
    }

    #[test]
    fn an_empty_cursor_over_an_empty_slot_moves_nothing() {
        assert_eq!(
            plan_click(Region::View, false, Cursor::Empty, 0, false),
            Plan::Nothing
        );
        assert_eq!(
            plan_click(Region::View, false, Cursor::Empty, 1, false),
            Plan::Nothing
        );
    }

    #[test]
    fn an_empty_view_slot_takes_a_cursor_as_a_deposit() {
        assert_eq!(
            plan_click(Region::View, false, Cursor::Other, 0, false),
            Plan::Deposit {
                mode: ExtractMode::Entire
            }
        );
    }

    #[test]
    fn a_shift_click_in_the_inventory_deposits_that_stack() {
        assert_eq!(
            plan_click(Region::Inventory, true, Cursor::Empty, 0, true),
            Plan::Deposit {
                mode: ExtractMode::Entire
            }
        );
        assert_eq!(
            plan_click(Region::Inventory, false, Cursor::Empty, 0, true),
            Plan::Nothing
        );
    }

    #[test]
    fn an_plain_inventory_click_is_the_cursor_rules() {
        assert_eq!(
            plan_click(Region::Inventory, true, Cursor::Empty, 0, false),
            Plan::Inventory(CursorMove::Take { half: false })
        );
        assert_eq!(
            plan_click(Region::Inventory, true, Cursor::Empty, 1, false),
            Plan::Inventory(CursorMove::Take { half: true })
        );
        assert_eq!(
            plan_click(Region::Inventory, false, Cursor::Same { room: 5 }, 0, false),
            Plan::Inventory(CursorMove::Place { one: false })
        );
        assert_eq!(
            plan_click(Region::Inventory, false, Cursor::Other, 1, false),
            Plan::Inventory(CursorMove::Place { one: true })
        );
        assert_eq!(
            plan_click(Region::Inventory, true, Cursor::Same { room: 5 }, 0, false),
            Plan::Inventory(CursorMove::Merge { one: false })
        );
        assert_eq!(
            plan_click(Region::Inventory, true, Cursor::Same { room: 5 }, 1, false),
            Plan::Inventory(CursorMove::Merge { one: true })
        );
        assert_eq!(
            plan_click(Region::Inventory, true, Cursor::Other, 0, false),
            Plan::Inventory(CursorMove::Swap)
        );
        assert_eq!(
            plan_click(Region::Inventory, false, Cursor::Empty, 0, false),
            Plan::Inventory(CursorMove::Nothing)
        );
    }

    // --- one disk ----------------------------------------------------------

    #[test]
    fn every_storage_disk_id_names_its_capacity() {
        assert_eq!(
            disk_capacity("refinedstorage:1k_storage_disk"),
            Some(DiskCapacity::Items(1024))
        );
        assert_eq!(
            disk_capacity("refinedstorage:4k_storage_disk"),
            Some(DiskCapacity::Items(4096))
        );
        assert_eq!(
            disk_capacity("refinedstorage:16k_storage_disk"),
            Some(DiskCapacity::Items(16384))
        );
        assert_eq!(
            disk_capacity("refinedstorage:64k_storage_disk"),
            Some(DiskCapacity::Items(65536))
        );
        assert_eq!(
            disk_capacity("refinedstorage:creative_storage_disk"),
            Some(DiskCapacity::Unlimited)
        );
        assert_eq!(
            disk_capacity("refinedstorage:storage_housing"),
            None,
            "an ingredient is not a storage disk"
        );
        assert_eq!(disk_capacity("minecraft:paper"), None);
    }

    #[test]
    fn a_4k_disk_takes_four_times_what_a_1k_does() {
        let mut disk = Disk::<TestItem>::new(CAPACITY_4K);
        assert_eq!(disk.free(), 4096);
        assert_eq!(
            disk.insert(TestItem::new("minecraft:cobblestone", 4096)),
            4096
        );
        assert_eq!(disk.total(), 4096);
        assert_eq!(disk.free(), 0);
        assert_eq!(
            disk.insert(TestItem::new("minecraft:dirt", 1)),
            0,
            "a full 4k disk refuses the 4,097th item"
        );
        assert_eq!(disk.amount("minecraft:cobblestone"), 4096);
    }

    #[test]
    fn the_creative_disk_has_no_ceiling() {
        let mut disk = Disk::<TestItem>::unlimited();
        assert_eq!(disk.capacity(), DiskCapacity::Unlimited);
        assert_eq!(disk.free(), u64::MAX, "unlimited means no room count");
        assert_eq!(
            disk.insert(TestItem::new("minecraft:cobblestone", 65536)),
            65536
        );
        assert_eq!(
            disk.insert(TestItem::new("minecraft:cobblestone", 65536)),
            65536,
            "the creative disk keeps ingesting past the 64k ceiling"
        );
        assert_eq!(disk.total(), 131072);
    }

    #[test]
    fn a_saved_fuller_than_capacity_disk_is_cut_to_its_own_room() {
        let mut disk = Disk::<TestItem>::new(CAPACITY_4K);
        disk.set_entries(vec![TestItem::new("minecraft:cobblestone", 5000)]);
        assert_eq!(
            disk.total(),
            4096,
            "a 4k disk is cut to 4,096, not to the 1k default"
        );
    }

    #[test]
    fn the_saved_spelling_marks_the_creative_disk_zero() {
        assert_eq!(DiskCapacity::from_saved(4096), DiskCapacity::Items(4096));
        assert_eq!(
            DiskCapacity::from_saved(0),
            DiskCapacity::Unlimited,
            "0 is the only spelling upstream's null has in a component"
        );
        assert_eq!(DiskCapacity::Items(65536).saved(), 65536);
        assert_eq!(DiskCapacity::Unlimited.saved(), 0);
    }

    #[test]
    fn a_component_cannot_give_a_disk_more_room_than_its_item() {
        assert_eq!(
            DiskCapacity::Items(4096).clamp(DiskCapacity::Items(65536)),
            DiskCapacity::Items(4096),
            "the item's own capacity is the limit"
        );
        assert_eq!(
            DiskCapacity::Items(65536).clamp(DiskCapacity::Items(4096)),
            DiskCapacity::Items(4096)
        );
        assert_eq!(
            DiskCapacity::Unlimited.clamp(DiskCapacity::Items(4096)),
            DiskCapacity::Items(4096),
            "a 1k item's component cannot claim to be creative"
        );
        assert_eq!(
            DiskCapacity::Unlimited.clamp(DiskCapacity::Unlimited),
            DiskCapacity::Unlimited
        );
        assert_eq!(
            DiskCapacity::Items(4096).clamp(DiskCapacity::Unlimited),
            DiskCapacity::Items(4096)
        );
    }

    #[test]
    fn a_capacity_reads_back_as_its_room() {
        assert_eq!(DiskCapacity::Items(4096).label(), "4096 items");
        assert_eq!(DiskCapacity::Items(65536).label(), "65536 items");
        assert_eq!(DiskCapacity::Unlimited.label(), "unlimited");
    }

    // --- the in-place upgrade (slice 12) -----------------------------------

    #[test]
    fn every_storage_disk_names_its_own_part() {
        assert_eq!(
            storage_part_of_disk("refinedstorage:1k_storage_disk"),
            Some("refinedstorage:1k_storage_part")
        );
        assert_eq!(
            storage_part_of_disk("refinedstorage:4k_storage_disk"),
            Some("refinedstorage:4k_storage_part")
        );
        assert_eq!(
            storage_part_of_disk("refinedstorage:16k_storage_disk"),
            Some("refinedstorage:16k_storage_part")
        );
        assert_eq!(
            storage_part_of_disk("refinedstorage:64k_storage_disk"),
            Some("refinedstorage:64k_storage_part")
        );
        assert_eq!(
            storage_part_of_disk("refinedstorage:creative_storage_disk"),
            None,
            "upstream's creative variant has no storage part"
        );
        assert_eq!(storage_part_of_disk("refinedstorage:storage_housing"), None);
        assert_eq!(
            disk_of_variant("16k"),
            Some("refinedstorage:16k_storage_disk")
        );
        assert_eq!(
            disk_of_variant("creative"),
            Some("refinedstorage:creative_storage_disk")
        );
        assert_eq!(disk_of_variant("2k"), None);
        assert_eq!(
            disk_variant("refinedstorage:64k_storage_disk").map(|variant| variant.name),
            Some("64k")
        );
    }

    #[test]
    fn an_upgrade_takes_only_the_smaller_disks() {
        assert_eq!(
            upgrade_sources("refinedstorage:4k_storage_disk"),
            vec!["refinedstorage:1k_storage_disk"]
        );
        assert_eq!(
            upgrade_sources("refinedstorage:16k_storage_disk"),
            vec![
                "refinedstorage:1k_storage_disk",
                "refinedstorage:4k_storage_disk"
            ]
        );
        assert_eq!(
            upgrade_sources("refinedstorage:64k_storage_disk"),
            vec![
                "refinedstorage:1k_storage_disk",
                "refinedstorage:4k_storage_disk",
                "refinedstorage:16k_storage_disk"
            ]
        );
        assert!(
            upgrade_sources("refinedstorage:creative_storage_disk").is_empty(),
            "upstream returns an empty source set for a destination with no capacity"
        );
        assert!(upgrade_sources("minecraft:stick").is_empty());
    }

    #[test]
    fn an_upgrade_grid_is_exactly_one_disk_and_its_part() {
        let to = "refinedstorage:16k_storage_disk";
        let mut cells: [Option<&str>; 9] = [None; 9];
        cells[3] = Some("refinedstorage:4k_storage_disk");
        cells[4] = Some("refinedstorage:16k_storage_part");
        assert_eq!(
            upgrade_cells(&cells, to),
            Some(UpgradeGrid { disk: 3, part: 4 })
        );
        // A shapeless pair: any two cells, either order.
        let mut reversed: [Option<&str>; 9] = [None; 9];
        reversed[0] = Some("refinedstorage:16k_storage_part");
        reversed[8] = Some("refinedstorage:4k_storage_disk");
        assert_eq!(
            upgrade_cells(&reversed, to),
            Some(UpgradeGrid { disk: 8, part: 0 })
        );
        // A third item is not part of a two-ingredient recipe.
        cells[5] = Some("minecraft:diamond");
        assert_eq!(
            upgrade_cells(&cells, to),
            None,
            "a shapeless recipe of two ingredients accepts no third item"
        );
        cells[5] = None;
        // The destination's own size is not smaller, so it is no source.
        cells[3] = Some("refinedstorage:16k_storage_disk");
        assert_eq!(upgrade_cells(&cells, to), None);
        // Another tier's part is not this recipe's part.
        cells[3] = Some("refinedstorage:4k_storage_disk");
        cells[4] = Some("refinedstorage:64k_storage_part");
        assert_eq!(upgrade_cells(&cells, to), None);
        // Two source disks are not one.
        cells[4] = Some("refinedstorage:16k_storage_part");
        cells[6] = Some("refinedstorage:1k_storage_disk");
        assert_eq!(upgrade_cells(&cells, to), None);
        let empty: [Option<&str>; 9] = [None; 9];
        assert_eq!(upgrade_cells(&empty, to), None);
        assert_eq!(
            upgrade_cells(&cells, "refinedstorage:creative_storage_disk"),
            None,
            "the creative disk is never a destination: it has no capacity to grow into"
        );
    }

    #[test]
    fn an_upgraded_disk_keeps_its_contents_with_the_new_room() {
        let mut source = Disk::<TestItem>::new(CAPACITY_4K);
        assert_eq!(
            source.insert(TestItem::new("minecraft:cobblestone", 4096)),
            4096
        );
        let upgraded = upgraded_disk(&source, "refinedstorage:16k_storage_disk")
            .expect("a 4k disk upgrades into a 16k one");
        assert_eq!(upgraded.capacity(), DiskCapacity::Items(CAPACITY_16K));
        assert_eq!(upgraded.total(), 4096, "the contents came across");
        assert_eq!(
            upgraded.free(),
            12288,
            "the destination item's room, not the source's"
        );
        assert_eq!(upgraded.amount("minecraft:cobblestone"), 4096);
        let empty = upgraded_disk(
            &Disk::<TestItem>::new(CAPACITY_1K),
            "refinedstorage:64k_storage_disk",
        )
        .expect("an empty 1k disk upgrades into a 64k one");
        assert_eq!(empty.capacity(), DiskCapacity::Items(CAPACITY_64K));
        assert_eq!(empty.total(), 0);
        assert!(empty.entries().is_empty());
    }

    #[test]
    fn an_upgrade_refuses_a_disk_that_is_not_bigger() {
        let source = Disk::<TestItem>::new(CAPACITY_4K);
        assert_eq!(
            upgraded_disk(&source, "refinedstorage:4k_storage_disk"),
            None,
            "a disk does not upgrade into its own size"
        );
        assert_eq!(
            upgraded_disk(&source, "refinedstorage:1k_storage_disk"),
            None,
            "nor into a smaller one"
        );
        assert_eq!(
            upgraded_disk(&source, "refinedstorage:creative_storage_disk"),
            None,
            "the creative disk is not a destination: no capacity to grow into"
        );
        assert_eq!(upgraded_disk(&source, "minecraft:stick"), None);
        assert_eq!(
            upgraded_disk(
                &Disk::<TestItem>::unlimited(),
                "refinedstorage:64k_storage_disk"
            ),
            None,
            "the creative disk is never a source either"
        );
    }

    #[test]
    fn a_disk_takes_items_up_to_its_capacity() {
        let mut disk = Disk::<TestItem>::new(CAPACITY_1K);
        assert_eq!(disk.free(), 1024);
        assert_eq!(disk.insert(TestItem::new("minecraft:stone", 1000)), 1000);
        assert_eq!(disk.total(), 1000);
        let taken = disk.insert(TestItem::new("minecraft:dirt", 64));
        assert_eq!(taken, 24, "1024 - 1000 leaves 24 free");
        assert_eq!(disk.total(), 1024);
        assert_eq!(disk.free(), 0);
        assert_eq!(disk.amount("minecraft:stone"), 1000);
        assert_eq!(disk.amount("minecraft:dirt"), 24);
    }

    #[test]
    fn one_resource_accumulates_and_empty_entries_disappear() {
        let mut disk = disk_with("minecraft:diamond", 10);
        assert_eq!(disk.insert(TestItem::new("minecraft:diamond", 5)), 5);
        assert_eq!(disk.entries().len(), 1);
        assert_eq!(disk.amount("minecraft:diamond"), 15);
        let taken = disk.extract("minecraft:diamond", 15).expect("held");
        assert_eq!(taken.count(), 15);
        assert!(disk.entries().is_empty(), "an emptied resource is gone");
        assert!(disk.extract("minecraft:diamond", 1).is_none());
    }

    #[test]
    fn extraction_is_partial_when_the_disk_is_short() {
        let mut disk = disk_with("minecraft:diamond", 7);
        let taken = disk.extract("minecraft:diamond", 64).expect("held");
        assert_eq!(taken.count(), 7);
        assert!(disk.entries().is_empty());
    }

    #[test]
    fn a_disk_keeps_a_bounded_number_of_resources() {
        let mut disk = Disk::<TestItem>::new(CAPACITY_1K);
        for index in 0..MAX_RESOURCES {
            let id: &'static str = Box::leak(format!("test:item{index}").into_boxed_str());
            assert_eq!(disk.insert(TestItem::new(id, 1)), 1);
        }
        assert_eq!(disk.entries().len(), MAX_RESOURCES);
        assert_eq!(
            disk.insert(TestItem::new("test:one_too_many", 1)),
            0,
            "a full disk refuses a new resource"
        );
    }

    #[test]
    fn a_saved_disk_never_exceeds_its_capacity() {
        let mut disk = Disk::<TestItem>::new(16);
        disk.set_entries(vec![
            TestItem::new("minecraft:stone", 10),
            TestItem::new("minecraft:dirt", 10),
            TestItem::new("minecraft:air", 0),
        ]);
        assert_eq!(
            disk.total(),
            16,
            "a hostile item component is cut to capacity"
        );
        assert_eq!(disk.entries().len(), 2);
    }

    // --- a network of disks ------------------------------------------------

    /// A resource whose stack cannot hold zero items, exactly as the game's
    /// `ItemStack` cannot (`set_count(0)` answers `Empty` and leaves the count
    /// alone). The runtime must never build a zero stack to say "nothing is
    /// left over": the amount a disk took is a count, not a stack.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ClampedItem {
        id: &'static str,
        count: u32,
    }

    impl Stored for ClampedItem {
        fn key(&self) -> String {
            self.id.to_owned()
        }

        fn count(&self) -> u32 {
            self.count
        }

        fn with_count(&self, count: u32) -> Self {
            ClampedItem {
                id: self.id,
                count: count.max(1),
            }
        }
    }

    #[test]
    fn a_stack_the_disk_took_entirely_leaves_nothing_over() {
        let mut disks = [Disk::new(CAPACITY_1K)];
        let left = insert(
            &mut disks,
            ClampedItem {
                id: "minecraft:dirt",
                count: 64,
            },
        );
        assert!(left.is_none(), "a deposit the disk took leaves nothing");
        assert_eq!(disks[0].total(), 64);
    }

    #[test]
    fn inserting_uses_one_disk_that_fits_the_whole_stack() {
        let mut disks = [disk_with("minecraft:stone", 1000), Disk::new(CAPACITY_1K)];
        let leftover = insert(&mut disks, TestItem::new("minecraft:dirt", 64));
        assert!(leftover.is_none());
        assert_eq!(
            disks[0].amount("minecraft:dirt"),
            0,
            "the full disk is skipped"
        );
        assert_eq!(disks[1].amount("minecraft:dirt"), 64);
    }

    #[test]
    fn inserting_refuses_when_no_disk_has_room() {
        let mut disks = [disk_with("minecraft:stone", CAPACITY_1K as u32)];
        let leftover = insert(&mut disks, TestItem::new("minecraft:dirt", 1));
        assert_eq!(
            leftover.expect("refused").count(),
            1,
            "a full network answers the whole stack"
        );
        assert_eq!(disks[0].total(), 1024);
    }

    #[test]
    fn extraction_crosses_disks_and_stops_when_short() {
        let mut disks = [
            disk_with("minecraft:diamond", 10),
            disk_with("minecraft:diamond", 10),
        ];
        let taken = extract(&mut disks, "minecraft:diamond", 64).expect("held");
        assert_eq!(taken.count(), 20);
        assert_eq!(amount(&disks, "minecraft:diamond"), 0);
        assert!(extract(&mut disks, "minecraft:diamond", 1).is_none());
    }

    #[test]
    fn aggregation_sums_a_resource_over_disks() {
        let disks = [
            disk_with("minecraft:diamond", 10),
            disk_with("minecraft:diamond", 5),
            disk_with("minecraft:gold_ingot", 3),
        ];
        let mut all = aggregate(&disks);
        all.sort_by_key(|entry| entry.id);
        assert_eq!(
            all,
            vec![
                TestItem::new("minecraft:diamond", 15),
                TestItem::new("minecraft:gold_ingot", 3)
            ]
        );
        assert_eq!(amount(&disks, "minecraft:diamond"), 15);
    }

    // --- the grid's view ---------------------------------------------------

    #[test]
    fn the_view_sorts_by_amount_and_then_id() {
        let disks = [
            disk_with("minecraft:diamond", 10),
            disk_with("minecraft:gold_ingot", 10),
            disk_with("minecraft:dirt", 5),
        ];
        let view = view(&disks, "", GRID_SLOTS, stack_of);
        assert_eq!(
            view,
            vec![
                TestItem::new("minecraft:dirt", 5),
                TestItem::new("minecraft:diamond", 10),
                TestItem::new("minecraft:gold_ingot", 10)
            ]
        );
    }

    #[test]
    fn a_second_grid_cannot_show_what_the_first_already_shows() {
        // Two grids on one network: the first shows all 10 diamonds.
        let disks = [disk_with("minecraft:diamond", 10)];
        let held = [("minecraft:diamond".to_string(), 10)];
        assert!(
            view_with(&disks, &held, "", GRID_SLOTS, stack_of).is_empty(),
            "nothing is left to promise the second grid"
        );
        // A partial promise leaves the rest visible.
        let held = [("minecraft:diamond".to_string(), 4)];
        assert_eq!(
            view_with(&disks, &held, "", GRID_SLOTS, stack_of),
            vec![TestItem::new("minecraft:diamond", 6)]
        );
        // Another resource's promise does not touch this one.
        let held = [("minecraft:dirt".to_string(), 64)];
        assert_eq!(
            view_with(&disks, &held, "", GRID_SLOTS, stack_of),
            vec![TestItem::new("minecraft:diamond", 10)]
        );
    }

    // --- the promise table -------------------------------------------------

    /// A view key in the End, showing the network whose controller is at
    /// `network`.
    fn place_at(network: [i32; 3], pos: [i32; 3]) -> ViewKey {
        ViewKey::new("minecraft:the_end", Some(network), Place::Grid(pos))
    }

    #[test]
    fn two_grids_of_one_network_never_show_the_same_items() {
        let gold = vec![("minecraft:gold_ingot".to_string(), 3)];
        let mut promises = Promises::default();
        promises.record(place_at([40, 64, 0], [43, 64, 0]), gold.clone());
        assert_eq!(
            promises.promised_elsewhere(&place_at([40, 64, 0], [42, 65, 0])),
            gold,
            "the second grid of the same network sets the first one's 3 gold aside"
        );
        assert_eq!(
            promises.promised_elsewhere(&ViewKey::new(
                "minecraft:overworld",
                Some([40, 64, 0]),
                Place::Grid([43, 64, 0])
            )),
            Vec::new(),
            "another dimension's grid is untouched"
        );
    }

    #[test]
    fn two_networks_in_one_dimension_do_not_hide_each_others_stock() {
        // Network A's grid at 43,64,0 (controller 40,64,0) and network B's grid
        // at 50,64,0 (controller 48,64,0), both in the End and both holding 3
        // gold: each must show its own 3.
        let gold = vec![("minecraft:gold_ingot".to_string(), 3)];
        let mut promises = Promises::default();
        promises.record(place_at([40, 64, 0], [43, 64, 0]), gold.clone());
        assert_eq!(
            promises.promised_elsewhere(&place_at([48, 64, 0], [50, 64, 0])),
            Vec::new(),
            "a grid of another network in the same dimension shows its own 3 gold"
        );
        // The same grid asked again still sees what the other views of *its*
        // network promise, so the scoping did not drop the rule.
        promises.record(place_at([48, 64, 0], [51, 65, 0]), gold.clone());
        assert_eq!(
            promises.promised_elsewhere(&place_at([48, 64, 0], [50, 64, 0])),
            gold,
            "network B's first grid sets the second one's 3 gold aside"
        );
        assert_eq!(
            promises.promised_elsewhere(&place_at([40, 64, 0], [43, 64, 0])),
            Vec::new(),
            "and network A's grid is still unaffected by B's two views"
        );
    }

    #[test]
    fn a_networks_root_is_its_controller_wherever_the_fill_started() {
        // controller - cable - grid, entered from either end.
        let at = |pos: [i32; 3]| match pos {
            [0, 0, 0] => Some(Node::plain(Kind::Controller)),
            [1, 0, 0] | [2, 0, 0] => Some(Node::plain(Kind::Cable)),
            _ => None,
        };
        let left = network([0, 0, 0], at);
        let right = network([2, 0, 0], at);
        assert_eq!(left.anchor(), Some([0, 0, 0]));
        assert_eq!(
            right.anchor(),
            left.anchor(),
            "the fill's start is not identity"
        );
        // A cable alone has no controller, so it has no root.
        let loose = network([10, 0, 0], |pos| {
            (pos == [10, 0, 0]).then(|| Node::plain(Kind::Cable))
        });
        assert_eq!(loose.anchor(), None);
        assert!(!loose.online());
    }

    #[test]
    fn the_view_caps_a_slot_to_one_stack_but_keeps_the_total() {
        let disks = [disk_with("minecraft:diamond", 100)];
        let view = view(&disks, "", GRID_SLOTS, stack_of);
        assert_eq!(view, vec![TestItem::new("minecraft:diamond", 64)]);
        assert_eq!(amount(&disks, "minecraft:diamond"), 100);
    }

    #[test]
    fn the_search_is_a_case_insensitive_substring_of_the_id() {
        let disks = [
            disk_with("minecraft:diamond", 10),
            disk_with("minecraft:diamond_block", 1),
            disk_with("refinedstorage:1k_storage_disk", 1),
        ];
        let ids: Vec<&str> = view(&disks, "DIAMOND", GRID_SLOTS, stack_of)
            .iter()
            .map(|entry| entry.id)
            .collect();
        assert_eq!(
            ids,
            vec!["minecraft:diamond_block", "minecraft:diamond"],
            "smallest amount first, `_block` sorts before the bare id"
        );
        assert!(
            view(&disks, "storage", GRID_SLOTS, stack_of)
                .iter()
                .all(|entry| entry.id.contains("storage"))
        );
        assert!(view(&disks, "nothing_matches", GRID_SLOTS, stack_of).is_empty());
        assert!(
            matches("minecraft:diamond", "  "),
            "an empty search lists all"
        );
    }

    #[test]
    fn the_view_shows_at_most_the_slots_it_has() {
        let mut disks = vec![Disk::new(CAPACITY_1K * 4)];
        for index in 0..GRID_SLOTS + 5 {
            let id: &'static str = Box::leak(format!("test:item{index}").into_boxed_str());
            disks[0].insert(TestItem::new(id, 1));
        }
        assert_eq!(view(&disks, "", GRID_SLOTS, stack_of).len(), GRID_SLOTS);
        assert_eq!(view(&disks, "", 10, stack_of).len(), 10);
    }

    // --- grid slot reconciliation -----------------------------------------

    #[test]
    fn taking_a_view_stack_withdraws_it_from_the_network() {
        let before = [Some(TestItem::new("minecraft:diamond", 64))];
        let after = [None];
        assert_eq!(
            deltas(&before, &after, &[false]),
            vec![Delta::Withdraw {
                slot: 0,
                stack: TestItem::new("minecraft:diamond", 64)
            }]
        );
    }

    #[test]
    fn a_partial_take_withdraws_only_the_difference() {
        let before = [Some(TestItem::new("minecraft:diamond", 64))];
        let after = [Some(TestItem::new("minecraft:diamond", 4))];
        assert_eq!(
            deltas(&before, &after, &[false]),
            vec![Delta::Withdraw {
                slot: 0,
                stack: TestItem::new("minecraft:diamond", 60)
            }]
        );
    }

    #[test]
    fn placing_a_stack_into_an_empty_view_slot_deposits_it() {
        let before = [None];
        let after = [Some(TestItem::new("minecraft:dirt", 10))];
        assert_eq!(
            deltas(&before, &after, &[false]),
            vec![Delta::Deposit {
                slot: 0,
                stack: TestItem::new("minecraft:dirt", 10)
            }]
        );
    }

    #[test]
    fn adding_to_a_view_stack_deposits_the_difference() {
        let before = [Some(TestItem::new("minecraft:dirt", 10))];
        let after = [Some(TestItem::new("minecraft:dirt", 12))];
        assert_eq!(
            deltas(&before, &after, &[false]),
            vec![Delta::Deposit {
                slot: 0,
                stack: TestItem::new("minecraft:dirt", 2)
            }]
        );
    }

    #[test]
    fn swapping_a_view_stack_withdraws_then_deposits() {
        let before = [Some(TestItem::new("minecraft:diamond", 3))];
        let after = [Some(TestItem::new("minecraft:dirt", 5))];
        assert_eq!(
            deltas(&before, &after, &[false]),
            vec![
                Delta::Withdraw {
                    slot: 0,
                    stack: TestItem::new("minecraft:diamond", 3)
                },
                Delta::Deposit {
                    slot: 0,
                    stack: TestItem::new("minecraft:dirt", 5)
                }
            ]
        );
    }

    #[test]
    fn an_unchanged_slot_is_no_work() {
        let before = [Some(TestItem::new("minecraft:diamond", 64)), None];
        let after = before.clone();
        assert!(deltas(&before, &after, &[false, false]).is_empty());
    }

    #[test]
    fn a_slot_the_network_could_not_take_is_the_player_s() {
        let before = [Some(TestItem::new("minecraft:dirt", 10))];
        let after = [None];
        assert!(
            deltas(&before, &after, &[true]).is_empty(),
            "emptying an owned slot does not take dirt off the network"
        );
    }

    #[test]
    fn a_fresh_grid_with_items_but_no_memory_does_not_deposit_them() {
        // The first tick after a load passes an empty `before`: the view is
        // rebuilt rather than deposited, so the caller must not use `deltas`
        // then. This pins the contract: equal-length slices only.
        let before: [Option<TestItem>; 0] = [];
        let after = [Some(TestItem::new("minecraft:dirt", 10))];
        assert_eq!(
            deltas(&before, &after, &[]),
            vec![Delta::Deposit {
                slot: 0,
                stack: TestItem::new("minecraft:dirt", 10)
            }],
            "an empty `before` is a deposit, which is why the glue skips the first tick"
        );
    }

    // --- whole flow --------------------------------------------------------

    #[test]
    fn a_grid_view_round_trip_conserves_items() {
        let mut disks = [disk_with("minecraft:diamond", 100)];
        // The view shows one capped stack.
        let shown = view(&disks, "", GRID_SLOTS, stack_of);
        // The player shift-clicks all 64 out of slot 0.
        let before = [Some(shown[0].clone())];
        let after: [Option<TestItem>; 1] = [None];
        for delta in deltas(&before, &after, &[false]) {
            if let Delta::Withdraw { stack, .. } = delta {
                let taken = extract(&mut disks, &stack.key(), stack.count()).expect("held");
                assert_eq!(taken.count(), 64);
            }
        }
        assert_eq!(amount(&disks, "minecraft:diamond"), 36);
        assert_eq!(view(&disks, "", GRID_SLOTS, stack_of)[0].count(), 36);
        // Depositing 36 back fills the disk to 72.
        let before = [Some(view(&disks, "", GRID_SLOTS, stack_of)[0].clone())];
        let after = [Some(TestItem::new("minecraft:diamond", 72))];
        for delta in deltas(&before, &after, &[false]) {
            if let Delta::Deposit { stack, .. } = delta {
                assert!(insert(&mut disks, stack).is_none());
            }
        }
        assert_eq!(amount(&disks, "minecraft:diamond"), 72);
    }

    // --- the transfer machines (slice 7) ------------------------------------

    fn key(id: &str) -> String {
        id.to_owned()
    }

    #[test]
    fn a_machine_without_speed_upgrades_works_every_nine_ticks() {
        assert_eq!(work_interval(0), WORK_TICK_RATE);
        assert_eq!(work_interval(0), 9);
    }

    #[test]
    fn each_speed_upgrade_shortens_the_work_interval_by_two() {
        assert_eq!(work_interval(1), 7);
        assert_eq!(work_interval(2), 5);
        assert_eq!(work_interval(3), 3);
        assert_eq!(work_interval(4), 1);
    }

    #[test]
    fn a_fifth_speed_upgrade_is_not_counted() {
        // Upstream's container refuses a fifth speed upgrade; a stray count
        // must not make the interval zero (which would divide by zero) or
        // negative.
        assert_eq!(work_interval(5), 1);
        assert_eq!(work_interval(64), 1);
    }

    #[test]
    fn a_work_pass_falls_on_every_interval_th_tick_starting_with_the_first() {
        let interval = work_interval(1); // 7
        let due: Vec<u64> = (0..15).filter(|tick| work_due(*tick, interval)).collect();
        assert_eq!(due, vec![0, 7, 14]);
        let due: Vec<u64> = (0..10)
            .filter(|tick| work_due(*tick, work_interval(4)))
            .collect();
        assert_eq!(due, (0..10).collect::<Vec<u64>>());
    }

    #[test]
    fn a_stack_upgrade_multiplies_one_pass_by_a_stack() {
        assert_eq!(transfer_quota(0), 1);
        assert_eq!(transfer_quota(1), 64);
        // One stack upgrade is all upstream's quota reads; a stray second one
        // does not raise it again.
        assert_eq!(transfer_quota(2), 64);
    }

    #[test]
    fn an_empty_filter_imports_everything() {
        let source = [Some((key("minecraft:stone"), 5))];
        assert_eq!(
            import_plan(&source, &[], 1),
            Some((0, key("minecraft:stone"), 1)),
            "an empty BLOCK filter blocks nothing"
        );
    }

    #[test]
    fn a_filter_imports_everything_but_what_it_names() {
        let source = [
            Some((key("minecraft:stone"), 5)),
            Some((key("minecraft:gold_ingot"), 4)),
        ];
        assert_eq!(
            import_plan(&source, &[key("minecraft:stone")], 1),
            Some((1, key("minecraft:gold_ingot"), 1)),
            "the named resource is blocked; the next one is taken"
        );
        assert_eq!(
            import_plan(
                &source,
                &[key("minecraft:stone"), key("minecraft:gold_ingot")],
                64
            ),
            None,
            "a filter naming both lets nothing through"
        );
    }

    #[test]
    fn an_import_takes_the_quota_and_no_more_than_the_slot_holds() {
        let source = [None, Some((key("minecraft:stone"), 5))];
        assert_eq!(
            import_plan(&source, &[], 64),
            Some((1, key("minecraft:stone"), 5))
        );
        assert_eq!(
            import_plan(&source, &[], 1),
            Some((1, key("minecraft:stone"), 1))
        );
        // An empty slot is skipped, and an empty source has nothing to take.
        assert_eq!(import_plan(&[None], &[], 1), None);
        assert_eq!(
            import_plan(&[Some((key("minecraft:stone"), 0))], &[], 1),
            None
        );
    }

    #[test]
    fn an_exporter_exports_only_what_its_filter_names() {
        let held = [
            (key("minecraft:stone"), 5),
            (key("minecraft:gold_ingot"), 3),
        ];
        assert_eq!(
            export_plan(&[key("minecraft:gold_ingot")], &held, 1),
            Some((key("minecraft:gold_ingot"), 1))
        );
        assert_eq!(
            export_plan(&[key("minecraft:diamond")], &held, 1),
            None,
            "a filter naming what the network does not hold exports nothing"
        );
    }

    #[test]
    fn an_exporter_without_a_filter_exports_nothing() {
        // Upstream's exporter has one task per filter entry; no entry, no work.
        let held = [(key("minecraft:gold_ingot"), 3)];
        assert_eq!(export_plan(&[], &held, 1), None);
    }

    #[test]
    fn an_export_takes_the_quota_and_no_more_than_the_network_holds() {
        let held = [(key("minecraft:gold_ingot"), 3)];
        assert_eq!(
            export_plan(&[key("minecraft:gold_ingot")], &held, 64),
            Some((key("minecraft:gold_ingot"), 3))
        );
        assert_eq!(
            export_plan(&[key("minecraft:gold_ingot")], &held, 1),
            Some((key("minecraft:gold_ingot"), 1))
        );
    }

    #[test]
    fn the_first_filter_entry_with_stock_wins() {
        let held = [(key("minecraft:gold_ingot"), 3)];
        assert_eq!(
            export_plan(
                &[key("minecraft:diamond"), key("minecraft:gold_ingot")],
                &held,
                1
            ),
            Some((key("minecraft:gold_ingot"), 1)),
            "the first entry with stock is the pass's task"
        );
    }

    #[test]
    fn a_machine_readout_names_its_upgrades_and_interval() {
        assert_eq!(
            machine_readout("Importer", [43, 64, 16], 1, 0, 1),
            "Importer at 43,64,16: 1 speed upgrade, 0 stack upgrades, work interval 7 ticks, 1 filter"
        );
        assert_eq!(
            machine_readout("Exporter", [41, 64, 17], 4, 1, 0),
            "Exporter at 41,64,17: 4 speed upgrades, 1 stack upgrade, work interval 1 tick, 0 filters"
        );
    }

    // --- the network's energy (slice 8) ------------------------------------

    #[test]
    fn a_transmitter_charges_sixteen_fe_a_tick() {
        assert_eq!(usage(Kind::WirelessTransmitter, Fit::default(), 0), 16);
    }

    #[test]
    fn each_range_upgrade_adds_eight_fe_to_the_transmitters_charge() {
        let fit = |range| Fit {
            range,
            ..Fit::default()
        };
        assert_eq!(usage(Kind::WirelessTransmitter, fit(1), 0), 24);
        assert_eq!(usage(Kind::WirelessTransmitter, fit(4), 0), 48);
        assert_eq!(
            usage(Kind::WirelessTransmitter, fit(9), 0),
            48,
            "a fifth range upgrade is not counted, as the range rule caps it"
        );
        assert_eq!(
            usage(
                Kind::WirelessTransmitter,
                Fit {
                    range: 4,
                    creative_range: true,
                    ..Fit::default()
                },
                0
            ),
            16,
            "the creative range upgrade replaces the range fee with its own 0"
        );
    }

    #[test]
    fn a_drive_charges_ten_and_four_for_each_disk() {
        assert_eq!(usage(Kind::DiskDrive, Fit::default(), 0), 10);
        assert_eq!(usage(Kind::DiskDrive, Fit::default(), 1), 14);
        assert_eq!(usage(Kind::DiskDrive, Fit::default(), 9), 46);
    }

    #[test]
    fn a_grid_a_machine_a_cable_and_the_controller_charge_their_upstream_rates() {
        assert_eq!(usage(Kind::Grid, Fit::default(), 0), 10);
        assert_eq!(usage(Kind::Importer, Fit::default(), 0), 2);
        assert_eq!(usage(Kind::Exporter, Fit::default(), 0), 2);
        assert_eq!(usage(Kind::Cable, Fit::default(), 0), 0);
        assert_eq!(usage(Kind::Controller, Fit::default(), 0), 0);
    }

    #[test]
    fn a_machines_upgrades_add_to_its_charge() {
        let four_speed_and_a_stack = Fit {
            speed: 4,
            stack: 1,
            ..Fit::default()
        };
        assert_eq!(
            usage(Kind::Importer, four_speed_and_a_stack, 0),
            34,
            "2 + 4 × 4 + 16"
        );
        assert_eq!(
            usage(
                Kind::Exporter,
                Fit {
                    speed: 1,
                    ..Fit::default()
                },
                0
            ),
            6
        );
        assert_eq!(
            usage(
                Kind::Importer,
                Fit {
                    speed: 9,
                    stack: 4,
                    ..Fit::default()
                },
                0
            ),
            34,
            "a stray stack of upgrades cannot charge past the caps"
        );
    }

    #[test]
    fn a_node_works_only_when_the_network_can_pay_its_charge() {
        assert!(
            Energy::new(16, 16).energized(),
            "exactly its charge is enough"
        );
        assert!(!Energy::new(15, 16).energized());
        assert_eq!(Energy::new(16, 16).draw(), Some(0));
        assert_eq!(Energy::new(15, 16).draw(), None, "no work, nothing taken");
    }

    #[test]
    fn a_tick_takes_exactly_the_charge_and_never_goes_negative() {
        let mut stored = 100;
        let mut ticks = 0;
        while let Some(left) = Energy::new(stored, TRANSMITTER_USAGE).draw() {
            stored = left;
            ticks += 1;
        }
        assert_eq!(ticks, 6, "100 / 16 is 6 whole ticks");
        assert_eq!(stored, 4, "the remainder stays (100 - 96)");
    }

    #[test]
    fn a_networks_energy_is_the_sum_of_its_controllers() {
        assert_eq!(bank([400, 600]), CONTROLLER_CAPACITY);
        assert_eq!(bank([]), 0);
    }

    #[test]
    fn the_energy_type_is_the_four_upstream_levels() {
        assert_eq!(EnergyType::of(1000, 1000), EnergyType::On);
        assert_eq!(EnergyType::of(400, 1000), EnergyType::On, "40 % is on");
        assert_eq!(EnergyType::of(399, 1000), EnergyType::NearlyOn);
        assert_eq!(
            EnergyType::of(300, 1000),
            EnergyType::NearlyOn,
            "30 % is nearly on"
        );
        assert_eq!(EnergyType::of(299, 1000), EnergyType::NearlyOff);
        assert_eq!(
            EnergyType::of(10, 1000),
            EnergyType::NearlyOff,
            "1 % is nearly off"
        );
        assert_eq!(EnergyType::of(9, 1000), EnergyType::Off);
        assert_eq!(EnergyType::of(0, 1000), EnergyType::Off);
        assert_eq!(EnergyType::of(5, 0), EnergyType::Off, "no capacity is off");
    }

    #[test]
    fn the_energy_type_has_the_property_values_the_content_declares() {
        assert_eq!(EnergyType::Off.property(), "off");
        assert_eq!(EnergyType::NearlyOff.property(), "nearly_off");
        assert_eq!(EnergyType::NearlyOn.property(), "nearly_on");
        assert_eq!(EnergyType::On.property(), "on");
    }

    #[test]
    fn no_energy_refuses_a_use_before_the_range_is_asked() {
        assert_eq!(
            admit(true, true, Energy::new(0, TRANSMITTER_USAGE), 100.0, 16),
            Err(Refusal::NoEnergy {
                stored: 0,
                usage: TRANSMITTER_USAGE
            }),
            "the node is inactive, so the use is refused for energy, not for range"
        );
        assert_eq!(
            Refusal::NoEnergy {
                stored: 0,
                usage: 16
            }
            .to_string(),
            "not enough energy: 0 of 16 FE"
        );
    }

    #[test]
    fn a_use_with_energy_still_names_the_range() {
        assert_eq!(
            admit(true, true, Energy::new(100, TRANSMITTER_USAGE), 17.0, 16),
            Err(Refusal::OutOfRange {
                distance: 17.0,
                range: 16
            })
        );
        assert_eq!(
            admit(true, true, Energy::new(16, TRANSMITTER_USAGE), 2.5, 16),
            Ok(())
        );
    }

    // --- the wireless grid's own buffer (slice 9) ---------------------------

    #[test]
    fn each_wireless_grid_use_costs_five_fe() {
        assert_eq!(
            charge_item(WIRELESS_GRID_CAPACITY, WirelessUse::Open),
            Ok(995),
            "a full grid pays 5 to open"
        );
        assert_eq!(
            charge_item(WIRELESS_GRID_CAPACITY, WirelessUse::Insert),
            Ok(995),
            "a deposit pays 5"
        );
        assert_eq!(
            charge_item(WIRELESS_GRID_CAPACITY, WirelessUse::Extract),
            Ok(995),
            "a withdrawal pays 5"
        );
    }

    #[test]
    fn the_item_buffer_pays_exactly_and_refuses_below_the_cost() {
        assert_eq!(
            charge_item(5, WirelessUse::Open),
            Ok(0),
            "exactly the cost is enough"
        );
        assert_eq!(
            charge_item(4, WirelessUse::Insert),
            Err(Refusal::NoItemEnergy { stored: 4, cost: 5 })
        );
        assert_eq!(
            charge_item(0, WirelessUse::Extract),
            Err(Refusal::NoItemEnergy { stored: 0, cost: 5 })
        );
    }

    #[test]
    fn the_item_refusal_names_the_items_own_energy_not_the_networks() {
        let item = Refusal::NoItemEnergy {
            stored: 0,
            cost: WIRELESS_GRID_OPEN,
        }
        .to_string();
        let network = Refusal::NoEnergy {
            stored: 0,
            usage: TRANSMITTER_USAGE,
        }
        .to_string();
        assert_eq!(item, "not enough item energy: 0 of 5 FE");
        assert_ne!(
            item, network,
            "an empty grid and an empty network must not read alike"
        );
    }

    #[test]
    fn the_wireless_grids_buffer_is_the_reference_capacity_and_costs() {
        assert_eq!(WIRELESS_GRID_CAPACITY, 1000, "WIRELESS_GRID_CAPACITY");
        assert_eq!(WirelessUse::Open.cost(), 5, "WIRELESS_GRID_OPEN");
        assert_eq!(WirelessUse::Insert.cost(), 5, "WIRELESS_GRID_INSERT");
        assert_eq!(WirelessUse::Extract.cost(), 5, "WIRELESS_GRID_EXTRACT");
    }

    // --- a broken block's own slots on its item (slice 10) ------------------

    #[test]
    fn the_slot_counts_are_what_the_content_registers() {
        assert_eq!(MACHINE_FILTERS, 9, "a machine's filter entries");
        assert_eq!(MACHINE_UPGRADES, 4, "a machine's upgrade slots");
        assert_eq!(TRANSMITTER_SLOTS, 4, "the transmitter's upgrade slots");
        assert_eq!(
            Kind::Importer.own_slots(),
            13,
            "nine filters, four upgrades"
        );
        assert_eq!(
            Kind::Exporter.own_slots(),
            13,
            "nine filters, four upgrades"
        );
        assert_eq!(Kind::WirelessTransmitter.own_slots(), 4, "four upgrades");
        for kind in [Kind::Controller, Kind::Cable, Kind::DiskDrive, Kind::Grid] {
            assert_eq!(kind.own_slots(), 0, "{kind:?} has no container of its own");
        }
    }

    #[test]
    fn a_machines_slots_travel_on_its_item_and_a_drives_do_not() {
        for kind in [Kind::Importer, Kind::Exporter, Kind::WirelessTransmitter] {
            assert!(
                kind.carries_contents(),
                "{kind:?} keeps its own slots on its item"
            );
        }
        for kind in [Kind::Controller, Kind::Cable, Kind::DiskDrive, Kind::Grid] {
            assert!(
                !kind.carries_contents(),
                "{kind:?} does not: a drive's disks spill as items, as upstream drops them"
            );
        }
    }

    #[test]
    fn an_empty_machine_carries_nothing() {
        assert_eq!(carry::<&str>(&[]), None);
        assert_eq!(
            carry::<&str>(&[None, None, None]),
            None,
            "an empty container carries nothing, so the item stays plain"
        );
        assert_eq!(
            carry(&[None, Some("minecraft:dirt")]),
            Some(vec![(1, "minecraft:dirt")]),
            "a slot that held something keeps its index"
        );
        assert_eq!(
            carry(&[Some("refinedstorage:speed_upgrade"), None]),
            Some(vec![(0, "refinedstorage:speed_upgrade")])
        );
    }

    #[test]
    fn a_carried_list_restores_the_slots_it_names() {
        let carried = [
            (0, "refinedstorage:speed_upgrade"),
            (4, "refinedstorage:stack_upgrade"),
            (8, "minecraft:dirt"),
        ];
        assert_eq!(
            restore(&carried, MACHINE_FILTERS + MACHINE_UPGRADES),
            carried.to_vec(),
            "every carried slot is restored where it was"
        );
    }

    #[test]
    fn a_doctored_carried_list_cannot_reach_past_the_container() {
        let carried = [
            (0, "refinedstorage:range_upgrade"),
            (3, "refinedstorage:range_upgrade"),
            (4, "minecraft:diamond"),
            (99, "minecraft:diamond"),
        ];
        assert_eq!(
            restore(&carried, TRANSMITTER_SLOTS),
            vec![
                (0, "refinedstorage:range_upgrade"),
                (3, "refinedstorage:range_upgrade"),
            ],
            "a slot at or past the container is dropped"
        );
        assert_eq!(
            restore(
                &[(2, "minecraft:dirt"), (2, "minecraft:stone")],
                MACHINE_FILTERS
            ),
            vec![(2, "minecraft:dirt")],
            "a slot named twice keeps its first entry"
        );
        assert_eq!(restore::<&str>(&[(0, "x")], 0), vec![], "no slots, nothing");
    }

    #[test]
    fn a_placement_keeps_the_payload_on_the_units_it_did_not_place() {
        assert_eq!(
            placed_hand(false, 2),
            PlacedHand::EngineConsumes,
            "survival takes the placed unit; the second unit keeps its own payload"
        );
        assert_eq!(
            placed_hand(false, 1),
            PlacedHand::EngineConsumes,
            "survival takes the last unit too, and nothing is left to strip"
        );
        assert_eq!(
            placed_hand(true, 1),
            PlacedHand::StripPlaced,
            "creative keeps the stack: the one unit's spent payload comes off"
        );
        assert_eq!(
            placed_hand(true, 2),
            PlacedHand::UnplacedKeep,
            "creative keeps the stack: the unit that was not placed keeps its payload"
        );
        assert_eq!(placed_hand(true, 3), PlacedHand::UnplacedKeep);
    }

    #[test]
    fn one_disk_costs_one_item_from_the_hand_that_held_it() {
        assert_eq!(
            after_taking_one(1),
            None,
            "the last disk leaves an empty hand"
        );
        assert_eq!(after_taking_one(2), Some(1), "a stack of two keeps one");
        assert_eq!(after_taking_one(9), Some(8));
        assert_eq!(
            after_taking_one(0),
            None,
            "nothing held, nothing taken: the insertion must not commit"
        );
    }

    #[test]
    fn a_break_carries_its_slots_only_when_it_hands_over_an_item() {
        assert_eq!(
            break_plan(true),
            BreakPlan::Carry,
            "a break that drops loot carries them on the item"
        );
        assert_eq!(
            break_plan(false),
            BreakPlan::Spill,
            "a break that drops nothing leaves the engine's spill alone"
        );
    }

    #[test]
    fn a_carry_with_no_item_to_ride_falls_back_to_the_ground() {
        assert_eq!(carry_plan(true), CarryPlan::OnTheItem);
        assert_eq!(
            carry_plan(false),
            CarryPlan::BackOnTheGround,
            "no machine item to carry them: they drop, they are not discarded"
        );
    }

    #[test]
    fn only_an_operator_may_write_energy() {
        assert_eq!(ENERGY_LEVEL, 2, "the game master level, as Command::op");
        assert!(!may_write_energy(0), "every player may not charge");
        assert!(!may_write_energy(1), "a moderator may not charge");
        assert!(may_write_energy(2), "a game master may");
        assert!(may_write_energy(4), "an owner may");
    }
}
