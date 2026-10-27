//! The herds, pinned as the mod's husbandry tests pin them (`territory/Husbandry.java`): what each trade brings,
//! feed eaten whole however often a herd is tended, growth to the pasture, hunger, shearing and culling.

use enterprise_core::herds::{feed_demand, seed, tend, Herds, Keepers};
use enterprise_core::state::{Ledger, Stock};
use enterprise_core::workshop::run;
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;

const WHEAT: usize = 0;
const CARROT: usize = 1;
const BEEF: usize = 3;
const PORK: usize = 4;
const WOOL: usize = 6;
const LEATHER: usize = 7;
const BOOTS: usize = 8;

fn content() -> Content {
    let goods = vec![
        Good::new("wheat", 1.0 / 20.0, 64),
        Good::new("carrot", 1.0 / 20.0, 64),
        Good::new("mutton", 1.0 / 5.0, 64),
        Good::new("beef", 1.0 / 5.0, 64),
        Good::new("porkchop", 1.0 / 5.0, 64),
        Good::new("chicken", 1.0 / 5.0, 64),
        Good::new("white_wool", 1.0 / 10.0, 64),
        Good::new("leather", 1.0 / 6.0, 64),
        Good::new("leather_boots", 1.0 / 2.0, 64),
    ];
    Content::new(goods, DAY)
}

fn ledger(c: &Content) -> Ledger {
    let n = c.goods.len();
    Ledger { made: vec![0.0; n], used: vec![0.0; n], delivered: vec![0.0; n], ..Default::default() }
}

fn stocked(c: &Content, good: usize, n: f64) -> Stock {
    let mut s = Stock(vec![0.0; c.goods.len()]);
    s.0[good] = n;
    s
}

fn herd_of(c: &Content, kind: &str, n: f64) -> Herds {
    let mut h = Herds::default();
    let k = c.animals.iter().position(|a| a.name == kind).unwrap();
    h.count = vec![0.0; c.animals.len()];
    h.owed = vec![0.0; c.animals.len()];
    h.seeded = vec![false; c.animals.len()];
    h.count[k] = n;
    h
}

fn count(c: &Content, h: &Herds, kind: &str) -> f64 {
    h.count_of(c.animals.iter().position(|a| a.name == kind).unwrap())
}

const SHEPHERD: Keepers = Keepers { shepherd: true, butcher: false, leatherworker: false };
const BUTCHER: Keepers = Keepers { shepherd: false, butcher: true, leatherworker: false };
const LEATHERWORKER: Keepers = Keepers { shepherd: false, butcher: false, leatherworker: true };
const NONE: Keepers = Keepers { shepherd: false, butcher: false, leatherworker: false };

#[test]
fn the_mods_four_kinds_are_content_matched_by_name() {
    let c = content();
    let names: Vec<&str> = c.animals.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["sheep", "cow", "pig", "chicken"]);
    let pig = &c.animals[2];
    assert_eq!(pig.feed, vec![CARROT], "the pig's other feed goods (potato, beetroot) this host lacks");
    assert_eq!((pig.feed_per_day, pig.meat_each), (0.25, 2.0));
    // A host without wheat has no sheep, cows or chickens.
    let c = Content::new(vec![Good::new("carrot", 0.05, 64), Good::new("porkchop", 0.2, 64)], DAY);
    assert_eq!(c.animals.len(), 1);
}

// parity: HusbandryTests.shepherdsAndButchersBringAnimals
#[test]
fn shepherds_and_butchers_bring_animals() {
    let c = content();
    let mut h = Herds::default();
    seed(&c, &mut h, SHEPHERD);
    assert_eq!(count(&c, &h, "sheep"), 3.0, "a Shepherd brings 3 sheep");
    seed(&c, &mut h, BUTCHER);
    assert_eq!((count(&c, &h, "pig"), count(&c, &h, "chicken")), (2.0, 2.0), "a Butcher, pigs and chickens");
    let flock = herd_of(&c, "sheep", 4.0);
    assert!((feed_demand(&c, &flock)[WHEAT] - 1.0).abs() < 1e-9, "4 sheep eat a wheat a day: the village asks for it");
}

#[test]
fn a_kind_is_seeded_once_and_a_starved_flock_is_not_given_back() {
    let c = content();
    let mut h = Herds::default();
    seed(&c, &mut h, SHEPHERD);
    let sheep = 0;
    h.count[sheep] = 0.0;
    seed(&c, &mut h, SHEPHERD);
    assert_eq!(h.count[sheep], 0.0, "not given back");
    seed(&c, &mut h, NONE);
    assert_eq!(count(&c, &h, "pig"), 0.0, "no trade, no animals");
}

// parity: HusbandryTests.leatherworkersKeepCowsForHides
#[test]
fn leatherworkers_keep_cows_for_hides() {
    let c = content();
    let mut h = Herds::default();
    seed(&c, &mut h, LEATHERWORKER);
    assert_eq!(count(&c, &h, "cow"), 2.0, "a Leatherworker brings 2 cows");
    let mut led = ledger(&c);
    let mut stock = stocked(&c, WHEAT, 64.0);
    let mut h = herd_of(&c, "cow", 16.0); // a full pasture: culled down toward three quarters of it
    tend(&c, &mut h, LEATHERWORKER, 2.0, &mut stock, &mut led);
    assert!(stock.0[LEATHER] > 2.0 && stock.0[BEEF] > 4.0, "culled for leather, and beef: {:?}", stock.0);
    // 9 leather make 2 pairs of boots (clothing).
    let boots = c.conversions.iter().find(|v| v.output == BOOTS).expect("boots").clone();
    stock.0[LEATHER] = 9.0;
    assert_eq!(run(&boots, 2.0, &mut stock, &mut led), 2.0, "9 leather make 2 pairs of boots");
}

// parity: HusbandryTests.aFedFlockGrowsAndIsShorn
#[test]
fn a_fed_flock_grows_and_is_shorn() {
    let c = content();
    let mut led = ledger(&c);
    let mut stock = stocked(&c, WHEAT, 64.0);
    let mut h = herd_of(&c, "sheep", 3.0);
    tend(&c, &mut h, SHEPHERD, 4.0, &mut stock, &mut led);
    assert!(count(&c, &h, "sheep") > 4.0, "fed, the flock grows: {h:?}");
    assert!(stock.0[WOOL] > 5.0, "and gives wool: {}", stock.0[WOOL]);
    assert_eq!(stock.0[WHEAT], 61.0, "3 wheat eaten");
    assert_eq!(led.used[WHEAT], 3.0);
    assert_eq!(led.made[WOOL], stock.0[WOOL]);
}

// parity: HusbandryTests.aHungryFlockDwindles
#[test]
fn a_hungry_flock_dwindles() {
    let c = content();
    let mut led = ledger(&c);
    let mut stock = Stock(vec![0.0; c.goods.len()]);
    let mut h = herd_of(&c, "sheep", 6.0);
    tend(&c, &mut h, NONE, 4.0, &mut stock, &mut led);
    assert!(count(&c, &h, "sheep") < 6.0, "no wheat: fewer sheep, {h:?}");
    assert_eq!(h.owed[0], 1.0, "what it couldn't eat isn't eaten back: owed is capped at one");
}

// parity: HusbandryTests.butchersSlaughterWhatTheFullPastureBreeds
#[test]
fn butchers_slaughter_what_the_full_pasture_breeds() {
    let c = content();
    let mut led = ledger(&c);
    let mut stock = stocked(&c, CARROT, 64.0);
    let mut h = herd_of(&c, "pig", 16.0);
    tend(&c, &mut h, BUTCHER, 2.0, &mut stock, &mut led);
    let pigs = count(&c, &h, "pig");
    assert!((12.0..16.0).contains(&pigs), "a full pasture culled toward three quarters of it, 12: {pigs}");
    assert!(stock.0[PORK] > 4.0, "porkchops: {}", stock.0[PORK]);
    assert_eq!(stock.0[CARROT], 56.0, "8 carrots eaten");
}

// parity: HusbandryTests.aCulledHerdGrowsToItsPastureFirst
#[test]
fn a_culled_herd_grows_to_its_pasture_first() {
    let c = content();
    let mut led = ledger(&c);
    let mut stock = stocked(&c, CARROT, 128.0);
    let mut h = herd_of(&c, "pig", 4.0);
    for _ in 0..80 {
        // 20 days, tended four times a day
        tend(&c, &mut h, BUTCHER, 0.25, &mut stock, &mut led);
    }
    assert!(count(&c, &h, "pig") > 11.0, "4 pigs breed up to near 12 before the Butcher culls: {h:?}");
    assert!(stock.0[PORK] > 5.0, "and then culls what they breed: {}", stock.0[PORK]);
}

// parity: HusbandryTests.aSecondPastureHoldsTwiceTheHerd
#[test]
fn a_second_pasture_holds_twice_the_herd() {
    let c = content();
    let mut led = ledger(&c);
    let mut stock = stocked(&c, WHEAT, 64.0);
    let mut h = herd_of(&c, "sheep", 16.0);
    h.pastures = 2;
    tend(&c, &mut h, SHEPHERD, 3.0, &mut stock, &mut led);
    assert!(count(&c, &h, "sheep") > 20.0, "a flock with two pastures outgrows one: {h:?}");
    let mut one = herd_of(&c, "sheep", 16.0);
    tend(&c, &mut one, SHEPHERD, 3.0, &mut stocked(&c, WHEAT, 64.0), &mut led);
    assert_eq!(count(&c, &one, "sheep"), 16.0, "and one pasture is full");
}

// parity: HusbandryTests.aHerdTendedOftenEatsOnlyItsShare
#[test]
fn a_herd_tended_often_eats_only_its_share() {
    let c = content();
    let mut led = ledger(&c);
    let mut stock = stocked(&c, WHEAT, 64.0);
    let mut h = herd_of(&c, "sheep", 4.0);
    for _ in 0..100 {
        // a day, tended a hundred times
        tend(&c, &mut h, NONE, 0.01, &mut stock, &mut led);
    }
    let eaten = 64.0 - stock.0[WHEAT];
    assert!((1.0..=2.0).contains(&eaten), "4 sheep eat a wheat a day however often tended, ate {eaten}");
    assert!(count(&c, &h, "sheep") > 4.4, "and, fed, grow: {h:?}");
}

#[test]
fn a_pig_eats_any_of_its_feed_goods_in_order() {
    let mut goods = content().goods;
    goods.push(Good::new("potato", 0.05, 64));
    let c = Content::new(goods, DAY);
    assert_eq!(c.animals[2].feed, vec![CARROT, 9]);
    let mut led = ledger(&c);
    let mut stock = stocked(&c, CARROT, 1.0);
    stock.0[9] = 5.0;
    let mut h = herd_of(&c, "pig", 8.0);
    tend(&c, &mut h, NONE, 2.0, &mut stock, &mut led);
    assert_eq!((stock.0[CARROT], stock.0[9]), (0.0, 2.0), "4 pig-days of feed: the carrot first, then potatoes");
}

#[test]
fn a_town_with_the_trades_keeps_herds_that_follow_the_clock() {
    let mut s = Society::new(content());
    let t = s.add_town("Alder", 100.0);
    s.set_people(t, 3.0, 3.0, 10.0);
    s.set_roster(t, &[("shepherd", 1), ("butcher", 1), ("farmer", 1)]);
    s.advance(1);
    let h = s.herds(t);
    assert_eq!(h.count_of(0), 3.0, "the sheep came");
    assert_eq!(h.count_of(2), 2.0, "and the pigs");
    s.set_pastures(t, 1);
    assert!(s.set_herd(t, "chicken", 2.0));
    assert_eq!(s.feed_demand(t)[WHEAT], 3.0 * 0.25 + 2.0 * 0.1, "sheep and chickens eat wheat");
    let before = s.herds(t);
    s.advance(10 * DAY);
    let after = s.herds(t);
    assert_ne!(before, after, "ten days on, tended");
    assert_eq!(after.last, 10 * DAY, "tended up to the clock");
    let loaded = Society::load(&s.save()).unwrap();
    assert_eq!(loaded.herds(t), after, "the herds are saved");
}

#[test]
fn a_town_with_a_farm_owes_the_products_with_its_harvest_and_asks_it_for_feed() {
    let mut s = Society::new(content());
    let t = s.add_town("Alder", 100.0);
    s.set_people(t, 3.0, 3.0, 10.0);
    s.set_roster(t, &[("shepherd", 1), ("farmer", 1)]);
    s.set_plots(t, WHEAT, 0);
    s.deliver(t, WHEAT, 64.0);
    s.advance(1);
    assert_eq!(s.herds(t).count_of(0), 3.0, "the sheep came");
    s.advance(4 * DAY);
    let farm = s.farm(t).expect("a farm").farm;
    let feed = s.feed_demand(t)[WHEAT];
    assert!(feed > 0.75, "a growing flock: {feed}");
    assert_eq!(farm.feed.get(&WHEAT).copied(), Some(feed), "the herds' feed is the farm's to grow");
    let wool = s.town(t).stock[WOOL];
    assert!(wool >= 5.0 && wool == wool.floor(), "the wool handed over whole, with the harvest: {wool}");
    let owed = farm.pending.get(&WOOL).copied().unwrap_or(0.0);
    assert!((0.0..1.0).contains(&owed), "and the fraction still owed: {owed}");
}

#[test]
fn a_leatherworker_culls_cows_for_beef_where_the_host_has_no_leather() {
    let c = Content::new(vec![Good::new("wheat", 0.05, 64), Good::new("beef", 0.2, 64)], DAY);
    let mut led = Ledger { made: vec![0.0; 2], used: vec![0.0; 2], delivered: vec![0.0; 2], ..Default::default() };
    let mut stock = Stock(vec![64.0, 0.0]);
    let mut h = herd_of(&c, "cow", 16.0);
    tend(&c, &mut h, LEATHERWORKER, 2.0, &mut stock, &mut led);
    assert!(stock.0[1] > 4.0, "culled for beef, as the mod's `kind == cow`: {:?}", stock.0);
}

/// A town with a farm fences a pasture for its herd, and another as the herd fills those (`Territory.keepHerds`), to
/// `pastures_max`, four emeralds each: a survival network's cows stopped at one pasture's sixteen in every town.
#[test]
fn a_town_fences_another_pasture_when_its_herd_fills_the_ones_it_has() {
    let mut s = Society::new(content());
    let t = s.add_town("Alder", 100.0);
    s.set_people(t, 3.0, 3.0, 10.0);
    s.set_roster(t, &[("shepherd", 1), ("farmer", 1)]);
    s.set_plots(t, WHEAT, 0);
    s.deliver(t, WHEAT, 640.0);
    let before = s.totals();
    s.advance(DAY + 1);
    assert_eq!(s.herds(t).pastures, 1, "a pasture for the flock");
    for day in 2..=40 {
        s.deliver(t, WHEAT, 16.0);
        s.advance(day * DAY + 1);
    }
    let h = s.herds(t);
    assert_eq!(h.pastures, s.content().dials.pastures_max, "fenced to the most: {h:?}");
    assert!(h.count_of(0) > 32.0, "a flock beyond two pastures' room: {h:?}");
    let paid: f64 = s.cash_book(t, 400).iter().filter(|(_, w, _)| w == "a pasture").map(|(_, _, e)| e.to_f64()).sum();
    assert_eq!(paid, -12.0, "four emeralds a pasture");
    s.conserved(&before).unwrap();
}
