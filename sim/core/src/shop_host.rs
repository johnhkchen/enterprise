//! Shops, boards and rent deeds through the wire (`docs/binding-plan.md` 2, concern 9; task `shop-adapter`): what a host
//! needs of the core's shops to show them in its world, which is Numismatics vendors. The rules are the core's own
//! (`orders.rs`, `rules/orders.rs`: a master Merchant's brokering; `desires.rs`: the shop board's listings;
//! `harvest.rs`: rents and a dealer's price for them). This module lets a host tell the core what its vendors are and
//! says what each should show.
//!
//! - **A vendor counted** ([`Input::Vendor`](crate::wire::Input), [`Society::read_vendor`]): the host reports facts (its
//!   mode, filter, lot, price, the items in it, the room it has, the bank card's balance, the coin it took out of its
//!   box), and the core says what the vendor is. A buying vendor is a player's standing order ([`Order`], filled under a
//!   contract at the next look: `OrderFilled`, whose goods and spurs the host moves into and out of the vendor), a
//!   selling vendor a stall ([`Stall`]: `StallBought`), one touching the desk the town's own shop ([`OwnShop`]), one it
//!   was told is the board's the board's. They are kept by where they stand ([`Order::at`], [`Stall::at`],
//!   [`OwnShop::at`]) so the host's count each look updates them in place: the host's count is the truth for what the
//!   vendor holds (a purse, a stall's stock, a shelf, the room), the core's for what the town does about it.
//! - **The town's shops** (`Output::Shelf`): the town's own shop (the Merchant's surplus: `rules/orders.rs`
//!   `sell_surplus`) and its board's show a good, a lot and a price, with items added to the shelf (moved out of the
//!   town's stock by the core, so the chests are written the same look, `Moved`) and what the old listing held taken
//!   out (and back in the town's stock). Only what changed since it was last shown, and only for a vendor counted in
//!   the look. A shelf the host counted lower than the core's was bought from: used, as a player's buying is; coin the
//!   host collected from the box is the shop's takings, banked by the master Merchant (own) or at once (the board's).
//! - **The shop board** (`ShopBoard.keep`): the listings (`desires.rs`, the town's three most wanted goods) are the
//!   core's; the board's vendors (`Input::Board`, each paid four emeralds when the host puts one up) show them in order.
//!   A listed good the town holds beyond what it keeps for its own people and what other markets want of it goes on the
//!   shelf, a stack at most. When the town lists more than it has vendors for, can pay for one and has a road, the core
//!   asks for another (`Output::BuildShop`).
//! - **Rent deeds** (`RentDeeds`): a freight hub holds rents (`harvest.rs`) and sells their deeds to players at a vendor
//!   each ([`Input::DeedShop`](crate::wire::Input)): at its ask while the rent is its own; once a player bought it (the
//!   vendor sold out, its takings paid in) the vendor buys it back at the bid from a bank card the core says to fund
//!   (`Output::Deed`), and the hub collects the coupons in its keeping; a deed sold back makes it the hub's again. A
//!   player bringing the deed to the hub's desk is handed the crop collected for it, as far as the stock goes
//!   (`DeedBrought`, `Output::DeedPaid`). What a deed's document looks like is the host's.
//!
//! **Where the core differs from the mod.** Enchanted commissions have no counterpart (a vendor asking for enchantments
//! is `Idle`). The shop board's takings are booked in the town's books as a sale with no market. A vendor the host didn't
//! count (its chunk unloaded, or a look whose counts were skipped) is left as it is: a look deals only with the vendors
//! counted for it ([`Vendors::counted`], cleared as each `Advance` ends), as the mod's brokers skip a desk that isn't
//! loaded, so an order isn't filled, a stall isn't bought from and a shelf isn't stocked where the host can't show it.
//! The sovereign's, the metal exchange's, the requisitions' and the freight hubs' vendors are other tasks'
//! (`crown-views`).

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::cash_book::{Account, Reason};
use crate::content::GoodId;
use crate::fixed::Fixed;
use crate::harvest::RentState;
use crate::messages::TownId;
use crate::orders::{Order, OwnShop, Stall};
use crate::points::{Kind, Spot};
use crate::society::Society;
use crate::state::{Ledger, Stock};
use crate::wire::{no, Answer, Count, DeedLine, Input, ListingLine, Output, Point, ShopLine, ShopRole, SignalLine, TallyLine,
    VendorMode, VendorRead};

/// A vendor's brass and glass, in emeralds, paid by the town when the host puts one up for its board (`ShopBoard.VENDOR_EMERALDS`).
pub const VENDOR_EMERALDS: f64 = 4.0;

/// The vendors the core knows the role of beyond what a host's count says: where each rent's deed is sold.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vendors {
    pub deeds: BTreeMap<Spot, usize>,
    /// The vendors counted since the last `Advance` ended ([`Society::read_vendor`]): the only ones its looks deal with.
    #[serde(default)]
    pub counted: BTreeSet<Spot>,
}

impl Vendors {
    /// Whether a look may deal with a shop: one no host told the core of (`None`), or a vendor counted for it.
    pub fn dealt(&self, at: Option<&Spot>) -> bool {
        at.map_or(true, |at| self.counted.contains(at))
    }
}

/// Spurs, to the nearest thousandth of an emerald at the mint's rate.
fn worth(spurs: f64, rate: f64) -> Fixed {
    Fixed::round(spurs / rate)
}

impl Society {
    /// Applies a vendor happening; empty for any other input. The wire checks them first (`wire::handle`).
    pub(crate) fn apply_vendors(&mut self, input: &Input) -> Vec<Output> {
        match input {
            Input::Vendor { point, read } => self.read_vendor(&point.spot(), read),
            Input::Board { point } => {
                self.board_vendor(&point.spot());
                Vec::new()
            }
            Input::DeedShop { rent, point } => {
                self.deed_shop(*rent as usize, point.as_ref().map(|p| p.spot()));
                Vec::new()
            }
            Input::DeedBrought { town, rent } => self.deed_brought(*town as TownId, *rent as usize),
            _ => Vec::new(),
        }
    }

    fn orders_mut(&mut self) -> bevy_ecs::prelude::Mut<'_, crate::orders::Orders> {
        self.world.resource_mut::<crate::orders::Orders>()
    }

    /// A vendor counted: see the module's doc. Ignored for a point that isn't a shop of a town (a stale view).
    pub fn read_vendor(&mut self, at: &Spot, read: &VendorRead) -> Vec<Output> {
        let Some(placed) = self.point(at).filter(|p| p.kind == Kind::Shop) else { return Vec::new() };
        let town = placed.town;
        let rate = self.mint_rate();
        self.world.resource_mut::<Vendors>().counted.insert(at.clone());
        if self.is_crown_shop(at) {
            return self.read_crown_shop(at, town, read);
        }
        if let Some(&rent) = self.world.resource::<Vendors>().deeds.get(at) {
            return self.read_deed(at, town, rent, read, rate);
        }
        if let Some(i) = self.own_at(at) {
            let board = self.world.resource::<crate::orders::Orders>().shops[i].board;
            self.read_own(i, read, board, rate);
            return Vec::new();
        }
        // A new vendor: what it is is what the host found.
        match (read.touches, read.mode, read.good) {
            (true, _, _) => {
                self.forget_vendor(at);
                let id = self.own_shop(town);
                let at_i = self.world.resource::<crate::orders::Orders>().shops.iter().position(|s| s.id == id).expect("it was just made");
                self.orders_mut().shops[at_i].at = Some(at.clone());
                self.read_own(at_i, read, false, rate);
            }
            (false, VendorMode::Buy, Some(good)) if read.lot >= 1 => self.read_order(at, town, good as GoodId, read),
            (false, VendorMode::Sell, Some(good)) if read.lot >= 1 => self.read_stall(at, town, good as GoodId, read),
            _ => self.forget_vendor(at),
        }
        Vec::new()
    }

    fn own_at(&self, at: &Spot) -> Option<usize> {
        self.world.resource::<crate::orders::Orders>().shops.iter().position(|s| s.at.as_ref() == Some(at))
    }

    /// A buying vendor: its order is kept and updated, or drafted anew when the good, the lot or the price changed (the
    /// contract closes: what its installments deliver at what price is the contract, as `Broker.contractFor` matches it).
    fn read_order(&mut self, at: &Spot, town: TownId, good: GoodId, read: &VendorRead) {
        let same = self.world.resource::<crate::orders::Orders>().orders.iter()
            .find(|o| o.at.as_ref() == Some(at))
            .map(|o| (o.id, o.good == good && o.lot == (read.lot as f64).max(1.0) && o.price == read.price as f64));
        let id = match same {
            Some((id, true)) => id,
            other => {
                if let Some((id, _)) = other {
                    self.cancel_order(id);
                }
                self.forget_stall_or_own(at);
                self.order(town, good, read.lot as f64, read.price as f64, read.purse as f64)
            }
        };
        let mut orders = self.orders_mut();
        let o = orders.orders.iter_mut().find(|o| o.id == id).expect("an order that stands");
        o.at = Some(at.clone());
        o.lot = (read.lot as f64).max(1.0);
        o.price = read.price as f64;
        o.purse = read.purse as f64;
        o.room = Some(read.room as f64);
    }

    /// A selling vendor: its stall is kept and updated (its stock as the host counted it).
    fn read_stall(&mut self, at: &Spot, town: TownId, good: GoodId, read: &VendorRead) {
        let found = self.world.resource::<crate::orders::Orders>().stalls.iter().position(|s| s.at.as_ref() == Some(at));
        if found.is_none() {
            self.forget_vendor(at);
            self.stall(town, good, read.lot as f64, read.price as f64, read.stock as f64);
        }
        let mut orders = self.orders_mut();
        let i = found.unwrap_or(orders.stalls.len() - 1);
        let s = &mut orders.stalls[i];
        s.at = Some(at.clone());
        s.good = good;
        s.lot = (read.lot as f64).max(1.0);
        s.price = read.price as f64;
        s.stock = read.stock as f64;
    }

    /// A shop of the town's (its own or its board's) counted: what players bought and paid since is booked.
    fn read_own(&mut self, i: usize, read: &VendorRead, board: bool, rate: f64) {
        let (town, good, mut sold, mut added) = {
            let mut orders = self.orders_mut();
            let shop = &mut orders.shops[i];
            shop.takings += read.coins as f64;
            let (mut sold, mut added) = (0.0, 0.0);
            if let Some(g) = shop.good {
                if read.good == Some(g as u32) {
                    let seen = read.stock as f64;
                    if seen < shop.shelf {
                        sold = shop.shelf - seen;
                    } else {
                        added = seen - shop.shelf;
                    }
                    shop.shelf = seen;
                    // (shown as the vendor is: a lot or a price it has otherwise is shown again)
                    shop.shown = Some((Some(g), read.lot as f64, read.price as f64, seen));
                } else {
                    // It shows something else: shown again, the other goods in it cleared.
                    shop.shown = Some((None, 0.0, 0.0, 0.0));
                }
            }
            (shop.town, shop.good, sold, added)
        };
        if let Some(g) = good {
            let mut ledger = self.world.resource_mut::<Ledger>();
            ledger.used[g] += std::mem::take(&mut sold);
            ledger.delivered[g] += std::mem::take(&mut added);
        }
        if board {
            self.bank_board(i, town, rate);
        }
    }

    /// The board's takings are the town's at once, and in its books as a sale with no market (`ShopBoard.keep`).
    fn bank_board(&mut self, i: usize, town: TownId, rate: f64) {
        let (takings, good, lot, price) = {
            let mut orders = self.orders_mut();
            let shop = &mut orders.shops[i];
            (std::mem::take(&mut shop.takings), shop.good, shop.lot, shop.price)
        };
        if takings <= 0.0 {
            return;
        }
        let money = worth(takings, rate);
        self.post(Account::Outside, Account::Treasury(town), money, Reason::Other("shop".into()));
        if let Some(g) = good {
            let today = self.now() / self.content().day_ticks.max(1);
            let count = takings / price.max(1.0) * lot;
            self.file(town, vec![crate::atlas::Entry { day: today, kind: crate::atlas::Kind::Shop, with: None, good: Some(g),
                count, money, why: None }]);
        }
    }

    /// Takes a vendor out of whatever shop it was (its order closed, its stall gone, its own shop's shelf used).
    fn forget_vendor(&mut self, at: &Spot) {
        let order = self.world.resource::<crate::orders::Orders>().orders.iter().find(|o| o.at.as_ref() == Some(at)).map(|o| o.id);
        if let Some(id) = order {
            self.cancel_order(id);
        }
        self.forget_stall_or_own(at);
        self.world.resource_mut::<Vendors>().deeds.remove(at);
    }

    fn forget_stall_or_own(&mut self, at: &Spot) {
        let mut orders = self.orders_mut();
        orders.stalls.retain(|s| s.at.as_ref() != Some(at));
        let gone = orders.shops.iter().position(|s| s.at.as_ref() == Some(at));
        let shelf = gone.map(|i| orders.shops.remove(i)).and_then(|s| s.good.map(|g| (g, s.shelf)));
        drop(orders);
        if let Some((g, shelf)) = shelf {
            // What was on a shelf that's gone left with it: used, as a player's taking is.
            self.world.resource_mut::<Ledger>().used[g] += shelf;
        }
    }

    /// A vendor point broken: its shop goes (`remove_point`).
    pub(crate) fn forget_shop_point(&mut self, at: &Spot) {
        self.forget_crown_shop(at);
        self.forget_vendor(at);
    }

    /// A vendor the host put up for the town's board: paid for, and its shelf kept in order from now on.
    fn board_vendor(&mut self, at: &Spot) {
        let Some(placed) = self.point(at).filter(|p| p.kind == Kind::Shop) else { return };
        let town = placed.town;
        if let Some(i) = self.own_at(at) {
            self.orders_mut().shops[i].board = true;
            return;
        }
        self.forget_vendor(at);
        let id = self.own_shop(town);
        let cost = Fixed::from(VENDOR_EMERALDS).min(self.balance(Account::Treasury(town)).unwrap_or_default().at_least_zero());
        self.post(Account::Treasury(town), Account::Outside, cost, Reason::Other("a shop".into()));
        let mut orders = self.orders_mut();
        let shop = orders.shops.iter_mut().find(|s| s.id == id).expect("it was just made");
        shop.at = Some(at.clone());
        shop.board = true;
    }

    /// Where a rent's deed is sold: the vendor, or none.
    fn deed_shop(&mut self, rent: usize, at: Option<Spot>) {
        let mut vendors = self.world.resource_mut::<Vendors>();
        vendors.deeds.retain(|_, r| *r != rent);
        if let Some(at) = at {
            vendors.deeds.insert(at.clone(), rent);
            drop(vendors);
            // (A vendor that was another kind of shop isn't now.)
            let order = self.world.resource::<crate::orders::Orders>().orders.iter().find(|o| o.at.as_ref() == Some(&at)).map(|o| o.id);
            if let Some(id) = order {
                self.cancel_order(id);
            }
            self.forget_stall_or_own(&at);
        }
    }

    /// A deed's shop counted (`RentDeeds.quote`): bought, sold back, or its price moved.
    fn read_deed(&mut self, at: &Spot, town: TownId, rent: usize, read: &VendorRead, rate: f64) -> Vec<Output> {
        let point = Point::of(at, Kind::Shop);
        let Some(r) = self.world.resource::<crate::harvest::Rents>().rents.get(rent).cloned() else {
            self.world.resource_mut::<Vendors>().deeds.remove(at);
            return Vec::new();
        };
        if r.state != RentState::Active || r.holder != town {
            // The rent isn't this dealer's to quote any more: its shop stands empty.
            self.world.resource_mut::<Vendors>().deeds.remove(at);
            return vec![Output::Shelf { point, good: None, lot: 0, price: 0, add: 0, clear: true }];
        }
        let (now, dealer_spread) = (self.now(), self.content().dials.dealer_spread);
        let price = self.rent_price(town, rent);
        let ask = crate::harvest::ask(price, dealer_spread).to_f64();
        let bid = crate::harvest::bid(price, dealer_spread).to_f64();
        let (ask_spurs, bid_spurs) = ((ask * rate).ceil() as u32, (bid * rate).floor() as u32);
        let mine = read.deed == Some(rent as u32);
        if read.coins > 0 {
            // What the vendor took: the town's.
            self.post(Account::Outside, Account::Treasury(town), worth(read.coins as f64, rate), Reason::Other("deed".into()));
        }
        let mut bearer = r.bearer;
        if !bearer {
            if mine && read.mode == VendorMode::Sell && read.stock == 0 {
                // A player bought the deed: the takings are the town's, and the vendor buys it back from now on.
                let mut rents = self.world.resource_mut::<crate::harvest::Rents>();
                rents.set_bearer(rent, true);
                rents.rents[rent].paid = worth(read.coins as f64, rate);
                rents.rents[rent].traded = now;
                bearer = true;
            } else {
                let right = mine && read.mode == VendorMode::Sell && read.price == ask_spurs && read.stock >= 1;
                return if right { Vec::new() } else {
                    vec![Output::Deed { point, rent: rent as u32, selling: true, price: ask_spurs, fund: 0 }]
                };
            }
        }
        debug_assert!(bearer);
        if mine && read.stock >= 1 {
            // A player sold the deed back: the rent is the dealer's own again, and the vendor sells it.
            let mut rents = self.world.resource_mut::<crate::harvest::Rents>();
            rents.set_bearer(rent, false);
            rents.rents[rent].paid = Fixed::round(bid);
            rents.rents[rent].traded = now;
            return vec![Output::Deed { point, rent: rent as u32, selling: true, price: ask_spurs, fund: 0 }];
        }
        // The vendor buys the deed back at the bid, from a card the town keeps funded from its treasury, a little over
        // what's short (coin rounds down), if it can spare it.
        let mut fund = 0;
        if read.purse < bid_spurs as u64 {
            let short = ((bid_spurs as u64 - read.purse + 1) as f64 / rate * 1.05).max(0.0);
            let spare = self.spare_money(town).to_f64();
            if spare >= short {
                self.post(Account::Treasury(town), Account::Outside, Fixed::round(short), Reason::Other("deed".into()));
                fund = (short * rate).floor() as u64;
            }
        }
        if read.mode == VendorMode::Buy && read.price == bid_spurs && fund == 0 && mine {
            return Vec::new();
        }
        vec![Output::Deed { point, rent: rent as u32, selling: false, price: bid_spurs, fund }]
    }

    /// A town's money beyond its reserve (negative under it): what it may spend on a shop or a deed's card.
    pub fn spare_money(&self, town: TownId) -> Fixed {
        let e = self.world.entity(self.towns[town]);
        let reserve = crate::rules::farms::reserve(self.content(), e.get::<crate::state::Trades>().expect("a town"),
            e.get::<crate::farms::Farm>());
        e.get::<crate::state::Money>().expect("a town").0 - reserve
    }

    /// A player brought a rent's deed to its holder's desk: the crop collected for it handed over, as far as the stock goes.
    fn deed_brought(&mut self, town: TownId, rent: usize) -> Vec<Output> {
        let Some(r) = self.world.resource::<crate::harvest::Rents>().rents.get(rent).cloned() else { return Vec::new() };
        if !r.bearer || r.holder != town || r.state != RentState::Active {
            return Vec::new();
        }
        let held = self.world.get::<Stock>(self.towns[town]).expect("a town").0[r.crop].floor();
        let handed = self.world.resource_mut::<crate::harvest::Rents>().take_accrued(rent, r.accrued.min(held).floor());
        if handed > 0.0 {
            self.world.get_mut::<Stock>(self.towns[town]).expect("a town").0[r.crop] -= handed;
            self.world.resource_mut::<Ledger>().used[r.crop] += handed;
        }
        let owed = self.world.resource::<crate::harvest::Rents>().rents[rent].accrued.ceil().max(0.0) as u32;
        vec![Output::DeedPaid { town: no(town), rent: no(rent), good: no(r.crop), count: handed as u32, owed }]
    }

    /// What the towns' shops show now, after a look's counts (`wire::handle`): the boards' shelves stocked with what each
    /// town can spare of what it lists, another vendor asked for where there's cause, and the shops counted in this look
    /// (`read`) told what they show if it changed since they were shown.
    pub fn show_shops(&mut self, towns: &[TownId], read: &[Spot]) -> Vec<Output> {
        let mut out = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for &t in towns {
            if t < self.town_count() && seen.insert(t) {
                out.extend(self.keep_board(t, read));
            }
        }
        for at in read {
            let Some(i) = self.own_at(at) else { continue };
            let shop = self.world.resource::<crate::orders::Orders>().shops[i].clone();
            let cur = (shop.good, shop.lot, shop.price, shop.shelf);
            if shop.shown == Some(cur) || (!shop.board && shop.good.is_none() && shop.shown.is_none()) {
                continue;
            }
            let same = shop.shown.is_some_and(|s| s.0.is_some() && s.0 == shop.good);
            let add = shop.shelf - if same { shop.shown.map_or(0.0, |s| s.3) } else { 0.0 };
            let clear = shop.shown.is_some() && !same;
            out.push(Output::Shelf { point: Point::of(at, Kind::Shop), good: shop.good.map(no), lot: shop.lot as u32,
                price: shop.price as u32, add: add as i64, clear });
            self.orders_mut().shops[i].shown = Some(cur);
        }
        out
    }

    /// A town's board: each vendor of it shows the listing at its place, the listed good the town can spare goes on its
    /// shelf (a stack at most); and a vendor more is asked for when the town lists more than it has vendors.
    fn keep_board(&mut self, town: TownId, read: &[Spot]) -> Vec<Output> {
        let ids: Vec<usize> = {
            let orders = self.world.resource::<crate::orders::Orders>();
            (0..orders.shops.len()).filter(|&i| orders.shops[i].town == town && orders.shops[i].board).collect()
        };
        let listings = self.listings(town);
        let rate = self.mint_rate();
        for (k, &i) in ids.iter().enumerate() {
            let Some(l) = listings.get(k) else {
                // Nothing wanted: the shop stands empty, and what was on its shelf is the town's again.
                let (good, shelf) = {
                    let mut orders = self.orders_mut();
                    let s = &mut orders.shops[i];
                    let was = (s.good.take(), std::mem::take(&mut s.shelf));
                    s.lot = 0.0;
                    s.price = 0.0;
                    was
                };
                if let Some(g) = good {
                    self.world.get_mut::<Stock>(self.towns[town]).expect("a town").0[g] += shelf;
                }
                continue;
            };
            let spurs = (l.each * l.lot * rate).ceil().max(1.0);
            let (g, lot) = (l.good, l.lot);
            let relist = {
                let o = self.world.resource::<crate::orders::Orders>();
                o.shops[i].good != Some(g) || o.shops[i].lot != lot
            };
            if relist {
                let (old, shelf) = {
                    let mut orders = self.orders_mut();
                    let s = &mut orders.shops[i];
                    let was = (s.good.replace(g), std::mem::take(&mut s.shelf));
                    s.lot = lot;
                    was
                };
                if let Some(old) = old {
                    self.world.get_mut::<Stock>(self.towns[town]).expect("a town").0[old] += shelf;
                }
            }
            self.orders_mut().shops[i].price = spurs;
            // What's come in goes on the shelf at that price: a stack's worth, beyond what the town keeps for itself
            // and what the markets its wagons supply want of it.
            let keep = {
                let content = self.content();
                match content.goods[g].comfort {
                    None => f64::INFINITY, // a good its lockers use: none for sale
                    Some(_) => {
                        let people = self.world.get::<crate::state::People>(self.towns[town]).expect("a town");
                        (crate::people::comfort_keep(content, people) / content.goods[g].value.max(1e-9)).ceil()
                    }
                }
            };
            if !self.world.resource::<crate::orders::Orders>().shops[i].at.as_ref().is_some_and(|at| read.contains(at)) {
                continue; // (a vendor not counted in this look isn't stocked: the host couldn't show it)
            }
            let outlet = self.outlet(town, None, g).map_or(0.0, |o| o.units);
            let stack = self.content().goods[g].stack as f64;
            let held = self.world.get::<Stock>(self.towns[town]).expect("a town").0[g].floor();
            let on_shelf = self.world.resource::<crate::orders::Orders>().shops[i].shelf;
            let spare = (held - keep - outlet).max(0.0);
            let top = (stack - on_shelf).min(spare);
            if top > 0.0 {
                self.world.get_mut::<Stock>(self.towns[town]).expect("a town").0[g] -= top;
                self.orders_mut().shops[i].shelf += top;
            }
        }
        // Another vendor, if the town lists more than it has and can pay for one.
        let mut out = Vec::new();
        if ids.len() < crate::desires::SHOP_SIZE && listings.len() > ids.len() && self.spare_money(town).to_f64() >= VENDOR_EMERALDS {
            out.push(Output::BuildShop { town: no(town) });
        }
        out
    }

    /// The rents a town holds as a dealer, with the price of each deed and where it's sold (`Query::Deeds`).
    pub fn deed_lines(&self, town: TownId) -> Answer {
        let hub = self.is_hub(town);
        let rate = self.mint_rate();
        let spread = self.content().dials.dealer_spread;
        let rents = self.world.resource::<crate::harvest::Rents>();
        let vendors = self.world.resource::<Vendors>();
        let mut deeds = Vec::new();
        if hub {
            for id in rents.held_by(town) {
                let r = &rents.rents[id];
                let price = self.rent_price(town, id);
                deeds.push(DeedLine { rent: no(id), farm: no(r.farm), farm_name: self.town(r.farm).name, crop: no(r.crop), quantity: r.quantity,
                    ask: (crate::harvest::ask(price, spread).to_f64() * rate).ceil() as u32,
                    bid: (crate::harvest::bid(price, spread).to_f64() * rate).floor() as u32, bearer: r.bearer,
                    accrued: r.accrued,
                    point: vendors.deeds.iter().find(|(_, &r)| r == id).map(|(at, _)| Point::of(at, Kind::Shop)) });
            }
        }
        Answer::Deeds { town: no(town), hub, deeds }
    }

    /// A town's shops, what its open orders want, what it has posted, what its board lists and its trade (`Query::Shops`).
    pub fn shop_lines(&self, town: TownId) -> Answer {
        let rate = self.mint_rate();
        let orders = self.world.resource::<crate::orders::Orders>();
        let vendors = self.world.resource::<Vendors>();
        let mut shops: BTreeMap<Spot, ShopLine> = BTreeMap::new();
        let blank = |at: &Spot, role| ShopLine { point: Point::of(at, Kind::Shop), role, good: None, lot: 0.0, price: 0.0,
            purse: 0.0, open: 0.0, delivered: 0.0, paid: 0.0, stock: 0.0, rent: None };
        for (at, p) in &self.points().0 {
            if p.kind == Kind::Shop && p.town == town {
                shops.insert(at.clone(), blank(at, ShopRole::Idle));
            }
        }
        for o in orders.orders.iter().filter(|o| o.town == town) {
            if let Some(line) = o.at.as_ref().and_then(|at| shops.get_mut(at)) {
                *line = ShopLine { role: ShopRole::Order, good: Some(no(o.good)), lot: o.lot, price: o.price, purse: o.purse,
                    open: o.open, delivered: o.delivered, paid: o.paid, ..line.clone() };
            }
        }
        for s in orders.stalls.iter().filter(|s| s.town == town) {
            if let Some(line) = s.at.as_ref().and_then(|at| shops.get_mut(at)) {
                *line = ShopLine { role: ShopRole::Stall, good: Some(no(s.good)), lot: s.lot, price: s.price, stock: s.stock,
                    ..line.clone() };
            }
        }
        for s in orders.shops.iter().filter(|s| s.town == town) {
            if let Some(line) = s.at.as_ref().and_then(|at| shops.get_mut(at)) {
                *line = ShopLine { role: if s.board { ShopRole::Board } else { ShopRole::Own }, good: s.good.map(no), lot: s.lot,
                    price: s.price, stock: s.shelf, paid: s.takings, ..line.clone() };
            }
        }
        for (at, &rent) in &vendors.deeds {
            if let Some(line) = shops.get_mut(at) {
                line.role = ShopRole::Deed;
                line.rent = Some(no(rent));
            }
        }
        let listings = self.listings(town).iter().map(|l| ListingLine { good: no(l.good), lot: l.lot as u32,
            spurs: (l.each * l.lot * rate).ceil().max(1.0) as u32 }).collect();
        let t = self.tally(town);
        Answer::Shops {
            town: no(town),
            shops: shops.into_values().collect(),
            wanted: self.order_wants(town).into_iter().map(|(g, c)| Count { good: no(g), count: c }).collect(),
            signals: self.posted_signals(town).into_iter().map(|s| SignalLine { good: no(s.good), count: s.count,
                spurs_each: s.spurs_each, why: s.why }).collect(),
            listings,
            tally: TallyLine { lots_sold: t.lots_sold, spurs_from_sales: t.spurs_from_sales, lots_bought: t.lots_bought,
                spurs_on_purchases: t.spurs_on_purchases, spurs_from_own_shops: t.spurs_from_own_shops },
        }
    }
}

// (An order, a stall and an own shop are the orders module's types: named here for the docs.)
#[allow(unused)]
type _Shops = (Order, Stall, OwnShop);
