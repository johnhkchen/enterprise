//! The pallets as a host shows them (`docs/pallets.md` section 4, `docs/binding-plan.md` task `pallet-views`): the
//! core implements pallets and the host draws them (John, 2026-10-18), with no pallet rules in the host.
//!
//! A pallet stays invisible: in the host's world it is its packages, one item each (a crate, a barrel, a bundle, or
//! loose stacks strapped together), tagged with the pallet they belong to, the first carrying the bill of lading. They
//! are in the world only where the pallet stands in a town's warehouse (`At::Town`): the desk's chests. What the core
//! keeps of that showing is [`Stowage`]: for each pallet whose packages the host put in a desk's chests, where, and the
//! pallet as it was then (the tick it came to the warehouse, `Pallet.since`, and its `digest` of bill and packages).
//! The showing holds while the pallet stays there unchanged but for packages lost (the host's other crates of it are
//! where they were); a pallet moved (loaded on a wagon, brought elsewhere), opened or repacked is shown afresh wherever
//! it is next, and the host's crates of it as it was are stale (their `since` isn't the pallet's, or their goods aren't
//! among its packages): the host takes them out where it finds them.
//!
//! - **Stowing** ([`Society::stow_view`], `Query::Stow`): a chest takes a pallet whole or not at all. Asked with a desk's
//!   free slots, the core offers its town's pallets not shown anywhere, in arrival order, each whole if its packages fit
//!   in what's left, else passed over; the host puts them in and says so (`Input::Stowed`, [`Society::stowed`]).
//! - **Packing** (`Input::Pack`, [`Society::pack_for`]): a player has the Merchant pack goods from stock for another
//!   town, as `Society::pack_pallets` packs (only what can be crated, a bill a pallet, under no contract): the pallets
//!   wait in the warehouse for a carrier, the consignee's wagon fetching them or a player carrying them
//!   (`PalletBrought`, `PackageBrought`; `pallet_messages.rs`).
//! - **Views** ([`Society::pallet_view`], `Query::Pallets`, `Pallet`, `Manifest`): a pallet's bill, packages, seal and
//!   showing as numbers and names; a town's pallets held and inbound with the registry's tallies (received, lost, the
//!   goods lost off pallets); a wagon's manifest and whether it needs a superbill.
//!
//! What a short pallet comes to (its seal broken, the rest a loss on the bill, booked as used) is
//! `pallet_messages.rs`'s: the host says what came and what was lost, the core judges.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::cargo::How;
use crate::content::GoodId;
use crate::messages::{RouteId, TownId};
use crate::pallet::{At, Lading, Load, Pallet, PalletId, Pallets, Payment};
use crate::points::{Kind, Points, Spot};
use crate::society::Society;
use crate::state::{RouteState, Town};
use crate::wire::{self, Answer, Count, ManifestLine, PackageLine, PalletView, Point};

/// Where a host put a pallet's packages, and the pallet as it was then.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Shown {
    /// The desk whose chests they went in.
    pub at: Spot,
    /// The tick the pallet came to its warehouse (`Pallet.since`): moved since, it's elsewhere.
    pub since: u64,
    /// Its digest then (`Pallet::digest`), kept up through packages lost (`package_lost_shown`): repacked since, it's
    /// another pallet's showing.
    pub digest: u64,
}

/// The pallets whose packages a host put in a desk's chests, by pallet (`pallet_host.rs`).
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stowage {
    pub shown: BTreeMap<PalletId, Shown>,
}

impl Stowage {
    /// Where pallet `p`'s packages are shown, if the showing is still of it: it stands in the warehouse of the town whose
    /// desk it was put at, since the same tick, with the same bill and packages.
    pub fn current<'a>(&'a self, p: &Pallet, points: &Points) -> Option<&'a Shown> {
        let s = self.shown.get(&p.id)?;
        let desk = points.0.get(&s.at).filter(|d| d.kind == Kind::Desk)?;
        (p.at == At::Town(desk.town) && p.since == s.since && p.digest() == s.digest).then_some(s)
    }

    /// Forgets every showing that's no longer of its pallet as it is (moved, opened, changed, its desk gone).
    fn prune(&mut self, pallets: &Pallets, points: &Points) {
        let stale: Vec<PalletId> = self.shown.keys().copied()
            .filter(|id| pallets.get(*id).is_none_or(|p| self.current(p, points).is_none())).collect();
        for id in stale {
            self.shown.remove(&id);
        }
    }
}

impl Society {
    /// A player has `town`'s Merchant pack `goods` from its stock for `consignee` (`Input::Pack`): pallets under no
    /// contract, a bill each, only what can be crated (`Society::pack_pallets` with [`How::CRATED`]). The pallets made,
    /// in order.
    pub fn pack_for(&mut self, town: TownId, consignee: TownId, goods: &[(GoodId, f64)]) -> Vec<PalletId> {
        if town == consignee {
            return Vec::new();
        }
        let lading = Lading::new(town, consignee, self.now());
        self.pack_pallets(town, lading, goods, How::CRATED)
    }

    /// The host put pallet `pallet`'s packages in the chests of the desk at `point` (`Input::Stowed`): shown there while
    /// it stays as it is. Ignored for a pallet that isn't in that desk's town's warehouse (a stale view).
    pub fn stowed(&mut self, point: &Point, pallet: PalletId) {
        let spot = point.spot();
        let Some(town) = self.point(&spot).filter(|p| p.kind == Kind::Desk).map(|p| p.town) else { return };
        let (pallets, points) = (self.world.resource::<Pallets>(), self.world.resource::<Points>());
        let shown = pallets.get(pallet).filter(|p| p.at == At::Town(town))
            .map(|p| Shown { at: spot, since: p.since, digest: p.digest() });
        let mut stowage = self.world.resource::<Stowage>().clone();
        stowage.prune(pallets, points);
        if let Some(shown) = shown {
            stowage.shown.insert(pallet, shown);
        }
        *self.world.resource_mut::<Stowage>() = stowage;
    }

    /// Package `package` of pallet `pallet` lost (`Input::PackageLost`, [`Society::package_lost`]): the pallet goes on
    /// short where it is, and a showing of it stays its showing (the host's other crates of it are where they were: only
    /// the lost one is gone), now of the pallet as it is.
    pub fn package_lost_shown(&mut self, pallet: PalletId, package: u32) {
        let shown = self.pallets().get(pallet)
            .is_some_and(|p| self.world.resource::<Stowage>().current(p, self.world.resource::<Points>()).is_some());
        self.package_lost(pallet, package);
        let digest = self.pallets().get(pallet).map(Pallet::digest);
        let mut stowage = self.world.resource_mut::<Stowage>();
        match (shown, digest) {
            (true, Some(digest)) => {
                if let Some(s) = stowage.shown.get_mut(&pallet) {
                    s.digest = digest;
                }
            }
            (_, None) => {
                stowage.shown.remove(&pallet);
            }
            _ => {}
        }
    }

    /// A town's name, for a host to show.
    fn town_name(&self, town: TownId) -> String {
        self.towns.get(town).and_then(|&e| self.world.get::<Town>(e)).map_or_else(String::new, |t| t.name.clone())
    }

    /// A pallet as a host shows it.
    pub fn pallet_view(&self, p: &Pallet) -> PalletView {
        let (pallets, points) = (self.world.resource::<Pallets>(), self.world.resource::<Points>());
        let shown = self.world.resource::<Stowage>().current(p, points).map(|s| Point::of(&s.at, Kind::Desk));
        let l = &p.lading;
        PalletView {
            id: p.id,
            at: p.at.into(),
            since: p.since,
            shipper: wire::no(l.shipper),
            consignee: wire::no(l.consignee),
            from: self.town_name(l.shipper),
            to: self.town_name(l.consignee),
            contract: l.contract.map(wire::no),
            paid: l.payment == Payment::Paid,
            issued: l.issued,
            sealed: p.seal.is_some(),
            intact: p.seal.is_some() && p.intact(),
            digest: p.digest(),
            packages: p.packages.iter()
                .map(|pk| PackageLine { kind: wire::no(pk.kind), good: wire::no(pk.good), count: pk.count }).collect(),
            items: p.packages.len() as u32,
            weight: Load::of_pallet(self.content(), p).weight,
            shown,
            arriving: pallets.arriving.get(&p.id).map(|a| a.parts.clone()).unwrap_or_default(),
        }
    }

    /// `Answer::Pallets` for `town`: its warehouse's pallets in arrival order, those made out to it elsewhere by id, and
    /// the registry's tallies.
    pub fn pallets_view(&self, town: TownId) -> Answer {
        let pallets = self.pallets();
        let held = pallets.at_place(At::Town(town)).map(|p| self.pallet_view(p)).collect();
        let inbound = pallets.pallets.values().filter(|p| p.consigned_to(town) && p.at != At::Town(town))
            .map(|p| self.pallet_view(p)).collect();
        Answer::Pallets {
            town: wire::no(town),
            held,
            inbound,
            received: pallets.received,
            lost: pallets.lost,
            lost_goods: pallets.lost_goods.iter().map(|(&g, &count)| Count { good: wire::no(g), count }).collect(),
        }
    }

    /// `Answer::Stow` for the desk at `point` with `free` slots empty: its town's pallets shown nowhere, in arrival order,
    /// each whole if its packages fit what's left (one slot an item), else passed over. `None` if no desk stands there.
    pub fn stow_view(&self, point: &Point, free: u32) -> Option<Answer> {
        let town = self.point(&point.spot()).filter(|p| p.kind == Kind::Desk)?.town;
        let (pallets, points, stowage) =
            (self.pallets(), self.world.resource::<Points>(), self.world.resource::<Stowage>());
        let mut left = free;
        let mut stow = Vec::new();
        for p in pallets.at_place(At::Town(town)).filter(|p| stowage.current(p, points).is_none()) {
            let items = p.packages.len() as u32;
            if items <= left {
                left -= items;
                stow.push(self.pallet_view(p));
            }
        }
        Some(Answer::Stow { point: point.clone(), pallets: stow })
    }

    /// `Answer::Manifest` for `route`'s wagon: a line for each pallet loose aboard, then each consolidation's pallets with
    /// its drop, and whether it needs a superbill (two lines or more).
    pub fn manifest_view(&self, route: RouteId) -> Answer {
        let wagon = self.routes.get(route).and_then(|&e| self.world.get::<RouteState>(e)).and_then(|s| s.wagon.as_ref());
        let Some(wagon) = wagon else {
            return Answer::Manifest { route: wire::no(route), lines: Vec::new(), superbill: false };
        };
        let pallets = self.pallets();
        let mut lines = Vec::new();
        for l in &wagon.manifest.lines {
            if let Some(p) = pallets.get(l.pallet) {
                lines.push(ManifestLine { pallet: self.pallet_view(p), drop: l.drop.map(wire::no), container: None });
            }
        }
        for c in &wagon.manifest.containers {
            for p in pallets.at_place(At::Container(c.container)) {
                lines.push(ManifestLine { pallet: self.pallet_view(p), drop: c.drop.map(wire::no), container: Some(c.container) });
            }
        }
        Answer::Manifest { route: wire::no(route), lines, superbill: wagon.manifest.needs_superbill() }
    }

    /// `Input::Pack` and `Input::Stowed` (checked before the look applies).
    pub(crate) fn apply_pallet_host(&mut self, input: &wire::Input) {
        match input {
            wire::Input::Pack { town, consignee, goods } => {
                let goods: Vec<(GoodId, f64)> = goods.iter().map(|c| (c.good as GoodId, c.count)).collect();
                self.pack_for(*town as TownId, *consignee as TownId, &goods);
            }
            wire::Input::Stowed { point, pallet } => self.stowed(point, *pallet),
            _ => {}
        }
    }
}
