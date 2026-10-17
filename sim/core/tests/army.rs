//! The army at the front (`army.rs`, `rules/army.rs`), pinned as the mod's war and logistics tests pin it: an army
//! eats from the front's warehouse, the heartiest food first, and keeps rations for the road; kit cuts the risk of
//! falling, whole sets most; the fallen are gone from their towns, which grieve; the hungry desert. Losses are the
//! expected values carried, so they're pinned to the soldier and the day. Goods are the mod's, by name.

use enterprise_core::army::{self, Army};
use enterprise_core::state::{Ledger, People, Stock};
use enterprise_core::{Content, Good, Hex, Incoterm, Society};

const DAY: u64 = 24_000;
const LOOK: u64 = 1_200;

const FOODS: [&str; 4] = ["bread", "cooked_beef", "cooked_chicken", "golden_carrot"];

/// The mod's foods (with their saturation), its weapons, armour and shield, and arrows.
fn content() -> Content {
    let mut goods: Vec<Good> = FOODS.iter().map(|n| army::food_good(n, 64)).collect();
    for g in army::WEAPONS.iter().chain(army::ARMOUR.iter().flatten()).chain([&army::SHIELD]) {
        goods.push(Good::priced(g.name, 1));
    }
    goods.push(Good::priced("arrow", 64));
    Content::new(goods, DAY)
}

fn good(c: &Content, name: &str) -> usize {
    c.good(name).unwrap_or_else(|| panic!("no good called {name}"))
}

fn stock(s: &Society, t: usize, name: &str) -> f64 {
    s.town(t).stock[good(s.content(), name)]
}

/// A town with its Merchant and `n` idle hands, at the map's corner; and the front, 1,500 blocks north of it, off the
/// map.
fn war(s: &mut Society, n: usize) -> (usize, usize) {
    let home = s.add_town("Home", 0.0);
    let mut hands = vec![("merchant", 1)];
    hands.extend(std::iter::repeat_n(("none", 1), n));
    s.set_roster(home, &hands);
    let hex = Hex::new(0, 0);
    s.place_town(home, hex.center_x(), hex.center_z());
    let front = s.add_off_map("Front", hex.center_x(), hex.center_z() - 1500.0, 0.0);
    (home, front)
}

fn deliver(s: &mut Society, t: usize, name: &str, n: f64) {
    let g = good(s.content(), name);
    s.deliver(t, g, n);
}

// parity: WarTests.anArmyEatsFromTheDepot
#[test]
fn an_army_eats_from_the_depot() {
    let mut s = Society::new(content());
    let (home, front) = war(&mut s, 5);
    deliver(&mut s, front, "bread", 64.0);
    assert_eq!(s.enlist(home, 5), 5, "five soldiers join");
    s.set_front(front);
    s.advance(LOOK);
    assert_eq!((s.army().eaten, stock(&s, front, "bread")), (0.0, 64.0), "the first campaign day arms the clock");
    s.advance(LOOK + DAY);
    assert!(s.army().eaten >= 5.0 * s.content().dials.army_ration, "five soldiers ate a day's rations: {}", s.army().eaten);
    assert_eq!(s.army().eaten, 40.0);
    assert_eq!(stock(&s, front, "bread"), 64.0 - 8.0, "eight loaves");
}

// parity: WarTests.theFrontKeepsRationsForTheRoad
#[test]
fn the_front_keeps_rations_for_the_road() {
    let c = content();
    let (near, far) = (army::ration_days(&c, 0.0), army::ration_days(&c, 1500.0));
    assert_eq!((near, far), (5, 11), "three round trips and two days spare");
    assert!(far > near && far >= 6, "a far front keeps more days of food: {near} vs {far}");
    let mut s = Society::new(content());
    let (home, _) = war(&mut s, 2);
    assert_eq!(s.muster(home, 2), 2);
    assert_eq!((s.army().waiting(), s.army().size()), (2, 0), "two recruits wait at the capital");
    let wants = s.army().wants(s.content(), far);
    let bread = wants.iter().find(|(g, _)| *g == good(s.content(), "bread")).expect("bread wanted").1;
    assert_eq!(bread, 36, "176 food points, in loaves");
    assert!(bread as f64 * 5.0 >= 2.0 * s.content().dials.army_ration * far as f64,
        "rations for the recruits on their way too: {bread}");
}

// parity: WarTests.theArmyEatsItsHeartiestFoodFirst
#[test]
fn the_army_eats_its_heartiest_food_first() {
    let c = content();
    let n = c.goods.len();
    let mut ledger = Ledger { made: vec![0.0; n], used: vec![0.0; n], delivered: vec![0.0; n], ..Default::default() };
    let mut depot = Stock(vec![0.0; n]);
    depot.0[good(&c, "bread")] = 16.0;
    depot.0[good(&c, "cooked_beef")] = 1.0;
    let meal = army::mess(&c, &mut ledger, &mut depot, 8.0);
    assert_eq!(meal.eaten, 8.0, "a steak before bread");
    assert_eq!((depot.0[good(&c, "cooked_beef")], depot.0[good(&c, "bread")]), (0.0, 16.0), "and all the bread left");
    assert!(meal.hearty / meal.eaten > 0.0, "a hearty meal");
    assert!((meal.hearty / meal.eaten - 1.0 / 3.0).abs() < 1e-9, "a steak is a third above bread a point");
    assert_eq!(ledger.used[good(&c, "cooked_beef")], 1.0, "eaten: gone from the world's goods");
    let heartiness = |name: &str| army::heartiness(&c.goods[good(&c, name)]);
    assert!(heartiness("cooked_chicken") > 0.0 && heartiness("bread") == 0.0, "any meat is hearty, bread isn't");
    assert!((heartiness("cooked_chicken") - army::MEAT_HEARTINESS).abs() < 1e-12, "chicken as hearty as a pork chop");
    assert_eq!(army::heartiness_of(1.2), 0.0, "bread is plain fare");
    assert_eq!(army::heartiness_of(6.0f32 as f64 / 5.0), 0.0, "and a float's rounding is no feast");
    assert_eq!(heartiness("golden_carrot"), 1.0, "golden carrots are the heartiest");
}

#[test]
fn the_heartiest_go_first_then_by_saturation_bread_last() {
    let c = content();
    let mut depot = Stock(vec![0.0; c.goods.len()]);
    for name in FOODS {
        depot.0[good(&c, name)] = 3.0;
    }
    let order: Vec<&str> = army::heartiest_first(&c, &depot).into_iter().map(|g| c.goods[g].name.as_str()).collect();
    assert_eq!(order, ["golden_carrot", "cooked_beef", "cooked_chicken", "bread"],
        "a golden carrot, then the meats (beef's saturation the higher), bread last");
}

// parity: WarTests.soldiersFallLessInBetterKit
#[test]
fn soldiers_fall_less_in_better_kit() {
    let d = content().dials;
    let bare = army::risk(&d, None, &[], false);
    let iron = army::risk(&d, Some(("iron_sword", 0.0)), &[("iron_chestplate", 0.0)], true);
    let diamond = army::risk(&d, Some(("diamond_sword", 0.0)), &[("diamond_chestplate", 0.0)], true);
    let sharpened = army::risk(&d, Some(("diamond_sword", 5.0)), &[("diamond_chestplate", 0.0)], true);
    assert!(bare > iron && iron > diamond && diamond > sharpened,
        "bare {bare:.4} > iron {iron:.4} > diamond {diamond:.4} > sharpened {sharpened:.4}");
    // 0.02 x 2 x 1.6; 0.02 x 1.0 x 1.27 x 0.85; 0.02 x 0.8 x 1.16 x 0.85; and Sharpness V takes a quarter off.
    for (got, want) in [(bare, 0.064), (iron, 0.02159), (diamond, 0.015776), (sharpened, 0.011832)] {
        assert!((got - want).abs() < 1e-12, "{got} against {want}");
    }
    assert_eq!(format!("{bare:.3} {iron:.4} {diamond:.4} {sharpened:.4}"), "0.064 0.0216 0.0158 0.0118");
    let protected = army::risk(&d, Some(("diamond_sword", 0.0)), &[("diamond_chestplate", 4.0)], true);
    assert!((protected / diamond - 0.76).abs() < 1e-12, "Protection IV takes 24% off");
    let deep = army::risk(&d, Some(("diamond_sword", 20.0)), &[("diamond_chestplate", 20.0)], true);
    assert!((deep / diamond - 0.6 * 0.5).abs() < 1e-12, "an edge cuts to no less than 0.6, a guard to a half");
}

// parity: LogisticsTests.soldiersAreSafestInWholeSets
#[test]
fn soldiers_are_safest_in_whole_sets() {
    let c = content();
    let d = &c.dials;
    let sword = Some(("iron_sword", 0.0));
    let chest = army::risk(d, sword, &[("iron_chestplate", 0.0)], false);
    let iron = ["iron_helmet", "iron_chestplate", "iron_leggings", "iron_boots"].map(|n| (n, 0.0));
    let set = army::risk(d, sword, &iron, false);
    let diamond = ["diamond_helmet", "diamond_chestplate", "diamond_leggings", "diamond_boots"].map(|n| (n, 0.0));
    let full = army::risk(d, sword, &diamond, false);
    assert!(chest > set && set > full, "chestplate {chest:.4} > iron set {set:.4} > diamond set {full:.4}");
    assert!((full / (d.death_base * 1.0) - 0.5).abs() < 1e-9, "a full diamond set halves the risk");
    assert!((chest - 0.0254).abs() < 1e-12 && (set - 0.0155).abs() < 1e-12, "{chest} {set}");
    assert_eq!((army::points(iron.iter().map(|p| p.0)), army::points(diamond.iter().map(|p| p.0))), (15.0, 20.0));
    let wants = Army::default().wants(&c, 4);
    assert!(wants.iter().any(|(g, _)| *g == good(&c, "iron_boots")), "the quartermaster wants boots too");
}

#[test]
fn the_quartermaster_wants_an_iron_set_each_shields_for_half_and_arrows() {
    let mut s = Society::new(content());
    let (home, _) = war(&mut s, 5);
    s.enlist(home, 5);
    let c = s.content().clone();
    let wants = s.army().wants(&c, 4);
    let want = |name: &str| wants.iter().find(|(g, _)| *g == good(&c, name)).map(|w| w.1);
    assert_eq!([want("iron_sword"), want("iron_helmet"), want("iron_chestplate"), want("iron_leggings"),
        want("iron_boots")], [Some(5); 5]);
    assert_eq!((want("shield"), want("arrow"), want("bread")), (Some(2), Some(80), Some(32)),
        "shields for half, two days' arrows each, four days' bread");
}

// parity: WarTests.theFallenAreGoneAndTheirTownGrieves
#[test]
fn the_fallen_are_gone_and_their_town_grieves() {
    let mut s = Society::new(content());
    let (home, front) = war(&mut s, 20);
    assert_eq!(s.enlist(home, 20), 20, "every idle hand, not the Merchant");
    s.set_front(front);
    s.advance(LOOK);
    // Its town grieves: half a point each fallen in the last 10 days, at most 2. Eight fell in the first ten days.
    let grieving = |s: &Society| {
        let people = s.town(home).people.clone();
        let recent: f64 = people.grief.iter().filter(|(until, _)| people.last < *until).map(|(_, n)| n).sum();
        let mut calm = people.clone();
        calm.grief.clear();
        let d = s.content().dials.clone();
        let lift = enterprise_core::people::mood_target(&d, &calm, None) - enterprise_core::people::mood_target(&d, &people, None);
        assert!((lift - army::grief(&d, &people)).abs() < 1e-12, "the mood target carries the grief");
        assert!((s.mood_target(home) - enterprise_core::people::mood_target(&d, &people, None)).abs() < 1e-12);
        (recent, lift)
    };
    s.advance(LOOK + 10 * DAY);
    assert_eq!(grieving(&s), (8.0, 2.0), "eight fallen: at most 2");
    s.advance(LOOK + 20 * DAY);
    assert_eq!(grieving(&s), (1.0, 0.5), "one fell on day 13");
    s.advance(LOOK + 30 * DAY);
    assert_eq!(grieving(&s), (0.0, 0.0), "and is mourned");
    let record = s.army().record(home);
    assert!(record.fallen > 0 && s.army().size() as u32 == 20 - record.fallen - record.deserted,
        "unarmed for a month, some fall and are gone: {record:?}");
    assert!(record.days > 0, "the rest serve on");
    assert!(record.hungry > 0, "unfed: they go hungry, and some desert");
    // The expected losses, carried: the same every run. By day 19 the last has deserted.
    assert_eq!((record.fallen, record.deserted, s.army().size()), (9, 11, 0), "{record:?}");
    assert_eq!((record.days, record.hungry), (99, 99), "99 soldier-days, all of them hungry");
    // Gone for good: off its roster (the Merchant and the deserters, home again, stay).
    let roster = s.roster(home);
    assert_eq!(roster.hands.len(), 21 - record.fallen as usize, "the fallen are off the roster");
    assert_eq!(s.town(home).people.villagers, 12.0, "its people counted without them");
    assert!(roster.hands.iter().all(|h| !h.away), "the deserters home again");
}

#[test]
fn grief_is_half_a_point_a_fallen_soldier_for_ten_days_at_most_two() {
    let c = content();
    let d = &c.dials;
    let mut p = People { last: 0, ..Default::default() };
    army::mourn(&c, &mut p, 0);
    assert_eq!(army::grief(d, &p), 0.5);
    for _ in 0..2 {
        army::mourn(&c, &mut p, 0);
    }
    assert_eq!(army::grief(d, &p), 1.5);
    for _ in 0..3 {
        army::mourn(&c, &mut p, DAY);
    }
    assert_eq!(army::grief(d, &p), 2.0, "six fallen: at most 2");
    p.last = 10 * DAY;
    assert_eq!(army::grief(d, &p), 1.5, "ten days on, the first three are mourned");
    p.last = 11 * DAY;
    assert_eq!(army::grief(d, &p), 0.0, "and then all");
    army::mourn(&c, &mut p, 11 * DAY);
    assert_eq!(p.grief, vec![(21 * DAY, 1.0)], "old griefs dropped when a new one comes");
}

#[test]
fn losses_are_the_expected_value_carried_not_dice() {
    // Ten bare soldiers, fed on bread: 0.064 a day each, 0.64 a day in all, carried.
    let fallen_by_day = |seed: u64| {
        let mut s = Society::new(content());
        s.seed(seed);
        let (home, front) = war(&mut s, 10);
        deliver(&mut s, front, "bread", 640.0);
        s.enlist(home, 10);
        s.set_front(front);
        s.advance(LOOK);
        (1..=6).map(|day| {
            s.advance(LOOK + day * DAY);
            (s.army().record(home).fallen, s.army().falling)
        }).collect::<Vec<_>>()
    };
    let a = fallen_by_day(1);
    let fallen: Vec<u32> = a.iter().map(|x| x.0).collect();
    assert_eq!(fallen, [0, 1, 1, 2, 2, 3], "0.64, 1.28, then 9 at 0.064 a day, then 8...");
    let carried: Vec<f64> = a.iter().map(|x| x.1).collect();
    for (got, want) in carried.iter().zip([0.64, 0.28, 0.856, 0.432, 0.944, 0.456]) {
        assert!((got - want).abs() < 1e-9, "{carried:?}");
    }
    assert_eq!(a, fallen_by_day(98765), "the world's dice don't come into it");
}

#[test]
fn the_hungry_desert_and_walk_home() {
    let mut s = Society::new(content());
    let (home, front) = war(&mut s, 10);
    s.enlist(home, 10);
    s.set_front(front);
    s.advance(LOOK);
    s.advance(LOOK + DAY);
    let record = s.army().record(home);
    assert_eq!((record.deserted, record.fallen, record.hungry), (1, 0, 9),
        "ten hungry at 0.1: one deserts; the nine left serve hungry");
    assert!((s.army().falling - 9.0 * 0.064 * 1.5).abs() < 1e-9, "at half again the risk");
    assert_eq!(s.army().walking.len(), 1, "it walks home");
    let away = |s: &Society| s.roster(home).hands.iter().filter(|h| h.away).count();
    assert_eq!(away(&s), 10, "still on the road");
    s.advance(LOOK + 2 * DAY + DAY / 2);
    let record = s.army().record(home);
    assert_eq!((record.deserted, s.army().walking.len()), (1, 0), "1,500 blocks: home in a day and a quarter");
    assert_eq!(away(&s) as u32, 10 - record.deserted - record.fallen, "at home again in its roster");
}

#[test]
fn a_hearty_army_deserts_half_as_readily_and_falls_less() {
    let run = |food: &str| {
        let mut s = Society::new(content());
        let (home, front) = war(&mut s, 10);
        deliver(&mut s, front, food, 5.0); // short: half fed
        s.enlist(home, 10);
        s.set_front(front);
        s.advance(LOOK);
        s.advance(LOOK + DAY);
        (s.army().deserting, s.army().falling, s.army().heartiness())
    };
    let (bread_desert, bread_fall, plain) = run("bread"); // 25 points: 3 fed, 7 hungry
    let (beef_desert, beef_fall, hearty) = run("cooked_beef"); // 40 points: 5 fed, 5 hungry
    assert_eq!(plain, 0.0);
    assert!((hearty - 1.0 / 3.0).abs() < 1e-12);
    assert!((bread_desert - 0.7).abs() < 1e-9, "seven hungry at 0.1: {bread_desert}");
    assert!((beef_desert - 5.0 * 0.1 * (1.0 - 0.5 / 3.0)).abs() < 1e-9, "five, at a sixth less: {beef_desert}");
    assert!((bread_fall - (3.0 + 7.0 * 1.5) * 0.064).abs() < 1e-9);
    assert!((beef_fall - (5.0 * (1.0 - 0.05) + 5.0 * 1.5) * 0.064).abs() < 1e-9, "the fed 5% less: {beef_fall}");
}

#[test]
fn soldiers_draw_the_best_kit_there_is_and_wear_it_out() {
    let mut s = Society::new(content());
    let (home, front) = war(&mut s, 2);
    for (name, n) in [("diamond_sword", 1.0), ("iron_sword", 3.0), ("iron_chestplate", 2.0), ("leather_boots", 1.0),
        ("shield", 2.0), ("bread", 640.0)] {
        deliver(&mut s, front, name, n);
    }
    s.enlist(home, 2);
    s.set_front(front);
    s.advance(LOOK);
    s.advance(LOOK + DAY);
    let c = s.content().clone();
    let weapon = |s: &Society, i: usize| s.army().soldiers[i].weapon.as_ref().map(|w| c.goods[w.good].name.clone());
    assert_eq!((weapon(&s, 0).as_deref(), weapon(&s, 1).as_deref()), (Some("diamond_sword"), Some("iron_sword")),
        "the best first");
    assert!(s.army().soldiers[0].armour[3].is_some() && s.army().soldiers[1].armour[3].is_none(), "one pair of boots");
    assert!(s.army().soldiers.iter().all(|x| x.shield.is_some() && x.armour[1].is_some()), "chestplates and shields");
    assert_eq!((stock(&s, front, "iron_sword"), stock(&s, front, "shield")), (2.0, 0.0), "drawn from the front's");
    assert_eq!(s.army().issued.get("diamond_sword"), Some(&1));
    let worn = s.army().soldiers[1].weapon.clone().unwrap();
    assert!((worn.worn - 0.1).abs() < 1e-12, "an iron sword's 10 days");
    // An iron sword gone after 11 days (the mod's ten tenths summed in doubles fall just short of 1), another drawn
    // the day after.
    s.advance(LOOK + 11 * DAY);
    assert_eq!(stock(&s, front, "iron_sword"), 2.0, "still the first");
    s.advance(LOOK + 12 * DAY);
    assert_eq!(stock(&s, front, "iron_sword"), 1.0, "worn out, and the next drawn");
    assert!(s.army().soldiers[1].weapon.is_some());
    let risk = army::risk(&c.dials, Some(("iron_sword", 0.0)), &[("iron_chestplate", 0.0)], true);
    assert!(risk < army::risk(&c.dials, None, &[], false));
}

#[test]
fn enchanted_pieces_go_first_and_unbreaking_lasts() {
    let mut s = Society::new(content());
    let (home, front) = war(&mut s, 2);
    deliver(&mut s, front, "iron_sword", 1.0);
    assert!(s.add_kit(front, "iron_sword", 1, 3.0, 1), "a sword with Sharpness III and Unbreaking I");
    deliver(&mut s, front, "bread", 640.0);
    s.enlist(home, 2);
    s.set_front(front);
    s.advance(LOOK);
    s.advance(LOOK + DAY);
    let army = s.army();
    let first = army.soldiers[0].weapon.clone().unwrap();
    assert_eq!((first.enchant, first.unbreaking), (3.0, 1), "the enchanted one first");
    assert!((first.worn - 0.05).abs() < 1e-12, "Unbreaking I: twice as long");
    assert_eq!(army.soldiers[1].weapon.as_ref().map(|w| w.enchant), Some(0.0), "then the plain one");
    assert_eq!((army.issued.get("iron_sword (enchanted)"), army.issued.get("iron_sword")), (Some(&1), Some(&1)));
    assert!(s.kit(front).pieces.is_empty(), "the front's enchanted piece is drawn");
    let d = &s.content().dials;
    let (fed_sharp, fed_plain) = (army::risk(d, Some(("iron_sword", 3.0)), &[], false), army::risk(d, Some(("iron_sword", 0.0)), &[], false));
    let exposure: Vec<f64> = army.soldiers.iter().map(|x| x.exposure).collect();
    assert!((exposure[0] - fed_sharp).abs() < 1e-12 && (exposure[1] - fed_plain).abs() < 1e-12, "{exposure:?}");
}

#[test]
fn an_archer_draws_arrows_and_no_shield() {
    let mut s = Society::new(content());
    let (home, front) = war(&mut s, 1);
    for (name, n) in [("bow", 1.0), ("arrow", 40.0), ("shield", 1.0), ("bread", 64.0)] {
        deliver(&mut s, front, name, n);
    }
    s.enlist(home, 1);
    s.set_front(front);
    s.advance(LOOK);
    s.advance(LOOK + DAY);
    let archer = s.army().soldiers[0].clone();
    assert_eq!(archer.arrows, 24, "four days' arrows drawn, a day's shot");
    assert!(archer.shield.is_none() && stock(&s, front, "shield") == 1.0, "a bow takes both hands");
    assert_eq!(stock(&s, front, "arrow"), 8.0);
    s.advance(LOOK + 4 * DAY);
    assert_eq!((s.army().soldiers[0].arrows, stock(&s, front, "arrow")), (0, 8.0), "the last of its 32 shot on day 4");
    s.advance(LOOK + 5 * DAY);
    assert_eq!((s.army().soldiers[0].arrows, stock(&s, front, "arrow")), (0, 0.0), "the front's last 8, shot on day 5");
    let d = s.content().dials.clone();
    let (bow, bare) = (army::risk(&d, Some(("bow", 0.0)), &[], false), army::risk(&d, None, &[], false));
    assert!((s.army().soldiers[0].exposure - 5.0 * bow).abs() < 1e-12, "five days an archer");
    s.advance(LOOK + 6 * DAY);
    let exposure = s.army().soldiers[0].exposure;
    assert!((exposure - (5.0 * bow + bare)).abs() < 1e-12, "a bow with no arrows is no weapon: {exposure}");
    assert!(s.army().soldiers[0].weapon.as_ref().is_some_and(|w| w.worn < 0.6 - 1e-9), "and doesn't wear");
}

#[test]
fn a_levy_takes_idle_hands_first_then_spare_trades_then_the_least_skilled() {
    let mut s = Society::new(content());
    let t = s.add_town("Town", 0.0);
    s.set_roster(t, &[("merchant", 3), ("farmer", 4), ("cleric", 5), ("farmer", 2), ("librarian", 1), ("none", 1),
        ("nitwit", 1)]);
    let order = army::levy_order(s.content(), &s.roster(t), 7);
    assert_eq!(order, [5, 6, 3, 1, 4, 2], "idle, then the Farmers (the least skilled first), then the rest; never the Merchant");
    assert_eq!(s.muster(t, 3), 3);
    let roster = s.roster(t);
    let away: Vec<usize> = (0..roster.hands.len()).filter(|&i| roster.hands[i].away).collect();
    assert_eq!(away, [3, 5, 6], "away from home, waiting at the capital");
    assert_eq!(army::levy_order(s.content(), &roster, 7), [4, 1, 2],
        "the one Farmer left is the only one of its trade: the least skilled of the rest first");
    // A child never goes.
    let kid = s.add_town("Young", 0.0);
    s.set_roster(kid, &[("merchant", 1)]);
    s.set_people(kid, 1.0, 4.0, 10.0);
    assert_eq!(s.muster(kid, 2), 0, "nobody but the Merchant");
}

#[test]
fn a_fallen_soldier_leaves_its_roster_and_its_locker() {
    let mut s = Society::new(content());
    let t = s.add_town("Town", 0.0);
    s.set_roster(t, &[("none", 1), ("merchant", 1), ("farmer", 2)]);
    let front = s.add_off_map("Front", 0.0, -1500.0, 0.0);
    assert_eq!(s.enlist(t, 1), 1, "the idle hand");
    s.set_front(front);
    s.advance(LOOK);
    // Bare and unfed, it would fall on the eleventh day (0.096 a day), but deserts on the tenth (0.1 a day).
    s.advance(LOOK + 11 * DAY);
    let record = s.army().record(t);
    assert_eq!((record.fallen, record.deserted), (0, 1), "a lone hungry soldier deserts first");
    let mut s = Society::new(content());
    let t = s.add_town("Town", 0.0);
    s.set_roster(t, &[("none", 1), ("merchant", 1), ("farmer", 2)]);
    let front = s.add_off_map("Front", 0.0, -1500.0, 0.0);
    deliver(&mut s, front, "bread", 640.0);
    s.switch(t, 2, 0, true); // the Farmer's bread, switched on
    s.enlist(t, 1);
    s.set_front(front);
    s.advance(LOOK);
    s.advance(LOOK + 15 * DAY);
    assert_eq!(s.army().record(t).fallen, 0);
    s.advance(LOOK + 16 * DAY);
    assert_eq!(s.army().record(t).fallen, 1, "fed, bare: 0.064 a day, fallen on the sixteenth");
    let roster = s.roster(t);
    assert_eq!(roster.hands.len(), 2);
    assert_eq!(roster.merchant, Some(0), "the Merchant still at its desk, one place up");
    assert_eq!(s.town(t).people.villagers, 2.0, "two left");
    assert_eq!(s.town(t).people.grief.len(), 1);
    let lockers = s.lockers(t).unwrap();
    assert_eq!(lockers.hands.len(), 2);
    assert!(lockers.hands[1].offers[0].on, "the Farmer keeps its own locker, one place up");
}

#[test]
fn pay_is_owed_home_through_its_levy_or_the_armys_ledger() {
    let mut s = Society::new(content());
    let (home, front) = war(&mut s, 4);
    let capital = s.add_town("Capital", 0.0);
    s.set_capital(Some(capital));
    let bread = good(s.content(), "bread");
    let levy = s.draft(home, capital, bread, 1.0, 0.0, 1000 * DAY, DAY, 1, Incoterm::Exw, true);
    // A contract of its home's with another town is no levy: its soldier's pay isn't owed through it (the mod's
    // `buyer == capital`).
    let other = s.add_town("Other", 0.0);
    let not_levy = s.draft(home, other, bread, 1.0, 0.0, 1000 * DAY, DAY, 1, Incoterm::Exw, true);
    deliver(&mut s, front, "bread", 960.0);
    assert_eq!(s.muster_levied(home, 2, Some(levy)), 2);
    assert_eq!(s.muster(home, 1), 1);
    assert_eq!(s.muster_levied(home, 1, Some(not_levy)), 1);
    assert_eq!(s.embark(10), 4, "all four reach the front");
    s.set_front(front);
    s.advance(LOOK);
    s.advance(LOOK + 3 * DAY);
    assert!((s.contract(levy).owing - 2.0 * 3.0 * 0.25).abs() < 1e-12, "the levied two: owed under the levy");
    assert!((s.army().pay_owed[&home] - 2.0 * 3.0 * 0.25).abs() < 1e-12, "the other two: in the army's ledger");
    assert_eq!(s.contract(not_levy).owing, 0.0, "none through a contract with another town");
    assert!((s.army().record(home).pay - 3.0).abs() < 1e-12, "12 soldier-days at 0.25");
}

#[test]
fn recruits_wait_at_the_capital_and_join_at_the_front() {
    let mut s = Society::new(content());
    let (home, front) = war(&mut s, 3);
    s.muster(home, 3);
    s.set_front(front);
    s.advance(LOOK);
    s.advance(LOOK + 2 * DAY);
    assert_eq!((s.army().size(), s.army().waiting(), s.army().record(home).raised), (0, 3, 0),
        "waiting at the capital: not yet raised, not at risk");
    assert_eq!(s.embark(2), 2);
    assert_eq!((s.army().size(), s.army().waiting(), s.army().record(home).raised), (2, 1, 2));
    assert_eq!(s.army().serving_from(home), 2);
}

#[test]
fn at_peace_they_walk_home() {
    let mut s = Society::new(content());
    let (home, front) = war(&mut s, 4);
    deliver(&mut s, front, "bread", 640.0);
    s.enlist(home, 3);
    s.muster(home, 1);
    s.set_front(front);
    s.advance(LOOK);
    s.advance(LOOK + DAY);
    assert_eq!(s.demobilise(), 4, "three soldiers and a recruit");
    assert_eq!((s.army().size(), s.army().waiting(), s.army().last_day), (0, 0, None));
    s.advance(2 * LOOK + DAY);
    let away = |s: &Society| s.roster(home).hands.iter().filter(|h| h.away).count();
    assert_eq!(away(&s), 3, "the recruit is home (no capital: it never left the town), the soldiers on the road");
    s.advance(LOOK + 3 * DAY);
    assert_eq!(away(&s), 0, "all home again");
    assert!(s.army().walking.is_empty());
}

#[test]
fn the_books_balance_through_a_campaign_and_a_save() {
    let mut s = Society::new(content());
    let (home, front) = war(&mut s, 12);
    for (name, n) in [("bread", 100.0), ("cooked_beef", 20.0), ("iron_sword", 4.0), ("bow", 2.0), ("arrow", 64.0),
        ("iron_helmet", 3.0), ("shield", 3.0)] {
        deliver(&mut s, front, name, n);
    }
    s.add_kit(front, "iron_helmet", 1, 2.0, 2);
    let start = s.totals();
    s.enlist(home, 8);
    s.muster(home, 2);
    s.set_front(front);
    s.advance(LOOK);
    s.advance(10 * DAY);
    s.conserved(&start).unwrap();
    s.solvent().unwrap();
    let blob = s.save();
    let mut loaded = Society::load(&blob).unwrap();
    assert!(loaded.unsaved().is_empty(), "{:?}", loaded.unsaved());
    s.advance(25 * DAY);
    loaded.advance(25 * DAY);
    assert_eq!(s.army(), loaded.army(), "load, then run: as never saved");
    assert_eq!(s.save(), loaded.save());
    s.conserved(&start).unwrap();
}

#[test]
fn a_front_kept_by_the_capital_wants_eleven_days_of_rations() {
    let mut s = Society::new(content());
    let (home, front) = war(&mut s, 2);
    s.set_capital(Some(home));
    s.enlist(home, 2);
    assert!(s.front_wants().is_empty(), "no front, no wants");
    s.set_front(front);
    let bread = good(s.content(), "bread");
    let loaves = s.front_wants().iter().find(|w| w.0 == bread).unwrap().1;
    assert_eq!(loaves, (2.0f64 * 8.0 * 11.0 / 5.0).ceil() as u32, "1,500 blocks: 11 days");
}
