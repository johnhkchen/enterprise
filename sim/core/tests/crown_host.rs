//! The crown, its realms and its markets off the map through the wire (`crown_host.rs`): a host naming the capital,
//! mobilising, declaring war and making peace (`Input::Capital`, `Mobilise`, `Declare`, `Peace`), opening a market off
//! the map as a point (`Input::OffMap`), putting up a vendor for the crown to buy gold at (`Output::BuildCrownShop`,
//! `Input::CrownShop`, then `Input::Vendor` each look), and asking `Query::Realms` and `Metals`. The rules are
//! `tests/crown.rs`' and `tests/realms.rs`': this pins what a host's commands and vendors do to them. 16 spurs an emerald.

use enterprise_core::army;
use enterprise_core::wire::*;
use enterprise_core::{Content, Fixed, Good, Society};

const DAY: u64 = 24_000;
const LOOK: u64 = 1_200;

fn content() -> Content {
    let mut goods: Vec<Good> = ["bread", "cooked_beef"].iter().map(|n| army::food_good(n, 64)).collect();
    goods.extend(["gold_ingot", "iron_ingot", "wheat", "paper", "book"].iter().map(|n| Good::priced(n, 64)));
    Content::new(goods, DAY)
}

fn at(kind: PointKind, x: i32, z: i32) -> Point {
    Point { kind, dim: "minecraft:overworld".into(), x, y: -60, z }
}

struct Host {
    society: Option<Society>,
}

impl Host {
    /// Two towns forty blocks apart, founded through the wire: the capital (0) with a Merchant, Vale (1) with a Merchant and
    /// four idle hands, each with bread and money.
    fn new() -> Host {
        let mut h = Host { society: Some(Society::new(content())) };
        h.apply(vec![
            Input::AddTown { name: "Crownmere".into(), desk: at(PointKind::Desk, 27, 27) },
            Input::AddTown { name: "Vale".into(), desk: at(PointKind::Desk, 67, 27) },
        ]);
        h.s().set_roster(0, &[("merchant", 3)]);
        h.s().set_roster(1, &[("merchant", 3), ("none", 1), ("none", 1), ("none", 1), ("none", 1)]);
        let bread = h.s().content().good("bread").unwrap();
        for t in 0..2 {
            h.s().deliver(t, bread, 256.0);
            h.s().pay(t, 500.0);
        }
        h.apply(vec![Input::Route { from: 0, to: 1, send: true }]);
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

    fn points(&mut self) -> Vec<TownPoint> {
        match self.ask(Query::Points) {
            Answer::Points { points } => points,
            a => panic!("{a:?}"),
        }
    }

    fn used(&mut self, good: u32) -> f64 {
        self.s().world.resource::<enterprise_core::state::Ledger>().used[good as usize]
    }

    fn gold(&mut self) -> u32 {
        self.s().content().good("gold_ingot").unwrap() as u32
    }
}

fn read() -> VendorRead {
    VendorRead { mode: VendorMode::Idle, good: None, deed: None, lot: 0, price: 0, stock: 0, room: 0, purse: 0, coins: 0,
        touches: false }
}

/// The gold shop mobilising brings: a vendor 10 blocks from the capital's desk, counted as it is.
fn mobilised() -> (Host, Point) {
    let mut h = Host::new();
    let shop = at(PointKind::Shop, 27, 17);
    h.apply(vec![Input::Capital { town: 0, spurs_per_day: 16 }]);
    let out = h.apply(vec![Input::Mobilise { town: 0, spurs_per_day: 480 }]);
    assert_eq!(out, vec![Output::BuildCrownShop { town: 0 }]);
    h.apply(vec![Input::AddPoint { point: shop.clone(), town: 0 }, Input::CrownShop { point: shop.clone() }]);
    (h, shop)
}

// parity-partial: CaravanTests.aCaravanDeliversToAMarketOffTheMap (the market off the map as a point the host opens; the
// delivery to it and the wagon are `tests/realms.rs`')
#[test]
fn a_market_off_the_map_is_a_town_at_a_point_with_money_and_no_one_in_it() {
    let mut h = Host::new();
    let far = at(PointKind::Market, 27, -1_473);
    let out = h.apply(vec![Input::OffMap { name: "the front".into(), at: far.clone(), sells: false, realm: String::new(),
        spurs: 1_600, partners: vec![0] }]);
    assert!(out.is_empty(), "{out:?}");
    assert_eq!(h.s().town_count(), 3);
    assert!(h.s().is_off_map(2));
    assert!(!h.s().is_realm(2), "a plain market, not a realm");
    assert_eq!(h.s().town(2).money, Fixed::from(100.0), "1,600 spurs at 16 an emerald, paid in from outside");
    assert!(h.points().iter().any(|p| p.point == far && p.town == 2), "listed with the host's points");
    // Its road to the town it trades with: a wagon can go from there.
    let routes = match h.ask(Query::Routes { town: 2 }) {
        Answer::Routes { routes, .. } => routes,
        a => panic!("{a:?}"),
    };
    assert_eq!(routes.len(), 1);
    assert_eq!((routes[0].from, routes[0].to), (2, 0));
    // A host's markets list the market by its point.
    match h.ask(Query::Routes { town: 0 }) {
        Answer::Routes { markets, .. } => assert!(markets.iter().any(|m| m.town == 2 && m.desk.as_ref() == Some(&far)), "{markets:?}"),
        a => panic!("{a:?}"),
    }
    // It has no chests: a stocktake or a cash count there is refused as no desk.
    assert!(matches!(h.refused(vec![Input::Cashcount { point: far.clone(), emeralds: 0, coins: 0, bank: None, coin_room: false,
        writes: false }]), Refusal::Input { at: 0, .. }));
}

// (whether a market off the map sells to caravans, as `OffMap.open`'s `sells` says)
#[test]
fn a_market_that_sells_is_told_to_the_core_as_selling() {
    let mut h = Host::new();
    h.apply(vec![Input::OffMap { name: "Over the sea".into(), at: at(PointKind::Market, -2_400, 27), sells: true,
        realm: String::new(), spurs: 0, partners: vec![] }]);
    assert!(h.s().is_off_map(2));
    h.apply(vec![Input::OffMap { name: "Beyond".into(), at: at(PointKind::Market, -2_400, 99), sells: false,
        realm: String::new(), spurs: 0, partners: vec![] }]);
    let sells = |h: &mut Host, t: usize| {
        let e = h.s().world.resource::<enterprise_core::state::Towns>().0[t];
        h.s().world.get::<enterprise_core::realms::OffMap>(e).unwrap().sells_food
    };
    assert!(sells(&mut h, 2));
    assert!(!sells(&mut h, 3));
}

// (`Realms.open`: a realm's market, money, three days' goods and roads to its partners, opened by its host)
#[test]
fn a_realm_opened_as_a_point_makes_its_goods_and_trades_with_its_partners() {
    let mut h = Host::new();
    let coast = at(PointKind::Market, -2_400, 27);
    h.apply(vec![Input::OffMap { name: "whatever".into(), at: coast.clone(), sells: false, realm: "coast".into(),
        spurs: 20_000, partners: vec![0, 1] }]);
    assert!(h.s().is_realm(2));
    let view = h.s().realm(2).unwrap();
    assert!(!view.wants.is_empty() || !view.makes.is_empty());
    let bread = h.s().content().good("bread").unwrap();
    assert!(h.s().town(2).stock[bread] > 0.0, "three days' goods on hand as it opens");
    assert_eq!(h.s().town(2).money, Fixed::from(1_250.0), "20,000 spurs");
    let a = h.ask(Query::Realms);
    let Answer::Realms { realms } = a else { panic!("{a:?}") };
    assert_eq!(realms.len(), 1);
    assert_eq!((realms[0].name.as_str(), realms[0].partners, realms[0].trips), ("the Coast", 2, 0));
    assert!((realms[0].in_hand_spurs - 20_000.0).abs() < 1.0, "{:?}", realms[0]);
    // Its wagons run to both partners.
    let routes = match h.ask(Query::Routes { town: 2 }) {
        Answer::Routes { routes, .. } => routes,
        a => panic!("{a:?}"),
    };
    assert_eq!(routes.iter().map(|r| r.to).collect::<Vec<_>>(), vec![0, 1]);
    // The March is the other.
    h.apply(vec![Input::OffMap { name: "x".into(), at: at(PointKind::Market, 3_000, 27), sells: false, realm: "march".into(),
        spurs: 0, partners: vec![] }]);
    let Answer::Realms { realms } = h.ask(Query::Realms) else { panic!() };
    assert_eq!(realms.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["the Coast", "the March"]);
}

#[test]
fn a_market_off_the_map_is_refused_where_it_cant_be() {
    let mut h = Host::new();
    let far = at(PointKind::Market, 100, 100);
    h.apply(vec![Input::OffMap { name: "A".into(), at: far.clone(), sells: true, realm: String::new(), spurs: 0, partners: vec![] }]);
    let open = |at: Point, realm: &str, partners: Vec<u32>| Input::OffMap { name: "B".into(), at, sells: true, realm: realm.into(),
        spurs: 0, partners };
    assert!(matches!(h.refused(vec![open(far.clone(), "", vec![])]), Refusal::Input { at: 0, .. }), "a place taken");
    assert!(matches!(h.refused(vec![open(at(PointKind::Desk, 7, 7), "", vec![])]), Refusal::Input { at: 0, .. }), "a desk");
    assert!(matches!(h.refused(vec![open(at(PointKind::Market, 7, 7), "narnia", vec![])]), Refusal::Input { at: 0, .. }), "a realm");
    assert!(matches!(h.refused(vec![open(at(PointKind::Market, 7, 7), "", vec![9])]), Refusal::Input { at: 0, .. }), "a partner");
    // Twice in one look at one place: the second is refused and nothing of the look is applied.
    let towns = h.s().town_count();
    assert!(matches!(h.refused(vec![open(at(PointKind::Market, 8, 8), "", vec![]), open(at(PointKind::Market, 8, 8), "", vec![])]),
        Refusal::Input { at: 1, .. }));
    assert_eq!(h.s().town_count(), towns);
    // A partner opened earlier in the same look is a town for the market after it.
    let next = h.s().town_count() as u32;
    h.apply(vec![open(at(PointKind::Market, 8, 8), "", vec![]), open(at(PointKind::Market, 9, 9), "", vec![next])]);
}

// parity-partial: WarTests.mobilisingTheSovereignMintsAndBuysGoldThroughAShop (the capital named, mobilising: the treasury
// mints and the vendor is asked for)
#[test]
fn the_capital_is_named_mobilises_and_asks_for_a_gold_shop() {
    let mut h = Host::new();
    h.apply(vec![Input::Capital { town: 0, spurs_per_day: 16 }]);
    assert_eq!(h.s().capital(), Some(0));
    // The port's exchange opens with the market across the sea: a point of its own.
    let markets: Vec<_> = h.points().into_iter().filter(|p| p.point.kind == PointKind::Market).collect();
    assert_eq!(markets.len(), 1, "{markets:?}");
    assert!(h.s().is_off_map(markets[0].town as usize));
    let Answer::Metals { desk, phase, metals } = h.ask(Query::Metals) else { panic!() };
    assert_eq!((desk, phase.as_str()), (Some(0), "peace"));
    assert!(metals.iter().any(|m| m.name.ends_with("iron_ingot") && m.target > 0.0), "{metals:?}");
    let treasury = h.s().town(0).money;
    let out = h.apply(vec![Input::Mobilise { town: 0, spurs_per_day: 480 }]);
    assert_eq!(out, vec![Output::BuildCrownShop { town: 0 }]);
    assert!(h.s().town(0).money > treasury, "today's coin is minted at once");
    let Answer::War { phase, capital, .. } = h.ask(Query::War) else { panic!() };
    assert_eq!((phase.as_str(), capital), ("mobilising", Some(0)));
    let Answer::Metals { phase, .. } = h.ask(Query::Metals) else { panic!() };
    assert_eq!(phase, "mobilising");
    // Mobilising again with a shop standing asks for none.
    let shop = at(PointKind::Shop, 27, 17);
    h.apply(vec![Input::AddPoint { point: shop.clone(), town: 0 }, Input::CrownShop { point: shop }]);
    assert!(h.apply(vec![Input::Mobilise { town: 0, spurs_per_day: 480 }]).is_empty());
    // Not a town: refused.
    assert!(matches!(h.refused(vec![Input::Capital { town: 5, spurs_per_day: 1 }]), Refusal::Input { at: 0, .. }));
    assert!(matches!(h.refused(vec![Input::Mobilise { town: 5, spurs_per_day: 1 }]), Refusal::Input { at: 0, .. }));
}

// parity-partial: WarTests.mobilisingTheSovereignMintsAndBuysGoldThroughAShop (the vendor buying gold: its card funded
// from the treasury, the gold sold into it the crown's)
#[test]
fn the_gold_shop_is_funded_from_the_treasury_and_the_gold_sold_into_it_is_the_crowns() {
    let (mut h, shop) = mobilised();
    let gold = h.gold();
    let before = h.s().totals();
    let worth = h.s().content().goods[gold as usize].value * 8.0 * h.s().content().dials.gold_premium * h.s().mint_rate();
    // New, set to nothing: the core says what it shows (8 ingots at three times their worth, in spurs at the mint's rate).
    let out = h.apply(vec![Input::Vendor { point: shop.clone(), read: read() }, Input::Advance { to: LOOK }]);
    let shown = out.iter().find_map(|o| match o {
        Output::CrownShop { point, good, lot, price, fund, taken } if *point == shop => Some((*good, *lot, *price, *fund, *taken)),
        _ => None,
    }).expect("the crown shop is shown");
    assert_eq!((shown.0, shown.1, shown.2), (gold, 8, worth.ceil() as u32), "{shown:?}");
    assert_eq!(shown.4, 0);
    // Set to buy gold with a card holding nothing: funded to two lots' price, from the treasury.
    let treasury = h.s().town(0).money;
    let buying = VendorRead { mode: VendorMode::Buy, good: Some(gold), lot: 8, price: shown.2, room: 54, ..read() };
    let out = h.apply(vec![Input::Vendor { point: shop.clone(), read: buying.clone() }, Input::Advance { to: 2 * LOOK }]);
    let fund = out.iter().find_map(|o| match o { Output::CrownShop { fund, .. } => Some(*fund), _ => None }).expect("funded");
    assert_eq!(fund, 2 * shown.2 as u64, "two lots' price");
    assert!(h.s().town(0).money < treasury, "paid out of the capital's treasury");
    // Counted again with the card as funded and nothing sold: nothing to show.
    let out = h.apply(vec![Input::Vendor { point: shop.clone(), read: VendorRead { purse: fund, ..buying.clone() } },
        Input::Advance { to: 3 * LOOK }]);
    assert!(!out.iter().any(|o| matches!(o, Output::CrownShop { .. })), "{out:?}");
    // A player sold 16 ingots: they're the crown's, to be taken out; the card, two lots down, is made up.
    let mint = h.used(gold);
    let out = h.apply(vec![Input::Vendor { point: shop.clone(), read: VendorRead { stock: 16, purse: fund - 2 * shown.2 as u64,
        ..buying.clone() } }, Input::Advance { to: 4 * LOOK }]);
    assert!(out.iter().any(|o| matches!(o, Output::CrownShop { taken: 16, fund: f, .. } if *f == 2 * shown.2 as u64)), "{out:?}");
    assert_eq!(h.used(gold) - mint, 16.0, "sent to the mint");
    h.s().conserved(&before).expect("goods and money are all accounted for");
}

#[test]
fn a_gold_shop_with_no_money_behind_it_is_not_funded() {
    let (mut h, shop) = mobilised();
    let gold = h.gold();
    let treasury = h.s().town(0).money;
    let drained = treasury - Fixed::from(0.5);
    h.s().post(enterprise_core::cash_book::Account::Treasury(0), enterprise_core::cash_book::Account::Outside, drained,
        enterprise_core::cash_book::Reason::Withdrawn);
    h.apply(vec![Input::Vendor { point: shop.clone(), read: read() }, Input::Advance { to: LOOK }]);
    let price = 1_152;
    let out = h.apply(vec![Input::Vendor { point: shop.clone(), read: VendorRead { mode: VendorMode::Buy, good: Some(gold), lot: 8,
        price, room: 54, ..read() } }, Input::Advance { to: 2 * LOOK }]);
    assert!(!out.iter().any(|o| matches!(o, Output::CrownShop { fund, .. } if *fund > 0)), "{out:?}");
}

#[test]
fn a_vendor_the_player_changed_is_shown_the_crowns_again() {
    let (mut h, shop) = mobilised();
    let gold = h.gold();
    let iron = h.s().content().good("iron_ingot").unwrap() as u32;
    h.apply(vec![Input::Vendor { point: shop.clone(), read: read() }, Input::Advance { to: LOOK }]);
    // Its filter changed to iron: its stock isn't the crown's, and the core shows its gold again.
    let out = h.apply(vec![Input::Vendor { point: shop.clone(), read: VendorRead { mode: VendorMode::Buy, good: Some(iron), lot: 8,
        price: 1, stock: 40, room: 54, ..read() } }, Input::Advance { to: 2 * LOOK }]);
    assert!(out.iter().any(|o| matches!(o, Output::CrownShop { good, taken: 0, .. } if *good == gold)), "{out:?}");
}

// parity-partial: WarTests.mobilisingTheSovereignMintsAndBuysGoldThroughAShop (peace: the gold shop comes down)
#[test]
fn war_is_declared_levies_the_towns_the_capital_reaches_and_peace_takes_the_gold_shop_down() {
    let (mut h, shop) = mobilised();
    h.apply(vec![Input::Advance { to: DAY }]);
    let out = h.apply(vec![Input::Declare { theatre: "faction".into() }]);
    assert_eq!(out, vec![Output::WarDeclared { levies: 1 }]);
    let Answer::War { phase, theatre, levies, .. } = h.ask(Query::War) else { panic!() };
    assert_eq!((phase.as_str(), theatre.as_str(), levies), ("war", "faction", 1));
    // The front: a point of its own, north of the capital.
    let markets: Vec<_> = h.points().into_iter().filter(|p| p.point.kind == PointKind::Market).collect();
    assert_eq!(markets.len(), 2, "the market across the sea and the front: {markets:?}");
    assert!(markets.iter().any(|p| p.point.z < -1_000 && p.point.x.abs() < 100), "{markets:?}");
    // A campaign's days.
    h.apply(vec![Input::Advance { to: 10 * DAY }]);
    let out = h.apply(vec![Input::Peace]);
    assert!(out.iter().any(|o| matches!(o, Output::PeaceMade { .. })), "{out:?}");
    assert!(out.contains(&Output::CrownShopDown { point: shop.clone() }), "{out:?}");
    let Answer::War { phase, .. } = h.ask(Query::War) else { panic!() };
    assert_eq!(phase, "peace");
    // A vendor counted after peace is no crown shop: it was forgotten, and it's counted as any vendor.
    let out = h.apply(vec![Input::Vendor { point: shop, read: read() }]);
    assert!(!out.iter().any(|o| matches!(o, Output::CrownShop { .. } | Output::CrownShopDown { .. })), "{out:?}");
    // War with no capital does nothing but say so.
    let mut bare = Host::new();
    assert_eq!(bare.apply(vec![Input::Declare { theatre: "undead".into() }]), vec![Output::WarDeclared { levies: 0 }]);
    assert!(matches!(bare.refused(vec![Input::Declare { theatre: "sea".into() }]), Refusal::Input { at: 0, .. }));
}

#[test]
fn a_gold_shop_counted_in_peace_comes_down_and_a_broken_vendor_is_forgotten() {
    let (mut h, shop) = mobilised();
    // Peace made behind the host's back (by the Rust API): the vendor counted next look is taken down.
    h.s().peace();
    let out = h.apply(vec![Input::Vendor { point: shop.clone(), read: read() }]);
    assert_eq!(out, vec![Output::CrownShopDown { point: shop.clone() }]);
    // A vendor broken forgets it too.
    let (mut h, shop) = mobilised();
    h.apply(vec![Input::RemovePoint { point: shop.clone() }]);
    assert_eq!(h.apply(vec![Input::Peace]).len(), 1, "only PeaceMade: no shop left to take down");
}

#[test]
fn a_crown_shop_must_be_a_shop_that_stands() {
    let mut h = Host::new();
    assert!(matches!(h.refused(vec![Input::CrownShop { point: at(PointKind::Desk, 27, 27) }]), Refusal::Input { at: 0, .. }));
    assert!(matches!(h.refused(vec![Input::CrownShop { point: at(PointKind::Shop, 1, 1) }]), Refusal::Input { at: 0, .. }));
}

#[test]
fn the_crown_is_saved_and_loaded() {
    let (mut h, shop) = mobilised();
    h.apply(vec![Input::Vendor { point: shop.clone(), read: read() }, Input::Advance { to: LOOK }]);
    let save = match handle(&mut h.society, Request::new(Command::Save)) {
        Response::Saved { save } => save,
        r => panic!("{r:?}"),
    };
    assert!(matches!(handle(&mut h.society, Request::new(Command::Load { save })), Response::Done { .. }));
    // It's the crown's shop still: peace takes it down.
    let out = h.apply(vec![Input::Peace]);
    assert!(out.contains(&Output::CrownShopDown { point: shop }), "{out:?}");
}
