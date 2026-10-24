//! Desires and backhauls (`desires.rs`, `rules/desires.rs`, `arrive.rs::sell`'s comfort branch, `plan.rs::plan_sale`),
//! pinned as the mod's caravan tests pin them: the test network's hub and far end four blocks apart (a point of food
//! a leg, 80 ticks on the road), paper at 24 an emerald, bread at 6 (5 food points), carrots at 22 (3 points), cookies
//! at 18 (treats), leather boots at 4 (clothing); the hub's Merchant at its desk and a squad of trained hands; the
//! far end's lockers take paper up to a stack (its par). A hub that isn't a town has no people (`set_people`: none).

use enterprise_core::atlas::Kind;
use enterprise_core::desires::{self, Facts, Knowledge, Listings, Outlet};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;
/// A look: the core looks every 1,200 ticks.
const LOOK: u64 = 1_200;
const PAPER: usize = 0;
const BREAD: usize = 1;
const CARROT: usize = 2;
const WHEAT: usize = 3;
const COOKIE: usize = 4;
const BOOTS: usize = 5;
const PIPE: usize = 6;
const PIE: usize = 7;
const HUB: usize = 0;
const DEST: usize = 1;

fn content() -> Content {
    let c = Content::new(Vec::new(), DAY);
    let kind = |name: &str| c.comfort(name).unwrap();
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("carrot", 1.0 / 22.0, 64).food(3.0).crop(), Good::new("wheat", 1.0 / 20.0, 64),
        Good::new("cookie", 1.0 / 18.0, 64).comfort(kind("treats")), Good::new("leather_boots", 4.0, 16).comfort(kind("clothing")),
        Good::new("pipe_tobacco", 0.5, 64).comfort(kind("leisure")),
        Good::new("pumpkin_pie", 0.25, 64).comfort(kind("treats"))], DAY)
}

/// The mod's test network (`CaravanTests.network`): a hub with 128 paper, `bread` and `emeralds`, its Merchant at the
/// desk and a squad of one Librarian (trained at level 2: no locker); a far end four blocks away holding `dest_spurs`
/// (16 an emerald) and two bread, whose lockers take paper up to a stack (its par). The hub's people are its two
/// hands; the far end has none until `town_of_five`.
fn network(bread: f64, emeralds: f64, dest_spurs: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", emeralds);
    let dest = s.add_town("Dest", dest_spurs / 16.0);
    s.record_capacity(dest, 1, &[(PAPER, 64.0 / 5.0)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 128.0);
    if bread > 0.0 {
        s.deliver(hub, BREAD, bread);
    }
    s.deliver(dest, BREAD, 2.0);
    s.set_roster(hub, &[("merchant", 1), ("librarian", 2)]);
    s.set_offers(hub, 1, Vec::new()); // trained: it keeps its trade without a workstation
    s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
    s
}

/// A town of five (`Census(5, 5, 5, 5, 0)`): the far end's people as a count, the hub's as three more hands.
fn town_of_five(s: &mut Society, town: usize) {
    if town == HUB {
        s.set_roster(HUB, &[("merchant", 1), ("librarian", 2), ("farmer", 1), ("farmer", 1), ("farmer", 1)]);
        s.set_offers(HUB, 1, Vec::new());
    } else {
        s.set_people(town, 5.0, 5.0, 5.0);
    }
}

/// What a route's wagon carries of a good, loose and crated.
fn aboard(s: &Society, route: usize, good: usize) -> f64 {
    let loose = s.wagon_cargo(route).map_or(0.0, |c| c[good]);
    loose + s.wagon_crated(route, good)
}

/// Its par of a kind: 5 villagers x 0.1 x `comfort_days`, in emeralds' worth.
const PAR: f64 = 5.0 * 0.1 * 6.0;

fn treats(s: &Society) -> usize {
    s.content().comfort("treats").unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

// parity: CaravanTests.caravansAllowForComfortsUsedSinceTheStocktake
#[test]
fn caravans_allow_for_comforts_used_since_the_stocktake() {
    let mut s = network(2.0, 5.0, 192.0);
    town_of_five(&mut s, DEST);
    let t = s.now();
    // At the last visit the town was full: its whole par of treats, in cookies at 18 an emerald.
    let par = (5.0 * 0.1 * s.dial("comfort_days").unwrap() * 18.0_f64).round();
    assert_eq!(par, 54.0);
    s.record_stocktake(DEST, &[(COOKIE, par)], Some(t));
    let kind = treats(&s);
    let now = s.comfort_wants(DEST, t);
    assert!(now[kind] < 1e-9, "just after: it wants no treats {now:?}");
    // Two days on, 5 villagers have eaten 1 emerald's worth: it wants that back.
    let later = s.comfort_wants(DEST, t + 2 * DAY);
    assert!(close(later[kind], 1.0), "two days on: {later:?}");
    // The other kinds it has none of: the whole par each.
    assert!(close(later[s.content().comfort("reading").unwrap()], PAR), "{later:?}");
}

#[test]
fn a_market_never_counted_is_read_as_it_stands_and_nothing_is_used() {
    let mut s = network(2.0, 5.0, 192.0);
    town_of_five(&mut s, DEST);
    s.deliver(DEST, COOKIE, 18.0); // a whole emerald of treats
    let kind = treats(&s);
    let wants = s.comfort_wants(DEST, 10 * DAY);
    assert!(close(wants[kind], PAR - 1.0), "{wants:?}: no stocktake, none used");
}

// parity: CaravanTests.aTownShortOfClothesBuysAPairOfBoots
#[test]
fn a_town_short_of_clothes_buys_a_pair_of_boots() {
    let mut s = network(2.0, 5.0, 192.0);
    town_of_five(&mut s, DEST);
    s.deliver(HUB, BOOTS, 3.0); // 4 emeralds each: more than the 3-emerald par
    s.advance(LOOK);
    // Half again what the town wants: a pair and another.
    assert_eq!(aboard(&s, 0, BOOTS), 2.0, "ceil(3 x 1.5 / 4) pairs");
    s.advance(2 * LOOK);
    assert_eq!(s.town(DEST).stock[BOOTS], 1.0, "one pair bought: whole items, past the gap's 0.75");
    s.advance(3 * LOOK);
    // (The hub's two hands wore a pair out at the first look.)
    assert_eq!(s.town(HUB).stock[BOOTS], 1.0, "and the one it didn't want came back with the wagon");
}

// parity: CaravanTests.townsBuyComfortsUpToTheirPar
#[test]
fn towns_buy_comforts_up_to_their_par() {
    let mut s = network(2.0, 5.0, 192.0);
    town_of_five(&mut s, DEST);
    let par = 54.0; // 5 x 0.1 x comfort_days, at 18 cookies an emerald
    s.deliver(HUB, COOKIE, 128.0);
    s.advance(LOOK);
    let carried = aboard(&s, 0, COOKIE);
    assert_eq!(carried, par * 3.0 / 2.0, "the wagon takes half again what the town wants: {carried}");
    s.advance(2 * LOOK);
    assert_eq!(s.town(DEST).stock[COOKIE], par, "the town buys its par of cookies");
}

// parity: CaravanTests.aTownsOwnCaravanShopsForHome
#[test]
fn a_towns_own_caravan_shops_for_home() {
    let mut s = network(2.0, 20.0, 192.0);
    town_of_five(&mut s, HUB); // the hub is a town of five, with no comforts
    s.deliver(DEST, COOKIE, 64.0); // the market it sends to has treats to spare
    s.advance(LOOK);
    let list = s.wagon_shopping(0).expect("it left");
    let kind = treats(&s);
    assert!(close(list[kind], PAR), "it shops for its par of treats: {list:?}");
    s.advance(2 * LOOK);
    // Its par of treats is 5 x 0.1 x comfort_days emeralds' worth, at 18 cookies an emerald.
    assert_eq!(aboard(&s, 0, COOKIE), 54.0, "it brought home its par of treats");
    assert!(s.wagon_shopping(0).unwrap().is_empty(), "and has shopped");
    s.advance(3 * LOOK);
    assert_eq!(s.town(HUB).stock[COOKIE], 54.0, "home with them");
    let (bought, paid, backhauled) = s.bought(0);
    assert_eq!((bought, backhauled), (54.0, 0.0));
    assert!(paid > 54.0 / 18.0 * 0.5 && paid <= 54.0 / 18.0, "at a price bargained up to their full worth: {paid}");
    // Both Merchants' books say so: the far end sold, the wagon's log says bought, and home's has it when it's home.
    assert!(s.books(DEST).iter().any(|e| e.kind == Kind::Sold && e.good == Some(COOKIE) && e.count == 54.0), "{:?}", s.books(DEST));
    assert!(s.books(HUB).iter().any(|e| e.kind == Kind::Bought && e.good == Some(COOKIE) && e.count == 54.0), "{:?}", s.books(HUB));
    assert!(s.cash_book(DEST, 3).iter().any(|(_, what, v)| what == "shopping" && *v > 0.0), "{:?}", s.cash_book(DEST, 3));
}

// parity: CaravanTests.aMerchantBuysToSellOnOnlyAtAMargin
#[test]
fn a_merchant_buys_to_sell_on_only_at_a_margin() {
    let s = network(2.0, 5.0, 192.0);
    let outlet = Outlet { market: DEST, each: 0.5, units: 40.0 };
    assert!(desires::worth_buying(s.content(), 0.40, &outlet), "0.40 against 0.50 there: worth it");
    assert!(!desires::worth_buying(s.content(), 0.45, &outlet), "0.45 leaves less than the 15% a Merchant wants: no");
    let blocks = s.road_food(0).blocks;
    let haul = desires::haul_each(s.content(), blocks, 0, 0, COOKIE);
    assert!(haul > 0.0 && haul < 0.01, "the road costs a cookie a fraction of a spur's worth: {haul}");
    // A round trip's food (2 x 1 point x 0.05) over a wagon's 27 stacks of 64.
    assert!(close(haul * 27.0 * 64.0, 2.0 * 0.05), "{haul}");
    // What the last wagon had: two mouths and the team's, over its slots.
    let heavy = desires::haul_each(s.content(), blocks, 4, 36, COOKIE);
    assert!(close(heavy * 36.0 * 64.0, 2.0 * 4.0 * 0.05), "{heavy}");
}

/// The network with another town on the hub's routes that wants carrots (its lockers take a stack's par of them), and
/// the far end holding `held` of them.
fn carrots(elsewhere: bool, held: f64) -> Society {
    let mut s = network(2.0, 5.0, 192.0);
    if elsewhere {
        let other = s.add_town("Elsewhere", 12.0);
        s.record_capacity(other, 1, &[(CARROT, 64.0 / 5.0)]);
        s.set_trade(other, CARROT, 0.0, 1e-9);
        s.add_route(HUB, other, 4.0 / 1200.0, 0.0, 4.0);
    }
    s.deliver(DEST, CARROT, held); // nobody there wants carrots: all surplus past food par
    s
}

// parity: CaravanTests.caravansBuyATownsSurplusForTheTripHome
#[test]
fn caravans_buy_a_towns_surplus_for_the_trip_home() {
    let mut s = carrots(true, 256.0);
    s.advance(LOOK);
    s.advance(2 * LOOK);
    let (bought, paid, backhauled) = s.bought(0);
    assert!(bought > 0.0, "the caravan bought carrots");
    let carried = aboard(&s, 0, CARROT);
    assert!(carried > 0.0 && carried <= bought, "carrots ride home (with any other surplus bought): {carried}");
    assert_eq!(backhauled, 0.0, "(surplus someone wants, not a glut)");
    // Only what the other town wants: its par, a stack (the hub holds none of its own).
    assert_eq!(carried, 64.0, "{carried}");
    // The town was paid for its surplus, at most three quarters of its worth.
    let book = s.cash_book(DEST, 3);
    let got: enterprise_core::Fixed = book.iter().filter(|(_, what, _)| what == "backhaul").map(|(_, _, v)| v).sum();
    // (Paid rounded up to the thousandth, as a price is: `fixed.rs`.)
    assert!(got > 0.0 && got <= paid && got <= enterprise_core::Fixed::ceil(64.0 / 22.0 * 0.75), "{book:?}");
    assert!(s.food_points(DEST) >= 320.0, "the town keeps its food par: {}", s.food_points(DEST));
}

// parity: CaravanTests.caravansDontBuySurplusNobodyWants
#[test]
fn caravans_dont_buy_surplus_nobody_wants() {
    let mut s = carrots(false, 256.0);
    s.advance(LOOK);
    s.advance(2 * LOOK);
    assert_eq!(s.bought(0).0, 0.0, "no town on the hub's routes wants carrots: none bought");
    assert!(s.town(DEST).stock[CARROT] >= 255.0, "(the wagon ate a carrot of road food)");
}

#[test]
fn surplus_food_is_bought_only_beyond_the_far_ends_food_par() {
    // 110 carrots are 330 points; with its 2 bread (10) that's 20 over the 320 it keeps: 6 carrots spare, though
    // another town wants a stack.
    let mut s = carrots(true, 110.0);
    s.advance(2 * LOOK);
    assert_eq!(aboard(&s, 0, CARROT), 6.0);
    // (It ate a carrot of road food after.)
    assert!(s.food_points(DEST) >= 320.0 - 3.0, "{}", s.food_points(DEST));
}

#[test]
fn a_merchant_buys_no_more_surplus_than_the_other_town_wants_less_what_it_holds() {
    // The hub holds 40 carrots already: another town's stack wants only 24 more.
    let mut s = carrots(true, 256.0);
    s.deliver(HUB, CARROT, 40.0);
    s.advance(2 * LOOK);
    let carried = aboard(&s, 0, CARROT);
    let held = s.stocktake(HUB).expect("counted as it left").stock[CARROT];
    assert_eq!(carried, 64.0 - held, "another town's stack, less what the hub held when it left");
}

#[test]
fn a_town_pays_up_to_twice_its_worth_half_when_it_has_its_fill() {
    let c = content();
    let value = 1.0 / 18.0;
    // Nothing of the kind had, plenty of money: twice the worth.
    assert!(close(desires::will_pay(&c, value, 0.0, 3.0, 100.0), 2.0 * value));
    // Half had: half the way up.
    assert!(close(desires::will_pay(&c, value, 0.5, 3.0, 100.0), 1.5 * value));
    // Fully met but still short of its par: the worth.
    assert!(close(desires::will_pay(&c, value, 1.0, 3.0, 100.0), value));
    // Holding its fill: half.
    assert!(close(desires::will_pay(&c, value, 0.0, 0.0, 100.0), 0.5 * value));
    // As far as its households and spare money can pay for what it's short of: money for half the shortfall, half the
    // price up; for none, the floor of half the worth.
    assert!(close(desires::will_pay(&c, value, 0.0, 3.0, 1.5), 1.0 * value));
    assert!(close(desires::will_pay(&c, value, 0.0, 3.0, 0.0), 0.5 * value));
    assert!(close(desires::will_pay(&c, value, 0.0, 3.0, -4.0), 0.5 * value), "(a treasury under its reserve adds nothing)");
}

#[test]
fn what_a_market_would_pay_is_its_shop_then_the_books_then_a_reckoning() {
    let mut s = network(2.0, 5.0, 192.0);
    town_of_five(&mut s, DEST);
    s.pay(DEST, 30.0);
    let value = 1.0 / 18.0;
    // Reckoned: no cookies there, none ever had, 40 emeralds to pay: twice the worth.
    assert!(close(s.will_pay(HUB, DEST, COOKIE), 2.0 * value), "{}", s.will_pay(HUB, DEST, COOKIE));
    // A good that's no comfort is worth its worth.
    assert!(close(s.will_pay(HUB, DEST, PAPER), 1.0 / 24.0));
    // The last price the Merchant's books show it sold there for beats the reckoning.
    s.file(HUB, vec![enterprise_core::atlas::Entry::new(0, Kind::Sold, DEST, COOKIE, 10.0, enterprise_core::Fixed::from(0.7))]);
    assert!(close(s.will_pay(HUB, DEST, COOKIE), 0.07), "{}", s.will_pay(HUB, DEST, COOKIE));
    // And the price its shop listed when a wagon last called beats that (see the shop's test).
    let c = content();
    let (stocktakes, books) = (Default::default(), Default::default());
    let mut listings = Listings::default();
    listings.seen.insert(DEST, [(COOKIE, 0.3)].into_iter().collect());
    let known = Knowledge { content: &c, now: 0, stocktakes: &stocktakes, books: &books, listings: &listings };
    let facts = Facts { villagers: 5.0, par: PAR, met: &[], purse: 0.0, spare: 0.0, stock: &[] };
    assert_eq!(known.will_pay(HUB, DEST, &facts, COOKIE), 0.3);
}

/// A home with cookies, a far end that is a town of five, and a town on the far end's own routes of `x_villagers`
/// (and 100 emeralds) holding `x_cookies`.
fn resale(x_villagers: f64, x_cookies: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let (home, far, x) = (s.add_town("Home", 20.0), s.add_town("Far", 200.0), s.add_town("Town", 100.0));
    s.set_roster(home, &[("merchant", 1), ("librarian", 2)]);
    s.set_offers(home, 1, Vec::new());
    s.set_people(far, 5.0, 5.0, 5.0);
    s.set_people(x, x_villagers, x_villagers, x_villagers);
    s.deliver(x, COOKIE, x_cookies);
    s.deliver(home, COOKIE, 200.0);
    s.add_route(home, far, 4.0 / 1200.0, 0.0, 4.0);
    s.add_route(far, x, 4.0 / 1200.0, 0.0, 4.0);
    s
}

/// What the far end's books say its Merchant bought of a good in all.
fn bought_of(s: &Society, town: usize, good: usize) -> f64 {
    s.books(town).iter().filter(|e| e.kind == Kind::Bought && e.good == Some(good)).map(|e| e.count).sum()
}

#[test]
fn a_merchant_supplying_other_towns_buys_comforts_for_them_too() {
    // The far end's own people have room for 54 cookies (its par); the town its routes supply has none.
    let mut s = resale(5.0, 0.0);
    s.advance(LOOK); // the wagon leaves with half again the far end's par
    assert_eq!(aboard(&s, 0, COOKIE), 81.0);
    let outlet = s.outlet(1, Some(0), COOKIE).expect("the town it supplies is short of cookies");
    assert_eq!(outlet.market, 2);
    assert_eq!(outlet.units, 54.0, "its whole par");
    assert!(outlet.each > 1.9 / 18.0 && outlet.each < 2.0 / 18.0, "twice the worth, less the road: {outlet:?}");
    s.advance(2 * LOOK);
    assert_eq!(bought_of(&s, 1, COOKIE), 81.0, "all of it: its own 54 and 27 more to sell on");
}

#[test]
fn without_an_outlet_a_town_buys_only_its_own_par() {
    let mut s = resale(0.0, 0.0); // the town it supplies has no people: nothing short
    assert_eq!(s.outlet(1, Some(0), COOKIE), None);
    s.advance(2 * LOOK);
    assert_eq!(bought_of(&s, 1, COOKIE), 54.0, "its own par and no more");
}

#[test]
fn a_merchant_wont_buy_to_sell_on_where_the_margin_is_thin() {
    // The town it supplies holds half its par and has eaten its rations (met): it pays only the worth, less the road,
    // which leaves no 15% for the Merchant.
    let mut s = resale(5.0, 27.0);
    s.advance(LOOK);
    assert!(s.town(2).people.comfort_met[treats(&s)] >= 1.0 - 1e-9, "{:?}", s.town(2).people.comfort_met);
    let outlet = s.outlet(1, Some(0), COOKIE).expect("still short of its par");
    assert!(outlet.each < 1.0 / 18.0 && !desires::worth_buying(s.content(), 1.0 / 18.0, &outlet), "{outlet:?}");
    s.advance(2 * LOOK);
    assert_eq!(bought_of(&s, 1, COOKIE), 54.0, "only its own people's room");
}

#[test]
fn a_wagon_takes_the_cheapest_goods_of_a_kind_first_and_a_little_over() {
    let c = content();
    let kind = c.comfort("treats").unwrap();
    let mut wants = vec![0.0; c.comforts.len()];
    wants[kind] = 3.0;
    let mut avail = vec![0.0; c.goods.len()];
    avail[COOKIE] = 100.0;
    avail[PIE] = 100.0;
    // Half again 3 emeralds is 4.5: cookies at 18 an emerald, 81 of them.
    assert_eq!(desires::pick_comforts(&c, &wants, &avail), vec![(COOKIE, 81.0)]);
    // With only 30 cookies, the rest of the 4.5 in pies at a quarter: ceil(2.83 / 0.25) = 12.
    avail[COOKIE] = 30.0;
    assert_eq!(desires::pick_comforts(&c, &wants, &avail), vec![(COOKIE, 30.0), (PIE, 12.0)]);
    // Nothing wanted of a kind, nothing taken.
    wants[kind] = 0.0;
    assert!(desires::pick_comforts(&c, &wants, &avail).is_empty());
}

#[test]
fn the_margin_a_wagon_takes_over_what_is_wanted_is_a_dial() {
    let mut s = network(2.0, 5.0, 192.0);
    town_of_five(&mut s, DEST);
    s.set_dial("comfort_margin", 1.0).unwrap();
    s.deliver(HUB, COOKIE, 128.0);
    s.advance(LOOK);
    assert_eq!(aboard(&s, 0, COOKIE), 54.0, "just what the town wants");
    assert_eq!(s.dial("desire_most"), Some(2.0));
    assert_eq!((s.dial("desire_glut"), s.dial("resale_margin"), s.dial("resale_ceiling")), (Some(0.5), Some(0.15), Some(0.75)));
}

#[test]
fn a_town_shops_only_with_money_worth_twice_the_road_food() {
    // Two mouths, a point a leg each: a round trip's food is 0.2; a trip is worth the road for at least half an emerald.
    let shopping_of = |emeralds: f64| {
        let mut s = network(2.0, emeralds, 192.0);
        town_of_five(&mut s, HUB);
        s.deliver(DEST, COOKIE, 64.0);
        s.advance(LOOK);
        s.wagon_shopping(0)
    };
    assert_eq!(shopping_of(0.5).map(|l| l.iter().sum::<f64>()), Some(0.0), "under the half emerald beyond the road: none");
    let rich = shopping_of(8.0).expect("it left");
    assert!(close(rich.iter().sum::<f64>(), 5.0 * PAR), "{rich:?}");
}

#[test]
fn the_accountant_limits_a_towns_shopping_and_the_wagon_gone_spends_the_budget() {
    let mut s = network(2.0, 20.0, 192.0);
    town_of_five(&mut s, HUB);
    s.deliver(DEST, COOKIE, 64.0);
    // Its day's budget is half its free money, 10; it has spent all but a little.
    let spent = s.allow(HUB, "works", 9.7);
    assert!(spent == 9.7);
    s.advance(LOOK);
    let shopping = s.wagon_shopping(0).expect("it left");
    assert_eq!(shopping.iter().sum::<f64>(), 0.0, "0.3 left of the budget: under the half emerald it takes: {shopping:?}");
    // A town with its budget whole sends the money, and the wagon gone, the budget is spent by it.
    let mut s = network(2.0, 20.0, 192.0);
    town_of_five(&mut s, HUB);
    s.deliver(DEST, COOKIE, 64.0);
    let before = s.budget_left(HUB);
    s.advance(LOOK);
    assert!(s.wagon_shopping(0).is_some_and(|l| l.iter().sum::<f64>() > 0.0));
    let left = s.budget_left(HUB);
    assert!(before - left == 10.0, "the shopping purse, 10 of the 10: {before} then {left}");
}

#[test]
fn a_town_buys_for_home_only_what_the_far_end_holds_beyond_its_own_keep() {
    let mut s = network(2.0, 20.0, 192.0);
    town_of_five(&mut s, HUB);
    town_of_five(&mut s, DEST); // the far end's people keep 4 days of rations: 2 emeralds' worth of treats
    s.deliver(DEST, COOKIE, 54.0); // its whole par, 3 emeralds' worth
    s.advance(2 * LOOK);
    // It spares about one emerald's worth, 18 cookies (a cookie or so having been eaten, or the sum a hair over).
    let got = aboard(&s, 0, COOKIE);
    assert!((16.0..=19.0).contains(&got), "{got}");
    assert!(s.town(DEST).stock[COOKIE] * (1.0 / 18.0) >= 2.0 - 1e-9, "it keeps its 2 emeralds' worth: {}", s.town(DEST).stock[COOKIE]);
}

#[test]
fn a_wagon_buys_a_glut_for_home_at_a_discount() {
    let mut s = network(2.0, 5.0, 192.0);
    s.set_trade(HUB, WHEAT, 0.0, 10.0); // the hub's people use wheat
    s.deliver(DEST, WHEAT, 1000.0); // the far end has a glut of it: a par of 64, a glut beyond 128
    s.advance(2 * LOOK);
    let (bought, paid, backhauled) = s.bought(0);
    assert!(bought > 0.0 && backhauled == bought, "bought {bought} of wheat as a backhaul: {backhauled}");
    // 1,000 held is 15 pars: half off the worth, the most a glut is discounted.
    assert!(paid == bought * 0.5 / 20.0, "{paid} for {bought}");
    assert!(s.cash_book(DEST, 3).iter().any(|(_, what, v)| what == "glut" && *v > 0.0));
    // And the wagon brings it home.
    assert_eq!(aboard(&s, 0, WHEAT), bought);
}

#[test]
fn a_glut_nobody_at_home_uses_is_left_where_it_is() {
    let mut s = network(2.0, 5.0, 192.0);
    s.deliver(DEST, WHEAT, 1000.0); // (the hub uses none, and its food isn't short)
    s.deliver(HUB, BREAD, 200.0);
    s.advance(2 * LOOK);
    assert_eq!(s.bought(0).0, 0.0);
}

/// The shop's listings: a town of five holding 40 emeralds and no leisure, a road to it.
// parity-partial: MoodTests.theShopListsWhatTheTownIsAfterAtWhatItWouldPay (the listing and the price the town would pay,
// and as news a wagon reads; the vendor, its lot on the shelf and its filling are Minecraft's)
#[test]
fn the_shop_lists_what_the_town_is_after_at_what_it_would_pay() {
    let mut s = network(2.0, 5.0, 192.0);
    town_of_five(&mut s, DEST);
    s.pay(DEST, 40.0);
    s.advance(LOOK);
    let listed: Vec<usize> = s.listings(DEST).iter().map(|l| l.good).collect();
    assert!(listed.contains(&PIPE), "no leisure in town, a road for it: pipe tobacco listed {listed:?}");
    let pipe = s.listings(DEST).into_iter().find(|l| l.good == PIPE).unwrap();
    assert!(pipe.lot >= 1.0 && pipe.each > 0.5, "out of stock, at a price above its worth (twice, none had): {pipe:?}");
    assert!(s.listings(DEST).len() <= desires::SHOP_SIZE);
    // A wagon calling reads the shop: the price is news the Merchant carries (`ShopBoard.sighted`).
    s.advance(2 * LOOK);
    assert_eq!(s.seen_price(DEST, PIPE), Some(pipe.each));
    // Once the leisure's in: no longer wanted.
    s.deliver(DEST, PIPE, 64.0);
    s.advance(3 * LOOK);
    assert!(s.listings(DEST).iter().all(|l| l.good != PIPE), "{:?}", s.listings(DEST));
}

#[test]
fn a_shop_lists_only_with_a_road_and_money_for_a_lot() {
    let mut s = network(2.0, 5.0, 192.0);
    let alone = s.add_town("Alone", 40.0);
    town_of_five(&mut s, alone);
    let poor = s.add_town("Poor", 0.0);
    town_of_five(&mut s, poor);
    s.add_route(poor, HUB, 4.0 / 1200.0, 0.0, 4.0);
    s.advance(LOOK);
    assert!(s.listings(alone).is_empty(), "no road for it to come by");
    assert!(s.listings(poor).is_empty(), "no money for a lot");
    // And a town with a road and money but no people keeps no shop, even for a good its lockers take.
    s.advance(2 * LOOK);
    assert!(s.listings(DEST).is_empty(), "(no people there: no shop)");
}

#[test]
fn a_merchant_buys_a_comfort_a_town_on_its_routes_is_short_of_where_it_pays() {
    // Dest has a glut of cookies past its keep; another town on the hub's routes has none: the Merchant buys what the
    // other town is short of (its 54, less what the hub holds), at the resale ceiling's bargain.
    let mut s = network(2.0, 20.0, 192.0);
    town_of_five(&mut s, DEST);
    s.deliver(DEST, COOKIE, 200.0);
    s.deliver(HUB, COOKIE, 30.0); // (its own par of treats, so it shops for none)
    let short = s.add_town("Short", 100.0);
    town_of_five(&mut s, short);
    s.add_route(HUB, short, 4.0 / 1200.0, 0.0, 4.0);
    s.advance(2 * LOOK);
    let got: enterprise_core::Fixed = s.cash_book(DEST, 3).iter().filter(|(_, what, _)| what == "backhaul").map(|(_, _, v)| v).sum();
    let value = 1.0 / 18.0;
    let held = s.stocktake(HUB).expect("counted as it left").stock[COOKIE];
    let wanted = 54.0 - held;
    assert!((28.0..=30.0).contains(&held), "(its two hands ate a cookie or two): {held}");
    // The same price for each, 0.625 of the worth (levels 1 and 1).
    assert!(got == enterprise_core::Fixed::ceil(wanted * 0.625 * value), "{got} for {wanted} cookies");
    assert_eq!(aboard(&s, 0, COOKIE), wanted);
    // And the Merchant carries the news home that it paid that: the next outlet reads it from the books.
    s.advance(3 * LOOK);
    assert!(s.books(HUB).iter().any(|e| e.kind == Kind::Bought && e.good == Some(COOKIE) && e.with == Some(DEST)),
        "{:?}", s.books(HUB));
}

/// A hub of five with nothing to sell and 20 emeralds, shopping at a far end four blocks off holding `cookies` and
/// `wheat` (a par of 64: a glut beyond 128), with wheat used at home.
fn shopper(cookies: f64, wheat: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 20.0);
    let dest = s.add_town("Dest", 12.0);
    s.deliver(hub, BREAD, 2.0);
    s.deliver(dest, BREAD, 2.0);
    if cookies > 0.0 {
        s.deliver(dest, COOKIE, cookies);
    }
    if wheat > 0.0 {
        s.deliver(dest, WHEAT, wheat);
    }
    s.set_trade(hub, WHEAT, 0.0, 10.0);
    town_of_five(&mut s, HUB);
    s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
    s
}

#[test]
fn a_shopping_trip_is_worth_what_the_far_end_would_sell_not_the_money() {
    let plan = |cookies: f64, wheat: f64| {
        let mut s = shopper(cookies, wheat);
        s.advance(LOOK);
        let note = s.route_state(0).1.last_plan.clone().expect("it planned");
        note
    };
    // A cookie there: the purse goes for its par of treats, but the trip is worth the one cookie, not the road.
    let one = plan(1.0, 0.0);
    assert!(close(one.worth, 1.0 / 18.0), "{}", one.worth);
    assert!(!one.went && one.cost > one.worth, "{one:?}");
    // Plenty there: worth its par of treats.
    let plenty = plan(64.0, 0.0);
    assert!(close(plenty.worth, PAR) && plenty.went, "{plenty:?}");
    // No treats, but a glut of wheat home uses: worth the glut's discount on what the money buys.
    let glut = plan(1.0, 1000.0);
    assert!(glut.worth > one.worth + 1.0 && glut.went, "{glut:?}");
}
