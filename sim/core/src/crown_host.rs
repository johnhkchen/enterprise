//! The crown, its realms and its markets off the map through the wire (`docs/binding-plan.md` 2, concern 10; task
//! `crown-views`): what a host needs of the sovereign to show it in its world. The rules are the core's own (`crown.rs`,
//! `army.rs`, `realms.rs`, `sourcing.rs` and their systems in `rules/`); this module lets a host name the capital,
//! mobilise, declare war and make peace, open a market off the map or a realm as a trade point, buy gold for the crown
//! at a vendor, and ask for what `/enterprise realms` and `metals` show.
//!
//! - **The sovereign's commands** (`Input::Capital`, `Mobilise`, `Declare`, `Peace`; `Sovereign.establish`, `mobilise`,
//!   `declare`, `peace`): the crown's host calls. `WarDeclared` says how many towns were levied, `PeaceMade` how many
//!   soldiers and recruits walked home; the soldiers themselves are hands (`HandAway` when they march, `HandBack` home,
//!   `HandGone` fallen: `wagon_host.rs`, `census.rs`), the army's the core's alone.
//! - **Markets off the map are points** (`Input::OffMap`, [`Kind::Market`]; `OffMap.open`, `Realms.open`): a realm or a
//!   plain market at a real position of the host's, with a town of its own, its roads planned from there, its goods and
//!   money the core's alone. The front a war is fought at and the market across the sea the core opens itself, at
//!   distances from the capital ([`Society::mark_off_map`]): each is a `Market` point too, beside the capital's desk's
//!   dimension, so a host lists every market off the map from `Query::Points`. A host's own reaches no further than a point.
//! - **The sovereign's gold shop** (`Sovereign.mobilise`'s Numismatics vendor, [`CrownShop`], `Output::BuildCrownShop`,
//!   `Input::CrownShop`): mobilising, the crown buys gold at `gold_premium` times its worth at a vendor by the capital. The
//!   host puts the vendor up and tells the core; while none stands the core asks again at each look its capital is
//!   counted in ([`Society::want_crown_shop`]). Each look the host counts it as any vendor (`Input::Vendor`) and the core,
//!   which knows it is the crown's, keeps its card funded from the capital's treasury and takes the gold sold into it
//!   (`CrownShop`), sent to the mint, which is all the mod did with it. At peace it comes down (`CrownShopDown`).
//!
//! **Where the core differs from the mod.** The card's money is spent out of the treasury when it is put on the card, as
//! a deed vendor's is (`shop_host.rs`): what a card holds beyond what its gold shop pays out stays on it. The metal
//! exchange's vendors and the requisitions' buying vendors, which the mod put up near the capital for players to sell metals
//! and kit to, are not here: the exchange and the requisitions deal with the markets' own Merchants (`sourcing.rs`,
//! `crown.rs`), a player sells them through a market's order shop (`shop_host.rs`) or by delivery.

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::army;
use crate::cash_book::{Account, Reason};
use crate::crown::Theatre;
use crate::sourcing::Phase;
use crate::fixed::Fixed;
use crate::messages::TownId;
use crate::points::{Kind, Spot};
use crate::society::Society;
use crate::state::Ledger;
use crate::wire::{no, Answer, Input, MetalLine, Output, Point, RealmLine, VendorMode, VendorRead};

/// A lot of the crown's gold (`Sovereign.mobilise`'s filter of 8 ingots).
pub const GOLD_LOT: u32 = 8;
/// The card of the gold shop is kept funded to this many lots' price.
const CARD_LOTS: u64 = 2;

/// The crown's gold shop: where it stands (a `Shop` point of the capital's, once the host has put one up), the price it
/// last showed, and the gold it has bought in all; and the shops taken down whose points the host hasn't removed yet (it
/// couldn't reach the vendor), told to come down again whenever they're counted.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CrownShop {
    pub at: Option<Spot>,
    pub shown: Option<u32>,
    pub bought: f64,
    pub down: Vec<Spot>,
}

/// Spurs, to the nearest thousandth of an emerald at the mint's rate.
fn worth(spurs: f64, rate: f64) -> Fixed {
    Fixed::round(spurs / rate)
}

impl Society {
    /// Applies a crown happening; empty for any other input. The wire checks them first (`wire::handle`).
    pub(crate) fn apply_crown(&mut self, input: &Input) -> Vec<Output> {
        match input {
            Input::OffMap { name, at, sells, realm, spurs, partners } => {
                self.open_off_map_point(name, at, *sells, realm, *spurs, partners);
                Vec::new()
            }
            Input::Capital { town, spurs_per_day } => {
                let town = *town as TownId;
                // A gold shop at another capital's comes down.
                let mut out = Vec::new();
                if let Some(at) = self.world.resource::<CrownShop>().at.clone() {
                    if self.point(&at).map(|p| p.town) != Some(town) {
                        out.extend(self.take_down_crown_shop());
                    }
                }
                self.name_capital(town, *spurs_per_day);
                out
            }
            Input::Mobilise { town, spurs_per_day } => {
                let town = *town as TownId;
                let mut out = Vec::new();
                // A shop at another capital comes down; one at this capital stands.
                if let Some(at) = self.world.resource::<CrownShop>().at.clone() {
                    if self.point(&at).map(|p| p.town) != Some(town) {
                        out.extend(self.take_down_crown_shop());
                    }
                }
                self.mobilise(town, *spurs_per_day);
                if self.world.resource::<CrownShop>().at.is_none() {
                    out.push(Output::BuildCrownShop { town: no(town) });
                }
                out
            }
            Input::Declare { theatre } => {
                let levies = self.declare(Theatre::named(theatre).expect("a theatre the wire checked"));
                vec![Output::WarDeclared { levies: levies as u32 }]
            }
            Input::Peace => {
                let home = self.peace();
                let mut out = vec![Output::PeaceMade { home: home as u32 }];
                out.extend(self.take_down_crown_shop());
                out
            }
            Input::CrownShop { point } => {
                let mut shop = self.world.resource_mut::<CrownShop>();
                let at = point.spot();
                shop.down.retain(|d| *d != at);
                shop.at = Some(at);
                shop.shown = None;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// A market off the map opened at a `Market` point (see the module's doc): a realm's, or a plain one.
    fn open_off_map_point(&mut self, name: &str, at: &Point, sells: bool, realm: &str, spurs: u64, partners: &[u32]) {
        let spot = at.spot();
        let (x, z) = (spot.x as f64, spot.z as f64);
        let town = match realm {
            "coast" => self.add_coast(x, z, 0.0),
            "march" => self.add_march(x, z, 0.0),
            _ => {
                let town = self.add_off_map(name, x, z, 0.0);
                self.set_off_map_sells(town, sells);
                town
            }
        };
        let rate = self.mint_rate();
        if spurs > 0 {
            self.pay(town, worth(spurs as f64, rate));
        }
        self.add_point(Kind::Market, spot, town).expect("a market's place the wire checked");
        let now = self.now();
        for &p in partners {
            let p = p as TownId;
            if self.route_from_to(town, p).is_none() {
                self.open_route(town, p);
            }
            self.world.resource_mut::<crate::atlas::Atlas>().learn_both(town, p, now);
        }
    }

    /// An off-map market the core opened itself (the front, the market across the sea), a point of its own at its
    /// position, in the dimension of `beside`'s desk and at height 0 (nothing stands there: the height is only part of the
    /// point's key, kept off the heights desks stand at). Nothing if `beside` has no desk, or a point stands there already.
    pub(crate) fn mark_off_map(&mut self, town: TownId, beside: TownId) {
        let Some(desk) = self.desk_of(beside) else { return };
        let Some((x, z)) = self.world.get::<crate::state::Town>(self.towns[town]).and_then(|t| t.at) else { return };
        let spot = Spot { dim: desk.dim.clone(), x: x.round() as i32, y: 0, z: z.round() as i32 };
        let _ = self.add_point(Kind::Market, spot, town);
    }

    /// Takes the crown's gold shop down: it is forgotten, and the host told to take the vendor away. Until the host removes
    /// its point (it may not reach the vendor now: peace is made from anywhere), each count of it says so again, so a
    /// vendor with the crown's card is never taken for a player's.
    fn take_down_crown_shop(&mut self) -> Vec<Output> {
        let mut shop = self.world.resource_mut::<CrownShop>();
        shop.shown = None;
        match shop.at.take() {
            Some(at) => {
                if !shop.down.contains(&at) {
                    shop.down.push(at.clone());
                }
                vec![Output::CrownShopDown { point: Point::of(&at, Kind::Shop) }]
            }
            None => Vec::new(),
        }
    }

    /// After a look (`wire::handle`): while the crown is mobilised or at war and has no gold shop, it asks for one again
    /// whenever its capital is counted, as a board asks for a vendor (`keep_board`). Nothing waits in the host: a request
    /// it lost (restarted, or no floor yet by the desk) comes again at the next look.
    pub(crate) fn want_crown_shop(&self, towns: &[TownId]) -> Vec<Output> {
        let Some(capital) = self.capital() else { return Vec::new() };
        if self.sovereign().phase == Phase::Peace || self.world.resource::<CrownShop>().at.is_some() || !towns.contains(&capital) {
            return Vec::new();
        }
        vec![Output::BuildCrownShop { town: no(capital) }]
    }

    /// The crown's gold shop, or one taken down whose point still stands.
    pub(crate) fn is_crown_shop(&self, at: &Spot) -> bool {
        let shop = self.world.resource::<CrownShop>();
        shop.at.as_ref() == Some(at) || shop.down.contains(at)
    }

    /// A point removed that was the crown's gold shop: it stands no more.
    pub(crate) fn forget_crown_shop(&mut self, at: &Spot) {
        let mut shop = self.world.resource_mut::<CrownShop>();
        shop.down.retain(|d| d != at);
        if shop.at.as_ref() == Some(at) {
            shop.at = None;
            shop.shown = None;
        }
    }

    /// The gold shop counted (see the module's doc): its gold bought, its card funded, its price shown.
    pub(crate) fn read_crown_shop(&mut self, at: &Spot, town: TownId, read: &VendorRead) -> Vec<Output> {
        let point = Point::of(at, Kind::Shop);
        if self.world.resource::<CrownShop>().down.contains(at) {
            return vec![Output::CrownShopDown { point }];
        }
        let gold = army::good_named(self.content(), "gold_ingot");
        let (Some(gold), true) = (gold, self.capital() == Some(town) && self.sovereign().phase != Phase::Peace) else {
            return self.take_down_crown_shop();
        };
        let rate = self.mint_rate();
        let price = (self.content().goods[gold].value * f64::from(GOLD_LOT) * self.content().dials.gold_premium * rate)
            .ceil().max(1.0) as u32;
        let mine = read.mode == VendorMode::Buy && read.good == Some(gold as u32);
        // What was sold into it is the crown's, sent to the mint.
        let taken = if mine { read.stock } else { 0 };
        if taken > 0 {
            let n = f64::from(taken);
            let mut ledger = self.world.resource_mut::<Ledger>();
            ledger.delivered[gold] += n;
            ledger.used[gold] += n;
            self.world.resource_mut::<CrownShop>().bought += n;
        }
        // Its card holds two lots' price: what it paid out is made up from the treasury, if the treasury has it.
        let want = u64::from(price) * CARD_LOTS;
        let mut fund = 0;
        if mine && read.purse < want {
            let short = want - read.purse;
            let cost = Fixed::round(short as f64 / rate * 1.05);
            if self.balance(Account::Treasury(town)).unwrap_or_default() >= cost {
                self.post(Account::Treasury(town), Account::Outside, cost, Reason::Other("gold shop".into()));
                fund = short;
            }
        }
        let shown = self.world.resource::<CrownShop>().shown;
        if mine && shown == Some(price) && read.price == price && fund == 0 && taken == 0 {
            return Vec::new();
        }
        self.world.resource_mut::<CrownShop>().shown = Some(price);
        vec![Output::CrownShop { point, good: no(gold), lot: GOLD_LOT, price, fund, taken }]
    }

    /// `Query::Realms`: each realm's trade, in the words of `Realms.report` (spurs at the mint's rate now).
    pub fn realms_answer(&self) -> Answer {
        let rate = self.mint_rate();
        let Answer::RouteBooks { routes: books, .. } = self.route_books_answer() else { unreachable!("route books") };
        let mut realms = Vec::new();
        for town in 0..self.town_count() {
            let Some(view) = self.realm(town) else { continue };
            let mine = books.iter().filter(|b| b.from == no(town));
            let mut line = RealmLine { town: no(town), name: self.named(town), trips: 0, partners: 0, sold_spurs: 0.0,
                bought_spurs: 0.0, customs_spurs: view.customs_paid.to_f64() * rate, made: view.goods_made,
                in_hand_spurs: self.town(town).money.to_f64() * rate };
            for b in mine {
                line.trips += b.trips;
                line.partners += 1;
                line.sold_spurs += b.sales_spurs;
                line.bought_spurs += b.purchase_spurs;
            }
            realms.push(line);
        }
        Answer::Realms { realms }
    }

    /// `Query::Metals`: the capital's metal exchange and each metal it keeps.
    pub fn metals_answer(&self) -> Answer {
        let ex = self.exchange();
        let content = self.content();
        let book = self.contracts();
        let metals = crate::sourcing::metals(content).iter().map(|m| {
            let coming = ex.desk.map_or(0.0, |desk| {
                book.iter().filter(|c| c.buyer == desk && c.good == m.good && c.training.is_none()
                    && matches!(c.state, crate::contracts::State::Drafted | crate::contracts::State::Active))
                    .map(|c| c.quantity * c.installments as f64 - c.delivered).sum()
            });
            MetalLine { good: no(m.good), name: content.goods[m.good].name.clone(), held: ex.held(m.good), target: ex.target(m),
                coming, premium: ex.premium(m) }
        }).collect();
        Answer::Metals { desk: ex.desk.map(no), phase: format!("{:?}", self.sovereign().phase).to_lowercase(), metals }
    }
}
