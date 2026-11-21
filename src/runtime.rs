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

/// The network blocks of the port: the slice's four, plus the wireless
/// transmitter (slice 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Controller,
    Cable,
    DiskDrive,
    Grid,
    WirelessTransmitter,
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
        ]
        .into_iter()
        .find(|kind| kind.block_id() == block)
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
    /// The user is further from the transmitter's block than its range.
    OutOfRange { distance: f64, range: u32 },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::DifferentDimension => write!(f, "different dimension"),
            Refusal::Offline => write!(f, "the network is offline"),
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
/// and the transmitter are in one dimension; `distance` the value
/// [`distance`] answered; `range` the transmitter's range in blocks.
///
/// The checks run in upstream's order (`NetworkItemPlayerValidator.isValid`):
/// dimension first, then the node's activeness, then the range, so a use in
/// another dimension is refused as such even when the transmitter is offline
/// too. The boundary is inclusive, as upstream's `<=` is.
pub fn admit(online: bool, same_dimension: bool, distance: f64, range: u32) -> Result<(), Refusal> {
    if !same_dimension {
        return Err(Refusal::DifferentDimension);
    }
    if !online {
        return Err(Refusal::Offline);
    }
    if distance > f64::from(range) {
        return Err(Refusal::OutOfRange { distance, range });
    }
    Ok(())
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

/// One storage disk's contents: at most `capacity` items over any number of
/// resources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disk<T> {
    capacity: u64,
    entries: Vec<T>,
}

impl<T: Stored> Disk<T> {
    /// An empty disk of `capacity` items.
    pub fn new(capacity: u64) -> Self {
        Disk {
            capacity,
            entries: Vec::new(),
        }
    }

    /// The disk's capacity in items.
    #[allow(dead_code, reason = "the commands report it; the tick reads `free`")]
    pub fn capacity(&self) -> u64 {
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
        self.capacity.saturating_sub(self.total())
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
        let mut room = self.capacity;
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
        assert_eq!(Kind::from_block("minecraft:stone"), None);
        assert_eq!(Kind::from_block("refinedstorage:importer"), None);
        assert_eq!(Kind::Controller.block_id(), "refinedstorage:controller");
        assert_eq!(
            Kind::WirelessTransmitter.block_id(),
            "refinedstorage:wireless_transmitter"
        );
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
            admit(false, false, 0.0, WIRELESS_RANGE),
            Err(Refusal::DifferentDimension),
            "upstream checks the dimension before the node's activeness"
        );
        assert_eq!(
            admit(true, false, 1.0, WIRELESS_RANGE),
            Err(Refusal::DifferentDimension)
        );
    }

    #[test]
    fn an_offline_transmitter_refuses_a_use_within_its_range() {
        assert_eq!(
            admit(false, true, 1.0, WIRELESS_RANGE),
            Err(Refusal::Offline)
        );
    }

    #[test]
    fn a_use_beyond_the_range_is_refused_naming_the_distance() {
        assert_eq!(
            admit(true, true, 16.5, WIRELESS_RANGE),
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
        assert_eq!(admit(true, true, 16.0, WIRELESS_RANGE), Ok(()));
        assert!(admit(true, true, 16.000_1, WIRELESS_RANGE).is_err());
        assert_eq!(admit(true, true, 0.0, WIRELESS_RANGE), Ok(()));
    }

    #[test]
    fn the_distance_is_measured_from_the_transmitter_block_to_the_user() {
        // 1.5 blocks east, 1.5 blocks south of the block's corner: sqrt(2.25 + 2.25).
        let d = distance([42.5, 64.0, 2.5], [41, 64, 1]);
        assert!((d - 2.121_320_3).abs() < 1e-6, "distance was {d}");
        assert_eq!(distance([41.0, 64.0, 1.0], [41, 64, 1]), 0.0);
    }

    // --- one disk ----------------------------------------------------------

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
}
