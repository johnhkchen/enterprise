//! Shops, boards and rent deeds as the host sees them (`docs/binding-plan.md` 2, concern 9; task `shop-adapter`):
//! facts about Numismatics vendors in, what to show of them out. The rules are the core's own (`orders.rs`,
//! `rules/orders.rs`, `desires.rs`, `harvest.rs`); `shop_host.rs` keeps what a host's vendors do to them.

use serde::{Deserialize, Serialize};

use super::{GoodNo, Point, TownNo};

/// What mode a vendor is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VendorMode {
    /// Nothing the core deals in: no goods set, goods it can't price (enchanted, a good the pack hasn't), a creative one.
    Idle,
    /// It buys what its filter names, a lot at a time at its price, from the bank card in it.
    Buy,
    /// It sells what its filter names, a lot at a time at its price.
    Sell,
}

/// What a host found at a vendor this look: facts only, none of the rules (`Input::Vendor`). Which of a player's order,
/// a player's stall, the town's own shop, its board's or a rent's deed it is, is the core's to say.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VendorRead {
    pub mode: VendorMode,
    /// The good its filter names (`None`: no filter, or a deed, or goods the core has no number for).
    pub good: Option<GoodNo>,
    /// The rent its filter is the deed to (a document naming it).
    pub deed: Option<u32>,
    /// What a lot is (its filter's count) and what a lot costs, in spurs.
    pub lot: u32,
    pub price: u32,
    /// Items it holds that its filter matches (the deeds, for a deed's shop).
    pub stock: u32,
    /// Lots it has room for (a buying vendor).
    pub room: u32,
    /// What the bank card in it holds, in spurs (`0`: no card, or none bound).
    pub purse: u64,
    /// Spurs of coin the host took out of its box this look (sales a player made): the core has them now.
    pub coins: u64,
    /// Whether it touches a desk (the town's own shop), not just stands near it.
    pub touches: bool,
}

/// What a vendor is to the core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShopRole {
    /// A player's buying shop: a standing order.
    Order,
    /// A player's selling shop: supply.
    Stall,
    /// A shop touching the desk, the town's own: its Merchant stocks it with surplus.
    Own,
    /// One of the town's shop board's: it lists what the town is after.
    Board,
    /// The shop where a rent's deed is sold.
    Deed,
    /// One the core deals with none of.
    Idle,
}

/// A vendor the core keeps, as the screens and commands show it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShopLine {
    pub point: Point,
    pub role: ShopRole,
    /// The good it deals in, a lot and its price in spurs.
    pub good: Option<GoodNo>,
    pub lot: f64,
    pub price: f64,
    /// A buying shop: what its card holds, the lots it still wants of the contract and the goods delivered so far, and
    /// the spurs paid for them. A selling shop: the goods on its shelf (`stock`). The town's own or its board's: the
    /// shelf. A deed's shop: the rent.
    pub purse: f64,
    pub open: f64,
    pub delivered: f64,
    pub paid: f64,
    pub stock: f64,
    pub rent: Option<u32>,
}

/// A want the desk has posted that no market holds, as `Output::Signal` says it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalLine {
    pub good: GoodNo,
    pub count: f64,
    pub spurs_each: f64,
    pub why: String,
}

/// A good the town's shop board lists: a lot at a price in spurs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListingLine {
    pub good: GoodNo,
    pub lot: u32,
    pub spurs: u32,
}

/// A town's trade with the shops, tallied.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TallyLine {
    pub lots_sold: f64,
    pub spurs_from_sales: f64,
    pub lots_bought: f64,
    pub spurs_on_purchases: f64,
    pub spurs_from_own_shops: f64,
}

/// A rent a dealer holds, as its shop shows it: what it's for, the dealer's ask and bid in spurs now, whether its deed
/// is a player's (in the dealer's keeping) and the crop accrued for the bearer, and the vendor where its deed is sold.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeedLine {
    pub rent: u32,
    pub farm: TownNo,
    /// The farm's town's name, for the deed to name it.
    pub farm_name: String,
    pub crop: GoodNo,
    pub quantity: f64,
    pub ask: u32,
    pub bid: u32,
    pub bearer: bool,
    pub accrued: f64,
    pub point: Option<Point>,
}
