//! Markets off the map, and the realms beyond the border (`merchant/OffMap.java`, `trade/Realms.java`,
//! `caravan/Caravans.java` `dispatch`'s `foreign` branch, `sell`, `arriveAtDestination`, `foodKnownAt`;
//! `docs/decisions.md` "Everything runs on the logistics framework", "Realms beyond the border").
//!
//! **Off the map** ([`OffMap`], `OffMap.open`, `is`, `sells`): a town at a real position with a ledger warehouse and its
//! own money and no one in it, never visited; contracts, caravans and bills treat it like any town. It sells no food
//! unless told ([`OffMap::sells_food`]; `OffMap.sells`): a wagon bound there packs the road home's food as well as the
//! way out's, as it does for any far end that can't feed it (`rules/plan.rs`: the far end's food reads as none). It has
//! no roster, so its wagons are never crewed.
//!
//! **A realm** ([`Realm`], `Realms.define`, `open`, `make`): an off-map market that makes what our villagers can't and
//! wants some of what they make. Each day (`Work`: [`rules::realms::work`](crate::rules::realms)) its people use what it
//! bought, up to [`Dials::realm_uses_per_day`](crate::Dials) of each good it wants (of food, only what's beyond what its
//! caravans need on the road), then make `per_day` of each good up to `keep`, through `Ledger.made`. It opens with 3
//! days' goods in hand. What it wants it takes by its warehouse's par: a capacity of 64 a day (`Warehouse.recordCapacity`
//! under its own key), comfort or not (`Caravans.sell`: "a realm's people want some comforts as a country does"), from
//! the money it holds with no Accountant to hold it back (`foreign ? Double.MAX_VALUE`).
//!
//! **Its wagons** need no squad (`dispatch`'s `foreign`: its Merchant and crew are its own, never seen): two mouths
//! ([`Dials::foreign_crew`](crate::Dials)), no team to feed, no home food kept back to cover a town's poverty, and every
//! bargain at `foreign_merchant_level` (4), its own market's as buyer too (`Warehouse.recordLevel(at, 4)`).
//!
//! **Customs** (`Realms.customs`, `arriveAtDestination` 694): a foreign wagon's takings at a town pay the crown
//! `customs` (10%) into its treasury, if a capital is named ([`Crown`]; the crown slice will set it, a host can now).
//!
//! **The Coast and the March** ([`Society::add_coast`], [`Society::add_march`]): the two realms' makes and wants, by
//! the good's name in the host's content (a name the host lacks is left out).
//!
//! **Stood in for:** the mod's warehouse slots (a realm's making stops when its chests are full: here only at `keep`),
//! its partners and Atlas (`open`'s routes: the host adds them with [`Society::add_route`] or [`Society::add_road`]),
//! the realm's bank balance (its treasury), and the customs paid in whole spurs (here exact).

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::messages::TownId;
use crate::roster::Roster;
use crate::society::Society;
use crate::state::*;

/// A market off the map: no one lives there. Whether it sells to caravans (`OffMap.Market.sells`: food, and anything a
/// wagon would buy there); a market on the map always does.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OffMap {
    pub sells_food: bool,
}

/// What a realm makes: `per_day` of a good, up to `keep` on hand (`Realms.Make`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Make {
    pub good: GoodId,
    pub per_day: f64,
    pub keep: f64,
}

/// A realm beyond the border (`Realms.Realm`): what it makes and wants, and its own books.
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Realm {
    pub makes: Vec<Make>,
    pub wants: Vec<GoodId>,
    /// The tick its people last worked (`lastDay`, advanced a whole day at a time).
    pub last_day: u64,
    /// Customs paid the crown in all, and goods made in all (`customsPaid`, `goodsMade`).
    pub customs_paid: f64,
    pub goods_made: f64,
}

/// The crown, as realms need it: the capital whose treasury takes customs (`Sovereign.treasury`). None: no duty is
/// paid. The crown slice sets it; until then a host does ([`Society::set_capital`]).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Crown {
    pub capital: Option<TownId>,
}

/// The Merchant's level a bargain at this town is struck at: a realm's fixed level, else its roster's Merchant's
/// (`Caravans.FOREIGN_MERCHANT_LEVEL`, `Warehouse.recordLevel(at, 4)`).
pub fn level(content: &Content, realm: Option<&Realm>, roster: Option<&Roster>) -> u8 {
    if realm.is_some() { content.dials.foreign_merchant_level.min(255) as u8 } else { crate::bargain::merchant_level(roster) }
}

/// Mouths on a foreign wagon (`Caravans.FOREIGN_CREW`).
pub fn crew(content: &Content) -> f64 {
    content.dials.foreign_crew as f64
}

/// What a realm keeps of a food it wants for its caravans' road food (`make`): none of anything else.
fn road_keep(content: &Content, good: GoodId) -> f64 {
    let points = crate::road::points(content, good);
    if points > 0.0 { (2.0 * crate::road::FOOD_PAR_POINTS / points).floor() } else { 0.0 }
}

/// The realm's people at work: `days` of what they use of what they bought, then of what they make, up to what is kept
/// (`Realms.make`).
pub fn make(content: &Content, ledger: &mut Ledger, stock: &mut Stock, realm: &mut Realm, days: f64) {
    let uses = content.dials.realm_uses_per_day as f64 * days;
    for &want in &realm.wants {
        let over = stock.0[want].floor() - road_keep(content, want);
        if over > 0.0 {
            let used = over.min(uses);
            stock.0[want] -= used;
            ledger.used[want] += used;
        }
    }
    for m in realm.makes.clone() {
        let add = (m.per_day * days).min(m.keep - stock.0[m.good].floor()).floor();
        if add > 0.0 {
            stock.0[m.good] += add;
            ledger.made[m.good] += add;
            realm.goods_made += add;
        }
    }
}

/// A realm as a host sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct RealmView {
    pub makes: Vec<Make>,
    pub wants: Vec<GoodId>,
    pub customs_paid: f64,
    pub goods_made: f64,
}

/// What the Coast makes (good, per day, keep) and wants (`Realms.define`).
pub const COAST_MAKES: [(&str, f64, f64); 4] =
    [("pipe_tobacco", 32.0, 256.0), ("tobacco_leaf", 48.0, 384.0), ("cocoa_beans", 16.0, 128.0), ("bread", 64.0, 192.0)];
pub const COAST_WANTS: [&str; 4] = ["wheat", "paper", "iron_ingot", "book"];
/// What the March makes and wants.
pub const MARCH_MAKES: [(&str, f64, f64); 3] = [("leather", 24.0, 192.0), ("white_wool", 24.0, 192.0), ("bread", 48.0, 192.0)];
pub const MARCH_WANTS: [&str; 4] = ["wheat", "bread", "carrot", "iron_ingot"];

impl Society {
    /// A market off the map at the point (x, z), holding `money`, with no one in it and no food to sell
    /// ([`Society::set_off_map_sells`] says otherwise): `OffMap.open`.
    pub fn add_off_map(&mut self, name: &str, x: f64, z: f64, money: f64) -> TownId {
        let town = self.add_town(name, money);
        self.place_town(town, x, z);
        // (An empty roster, as a town of the base world has: a town with none has no lockers to ask about.)
        self.set_roster(town, &[]);
        self.world.entity_mut(self.towns[town]).insert(OffMap::default());
        town
    }

    /// Whether an off-map market sells to caravans (food among it): `OffMap.open`'s `sells`.
    pub fn set_off_map_sells(&mut self, town: TownId, sells: bool) {
        self.world.get_mut::<OffMap>(self.towns[town]).expect("an off-map market").sells_food = sells;
    }

    /// Is the town off the map (`OffMap.is`)?
    pub fn is_off_map(&self, town: TownId) -> bool {
        self.world.get::<OffMap>(self.towns[town]).is_some()
    }

    /// A realm at the point (x, z) with `money`, making `makes` (good, per day, keep) and wanting `wants`
    /// (`Realms.open`): off the map, selling to caravans, with 3 days' goods on hand and a demand of
    /// `realm_uses_per_day` a day of each good it wants. Its roads to its partners are the host's to add
    /// ([`Society::add_route`], [`Society::add_road`] from the realm).
    pub fn add_realm(&mut self, name: &str, x: f64, z: f64, money: f64, makes: &[(GoodId, f64, f64)], wants: &[GoodId])
            -> TownId {
        let town = self.add_off_map(name, x, z, money);
        self.set_off_map_sells(town, true);
        let now = self.now();
        let e = self.towns[town];
        let mut realm = Realm {
            makes: makes.iter().map(|&(good, per_day, keep)| Make { good, per_day, keep }).collect(),
            wants: wants.to_vec(),
            last_day: now,
            ..Default::default()
        };
        let per_day = self.content().dials.realm_uses_per_day as f64;
        let content = self.world.remove_resource::<Content>().expect("content");
        let mut ledger = std::mem::take(&mut *self.world.resource_mut::<Ledger>());
        {
            let mut entity = self.world.entity_mut(e);
            let mut trades = entity.get_mut::<Trades>().expect("a town");
            for &g in wants {
                trades.capacity[g] = per_day;
            }
            let mut stock = entity.get_mut::<Stock>().expect("a town");
            make(&content, &mut ledger, &mut stock, &mut realm, 3.0); // a few days' goods on hand when it opens
        }
        *self.world.resource_mut::<Ledger>() = ledger;
        self.world.insert_resource(content);
        self.world.entity_mut(e).insert(realm);
        town
    }

    fn preset(&mut self, name: &str, x: f64, z: f64, money: f64, makes: &[(&str, f64, f64)], wants: &[&str]) -> TownId {
        let c = self.content();
        let makes: Vec<(GoodId, f64, f64)> = makes.iter().filter_map(|&(n, d, k)| c.good(n).map(|g| (g, d, k))).collect();
        let wants: Vec<GoodId> = wants.iter().filter_map(|n| c.good(n)).collect();
        self.add_realm(name, x, z, money, &makes, &wants)
    }

    /// The Coast: pipe tobacco, leaf, cocoa and bread; wants wheat, paper, iron and books.
    pub fn add_coast(&mut self, x: f64, z: f64, money: f64) -> TownId {
        self.preset("the Coast", x, z, money, &COAST_MAKES, &COAST_WANTS)
    }

    /// The March: leather, wool and bread; wants wheat, bread, carrots and iron.
    pub fn add_march(&mut self, x: f64, z: f64, money: f64) -> TownId {
        self.preset("the March", x, z, money, &MARCH_MAKES, &MARCH_WANTS)
    }

    /// Is the town a realm (`Realms.isForeign`)?
    pub fn is_realm(&self, town: TownId) -> bool {
        self.world.get::<Realm>(self.towns[town]).is_some()
    }

    /// A realm's makes and wants and its books so far (`Realms.report`).
    pub fn realm(&self, town: TownId) -> Option<RealmView> {
        self.world.get::<Realm>(self.towns[town]).map(|r| RealmView {
            makes: r.makes.clone(), wants: r.wants.clone(), customs_paid: r.customs_paid, goods_made: r.goods_made })
    }

    /// Names the capital whose treasury takes the crown's customs, or none.
    pub fn set_capital(&mut self, capital: Option<TownId>) {
        self.world.resource_mut::<Crown>().capital = capital;
    }
}
