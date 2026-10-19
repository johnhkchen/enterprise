//! Wagons through the wire (`wagon_host.rs`): a host starting and stopping a town's wagons (`Input::Route`), told when
//! a hand goes away with one and comes back (`Output::HandAway`, `HandBack`), and asking what its screens show
//! (`Query::Routes`). The world is the wire's own: two towns founded at their desks forty blocks apart, Hub with its
//! Merchant and a villager of no trade, Far with books Hub's people are short of, so Hub's wagon goes shopping.

use enterprise_core::wire::*;
use enterprise_core::{Fixed, Society};

const DAY: u64 = 24_000;
const LOOK: u64 = 1_200;
const BREAD: GoodNo = 1;
const BOOK: GoodNo = 2;

fn pack() -> ContentPack {
    let mut wheat = PackGood::new("minecraft:wheat", 0.05, 64);
    wheat.flags = vec![GoodFlag::Crop];
    let mut bread = PackGood::new("minecraft:bread", 0.2, 64);
    bread.food = 5.0;
    bread.saturation = 6.0;
    let mut book = PackGood::new("minecraft:book", 0.5, 64);
    book.comfort = Some("reading".into());
    let mut p = ContentPack::new(DAY, vec![wheat, bread, book, PackGood::new("minecraft:paper", 0.04, 64)]);
    p.comforts = vec!["reading".into()];
    p.trades = ["merchant", "farmer", "librarian", "cartographer", "none", "nitwit"].map(String::from).to_vec();
    p
}

fn desk(x: i32, z: i32) -> Point {
    Point { kind: PointKind::Desk, dim: "minecraft:overworld".into(), x, y: -60, z }
}

struct Host {
    society: Option<Society>,
}

impl Host {
    /// Hub (town 0) and Far (town 1), forty blocks apart; Hub's lead is a villager of no trade called `lead` (none:
    /// a hand the host never named), and `more` hands of these trades beside it.
    fn new(lead: &str, more: &[&str]) -> Host {
        let mut h = Host { society: None };
        h.done(Command::Create { pack: pack(), seed: 7 });
        let mut inputs = vec![
            Input::AddTown { name: "Hub".into(), desk: desk(27, 27) },
            Input::AddTown { name: "Far".into(), desk: desk(67, 27) },
            Input::AddHand { town: 0, trade: "merchant".into(), level: 1, tag: "villager-hub".into() },
            Input::AddHand { town: 0, trade: "none".into(), level: 1, tag: lead.into() },
            Input::AddHand { town: 1, trade: "merchant".into(), level: 1, tag: "villager-far".into() },
            Input::Deliver { town: 0, good: BREAD, count: 128.0 },
            Input::Deliver { town: 1, good: BREAD, count: 128.0 },
            Input::Deliver { town: 1, good: BOOK, count: 64.0 },
            Input::Pay { town: 0, money: Fixed::from(50.0) },
            Input::Pay { town: 1, money: Fixed::from(50.0) },
        ];
        for (i, trade) in more.iter().enumerate() {
            inputs.push(Input::AddHand { town: 0, trade: (*trade).into(), level: 2, tag: format!("villager-{i}") });
        }
        h.apply(inputs);
        h
    }

    fn done(&mut self, command: Command) -> (Vec<Output>, Vec<Answer>) {
        match handle(&mut self.society, Request::new(command)) {
            Response::Done { outputs, answers } => (outputs, answers),
            r => panic!("{r:?}"),
        }
    }

    fn apply(&mut self, inputs: Vec<Input>) -> Vec<Output> {
        self.done(Command::Apply { inputs }).0
    }

    fn advance(&mut self, to: u64) -> Vec<Output> {
        self.apply(vec![Input::Advance { to }])
    }

    fn routes(&mut self, town: TownNo) -> (Vec<RouteLine>, Vec<MarketLine>, Vec<String>) {
        match self.done(Command::Ask { queries: vec![Query::Routes { town }] }).1.remove(0) {
            Answer::Routes { routes, markets, squad, .. } => (routes, markets, squad),
            a => panic!("{a:?}"),
        }
    }

    fn hands(&mut self, town: TownNo) -> Vec<HandView> {
        match self.done(Command::Ask { queries: vec![Query::Hands { town }] }).1.remove(0) {
            Answer::Hands { hands, .. } => hands,
            a => panic!("{a:?}"),
        }
    }

    fn refused(&mut self, inputs: Vec<Input>) -> Refusal {
        match handle(&mut self.society, Request::new(Command::Apply { inputs })) {
            Response::Refused { why } => why,
            r => panic!("not refused: {r:?}"),
        }
    }
}

fn aways(out: &[Output]) -> Vec<&Output> {
    out.iter().filter(|o| matches!(o, Output::HandAway { .. } | Output::HandBack { .. })).collect()
}

// parity-partial: CaravanTests.theSquadLeavesTheWorldAndComesBack (the core says who goes and when they're back; the
// host stows and lets out the villagers)
#[test]
fn a_hand_the_host_named_goes_away_with_its_wagon_and_comes_back() {
    let mut h = Host::new("villager-lead", &[]);
    let lead = h.hands(0)[1].hand;
    assert!(h.apply(vec![Input::Route { from: 0, to: 1, send: true }]).is_empty());
    let out = h.advance(LOOK);
    assert!(out.iter().any(|o| matches!(o, Output::WagonLeft { route: 0, .. })), "{out:?}");
    assert_eq!(aways(&out), vec![&Output::HandAway { town: 0, hand: lead, tag: "villager-lead".into() }]);
    assert!(h.hands(0)[1].away, "away while on the road");
    let (routes, _, squad) = h.routes(0);
    let wagon = routes[0].wagon.as_ref().expect("its wagon on the road");
    assert!(wagon.out && wagon.crew == vec![lead] && !wagon.porter, "{wagon:?}");
    assert_eq!(routes[0].status, "on the way out");
    assert!(squad.is_empty(), "nobody else at home to go: {squad:?}");
    assert!(aways(&h.advance(2 * LOOK)).is_empty(), "on its way home, still away");
    let out = h.advance(3 * LOOK);
    assert!(out.iter().any(|o| matches!(o, Output::WagonHome { route: 0, .. })), "{out:?}");
    match aways(&out).as_slice() {
        [Output::HandBack { town: 0, hand, tag, level: 1, .. }] => assert!(*hand == lead && tag == "villager-lead"),
        a => panic!("{a:?}"),
    }
    assert!(!h.hands(0)[1].away, "home");
    let (routes, _, _) = h.routes(0);
    assert!(routes[0].wagon.is_none() && routes[0].trips == 1 && routes[0].status.starts_with("home"), "{:?}", routes[0]);
}

// parity-partial: CaravanTests.aTownShortOfCrewTakesOnAHand ("home again, still a travelling Merchant": the trade it was
// taken on for comes back with it, for the host's villager)
#[test]
fn a_hand_taken_on_to_lead_comes_back_in_its_new_trade() {
    let mut h = Host::new("villager-lead", &[]);
    h.apply(vec![Input::Route { from: 0, to: 1, send: true }]);
    let out = h.advance(LOOK);
    assert!(!out.iter().any(|o| matches!(o, Output::HandLevel { .. })), "no word on a hand while it's away: {out:?}");
    assert_eq!(h.hands(0)[1].trade, "merchant", "taken on as the travelling Merchant");
    h.advance(2 * LOOK);
    let out = h.advance(3 * LOOK);
    assert!(aways(&out).iter().any(|o| matches!(o, Output::HandBack { trade, xp, .. } if trade == "merchant" && *xp >= 1)),
        "the villager comes home a Merchant: {out:?}");
}

#[test]
fn a_hand_the_host_never_named_comes_back_for_a_villager_spawned() {
    let mut h = Host::new("", &[]);
    h.apply(vec![Input::Route { from: 0, to: 1, send: true }]);
    let out = h.advance(LOOK);
    assert!(out.iter().any(|o| matches!(o, Output::WagonLeft { .. })) && aways(&out).is_empty(),
        "no villager to stow: {out:?}");
    h.advance(2 * LOOK);
    let out = h.advance(3 * LOOK);
    assert!(matches!(aways(&out).as_slice(), [Output::HandBack { tag, .. }] if tag.is_empty()), "{out:?}");
}

#[test]
fn a_trip_within_one_advance_never_leaves_the_hosts_world() {
    let mut h = Host::new("villager-lead", &[]);
    h.apply(vec![Input::Route { from: 0, to: 1, send: true }]);
    let out = h.advance(DAY);
    assert!(out.iter().any(|o| matches!(o, Output::WagonHome { .. })), "{out:?}");
    assert!(aways(&out).is_empty(), "gone and back within the advance: {out:?}");
}

// parity-partial: CaravanTests.caravansAreCommissionedAtTheDesk (the screen's send and stop, through the core)
#[test]
fn a_route_started_and_stopped_by_its_host() {
    let mut h = Host::new("villager-lead", &["librarian"]);
    let (routes, markets, squad) = h.routes(0);
    assert!(routes.is_empty());
    assert_eq!(markets.len(), 1, "Far, by word of mouth");
    let far = &markets[0];
    assert!(far.town == 1 && far.desk == Some(desk(67, 27)) && far.blocks == Some(40) && far.reachable && !far.routed,
        "{far:?}");
    assert_eq!(squad, ["librarian", "none"], "its squad at home, in the squad's order");
    h.apply(vec![Input::Route { from: 0, to: 1, send: true }, Input::Route { from: 0, to: 1, send: true }]);
    let (routes, markets, _) = h.routes(0);
    assert_eq!(routes.len(), 1, "started twice is one route");
    assert!(routes[0].open && routes[0].from == 0 && routes[0].to == 1 && routes[0].blocks == 40 && markets[0].routed);
    let (far_routes, _, _) = h.routes(1);
    assert_eq!(far_routes, routes, "the far end lists it too");
    h.apply(vec![Input::Route { from: 0, to: 1, send: false }]);
    let (routes, markets, _) = h.routes(0);
    assert!(!routes[0].open && !markets[0].routed && routes[0].status == "wound down", "{:?}", routes[0]);
    let out = h.advance(3 * DAY);
    assert!(!out.iter().any(|o| matches!(o, Output::WagonLeft { .. })), "stopped: no wagon leaves: {out:?}");
    h.apply(vec![Input::Route { from: 0, to: 1, send: true }]);
    let (routes, _, _) = h.routes(0);
    assert!(routes.len() == 1 && routes[0].route == 0 && routes[0].open, "the same route, open again: {routes:?}");
    let out = h.advance(3 * DAY + LOOK);
    assert!(out.iter().any(|o| matches!(o, Output::WagonLeft { route: 0, .. })), "{out:?}");
}

#[test]
fn a_wagon_on_the_road_when_its_route_stops_comes_home_and_stays() {
    let mut h = Host::new("villager-lead", &[]);
    h.apply(vec![Input::Route { from: 0, to: 1, send: true }]);
    h.advance(LOOK);
    h.apply(vec![Input::Route { from: 0, to: 1, send: false }]);
    let mut out = h.advance(2 * LOOK);
    out.extend(h.advance(3 * LOOK));
    assert!(aways(&out).iter().any(|o| matches!(o, Output::HandBack { .. })), "its crew home: {out:?}");
    let out = h.advance(5 * DAY);
    assert!(!out.iter().any(|o| matches!(o, Output::WagonLeft { .. })), "{out:?}");
    assert!(h.routes(0).0[0].wagon.is_none());
}

// parity-partial: CaravanTests.foodCapsALegAt3200Blocks (the command's "Too far": nothing opens)
#[test]
fn a_far_end_beyond_a_legs_food_opens_no_route() {
    let mut h = Host::new("villager-lead", &[]);
    h.apply(vec![Input::AddTown { name: "Distant".into(), desk: desk(27, 9_027) }]);
    h.apply(vec![Input::Route { from: 0, to: 2, send: true }]);
    assert!(h.routes(0).0.is_empty() && h.routes(2).0.is_empty(), "too far for a leg's food");
    let (_, markets, _) = h.routes(0);
    assert!(markets.iter().all(|m| m.town != 2), "nor a market Hub knows: {markets:?}");
}

#[test]
fn a_route_wound_down_reopens_only_within_a_legs_food_now() {
    let mut h = Host::new("villager-lead", &[]);
    h.apply(vec![Input::Route { from: 0, to: 1, send: true }, Input::Route { from: 0, to: 1, send: false }]);
    h.apply(vec![Input::SetDial { name: "food_capacity".into(), value: Some(1.0) }]);
    h.apply(vec![Input::Route { from: 0, to: 1, send: true }]);
    let (routes, markets, _) = h.routes(0);
    assert!(routes.len() == 1 && !routes[0].open && !markets[0].reachable, "too far now, kept wound down: {routes:?}");
    h.apply(vec![Input::SetDial { name: "food_capacity".into(), value: None }]);
    h.apply(vec![Input::Route { from: 0, to: 1, send: true }]);
    let (routes, _, _) = h.routes(0);
    assert!(routes.len() == 1 && routes[0].route == 0 && routes[0].open, "{routes:?}");
}

#[test]
fn a_route_to_itself_or_a_town_not_there_is_refused_and_nothing_applies() {
    let mut h = Host::new("villager-lead", &[]);
    assert!(matches!(h.refused(vec![Input::Route { from: 1, to: 1, send: true }]), Refusal::Input { at: 0, .. }));
    assert!(matches!(h.refused(vec![Input::Route { from: 0, to: 1, send: true }, Input::Route { from: 7, to: 0, send: false }]),
        Refusal::Input { at: 1, .. }));
    assert!(h.routes(0).0.is_empty(), "nothing of the refused look applied");
    assert!(matches!(handle(&mut h.society, Request::new(Command::Ask { queries: vec![Query::Routes { town: 5 }] })),
        Response::Refused { why: Refusal::Query { at: 0, .. } }));
}

#[test]
fn routes_and_a_crew_on_the_road_survive_a_save() {
    let mut h = Host::new("villager-lead", &[]);
    h.apply(vec![Input::Route { from: 0, to: 1, send: true }]);
    h.advance(LOOK);
    let before = h.routes(0);
    let save = match handle(&mut h.society, Request::new(Command::Save)) {
        Response::Saved { save } => save,
        r => panic!("{r:?}"),
    };
    let mut loaded = Host { society: None };
    loaded.done(Command::Load { save });
    assert_eq!(loaded.routes(0), before);
    loaded.advance(2 * LOOK);
    let out = loaded.advance(3 * LOOK);
    assert!(aways(&out).iter().any(|o| matches!(o, Output::HandBack { tag, .. } if tag == "villager-lead")), "{out:?}");
}

/// A hand back is seen by the census from then: a census that doesn't see its villager (not out yet) counts its days
/// unseen from its return. Seen last on day 0, its wagon making trips every four days, the census on day 11 that
/// doesn't see it would have it leave (ten days unseen) but for its last return.
#[test]
fn a_hand_back_counts_as_seen() {
    let mut h = Host::new("villager-lead", &[]);
    let census = |tags: &[&str]| Input::Census { town: 0, beds: 4, residents: tags.iter().map(|t| Resident {
        tag: (*t).into(), trade: "merchant".into(), level: 1, baby: false }).collect() };
    h.apply(vec![census(&["villager-hub", "villager-lead"])]);
    h.apply(vec![Input::Route { from: 0, to: 1, send: true }]);
    let mut back = false;
    for look in 1..=(11 * DAY / LOOK) {
        let out = h.advance(look * LOOK);
        back |= aways(&out).iter().any(|o| matches!(o, Output::HandBack { .. }));
        if look == 11 * DAY / LOOK - 1 {
            h.apply(vec![Input::Route { from: 0, to: 1, send: false }]);
        }
    }
    assert!(back, "its wagon went and came back");
    h.apply(vec![census(&["villager-hub"])]);
    assert!(h.hands(0).iter().any(|x| x.tag == "villager-lead"), "seen as it came back, it hasn't left");
}
