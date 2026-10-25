//! Public works (`works.rs`), pinned as the mod's game tests pin them (`GrowthTests`, `BuildingTests`,
//! `CountingHouseTests`, `MasterTests`) and as its Java fixes the numbers: masons lay 32 blocks a day each from stone
//! at 16 a hex a day, two masons at an emerald a day, one project at a time.

use std::collections::BTreeMap;

use enterprise_core::cargo::LADING;
use enterprise_core::cash_book::{Account, Reason};
use enterprise_core::works::{deck_size, ring_size, Kind, What, KINDS};
use enterprise_core::{Content, Good, Hex, Society};

const DAY: u64 = 24_000;

fn skills(of: &[(&str, u8)]) -> BTreeMap<String, u8> {
    of.iter().map(|&(t, l)| (t.to_string(), l)).collect()
}

/// One look on.
fn look(s: &mut Society) {
    let next = s.now() + s.content().dials.look_ticks;
    s.advance(next);
}

/// The mod's fixture for the bridge: a town with 10 emeralds and 54 more, holding four hexes (so 64 blocks of stone a
/// day for two masons at 32), and a bridge on the hex at (5000, 5000) under way, its deck running east and west.
const HEX: Hex = Hex::new(5000, 5000);
const FROM: Hex = Hex::new(4999, 5000);
const TO: Hex = Hex::new(5001, 5000);

fn bridging() -> Society {
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64)], DAY));
    let t = s.add_town("Bridgeford", 64.0);
    for hex in [Hex::new(0, 0), Hex::new(1, 0), Hex::new(0, 1), Hex::new(1, -1)] {
        assert!(s.claim_hex(t, hex));
    }
    s.start_project(t, What::Bridge { hex: HEX, from: FROM, to: TO });
    s
}

#[test]
fn the_decks_and_the_ring_are_counted_from_the_mods_geometry() {
    // `GrowthTests.aWallRingsTheBellWithFourGates`: about 2 pi x 40 columns, none in the gates.
    assert_eq!(ring_size(40), 252);
    assert_eq!(s_default().wall_blocks, 252 * 3, "the dial's default is the ring, 3 high");
    let deck = deck_size(HEX, FROM, TO);
    assert!((440..=470).contains(&deck), "a deck of about 450 blocks: {deck}");
    assert!(ring_size(3) > 0 && ring_size(3) < 40, "a small ring");
    assert_eq!(Kind::Temple.blocks(), 196, "7 x 7 x 4");
    assert_eq!(Kind::Square.blocks(), 81);
}

fn s_default() -> enterprise_core::Dials {
    enterprise_core::Dials::default()
}

// parity: GrowthTests.masonsBuildABridgeForWages
#[test]
fn masons_build_a_bridge_for_wages() {
    let mut s = bridging();
    let before = s.totals();
    let deck = deck_size(HEX, FROM, TO) as f64;
    look(&mut s); // starts the clock
    let t0 = s.now();
    assert_eq!(s.works(0).projects[0].laid, 0.0);
    s.advance(t0 + 3 * DAY);
    // 4 hexes of stone a day (64 blocks) for 2 masons at 32 a day: 192 blocks in 3 days, 6 emeralds of wages.
    let p = s.works(0).projects[0].clone();
    assert!((p.progress() * deck - 192.0).abs() < 1.0, "192 blocks laid: {}", p.progress() * deck);
    assert!(s.town(0).money == 58.0, "6 emeralds of wages paid: {}", s.town(0).money);
    assert_eq!(s.account_book(enterprise_core::cash_book::Account::Treasury(0), 5).iter()
        .filter(|(_, what, _)| what == "masons").map(|(_, _, n)| *n).sum::<enterprise_core::Fixed>(), enterprise_core::Fixed::emeralds(-6), "booked to the masons");
    assert!(!s.map().ground(HEX).bridged, "not yet");
    s.advance(t0 + 10 * DAY);
    let p = &s.works(0).projects[0];
    assert!(p.done && s.map().ground(HEX).bridged, "finished within 10 days");
    assert!((s.town(0).money.to_f64() - (64.0 - 2.0 * 7.0)).abs() < 1.0 + 2.0, "and no more wages after: {}", s.town(0).money);
    s.conserved(&before).unwrap();
    s.solvent().unwrap();
}

#[test]
fn the_first_touch_only_starts_the_clock_and_a_town_with_little_land_builds_slowly() {
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64)], DAY));
    let t = s.add_town("Smallholding", 100.0);
    s.claim_hex(t, Hex::new(0, 0)); // one hex: 16 blocks of stone a day, not 64
    s.start_project(t, What::Hall(Kind::Square));
    look(&mut s);
    assert_eq!(s.town(t).money, 100.0, "nothing paid at the first look: it only starts the clock");
    let t0 = s.now();
    s.advance(t0 + 2 * DAY);
    let p = &s.works(t).projects[0];
    assert!((p.laid - 32.0).abs() < 1e-9, "16 blocks a day from one hex: {}", p.laid);
    assert!(s.town(t).money == 96.0, "but the masons are paid for their days: {}", s.town(t).money);
}

#[test]
fn unpaid_masons_dont_lay_and_arent_owed() {
    let mut s = bridging();
    s.post(Account::Treasury(0), Account::Outside, 64.0, Reason::Other("gone".into()));
    look(&mut s);
    let t0 = s.now();
    s.advance(t0 + 3 * DAY);
    let p = &s.works(0).projects[0];
    assert_eq!(p.laid, 0.0, "no money, no work");
    assert_eq!(p.wage_owed, 0.0, "and nothing owed");
    s.pay(0, 100.0);
    s.advance(t0 + 4 * DAY);
    assert!((s.works(0).projects[0].laid - 64.0).abs() < 1e-9, "paid again, they lay a day's work");
}

#[test]
fn one_project_at_a_time_bridges_first_then_the_wall_then_halls() {
    let mut s = bridging();
    s.start_project(0, What::Wall);
    s.start_project(0, What::Hall(Kind::Square));
    look(&mut s);
    let t0 = s.now();
    s.advance(t0 + 2 * DAY);
    let works = s.works(0);
    assert!(works.projects[0].laid > 0.0 && works.projects[1].laid == 0.0 && works.projects[2].laid == 0.0,
        "the bridge only: {:?}", works.projects.iter().map(|p| p.laid).collect::<Vec<_>>());
    assert!(s.town(0).money == 60.0, "two masons' wages for two days, not six: {}", s.town(0).money);
    s.finish_project(0, What::Bridge { hex: HEX, from: FROM, to: TO });
    assert_eq!(s.works(0).underway(), Some(1), "then the wall");
    assert!(s.map().ground(HEX).bridged, "a bridge finished at once is on the map");
}

// parity: GrowthTests.aWalledTownFeelsSafe
#[test]
fn a_walled_town_feels_safe() {
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64)], DAY));
    let t = s.add_town("Walled", 0.0);
    s.set_people(t, 3.0, 3.0, 3.0);
    s.start_project(t, What::Wall);
    s.finish_project(t, What::Wall);
    s.advance(30 * DAY);
    // No comforts, 3 villagers: -3/8 for size, +1 for the wall.
    let mood = s.town(t).people.mood;
    assert!((mood - (1.0 - 3.0 / 8.0)).abs() < 0.01, "walled: {mood}");
}

#[test]
fn half_a_wall_is_half_the_mood_and_half_the_culture() {
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64)], DAY));
    let t = s.add_town("Halfway", 0.0);
    s.start_project(t, What::Wall);
    s.help_project(t, 378.0);
    assert!((s.walled(t) - 0.5).abs() < 1e-9);
    assert!((s.mood_target(t) - 0.5).abs() < 1e-9, "the share standing");
    look(&mut s);
    look(&mut s);
    assert!((s.holding(t).extra_culture - 1.0).abs() < 1e-9, "2 culture a day whole: {}", s.holding(t).extra_culture);
    s.help_project(t, 1000.0);
    look(&mut s);
    assert!((s.holding(t).extra_culture - 2.0).abs() < 1e-9);
}

// parity: BuildingTests.aSquareMakesItsTownHappier
#[test]
fn a_square_makes_its_town_happier() {
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64)], DAY));
    let t = s.add_town("Squareton", 0.0);
    s.set_people(t, 3.0, 3.0, 3.0);
    let before = s.mood_target(t);
    s.start_project(t, What::Hall(Kind::Square));
    assert_eq!(s.mood_target(t), before, "under way, not yet");
    s.finish_project(t, What::Hall(Kind::Square));
    s.advance(DAY);
    let after = s.mood_target(t);
    assert!((after - before - 0.5).abs() < 1e-9, "a square: +0.5, {before} -> {after}");
}

// parity: BuildingTests.aVisitingMasonLaysADaysWork
#[test]
fn a_visiting_mason_lays_a_days_work() {
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64)], DAY));
    let t = s.add_town("Masonless", 0.0);
    s.start_project(t, What::Hall(Kind::Temple));
    assert!(s.masonry_wanted(t, 2, 5.0), "a temple under way and no Mason: wanted");
    assert!(!s.masonry_wanted(t, 2, 1.0), "unless the visitor's market can't pay a level-2 Mason's 2");
    let fee = s.masonry_abroad(t, 2);
    let p = &s.works(t).projects[0];
    assert!((p.progress() - 64.0 / Kind::Temple.blocks() as f64).abs() < 1e-9 && fee == 2.0,
        "a level-2 Mason lays 64 blocks for 2 emeralds: {}, {fee}", p.progress());
    // A town with a Mason of its own wants none; nor one with nothing under way.
    let home = s.add_town("Masonful", 0.0);
    s.set_roster(home, &[("mason", 1)]);
    s.start_project(home, What::Hall(Kind::Temple));
    assert!(!s.masonry_wanted(home, 2, 5.0));
    let idle = s.add_town("Idle", 0.0);
    assert!(!s.masonry_wanted(idle, 2, 5.0), "nothing under way");
}

// parity: CountingHouseTests.aCountingHouseNeedsBothItsTrades
#[test]
fn a_counting_house_needs_both_its_trades() {
    let house = Kind::CountingHouse;
    assert!(house.partnered(&skills(&[("librarian", 3)]), 3), "a Librarian and a Merchant, both at 3");
    assert!(!house.partnered(&skills(&[("librarian", 3)]), 1), "a Novice Merchant isn't enough");
    assert!(!house.partnered(&skills(&[]), 5), "nor a master Merchant with no Librarian");
    assert!(Kind::Smithy.partnered(&skills(&[("armorer", 3)]), 1), "a smithy: any one of its smiths");
    assert!(!Kind::Smithy.partnered(&skills(&[("armorer", 2)]), 5), "at 3");
    assert!(Kind::Square.partnered(&skills(&[]), 1), "a square needs no trade");
    assert!(Kind::Caravanserai.partnered(&skills(&[]), 3), "a caravanserai: a Merchant at 3");
}

// parity: CountingHouseTests.aCountingHousesClerksDrawUpPapersCheaply
#[test]
fn a_counting_houses_clerks_draw_up_papers_cheaply() {
    let mut s = Society::new(Content::new(vec![Good::new("paper", 0.05, 64), Good::new("ink_sac", 0.05, 64)], DAY));
    let t = s.add_town("Haygate", 10.0);
    s.draw_papers(t, 10, LADING);
    look(&mut s);
    let scribes = s.paperwork(t).scribe_fees;
    assert!(scribes == 1.0, "no stationery: a scribe at 0.1 a document, {scribes}");
    s.start_project(t, What::Hall(Kind::CountingHouse));
    s.finish_project(t, What::Hall(Kind::CountingHouse));
    s.draw_papers(t, 10, LADING);
    look(&mut s);
    let clerks = s.paperwork(t).scribe_fees - scribes;
    assert!(clerks == 0.2, "with a counting house: its clerks at 0.02, {clerks}");
    // And half the stationery: ten documents take five sheets, not ten.
    s.deliver(t, 0, 20.0);
    s.deliver(t, 1, 20.0);
    s.draw_papers(t, 10, LADING);
    look(&mut s);
    assert!((s.town(t).stock[0] - 15.0).abs() < 1e-9, "five sheets of twenty: {}", s.town(t).stock[0]);
}

#[test]
fn a_counting_house_widens_the_accountants_budget() {
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64)], DAY));
    let t = s.add_town("Haygate", 100.0);
    assert!(s.budget_left(t) == 50.0, "half the free money");
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64)], DAY));
    let t = s.add_town("Haygate", 100.0);
    s.start_project(t, What::Hall(Kind::CountingHouse));
    s.finish_project(t, What::Hall(Kind::CountingHouse));
    assert!(s.budget_left(t) == 75.0, "half as much again: {}", s.budget_left(t));
    // And it holds back bills for twice the days: one falling due in four days is free money to three days' notice,
    // but not to six.
    let content = Content::new(vec![Good::new("bread", 0.2, 64)], DAY);
    let mut bills = enterprise_core::bills::Bills::default();
    bills.draw(0, 1, 10.0, 0, DAY);
    let mut trades = enterprise_core::state::Trades { makes: vec![0.0], uses: vec![0.0], capacity: vec![0.0], counting_house: false, ..Default::default() };
    let money = enterprise_core::state::Money(enterprise_core::Fixed::emeralds(100));
    assert_eq!(enterprise_core::rules::free(&content, &trades, &money, &bills, 0, 0), 100.0, "three days: not yet counted");
    trades.counting_house = true;
    let held = enterprise_core::rules::free(&content, &trades, &money, &bills, 0, 0);
    assert!(held <= enterprise_core::Fixed::emeralds(90), "six days: counted against it: {held}");
}

// parity-partial: MasterTests.aMasterFarmerPlantsFlowerBeds (the beds as blocks on the square, and the dandelions in the Merchant's chest, are Minecraft's: the flag, the eight flowers and the mood are pinned)
#[test]
fn a_master_farmer_plants_flower_beds_on_the_square() {
    // (The mod's beds are blocks in the square; the core keeps the flag, the eight flowers and the mood.)
    let mut s = Society::new(Content::new(vec![Good::new("dandelion", 0.05, 64), Good::new("poppy", 0.05, 64)], DAY));
    let t = s.add_town("Flowerby", 0.0);
    s.set_people(t, 3.0, 3.0, 3.0);
    s.set_roster(t, &[("farmer", 5), ("none", 1), ("none", 1)]);
    s.start_project(t, What::Hall(Kind::Square));
    s.finish_project(t, What::Hall(Kind::Square));
    let before = s.mood_target(t);
    s.deliver(t, 0, 5.0);
    s.advance(DAY);
    assert!(!s.works(t).flowers, "five flowers aren't eight");
    s.deliver(t, 1, 3.0);
    s.advance(2 * DAY);
    assert!(s.works(t).flowers, "flower beds on the square");
    assert_eq!(s.town(t).stock[..2], [0.0, 0.0], "of the eight flowers");
    assert!((s.mood_target(t) - before - 0.5).abs() < 1e-9, "+0.5 mood");
    // Not a Farmer of lesser skill.
    let mut s = Society::new(Content::new(vec![Good::new("dandelion", 0.05, 64)], DAY));
    let t = s.add_town("Lesser", 0.0);
    s.set_roster(t, &[("farmer", 4)]);
    s.start_project(t, What::Hall(Kind::Square));
    s.finish_project(t, What::Hall(Kind::Square));
    s.deliver(t, 0, 8.0);
    s.advance(2 * DAY);
    assert!(!s.works(t).flowers, "a master's work");
}

#[test]
fn a_granary_finished_sets_up_the_farms_store() {
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 0.05, 64)], DAY));
    let t = s.add_town("Barnsley", 0.0);
    s.set_plots(t, 0, 4);
    s.start_project(t, What::Hall(Kind::Granary));
    assert!(s.farm(t).unwrap().farm.stores.is_empty());
    s.finish_project(t, What::Hall(Kind::Granary));
    assert_eq!(s.farm(t).unwrap().farm.stores, [enterprise_core::farms::Store::Granary], "its store is set up");
    assert!(s.works(t).built(Kind::Granary) && s.works(t).built_of() == [Kind::Granary]);
}

/// A town of `villagers` with every comfort for `days`, content (its mood heads for several points), money, holding
/// `hexes` hexes, with `roster`.
fn content_town(money: f64, hexes: usize, roster: &[(&str, u8)]) -> Society {
    let mut goods: Vec<Good> = Content::COMFORTS.iter().enumerate()
        .map(|(k, name)| Good::new(&format!("{name}_good"), 0.05, 64).comfort(k)).collect();
    goods.push(Good::new("oak_log", 0.125, 64));
    goods.push(Good::new("bread", 0.2, 64).food(5.0));
    let mut s = Society::new(Content::new(goods, DAY));
    s.content_mut().dials.budget_share = 1.0;
    s.content_mut().dials.standing_orders = false;
    let t = s.add_town("Contentville", money);
    s.set_people(t, 8.0, 8.0, 8.0);
    for k in 0..5 {
        s.deliver(t, k, (8.0f64 * 0.1 * 60.0 / 0.05).ceil());
    }
    if !roster.is_empty() {
        s.set_roster(t, roster);
    }
    let home = Hex::new(0, 0);
    s.place_town(t, home.center_x(), home.center_z());
    let mut held = vec![home];
    while held.len() < hexes {
        let next = held.iter().flat_map(|h| h.neighbors()).find(|h| !held.contains(h)).unwrap();
        held.push(next);
    }
    for h in held {
        s.claim_hex(t, h);
    }
    s
}

const CONTENT_ROSTER: [(&str, u8); 8] = [("merchant", 1), ("farmer", 1), ("farmer", 1), ("none", 1), ("none", 1), ("none", 1),
    ("none", 1), ("none", 1)];

#[test]
fn a_content_town_with_money_to_spare_walls_itself_in_and_a_poor_one_doesnt() {
    // Mood 0 at first: it isn't content (0.5), so it builds nothing yet.
    let mut s = content_town(300.0, 4, &CONTENT_ROSTER);
    s.advance(DAY);
    assert!(s.works(0).projects.is_empty(), "not content yet: {:?}", s.town(0).people.mood);
    s.advance(20 * DAY);
    assert!(s.town(0).people.mood >= 0.5, "content: {}", s.town(0).people.mood);
    let works = s.works(0);
    assert!(works.wall().is_some() || !works.projects.is_empty(), "a content town with money builds: {works:?}");
    // A poor town doesn't (12 emeralds to spare and its reserve).
    let mut s = content_town(5.0, 4, &CONTENT_ROSTER);
    s.advance(25 * DAY);
    assert!(s.works(0).projects.is_empty(), "a poor town builds nothing: {:?}", s.works(0).projects);
}

#[test]
fn a_content_town_builds_its_wall_and_feels_safe_inside_it() {
    let mut s = content_town(300.0, 4, &CONTENT_ROSTER);
    let before = s.totals();
    s.advance(60 * DAY);
    let works = s.works(0);
    let wall = works.wall().expect("a wall started");
    assert!(wall.done, "63 blocks a day, 756 blocks: done in about 12 days: {:.2}", wall.progress());
    assert!((wall.wages_paid.to_f64() - (756.0f64 / 64.0 * 2.0).floor()).abs() <= 2.0, "wages paid: {}", wall.wages_paid);
    assert_eq!(s.walled(0), 1.0);
    s.conserved(&before).unwrap();
}

#[test]
fn a_town_builds_its_halls_in_order_when_it_can_pay_twice_over() {
    // A Mason of level 2 can build a square (8 emeralds of materials, 81 blocks of wages: 2.53).
    let mut roster = CONTENT_ROSTER.to_vec();
    roster.push(("mason", 2));
    let mut s = content_town(1000.0, 4, &roster);
    s.content_mut().dials.wall_start_emeralds = 1e9; // (no wall)
    s.advance(25 * DAY);
    let works = s.works(0);
    assert!(works.has_started(Kind::Square), "a square first: {:?}", works.projects);
    assert!(works.built(Kind::Square), "and built");
    let book = s.cash_book(0, 30);
    assert!(book.iter().any(|(_, what, n)| what == "a square" && *n == -8.0), "its materials: {book:?}");
    // Nothing else: a level-2 Mason can't build the rest (level 3), and the town lacks their trades.
    assert!(works.projects.iter().all(|p| p.what == What::Hall(Kind::Square)), "{:?}", works.projects);
}

#[test]
fn logs_in_the_warehouse_stand_in_for_materials() {
    let mut roster = CONTENT_ROSTER.to_vec();
    roster.push(("mason", 2));
    let mut s = content_town(1000.0, 4, &roster);
    s.content_mut().dials.wall_start_emeralds = 1e9;
    // (Its idle hands gather what their land gives, logs on plains (`kit.rs`): here the land is water, which gives none.)
    for q in -3..4 {
        for r in -3..4 {
            s.set_land(Hex::new(q, r), enterprise_core::map::Land::Water);
        }
    }
    s.deliver(0, 5, 32.0); // 32 logs at an eighth: 4 emeralds of the 8
    s.advance(25 * DAY);
    let book = s.cash_book(0, 30);
    assert!(book.iter().any(|(_, what, n)| what == "a square" && *n == -4.0), "half the materials in logs: {book:?}");
    assert_eq!(s.town(0).stock[5], 32.0 - 32.0, "all 32 logs used (the square wanted 64, it had 32)");
}

#[test]
fn a_town_without_a_mason_hires_builders_up_to_level_three() {
    // A temple wants a Mason at 3: a town with a Cleric at 3 and no Mason hires builders who can (HIRED_SKILL 3)...
    let roster = [("merchant", 1), ("cleric", 3), ("farmer", 1), ("farmer", 1), ("farmer", 1), ("farmer", 1), ("farmer", 1),
        ("farmer", 1)]; // (no idle hands, who'd take up a Mason's trade for the halls it wants)
    let mut s = content_town(2000.0, 4, &roster);
    s.content_mut().dials.wall_start_emeralds = 1e9;
    s.start_project(0, What::Hall(Kind::Square));
    s.finish_project(0, What::Hall(Kind::Square));
    s.advance(25 * DAY);
    assert!(s.works(0).has_started(Kind::Temple), "hired builders: {:?}", s.works(0).projects);
    // ...while a Mason of its own at level 2 can't.
    let mut own = roster.to_vec();
    own.push(("mason", 2));
    let mut s = content_town(2000.0, 4, &own);
    s.content_mut().dials.wall_start_emeralds = 1e9;
    s.start_project(0, What::Hall(Kind::Square));
    s.finish_project(0, What::Hall(Kind::Square));
    s.advance(25 * DAY);
    assert!(!s.works(0).has_started(Kind::Temple), "its Mason is level 2: {:?}", s.works(0).projects);
}

#[test]
fn a_counting_house_only_where_the_papers_pile_up() {
    let roster = [("merchant", 3), ("librarian", 3), ("mason", 3), ("none", 1), ("none", 1), ("none", 1), ("none", 1), ("farmer", 1)];
    let build = |documents: u32| {
        let mut s = content_town(2000.0, 4, &roster);
        s.content_mut().dials.wall_start_emeralds = 1e9;
        s.start_project(0, What::Hall(Kind::Square));
        s.finish_project(0, What::Hall(Kind::Square));
        s.start_project(0, What::Hall(Kind::Library));
        s.finish_project(0, What::Hall(Kind::Library)); // (so the library isn't what it saves for)
        s.draw_papers(0, documents, LADING);
        s.advance(20 * DAY);
        s.works(0).has_started(Kind::CountingHouse)
    };
    assert!(build(60), "three documents a day or more");
    assert!(!build(0), "none");
}

#[test]
fn a_library_and_a_caravanserai_bring_culture() {
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64)], DAY));
    let t = s.add_town("Booktown", 0.0);
    s.set_culture_extras(t, 5.0); // a host's own figure stays
    s.start_project(t, What::Hall(Kind::Library));
    s.finish_project(t, What::Hall(Kind::Library));
    s.start_project(t, What::Hall(Kind::Caravanserai));
    s.finish_project(t, What::Hall(Kind::Caravanserai));
    look(&mut s);
    assert_eq!(s.holding(t).extra_culture, 5.0 + 3.0 + 1.0, "3 and 1 a day beside the host's 5");
    s.set_culture_extras(t, 2.0); // and the works' stay when the host's figure changes after
    look(&mut s);
    assert_eq!(s.holding(t).extra_culture, 2.0 + 3.0 + 1.0, "3 and 1 a day beside the host's 2");
}

#[test]
fn a_town_starts_the_bridge_that_repays_its_wages() {
    use std::collections::BTreeSet;
    let bridge_hex = Hex::new(6, 0);
    let (start, goal) = (Hex::new(0, 0), Hex::new(12, 0));
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64).food(5.0)], DAY));
    s.content_mut().dials.budget_share = 1.0;
    let town = s.add_town("Bridgeford", 200.0);
    s.place_town(town, 999_000.5, 999_000.5);
    for dq in -2..=2 {
        for dr in -2..=2 {
            let hex = Hex::new(bridge_hex.q + dq, bridge_hex.r + dr);
            if hex.distance(bridge_hex) <= 2 {
                s.claim_hex(town, hex);
            }
        }
    }
    // A river along q = 6, with a ford far off at (6, -6): the road from START to GOAL goes around by it.
    for r in -30..=30 {
        let hex = Hex::new(6, r);
        s.set_ground(hex, if hex == Hex::new(6, -6) { 0.0 } else { 1.0 }, 0.0);
    }
    let from = s.add_town("Start", 0.0);
    let to = s.add_town("Goal", 0.0);
    s.place_town(from, start.center_x(), start.center_z());
    s.place_town(to, goal.center_x(), goal.center_z());
    s.add_route(from, to, 1.0, 1.0, 1.0);
    let before = s.totals();
    let value = s.bridge_value(town, bridge_hex);
    assert!(value.tolls_per_day > 0.0 && value.from.is_some(), "a bridge would bring tolls: {value:?}");
    s.advance(10 * DAY);
    let works = s.works(town);
    let started: BTreeSet<Hex> = works.projects.iter().filter_map(|p| match p.what {
        What::Bridge { hex, .. } => Some(hex),
        _ => None,
    }).collect();
    assert_eq!(started.len(), 1, "one bridge, one at a time: {:?}", works.projects);
    assert_eq!(started.iter().next().unwrap().q, 6, "on the river");
    s.advance(40 * DAY);
    let hex = *started.iter().next().unwrap();
    assert!(s.works(town).bridged(hex) && s.map().ground(hex).bridged, "built and on the map");
    s.conserved(&before).unwrap();
}

#[test]
fn every_kind_is_in_the_order_a_town_builds_them() {
    let ids: Vec<&str> = KINDS.iter().map(|k| k.id()).collect();
    assert_eq!(ids, ["square", "granary", "root_cellar", "counting_house", "temple", "library", "smithy", "watchtower",
        "stone_docks", "caravanserai"]);
    assert_eq!(Kind::Temple.mason_level(), 3);
    assert_eq!(Kind::Caravanserai.mason_level(), 5);
    assert_eq!(Kind::StoneDocks.mason_level(), 4);
}

use crate::common;

#[test]
fn the_random_worlds_exercise_the_works() {
    let (mut walls, mut bridges, mut halls, mut started_by_towns) = (0, 0, 0, 0);
    for (w, b, h, st) in common::each_seed(common::seeds(60), |seed| {
        let (mut walls, mut bridges, mut halls, mut started_by_towns) = (0, 0, 0, 0);
        let mut s = common::world(seed);
        for day in 1..=80 {
            s.advance(day * DAY);
            common::slices::daily(&mut s, seed, day);
        }
        for t in 0..s.town_count() {
            for p in s.works(t).projects.iter().filter(|p| p.done) {
                match p.what {
                    What::Wall => walls += 1,
                    What::Bridge { .. } => bridges += 1,
                    What::Hall(_) => halls += 1,
                }
            }
            started_by_towns += s.works(t).projects.iter().filter(|p| p.started.is_some()).count();
        }
        (walls, bridges, halls, started_by_towns)
    }) {
        walls += w;
        bridges += b;
        halls += h;
        started_by_towns += st;
    }
    assert!(walls > 0 && bridges > 0 && halls > 0 && started_by_towns > 20, "{walls} walls, {bridges} bridges, {halls} halls");
}

#[test]
fn a_mason_learns_from_the_works_it_builds() {
    // A Novice Mason and the town's wall under way: a day of laying is a trade's XP (2), so in five days it's an
    // Apprentice (10), the level a square needs, with no clay to trade for at its locker.
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64)], DAY));
    let t = s.add_town("Stoneby", 100.0);
    s.set_roster(t, &[("merchant", 1), ("mason", 1)]);
    for hex in [Hex::new(0, 0), Hex::new(1, 0), Hex::new(0, 1), Hex::new(1, -1)] {
        assert!(s.claim_hex(t, hex));
    }
    s.start_project(t, What::Wall);
    look(&mut s); // starts the clock
    let t0 = s.now();
    let mason = |s: &Society| s.roster(t).hands.iter().find(|h| s.content().trades[h.trade].name == "mason").cloned().unwrap();
    s.advance(t0 + 2 * DAY);
    assert_eq!((mason(&s).level, mason(&s).xp), (1, 4), "two days' work");
    s.advance(t0 + 5 * DAY);
    assert_eq!(mason(&s).level, 2, "five days: an Apprentice ({} XP)", mason(&s).xp);
}
