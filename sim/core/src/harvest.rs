//! Forward crop sales, rents and a dealer in them, as the mod keeps them (`contract/Harvest.java` `windowTicks`,
//! `carried`; `contract/ContractBook.java` `review` (the harvest branch) and `close`; `contract/Rents.java` `capital`,
//! `sell`, `review`, `issue`, `setBearer`, `takeAccrued`, `redeem`; `caravan/ForwardSales.java` `offer`, `setAside`,
//! `promised`, `boughtForward`; `market/RentDealer.java` `price`; `market/RentDeeds.java` `quote`).
//!
//! A forward sale is a [`Contract`](crate::contracts::Contract) with a [`Harvest`] term: each installment is judged
//! `window_days` after its day, not after the grace; a short one rolls onto the next and costs a fraction of a miss
//! in trust; only arrears past `harvest_arrears` installments, or what is still owed at the last, are misses (the
//! judging is in `contracts.rs`, `Book::review_with`). A rent ([`Rents`]) is the same paper sold for a capital: the
//! holder pays the farm eight seasons' crop once, the farm owes a coupon (a one-installment contract already paid
//! for) each season, a season's shortfall rides on the next coupon, and arrears past two seasons are a default.
//!
//! The farms are the core's own (`farms.rs`): the crops are [`farms::field_crops`], a farm sells forward only with a
//! farm plan, what it could grow is [`could_grow`], and what it has sold forward is its Farmers' first need ([`sold`]).
//! Stood in for: a market's wants for the dealer's price are what its people use beyond what it makes. The dealer's
//! deals in rents between towns, the crates a farm earmarks its harvest in, and the player's deed at a vendor
//! (Numismatics) are left to later slices; the dealer's price, ask and bid are here.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::contracts::{Book, Contract, Incoterm, State};
use crate::dials::Dials;
use crate::farms::{Farm, YIELD_PER_PLOT_PER_DAY};
use crate::content::{Content, GoodId};
use crate::messages::TownId;

/// A harvest term (`Harvest.java`): installments delivered within `window_days` of their day, shortfalls rolled over.
/// `carried`: a rent's coupon, whose shortfall the rent carries on (it always rolls, the last installment too).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Harvest {
    pub window_days: u32,
    pub carried: bool,
}

impl Harvest {
    /// `Harvest.windowTicks`, in a day of `day` ticks.
    pub fn window_ticks(&self, day: u64) -> u64 {
        self.window_days as u64 * day
    }
}

/// What the judging of a harvest term reads from the dials: installments a farm may fall behind before a shortfall is
/// a miss (`harvest_arrears`), and the share of a miss a rolled-over shortfall costs in trust (`harvest_rolled_weight`).
#[derive(Clone, Copy, Debug)]
pub struct HarvestDials {
    pub arrears: f64,
    pub rolled_weight: f64,
}

impl HarvestDials {
    pub fn of(d: &Dials) -> Self {
        HarvestDials { arrears: d.harvest_arrears, rolled_weight: d.harvest_rolled_weight }
    }
}

impl Default for HarvestDials {
    fn default() -> Self {
        Self::of(&Dials::default())
    }
}

/// What a rent reads from the dials, and the day's length.
#[derive(Clone, Copy, Debug)]
pub struct RentTerms {
    pub day: u64,
    pub window_days: u32,
    pub season_days: u32,
    pub purchase_seasons: f64,
    pub arrears_seasons: f64,
}

impl RentTerms {
    pub fn of(d: &Dials, day: u64) -> Self {
        RentTerms { day, window_days: d.harvest_window_days, season_days: d.rent_season_days,
            purchase_seasons: d.rent_purchase_seasons, arrears_seasons: d.rent_arrears_seasons }
    }

    /// A season, in ticks: `Rents.seasonTicks`.
    pub fn season_ticks(&self) -> u64 {
        self.season_days as u64 * self.day
    }

    /// A rent's price: `rent_purchase_seasons` seasons' crop at `each` emeralds (`Rents.capital`), rounded up to the
    /// thousandth: the holder's to pay.
    pub fn capital(&self, quantity: f64, each: f64) -> crate::fixed::Fixed {
        crate::fixed::Fixed::ceil(self.purchase_seasons * quantity * each)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RentState {
    Active,
    Redeemed,
    Defaulted,
}

/// A rent: the farm owes the holder `quantity` of `crop` a season, bought for `capital`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rent {
    pub id: usize,
    pub farm: TownId,
    /// Whoever holds it now.
    pub holder: TownId,
    pub crop: GoodId,
    pub quantity: f64,
    pub capital: crate::fixed::Fixed,
    pub bought: u64,
    pub next_coupon: u64,
    /// The coupon in force (a contract in the book).
    pub coupon: Option<usize>,
    /// Coupons issued, in all.
    pub coupons: u32,
    /// Of the crop, owed from past seasons.
    pub arrears: f64,
    /// Of the crop, delivered in all.
    pub delivered: f64,
    pub state: RentState,
    /// Its holder's caravans collect the coupons at the farm (a dealer's), not the farm delivering.
    pub ex_works: bool,
    /// Its deed is a player's: the holder (a dealer) collects the coupons in its keeping.
    pub bearer: bool,
    /// Of the crop, collected for the bearer and not yet handed over.
    pub accrued: f64,
    /// What its holder paid for it, and when it last changed hands.
    pub paid: crate::fixed::Fixed,
    pub traded: u64,
}

/// Every rent.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Rents {
    pub rents: Vec<Rent>,
}

impl Rents {
    /// Sold: the holder has paid the capital (the caller moves the money); the first coupon is issued at once.
    #[allow(clippy::too_many_arguments)]
    pub fn sell(&mut self, farm: TownId, holder: TownId, crop: GoodId, quantity: f64, capital: impl Into<crate::fixed::Fixed>,
            book: &mut Book, now: u64, terms: &RentTerms) -> usize {
        let capital = capital.into();
        let id = self.rents.len();
        self.rents.push(Rent { id, farm, holder, crop, quantity, capital, bought: now, next_coupon: now, coupon: None,
            coupons: 0, arrears: 0.0, delivered: 0.0, state: RentState::Active, ex_works: false, bearer: false,
            accrued: 0.0, paid: capital, traded: now });
        self.issue(id, book, now, terms);
        id
    }

    /// Each season: the last coupon's shortfall goes into the arrears (its contract closed) and a new coupon is issued
    /// for the season's crop and the arrears; arrears past `rent_arrears_seasons` seasons are a default. A rent bought
    /// back or defaulted has no coupon standing.
    pub fn review(&mut self, book: &mut Book, now: u64, terms: &RentTerms) {
        for i in 0..self.rents.len() {
            if self.rents[i].state != RentState::Active {
                if let Some(c) = self.rents[i].coupon.filter(|&c| book.contracts[c].in_force()) {
                    book.close(c);
                }
                continue;
            }
            if now >= self.rents[i].next_coupon {
                self.issue(i, book, now, terms);
            }
        }
    }

    fn issue(&mut self, i: usize, book: &mut Book, now: u64, terms: &RentTerms) {
        let rent = &mut self.rents[i];
        let mut owed = 0.0;
        if let Some(id) = rent.coupon {
            let c = &book.contracts[id];
            rent.delivered += c.delivered;
            if rent.bearer {
                rent.accrued += c.delivered; // in the dealer's keeping, for whoever brings the deed
            }
            // What the holder's caravan has collected and carries home is the holder's already.
            owed = (c.quantity - c.delivered - if c.incoterm.seller_bears_road() { 0.0 } else { c.shipped }).max(0.0);
            book.close(id);
        }
        rent.arrears = owed;
        if rent.arrears > terms.arrears_seasons * rent.quantity {
            rent.state = RentState::Defaulted;
            book.missed(rent.farm, rent.holder, 3.0, now, terms.day);
            return;
        }
        // The coupon: this season's crop and the arrears, already paid for, due at the season's end, both parties
        // holding the rent's deed (so it's in force at once).
        let incoterm = if rent.ex_works { Incoterm::Exw } else { Incoterm::Ddp };
        let coupon = book.draft_harvest(rent.farm, rent.holder, rent.crop, rent.quantity + rent.arrears, 0.0,
            now + terms.season_ticks(), terms.season_ticks(), 1, incoterm, true, true,
            Harvest { window_days: terms.window_days, carried: true });
        rent.coupon = Some(coupon);
        rent.coupons += 1;
        rent.next_coupon = now + terms.season_ticks() + terms.window_days as u64 * terms.day;
    }

    /// The deed changed hands with a player: `bearer`, it's the player's, in the holder's keeping.
    pub fn set_bearer(&mut self, id: usize, bearer: bool) {
        self.rents[id].bearer = bearer;
    }

    /// Hands over up to `count` of what's accrued for the bearer; returns how many.
    pub fn take_accrued(&mut self, id: usize, count: f64) -> f64 {
        let rent = &mut self.rents[id];
        let taken = count.min(rent.accrued).max(0.0);
        rent.accrued -= taken;
        taken
    }

    /// Bought back: the farm has paid the holder its capital (the caller moves the money) and owes nothing more; its
    /// coupon closes.
    pub fn redeem(&mut self, id: usize, book: &mut Book) {
        if let Some(c) = self.rents[id].coupon {
            book.close(c);
        }
        self.rents[id].state = RentState::Redeemed;
    }

    /// Rents in force a farm owes this holder: `Rents.owed`.
    pub fn owed(&self, farm: TownId, holder: TownId) -> Vec<usize> {
        self.rents.iter().filter(|r| r.state == RentState::Active && r.farm == farm && r.holder == holder)
            .map(|r| r.id).collect()
    }

    /// Rents in force this desk holds: `Rents.heldBy`.
    pub fn held_by(&self, holder: TownId) -> Vec<usize> {
        self.rents.iter().filter(|r| r.state == RentState::Active && r.holder == holder).map(|r| r.id).collect()
    }
}

/// What the farm keeps back of a crop for its forward sales and rents' coupons: what falls due under harvest contracts
/// by the end of the next harvest window, not yet delivered or on the road: `ForwardSales.setAside`. Its spot sales
/// (and its workshop) use only the rest. (The mod crates it as well, `earmarked`; the core has no crates.)
pub fn set_aside(book: &Book, farm: TownId, crop: GoodId, now: u64, day: u64) -> f64 {
    book.open.iter().map(|&id| &book.contracts[id])
        .filter(|c| c.seller == farm && c.good == crop && c.in_force())
        .filter_map(|c| c.harvest.map(|h| (c, h)))
        .map(|(c, h)| (c.owed(now + h.window_ticks(day)) - c.shipped).max(0.0))
        .sum()
}

/// What a farm could add of a crop to what it grows now, items a day: its spare plots at a plot's yield, its own share
/// of what it grows of the crop, and the other share of its other rotation crops (`Territory.couldGrow`).
pub fn could_grow(content: &Content, farm: &Farm, crop: GoodId) -> f64 {
    let d = &content.dials;
    let at = |g: GoodId| farm.per_day.get(&g).copied().unwrap_or(0.0);
    let others: f64 = crate::farms::rotation(content).into_iter().filter(|&c| c != crop).map(at).sum();
    farm.spare_plots as f64 * YIELD_PER_PLOT_PER_DAY + d.could_grow_own_share * at(crop) + d.could_grow_other_share * others
}

/// What a town has sold forward, in force: by crop, items a day and what an item is worth there (`ForwardSales.sold`).
/// Its Farmers sow for these first.
pub fn sold(content: &Content, book: &Book, seller: TownId) -> BTreeMap<GoodId, (f64, f64)> {
    let mut sold: BTreeMap<GoodId, (f64, f64)> = BTreeMap::new();
    for c in book.open.iter().map(|&id| &book.contracts[id]) {
        if c.seller != seller || c.harvest.is_none() || !c.in_force() {
            continue;
        }
        let per_day = c.quantity * content.day_ticks as f64 / c.every.max(1) as f64;
        let each = c.price.max(content.goods[c.good].value);
        let e = sold.entry(c.good).or_insert((0.0, 0.0));
        *e = (e.0 + per_day, e.1.max(each));
    }
    sold
}

/// Items a day of every crop the farm has sold forward, drafted or in force (contracts and rents' coupons):
/// `ForwardSales.promised`.
pub fn promised(book: &Book, farm: TownId, day: u64) -> f64 {
    book.open.iter().map(|&id| &book.contracts[id])
        .filter(|c| c.seller == farm && c.harvest.is_some() && matches!(c.state, State::Active | State::Drafted))
        .map(|c| c.quantity * day as f64 / c.every.max(1) as f64).sum()
}

/// Items a day of a crop the buyer has bought forward, from every farm: `ForwardSales.boughtForward`.
pub fn bought_forward(book: &Book, buyer: TownId, crop: GoodId, day: u64) -> f64 {
    book.open.iter().map(|&id| &book.contracts[id])
        .filter(|c| c.buyer == buyer && c.good == crop && c.harvest.is_some() && matches!(c.state, State::Active | State::Drafted))
        .map(|c| c.quantity * day as f64 / c.every.max(1) as f64).sum()
}

/// A dealer's price for a rent: its capital times how short the markets are of the crop, from half (all of it bought
/// ahead) to twice (none): `RentDealer.price`. `wanted` is the markets' wants of the crop a day, `ahead` the crop a day
/// all rents in force have bought ahead. To the nearest thousandth.
pub fn dealer_price(capital: impl Into<crate::fixed::Fixed>, wanted: f64, ahead: f64) -> crate::fixed::Fixed {
    let capital = capital.into();
    let share = if wanted + ahead <= 0.0 { 0.5 } else { wanted / (wanted + ahead) };
    capital.mul_round(0.5 + 1.5 * share)
}

/// The dealer sells at its price a spread over (rounded up: the buyer's to pay) and buys a spread under (rounded down:
/// what it pays out): `RentDeeds.quote`.
pub fn ask(price: impl Into<crate::fixed::Fixed>, spread: f64) -> crate::fixed::Fixed {
    price.into().mul_ceil(1.0 + spread)
}

pub fn bid(price: impl Into<crate::fixed::Fixed>, spread: f64) -> crate::fixed::Fixed {
    price.into().mul_floor(1.0 - spread)
}

impl Contract {
    /// How long past its day an installment may wait before it's judged: the harvest window, or the grace.
    pub fn wait_ticks(&self, grace: u64, day: u64) -> u64 {
        self.harvest.map_or(grace, |h| h.window_ticks(day))
    }
}

use crate::cash_book::{Account, Reason};
use crate::society::Society;
use crate::state::{Route, Trades};

impl Society {
    /// A contract with a harvest term, as [`Society::draft`] drafts one.
    #[allow(clippy::too_many_arguments)]
    pub fn draft_harvest(&mut self, seller: TownId, buyer: TownId, good: GoodId, quantity: f64, price: f64, start: u64,
            every: u64, installments: u32, incoterm: Incoterm, both: bool, carried: bool) -> usize {
        let window_days = self.content().dials.harvest_window_days;
        self.world.resource_mut::<Book>().draft_harvest(seller, buyer, good, quantity, price, start, every, installments,
            incoterm, true, both, Harvest { window_days, carried })
    }

    fn rent_terms(&self) -> RentTerms {
        RentTerms::of(&self.content().dials, self.content().day_ticks)
    }

    /// The holder buys a rent of `quantity` of `crop` a season from the farm at `each` emeralds: the capital (eight
    /// seasons' crop) paid holder to farm, the first coupon in force at once. `None` if the holder can't pay it.
    pub fn sell_rent(&mut self, farm: TownId, holder: TownId, crop: GoodId, quantity: f64, each: f64) -> Option<usize> {
        let terms = self.rent_terms();
        let capital = terms.capital(quantity, each);
        if self.balance(Account::Treasury(holder))? < capital {
            return None;
        }
        self.post(Account::Treasury(holder), Account::Treasury(farm), capital, Reason::Other("rent".into()));
        let now = self.now();
        let mut rents = std::mem::take(&mut *self.world.resource_mut::<Rents>());
        let id = rents.sell(farm, holder, crop, quantity, capital, &mut self.world.resource_mut::<Book>(), now, &terms);
        *self.world.resource_mut::<Rents>() = rents;
        Some(id)
    }

    /// Every rent, in the order sold.
    pub fn rents(&self) -> Vec<Rent> {
        self.world.resource::<Rents>().rents.clone()
    }

    /// The farm buys the rent back: its capital paid to the holder, its coupon closed. `false` if it can't pay.
    pub fn redeem_rent(&mut self, id: usize) -> bool {
        let rent = self.world.resource::<Rents>().rents[id].clone();
        if rent.state != RentState::Active || self.balance(Account::Treasury(rent.farm)).unwrap_or_default() < rent.capital {
            return false;
        }
        self.post(Account::Treasury(rent.farm), Account::Treasury(rent.holder), rent.capital, Reason::Other("rent".into()));
        let mut rents = std::mem::take(&mut *self.world.resource_mut::<Rents>());
        rents.redeem(id, &mut self.world.resource_mut::<Book>());
        *self.world.resource_mut::<Rents>() = rents;
        true
    }

    /// A rent's deed went to a player (or came back): the holder collects its coupons in its keeping.
    pub fn set_bearer(&mut self, id: usize, bearer: bool) {
        self.world.resource_mut::<Rents>().set_bearer(id, bearer);
    }

    /// Hands over up to `count` of the crop accrued for the bearer; returns how many.
    pub fn take_accrued(&mut self, id: usize, count: f64) -> f64 {
        self.world.resource_mut::<Rents>().take_accrued(id, count)
    }

    /// Issues the seasons' coupons that are due now, as each look does.
    pub fn review_rents(&mut self) {
        let (now, terms) = (self.now(), self.rent_terms());
        let mut rents = std::mem::take(&mut *self.world.resource_mut::<Rents>());
        rents.review(&mut self.world.resource_mut::<Book>(), now, &terms);
        *self.world.resource_mut::<Rents>() = rents;
    }

    /// What the town keeps back of a good for what it owes under harvest contracts: kept from its spot sales and its
    /// workshop (the workshop's own hook is [`Society::set_aside`]).
    pub fn set_aside_for_harvest(&self, town: TownId, good: GoodId) -> f64 {
        crate::cargo::set_aside(self.world.resource::<Book>(), self.world.resource::<crate::pallet::Pallets>(), town, good,
            self.now(), self.content().day_ticks)
    }

    /// A rent's price at a hub now: its capital times how short the markets the hub's wagons reach (and the hub itself)
    /// are of the crop, a market's want being what its people use beyond what it makes.
    pub fn rent_price(&self, hub: TownId, id: usize) -> crate::fixed::Fixed {
        let rents = self.world.resource::<Rents>();
        let rent = &rents.rents[id];
        let capital = self.rent_terms().capital(rent.quantity, self.content().goods[rent.crop].value);
        let mut markets = vec![hub];
        for &e in &self.routes {
            let r = self.world.get::<Route>(e).expect("a route");
            if r.from_id == hub && !markets.contains(&r.to_id) {
                markets.push(r.to_id);
            }
        }
        let wanted: f64 = markets.iter().map(|&m| {
            let t = self.world.get::<Trades>(self.towns[m]).expect("a town");
            (t.uses[rent.crop] - t.makes[rent.crop]).max(0.0)
        }).sum();
        let ahead: f64 = rents.rents.iter().filter(|r| r.state == RentState::Active && r.crop == rent.crop)
            .map(|r| r.quantity / self.rent_terms().season_days as f64).sum();
        dealer_price(capital, wanted, ahead)
    }

    /// What the dealer at `hub` sells the rent's deed for, and buys it back for.
    pub fn rent_ask(&self, hub: TownId, id: usize) -> crate::fixed::Fixed {
        ask(self.rent_price(hub, id), self.content().dials.dealer_spread)
    }

    pub fn rent_bid(&self, hub: TownId, id: usize) -> crate::fixed::Fixed {
        bid(self.rent_price(hub, id), self.content().dials.dealer_spread)
    }
}
