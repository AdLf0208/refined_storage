//! Refined Storage for Horizon: the item network core (slice 1).
//!
//! A **Controller** is the network's root: a connected component of network
//! blocks with one is online, without one nothing works. **Cables** wire the
//! component, a **Disk Drive** holds nine **1k Storage Disks** (the items live
//! in the disk item's own component, so they travel with the disk), and the
//! **Grid** is a 9×6 menu that shows the network's resources and withdraws
//! from or deposits into them.
//!
//! [`runtime`] is the pure model (the component, the disks, the grid's view and
//! the reconciliation of its menu slots); [`support`] is the game glue. See
//! `notes.md` for the slice boundary, the gaps and the two review repairs this
//! migration keeps.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "runtime.rs carries the `view` wrapper its tests use; the port calls `view_with`"
    )
)]
mod runtime;
mod support;

use mod_api::{
    self, ComponentTypesEvent, Flow, Hooks, HostSide, RegisterCommandsEvent, Registrar,
    RegistrationContext, RightClickBlockEvent, RunModCommandEvent, ServerHost, ServerStartedEvent,
    WorldLifecycleEvent,
};

/// The mod holds no state of its own: everything the network needs is derived
/// from the loaded blocks and their block entities, and lives in [`support`].
#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Declare the network's one block-entity type, its Part, the two block
    /// containers (the drive's and the grid's menus) and the disk item.
    fn register(&self, registrar: &mut dyn Registrar, cx: &RegistrationContext) {
        support::register(registrar, cx);
    }

    /// The component that carries a storage disk's contents, before content
    /// loads.
    fn component_types(&self, _cx: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        support::register_component_types(event);
    }

    /// A world finished opening: its cache starts empty (the old
    /// `WorldOpenReady` phase is this hook at the new pin).
    fn server_started(&self, _host: &mut dyn ServerHost, _event: &mut ServerStartedEvent) {
        support::reset();
    }

    /// A world that failed to open or is closing drops what was derived from
    /// it.
    fn world_lifecycle(&self, _host: &mut dyn ServerHost, event: &mut WorldLifecycleEvent<'_>) {
        support::on_world_lifecycle(event.phase);
    }

    /// A storage disk right-clicked onto a drive goes into its first free
    /// slot; every other click is the block's own (the menu opens).
    fn right_click_block(&self, host: HostSide<'_>, event: &mut RightClickBlockEvent<'_>) -> Flow {
        support::on_right_click_block(host, event)
    }

    /// `/refinedstorage info|filter`.
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        support::on_register_commands(event);
    }

    /// The bodies of `/refinedstorage info|filter`.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        support::on_run_mod_command(host, event)
    }
}
