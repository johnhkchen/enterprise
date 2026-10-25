//! Shops, boards and rent deeds through the wire (`shop_host.rs`): a host counting Numismatics vendors (`Input::Vendor`),
//! shown what each should show (`OrderFilled`, `StallBought`, `Shelf`, `Deed`, `DeedPaid`, `BuildShop`) and asking what
//! its screens show (`Query::Shops`, `Query::Deeds`). The rules are `tests/orders.rs`' and `tests/harvest.rs`': this
//! pins what a host's vendors do to them. Paper at 24 an emerald, iron at 4, sticks at 32, wheat at 20; 16 spurs an emerald.

use enterprise_core::wire::*;
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;
const LOOK: u64 = 1_200;
const PAPER: u32 = 0;
const IRON: u32 = 1;
const STICK: u32 = 2;
const WHEAT: u32 = 3;
const COOKIE: u32 = 4;
const DESK: u32 = 0;

fn content() -> Content {
    let c = Content::new(Vec::new(), DAY);
    let treats = c.comfort("treats").unwrap();
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("iron_ingot", 1.0 / 4.0, 64),
        Good::new("stick", 1.0 / 32.0, 64), Good::new("wheat", 1.0 / 20.0, 64),
        Good::new("cookie", 1.0 / 18.0, 64).comfort(treats)], DAY)
}

fn at(kind: PointKind, x: i32, z: i32) -> Point {
    Point { kind, dim: "minecraft:overworld".into(), x, y: -60, z }
}

fn shop(n: i32) -> Point {
    at(PointKind::Shop, 30 + n, 20)
}

struct Host {
    society: Option<Society>,
}

impl Host {
    /// A market at (27, 27) whose Merchant is of `level`, holding `emeralds`, founded through the wire.
    fn new(level: u8, emeralds: f64) -> Host {
        let mut society = Society::new(content());
        society.content_mut().dials.standing_orders = false;
        let mut h = Host { society: Some(society) };
        h.apply(vec![Input::AddTown { name: "Desk".into(), desk: at(PointKind::Desk, 27, 27) }]);
        h.s().set_roster(DESK as usize, &[("merchant", level)]);
        h.s().set_people(DESK as usize, 1.0, 1.0, 1.0); // its Merchant in a bed (a homeless town keeps a home's price)
        h.s().pay(DESK as usize, emeralds);
        h
    }

    fn s(&mut self) -> &mut Society {
        self.society.as_mut().unwrap()
    }

    fn apply(&mut self, inputs: Vec<Input>) -> Vec<Output> {
        match handle(&mut self.society, Request::new(Command::Apply { inputs })) {
            Response::Done { outputs, .. } => outputs,
            r => panic!("{r:?}"),
        }
    }

    fn refused(&mut self, inputs: Vec<Input>) -> Refusal {
        match handle(&mut self.society, Request::new(Command::Apply { inputs })) {
            Response::Refused { why } => why,
            r => panic!("{r:?}"),
        }
    }

    fn ask(&mut self, query: Query) -> Answer {
        match handle(&mut self.society, Request::new(Command::Ask { queries: vec![query] })) {
            Response::Done { mut answers, .. } => answers.remove(0),
            r => panic!("{r:?}"),
        }
    }

    /// Vendors placed at the desk's town.
    fn place(&mut self, points: &[Point]) {
        self.apply(points.iter().map(|p| Input::AddPoint { point: p.clone(), town: DESK }).collect());
    }

    /// A look: the vendors counted, then time to `to`.
    fn look(&mut self, reads: Vec<(Point, VendorRead)>, to: u64) -> Vec<Output> {
        let mut inputs: Vec<Input> = reads.into_iter().map(|(point, read)| Input::Vendor { point, read }).collect();
        inputs.push(Input::Advance { to });
        self.apply(inputs)
    }

    fn shops(&mut self) -> (Vec<ShopLine>, Vec<Count>, Vec<SignalLine>, Vec<ListingLine>, TallyLine) {
        match self.ask(Query::Shops { town: DESK }) {
            Answer::Shops { shops, wanted, signals, listings, tally, .. } => (shops, wanted, signals, listings, tally),
            a => panic!("{a:?}"),
        }
    }

    fn stock(&mut self, good: u32) -> f64 {
        self.s().town(DESK as usize).stock[good as usize]
    }
}

fn read() -> VendorRead {
    VendorRead { mode: VendorMode::Idle, good: None, deed: None, lot: 0, price: 0, stock: 0, room: 0, purse: 0, coins: 0,
        touches: false }
}

/// A player's buying vendor: `lot` of `good` at `price` spurs, a card holding `purse`, room for `room` lots.
fn buying(good: u32, lot: u32, price: u32, purse: u64, room: u32) -> VendorRead {
    VendorRead { mode: VendorMode::Buy, good: Some(good), lot, price, purse, room, ..read() }
}

fn selling(good: u32, lot: u32, price: u32, stock: u32) -> VendorRead {
    VendorRead { mode: VendorMode::Sell, good: Some(good), lot, price, stock, ..read() }
}

// parity-partial: MarketTests.aMasterMerchantFillsAPlayersOrder (the contract and the fills in `tests/orders.rs`; here
// the vendor is a point the host counts, the fill told back for it to move: the goods in, the spurs off its card)
#[test]
fn a_buying_vendor_is_a_standing_order_and_its_fills_come_back_for_it() {
    let mut h = Host::new(5, 0.0);
    h.place(&[shop(0)]);
    h.apply(vec![Input::Deliver { town: DESK, good: PAPER, count: 64.0 }]);
    let before = h.s().totals();
    let outs = h.look(vec![(shop(0), buying(PAPER, 16, 16, 40, 54))], LOOK);
    assert!(outs.iter().any(|o| matches!(o, Output::OrderFilled { lots: 1, count, spurs, point: Some(p), .. }
        if *count == 16.0 && *spurs == 16.0 && *p == shop(0))), "the first lot today, for the vendor to take: {outs:?}");
    let (shops, ..) = h.shops();
    assert_eq!(shops.len(), 1);
    assert_eq!((shops[0].role, shops[0].good, shops[0].lot, shops[0].price), (ShopRole::Order, Some(PAPER), 16.0, 16.0));
    assert_eq!((shops[0].delivered, shops[0].paid, shops[0].purse), (16.0, 16.0, 24.0));
    // Tomorrow: the host counted its card at 24 spurs and the goods in it; the second lot.
    let outs = h.look(vec![(shop(0), VendorRead { stock: 16, ..buying(PAPER, 16, 16, 24, 53) })], LOOK + DAY);
    assert!(outs.iter().any(|o| matches!(o, Output::OrderFilled { lots: 1, .. })), "the second tomorrow: {outs:?}");
    assert_eq!(h.stock(PAPER), 32.0, "from the warehouse");
    h.s().conserved(&before).expect("goods and money are all accounted for");
}

#[test]
fn a_vendor_with_no_room_is_handed_nothing() {
    let mut h = Host::new(5, 0.0);
    h.place(&[shop(0)]);
    h.apply(vec![Input::Deliver { town: DESK, good: PAPER, count: 64.0 }]);
    let outs = h.look(vec![(shop(0), buying(PAPER, 16, 16, 160, 1))], LOOK);
    assert!(outs.iter().any(|o| matches!(o, Output::OrderFilled { lots: 1, .. })));
    // Room for one lot: a day's lot is due, but no second the same look, however many fell due.
    let outs = h.look(vec![(shop(0), buying(PAPER, 16, 16, 144, 0))], LOOK + 3 * DAY);
    assert!(!outs.iter().any(|o| matches!(o, Output::OrderFilled { .. })), "no room, no lot: {outs:?}");
    assert_eq!(h.stock(PAPER), 48.0);
}

/// A vendor the host didn't count (its chunk unloaded) is left as it is: no lot handed to an order it can't take in, no stall
/// bought from that can't be emptied, no shelf stocked it can't show. Counted again, it's dealt with again.
#[test]
fn a_vendor_the_host_didnt_count_is_left_as_it_is() {
    let mut h = Host::new(5, 10.0);
    h.place(&[shop(0), shop(1), shop(2)]);
    h.apply(vec![Input::Deliver { town: DESK, good: PAPER, count: 64.0 }, Input::Deliver { town: DESK, good: STICK, count: 64.0 }]);
    let before = h.s().totals();
    let own = VendorRead { touches: true, ..selling(STICK, 16, 9, 0) };
    let outs = h.look(vec![(shop(0), buying(PAPER, 16, 16, 160, 54)), (shop(1), selling(IRON, 4, 12, 8)),
        (shop(2), VendorRead { mode: VendorMode::Idle, good: None, lot: 0, price: 0, ..own.clone() })], LOOK);
    assert!(outs.iter().any(|o| matches!(o, Output::OrderFilled { lots: 1, .. })), "{outs:?}");
    assert!(outs.iter().any(|o| matches!(o, Output::Shelf { add: 64, .. })), "{outs:?}");
    let sticks = h.stock(STICK);
    // Its owner walked away: days of looks with none of them counted.
    let outs = h.look(vec![], LOOK + 3 * DAY);
    assert!(outs.iter().all(|o| !matches!(o, Output::OrderFilled { .. } | Output::StallBought { .. } | Output::Shelf { .. })),
        "nothing done at vendors the host can't show: {outs:?}");
    assert_eq!((h.stock(PAPER), h.stock(STICK)), (48.0, sticks), "nothing out of the warehouse for them");
    let (shops, ..) = h.shops();
    assert_eq!(shops.iter().find(|l| l.role == ShopRole::Order).map(|l| (l.delivered, l.purse)), Some((16.0, 144.0)));
    h.s().conserved(&before).expect("goods and money are all accounted for");
    // Back: counted again, the order is filled again.
    let outs = h.look(vec![(shop(0), VendorRead { stock: 16, ..buying(PAPER, 16, 16, 144, 53) })], LOOK + 3 * DAY + LOOK);
    assert!(outs.iter().any(|o| matches!(o, Output::OrderFilled { .. })), "counted again, filled again: {outs:?}");
}

#[test]
fn a_vendor_changing_what_it_buys_is_a_new_order_and_one_with_nothing_set_is_forgotten() {
    let mut h = Host::new(5, 0.0);
    h.place(&[shop(0)]);
    h.look(vec![(shop(0), buying(IRON, 4, 24, 48, 10))], LOOK);
    let (shops, wanted, ..) = h.shops();
    assert_eq!((shops[0].good, wanted.len()), (Some(IRON), 1), "{shops:?} {wanted:?}");
    h.look(vec![(shop(0), buying(STICK, 8, 8, 48, 10))], 2 * LOOK);
    let (shops, wanted, ..) = h.shops();
    assert_eq!(shops[0].good, Some(STICK));
    assert!(wanted.iter().all(|w| w.good == STICK), "the iron order is closed: {wanted:?}");
    h.look(vec![(shop(0), read())], 3 * LOOK);
    let (shops, wanted, ..) = h.shops();
    assert_eq!((shops[0].role, wanted.len()), (ShopRole::Idle, 0), "nothing set: no order");
}

// parity-partial: MarketTests.itBuysFromAPlayersStallForItsOrders (the buying in `tests/orders.rs`; here the vendor is told
// what was taken from it and what it was paid)
#[test]
fn a_selling_vendor_is_bought_from_for_the_orders_and_told_what_it_was_paid() {
    let mut h = Host::new(5, 10.0);
    h.place(&[shop(0), shop(1)]);
    let before = h.s().totals();
    let outs = h.look(vec![(shop(0), buying(IRON, 4, 24, 48, 10)), (shop(1), selling(IRON, 4, 12, 8))], LOOK);
    assert!(outs.iter().any(|o| matches!(o, Output::StallBought { good: IRON, lots: 2, count, spurs, point }
        if *count == 8.0 && *spurs == 24.0 && *point == shop(1))), "the stall's two lots: {outs:?}");
    let (shops, ..) = h.shops();
    assert_eq!(shops.iter().find(|l| l.role == ShopRole::Stall).map(|l| l.stock), Some(0.0));
    let outs = h.look(vec![(shop(0), buying(IRON, 4, 24, 48, 10)), (shop(1), selling(IRON, 4, 12, 0))], LOOK + DAY);
    assert!(outs.iter().any(|o| matches!(o, Output::OrderFilled { good: IRON, .. })), "and fills the order: {outs:?}");
    let (_, _, _, _, tally) = h.shops();
    assert_eq!((tally.spurs_from_sales, tally.spurs_on_purchases, tally.lots_bought), (48.0, 24.0, 2.0), "{tally:?}");
    h.s().conserved(&before).expect("goods and money are all accounted for");
}

// parity-partial: MarketTests.itsOwnShopSellsTheSurplus (the listing and the banking in `tests/orders.rs`; here the vendor
// is told what to show and counted again)
#[test]
fn the_towns_own_shop_shows_its_surplus_and_banks_what_the_host_collected() {
    let mut h = Host::new(5, 0.0);
    h.place(&[shop(0)]);
    h.apply(vec![Input::Deliver { town: DESK, good: STICK, count: 64.0 }]);
    let before = h.s().totals();
    let own = VendorRead { touches: true, ..selling(STICK, 16, 9, 0) };
    let mut outs = h.look(vec![(shop(0), VendorRead { good: None, mode: VendorMode::Idle, lot: 0, price: 0, ..own.clone() })], LOOK);
    let shelf = outs.iter().find(|o| matches!(o, Output::Shelf { .. })).cloned();
    assert_eq!(shelf, Some(Output::Shelf { point: shop(0), good: Some(STICK), lot: 16, price: 9, add: 64, clear: false }),
        "the vendor shows the sticks, in lots of 16 at a tenth over their worth, a stack on the shelf: {outs:?}");
    assert_eq!(h.stock(STICK), 0.0, "all on its shelf");
    h.s().conserved(&before).expect("goods on a shelf are still the world's");
    // Counted again with a lot bought and its coin collected: sold, banked at the next look; nothing to show.
    outs = h.look(vec![(shop(0), VendorRead { stock: 48, coins: 9, ..own.clone() })], 2 * LOOK);
    assert!(!outs.iter().any(|o| matches!(o, Output::Shelf { .. })), "unchanged but for what was sold: {outs:?}");
    assert_eq!(h.s().tally(DESK as usize).spurs_from_own_shops, 9.0, "and the town banks the takings");
    h.s().conserved(&before).expect("a lot sold is used");
    let (shops, ..) = h.shops();
    assert_eq!((shops[0].role, shops[0].stock), (ShopRole::Own, 48.0));
    // The Merchant stocks it again from what comes in: the shelf topped up to a stack.
    h.apply(vec![Input::Deliver { town: DESK, good: STICK, count: 64.0 }]);
    outs = h.look(vec![(shop(0), VendorRead { stock: 48, ..own })], 3 * LOOK);
    assert!(outs.contains(&Output::Shelf { point: shop(0), good: Some(STICK), lot: 16, price: 9, add: 16, clear: false }),
        "16 more: {outs:?}");
}

#[test]
fn a_shop_that_shows_something_else_is_shown_again_and_cleared() {
    let mut h = Host::new(5, 0.0);
    h.place(&[shop(0)]);
    h.apply(vec![Input::Deliver { town: DESK, good: STICK, count: 64.0 }]);
    let own = VendorRead { touches: true, ..selling(STICK, 16, 9, 0) };
    h.look(vec![(shop(0), VendorRead { mode: VendorMode::Idle, good: None, lot: 0, price: 0, ..own.clone() })], LOOK);
    // A player set it to iron: the next count shows it, the sticks put back on the shelf and the rest cleared.
    let outs = h.look(vec![(shop(0), VendorRead { good: Some(IRON), stock: 3, ..own })], 2 * LOOK);
    assert!(outs.contains(&Output::Shelf { point: shop(0), good: Some(STICK), lot: 16, price: 9, add: 64, clear: true }), "{outs:?}");
}

#[test]
fn a_vendor_broken_takes_its_shop_with_it() {
    let mut h = Host::new(5, 0.0);
    h.place(&[shop(0), shop(1)]);
    h.look(vec![(shop(0), buying(IRON, 4, 24, 48, 10)), (shop(1), selling(STICK, 8, 4, 16))], LOOK);
    assert_eq!(h.shops().0.len(), 2);
    h.apply(vec![Input::RemovePoint { point: shop(0) }, Input::RemovePoint { point: shop(1) }]);
    let (shops, wanted, ..) = h.shops();
    assert!(shops.is_empty() && wanted.is_empty(), "{shops:?} {wanted:?}");
    assert!(h.s().orders(DESK as usize).is_empty() && h.s().stalls(DESK as usize).is_empty());
}

#[test]
fn what_isnt_a_shop_is_refused() {
    let mut h = Host::new(5, 0.0);
    h.place(&[shop(0)]);
    let desk = at(PointKind::Desk, 27, 27);
    assert!(matches!(h.refused(vec![Input::Vendor { point: desk.clone(), read: read() }]), Refusal::Input { at: 0, .. }));
    assert!(matches!(h.refused(vec![Input::Vendor { point: shop(5), read: read() }]), Refusal::Input { at: 0, .. }), "none there");
    assert!(matches!(h.refused(vec![Input::Vendor { point: shop(0), read: buying(99, 1, 1, 0, 0) }]), Refusal::Input { at: 0, .. }),
        "no good 99");
    assert!(matches!(h.refused(vec![Input::Board { point: desk }]), Refusal::Input { at: 0, .. }));
    assert!(matches!(h.refused(vec![Input::DeedShop { rent: 0, point: Some(shop(0)) }]), Refusal::Input { at: 0, .. }), "no rent 0");
    assert!(matches!(h.refused(vec![Input::DeedBrought { town: 9, rent: 0 }]), Refusal::Input { at: 0, .. }));
}

// parity-partial: MoodTests.theShopListsWhatTheTownIsAfterAtWhatItWouldPay (the listing in `tests/desires.rs`; here the board's
// vendors show it, are asked for and paid for)
#[test]
fn the_shop_board_is_asked_for_paid_for_and_shows_what_the_town_is_after() {
    let mut h = Host::new(1, 40.0);
    {
        let s = h.s();
        s.set_people(DESK as usize, 5.0, 5.0, 0.0);
        let far = s.add_town("Far", 10.0);
        s.add_route(DESK as usize, far, 4.0 / 1200.0, 0.0, 4.0);
        s.deliver(far, WHEAT as usize, 4.0);
    }
    h.place(&[shop(0)]);
    let desk = at(PointKind::Desk, 27, 27);
    // A look that counts the desk: the town lists cookies (a comfort it has none of, a road for it), has no vendor for them.
    let outs = h.apply(vec![Input::Stocktake { point: desk.clone(), counts: vec![], free: 20 }, Input::Advance { to: LOOK }]);
    let (_, _, _, listings, _) = h.shops();
    assert!(listings.iter().any(|l| l.good == COOKIE), "cookies listed: {listings:?}");
    assert!(outs.contains(&Output::BuildShop { town: DESK }), "a vendor for them is asked for: {outs:?}");
    // The host puts one up (the point it placed): paid for, its shelf kept from now on.
    let money = h.s().town(DESK as usize).money;
    h.apply(vec![Input::Board { point: shop(0) }]);
    assert_eq!(money - h.s().town(DESK as usize).money, 4.0, "a vendor cost four emeralds");
    let outs = h.look(vec![(shop(0), read())], 2 * LOOK);
    let cookie = listings.iter().find(|l| l.good == COOKIE).unwrap();
    assert!(outs.contains(&Output::Shelf { point: shop(0), good: Some(COOKIE), lot: cookie.lot, price: cookie.spurs, add: 0, clear: false }),
        "the board's vendor shows the listing: {outs:?}");
    assert!(!outs.iter().any(|o| matches!(o, Output::BuildShop { .. })), "one vendor for one listing");
    // A cookie coming in goes on the shelf (a stack at most) at that price; the takings are banked at once.
    let (shops, ..) = h.shops();
    assert_eq!(shops[0].role, ShopRole::Board);
    h.apply(vec![Input::Deliver { town: DESK, good: COOKIE, count: 200.0 }]);
    let outs = h.apply(vec![Input::Vendor { point: shop(0), read: VendorRead { stock: 0, coins: 0, ..read() } }]);
    let shelf = outs.iter().find_map(|o| match o {
        Output::Shelf { good: Some(COOKIE), add, .. } => Some(*add),
        _ => None,
    });
    assert_eq!(shelf, Some(64), "a stack on the shelf: {outs:?}");
    let before = h.s().town(DESK as usize).money;
    h.apply(vec![Input::Vendor { point: shop(0), read: VendorRead { good: Some(COOKIE), lot: cookie.lot, price: cookie.spurs,
        stock: 60, coins: 32, mode: VendorMode::Sell, touches: true, ..read() } }]);
    let banked = h.s().town(DESK as usize).money - before;
    assert!(banked > 1.9 && banked < 2.1, "32 spurs banked at 16 an emerald: {banked}");
}

/// A hub (town 0) holding a rent on a farm (town 1), its deed on sale at a vendor.
fn dealer() -> (Host, usize) {
    let mut h = Host::new(1, 64.0);
    let farm = {
        let s = h.s();
        let farm = s.add_town("Farm", 100.0);
        s.make_hub(DESK as usize);
        farm
    };
    let rent = h.s().sell_rent(farm, DESK as usize, WHEAT as usize, 16.0, 0.5).expect("the dealer bought a rent");
    h.place(&[shop(0)]);
    h.apply(vec![Input::DeedShop { rent: rent as u32, point: Some(shop(0)) }]);
    (h, rent)
}

fn deed_read(mode: VendorMode, rent: usize, price: u32, stock: u32, coins: u64, purse: u64) -> VendorRead {
    VendorRead { mode, deed: Some(rent as u32), lot: 1, price, stock, coins, purse, room: 26, ..read() }
}

// parity-partial: MarketTests.aPlayerBuysARentsDeedCollectsItsCropAndSellsItBack (the prices in `tests/harvest.rs`; here the
// vendor sells the deed at the ask, buys it back at the bid from a funded card, and a player brings it to the desk)
#[test]
fn a_rents_deed_is_sold_at_the_ask_bought_back_at_the_bid_and_honoured_at_the_desk() {
    let (mut h, rent) = dealer();
    let line = match h.ask(Query::Deeds { town: DESK }) {
        Answer::Deeds { hub: true, mut deeds, .. } => deeds.remove(0),
        a => panic!("{a:?}"),
    };
    assert_eq!((line.rent as usize, line.bearer, line.point.clone()), (rent, false, Some(shop(0))));
    assert!(line.ask > line.bid && line.bid > 0, "the dealer's spread: {line:?}");
    // The vendor isn't set up yet: told to sell its deed at the ask.
    let outs = h.look(vec![(shop(0), read())], LOOK);
    assert!(outs.contains(&Output::Deed { point: shop(0), rent: rent as u32, selling: true, price: line.ask, fund: 0 }), "{outs:?}");
    // Set up as told, it's left alone.
    let outs = h.look(vec![(shop(0), deed_read(VendorMode::Sell, rent, line.ask, 1, 0, 0))], 2 * LOOK);
    assert!(!outs.iter().any(|o| matches!(o, Output::Deed { .. })), "{outs:?}");
    // A player bought it: its takings are the town's, the rent the player's, and the vendor buys it back at the bid, its
    // card funded from the treasury.
    let money = h.s().town(DESK as usize).money;
    let outs = h.look(vec![(shop(0), deed_read(VendorMode::Sell, rent, line.ask, 0, line.ask as u64, 0))], 3 * LOOK);
    assert!(h.s().rents()[rent].bearer, "the player's now");
    let fund = outs.iter().find_map(|o| match o {
        Output::Deed { selling: false, price, fund, .. } if *price == line.bid => Some(*fund),
        _ => None,
    }).unwrap_or_else(|| panic!("buys it back at the bid: {outs:?}"));
    assert!(fund as u32 >= line.bid, "the card funded to pay the bid: {fund}");
    let paid = h.s().cash_book(DESK as usize, 3).iter().filter(|(_, what, _)| what == "deed").map(|(_, _, v)| v.to_f64()).sum::<f64>();
    assert!(paid.abs() > 0.0, "the deed's takings and the card's funding are in the cash book");
    let _ = money;
    // The coupon's crop is collected in the dealer's keeping; the deed at the desk is handed it, as far as the stock goes.
    h.apply(vec![Input::Deliver { town: DESK, good: WHEAT, count: 10.0 }]);
    h.s().world.resource_mut::<enterprise_core::harvest::Rents>().rents[rent].accrued = 16.0;
    let before = h.s().totals();
    let outs = h.apply(vec![Input::DeedBrought { town: DESK, rent: rent as u32 }]);
    assert_eq!(outs, vec![Output::DeedPaid { town: DESK, rent: rent as u32, good: WHEAT, count: 10, owed: 6 }],
        "the wheat there is, and the rest owed");
    assert_eq!(h.stock(WHEAT), 0.0);
    h.s().conserved(&before).expect("the wheat handed over is used");
    // Someone else's deed, or none: nothing.
    assert!(h.apply(vec![Input::DeedBrought { town: 1, rent: rent as u32 }]).is_empty());
    // A player sold it back: the vendor holds the deed, the rent is the dealer's own again and the vendor sells it.
    let outs = h.look(vec![(shop(0), deed_read(VendorMode::Buy, rent, line.bid, 1, 0, fund))], 4 * LOOK);
    assert!(!h.s().rents()[rent].bearer, "the dealer's own again");
    assert!(outs.iter().any(|o| matches!(o, Output::Deed { selling: true, .. })), "{outs:?}");
}

#[test]
fn a_deed_shop_whose_rent_is_gone_stands_empty() {
    let (mut h, rent) = dealer();
    // The farm bought it back (or defaulted): the dealer holds nothing to quote.
    assert!(h.s().redeem_rent(rent));
    let outs = h.look(vec![(shop(0), read())], LOOK);
    assert!(outs.contains(&Output::Shelf { point: shop(0), good: None, lot: 0, price: 0, add: 0, clear: true }), "{outs:?}");
    match h.ask(Query::Deeds { town: DESK }) {
        Answer::Deeds { deeds, .. } => assert!(deeds.is_empty()),
        a => panic!("{a:?}"),
    }
}

#[test]
fn a_town_that_isnt_a_hub_deals_in_no_deeds() {
    let mut h = Host::new(1, 64.0);
    match h.ask(Query::Deeds { town: DESK }) {
        Answer::Deeds { hub, deeds, .. } => assert!(!hub && deeds.is_empty()),
        a => panic!("{a:?}"),
    }
}

#[test]
fn the_shops_survive_a_save() {
    let mut h = Host::new(5, 10.0);
    h.place(&[shop(0), shop(1), shop(2)]);
    h.apply(vec![Input::Deliver { town: DESK, good: STICK, count: 64.0 }]);
    h.look(vec![(shop(0), buying(IRON, 4, 24, 48, 10)), (shop(1), selling(IRON, 4, 12, 8)),
        (shop(2), VendorRead { touches: true, ..read() })], LOOK);
    let shops = h.shops().0;
    let bytes = h.s().save();
    let mut loaded = Host { society: Some(Society::load(&bytes).expect("loads")) };
    assert_eq!(loaded.shops().0, shops);
}

/// A homeless town isn't asked for a board's vendor out of a home's price, from its first look: the reserve for the
/// home is reckoned from its people as they are, not as the last touch left it (a new town spent 12 of its 32 on its
/// board's vendors before any touch, and stayed homeless).
#[test]
fn a_homeless_town_keeps_a_homes_price_from_its_boards_vendors() {
    let asked = |beds: f64| {
        let mut h = Host::new(1, 13.0);
        {
            let s = h.s();
            s.set_people(DESK as usize, 5.0, beds, 20.0);
            let far = s.add_town("Far", 10.0);
            s.add_route(DESK as usize, far, 4.0 / 1200.0, 0.0, 4.0);
            s.deliver(far, WHEAT as usize, 4.0);
        }
        h.place(&[shop(0)]);
        let desk = at(PointKind::Desk, 27, 27);
        let outs = h.apply(vec![Input::Stocktake { point: desk, counts: vec![], free: 20 }, Input::Advance { to: LOOK }]);
        outs.contains(&Output::BuildShop { town: DESK })
    };
    assert!(asked(5.0), "in beds, 13 emeralds: a vendor at 4");
    assert!(!asked(0.0), "five without beds: 10 of the 13 kept for a home, 3 left");
}
