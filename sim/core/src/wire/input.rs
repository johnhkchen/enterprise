//! Happenings in: what the host's game did since the last look, for the core to book.

use serde::{Deserialize, Serialize};

use super::{GoodNo, HandNo, HexAt, LandKind, Point, TownNo, VendorRead, Work};
use crate::fixed::Fixed;

/// A happening the host hands the core.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Input {
    /// Time has passed, to this tick.
    Advance { to: u64 },
    /// Goods handed to a town from outside the society (a player's delivery).
    Deliver { town: TownNo, good: GoodNo, count: f64 },
    /// Money paid a town from outside the society (a player buying at its shops).
    Pay { town: TownNo, money: Fixed },
    /// A player's buying shop at a town's desk (`orders.rs`): `lot` of `good` at `price` spurs a lot, paid from `purse`.
    Order { town: TownNo, good: GoodNo, lot: f64, price: f64, purse: f64 },
    /// A player's selling shop at a town's desk: `stock` of `good` in lots of `lot` at `price` spurs a lot.
    Stall { town: TownNo, good: GoodNo, lot: f64, price: f64, stock: f64 },
    /// A shop touching a town's desk, the town's own for its Merchant to stock.
    OwnShop { town: TownNo },
    /// A player buys `count` of `good` from a town's own shop, at the price it lists.
    Buy { town: TownNo, good: GoodNo, count: f64 },
    /// A new town at its desk (a Merchant's desk placed): it stands where its desk does, with nothing held and no money
    /// (money comes in by `Pay`). It's numbered after the towns there are, and its desk is a point of its own.
    /// A desk placed where a town's desk stood before it was broken is that town's desk again, and `name` is unused.
    AddTown { name: String, desk: Point },
    /// A trade point of a town's (a locker, a shop, a banker, a depot, another desk) placed.
    AddPoint { point: Point, town: TownNo },
    /// A trade point broken: its chests are no longer its town's, so what the core took them to hold leaves the town
    /// with them (never more than it holds) and a point placed there again counts them in afresh. The town stays, with
    /// the rest of what it held; a desk placed where its desk stood is its desk again (`AddTown`).
    RemovePoint { point: Point },
    /// What a point's chests hold now, every good there (a good not listed holds none, a shulker box's goods are the
    /// chests'), and how many of their slots stand empty: the core books the difference from what it took them to hold
    /// as goods delivered to its town or taken from it (`points.rs`). At a desk or a depot, the town's stock is then
    /// shown there: what the core moved in or out of it since comes back as `Moved` at the end of the look.
    Stocktake { point: Point, counts: Vec<Count>, free: u32 },
    /// A player carried a whole pallet (every package of it) to a town's desk (`PalletBrought`, `pallet_messages.rs`).
    /// Made out to the town, it is received there and then (its goods into stock, a contract's goods delivered under
    /// it, the crates kept); made out elsewhere, it waits in the town's warehouse to be fetched. A pallet that isn't
    /// in a town's warehouse (already moved, or on a wagon) is ignored: the host's view was stale.
    PalletBrought { town: TownNo, id: u64 },
    /// A player carried one package of a pallet made out to the town to its desk: `package` is the package's place on
    /// the pallet (`PalletPacked` lists them in order). The pallet is received when every package has come, or at the
    /// town's next look with those that have, its seal broken and the rest lost. Ignored for a pallet made out to
    /// another town.
    PackageBrought { town: TownNo, pallet: u64, package: u32 },
    /// A package of a pallet was lost (raided, broken, thrown in lava): it leaves the world (its goods booked as used,
    /// reason lost), the pallet's seal is broken and it goes on short; the packages after it move up one place. A pallet
    /// left with nothing is gone, and a bill of exchange that paid for it is unbacked.
    PackageLost { pallet: u64, package: u32 },
    /// A dial set for this world (`/enterprise dial <name> <value>`), or put back to the world's start with no `value`
    /// (`reset`). Saved with the society. A name the core hasn't, or a value the dial won't take (not a number, a
    /// negative count), is refused.
    SetDial { name: String, value: Option<f64> },
    /// A villager came to live at a town (a Merchant taking its desk, one born or settled there in the host's world): a
    /// hand of `trade` (the core's name for it, with or without the host's namespace: `minecraft:farmer`) at `level` (1
    /// to 5, with the XP it took), known to the host as `tag` (its villager's UUID; empty for none). The core gives it
    /// an id of its own (`Query::Hands` lists it with its tag); a Merchant becomes the desk's if the town has none. A
    /// town that kept no roster starts one, and its villagers follow it from then on. Refused for a trade the core
    /// hasn't, a level outside 1 to 5, or a tag another hand has.
    AddHand { town: TownNo, trade: String, level: u8, tag: String },
    /// A hand at home left the society for good (its villager died, or was taken from the world): off its town's
    /// roster, with its locker. Refused for a hand that isn't on a roster, or is away (out with a caravan or at war: off
    /// the host's world, the core brings it home or buries it).
    HandLost { hand: HandNo },
    /// The host's count of a town's people (its census, `settlement/CensusTaker.java`): the beds there, and every
    /// villager it saw living there, each by the host's tag for it (its UUID) with its trade, level and whether it's a
    /// child. The town's roster follows it (`census.rs`): a villager the core doesn't know joins as a hand (a child as a
    /// child), one it knows is seen (a profession taken in the world is its trade now, a level earned there its level,
    /// never less), and one at home that no census has seen for `FORGET_AFTER_DAYS` is taken to have left. A villager
    /// whose hand is away, another town's or with settlers is left as the core has it; one whose hand is gone (the rules
    /// took it from the world) comes back as `HandGone`. Lenient, as a count of the world is: a resident of a trade the
    /// core hasn't, or with no tag, isn't counted, and a tag listed twice is counted once.
    Census { town: TownNo, beds: u32, residents: Vec<Resident> },
    /// A villager now stands for a hand that had none (the host spawned it for `Born` or `Settled`): the host's tag for
    /// it from now on. Ignored when the hand isn't at home on a roster by the time it applies, or another living hand has
    /// the tag. Refused only for an empty tag.
    TagHand { hand: HandNo, tag: String },
    /// What the host couldn't do of a `Moved` at a desk or a depot: `count` of the good didn't fit (above nothing: still
    /// the town's, kept on its books and moved in again when its chests have room) or wasn't there to take (below
    /// nothing: a player or a hopper took it first, which the next stocktake books). Sent before the point's next
    /// stocktake. Ignored for a point that isn't there any more, or is another kind (the host's view was stale).
    Unmoved { point: Point, good: GoodNo, count: i64 },
    /// What a desk's chests and Blaze Banker hold of money now (`till.rs`): `emeralds` whole emeralds in the chests
    /// (loose and in shulker boxes), `coins` spurs of Numismatics coin loose in them, and the Banker's balance in spurs
    /// (`None`: no Banker touches the desk). What's more than the core took them to hold is money paid into the town from
    /// outside the society, what's less is money taken out; spurs count at the mint's rate now, and from then on for
    /// the worth they came in at. `coin_room` is whether the chests could take more coin, `writes` whether the host
    /// writes the core's `Cash` moves to them (false: it only counts, and the core sells nothing to the mint and moves
    /// nothing). Sent beside the desk's `Stocktake`, after it. Refused for a point that isn't a desk.
    Cashcount { point: Point, emeralds: u64, coins: u64, bank: Option<u64>, coin_room: bool, writes: bool },
    /// What the host couldn't do of a `Cash` move at a desk: `emeralds` didn't fit (above nothing) or weren't there to
    /// take (below), and the same for `coins` loose in the chests and the Banker's `bank` spurs. Sent before the desk's
    /// next `Cashcount`. Ignored for a desk that isn't there any more (the host's view was stale).
    CashUnmoved { point: Point, emeralds: i64, coins: i64, bank: i64 },
    /// A hex's ground as the host surveyed it (`territory/Terrain.java`: its chunks' surface columns under water and
    /// their heights, a chunk never loaded guessed from its biome): its share under water (0 to 1), its relief in blocks
    /// (highest surface less lowest), what its land gives (its biome at its centre) and the height of its ground (its
    /// chunks' lowest surfaces' median; none where none is surveyed), and whether a river cuts it (`Terrain.river`:
    /// water parting its land into sides that each alone reach some neighbour). Caravans go round water and cliffs by
    /// it and climb from hex to hex only so steeply, a wagon crosses a river only by a bridge and llamas ford it
    /// (`map.rs`), a town's hands gather what its land gives (`kit.rs`) and bridges go where it's wet (`works.rs`). A
    /// bridge the core finished stays.
    Ground { hex: HexAt, water: f64, relief: f64, land: LandKind, height: Option<f64>, river: bool },
    /// The crop plots the host counted in one chunk's part of a hex (`territory/Crops.java`: crop blocks within 16 of
    /// the bell's height, by the good working them gives), every crop there (one not listed has none). The core keeps
    /// each chunk's last count, so a chunk not loaded keeps what it last had; a town's fields are the plots counted in
    /// the hexes it holds (`land.rs`), set at its next `Survey`.
    Crops { hex: HexAt, chunk_x: i32, chunk_z: i32, counts: Vec<Count> },
    /// A town's land as the host found it this look (`land.rs`): its bell's block, where its land grows from and its
    /// wall rings. Its farm is the plots counted in the hexes it holds from now on, and the works it has to show come
    /// back as `Build` at the end of the look. Sent for a town whose ground is loaded.
    Survey { town: TownNo, bell_x: i32, bell_z: i32 },
    /// The host has laid `share` (0 to 1) of a town's work in its world (a `Build` shown): the core won't ask for that
    /// much again. Ignored for a work the town hasn't (a stale view).
    Built { town: TownNo, work: Work, share: f64 },
    /// One of a hand's locker offers switched on or off (the Merchant's screen, `ToggleTradePayload`): the offer at
    /// `offer`, its place among the hand's offers as `Answer::Lockers` lists them, which is of `good` (a check that the
    /// screen's view isn't stale). The Merchant runs a switched-on offer at its locker's restocks while it manages the
    /// locker. Ignored for a hand that isn't on a roster by the time it applies, or an offer that isn't there or isn't of
    /// that good (the screen showed what has changed since).
    Switch { hand: HandNo, offer: u32, good: GoodNo, on: bool },
    /// Where a hand's locker stands: its workstation, with a chest touching it (a `Locker` point of the hand's town's,
    /// placed with `AddPoint`), or nowhere (`None`: its workstation lost, its chest broken). In a town whose lockers its
    /// host places (every town founded by `AddTown`), a hand has a locker the Merchant can manage only where one stands;
    /// the point's chests are counted at each look and written as the town's (`Moved`): what the town uses is taken from
    /// them first, and what they still hold the Merchant carries to its desk and depots. A point is one hand's locker: a
    /// hand put where another's stood takes it. Ignored for a hand that isn't on a roster by the time it applies, or a
    /// point of another town's. Refused for a point that isn't a locker standing there.
    Locker { hand: HandNo, at: Option<Point> },
    /// Wagons from one town's market to another's, started (`send`) or stopped (the Merchant's screen,
    /// `CommissionPayload`; `/enterprise route add` and `remove`): started, a route opens if the far end is within a leg's
    /// food and there's a way there (a route wound down is opened again), and nothing happens if one is open already or
    /// none can be; stopped, an open route is wound down (its wagon on the road comes home and stays). `Answer::Routes`
    /// says what came of it. Refused for a town that isn't there, or a town to itself.
    Route { from: TownNo, to: TownNo, send: bool },
    /// A player has a town's Merchant pack goods from its stock for another town (`pallet_host.rs`): `goods` (no more of
    /// each than the town holds, whole items) on pallets made out to `consignee`, under no contract, each pallet within
    /// its kind's caps with a bill of lading of its own, in packages of each good's kind (the town's empty crates first,
    /// else new ones as far as it can spare the money; what can't be crated stays in stock). The pallets wait in the
    /// town's warehouse (`PalletPacked`) for a carrier: a wagon of the consignee's fetching them, or a player
    /// (`PalletBrought`). Refused for a town that isn't there, or a town to itself.
    Pack { town: TownNo, consignee: TownNo, goods: Vec<Count> },
    /// The host put a pallet's packages in a desk's chests (as `Answer::Stow` offered them): its crates are there now,
    /// one item a package, and the core won't offer it again while it stays where it is, as it is (`PalletView.shown`).
    /// Ignored for a pallet that isn't in that desk's town's warehouse by the time it applies (moved on, opened: a stale
    /// view). Refused for a point that isn't a desk standing there.
    Stowed { point: Point, pallet: u64 },
    /// A Numismatics vendor counted this look (`shop_host.rs`): a `Shop` point of a town's (placed with `AddPoint`), and
    /// what the host found in it. The core says what it is (a player's order or stall, the town's own shop, its board's, a
    /// deed's): a standing order drafted, a stall bought from, a shelf stocked and its takings banked. What it does about
    /// it comes back as `OrderFilled`, `StallBought`, `Shelf` and `Deed` at the end of the look. A vendor that deals in
    /// nothing the core knows (`Idle`) is forgotten. Sent in core mode only, once a look for every vendor near a loaded
    /// desk, before the look's `Advance`. Refused for a point that isn't a shop.
    Vendor { point: Point, read: VendorRead },
    /// A vendor the host put up for a town's shop board (`Output::BuildShop`): a `Shop` point of the town's, placed. Its
    /// cost is paid from the treasury now. The board's vendors show what the town is after at what it would pay, and a
    /// vendor the core knows as one stays so through a save. Refused for a point that isn't a shop.
    Board { point: Point },
    /// Where a rent's deed is sold (`Query::Deeds` lists the rents a dealer holds): the vendor, a `Shop` point of the
    /// dealer's town, or `None` to take the shop away from the rent (its vendor broken). Refused for a rent that isn't
    /// there or a point that isn't a shop.
    DeedShop { rent: u32, point: Option<Point> },
    /// A player brought a rent's deed to its holder's desk: the crop collected for the bearer is handed over as far as
    /// the town's stock goes (`Output::DeedPaid`). Ignored for a rent that isn't a player's or isn't this town's (the
    /// host's view was stale).
    DeedBrought { town: TownNo, rent: u32 },
    /// A market off the map opened (`crown_host.rs`, `merchant/OffMap.java`, `trade/Realms.java` `open`): a `Market` point
    /// at a real position (its roads are planned from there) with a town of its own and no one in it, called `name`,
    /// holding `spurs` of money paid in from outside, selling food and goods to caravans if `sells`. A `realm` ("coast":
    /// pipe tobacco, leaf, cocoa and bread, wanting wheat, paper, iron and books; "march": leather, wool and bread, wanting
    /// wheat, bread, carrots and iron; empty: none) makes what it makes and wants what it wants (it sells, whatever
    /// `sells` says, and is called as the realm is). Its wagons go to each of the `partners` it is routed to, each
    /// knowing the other. Refused for a point that isn't a free `Market` place, a realm the core hasn't, or a partner that
    /// isn't a town.
    OffMap { name: String, at: Point, sells: bool, realm: String, spurs: u64, partners: Vec<TownNo> },
    /// The sovereign's capital named in peacetime (`/enterprise war capital`; `Sovereign.establish`): the town's treasury
    /// takes `spurs_per_day` a day, its port's metal exchange opens, and its bank. A gold shop at another town's desk comes
    /// down (`Output::CrownShopDown`). Refused for a town that isn't there.
    Capital { town: TownNo, spurs_per_day: u64 },
    /// Mobilisation (`/enterprise war mobilise`; `Sovereign.mobilise`): peace first, then the capital mints `spurs_per_day`
    /// a day into its treasury and stockpiles. The core asks for a shop to buy gold by the capital (`Output::BuildCrownShop`).
    /// Refused for a town that isn't there.
    Mobilise { town: TownNo, spurs_per_day: u64 },
    /// War (`/enterprise war declare`; `Sovereign.declare`) against a `theatre` ("faction", "undead" or "nether"): the
    /// front opens and the towns the capital's wagons reach are levied (`Output::WarDeclared` says how many). Nothing
    /// happens with no capital. Refused for another theatre.
    Declare { theatre: String },
    /// Peace (`/enterprise war peace`; `Sovereign.peace`): the army walks home, the gold shop comes down
    /// (`Output::CrownShopDown`), the minting falls back to the crown's revenue.
    Peace,
    /// The vendor the host put up for the crown to buy gold at (`Output::BuildCrownShop`): a `Shop` point of the capital's,
    /// placed. The crown's treasury funds its card and the core takes what is sold into it. Refused for a point that isn't
    /// a shop.
    CrownShop { point: Point },
    /// The host has laid a harbour for a town (`territory/Harbour.java`): its jetty's root at block `x`, `z`. From then
    /// on its roads to another town with a port may go by ship, sailing water from port to port (`map.rs`
    /// `Carrier::Ship`), where that's cheaper than the road over land or there's no road over land. Sent again, the
    /// port moves. Refused for a town that isn't there.
    Port { town: TownNo, x: i32, z: i32 },
}

/// A villager a census saw: the host's tag for it (its UUID), its trade by the host's name (with or without the host's
/// namespace), its level (1 to 5; another is taken as the nearest of them) and whether it's a child.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Resident {
    pub tag: String,
    pub trade: String,
    pub level: u8,
    pub baby: bool,
}

/// So many of a good.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Count {
    pub good: GoodNo,
    pub count: f64,
}
