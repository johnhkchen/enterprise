//! The host's land as the core's map and fields, and a town's works as things the host lays (`land.rs`): a surveyed
//! hex's ground is the map's, a town's fields are the crops counted in the hexes it holds, and what its masons finish
//! and the fields it plants are asked of the host at every look that surveys it until the host says they're laid.

use enterprise_core::wire::{self, Answer, Command, Count, HexAt, Input, LandKind, Output, Query, Refusal, Request, Response, Work};
use enterprise_core::works::{Kind, What};
use enterprise_core::{Content, Good, Hex, Society};

const DAY: u64 = 24_000;
const WHEAT: u32 = 0;
const CARROT: u32 = 1;
const BREAD: usize = 7;

/// The mod's crops at their locker prices, paper and bread (as `tests/farms.rs` has them).
fn content() -> Content {
    Content::new(vec![
        Good::new("wheat", 1.0 / 20.0, 64).crop(),
        Good::new("carrot", 1.0 / 22.0, 64).crop().food(3.0),
        Good::new("potato", 1.0 / 26.0, 64).crop().food(1.0),
        Good::new("beetroot", 1.0 / 15.0, 64).crop().food(1.0),
        Good::new("sugar_cane", 1.0 / 24.0, 64).crop(),
        Good::new("sweet_berries", 1.0 / 10.0, 64).crop().food(2.0),
        Good::new("paper", 1.0 / 24.0, 64),
        Good::new("bread", 1.0 / 6.0, 64).food(5.0),
    ], DAY)
}

fn at(q: i32, r: i32) -> HexAt {
    HexAt { q, r }
}

fn wheat(n: f64) -> Count {
    Count { good: WHEAT, count: n }
}

/// A society with one town standing at the block (8, 8) (in hex (0, 0)), `money` in its treasury.
fn town(money: f64) -> Option<Society> {
    let mut s = Society::new(content());
    s.content_mut().dials.villagers_per_want = f64::INFINITY;
    let t = s.add_town("Meadowworth", money);
    s.place_town(t, 8.0, 8.0);
    s.set_people(t, 1.0, 1.0, 1.0);
    Some(s)
}

fn apply(host: &mut Option<Society>, inputs: Vec<Input>) -> Vec<Output> {
    match wire::handle(host, Request::new(Command::Apply { inputs })) {
        Response::Done { outputs, .. } => outputs,
        other => panic!("the look was refused: {other:?}"),
    }
}

fn ask(host: &mut Option<Society>, query: Query) -> Answer {
    match wire::handle(host, Request::new(Command::Ask { queries: vec![query] })) {
        Response::Done { mut answers, .. } => answers.remove(0),
        other => panic!("the question was refused: {other:?}"),
    }
}

fn survey() -> Input {
    Input::Survey { town: 0, bell_x: 8, bell_z: 4 }
}

fn builds(outputs: &[Output]) -> Vec<(Work, f64)> {
    outputs.iter().filter_map(|o| match o {
        Output::Build { town: 0, work, share } => Some((work.clone(), *share)),
        _ => None,
    }).collect()
}

fn plots(host: &mut Option<Society>) -> Vec<Count> {
    match ask(host, Query::Land { town: 0 }) {
        Answer::Land { plots, .. } => plots,
        other => panic!("{other:?}"),
    }
}

// The survey of the chunks is Minecraft's (TollTests.surveysSeeWater, in parity-skip.txt): here its numbers reach the map.
#[test]
fn a_surveyed_hexs_ground_is_the_maps() {
    let mut host = town(0.0);
    apply(&mut host, vec![
        Input::Ground { hex: at(1, 0), water: 0.75, relief: 3.0, land: LandKind::Water, height: None, river: false },
        Input::Ground { hex: at(0, 1), water: 0.0, relief: 40.0, land: LandKind::Hills, height: None, river: false },
        Input::Ground { hex: at(-1, 0), water: 2.0, relief: 0.0, land: LandKind::Woods, height: None, river: false }, // clamped to all water
    ]);
    let s = host.as_ref().unwrap();
    assert_eq!(s.map().crossing_factor(Hex::new(1, 0)), f64::INFINITY, "a hex mostly under water can't be crossed");
    assert_eq!(s.map().crossing_factor(Hex::new(0, 1)), f64::INFINITY, "a cliff can't be crossed");
    assert_eq!(s.map().land(Hex::new(-1, 0)), enterprise_core::Land::Woods);
    let Answer::Ground { ground } = ask(&mut host, Query::Ground { hexes: vec![at(1, 0), at(-1, 0), at(5, 5)] }) else { panic!() };
    assert_eq!((ground[0].water, ground[0].relief, ground[0].land, ground[0].bridged), (0.75, 3.0, LandKind::Water, false));
    assert_eq!(ground[1].water, 1.0);
    assert_eq!((ground[2].water, ground[2].relief, ground[2].land), (0.0, 0.0, LandKind::Plains), "never surveyed: dry, flat plains");
    // A bridge the core finished stays when the hex is surveyed again.
    host.as_mut().unwrap().bridge(Hex::new(1, 0));
    apply(&mut host, vec![Input::Ground { hex: at(1, 0), water: 0.8, relief: 3.0, land: LandKind::Water, height: None, river: false }]);
    assert_eq!(host.as_ref().unwrap().map().crossing_factor(Hex::new(1, 0)), 1.0, "a bridged hex that's mostly water is open ground");
}

#[test]
fn ground_and_crops_that_arent_numbers_are_refused() {
    let mut host = town(0.0);
    for bad in [
        Input::Ground { hex: at(0, 0), water: f64::NAN, relief: 0.0, land: LandKind::Plains, height: None, river: false },
        Input::Ground { hex: at(0, 0), water: 0.0, relief: -1.0, land: LandKind::Plains, height: None, river: false },
        Input::Ground { hex: at(0, 0), water: 0.0, relief: 0.0, land: LandKind::Plains, height: Some(f64::INFINITY), river: false },
        Input::Crops { hex: at(0, 0), chunk_x: 0, chunk_z: 0, counts: vec![Count { good: 99, count: 1.0 }] },
        Input::Crops { hex: at(0, 0), chunk_x: 0, chunk_z: 0, counts: vec![wheat(-1.0)] },
        Input::Survey { town: 3, bell_x: 0, bell_z: 0 },
        Input::Built { town: 0, work: Work::Hall { kind: "palace".into() }, share: 1.0 },
        Input::Built { town: 0, work: Work::Field { crop: 99 }, share: 1.0 },
        Input::Built { town: 0, work: Work::Wall, share: f64::INFINITY },
    ] {
        let r = wire::handle(&mut host, Request::new(Command::Apply { inputs: vec![bad.clone()] }));
        assert!(matches!(r, Response::Refused { why: Refusal::Input { at: 0, .. } }), "{bad:?}: {r:?}");
    }
    // A work the town hasn't is a stale view: ignored.
    apply(&mut host, vec![Input::Built { town: 0, work: Work::Hall { kind: "temple".into() }, share: 1.0 },
        Input::Built { town: 0, work: Work::Field { crop: CARROT }, share: 1.0 }]);
}

// parity-partial: TerritoryTests.farmersProduceWhatGrowsInTheirHex (counting the crop blocks in a chunk is Minecraft's: here the counts
// the host sends are the farm's plots, and the Farmer works them)
#[test]
fn a_towns_fields_are_the_crops_counted_in_the_hexes_it_holds() {
    let mut host = town(0.0);
    host.as_mut().unwrap().set_roster(0, &[("farmer", 1)]);
    apply(&mut host, vec![
        Input::Crops { hex: at(0, 0), chunk_x: 0, chunk_z: 0, counts: vec![wheat(30.0)] },
        Input::Crops { hex: at(0, 0), chunk_x: 1, chunk_z: 0, counts: vec![wheat(10.0), Count { good: CARROT, count: 5.0 }] },
        Input::Crops { hex: at(3, 3), chunk_x: 20, chunk_z: 20, counts: vec![wheat(500.0)] }, // nobody's land
    ]);
    assert!(host.as_ref().unwrap().farm(0).is_none(), "no farm before the town's land is surveyed");
    apply(&mut host, vec![survey()]);
    // Before its first count the town's home hex is its; nothing of the hex nobody holds.
    assert_eq!(plots(&mut host), vec![wheat(40.0), Count { good: CARROT, count: 5.0 }]);
    let s = host.as_ref().unwrap();
    let farm = s.farm(0).expect("the survey set the farm up").farm;
    let worked: f64 = farm.per_day.values().sum();
    assert_eq!(worked, 32.0 * 0.5, "one level-1 Farmer works 32 of the 45 plots, half an item each");
    // A chunk counted again replaces its count; one with nothing left is gone; a chunk not counted keeps its last.
    apply(&mut host, vec![
        Input::Crops { hex: at(0, 0), chunk_x: 1, chunk_z: 0, counts: vec![] },
        Input::Crops { hex: at(0, 0), chunk_x: 0, chunk_z: 0, counts: vec![wheat(12.0)] },
        survey(),
    ]);
    assert_eq!(plots(&mut host), vec![wheat(12.0)]);
    // Once the town has counted and claimed its hexes, it's their crops that count.
    apply(&mut host, vec![Input::Advance { to: DAY }, survey()]);
    let Answer::Land { hexes, .. } = ask(&mut host, Query::Land { town: 0 }) else { panic!() };
    assert_eq!(hexes, vec![at(0, 0)]);
    assert_eq!(plots(&mut host), vec![wheat(12.0)]);
}

#[test]
fn a_finished_hall_is_asked_of_the_host_until_it_says_its_laid() {
    let mut host = town(0.0);
    let tower = What::Hall(Kind::Watchtower);
    host.as_mut().unwrap().start_project(0, tower);
    assert!(builds(&apply(&mut host, vec![survey()])).is_empty(), "nothing to lay while the masons work");
    host.as_mut().unwrap().finish_project(0, tower);
    let hall = Work::Hall { kind: "watchtower".into() };
    assert!(builds(&apply(&mut host, vec![])).is_empty(), "a town not surveyed in the look is shown nothing");
    assert_eq!(builds(&apply(&mut host, vec![survey()])), vec![(hall.clone(), 1.0)]);
    assert_eq!(builds(&apply(&mut host, vec![survey(), survey()])), vec![(hall.clone(), 1.0)], "asked again, once a look");
    assert!(builds(&apply(&mut host, vec![Input::Built { town: 0, work: hall.clone(), share: 1.0 }, survey()])).is_empty(),
        "laid: not asked again");
    let Answer::Land { works, .. } = ask(&mut host, Query::Land { town: 0 }) else { panic!() };
    assert_eq!((works[0].work.clone(), works[0].laid, works[0].done, works[0].shown), (hall, 1.0, true, 1.0));
}

#[test]
fn a_wall_is_shown_a_quarter_at_a_time_and_a_bridge_once_finished() {
    let mut host = town(0.0);
    let s = host.as_mut().unwrap();
    s.start_project(0, What::Wall);
    let blocks = s.works(0).projects[0].blocks_needed as f64;
    s.help_project(0, blocks * 0.3);
    assert_eq!(builds(&apply(&mut host, vec![survey()])), vec![(Work::Wall, 0.25)]);
    apply(&mut host, vec![Input::Built { town: 0, work: Work::Wall, share: 0.25 }]);
    assert!(builds(&apply(&mut host, vec![survey()])).is_empty(), "the first quarter stands");
    host.as_mut().unwrap().help_project(0, blocks * 0.25);
    assert_eq!(builds(&apply(&mut host, vec![survey()])), vec![(Work::Wall, 0.5)]);
    host.as_mut().unwrap().help_project(0, blocks);
    assert_eq!(builds(&apply(&mut host, vec![survey()])), vec![(Work::Wall, 1.0)], "the rest when it's done");
    // A host that laid less than it was asked is asked for the rest.
    apply(&mut host, vec![Input::Built { town: 0, work: Work::Wall, share: 0.75 }]);
    assert_eq!(builds(&apply(&mut host, vec![survey()])), vec![(Work::Wall, 1.0)]);

    let bridge = What::Bridge { hex: Hex::new(1, 0), from: Hex::new(0, 0), to: Hex::new(2, 0) };
    host.as_mut().unwrap().start_project(0, bridge);
    host.as_mut().unwrap().finish_project(0, bridge);
    let deck = Work::Bridge { hex: at(1, 0), from: at(0, 0), to: at(2, 0) };
    assert_eq!(builds(&apply(&mut host, vec![survey()])), vec![(Work::Wall, 1.0), (deck.clone(), 1.0)]);
    assert!(host.as_ref().unwrap().map().ground(Hex::new(1, 0)).bridged, "the finished bridge is the map's");
}

/// The town's rules plant a field (`Fields.consider`): a level-2 Farmer with no plots has hands to spare, and a town
/// short of food plants carrots. The field is the farm's at once and asked of the host until laid; once laid it's the
/// crops counted where the host put it, never twice.
// parity-partial: TerritoryTests.farmersProduceWhatGrowsInTheirHex (the field laid and counted again)
#[test]
fn a_field_the_town_plants_is_laid_by_the_host_and_counted_once() {
    let mut host = town(200.0);
    host.as_mut().unwrap().set_roster(0, &[("farmer", 2)]);
    apply(&mut host, vec![survey()]);
    let mut out = Vec::new();
    for day in 1..=4 {
        out = builds(&apply(&mut host, vec![Input::Advance { to: day * DAY }, survey()]));
        if !out.is_empty() {
            break;
        }
    }
    let field = Work::Field { crop: CARROT };
    assert_eq!(out, vec![(field.clone(), 1.0)], "a field of carrots to lay");
    let carrots = |host: &mut Option<Society>| plots(host).iter().find(|c| c.good == CARROT).map_or(0.0, |c| c.count);
    assert_eq!(carrots(&mut host), 80.0, "the field's 80 plots are the farm's before the host lays it");
    // Not laid yet (its ground not loaded): still the farm's, asked again.
    assert_eq!(builds(&apply(&mut host, vec![survey()])), vec![(field.clone(), 1.0)]);
    assert_eq!(carrots(&mut host), 80.0);
    // Laid, and its chunks counted in the same look: the plots are the counted ones, not those and the field's.
    apply(&mut host, vec![
        Input::Crops { hex: at(0, 0), chunk_x: 2, chunk_z: 0, counts: vec![Count { good: CARROT, count: 80.0 }] },
        Input::Built { town: 0, work: field.clone(), share: 1.0 },
        survey(),
    ]);
    assert_eq!(carrots(&mut host), 80.0);
    let Answer::Land { works, .. } = ask(&mut host, Query::Land { town: 0 }) else { panic!() };
    assert!(works.iter().all(|w| w.work != field), "the field laid is no work of the town's now: {works:?}");
}

#[test]
fn the_land_book_is_saved() {
    let mut host = town(0.0);
    let s = host.as_mut().unwrap();
    s.start_project(0, What::Wall);
    s.finish_project(0, What::Wall);
    apply(&mut host, vec![
        Input::Crops { hex: at(0, 0), chunk_x: 0, chunk_z: 0, counts: vec![wheat(30.0)] },
        Input::Built { town: 0, work: Work::Wall, share: 0.5 },
    ]);
    let blob = host.as_mut().unwrap().save();
    let mut loaded = Some(Society::load(&blob).expect("the save loads"));
    assert_eq!(builds(&apply(&mut loaded, vec![survey()])), vec![(Work::Wall, 1.0)]);
    assert_eq!(plots(&mut loaded), vec![wheat(30.0)]);
    assert!(loaded.as_ref().unwrap().unsaved().is_empty());
    let _ = BREAD;
}
