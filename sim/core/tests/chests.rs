//! Chests stay physical (`chests.rs`, task `stock-adapter`): a town's stock shown in the desk's and depots' chests
//! counted in a look, as `Moved` at the end of it; what the host couldn't move back as `Unmoved`; a chest always counted
//! before it's written; a point broken taking its chests' goods with it, and a desk put back being its town again. A
//! host here is a few chests of so many slots (`Chests`), written as a host would.

use crate::common;

use common::DAY;
use enterprise_core::points::{Kind, Spot};
use enterprise_core::wire::*;
use enterprise_core::{Content, Good, Society};

const WHEAT: GoodNo = 0;
const BREAD: GoodNo = 1;

fn pack() -> ContentPack {
    let wheat = PackGood::new("minecraft:wheat", 0.05, 64);
    let mut bread = PackGood::new("minecraft:bread", 0.2, 64);
    bread.food = 5.0;
    bread.saturation = 6.0;
    ContentPack::new(DAY, vec![wheat, bread])
}

fn at(kind: PointKind, x: i32) -> Point {
    Point { kind, dim: "minecraft:overworld".into(), x, y: -60, z: 0 }
}

fn desk(x: i32) -> Point {
    at(PointKind::Desk, x)
}

fn spot(p: &Point) -> Spot {
    Spot { dim: p.dim.clone(), x: p.x, y: p.y, z: p.z }
}

/// A host's chests at a point: so many slots, the goods in them (stacks of 64, each good in as few as it fits), and
/// slots taken by things that aren't goods (dirt, a hopper's doing).
#[derive(Clone, Debug)]
struct Chests {
    slots: u32,
    goods: Vec<i64>,
    junk: u32,
}

impl Chests {
    fn new(slots: u32, goods: &[(GoodNo, i64)]) -> Chests {
        let mut c = Chests { slots, goods: vec![0; 2], junk: 0 };
        for &(g, n) in goods {
            c.goods[g as usize] += n;
        }
        c
    }

    fn used(&self) -> u32 {
        self.goods.iter().map(|&n| (n as u32).div_ceil(64)).sum::<u32>() + self.junk
    }

    fn free(&self) -> u32 {
        self.slots.saturating_sub(self.used())
    }

    fn stocktake(&self, point: &Point) -> Input {
        let counts = self.goods.iter().enumerate().filter(|(_, n)| **n > 0)
            .map(|(g, &n)| Count { good: g as GoodNo, count: n as f64 }).collect();
        Input::Stocktake { point: point.clone(), counts, free: self.free() }
    }

    /// A move written as the mod's adapter writes it: as much as fits in (merging into the good's partial stack, then
    /// free slots), as much as is there out. What it couldn't do is returned.
    fn write(&mut self, good: GoodNo, count: i64) -> i64 {
        let g = good as usize;
        if count > 0 {
            let partial = self.goods[g] % 64;
            let room = if partial > 0 { 64 - partial } else { 0 } + self.free() as i64 * 64;
            let put = count.min(room);
            self.goods[g] += put;
            count - put
        } else {
            let take = (-count).min(self.goods[g]);
            self.goods[g] -= take;
            count + take
        }
    }
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
        Response::Done { outputs, .. } => outputs,
        r => panic!("applied: {r:?}"),
    }
}

fn moved(outputs: &[Output]) -> Vec<(i32, GoodNo, i64)> {
    outputs.iter().filter_map(|o| match o {
        Output::Moved { point, good, count } => Some((point.x, *good, *count)),
        _ => None,
    }).collect()
}

fn stock(s: &Option<Society>, town: usize) -> Vec<f64> {
    s.as_ref().unwrap().town(town).stock
}

fn held(s: &Option<Society>, p: &Point) -> Vec<f64> {
    let mut h = s.as_ref().unwrap().chests(&spot(p)).expect("a point").held;
    h.resize(2, 0.0);
    h
}

/// A look as the mod's host runs one: what it couldn't move last time, a stocktake at each of `points`, an advance;
/// then the moves written to the chests, and what couldn't be done kept for the next look.
fn look(s: &mut Option<Society>, chests: &mut [(Point, Chests)], unmoved: &mut Vec<Input>, to: Option<u64>) -> Vec<Output> {
    let mut inputs: Vec<Input> = std::mem::take(unmoved);
    inputs.extend(chests.iter().map(|(p, c)| c.stocktake(p)));
    inputs.extend(to.map(|to| Input::Advance { to }));
    let outputs = apply(s, inputs);
    for o in &outputs {
        if let Output::Moved { point, good, count } = o {
            let (_, c) = chests.iter_mut().find(|(p, _)| p == point).expect("a move at a point the host counted");
            let left = c.write(*good, *count);
            if left != 0 {
                unmoved.push(Input::Unmoved { point: point.clone(), good: *good, count: left });
            }
        }
    }
    outputs
}

#[test]
fn what_the_town_gains_outside_its_chests_is_put_in_them_after_their_stocktake() {
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(0) }]);
    let mut chests = vec![(desk(0), Chests::new(27, &[(WHEAT, 30)]))];
    let mut unmoved = Vec::new();
    // The first stocktake counts the chests in: nothing moves.
    assert!(moved(&look(&mut s, &mut chests, &mut unmoved, None)).is_empty());
    // A delivery handed to the town, not through its chests (a pallet received, a shop's sale): shown there at the
    // desk's next stocktake, in the same look.
    apply(&mut s, vec![Input::Deliver { town: 0, good: BREAD, count: 40.0 }]);
    assert_eq!(chests[0].1.goods, vec![30, 0], "nothing is written to a chest the look didn't count");
    assert_eq!(moved(&look(&mut s, &mut chests, &mut unmoved, None)), vec![(0, BREAD, 40)]);
    assert_eq!(chests[0].1.goods, vec![30, 40]);
    assert_eq!(held(&s, &desk(0)), vec![30.0, 40.0], "the core takes the chests to hold what it moved");
    // And the next stocktake, the chests as written, books nothing and moves nothing.
    let before = stock(&s, 0);
    assert!(moved(&look(&mut s, &mut chests, &mut unmoved, None)).is_empty());
    assert_eq!(stock(&s, 0), before);
    assert_eq!(before, vec![30.0, 40.0]);
}

/// A master Merchant stocks the town's own shop with its biggest surplus (`orders.rs`): the stack that goes to the
/// shelf comes out of the desk's chests, at the end of the look that moved it.
#[test]
fn what_the_core_takes_from_the_towns_stock_comes_out_of_its_chests() {
    let mut s = created();
    apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk(0) },
        Input::AddHand { town: 0, trade: "merchant".into(), level: 5, tag: "m".into() },
        Input::OwnShop { town: 0 },
    ]);
    let mut chests = vec![(desk(0), Chests::new(27, &[(WHEAT, 300), (BREAD, 10)]))];
    let mut unmoved = Vec::new();
    look(&mut s, &mut chests, &mut unmoved, None);
    let out = look(&mut s, &mut chests, &mut unmoved, Some(DAY));
    assert_eq!(moved(&out), vec![(0, WHEAT, -64)], "the shelf's stack out of the chests: {out:?}");
    assert_eq!(chests[0].1.goods, vec![236, 10]);
    assert_eq!(stock(&s, 0), vec![236.0, 10.0]);
    assert!(unmoved.is_empty());
}

/// A wagon's sale through the wire (the farm's wheat to the mill, a day's road): each day's look shows each town's
/// stock in its desk's chests, the farm's harvest less the wagon's load in and out of the farm's, the sale less the
/// mill's people's use in and out of the mill's; the chests hold the towns' whole items every day, and nothing is made
/// or lost.
#[test]
fn a_wagons_load_and_sale_are_moved_out_of_and_into_the_desks_chests() {
    let content = Content::new(vec![Good::new("wheat", 0.05, 64), Good::new("bread", 0.2, 64)], DAY);
    let mut society = Society::new(content);
    society.content_mut().dials.standing_orders = false;
    let farm = society.add_town("Wheatfield", 20.0);
    let mill = society.add_town("Hayford", 30.0);
    society.set_trade(farm, 0, 128.0, 0.0);
    society.set_trade(mill, 0, 0.0, 64.0);
    society.add_route(farm, mill, 1.0, 0.5, 1.0);
    society.add_point(Kind::Desk, spot(&desk(0)), farm).unwrap();
    society.add_point(Kind::Desk, spot(&desk(600)), mill).unwrap();
    let start = society.totals();
    let mut s = Some(society);
    let mut chests = vec![(desk(0), Chests::new(54, &[])), (desk(600), Chests::new(54, &[]))];
    let mut unmoved = Vec::new();
    let (mut sold, mut into_mill, mut out_of_farm) = (0.0, 0, 0);
    for day in 1..=20 {
        let out = look(&mut s, &mut chests, &mut unmoved, Some(day * DAY));
        for o in &out {
            match o {
                Output::WagonSold { sold: n, .. } => sold += n,
                Output::Moved { point, count, .. } if point.x == 600 && *count > 0 => into_mill += count,
                Output::Moved { point, count, .. } if point.x == 0 && *count < 0 => out_of_farm -= count,
                _ => {}
            }
        }
        assert!(unmoved.is_empty(), "day {day}: two double chests hold it all");
        for (town, (_, c)) in chests.iter().enumerate() {
            let whole: Vec<i64> = stock(&s, town).iter().map(|n| (n + 1e-9).floor() as i64).collect();
            assert_eq!(c.goods, whole, "day {day}: town {town}'s chests show its stock");
        }
        s.as_mut().unwrap().conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
    assert!(sold > 500.0, "the wagon sold {sold} wheat");
    // A look shows what it moved all told: the farm's harvest and the wagon's load net out within a day.
    assert!(into_mill > 400 && out_of_farm > 0, "moved into the mill's chests {into_mill}, out of the farm's {out_of_farm}");
}

/// The plan's race (`docs/binding-plan.md` 4): a hopper fills the chest between its stocktake and the core's write. The
/// goods that don't fit stay the town's, kept on its books, and are moved in once a stocktake shows room; the hopper's
/// own goods are booked by the next stocktake; nothing is made or lost.
#[test]
fn a_hopper_filling_a_chest_during_a_move_keeps_what_didnt_fit_on_the_towns_books() {
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(0) }]);
    // A chest of two slots, one of wheat.
    let mut chests = vec![(desk(0), Chests::new(2, &[(WHEAT, 64)]))];
    let mut unmoved = Vec::new();
    look(&mut s, &mut chests, &mut unmoved, None);
    apply(&mut s, vec![Input::Deliver { town: 0, good: BREAD, count: 100.0 }]);
    // The look counts one free slot, so the core moves a stack in; before the host writes it, a hopper fills the slot.
    let out = apply(&mut s, vec![chests[0].1.stocktake(&desk(0))]);
    assert_eq!(moved(&out), vec![(0, BREAD, 64)], "a stack for the one free slot; 36 wait for room");
    chests[0].1.junk += 1;
    assert_eq!(chests[0].1.write(BREAD, 64), 64, "nothing fits");
    // What it couldn't do comes back before the next stocktake: the chests are taken to hold no bread, the town keeps it.
    let out = apply(&mut s, vec![Input::Unmoved { point: desk(0), good: BREAD, count: 64 }, chests[0].1.stocktake(&desk(0))]);
    assert!(moved(&out).is_empty(), "no room counted, nothing moved: {out:?}");
    assert_eq!(stock(&s, 0), vec![64.0, 100.0]);
    assert_eq!(held(&s, &desk(0)), vec![64.0, 0.0]);
    // The hopper drops wheat in too, and a player clears the junk: the stocktake books the wheat, the room takes bread.
    chests[0].1.junk = 0;
    chests[0].1.goods[WHEAT as usize] += 10;
    chests[0].1.slots = 4;
    let out = look(&mut s, &mut chests, &mut unmoved, None);
    assert_eq!(moved(&out), vec![(0, BREAD, 100)]);
    assert_eq!(stock(&s, 0), vec![74.0, 100.0]);
    assert_eq!(chests[0].1.goods, vec![74, 100]);
    assert!(unmoved.is_empty());
}

#[test]
fn what_has_no_room_at_the_desk_goes_to_a_depot_and_what_has_none_anywhere_waits() {
    let mut s = created();
    apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk(0) },
        Input::AddPoint { point: at(PointKind::Depot, 5), town: 0 },
        Input::AddPoint { point: at(PointKind::Depot, 7), town: 0 },
    ]);
    // The desk's chest is full but for 20 bread's worth of a stack; one depot has a slot, the other none.
    let mut chests = vec![
        (desk(0), Chests::new(2, &[(WHEAT, 64), (BREAD, 44)])),
        (at(PointKind::Depot, 5), Chests::new(1, &[])),
        (at(PointKind::Depot, 7), Chests::new(1, &[(WHEAT, 64)])),
    ];
    let mut unmoved = Vec::new();
    look(&mut s, &mut chests, &mut unmoved, None);
    apply(&mut s, vec![Input::Deliver { town: 0, good: BREAD, count: 100.0 }]);
    let out = look(&mut s, &mut chests, &mut unmoved, None);
    assert_eq!(moved(&out), vec![(0, BREAD, 20), (5, BREAD, 64)], "the desk's stack topped up, a depot's slot filled");
    assert_eq!(stock(&s, 0)[BREAD as usize], 144.0);
    assert_eq!(chests.iter().map(|(_, c)| c.goods[BREAD as usize]).sum::<i64>(), 128, "16 wait on the books");
    // Out again: the desk first, then the depots; a depot not counted in the look isn't touched.
    s.as_mut().unwrap().withdraw(0, BREAD as usize, 100.0);
    let out = apply(&mut s, vec![chests[0].1.stocktake(&desk(0)), chests[2].1.stocktake(&at(PointKind::Depot, 7))]);
    assert_eq!(moved(&out), vec![(0, BREAD, -64)], "only the desk counted holds bread: the rest waits for the depot");
    let out = apply(&mut s, vec![chests[1].1.stocktake(&at(PointKind::Depot, 5))]);
    assert_eq!(moved(&out), vec![(5, BREAD, -20)], "the depot counted gives up what's left to take");
}

/// Write-back safety (`docs/decisions.md`, lockers): a chest not counted (its chunks unloaded) isn't written, and when
/// it's counted again what changed in it meanwhile (a hand edit, a rollback) is booked before what the town moved is
/// shown: never a blind write of what the core thought was there.
#[test]
fn a_chest_changed_while_it_wasnt_counted_is_counted_before_its_written() {
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(0) }]);
    let mut chests = vec![(desk(0), Chests::new(27, &[(WHEAT, 50)]))];
    let mut unmoved = Vec::new();
    look(&mut s, &mut chests, &mut unmoved, None);
    // Unloaded: the town takes in bread and days pass, and someone takes 20 wheat out of the chest meanwhile.
    apply(&mut s, vec![Input::Deliver { town: 0, good: BREAD, count: 30.0 }, Input::Advance { to: 3 * DAY }]);
    chests[0].1.goods[WHEAT as usize] -= 20;
    let out = look(&mut s, &mut chests, &mut unmoved, Some(3 * DAY + 1200));
    assert_eq!(moved(&out), vec![(0, BREAD, 30)], "the wheat taken is booked, not put back");
    assert_eq!(stock(&s, 0), vec![30.0, 30.0]);
    assert_eq!(chests[0].1.goods, vec![30, 30]);
}

/// A move out the host couldn't do (a player took the goods first) is booked by the next stocktake, as that player's
/// taking; what the chests gave up beyond what the town held is the point's `short`.
#[test]
fn what_wasnt_there_to_take_is_booked_by_the_next_stocktake_and_what_the_town_never_had_is_short() {
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(0) }]);
    let mut chests = vec![(desk(0), Chests::new(27, &[(WHEAT, 30)]))];
    let mut unmoved = Vec::new();
    look(&mut s, &mut chests, &mut unmoved, None);
    s.as_mut().unwrap().withdraw(0, WHEAT as usize, 10.0); // the core's own use of 10
    let out = apply(&mut s, vec![chests[0].1.stocktake(&desk(0))]);
    assert_eq!(moved(&out), vec![(0, WHEAT, -10)]);
    // Before the host takes them, a player takes 25: only 5 are there to take.
    chests[0].1.goods[WHEAT as usize] -= 25;
    assert_eq!(chests[0].1.write(WHEAT, -10), -5);
    assert_eq!(chests[0].1.goods[WHEAT as usize], 0);
    apply(&mut s, vec![Input::Unmoved { point: desk(0), good: WHEAT, count: -5 }, chests[0].1.stocktake(&desk(0))]);
    assert_eq!(stock(&s, 0), vec![0.0, 0.0], "the player's 25 taken, of the 20 the town held");
    let c = s.as_ref().unwrap().chests(&spot(&desk(0))).unwrap();
    assert_eq!((c.held[0], c.short.clone()), (0.0, vec![5.0, 0.0]), "5 the chests gave up that the town no longer had");
    // And it's asked on the wire as the core takes it.
    match &call(&mut s, Command::Ask { queries: vec![Query::Chests { point: desk(0) }] }) {
        Response::Done { answers, .. } => assert_eq!(answers[0], Answer::Chests { point: desk(0), town: 0, held: vec![0.0, 0.0],
            free: 27, short: vec![5.0, 0.0] }),
        r => panic!("{r:?}"),
    }
}

#[test]
fn a_point_broken_takes_its_chests_goods_and_a_desk_put_back_is_its_town_again() {
    let mut s = created();
    apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk(0) },
        Input::AddPoint { point: at(PointKind::Depot, 3), town: 0 },
        Input::Pay { town: 0, money: enterprise_core::Fixed::emeralds(5) },
    ]);
    apply(&mut s, vec![
        Chests::new(27, &[(WHEAT, 30)]).stocktake(&desk(0)),
        Chests::new(27, &[(BREAD, 8)]).stocktake(&at(PointKind::Depot, 3)),
        Input::Deliver { town: 0, good: BREAD, count: 0.5 },
    ]);
    let start = s.as_mut().unwrap().totals();
    assert_eq!(stock(&s, 0), vec![30.0, 8.5]);
    // The desk broken: its 30 wheat go with its chests; the depot's bread and the half loaf stay the town's.
    apply(&mut s, vec![Input::RemovePoint { point: desk(0) }]);
    assert_eq!(stock(&s, 0), vec![0.0, 8.5]);
    // Put back where it stood (with its chests as they are, in the same look): the same town, its name and money kept,
    // its chests counted in afresh: the 30 wheat once.
    let out = apply(&mut s, vec![
        Input::AddTown { name: "Newton".into(), desk: desk(0) },
        Input::Deliver { town: 0, good: WHEAT, count: 2.0 },
        Chests::new(27, &[(WHEAT, 30)]).stocktake(&desk(0)),
    ]);
    assert_eq!(moved(&out), vec![(0, WHEAT, 2)], "the delivery shown");
    let society = s.as_mut().unwrap();
    assert_eq!((society.town_count(), society.town(0).name.as_str()), (1, "Ashford"));
    assert_eq!(society.town(0).money, enterprise_core::Fixed::emeralds(5));
    assert_eq!(society.town(0).stock, vec![32.0, 8.5]);
    society.conserved(&start).unwrap();
    // Broken and put back in one look: still the same town, and a look's checks know it (town 1 is still no town).
    let refused = call(&mut s, Command::Apply { inputs: vec![
        Input::RemovePoint { point: desk(0) },
        Input::AddTown { name: "Newton".into(), desk: desk(0) },
        Input::Deliver { town: 1, good: WHEAT, count: 1.0 },
    ] });
    assert!(matches!(refused, Response::Refused { why: Refusal::Input { at: 2, .. } }), "{refused:?}");
    apply(&mut s, vec![Input::RemovePoint { point: desk(0) }, Input::AddTown { name: "Newton".into(), desk: desk(0) }]);
    assert_eq!(s.as_ref().unwrap().town_count(), 1);
    // A desk somewhere else is a new town; and a place another point took is forgotten.
    apply(&mut s, vec![
        Input::RemovePoint { point: desk(0) },
        Input::AddPoint { point: at(PointKind::Shop, 0), town: 0 },
        Input::RemovePoint { point: at(PointKind::Shop, 0) },
        Input::AddTown { name: "Bly".into(), desk: desk(0) },
        Input::AddTown { name: "Cray".into(), desk: desk(900) },
    ]);
    let society = s.as_ref().unwrap();
    assert_eq!(society.town_count(), 3);
    assert_eq!((society.town(1).name.as_str(), society.town(2).name.as_str()), ("Bly", "Cray"));
}

#[test]
fn a_town_shows_whole_items_and_keeps_its_fractions() {
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(0) }]);
    let mut chests = vec![(desk(0), Chests::new(27, &[]))];
    let mut unmoved = Vec::new();
    look(&mut s, &mut chests, &mut unmoved, None);
    apply(&mut s, vec![Input::Deliver { town: 0, good: BREAD, count: 10.7 }]);
    assert_eq!(moved(&look(&mut s, &mut chests, &mut unmoved, None)), vec![(0, BREAD, 10)]);
    s.as_mut().unwrap().withdraw(0, BREAD as usize, 0.9); // 9.8: a loaf less to show
    assert_eq!(moved(&look(&mut s, &mut chests, &mut unmoved, None)), vec![(0, BREAD, -1)]);
    assert!(moved(&look(&mut s, &mut chests, &mut unmoved, None)).is_empty());
    assert_eq!(chests[0].1.goods[BREAD as usize], 9);
}

/// A shop's or a banker's chests aren't written to (their adapters say what they are), but what their stocktakes
/// counted is the town's, so it isn't shown again in the desk's.
#[test]
fn what_a_shops_stocktake_counted_isnt_shown_again_at_the_desk() {
    let mut s = created();
    apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk(0) },
        Input::AddPoint { point: at(PointKind::Shop, 3), town: 0 },
    ]);
    let out = apply(&mut s, vec![
        Chests::new(27, &[(WHEAT, 5)]).stocktake(&desk(0)),
        Chests::new(27, &[(BREAD, 3)]).stocktake(&at(PointKind::Shop, 3)),
    ]);
    assert!(moved(&out).is_empty(), "{out:?}");
    s.as_mut().unwrap().withdraw(0, BREAD as usize, 3.0);
    let out = apply(&mut s, vec![Chests::new(27, &[(BREAD, 3)]).stocktake(&at(PointKind::Shop, 3))]);
    assert!(moved(&out).is_empty(), "a shop's chests aren't written to: {out:?}");
    // Nor is an Unmoved there anything (the core never moved into it): what it counted stands.
    let counted = held(&s, &at(PointKind::Shop, 3));
    apply(&mut s, vec![Input::Unmoved { point: at(PointKind::Shop, 3), good: BREAD, count: 3 }]);
    assert_eq!(held(&s, &at(PointKind::Shop, 3)), counted, "an Unmoved at a shop changed what it holds");
}

/// A locker's chests are the Merchant's shelf, not the town's store (`locker_host.rs`): what its stocktake counted is
/// the town's, carried to the desk's chests counted in the same look as far as they have room, and what the town uses
/// is taken from a counted locker first. What the host couldn't take out of it comes back as an Unmoved, and stands.
#[test]
fn a_lockers_chests_are_emptied_into_the_desk_and_used_from_first() {
    let mut s = created();
    let locker = at(PointKind::Locker, 3);
    apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk(0) },
        Input::AddPoint { point: locker.clone(), town: 0 },
    ]);
    // The desk's chests have one slot free: room for a stack of the locker's 100 bread, and 36 stay.
    let out = apply(&mut s, vec![
        Chests::new(2, &[(WHEAT, 5)]).stocktake(&desk(0)),
        Chests::new(27, &[(BREAD, 100)]).stocktake(&locker),
    ]);
    assert_eq!(moved(&out), vec![(3, BREAD, -64), (0, BREAD, 64)], "{out:?}");
    assert_eq!(held(&s, &locker), vec![0.0, 36.0]);
    assert_eq!(stock(&s, 0), vec![5.0, 100.0], "the town holds the same, in other chests");
    // The town gives up 10 bread: taken from the locker it's left in, not the desk.
    s.as_mut().unwrap().withdraw(0, BREAD as usize, 10.0);
    let out = apply(&mut s, vec![
        Chests::new(2, &[(WHEAT, 5), (BREAD, 64)]).stocktake(&desk(0)),
        Chests::new(27, &[(BREAD, 36)]).stocktake(&locker),
    ]);
    assert_eq!(moved(&out), vec![(3, BREAD, -10)], "{out:?}");
    // A locker not counted in the look is neither used from nor emptied: a hand at the desk is.
    s.as_mut().unwrap().withdraw(0, BREAD as usize, 4.0);
    let out = apply(&mut s, vec![Chests::new(2, &[(WHEAT, 5), (BREAD, 64)]).stocktake(&desk(0))]);
    assert_eq!(moved(&out), vec![(0, BREAD, -4)], "{out:?}");
    // What the host couldn't take out of the locker (a hopper took it first) it says: the locker holds that much more,
    // as it counted, until its next stocktake books what's there.
    let before = held(&s, &locker);
    apply(&mut s, vec![Input::Unmoved { point: locker.clone(), good: BREAD, count: -5 }]);
    assert_eq!(held(&s, &locker)[1], before[1] + 5.0);
}

#[test]
fn an_unmoved_for_a_point_gone_is_a_stale_view_and_a_good_that_isnt_is_refused() {
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(0) }]);
    apply(&mut s, vec![
        Input::Unmoved { point: desk(40), good: WHEAT, count: 3 },
        Input::Unmoved { point: at(PointKind::Depot, 0), good: WHEAT, count: 3 },
    ]);
    assert_eq!(held(&s, &desk(0)), vec![0.0, 0.0]);
    let r = call(&mut s, Command::Apply { inputs: vec![Input::Unmoved { point: desk(0), good: 2, count: 1 }] });
    assert!(matches!(r, Response::Refused { why: Refusal::Input { at: 0, .. } }), "{r:?}");
    let r = call(&mut s, Command::Ask { queries: vec![Query::Chests { point: at(PointKind::Depot, 0) }] });
    assert!(matches!(r, Response::Refused { why: Refusal::Query { at: 0, .. } }), "{r:?}");
}

#[test]
fn chests_as_the_core_takes_them_and_a_desks_former_place_are_kept_by_a_save() {
    let mut s = created();
    apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk(0) },
        Input::AddTown { name: "Bly".into(), desk: desk(600) },
        Chests::new(9, &[(WHEAT, 70)]).stocktake(&desk(0)),
        Input::RemovePoint { point: desk(600) },
    ]);
    let Response::Saved { save } = call(&mut s, Command::Save) else { panic!("saved") };
    let mut loaded = None;
    assert_eq!(call(&mut loaded, Command::Load { save }), Response::Done { outputs: vec![], answers: vec![] });
    assert_eq!(loaded.as_ref().unwrap().chests(&spot(&desk(0))), s.as_ref().unwrap().chests(&spot(&desk(0))));
    apply(&mut loaded, vec![Input::AddTown { name: "Bly again".into(), desk: desk(600) }]);
    assert_eq!(loaded.as_ref().unwrap().town_count(), 2, "the desk put back after the load is Bly's");
}
