//! The wire (`wire/`): a host's one way in. A society created from a content pack, its desks' stocktakes and
//! deliveries booked, its stock and points asked; saved and loaded through `handle`, running on as if never saved;
//! every refusal a message with nothing applied; and postcard bytes in and out as `handle` does. That no wire type
//! holds anything but numbers, strings, ids, vectors and other wire types is `wire-gen`'s to check, by the registry
//! it traces (`sim/wire-gen/tests/wire_gen.rs`).

mod common;

use common::{world, DAY};
use enterprise_core::wire::*;
use enterprise_core::{Content, Dials, Fixed, Good, Society};

fn pack() -> ContentPack {
    let mut wheat = PackGood::new("minecraft:wheat", 0.05, 64);
    wheat.flags = vec![GoodFlag::Crop];
    let mut bread = PackGood::new("minecraft:bread", 0.2, 64);
    bread.food = 5.0;
    bread.saturation = 6.0;
    let mut book = PackGood::new("minecraft:book", 0.5, 64);
    book.comfort = Some("reading".into());
    let mut beef = PackGood::new("minecraft:cooked_beef", 0.3, 64);
    beef.food = 8.0;
    beef.saturation = 12.8;
    beef.flags = vec![GoodFlag::Meat];
    ContentPack::new(DAY, vec![wheat, bread, book, beef])
}

const WHEAT: GoodNo = 0;
const BREAD: GoodNo = 1;
const BOOK: GoodNo = 2;

fn desk(x: i32, z: i32) -> Point {
    Point { kind: PointKind::Desk, dim: "minecraft:overworld".into(), x, y: -60, z }
}

fn at(kind: PointKind, x: i32, z: i32) -> Point {
    Point { kind, ..desk(x, z) }
}

fn call(s: &mut Option<Society>, command: Command) -> Response {
    handle(s, Request::new(command))
}

fn created() -> Option<Society> {
    let mut s = None;
    assert_eq!(call(&mut s, Command::Create { pack: pack(), seed: 7 }), Response::Done { outputs: vec![], answers: vec![] });
    s
}

fn apply(s: &mut Option<Society>, inputs: Vec<Input>) -> Vec<Output> {
    match call(s, Command::Apply { inputs }) {
        Response::Done { outputs, answers } if answers.is_empty() => outputs,
        r => panic!("applied: {r:?}"),
    }
}

fn ask(s: &mut Option<Society>, queries: Vec<Query>) -> Vec<Answer> {
    match call(s, Command::Ask { queries }) {
        Response::Done { outputs, answers } if outputs.is_empty() => answers,
        r => panic!("asked: {r:?}"),
    }
}

fn stock(s: &mut Option<Society>, town: TownNo) -> Vec<f64> {
    match &ask(s, vec![Query::Stock { town }])[..] {
        [Answer::Stock { town: t, stock }] if *t == town => stock.clone(),
        a => panic!("stock: {a:?}"),
    }
}

fn refusal(r: Response) -> Refusal {
    match r {
        Response::Refused { why } => why,
        r => panic!("not refused: {r:?}"),
    }
}

fn counts(of: &[(GoodNo, f64)]) -> Vec<Count> {
    of.iter().map(|&(good, count)| Count { good, count }).collect()
}

#[test]
fn a_society_from_a_pack_takes_a_delivery_and_says_its_stock() {
    let mut s = created();
    let out = apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) },
        Input::Deliver { town: 0, good: WHEAT, count: 40.0 },
        Input::Deliver { town: 0, good: BOOK, count: 3.0 },
    ]);
    assert!(out.is_empty());
    assert_eq!(stock(&mut s, 0), vec![40.0, 0.0, 3.0, 0.0]);
    assert_eq!(ask(&mut s, vec![Query::Points]), vec![Answer::Points { points: vec![TownPoint { point: desk(27, 27), town: 0 }] }]);
    let society = s.as_ref().unwrap();
    assert_eq!(society.town(0).name, "Ashford");
    assert_eq!(society.town_hex(0), Some(enterprise_core::Hex::at(27.0, 27.0)), "the town stands at its desk");
    assert_eq!(society.town(0).money, Fixed::ZERO, "no money until it's paid some");
}

#[test]
fn a_pack_makes_the_content_the_core_would_for_its_goods() {
    let content = Content::from_pack(&pack()).unwrap();
    let goods = vec![
        Good::new("minecraft:wheat", 0.05, 64).crop(),
        Good::new("minecraft:bread", 0.2, 64).food(5.0).saturation(6.0),
        Good::new("minecraft:book", 0.5, 64).comfort(Content::COMFORTS.iter().position(|k| *k == "reading").unwrap()),
        Good::new("minecraft:cooked_beef", 0.3, 64).food(8.0).saturation(12.8).meat(),
    ];
    // (a pack that lists no recipes or animals brings none: the core's own are for hosts that bring no pack)
    let mut want = Content::new(goods, DAY);
    want.conversions.clear();
    want.animals.clear();
    assert_eq!(format!("{content:?}"), format!("{want:?}"));
}

/// A pack with all the game brings: weights, kinds of comfort, trades, recipes, animals and dials.
fn full_pack() -> ContentPack {
    let mut p = pack();
    let mut iron = PackGood::new("minecraft:iron_ingot", 0.25, 64);
    iron.weight = 4.0;
    let mut cane = PackGood::new("minecraft:sugar_cane", 0.04, 64);
    cane.weight = 0.5;
    let mut paper = PackGood::new("minecraft:paper", 0.04, 64);
    paper.comfort = Some("reading".into());
    p.goods.extend([iron, cane, paper]);
    p.comforts = vec!["clothing".into(), "reading".into()];
    p.trades = ["merchant", "farmer", "librarian", "shepherd", "none", "nitwit"].map(String::from).to_vec();
    p.recipes = vec![
        PackRecipe { trades: vec!["farmer".into()], inputs: vec![PackAmount { good: "minecraft:wheat".into(), count: 3.0 }],
            output: "minecraft:bread".into(), makes: 1.0 },
        PackRecipe { trades: vec!["librarian".into()], inputs: vec![PackAmount { good: "minecraft:sugar_cane".into(), count: 1.0 }],
            output: "minecraft:paper".into(), makes: 1.0 },
    ];
    p.animals = vec![PackAnimal { name: "cow".into(), feed: vec!["minecraft:wheat".into()], feed_per_day: 0.25,
        meat: Some("minecraft:cooked_beef".into()), meat_each: 2.0, wool: None, hide: None }];
    p.dials = vec![PackDial { name: "birth_mood".into(), value: 0.5 }, PackDial { name: "wagon_slots".into(), value: 30.0 }];
    p
}

#[test]
fn a_pack_brings_its_comforts_trades_recipes_animals_dials_and_weights() {
    let content = Content::from_pack(&full_pack()).unwrap();
    assert_eq!(content.comforts, vec!["clothing", "reading"]);
    assert_eq!(content.goods[2].comfort, Some(1), "the book is the pack's second kind of comfort");
    let names: Vec<&str> = content.trades.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["merchant", "farmer", "librarian", "shepherd", "none", "nitwit"]);
    assert!(content.trades[0].merchant && content.trades[0].leads && content.trades[0].squad);
    assert!(content.trades[4].none && content.trades[5].nitwit && !content.trades[1].leads);
    // The recipes are in the pack's order, by place: wheat (0) to bread (1); cane (5) to paper (6) by the librarian (2).
    assert_eq!(content.conversions.len(), 2);
    assert_eq!((content.conversions[0].trades.clone(), content.conversions[0].inputs.clone(), content.conversions[0].output,
        content.conversions[0].out), (vec![1], vec![(0, 3.0)], 1, 1.0));
    assert_eq!((content.conversions[1].trades.clone(), content.conversions[1].inputs.clone(), content.conversions[1].output),
        (vec![2], vec![(5, 1.0)], 6));
    assert_eq!(content.animals.len(), 1);
    assert_eq!((content.animals[0].feed.clone(), content.animals[0].meat, content.animals[0].wool), (vec![0], Some(3), None));
    // A dial the pack sets is the world's start; the rest are the core's.
    assert_eq!((content.dials.birth_mood, content.dials.wagon_slots), (0.5, 30));
    assert_eq!((content.dial_defaults.birth_mood, content.dial_defaults.shelf_days), (0.5, Dials::default().shelf_days));
    // A stack weighs what the pack says; a piece, its share of it; a plain good is the stack's 1.
    assert_eq!(content.goods[4].weight * 64.0, 4.0);
    assert_eq!(content.goods[5].weight * 64.0, 0.5);
    assert_eq!(content.goods[0].weight * 64.0, 1.0);
    // The villagers' trades are vanilla's for the pack's goods: a farmer sells bread, buys wheat.
    assert!(content.trades[1].offers.iter().any(|o| o.sells && o.good == 1), "the farmer sells bread");
}

#[test]
fn a_pack_names_goods_without_the_games_namespace_to_the_rules_that_look_for_bread_and_wheat() {
    let content = Content::from_pack(&pack()).unwrap();
    assert_eq!(content.good("wheat"), Some(0));
    assert_eq!(content.good("minecraft:bread"), Some(1));
    assert_eq!(content.good("bread"), Some(1));
    assert_eq!(content.good("heat"), None, "a suffix that isn't a name after a colon is no match");
    assert_eq!(content.good("minecraft:"), None);
}

#[test]
fn a_bad_recipe_animal_trade_comfort_or_dial_is_refused_with_all_of_them() {
    let mut bad = full_pack();
    bad.comforts.push("reading".into());
    bad.comforts.push(" ".into());
    bad.trades.retain(|t| t != "nitwit");
    bad.trades.push("minecraft:farmer".into());
    bad.trades.push("farmer".into());
    bad.goods[2].comfort = Some("tea".into());
    bad.goods[4].weight = 0.0;
    bad.goods[5].weight = f64::NAN;
    bad.recipes.push(PackRecipe { trades: vec![], inputs: vec![], output: "minecraft:glass".into(), makes: 0.0 });
    bad.recipes.push(PackRecipe { trades: vec!["baker".into()],
        inputs: vec![PackAmount { good: "minecraft:flour".into(), count: -1.0 }], output: "minecraft:bread".into(), makes: f64::INFINITY });
    bad.animals.push(PackAnimal { name: "cow".into(), feed: vec![], feed_per_day: -1.0, meat: Some("minecraft:veal".into()),
        meat_each: f64::NAN, wool: Some("minecraft:fleece".into()), hide: None });
    bad.animals.push(PackAnimal { name: "goat".into(), feed: vec!["minecraft:hay".into()], feed_per_day: 0.1, meat: None,
        meat_each: 0.0, wool: None, hide: Some("minecraft:leather".into()) });
    bad.dials.push(PackDial { name: "birth_mood".into(), value: 1.0 });
    bad.dials.push(PackDial { name: "no_such_dial".into(), value: 1.0 });
    bad.dials.push(PackDial { name: "max_rings".into(), value: -3.0 });
    bad.dials.push(PackDial { name: "shelf_days".into(), value: f64::NAN });
    let why = Content::from_pack(&bad).unwrap_err();
    for wrong in [
        "comfort 2 (\"reading\"): named as another is", "comfort 3: no name",
        "no trade called \"nitwit\", which the core needs", "trade 5 (\"minecraft:farmer\"): named with a namespace",
        "trade 6 (\"farmer\"): named as another is",
        "good 2 (\"minecraft:book\"): a comfort the pack doesn't list (\"tea\"",
        "good 4 (\"minecraft:iron_ingot\"): a stack weighing 0", "good 5 (\"minecraft:sugar_cane\"): a stack weighing NaN",
        "recipe 2 (\"minecraft:glass\"): no trade makes it", "recipe 2 (\"minecraft:glass\"): nothing goes in",
        "recipe 2 (\"minecraft:glass\"): no good called \"minecraft:glass\" to make",
        "recipe 3 (\"minecraft:bread\"): no trade called \"baker\"", "no good called \"minecraft:flour\" to put in",
        "-1 of \"minecraft:flour\" in", "makes inf",
        "animal 1 (\"cow\"): named as another is", "animal 1 (\"cow\"): eats nothing", "eats -1 a day", "gives NaN meat",
        "no good called \"minecraft:veal\" for its meat", "no good called \"minecraft:fleece\" for its wool",
        "animal 2 (\"goat\"): no good called \"minecraft:hay\" to feed it",
        "dial 2 (\"birth_mood\"): set twice", "dial 3 (\"no_such_dial\"): no dial called no_such_dial",
        "dial 4 (\"max_rings\"): -3 isn't a count", "dial 5 (\"shelf_days\"): NaN isn't a number"] {
        assert!(why.contains(wrong), "{wrong:?} in {why:?}");
    }
    // A pack that lists no Merchant, no `none` and no `nitwit` is told what the core needs.
    let mut bare = pack();
    bare.trades = vec!["farmer".into()];
    let why = Content::from_pack(&bare).unwrap_err();
    for needed in ["merchant", "none", "nitwit"] {
        assert!(why.contains(&format!("no trade called \"{needed}\"")), "{needed} in {why:?}");
    }
}

#[test]
fn a_dial_is_set_for_the_world_reset_to_the_packs_and_kept_by_a_save() {
    let mut s = None;
    assert!(matches!(call(&mut s, Command::Create { pack: full_pack(), seed: 7 }), Response::Done { .. }));
    let dial = |s: &mut Option<Society>, name: &str| match &ask(s, vec![Query::Dials])[..] {
        [Answer::Dials { dials }] => dials.iter().find(|d| d.name == name).cloned().unwrap_or_else(|| panic!("no dial {name}")),
        a => panic!("dials: {a:?}"),
    };
    // The pack's own setting is where the world starts; the dial says what it does when its doc comment does.
    let birth = dial(&mut s, "birth_mood");
    assert_eq!((birth.value, birth.initial), (0.5, 0.5));
    let shelf = dial(&mut s, "shelf_days");
    assert_eq!((shelf.value, shelf.initial), (5.0, 5.0));
    assert!(shelf.does.starts_with("Days of what its people use a town's warehouse aims to hold"), "{:?}", shelf.does);
    // A dial documented with its neighbours is told the group's words.
    let cost = dial(&mut s, "birth_cost");
    assert!(cost.does.starts_with("A child once a bed is free"), "{:?}", cost.does);
    // Every dial of the core is listed, once, in the order it declares them.
    let all = match &ask(&mut s, vec![Query::Dials])[..] {
        [Answer::Dials { dials }] => dials.iter().map(|d| d.name.clone()).collect::<Vec<_>>(),
        a => panic!("dials: {a:?}"),
    };
    assert_eq!(all, enterprise_core::Dials::names());

    // Set, a switch and a count rounded as the mod's are; a reset goes back to the pack's, not the core's.
    let out = apply(&mut s, vec![Input::SetDial { name: "birth_mood".into(), value: Some(0.9) },
        Input::SetDial { name: "wagon_slots".into(), value: Some(12.4) }, Input::SetDial { name: "bargain".into(), value: Some(0.0) }]);
    assert!(out.is_empty());
    assert_eq!((dial(&mut s, "birth_mood").value, dial(&mut s, "wagon_slots").value, dial(&mut s, "bargain").value), (0.9, 12.0, 0.0));
    assert_eq!(s.as_ref().unwrap().dial("birth_mood"), Some(0.9), "the rules read it");
    // A save keeps them with the world, and the pack's start with it.
    let Response::Saved { save } = call(&mut s, Command::Save) else { panic!("a save") };
    let mut loaded = None;
    assert!(matches!(call(&mut loaded, Command::Load { save }), Response::Done { .. }));
    assert_eq!((dial(&mut loaded, "birth_mood").value, dial(&mut loaded, "birth_mood").initial), (0.9, 0.5));
    assert_eq!(dial(&mut loaded, "wagon_slots").value, 12.0);
    apply(&mut loaded, vec![Input::SetDial { name: "birth_mood".into(), value: None }, Input::SetDial { name: "wagon_slots".into(), value: None }]);
    assert_eq!((dial(&mut loaded, "birth_mood").value, dial(&mut loaded, "wagon_slots").value), (0.5, 30.0), "the pack's 0.5 and 30");
}

#[test]
fn a_dial_that_isnt_there_or_wont_take_its_value_refuses_the_whole_look() {
    let mut s = created();
    let before = s.as_ref().unwrap().dial("shelf_days");
    for (name, value, why) in [("no_such_dial", Some(1.0), "no dial called no_such_dial"), ("no_such_dial", None, "no dial called no_such_dial"),
        ("shelf_days", Some(f64::NAN), "NaN isn't a number"), ("wagon_slots", Some(-1.0), "-1 isn't a count"),
        ("wagon_slots", Some(f64::INFINITY), "inf isn't a count")] {
        let r = refusal(call(&mut s, Command::Apply { inputs: vec![
            Input::SetDial { name: "shelf_days".into(), value: Some(9.0) }, Input::SetDial { name: name.into(), value }] }));
        match r {
            Refusal::Input { at: 1, why: said } => assert!(said.contains(why), "{said:?} for {name}"),
            r => panic!("{r:?} for {name}"),
        }
        assert_eq!(s.as_ref().unwrap().dial("shelf_days"), before, "nothing of a refused look is applied");
    }
}

#[test]
fn a_stocktake_books_what_the_chests_gained_and_lost_since_the_last() {
    let mut s = created();
    apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) },
        Input::AddPoint { point: at(PointKind::Locker, 30, 27), town: 0 },
    ]);
    let before = s.as_mut().unwrap().totals();
    // The first stocktake counts from nothing.
    apply(&mut s, vec![Input::Stocktake { point: desk(27, 27), counts: counts(&[(WHEAT, 30.0), (BREAD, 5.0)]) }]);
    assert_eq!(stock(&mut s, 0), vec![30.0, 5.0, 0.0, 0.0]);
    // A player took 20 wheat out and put 2 books in; bread is as it was.
    apply(&mut s, vec![Input::Stocktake { point: desk(27, 27), counts: counts(&[(WHEAT, 10.0), (BREAD, 5.0), (BOOK, 2.0)]) }]);
    assert_eq!(stock(&mut s, 0), vec![10.0, 5.0, 2.0, 0.0]);
    // A locker's chests are counted apart from the desk's, for the same town; a good listed twice is summed.
    apply(&mut s, vec![Input::Stocktake { point: at(PointKind::Locker, 30, 27), counts: counts(&[(BREAD, 1.0), (BREAD, 2.0)]) }]);
    assert_eq!(stock(&mut s, 0), vec![10.0, 8.0, 2.0, 0.0]);
    // The town has used 6 wheat the chests still show; then the chests are emptied: only what it holds is taken.
    assert_eq!(s.as_mut().unwrap().withdraw(0, WHEAT as usize, 6.0), 6.0);
    apply(&mut s, vec![Input::Stocktake { point: desk(27, 27), counts: counts(&[(BREAD, 5.0), (BOOK, 2.0)]) }]);
    assert_eq!(stock(&mut s, 0), vec![0.0, 8.0, 2.0, 0.0]);
    let society = s.as_mut().unwrap();
    society.conserved(&before).unwrap();
    society.solvent().unwrap();
    // A point broken and placed again counts from nothing again.
    apply(&mut s, vec![
        Input::RemovePoint { point: at(PointKind::Locker, 30, 27) },
        Input::AddPoint { point: at(PointKind::Locker, 30, 27), town: 0 },
        Input::Stocktake { point: at(PointKind::Locker, 30, 27), counts: counts(&[(BREAD, 3.0)]) },
    ]);
    assert_eq!(stock(&mut s, 0), vec![0.0, 11.0, 2.0, 0.0]);
    s.as_mut().unwrap().conserved(&before).unwrap();
}

#[test]
fn points_are_answered_in_their_places_order_with_their_towns() {
    let mut s = created();
    apply(&mut s, vec![
        Input::AddTown { name: "B".into(), desk: desk(500, 0) },
        Input::AddTown { name: "A".into(), desk: desk(-500, 0) },
        Input::AddPoint { point: at(PointKind::Banker, 502, 0), town: 0 },
        Input::AddPoint { point: Point { dim: "minecraft:the_nether".into(), ..at(PointKind::Depot, 0, 0) }, town: 1 },
    ]);
    let Answer::Points { points } = &ask(&mut s, vec![Query::Points])[0] else { panic!() };
    let seen: Vec<(PointKind, &str, i32, TownNo)> = points.iter().map(|p| (p.point.kind, p.point.dim.as_str(), p.point.x, p.town)).collect();
    assert_eq!(seen, vec![
        (PointKind::Desk, "minecraft:overworld", -500, 1),
        (PointKind::Desk, "minecraft:overworld", 500, 0),
        (PointKind::Banker, "minecraft:overworld", 502, 0),
        (PointKind::Depot, "minecraft:the_nether", 0, 1),
    ]);
}

/// A host that loads a save maps its items to good numbers from the society's own content, not from the pack it would
/// build today: the goods come back by name in the pack's order, before and after a save and load (`proof-in-mod`'s
/// `StockAdapter` reads them so).
#[test]
fn the_goods_are_named_in_their_numbers_order_and_kept_by_a_save() {
    let mut s = created();
    let names: Vec<String> = pack().goods.iter().map(|g| g.name.clone()).collect();
    assert_eq!(ask(&mut s, vec![Query::Goods]), vec![Answer::Goods { goods: names.clone() }]);
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) }]);
    let Response::Saved { save } = call(&mut s, Command::Save) else { panic!("saved") };
    let mut loaded = None;
    assert_eq!(call(&mut loaded, Command::Load { save }), Response::Done { outputs: vec![], answers: vec![] });
    assert_eq!(ask(&mut loaded, vec![Query::Goods, Query::Stock { town: 0 }]), vec![
        Answer::Goods { goods: names },
        Answer::Stock { town: 0, stock: vec![0.0; 4] },
    ]);
    assert_eq!(refusal(call(&mut None, Command::Ask { queries: vec![Query::Goods] })), Refusal::NoSociety);
}

/// A world saved and loaded through the wire runs on as the one kept, to the bit.
fn saved_and_loaded_runs_on_alike(mut kept: Option<Society>, days: std::ops::RangeInclusive<u64>) {
    let Response::Saved { save } = call(&mut kept, Command::Save) else { panic!("saved") };
    assert_eq!((save.format, save.schema, save.core.as_str()), (enterprise_core::save::FORMAT, SCHEMA, CORE));
    let mut loaded = None;
    assert_eq!(call(&mut loaded, Command::Load { save }), Response::Done { outputs: vec![], answers: vec![] });
    let n = kept.as_ref().unwrap().town_count();
    let queries: Vec<Query> = (0..n as TownNo).map(|town| Query::Stock { town }).chain([Query::Points]).collect();
    assert_eq!(ask(&mut loaded, queries.clone()), ask(&mut kept, queries.clone()));
    for day in days {
        let advance = vec![Input::Advance { to: day * DAY }];
        assert_eq!(apply(&mut loaded, advance.clone()), apply(&mut kept, advance), "day {day}: the same outputs");
        assert_eq!(ask(&mut loaded, queries.clone()), ask(&mut kept, queries.clone()), "day {day}: the same answers");
    }
    let (Response::Saved { save: a }, Response::Saved { save: b }) = (call(&mut kept, Command::Save), call(&mut loaded, Command::Save))
        else { panic!("saved") };
    assert!(a == b, "the same save at the end");
}

#[test]
fn a_society_saved_and_loaded_through_handle_runs_on_as_never_saved() {
    let mut s = created();
    apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) },
        Input::AddTown { name: "Bly".into(), desk: desk(600, 40) },
        Input::Pay { town: 0, money: Fixed::emeralds(40) },
        Input::Stocktake { point: desk(27, 27), counts: counts(&[(WHEAT, 64.0), (BREAD, 12.0)]) },
        Input::Advance { to: 3 * DAY },
    ]);
    saved_and_loaded_runs_on_alike(s, 4..=20);
    // And a whole random world, every slice's state in it, with wagons on the road.
    let mut w = world(77);
    w.advance(30 * DAY);
    saved_and_loaded_runs_on_alike(Some(w), 31..=60);
}

#[test]
fn a_request_for_a_society_before_there_is_one_is_refused() {
    let mut s = None;
    for command in [Command::Save, Command::Apply { inputs: vec![Input::Advance { to: DAY }] }, Command::Ask { queries: vec![Query::Points] }] {
        assert_eq!(refusal(call(&mut s, command)), Refusal::NoSociety);
    }
    assert!(s.is_none());
}

#[test]
fn a_request_built_against_another_abi_or_schema_is_refused() {
    let mut s = created();
    let command = Command::Ask { queries: vec![Query::Points] };
    assert_eq!(refusal(handle(&mut s, Request { abi: ABI + 1, schema: SCHEMA, command: command.clone() })),
        Refusal::Abi { core: ABI, request: ABI + 1 });
    assert_eq!(refusal(handle(&mut s, Request { abi: ABI, schema: SCHEMA ^ 1, command: command.clone() })),
        Refusal::Schema { core: SCHEMA, request: SCHEMA ^ 1 });
    // Before a society, too: the envelope is checked first.
    assert_eq!(refusal(handle(&mut None, Request { abi: ABI, schema: SCHEMA ^ 1, command })),
        Refusal::Schema { core: SCHEMA, request: SCHEMA ^ 1 });
}

#[test]
fn a_bad_pack_is_refused_with_everything_wrong_with_it_and_the_society_kept() {
    let mut bad = pack();
    bad.day_ticks = 0;
    bad.goods[1].name = "minecraft:wheat".into();
    bad.goods[2].worth = f64::NAN;
    bad.goods[2].comfort = Some("gambling".into());
    bad.goods[3].stack = 0;
    bad.goods[3].food = -1.0;
    bad.goods.push(PackGood::new(" ", 1.0, 1));
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) }]);
    let Refusal::Pack { why } = refusal(call(&mut s, Command::Create { pack: bad, seed: 1 })) else { panic!("a pack refusal") };
    for wrong in ["a day of no ticks", "good 1 (\"minecraft:wheat\"): named as another is", "good 2 (\"minecraft:book\"): worth NaN",
        "a comfort the pack doesn't list (\"gambling\"", "good 3 (\"minecraft:cooked_beef\"): a stack of none", "food points -1",
        "good 4 (\" \"): no name"] {
        assert!(why.contains(wrong), "{wrong:?} in {why:?}");
    }
    assert_eq!(s.as_ref().unwrap().town_count(), 1, "the society there is kept");
    let empty = ContentPack::new(DAY, vec![]);
    assert_eq!(refusal(call(&mut None, Command::Create { pack: empty, seed: 1 })), Refusal::Pack { why: "no goods".into() });
}

#[test]
fn a_save_of_another_format_or_none_at_all_is_refused_and_the_society_kept() {
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) }]);
    let Response::Saved { save } = call(&mut s, Command::Save) else { panic!("saved") };
    let newer = SaveFile { format: enterprise_core::save::FORMAT + 1, ..save.clone() };
    assert_eq!(refusal(call(&mut s, Command::Load { save: newer })),
        Refusal::SaveFormat { core: enterprise_core::save::FORMAT, save: enterprise_core::save::FORMAT + 1 });
    let junk = SaveFile { blob: b"not a save".to_vec(), ..save.clone() };
    assert!(matches!(refusal(call(&mut s, Command::Load { save: junk })), Refusal::Save { .. }));
    let cut = SaveFile { blob: save.blob[..save.blob.len() / 2].to_vec(), ..save };
    assert!(matches!(refusal(call(&mut s, Command::Load { save: cut })), Refusal::Save { .. }));
    assert_eq!(s.as_ref().unwrap().town_count(), 1, "the society there is kept");
}

#[test]
fn a_look_naming_what_isnt_there_is_refused_whole() {
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) }]);
    let refused = |s: &mut Option<Society>, inputs: Vec<Input>| refusal(call(s, Command::Apply { inputs }));
    let deliver = Input::Deliver { town: 0, good: WHEAT, count: 5.0 };
    let cases: Vec<(Input, &str)> = vec![
        (Input::Deliver { town: 1, good: WHEAT, count: 1.0 }, "no town 1"),
        (Input::Deliver { town: 0, good: 4, count: 1.0 }, "no good 4"),
        (Input::Deliver { town: 0, good: WHEAT, count: f64::NAN }, "a count of NaN"),
        (Input::Buy { town: 0, good: WHEAT, count: -1.0 }, "a count of -1"),
        (Input::Pay { town: 0, money: Fixed::from_millis(-1) }, "a payment of"),
        (Input::Order { town: 0, good: BREAD, lot: 1.0, price: f64::INFINITY, purse: 0.0 }, "a price of inf"),
        (Input::OwnShop { town: 9 }, "no town 9"),
        (Input::AddTown { name: "Bly".into(), desk: at(PointKind::Locker, 0, 0) }, "a town's desk that's a Locker"),
        (Input::AddTown { name: "Bly".into(), desk: desk(27, 27) }, "a point already stands"),
        (Input::AddPoint { point: Point { kind: PointKind::Shop, ..desk(27, 27) }, town: 0 }, "a point already stands"),
        (Input::AddPoint { point: at(PointKind::Shop, 1, 1), town: 3 }, "no town 3"),
        (Input::RemovePoint { point: desk(1, 1) }, "no point at"),
        (Input::Stocktake { point: desk(1, 1), counts: vec![] }, "no point at"),
        (Input::Stocktake { point: desk(27, 27), counts: counts(&[(7, 1.0)]) }, "no good 7"),
        (Input::RemovePoint { point: at(PointKind::Locker, 27, 27) }, "a Locker at"),
        (Input::Stocktake { point: at(PointKind::Shop, 27, 27), counts: vec![] }, "where a Desk stands"),
        (Input::Advance { to: u64::MAX }, "more than 36500 days"),
    ];
    for (input, why) in cases {
        // A good delivery first: refused with the rest, so nothing of the look is applied.
        match refused(&mut s, vec![deliver.clone(), input.clone()]) {
            Refusal::Input { at: 1, why: w } if w.contains(why) => {}
            r => panic!("{input:?}: {r:?}, not {why:?}"),
        }
    }
    assert_eq!(stock(&mut s, 0), vec![0.0; 4], "nothing applied");
    // Checked in order, as applied: a town added is there for what follows it, a point removed isn't.
    apply(&mut s, vec![
        Input::AddTown { name: "Bly".into(), desk: desk(600, 0) },
        Input::Deliver { town: 1, good: BREAD, count: 2.0 },
        Input::AddPoint { point: at(PointKind::Depot, 600, 5), town: 1 },
        Input::Stocktake { point: at(PointKind::Depot, 600, 5), counts: counts(&[(BREAD, 1.0)]) },
    ]);
    assert_eq!(stock(&mut s, 1), vec![0.0, 3.0, 0.0, 0.0]);
    assert!(matches!(refused(&mut s, vec![
        Input::RemovePoint { point: at(PointKind::Depot, 600, 5) },
        Input::Stocktake { point: at(PointKind::Depot, 600, 5), counts: vec![] },
    ]), Refusal::Input { at: 1, .. }));
    assert!(matches!(refused(&mut s, vec![
        Input::AddTown { name: "Cray".into(), desk: desk(-600, 0) },
        Input::AddTown { name: "Dun".into(), desk: desk(-600, 0) },
    ]), Refusal::Input { at: 1, .. }));
    assert_eq!(s.as_ref().unwrap().town_count(), 2);
}

#[test]
fn a_question_about_what_isnt_there_is_refused() {
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) }]);
    assert_eq!(refusal(call(&mut s, Command::Ask { queries: vec![Query::Points, Query::Stock { town: 1 }] })),
        Refusal::Query { at: 1, why: "no town 1".into() });
}

#[test]
fn bytes_in_and_out_are_handle_in_postcard() {
    let (mut a, mut b) = (None, None);
    let requests = vec![
        Request::new(Command::Create { pack: pack(), seed: 3 }),
        Request::new(Command::Apply { inputs: vec![
            Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) },
            Input::Stocktake { point: desk(27, 27), counts: counts(&[(WHEAT, 9.0)]) },
            Input::Advance { to: 2 * DAY },
        ] }),
        Request::new(Command::Ask { queries: vec![Query::Stock { town: 0 }, Query::Points] }),
        Request::new(Command::Ask { queries: vec![Query::Stock { town: 5 }] }),
        Request::new(Command::Save),
    ];
    for request in requests {
        let bytes = handle_bytes(&mut a, &postcard::to_allocvec(&request).unwrap());
        let response: Response = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(response, handle(&mut b, request));
    }
    let garbled: Response = postcard::from_bytes(&handle_bytes(&mut a, &[0xff, 0xff, 0xff])).unwrap();
    assert!(matches!(garbled, Response::Refused { why: Refusal::Unreadable { .. } }), "{garbled:?}");
    let truncated = &postcard::to_allocvec(&Request::new(Command::Save)).unwrap()[..2];
    let garbled: Response = postcard::from_bytes(&handle_bytes(&mut a, truncated)).unwrap();
    assert!(matches!(garbled, Response::Refused { why: Refusal::Unreadable { .. } }), "{garbled:?}");
}

#[test]
fn money_crosses_as_its_thousandths_in_an_i64() {
    for millis in [0, 1, -1, 1500, -1500, i64::MAX, i64::MIN] {
        assert_eq!(postcard::to_allocvec(&Fixed::from_millis(millis)).unwrap(), postcard::to_allocvec(&millis).unwrap());
    }
}

#[test]
fn the_host_names_the_same_messages_from_messages() {
    let input: enterprise_core::messages::Input = Input::Advance { to: 1 };
    let _: enterprise_core::Input = input;
    let output: enterprise_core::messages::Output = Output::WagonHome { route: 0, takings: Fixed::ZERO };
    let _: enterprise_core::Output = output;
}

#[test]
fn a_requests_own_bytes_are_what_handle_bytes_reads() {
    // `Request::to_bytes` is how a host in Rust writes requests for another to play (the study host's `--wire-looks`).
    let mut s = None;
    let create = Request::new(Command::Create { pack: pack(), seed: 7 });
    assert_eq!(create.to_bytes(), postcard::to_allocvec(&create).unwrap());
    let done: Response = postcard::from_bytes(&handle_bytes(&mut s, &create.to_bytes())).unwrap();
    assert_eq!(done, Response::Done { outputs: vec![], answers: vec![] });
    let ask = Request::new(Command::Ask { queries: vec![Query::Points] });
    let answered: Response = postcard::from_bytes(&handle_bytes(&mut s, &ask.to_bytes())).unwrap();
    assert_eq!(answered, Response::Done { outputs: vec![], answers: vec![Answer::Points { points: vec![] }] });
}
