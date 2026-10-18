//! The binding's protocol for pallets (`docs/pallets.md` section 4, task `pallet-messages`): what a host's players do
//! with the crates of a pallet, as inputs; what the pallets do, as outputs.
//!
//! **Outputs** are queued by the registry's movers (`Pallets::pack`, `move_to`, `open`, `unpack` and `repack`: `PalletPacked`,
//! `PalletMoved`, `PalletOpened`) once a host has asked for them (`Society::apply` turns [`Pallets::report`] on), and
//! come out of `apply` after the happening's own outputs. They are not state: a run with no host queues nothing, and
//! nothing a rule reads depends on them.
//!
//! **Inputs** (`Society::apply`):
//! - `PalletBrought { town, id }`: a player carried a whole pallet to a desk ([`Society::pallet_brought`]). Made out to
//!   the town it is received (`cargo::receive`, as `deliver_pallet` receives a pallet made from outside: goods into stock,
//!   a contract's goods delivered under it, the crates kept; the seal kept, since a player who brings every package is a
//!   carrier who kept it); made out elsewhere it waits in the town's warehouse to be fetched.
//! - `PackageBrought { town, pallet, package }`: one package of a pallet made out to the town ([`Society::package_brought`]).
//!   The pallet is received when every package has come, or at the town's next look ([`crate::rules::pallet_messages`])
//!   with those that have: the seal broken, the others lost.
//! - `PackageLost { pallet, package }`: a package destroyed ([`Society::package_lost`]). Its goods leave the world as
//!   used (the ledger, reason lost: [`Pallets::lost_goods`]), the seal is broken and the pallet goes on short; the
//!   packages after it move up a place. One left with no package is gone, and a bill of exchange that paid for it is
//!   unbacked ([`crate::bills::Bill::goods_lost`], read by the discount desk).
//!
//! A pallet is only in the world, as crates, where it stands in a town's warehouse (`At::Town`): on a wagon or in a
//! hub's custody it is a record, so these inputs name a pallet there as a stale view and are ignored.
//! Reference: `CaravanTests.whatAPlayerBroughtIsPaidForByTheNextCaravan` (the mod's `deliverShipment`).

use crate::cargo::{receive, Shipments};
use crate::contracts::Book;
use crate::messages::TownId;
use crate::pallet::{At, PalletId, Pallets};
use crate::society::Society;
use crate::state::{Ledger, Stock};

/// Takes a package off a pallet for good: its goods leave the world as used (reason lost), the seal is broken.
fn lose_package(pallets: &mut Pallets, ledger: &mut Ledger, id: PalletId, package: usize) {
    let p = pallets.pallets.get_mut(&id).expect("a pallet");
    let lost = p.packages.remove(package);
    p.seal = None;
    ledger.used[lost.good] += lost.count;
    *pallets.lost_goods.entry(lost.good).or_insert(0.0) += lost.count;
}

/// Receives a pallet at its consignee `town`, whose packages in `parts` (all, if none) have come: the others are lost,
/// the seal broken; then it is opened and its goods taken in (`cargo::receive`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn take_in(pallets: &mut Pallets, book: &mut Book, ledger: &mut Ledger, now: u64, town: TownId,
        stock: &mut [f64], store: &mut Shipments, id: PalletId, parts: Option<&[u32]>) {
    if let Some(parts) = parts {
        let len = pallets.pallets[&id].packages.len();
        for package in (0..len).rev().filter(|&i| !parts.contains(&(i as u32))) {
            lose_package(pallets, ledger, id, package);
        }
    }
    pallets.arriving.remove(&id);
    let p = pallets.open(id).expect("a pallet");
    receive(book, now, town, stock, store, &p);
}

impl Society {
    /// A pallet (in a town's warehouse, or a hub's: no matter, it is where the crates are) in `place` to the host: only
    /// in a town's warehouse are its crates in the world.
    fn in_a_warehouse(&self, id: PalletId) -> bool {
        self.pallets().get(id).is_some_and(|p| matches!(p.at, At::Town(_)))
    }

    /// Receives pallet `id` at `town`, caught up first (as `deliver` catches it up), with the packages in `parts`.
    fn receive_pallet(&mut self, town: TownId, id: PalletId, parts: Option<&[u32]>) {
        self.deliver(town, 0, 0.0); // brings the town up to date: nothing delivered
        let (e, now) = (self.towns[town], self.now());
        self.world.resource_scope(|world, mut pallets: bevy_ecs::prelude::Mut<Pallets>| {
            world.resource_scope(|world, mut book: bevy_ecs::prelude::Mut<Book>| {
                world.resource_scope(|world, mut ledger: bevy_ecs::prelude::Mut<Ledger>| {
                    let mut stock = world.get::<Stock>(e).expect("a town").0.clone();
                    let mut store = world.get::<Shipments>(e).expect("a town").clone();
                    take_in(&mut pallets, &mut book, &mut ledger, now, town, &mut stock, &mut store, id, parts);
                    world.get_mut::<Stock>(e).expect("a town").0 = stock;
                    *world.get_mut::<Shipments>(e).expect("a town") = store;
                });
            });
        });
    }

    /// A player carried the whole of pallet `id` to `town`'s desk (`PalletBrought`).
    pub fn pallet_brought(&mut self, town: TownId, id: PalletId) {
        if !self.in_a_warehouse(id) {
            return;
        }
        if self.pallets().get(id).is_some_and(|p| p.consigned_to(town)) {
            self.receive_pallet(town, id, None);
        } else {
            let now = self.now();
            let mut pallets = self.world.resource_mut::<Pallets>();
            pallets.arriving.remove(&id);
            pallets.move_to(id, At::Town(town), now);
        }
    }

    /// A player carried package `package` of pallet `pallet`, made out to `town`, to its desk (`PackageBrought`).
    pub fn package_brought(&mut self, town: TownId, pallet: PalletId, package: u32) {
        let Some(p) = self.pallets().get(pallet) else { return };
        if !self.in_a_warehouse(pallet) || !p.consigned_to(town) || package as usize >= p.packages.len() {
            return;
        }
        let (len, now) = (p.packages.len(), self.now());
        let mut pallets = self.world.resource_mut::<Pallets>();
        let a = pallets.arriving.entry(pallet).or_default();
        if a.parts.is_empty() || a.town != town {
            *a = crate::pallet::Arriving { town, since: now, parts: Vec::new() };
        }
        if !a.parts.contains(&package) {
            a.parts.push(package);
        }
        if a.parts.len() == len {
            self.receive_pallet(town, pallet, None);
        }
    }

    /// Package `package` of pallet `pallet` was lost (`PackageLost`).
    pub fn package_lost(&mut self, pallet: PalletId, package: u32) {
        let Some(p) = self.pallets().get(pallet) else { return };
        if !self.in_a_warehouse(pallet) || package as usize >= p.packages.len() {
            return;
        }
        let emptied = p.packages.len() == 1;
        self.world.resource_scope(|world, mut pallets: bevy_ecs::prelude::Mut<Pallets>| {
            world.resource_scope(|world, mut ledger: bevy_ecs::prelude::Mut<Ledger>| {
                lose_package(&mut pallets, &mut ledger, pallet, package as usize);
                // Those after it move up a place, so the parts brought do too.
                if let Some(a) = pallets.arriving.get_mut(&pallet) {
                    a.parts.retain(|&i| i != package);
                    for i in a.parts.iter_mut().filter(|i| **i > package) {
                        *i -= 1;
                    }
                    if a.parts.is_empty() {
                        pallets.arriving.remove(&pallet); // nothing brought is left: not arriving
                    }
                }
                if emptied {
                    let p = pallets.pallets.remove(&pallet).expect("a pallet");
                    pallets.unindex(p.at, pallet);
                    pallets.arriving.remove(&pallet);
                    pallets.lost += 1;
                    for bill in world.resource_mut::<crate::bills::Bills>().bills.iter_mut()
                        .filter(|b| b.pallets.contains(&pallet)) {
                        bill.lost.push(pallet);
                    }
                }
            });
        });
    }
}
