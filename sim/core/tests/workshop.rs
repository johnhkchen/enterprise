//! The workshop, pinned as the mod's jobs tests pin it (`territory/Workshop.java`): the seven conversions, whole
//! batches, a trade on the roster to run one, what is wanted, the most gain per emerald first, and demand for inputs
//! traced back two steps.

use enterprise_core::state::{Ledger, People, Stock};
use enterprise_core::workshop::{self, gain, run, wanted, FOOD_PAR_POINTS};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;

// Goods, in the mod's prices (`Prices.java`): items per emerald, so values are their inverses.
const WHEAT: usize = 0;
const CANE: usize = 1;
const PAPER: usize = 2;
const BREAD: usize = 3;
const COOKIE: usize = 4;
const BOOK: usize = 5;
const LEATHER: usize = 6;

fn content() -> Content {
    let goods = vec![
        Good::new("wheat", 1.0 / 20.0, 64),
        Good::new("sugar_cane", 1.0 / 24.0, 64),
        Good::new("paper", 1.0 / 24.0, 64),
        Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("cookie", 1.0 / 18.0, 64).comfort(0),
        Good::new("book", 1.0 / 4.0, 64).comfort(1),
        Good::new("leather", 1.0 / 6.0, 64),
    ];
    Content::new(goods, DAY)
}

fn conv(c: &Content, output: usize) -> workshop::Conversion {
    c.conversions.iter().find(|v| v.output == output).expect("a conversion").clone()
}

fn ledger(c: &Content) -> Ledger {
    let n = c.goods.len();
    Ledger { made: vec![0.0; n], used: vec![0.0; n], delivered: vec![0.0; n], ..Default::default() }
}

#[test]
fn the_mods_seven_conversions_are_content_matched_by_name() {
    let c = content();
    assert_eq!(c.conversions.len(), 3 + 1, "paper, bread, cookies and books: the goods this host has");
    let paper = conv(&c, PAPER);
    assert_eq!(paper.inputs, vec![(CANE, 1.0)]);
    assert_eq!(paper.out, 1.0);
    assert_eq!(paper.trades, vec![c.trade("librarian").unwrap(), c.trade("cartographer").unwrap()]);
    let cookie = conv(&c, COOKIE);
    assert_eq!((cookie.inputs.clone(), cookie.out), (vec![(WHEAT, 2.0), (CANE, 1.0)], 4.0));
    let book = conv(&c, BOOK);
    assert_eq!((book.inputs.clone(), book.out), (vec![(PAPER, 3.0), (LEATHER, 1.0)], 1.0));
    // A host with every good has all seven, in the mod's order.
    let mut goods = c.goods.clone();
    for name in ["white_wool", "white_carpet", "leather_boots", "cod", "cooked_cod"] {
        goods.push(Good::new(name, 0.1, 64));
    }
    let full = Content::new(goods, DAY);
    assert_eq!(full.conversions.len(), 7);
    let outputs: Vec<&str> = full.conversions.iter().map(|v| full.goods[v.output].name.as_str()).collect();
    assert_eq!(outputs, ["paper", "bread", "white_carpet", "leather_boots", "cooked_cod", "cookie", "book"]);
}

// parity: JobsTests.caneIsPressedIntoPaper
#[test]
fn cane_is_pressed_into_paper_and_wheat_baked_into_bread_in_whole_batches() {
    let c = content();
    let mut led = ledger(&c);
    let mut stock = Stock(vec![0.0; c.goods.len()]);
    stock.0[CANE] = 40.0;
    let made = run(&conv(&c, PAPER), 64.0, &mut stock, &mut led);
    assert_eq!((made, stock.0[PAPER]), (40.0, 40.0), "40 cane, 40 paper");
    stock.0[WHEAT] = 10.0;
    let bread = run(&conv(&c, BREAD), 10.0, &mut stock, &mut led);
    assert_eq!(bread, 3.0, "10 wheat bake 3 bread");
    assert_eq!(stock.0[WHEAT], 1.0, "a wheat over");
    assert_eq!((led.used[CANE], led.made[PAPER], led.used[WHEAT], led.made[BREAD]), (40.0, 40.0, 9.0, 3.0),
        "inputs used, outputs made");
}

// parity-partial: JobsTests.townsMakeTheirOwnComforts (the mod's tag check that pipe tobacco is leisure is Minecraft's own)
#[test]
fn biscuits_run_out_of_cane_first_and_a_cane_earns_more_in_them_than_in_paper() {
    let c = content();
    let mut led = ledger(&c);
    let mut stock = Stock(vec![0.0; c.goods.len()]);
    stock.0[WHEAT] = 10.0;
    stock.0[CANE] = 2.0;
    let cookies = run(&conv(&c, COOKIE), 64.0, &mut stock, &mut led);
    assert_eq!(cookies, 8.0, "10 wheat and 2 cane: two batches of biscuits, the cane runs out first");
    assert_eq!(stock.0[WHEAT], 6.0, "4 wheat used, 6 left");
    let biscuit = gain(&c, &conv(&c, COOKIE), c.goods[COOKIE].value);
    let paper = gain(&c, &conv(&c, PAPER), c.goods[PAPER].value);
    assert!((paper - 1.0).abs() < 1e-9, "paper is worth what its cane is: {paper}");
    assert!(biscuit > 1.4 * paper, "biscuits {biscuit} an emerald of inputs, paper {paper}");
}

#[test]
fn a_comfort_is_wanted_up_to_the_par_less_what_is_held_and_again_for_each_route_out() {
    let c = content();
    let people = People { villagers: 10.0, ..Default::default() };
    let mut stock = Stock(vec![0.0; c.goods.len()]);
    // par = 10 villagers x 0.1 ration x 6 days = 6 emeralds of each kind; a cookie is 1/18 of one.
    assert_eq!(wanted(&c, COOKIE, &people, &stock, 0, 0.0), 108.0);
    assert_eq!(wanted(&c, COOKIE, &people, &stock, 2, 0.0), 324.0, "as much again for each of 2 routes out");
    stock.0[COOKIE] = 54.0;
    assert_eq!(wanted(&c, COOKIE, &people, &stock, 0, 0.0), 54.0, "less the three emeralds held");
    stock.0[COOKIE] = 200.0;
    assert_eq!(wanted(&c, COOKIE, &people, &stock, 1, 0.0), 0.0, "held past the par: none wanted");
}

#[test]
fn bread_is_wanted_while_food_is_under_twice_the_food_par_and_paper_up_to_a_lockers_par() {
    let c = content();
    let people = People { villagers: 10.0, ..Default::default() };
    let mut stock = Stock(vec![0.0; c.goods.len()]);
    assert_eq!(FOOD_PAR_POINTS, 320.0);
    assert_eq!(wanted(&c, BREAD, &people, &stock, 3, 0.0), 128.0, "(2 x 320 - 0) / 5, routes don't matter");
    stock.0[BREAD] = 64.0;
    assert_eq!(wanted(&c, BREAD, &people, &stock, 0, 0.0), 64.0, "320 points held, 320 short");
    stock.0[BREAD] = 130.0;
    assert_eq!(wanted(&c, BREAD, &people, &stock, 0, 0.0), 0.0, "past twice the par");
    assert_eq!(wanted(&c, PAPER, &people, &stock, 0, 0.0), 0.0, "no locker buys paper: none wanted");
    stock.0[PAPER] = 10.0;
    assert_eq!(wanted(&c, PAPER, &people, &stock, 0, 64.0), 54.0, "a locker's par of 64, 10 held");
}

/// A town of `hands` with wheat, cane and the like in its warehouse.
fn town(hands: &[(&str, u8)], stock: &[(usize, f64)]) -> Society {
    let mut s = Society::new(content());
    let t = s.add_town("Meadowworth", 50.0);
    s.set_people(t, hands.len() as f64, hands.len() as f64, 20.0);
    if !hands.is_empty() {
        s.set_roster(t, hands);
    }
    for &(g, n) in stock {
        s.deliver(t, g, n);
    }
    s
}

/// What the workshop made of a good (the ledger's; people use some of it after).
fn made(s: &Society, good: usize) -> f64 {
    s.world.resource::<Ledger>().made[good]
}

#[test]
fn nothing_runs_without_a_trade_on_the_roster() {
    let mut s = town(&[("mason", 1), ("merchant", 1)], &[(WHEAT, 10.0), (CANE, 40.0)]);
    s.set_locker_par(0, PAPER, 64.0);
    s.advance(DAY);
    assert_eq!((made(&s, BREAD), made(&s, PAPER), made(&s, COOKIE)), (0.0, 0.0, 0.0), "no farmer, no librarian: nothing made");
    let mut s = town(&[("farmer", 1), ("librarian", 1)], &[(WHEAT, 10.0), (CANE, 40.0)]);
    s.set_locker_par(0, PAPER, 64.0);
    s.advance(DAY);
    assert!(made(&s, PAPER) > 0.0 && made(&s, COOKIE) > 0.0, "with them, it runs");
}

#[test]
fn a_touch_bakes_the_wheat_the_town_is_short_of_bread_for() {
    let mut s = town(&[("farmer", 1)], &[(WHEAT, 10.0)]);
    s.advance(DAY);
    let st = s.town(0).stock;
    assert_eq!(made(&s, BREAD), 3.0, "10 wheat, 3 bread");
    assert_eq!(st[WHEAT], 1.0, "and 1 left");
    // Biscuits come too, while the town is short of treats: they take the wheat first (more gain per emerald).
    let mut s = town(&[("farmer", 1)], &[(WHEAT, 10.0), (CANE, 2.0)]);
    s.advance(DAY);
    assert_eq!((made(&s, COOKIE), s.town(0).stock[CANE]), (8.0, 0.0), "10 wheat + 2 cane: 8 biscuits, the cane gone");
    assert_eq!(made(&s, BREAD), 2.0, "the 6 wheat left bake 2 bread");
}

#[test]
fn the_cane_goes_to_the_biscuits_before_the_paper() {
    let mut s = town(&[("farmer", 1), ("librarian", 1)], &[(WHEAT, 10.0), (CANE, 2.0)]);
    s.set_locker_par(0, PAPER, 64.0);
    s.advance(DAY);
    assert_eq!((made(&s, COOKIE), made(&s, PAPER), s.town(0).stock[CANE]), (8.0, 0.0, 0.0),
        "biscuits first, no cane left for paper");
}

#[test]
fn wheat_sold_forward_is_left_alone() {
    let mut s = town(&[("farmer", 1)], &[(WHEAT, 10.0)]);
    s.content_mut().dials.meal_points = 0.0; // no meals: the stock as put
    s.set_aside(0, WHEAT, 6.0);
    s.advance(DAY);
    let st = s.town(0).stock;
    assert_eq!((st[BREAD], st[WHEAT]), (1.0, 7.0), "4 free wheat: one batch");
}

#[test]
fn a_towns_runs_out_make_it_want_more_of_its_comforts() {
    let mut s = town(&[("farmer", 1), ("librarian", 1)], &[(WHEAT, 5000.0), (CANE, 5000.0)]);
    s.advance(DAY);
    let alone = made(&s, COOKIE);
    let mut s = town(&[("farmer", 1), ("librarian", 1), ("merchant", 1)], &[(WHEAT, 5000.0), (CANE, 5000.0)]);
    s.add_town("Far", 10.0);
    s.add_route(0, 1, 1.0, 0.0, 2.0);
    s.advance(DAY);
    assert!(made(&s, COOKIE) > alone, "a route out doubles the biscuits wanted: {} against {alone}", made(&s, COOKIE));
}

#[test]
fn the_demand_for_inputs_runs_back_two_steps_books_to_paper_to_cane() {
    let mut s = town(&[("librarian", 1); 10], &[]);
    let t = 0;
    s.add_town("Far", 10.0);
    s.add_route(t, 1, 1.0, 0.0, 2.0);
    let d = s.input_demand(t);
    let of = |g: usize| d.iter().find(|d| d.good == g).copied();
    // 10 villagers x 0.1 ration x 2 towns = 2 emeralds a day of books, at 4 an emerald: 8 books, as many batches.
    let (paper, leather, cane) = (of(PAPER).unwrap(), of(LEATHER).unwrap(), of(CANE).unwrap());
    assert!((paper.per_day - 24.0).abs() < 1e-9 && (leather.per_day - 8.0).abs() < 1e-9, "{d:?}");
    // A book's 1/4 emerald shared by its inputs' worth: 3 paper at 1/24 and a leather at 1/6.
    let worth = 3.0 / 24.0 + 1.0 / 6.0;
    assert!((paper.each - 0.25 / 24.0 / worth).abs() < 1e-9);
    assert!((leather.each - 0.25 / 6.0 / worth).abs() < 1e-9);
    assert!((cane.per_day - 24.0).abs() < 1e-9, "a cane for each paper: {}", cane.per_day);
    assert!((cane.each - paper.each).abs() < 1e-9, "a pressed cane earns what the paper does");
    assert_eq!(of(WHEAT), None, "no farmer: no wheat drawn");
    assert_eq!(of(BOOK), None, "an output isn't an input");
    // Add a farmer: biscuits draw wheat, and cane at the higher worth they earn there.
    s.add_hand(t, "farmer", 1);
    let d = s.input_demand(t);
    let cane2 = d.iter().find(|d| d.good == CANE).unwrap();
    assert!(cane2.per_day > cane.per_day && cane2.each > cane.each, "biscuits draw cane at a higher worth: {cane2:?}");
    assert!(d.iter().any(|d| d.good == WHEAT));
}

#[test]
fn the_workshop_state_is_saved() {
    let mut s = town(&[("farmer", 1)], &[(WHEAT, 10.0)]);
    s.set_aside(0, WHEAT, 6.0);
    s.set_locker_par(0, PAPER, 64.0);
    let bytes = s.save();
    let mut loaded = Society::load(&bytes).unwrap();
    loaded.advance(DAY);
    s.advance(DAY);
    assert_eq!(loaded.town(0).stock, s.town(0).stock);
    assert_eq!(loaded.content().conversions.len(), s.content().conversions.len());
}

#[test]
fn a_workshop_keeps_back_from_the_lockers_what_its_comforts_short_need() {
    // A town of four short of reading (none met), its Librarian binding books of 3 paper and 1 leather: it keeps back
    // 3 paper for each leather it holds, as far as its par of books goes; with no leather, no paper.
    let c = content();
    let roster = town(&[("librarian", 1), ("farmer", 1), ("farmer", 1), ("farmer", 1)], &[]).roster(0);
    let people = People { villagers: 4.0, comfort_met: vec![0.0; c.comforts.len()], ..Default::default() };
    let stock = |leather: f64| {
        let mut s = Stock(vec![0.0; c.goods.len()]);
        s.0[PAPER] = 100.0;
        s.0[LEATHER] = leather;
        s
    };
    // Its par: 4 x 0.1 x 6 days, 2.4 emeralds of reading, 10 books at a quarter each.
    assert_eq!(workshop::keeps(&c, &roster, &people, &stock(2.0))[PAPER], 6.0, "two leather: two books' paper");
    assert_eq!(workshop::keeps(&c, &roster, &people, &stock(50.0))[PAPER], 30.0, "its par of ten books' paper");
    assert_eq!(workshop::keeps(&c, &roster, &people, &stock(0.0))[PAPER], 0.0, "no leather: no paper kept");
    // Reading met: nothing kept.
    let mut met = people.clone();
    met.comfort_met = vec![1.0; c.comforts.len()];
    assert_eq!(workshop::keeps(&c, &roster, &met, &stock(2.0))[PAPER], 0.0);
}

#[test]
fn a_short_comforts_inputs_are_worth_what_a_short_town_pays_for_it() {
    // A Librarian's books (reading): with none of it met, its paper and leather earn twice their share of a book's
    // worth (`desire_most`, as `desires::will_pay`); met, its worth.
    let c = content();
    let roster = town(&[("librarian", 1)], &[]).roster(0);
    let each = |met: f64| workshop::input_demand(&c, &roster, 4.0, 0, &[met; 2]).into_iter()
        .find(|d| d.good == PAPER).expect("paper drawn").each;
    assert!((each(0.0) - 2.0 * each(1.0)).abs() < 1e-12, "short: {}, met: {}", each(0.0), each(1.0));
    assert!((each(0.5) - 1.5 * each(1.0)).abs() < 1e-12, "half met: half again");
}

/// A town makes what the far end of a route out takes at its lockers and lacks, as its wagon's plan sees it: cane and a
/// Cartographer, and a Librarian down the road who buys paper, make paper; with nobody there buying it, none (a
/// survival world's town sat on 453 cane for 30 days while the town down its road bought paper).
#[test]
fn a_town_makes_what_the_town_down_its_road_buys() {
    let road = |buys: bool| {
        let mut s = town(&[("cartographer", 1)], &[(CANE, 200.0)]);
        let far = s.add_town("Farfield", 50.0);
        s.set_people(far, 1.0, 1.0, 20.0);
        s.set_roster(far, &[("librarian", 1)]);
        if buys {
            s.record_capacity(far, 1, &[(PAPER, 64.0 / 5.0)]); // its locker takes a stack of paper
        }
        s.add_route(0, far, 1.0, 0.0, 4.0);
        s.advance(DAY);
        s
    };
    assert_eq!(made(&road(false), PAPER), 0.0, "nobody down the road buys paper: none made");
    let s = road(true);
    assert!(made(&s, PAPER) >= 64.0, "a stack for the Librarian down the road: {}", made(&s, PAPER));
}

/// A wagon takes the far end a raw good its own trades turn into what its lockers buy: cane, from a town with nobody to
/// press it, for the Cartographer down the road whose locker takes paper; there it's made into paper (a survival
/// world's leatherworkers' town held 406 cane, and its wagon's best load was worth 4.5 against an 8-emerald road).
#[test]
fn a_wagon_takes_cane_to_the_cartographer_down_the_road() {
    let road = |cartographer: bool| {
        let mut s = town(&[], &[(CANE, 640.0)]); // (no roster: no squad to muster, a wagon of its own)
        let far = s.add_town("Farfield", 50.0);
        s.set_people(far, 2.0, 2.0, 20.0);
        s.set_roster(far, &[("merchant", 1), (if cartographer { "cartographer" } else { "mason" }, 1)]);
        s.record_capacity(far, 1, &[(PAPER, 64.0 * 4.0 / 5.0)]); // its locker takes paper: a par of 4 stacks
        s.set_locker_par(far, PAPER, 256.0);
        s.add_route(0, far, 0.25, 0.0, 1.0);
        s.advance(3 * DAY);
        s
    };
    let s = road(true);
    assert!(s.town(0).stock[CANE] < 640.0, "the cane went down the road: {} left", s.town(0).stock[CANE]);
    assert!(made(&s, PAPER) > 0.0, "and was made into paper there");
    let s = road(false);
    assert_eq!(s.town(0).stock[CANE], 640.0, "nobody there to press it: the cane stays");
}

/// A Butcher cooks the town's beef over a campfire while its food is short (no coal), as far as twice the food par.
#[test]
fn a_butcher_cooks_the_towns_beef_while_its_food_is_short() {
    let mut goods = content().goods;
    goods.push(Good::new("beef", 0.125, 64));
    goods.push(Good::new("cooked_beef", 0.2, 64).food(8.0));
    let c = Content::new(goods, DAY);
    let (beef, cooked) = (c.good("beef").unwrap(), c.good("cooked_beef").unwrap());
    let mut s = Society::new(c);
    s.content_mut().dials.meal_points = 0.0; // no meals: the stock as put
    let t = s.add_town("Meadowworth", 50.0);
    s.set_people(t, 2.0, 2.0, 20.0);
    s.set_roster(t, &[("butcher", 1), ("merchant", 1)]);
    s.deliver(t, beef, 300.0);
    s.advance(DAY);
    assert_eq!(made(&s, cooked), (2.0 * FOOD_PAR_POINTS / 8.0).floor(), "up to twice the food par, in 8-point steaks");
    assert_eq!(s.town(t).stock[beef], 300.0 - made(&s, cooked), "a beef a steak");
}
