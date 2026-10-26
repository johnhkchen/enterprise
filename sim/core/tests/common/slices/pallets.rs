//! Pallets in the random worlds (`pallet.rs`, `docs/pallets.md` section 5 "Invariants for the random-worlds sweep"):
//! every day, in every world, each invariant of the registry, and a host's doings that bring pallets in.
//!
//! The checks are plain functions over a [`View`] (the registry and the facts about the world it is checked against),
//! so a test can hand them a registry it has broken on purpose (`tests/pallet_sweep.rs`: a pallet indexed twice, a
//! sealed pallet changed...) and see each one fail with a message that says what is wrong:
//!
//! - **Packages** ([`packages`]): every package on exactly one pallet (they are inside it) or in one open lot; no
//!   pallet empty; every package's count positive (a returnable crate empty is not a pallet's) and finite, its kind and
//!   good ones the content has, goods of one handling a pallet (since `packing`: goods of one key share a pallet).
//! - **Bills** ([`bills`]): every pallet on one bill whose consignee and shipper are towns, whose contract (if any) is in
//!   the book, whose `Payment::ByBill` bill exists and names this pallet; no two bills of exchange name one pallet.
//! - **Places** ([`places`]): `by_place` matches each pallet's `at`, and nothing else; a pallet `At::Wagon(r)` only while
//!   route `r` has a wagon on the road; `At::Custody(h)` only at a hub, with a custody record; `At::Container(c)` only in
//!   a consolidation that exists; no pallet at its own consignee unopened.
//! - **Seals** ([`seals`]): a pallet on a carrier, in a consolidation or in custody is sealed, and a sealed pallet's digest
//!   is its seal's.
//! - **Caps** ([`wagon_caps`], [`pallet_caps`]): every wagon's load (loose cargo, pallets) within its carrier's slots,
//!   weight and pallets; every pallet within its kind's slots and weight (switched on by [`CAPS_BIND`] since `pallet-caps`).
//! - **Manifests** ([`manifests`]): a wagon's open manifest lines are exactly the pallets and consolidations aboard, once
//!   each; a consolidation's lines are exactly its pallets, two or more on freight tickets, and share one next stop, the one
//!   its line on the wagon says; every consolidation is aboard one wagon on the road (since `superbills`).
//! - **Superbills** ([`superbills`]): every consolidation in the registry had its superbill drawn at its issuer's desk.
//! - **Goods conserved:** `check.rs` counts the pallets once, from the registry; the harness runs `conserved` each day.
//!
//! Later tasks add a line each where marked `// later:` below (claims, superbills, losses).

use super::super::*;
use enterprise_core::cargo::{one_pallet, Lading, Payment};
use enterprise_core::content::Content;
use enterprise_core::pallet::{At, Caps, Load, Manifest, Pallet, PalletId, Pallets};
use enterprise_core::State;
use std::collections::{BTreeMap, BTreeSet};

/// Whether the caps `pallet-caps` brings are checked: a pallet's own slots and weight (its kind's), and a wagon's load
/// against its slots, weight and pallets. Before that task a town crated a whole sale in one pallet of however many crates,
/// a heavy good filled no wagon by weight, a porter run carried as many pallets as there were, and several paths loaded a
/// wagon past its stacks (see [`wagon_caps`]). `pallet-caps` fits them all and turns this on (the test
/// `the_worlds_that_went_over_their_caps_keep_within_them` replays the worlds that went over).
pub const CAPS_BIND: bool = true;

/// A wagon on the road, as the checks see it.
#[derive(Clone, Debug)]
pub struct WagonView {
    pub manifest: Manifest,
    /// Loose cargo by good.
    pub cargo: Vec<f64>,
    /// What its carrier holds at most: the stacks the wagon was sent out with (`CrewView.slots`, when set), the weight and
    /// pallets of [`Caps::of`] its crew.
    pub caps: Caps,
}

/// The registry and what it is checked against: plain data, so a test can build one by hand or break a world's.
#[derive(Clone, Debug)]
pub struct View<'a> {
    /// Superbills each town has drawn, all told (`Paperwork.by_kind`).
    pub superbills: BTreeMap<usize, u64>,
    pub content: &'a Content,
    pub pallets: &'a Pallets,
    pub towns: usize,
    /// Ids of the contracts in the book.
    pub contracts: BTreeSet<usize>,
    /// Every bill of exchange by id, with the pallets it names.
    pub bills: Vec<Vec<PalletId>>,
    /// The routes with a wagon on the road.
    pub wagons: BTreeMap<usize, WagonView>,
    /// The pallets each hub keeps custody records of.
    pub custody: BTreeMap<usize, BTreeSet<PalletId>>,
    pub hubs: BTreeSet<usize>,
    /// Packages in lots for sale on wagons (open lots), by route.
    pub lots: BTreeMap<usize, Vec<Vec<enterprise_core::pallet::Package>>>,
}

impl<'a> View<'a> {
    /// A world as it stands.
    pub fn of(s: &'a Society) -> View<'a> {
        let content = s.content();
        let mut wagons = BTreeMap::new();
        let mut lots = BTreeMap::new();
        for route in 0..s.route_count() {
            let Some(manifest) = s.wagon_manifest(route) else { continue };
            let crew = s.crew(route);
            let trades = crew.crew.iter().filter_map(|(name, _)| content.trade(name));
            let mut caps = Caps::of(content, trades, crew.porter, crew.llamas);
            if crew.slots > 0 {
                caps.slots = crew.slots;
            }
            let cargo = s.wagon_cargo(route).unwrap_or_default();
            wagons.insert(route, WagonView { manifest, cargo, caps });
            let (_, open, _) = s.wagon_pallets(route);
            lots.insert(route, open.into_iter().map(|l| l.packages).collect());
        }
        View {
            superbills: (0..s.town_count())
                .map(|t| (t, s.paperwork(t).by_kind.get(enterprise_core::cargo::SUPERBILL).copied().unwrap_or(0))).collect(),
            content,
            pallets: s.pallets(),
            towns: s.town_count(),
            contracts: s.contracts().iter().map(|c| c.id).collect(),
            bills: s.bills().iter().map(|b| b.pallets.clone()).collect(),
            wagons,
            custody: (0..s.town_count()).map(|t| (t, s.hub(t).custody.keys().copied().collect())).collect(),
            hubs: (0..s.town_count()).filter(|&t| s.is_hub(t)).collect(),
            lots,
        }
    }
}

fn counted(n: f64) -> bool {
    n > 0.0 && n.is_finite()
}

/// Every package on one pallet or in one open lot: none empty, counts positive and finite, goods of one handling a pallet.
pub fn packages(v: &View) -> Result<(), String> {
    let goods = v.content.goods.len();
    for p in v.pallets.pallets.values() {
        if p.packages.is_empty() {
            return Err(format!("pallet {} holds no package: {p:?}", p.id));
        }
        for (i, pk) in p.packages.iter().enumerate() {
            if !counted(pk.count) {
                return Err(format!("pallet {}'s package {i} holds {} (a count is positive and finite)", p.id, pk.count));
            }
            if pk.good >= goods || pk.kind >= v.content.packages.len() {
                return Err(format!("pallet {}'s package {i} is of good {} in kind {}, which the content lacks", p.id, pk.good, pk.kind));
            }
            // (Goods of one key may share a pallet since `packing`; the key includes the goods' handling, so those that need
            // care never share a pallet with those that don't.)
            if v.content.goods[pk.good].handling != v.content.goods[p.good()].handling {
                return Err(format!("pallet {} mixes handling: package {i} is of {} ({:?}), the pallet's first is {} ({:?})", p.id,
                    pk.good, v.content.goods[pk.good].handling, p.good(), v.content.goods[p.good()].handling));
            }
        }
        if p.kind >= v.content.pallet_kinds.len() {
            return Err(format!("pallet {} is of kind {}, which the content lacks", p.id, p.kind));
        }
    }
    for (route, lots) in &v.lots {
        for (n, lot) in lots.iter().enumerate() {
            if lot.is_empty() || lot.iter().any(|pk| !counted(pk.count)) {
                return Err(format!("route {route}'s lot {n} for sale holds an empty or uncounted package: {lot:?}"));
            }
        }
    }
    Ok(())
}

/// Every pallet on one bill: towns for its parties, a contract in the book, a bill of exchange that names it back.
pub fn bills(v: &View) -> Result<(), String> {
    for p in v.pallets.pallets.values() {
        let l = &p.lading;
        if l.consignee >= v.towns || l.shipper >= v.towns {
            return Err(format!("pallet {}'s bill is from town {} to town {}, of {} towns", p.id, l.shipper, l.consignee, v.towns));
        }
        if let Some(c) = l.contract.filter(|c| !v.contracts.contains(c)) {
            return Err(format!("pallet {}'s bill names contract {c}, which isn't in the book", p.id));
        }
        if let Payment::ByBill(b) = l.payment {
            match v.bills.get(b) {
                None => return Err(format!("pallet {} is paid by bill {b}, which doesn't exist ({} bills)", p.id, v.bills.len())),
                Some(named) if !named.contains(&p.id) => {
                    return Err(format!("pallet {} is paid by bill {b}, which names {named:?}, not it", p.id));
                }
                Some(_) => {}
            }
        }
    }
    let mut named: BTreeMap<PalletId, usize> = BTreeMap::new();
    for (b, pallets) in v.bills.iter().enumerate() {
        for id in pallets.iter().filter(|id| v.pallets.get(**id).is_some()) {
            if let Some(first) = named.insert(*id, b) {
                return Err(format!("pallet {id} is named by two bills of exchange, {first} and {b}"));
            }
        }
    }
    Ok(())
}

/// Every pallet in one place, and the index of places is exactly the pallets.
pub fn places(v: &View) -> Result<(), String> {
    let r = v.pallets;
    let mut seen: BTreeSet<PalletId> = BTreeSet::new();
    for (place, ids) in &r.by_place {
        if ids.is_empty() {
            return Err(format!("{place:?} is indexed with no pallets (an empty place isn't kept)"));
        }
        for id in ids {
            if !seen.insert(*id) {
                return Err(format!("pallet {id} is indexed twice (second time at {place:?})"));
            }
            match r.get(*id) {
                None => return Err(format!("pallet {id} is indexed at {place:?}, and isn't in the registry")),
                Some(p) if p.at != *place => {
                    return Err(format!("pallet {id} is indexed at {place:?}, but it's at {:?}", p.at));
                }
                Some(_) => {}
            }
        }
    }
    if let Some(p) = r.pallets.values().find(|p| !seen.contains(&p.id)) {
        return Err(format!("pallet {} is in the registry at {:?}, but in no place's index", p.id, p.at));
    }
    for p in r.pallets.values() {
        if p.id >= r.next {
            return Err(format!("pallet {} is at or past the next id, {}", p.id, r.next));
        }
        match p.at {
            At::Town(t) | At::Custody(t) if t >= v.towns => {
                return Err(format!("pallet {} is at {:?}, of {} towns", p.id, p.at, v.towns));
            }
            At::Town(t) if p.consigned_to(t) => {
                return Err(format!("pallet {} waits at its own consignee, town {t}, unopened", p.id));
            }
            At::Custody(h) => {
                if !v.hubs.contains(&h) {
                    return Err(format!("pallet {} is in custody at town {h}, which is no hub", p.id));
                }
                if !v.custody.get(&h).is_some_and(|c| c.contains(&p.id)) {
                    return Err(format!("pallet {} is in hub {h}'s custody, with no custody record", p.id));
                }
            }
            At::Wagon(route) if !v.wagons.contains_key(&route) => {
                return Err(format!("pallet {} is aboard route {route}'s wagon, which isn't on the road", p.id));
            }
            At::Container(c) if !r.containers.contains_key(&c) => {
                return Err(format!("pallet {} is in container {c}, which doesn't exist", p.id));
            }
            _ => {}
        }
    }
    for (h, records) in &v.custody {
        if let Some(id) = records.iter().find(|id| r.get(**id).is_none_or(|p| p.at != At::Custody(*h))) {
            return Err(format!("hub {h} keeps a custody record of pallet {id}, which it doesn't hold"));
        }
    }
    Ok(())
}

/// Sealed pallets unchanged between premises: on a carrier or in custody, sealed, and the digest the seal's.
pub fn seals(v: &View) -> Result<(), String> {
    for p in v.pallets.pallets.values() {
        if matches!(p.at, At::Wagon(_) | At::Custody(_) | At::Container(_)) && p.seal.is_none() {
            return Err(format!("pallet {} is at {:?} and isn't sealed", p.id, p.at));
        }
        if !p.intact() {
            return Err(format!(
                "pallet {} is at {:?}, changed since it was sealed (seal {:?}, digest now {})", p.id, p.at, p.seal, p.digest()));
        }
    }
    Ok(())
}

/// What a wagon holds: its loose cargo, its pallets, and its lots for sale (a lot takes the stacks and weight of its
/// packages, as `cargo::stacks` reckons it).
fn load_of<'a>(v: &View, route: usize, w: &WagonView, aboard: impl IntoIterator<Item = &'a Pallet>) -> Load {
    let mut cargo = w.cargo.clone();
    cargo.resize(v.content.goods.len(), 0.0);
    let mut load = Load::of(v.content, &cargo, aboard);
    for pk in v.lots.get(&route).into_iter().flatten().flatten() {
        load.slots += pk.slots(v.content);
        load.weight += pk.weight(v.content);
    }
    load
}

/// Every wagon's load (loose cargo, pallets, lots for sale) within its carrier's slots, weight and pallets. Switched on by
/// [`CAPS_BIND`]. Before `pallet-caps` the rules held a wagon to its stacks at planning only, and these paths put more
/// aboard than it held: `stranded::load` loaded a stranded pallet while the wagon had room *left* (`room > 0`, the Java's
/// `loadStranded`), the Ex Works collection (`contracts::collect`) whatever room was left, the goods owed went without a
/// room (`plan_owed`), a hub's wagon bought a town's stranded pallets whatever its room (`buy_at_town`), shopping at the
/// far end bought past the slots, and a porter run took several pallets.
pub fn wagon_caps(v: &View) -> Result<(), String> {
    for (route, w) in &v.wagons {
        let load = load_of(v, *route, w, v.pallets.aboard(*route, &w.manifest));
        if !w.caps.holds(&load) {
            return Err(format!("route {route}'s wagon carries {load:?}, over its carrier's {:?}", w.caps));
        }
    }
    Ok(())
}

/// Every pallet within its kind's slots and weight (when [`CAPS_BIND`]; callable regardless).
pub fn pallet_caps(v: &View) -> Result<(), String> {
    let d = &v.content.dials;
    for p in v.pallets.pallets.values() {
        let kind = &v.content.pallet_kinds[p.kind];
        let weight: f64 = p.packages.iter().map(|pk| pk.weight(v.content)).sum();
        if p.slots(v.content) > kind.slots(d) || weight > kind.weight(d) + 1e-9 {
            return Err(format!("pallet {} takes {} slots and weighs {weight}, over its kind {:?}'s {} and {}",
                p.id, p.slots(v.content), kind.name, kind.slots(d), kind.weight(d)));
        }
    }
    Ok(())
}

/// A manifest is the load: a wagon's open lines are exactly the pallets and consolidations aboard (once each); a
/// consolidation's lines are its pallets, two or more on freight tickets, sharing one next stop, the one its line on the wagon
/// says; and every consolidation is aboard one wagon (`superbills`: made up as a run loads, dissolved at its stop).
pub fn manifests(v: &View) -> Result<(), String> {
    for (route, w) in &v.wagons {
        let mut lines: Vec<PalletId> = w.manifest.lines.iter().map(|l| l.pallet).collect();
        let mut aboard: Vec<PalletId> = v.pallets.at(At::Wagon(*route)).to_vec();
        lines.sort_unstable();
        aboard.sort_unstable();
        if let Some(pair) = lines.windows(2).find(|x| x[0] == x[1]) {
            return Err(format!("route {route}'s manifest lists pallet {} twice", pair[0]));
        }
        if lines != aboard {
            return Err(format!("route {route}'s manifest lists {lines:?}, but {aboard:?} are aboard"));
        }
    }
    for (c, container) in &v.pallets.containers {
        let mut lines: Vec<PalletId> = container.manifest.lines.iter().map(|l| l.pallet).collect();
        let mut held: Vec<PalletId> = v.pallets.at(At::Container(*c)).to_vec();
        lines.sort_unstable();
        held.sort_unstable();
        if lines != held {
            return Err(format!("container {c}'s manifest lists {lines:?}, but {held:?} are in it"));
        }
        let stops: BTreeSet<_> = container.manifest.lines.iter().map(|l| l.drop).collect();
        if stops.len() > 1 {
            return Err(format!("container {c}'s pallets drop at {stops:?}, not one next stop"));
        }
        if lines.len() < 2 {
            return Err(format!("container {c} holds {lines:?}: a consolidation is of two pallets or more"));
        }
        if let Some(l) = container.manifest.lines.iter().find(|l| l.booking.is_none() || l.for_forwarder) {
            return Err(format!("container {c}'s line {l:?} isn't on freight tickets (a hub consolidates what's booked)"));
        }
    }
    // The consolidations aboard each wagon: lines of its manifest, each once and on one wagon, each in the registry and
    // coming off where its pallets drop.
    let mut on: BTreeMap<u64, usize> = BTreeMap::new();
    for (route, w) in &v.wagons {
        for cl in &w.manifest.containers {
            if let Some(first) = on.insert(cl.container, *route) {
                return Err(format!("container {} is on route {first}'s manifest and route {route}'s", cl.container));
            }
            let Some(container) = v.pallets.containers.get(&cl.container) else {
                return Err(format!("route {route}'s manifest lists container {}, which doesn't exist", cl.container));
            };
            if let Some(l) = container.manifest.lines.iter().find(|l| l.drop != cl.drop) {
                return Err(format!("container {} comes off route {route}'s wagon at {:?}, but its pallet {} drops at {:?}",
                    cl.container, cl.drop, l.pallet, l.drop));
            }
        }
    }
    if let Some(c) = v.pallets.containers.keys().find(|c| !on.contains_key(c)) {
        return Err(format!("container {c} is aboard no wagon on the road"));
    }
    Ok(())
}

/// Every consolidation had its superbill drawn at its issuer's: at least as many superbills drawn there, all told, as it
/// has consolidations still in the registry.
pub fn superbills(v: &View) -> Result<(), String> {
    let mut issued: BTreeMap<usize, u64> = BTreeMap::new();
    for c in v.pallets.containers.values() {
        *issued.entry(c.issuer).or_insert(0) += 1;
    }
    for (town, n) in issued {
        let drawn = v.superbills.get(&town).copied().unwrap_or(0);
        if drawn < n {
            return Err(format!("town {town} issued {n} consolidations still aboard, but drew {drawn} superbills"));
        }
    }
    Ok(())
}

/// Every invariant, in the order they are listed above.
pub fn all(v: &View) -> Result<(), String> {
    packages(v)?;
    bills(v)?;
    places(v)?;
    seals(v)?;
    if CAPS_BIND {
        wagon_caps(v)?;
        pallet_caps(v)?;
    }
    manifests(v)?;
    superbills(v)?;
    // later: pallet-claims (a claimed pallet is on one route's plan, and that route's town holds it),
    // later: pallet-messages (a lost pallet's bill is unbacked, and no lost pallet stays in the registry).
    Ok(())
}

/// In about a third of the worlds a content pack's heavy good: a stack of it weighs 4, so the weight caps bind once they
/// are enforced (the weight dials' defaults count a stack as 1).
pub fn extend(s: &mut Society, r: &mut Rng) {
    if r.below(3) != 0 {
        return;
    }
    let good = r.below(s.content().goods.len() as u64) as usize;
    let mut content = s.content_mut();
    let stack = content.goods[good].stack.max(1) as f64;
    content.goods[good].weight = 4.0 / stack;
}

/// A host's doings: a player brings a whole pallet, one a different way each time it is told how it is paid for (owed,
/// paid, by a bill of exchange) or made out to another town than the one it was brought to.
// (pallet-messages' part of section 5's "bring, split and lose", a `PackageBrought` of one package and a `PackageLost`,
// is in `pallet_types.rs`'s daily doings; this brings whole pallets only.)
// (`deliver_pallet` takes a pallet under a contract whatever its good: one of another good moves the wrong good on the
// contract's book and breaks conservation, so the owed doing below crates the contract's good. pallet-messages should
// refuse such a message at the core's boundary rather than trust the host.)
pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    let n_towns = s.town_count();
    if n_towns < 2 || r.below(3) != 0 {
        return;
    }
    let (at, mut to) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
    if at == to {
        to = (to + 1) % n_towns;
    }
    let good = r.below(s.content().goods.len() as u64) as usize;
    let count = (r.range(0.0, 300.0).floor()).max(1.0);
    let packages = one_pallet(s.content(), good, count);
    let now = s.now();
    let kind = r.below(4);
    match kind {
        0 => {
            // Owed: the buyer's contract in force, if there is one between the two; else just made out to `to`.
            let c = s.contracts().into_iter().find(|c| c.state == State::Active && c.seller == at && c.buyer == to);
            let (lading, packages) = match c {
                // (Its goods are the contract's: the book counts them against it.)
                Some(c) => (Lading { contract: Some(c.id), copy: true, ..Lading::new(at, to, now) },
                    one_pallet(s.content(), c.good, count)),
                None => (Lading::new(at, to, now), packages),
            };
            s.deliver_pallet(at, lading, packages);
        }
        1 => {
            let lading = Lading { payment: Payment::Paid, ..Lading::new(at, to, now) };
            s.deliver_pallet(at, lading, packages);
        }
        2 => {
            // Paid for by a bill of exchange on its consignee, which names it.
            let bill = s.draw_bill(to, at, r.range(2.0, 40.0));
            let lading = Lading { payment: Payment::ByBill(bill), ..Lading::new(at, to, now) };
            if let Some(id) = s.deliver_pallet(at, lading, packages) {
                s.bill_pallets(bill, &[id]);
            }
        }
        _ => {
            // Brought to its own consignee: received at once, never in the registry.
            let lading = Lading { payment: Payment::Paid, ..Lading::new(at, at, now) };
            s.deliver_pallet(at, lading, packages);
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    all(&View::of(s))
}
