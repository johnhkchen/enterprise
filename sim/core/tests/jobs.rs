//! Work for a town's idle hands, pinned as the mod's `JobsTests` pin it: the bag of marbles an idle hand draws its
//! trade from (what its villagers would buy of the town's supply, idle plots, halls wanted, the comforts the town
//! lacks), the seeded draw, and the stall a town pays for (6 emeralds beyond its reserve).

use std::collections::BTreeMap;

use enterprise_core::cash_book::{Account, Reason};
use enterprise_core::jobs::{bag, comfort_marbles, draw, Marbles};
use enterprise_core::roster::{Hand, Roster};
use enterprise_core::state::{Dice, Rng};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;
const PAPER: usize = 0;
const CARROT: usize = 1;
const WHEAT: usize = 2;
const LEATHER: usize = 5;
const N: usize = 10;
// (goods 3 to 9: wool, carpets, leather, boots, cookies, cane, books)

/// The mod's prices (`Prices`: paper 24 to the emerald, carrots 22, wheat 20), and the goods the comfort
/// conversions make: carpets (furnishings), boots (clothing), cookies (treats), books (reading).
fn content() -> Content {
    let c = Content::COMFORTS;
    let kind = |name: &str| c.iter().position(|k| *k == name).unwrap();
    Content::new(vec![
        Good::new("paper", 1.0 / 24.0, 64),
        Good::new("carrot", 1.0 / 22.0, 64),
        Good::new("wheat", 1.0 / 20.0, 64),
        Good::new("white_wool", 1.0 / 18.0, 64),
        Good::new("white_carpet", 0.5, 64).comfort(kind("furnishings")),
        Good::new("leather", 1.0 / 6.0, 64),
        Good::new("leather_boots", 1.0, 16).comfort(kind("clothing")),
        Good::new("cookie", 0.2, 64).comfort(kind("treats")),
        Good::new("sugar_cane", 1.0 / 24.0, 64),
        Good::new("book", 0.5, 64).comfort(kind("reading")),
    ], DAY)
}

/// A supply with these goods worth these emeralds.
fn supply(worth: &[(usize, f64)]) -> Vec<f64> {
    let mut s = vec![0.0; N];
    for &(g, e) in worth {
        s[g] = e;
    }
    s
}

fn roster_of(c: &Content, hands: &[&str]) -> Roster {
    Roster::new(c, hands.iter().map(|t| Hand::new(c, c.trade(t).unwrap(), 1)).collect())
}

fn marbles(c: &Content, bag: &Marbles, trade: &str) -> f64 {
    let t = c.trade(trade).unwrap();
    bag.iter().filter(|(b, _)| *b == t).map(|(_, m)| m).sum()
}

fn none() -> BTreeMap<usize, f64> {
    BTreeMap::new()
}

// parity: JobsTests.aComfortTheTownLacksDrawsTheTradeThatMakesIt
#[test]
fn a_comfort_the_town_lacks_draws_the_trade_that_makes_it() {
    // The comforts' makers are read off the content's conversions, as the mod reads `Workshop.CONVERSIONS`.
    let c = content();
    let kind = |name: &str| c.comfort(name).unwrap();
    let mut met = vec![0.0; 5];
    met[kind("furnishings")] = 0.0;
    met[kind("clothing")] = 0.5;
    met[kind("treats")] = 1.0;
    let share = c.dials.comfort_marbles;
    assert_eq!(share, 0.2);
    let marbles_ = comfort_marbles(&c, &met, 500.0, &roster_of(&c, &[]));
    assert!((marbles(&c, &marbles_, "shepherd") - 500.0 * share).abs() < 1e-9, "no furnishings: a Shepherd's share of savings");
    assert!((marbles(&c, &marbles_, "leatherworker") - 0.5 * 500.0 * share).abs() < 1e-9, "clothing half met: half");
    assert_eq!(marbles(&c, &marbles_, "farmer"), 0.0, "treats met: nothing for the Farmer's cookies");
    let with_shepherd = comfort_marbles(&c, &met, 500.0, &roster_of(&c, &["shepherd"]));
    assert!(with_shepherd.iter().all(|(t, _)| *t != c.trade("shepherd").unwrap()), "the town has a Shepherd: none for it");
    assert_eq!(marbles(&c, &with_shepherd, "leatherworker"), 50.0, "the Leatherworker's are unchanged");
}

// parity: JobsTests.anIdleHandDrawsATradeByWhatItWouldMeet
#[test]
fn an_idle_hand_draws_a_trade_by_what_it_would_meet() {
    let c = content();
    let nobody = roster_of(&c, &[]);
    let paper = supply(&[(PAPER, 3.0), (CARROT, 1.0)]);
    let b = bag(&c, &paper, &nobody, &none(), 0.0, &[]);
    assert_eq!(marbles(&c, &b, "librarian"), 3.0, "paper: 3 marbles a Librarian's");
    assert_eq!(marbles(&c, &b, "cartographer"), 3.0, "and a Cartographer's");
    assert_eq!(marbles(&c, &b, "farmer"), 1.0, "carrots 1 a Farmer's");
    let with_librarian = bag(&c, &paper, &roster_of(&c, &["librarian"]), &none(), 0.0, &[]);
    assert_eq!(marbles(&c, &with_librarian, "librarian"), 1.5, "with a Librarian already, a second shares the paper");
    assert_eq!(marbles(&c, &with_librarian, "cartographer"), 3.0, "the Cartographer's are unchanged");
    let mut rng = Rng::new(1);
    assert_eq!(draw(&bag(&c, &supply(&[]), &nobody, &none(), 0.0, &[]), &mut rng), None, "nothing to sell: no stall");
    let carrots = bag(&c, &supply(&[(CARROT, 2.0)]), &nobody, &none(), 0.0, &[]);
    assert_eq!(draw(&carrots, &mut rng), c.trade("farmer"), "only carrots: a Farmer");
    // Untended plots: four days of what they'd yield, in a Farmer's marbles.
    let idle = |per_day: f64| BTreeMap::from([(WHEAT, per_day)]);
    let forty = bag(&c, &supply(&[]), &nobody, &idle(40.0), 0.0, &[]);
    assert!((marbles(&c, &forty, "farmer") - 8.0).abs() < 1e-9, "40 wheat a day untended: 8 marbles a Farmer's");
    let four = bag(&c, &supply(&[]), &nobody, &idle(4.0), 0.0, &[]);
    assert_eq!(marbles(&c, &four, "farmer"), 0.0, "a few plots idle: not worth a hand");
    // A town with halls it wants and could pay for: a Mason's marbles, until it has one.
    let halls = bag(&c, &supply(&[]), &nobody, &none(), 8.0, &[]);
    assert_eq!(marbles(&c, &halls, "mason"), 8.0, "8 emeralds of building wanted: 8 marbles a Mason's");
    let with_mason = bag(&c, &supply(&[]), &roster_of(&c, &["mason"]), &none(), 8.0, &[]);
    assert_eq!(marbles(&c, &with_mason, "mason"), 0.0, "one is enough");
    // Drawn often enough, each trade comes up about as often as its share.
    let mut librarians = 0;
    for _ in 0..1000 {
        if draw(&b, &mut rng) == c.trade("librarian") {
            librarians += 1;
        }
    }
    assert!(librarians > 330 && librarians < 530,
        "Librarian, Cartographer and Farmer at 3:3:1: about 3 in 7 Librarians, {librarians} of 1000");
}

#[test]
fn the_markets_signals_join_the_bag() {
    let c = content();
    let nobody = roster_of(&c, &[]);
    let fletcher = c.trade("fletcher").unwrap();
    let b = bag(&c, &supply(&[(PAPER, 3.0)]), &nobody, &none(), 0.0, &[(fletcher, 5.0), (c.trade("librarian").unwrap(), 2.0)]);
    assert_eq!(marbles(&c, &b, "fletcher"), 5.0, "goods wanted that a Fletcher would supply");
    assert_eq!(marbles(&c, &b, "librarian"), 5.0, "added to what a Librarian would meet");
}

#[test]
fn the_same_seed_draws_the_same() {
    let c = content();
    let b = bag(&c, &supply(&[(PAPER, 3.0), (CARROT, 1.0), (WHEAT, 4.0)]), &roster_of(&c, &[]), &none(), 0.0, &[]);
    let draws = |seed| {
        let mut rng = Rng::new(seed);
        (0..50).map(|_| draw(&b, &mut rng)).collect::<Vec<_>>()
    };
    assert_eq!(draws(9), draws(9));
    assert_ne!(draws(9), draws(10), "another seed, another run");
}

/// A town with a Merchant (a desk), an idle hand and `paper` in the warehouse (3 emeralds' worth for 72).
fn town(money: f64, paper: f64) -> (Society, usize) {
    let mut s = Society::new(content());
    s.seed(1);
    let t = s.add_town("Inkwell", money);
    s.set_roster(t, &[("merchant", 1), ("none", 1)]);
    if paper > 0.0 {
        s.deliver(t, PAPER, paper);
    }
    (s, t)
}

fn stall_paid(s: &Society, t: usize) -> enterprise_core::Fixed {
    s.cash_book(t, 400).iter().filter(|(_, what, _)| what == "stall").map(|(_, _, e)| e).sum()
}

#[test]
fn an_idle_hand_takes_the_trade_its_stall_is_paid_for() {
    let (mut s, t) = town(20.0, 72.0);
    let start = s.totals();
    let (hand, trade, retrained) = s.review_jobs(t).expect("a stall");
    assert_eq!((hand, retrained), (1, false));
    assert!(["librarian", "cartographer"].contains(&trade.as_str()), "paper's buyers: {trade}");
    assert_eq!(s.town(t).money, 14.0, "6 emeralds for the stall");
    assert_eq!(stall_paid(&s, t), -6.0, "in the cash book as the stall");
    let roster = s.roster(t);
    assert_eq!(roster.hands[1].trade, s.content().trade(&trade).unwrap());
    assert_eq!(roster.hands[1].level, 1);
    let locker = &s.lockers(t).unwrap().hands[1];
    assert!(locker.offers.iter().any(|o| o.on) && locker.offers.iter().filter(|o| o.on).all(|o| !o.sells && o.level <= 1),
        "its trades that buy goods for emeralds are switched on");
    assert_eq!(locker.slack, -10, "twice the slack days to get going");
    s.conserved(&start).unwrap();
    s.solvent().unwrap();
    assert!(s.review_jobs(t).is_none(), "no one left idle");
}

#[test]
fn a_hand_the_draw_puts_to_a_trade_has_its_stall_put_up_by_the_host() {
    // The day's review hires the idle hand, and says so: the host builds the workstation and a chest against it.
    let (mut s, t) = town(20.0, 72.0);
    let out = s.advance(DAY);
    let stalls: Vec<_> = out.iter().filter_map(|o| match o {
        enterprise_core::Output::Stall { town, trade, .. } => Some((*town as usize, trade.clone())),
        _ => None,
    }).collect();
    let trade = s.content().trades[s.roster(t).hands[1].trade].name.clone();
    assert_eq!(stalls, vec![(t, trade)], "one stall, of the trade it took");
}

#[test]
fn no_supply_no_stall() {
    let (mut s, t) = town(20.0, 0.0);
    assert!(s.job_bag(t).iter().all(|(_, m)| *m == 0.0), "nothing to sell, no savings to spend on comforts");
    assert!(s.review_jobs(t).is_none());
    assert_eq!(s.town(t).money, 20.0);
    assert_eq!(s.roster(t).hands[1].trade, s.content().trade("none").unwrap(), "still idle");
}

#[test]
fn a_town_without_a_desk_puts_up_no_stall() {
    let mut s = Society::new(content());
    let t = s.add_town("Hamlet", 50.0);
    s.set_roster(t, &[("none", 1)]);
    s.deliver(t, PAPER, 72.0);
    assert!(s.review_jobs(t).is_none());
}

#[test]
fn the_stall_comes_out_of_what_is_beyond_the_reserve() {
    let (mut s, t) = town(6.5, 72.0);
    s.set_trade(t, WHEAT, 0.0, 10.0); // its people use 10 wheat a day: 2 days of it, 1 emerald, is the reserve
    assert!(s.review_jobs(t).is_none(), "5.5 beyond the reserve is short of 6");
    assert_eq!(s.town(t).money, 6.5);
    // Beyond the reserve and the Accountant's say (half the free money a day): 6 beyond it isn't enough, 12 is.
    s.pay(t, 0.5);
    assert!(s.review_jobs(t).is_none(), "the Accountant allows half of 6 a day");
    // (The day's budget is set at its first look, so a richer town is a new one.)
    let (mut s, t) = town(13.0, 72.0);
    s.set_trade(t, WHEAT, 0.0, 10.0);
    assert!(s.review_jobs(t).is_some(), "12 beyond it is enough");
    assert_eq!(s.town(t).money, 7.0, "13 less the stall; the reserve is left");
}

#[test]
fn short_of_coin_the_town_borrows_its_households_savings() {
    let (mut s, t) = town(6.5, 72.0);
    s.set_trade(t, WHEAT, 0.0, 10.0);
    s.post(Account::Outside, Account::Households(t), 10.0, Reason::PaidIn);
    let start = s.totals();
    assert!(s.review_jobs(t).is_some());
    assert_eq!(s.town(t).money, 1.0, "the half emerald short borrowed, the stall paid");
    assert_eq!(s.deposits(t), 0.5, "owed the households");
    assert_eq!(s.town(t).people.purse, 9.5);
    s.conserved(&start).unwrap();
}

#[test]
fn a_trade_a_contract_needs_comes_first() {
    let (mut s, t) = town(20.0, 72.0);
    s.set_needed(t, &["fletcher"]);
    let (_, trade, _) = s.review_jobs(t).expect("a stall");
    assert_eq!(trade, "fletcher", "nothing of sticks in the warehouse, but a contract needs one");
    // With one already, the draw takes over.
    let (mut s, t) = town(20.0, 72.0);
    s.set_roster(t, &[("merchant", 1), ("none", 1), ("fletcher", 1)]);
    s.set_needed(t, &["fletcher"]);
    let (_, trade, _) = s.review_jobs(t).expect("a stall");
    assert_ne!(trade, "fletcher");
}

/// What the town holds most of, worth 16 emeralds or more, trains its next idle hand to the trade that takes it, where it
/// has none: Ryegate's 322 beef had no Butcher.
#[test]
fn a_glut_trains_the_trade_that_takes_it() {
    let (mut s, t) = town(20.0, 72.0);
    s.deliver(t, LEATHER, 120.0);
    let (_, trade, _) = s.review_jobs(t).expect("a stall");
    assert_eq!(trade, "leatherworker", "20 emeralds of leather, not drawn");
    let c = content();
    let stock = |leather: f64| {
        let mut v = vec![0.0; N];
        v[LEATHER] = leather;
        enterprise_core::state::Stock(v)
    };
    let jobs = |hands: &[&str], leather: f64| enterprise_core::jobs::for_glut(&c, &stock(leather), &roster_of(&c, hands), &[]);
    assert_eq!(jobs(&["none"], 90.0), None, "15 emeralds is no glut");
    assert_eq!(jobs(&["none", "leatherworker"], 600.0), None, "one Leatherworker already");
    assert_eq!(jobs(&["none"], 96.0), c.trade("leatherworker"));
}

#[test]
fn a_comfort_the_town_lacks_is_in_the_bag_of_a_town_with_savings() {
    let (mut s, t) = town(20.0, 0.0);
    s.post(Account::Outside, Account::Households(t), 100.0, Reason::PaidIn);
    let bag = s.job_bag(t);
    let of = |name: &str| bag.iter().filter(|(n, _)| n == name).map(|(_, m)| m).sum::<f64>();
    // No comfort has been counted yet: each is lacking in full, and a fifth of 100 goes to its maker.
    assert_eq!(of("shepherd"), 20.0);
    assert_eq!(of("leatherworker"), 20.0);
    assert_eq!(of("farmer"), 20.0, "cookies");
    assert_eq!(of("librarian"), 20.0, "books");
}

#[test]
fn signals_and_halls_wanted_are_in_the_bag() {
    let (mut s, t) = town(20.0, 0.0);
    s.set_signal(t, "fletcher", 5.0);
    s.set_build_work(t, 8.0);
    let bag = s.job_bag(t);
    assert!(bag.contains(&("fletcher".to_string(), 5.0)) && bag.contains(&("mason".to_string(), 8.0)), "{bag:?}");
    let (_, trade, _) = {
        s.set_needed(t, &[]);
        s.review_jobs(t).expect("a stall")
    };
    assert!(["fletcher", "mason"].contains(&trade.as_str()));
    // They are saved with the world.
    let (mut s, t) = town(20.0, 0.0);
    s.set_signal(t, "fletcher", 5.0);
    let loaded = Society::load(&s.save()).unwrap();
    assert_eq!(loaded.job_bag(t), s.job_bag(t));
}

/// A town with no idle hand: a Merchant and a `trade` whose locker has done nothing for the town, `reviews` times.
fn slack_town(trade: &str, reviews: usize) -> (Society, usize) {
    let mut s = Society::new(content());
    s.seed(1);
    let t = s.add_town("Inkwell", 20.0);
    s.set_roster(t, &[("merchant", 1), (trade, 1)]);
    s.deliver(t, PAPER, 72.0);
    for _ in 0..reviews {
        s.review_lockers(t);
    }
    (s, t)
}

#[test]
fn a_slack_trade_gives_way_to_one_the_town_needs() {
    let (mut s, t) = slack_town("fletcher", 4);
    assert!(s.review_jobs(t).is_none(), "4 reviews of nothing is not 5 days");
    s.review_lockers(t);
    let (hand, trade, retrained) = s.review_jobs(t).expect("it retrains");
    assert!(retrained && hand == 1 && ["librarian", "cartographer"].contains(&trade.as_str()), "{trade}");
    assert_eq!(s.roster(t).hands[1].trade, s.content().trade(&trade).unwrap());
    assert_eq!(s.town(t).money, 14.0, "a stall for the new trade");
    assert!(s.review_jobs(t).is_none(), "and time to get going");
}

#[test]
fn a_slack_trade_stays_where_roads_carry_and_gives_way_where_they_dont() {
    use enterprise_core::jobs::carries;
    use enterprise_core::state::RouteState;
    // Where wagons come and go a trade's work may be its workshop's, or goods carried away: it stays.
    let c = content();
    let lately = RouteState { last_departure: Some(10 * DAY), ..Default::default() };
    assert!(carries(&c, &lately, 12 * DAY), "a wagon gone two days ago: the road carries");
    assert!(!carries(&c, &lately, 16 * DAY), "six days ago: not lately (`slack_days`, 5)");
    assert!(!carries(&c, &RouteState::default(), 16 * DAY), "never: not at all");
    // A road no wagon has taken is no reason to keep it (John: "fix that": Meadowfell's Leatherworkers).
    let (mut s, t) = slack_town("fletcher", 6);
    let other = s.add_town("Elsewhere", 0.0);
    s.add_route(t, other, 1.0, 0.0, 4.0);
    let (_, _, retrained) = s.review_jobs(t).expect("it retrains");
    assert!(retrained);
}

#[test]
fn farmers_masons_and_clerics_are_never_slack() {
    for trade in ["farmer", "mason", "cleric"] {
        let (mut s, t) = slack_town(trade, 8);
        assert!(s.review_jobs(t).is_none(), "{trade}: its work isn't in its locker's trades");
    }
}

#[test]
fn a_slack_trade_keeps_its_hand_when_the_town_has_all_it_needs() {
    // The only trades with marbles are ones the town already has.
    let mut s = Society::new(content());
    s.seed(1);
    let t = s.add_town("Inkwell", 20.0);
    s.set_roster(t, &[("merchant", 1), ("fletcher", 1), ("librarian", 1), ("cartographer", 1)]);
    s.set_signal(t, "fletcher", 3.0);
    for _ in 0..6 {
        s.review_lockers(t);
    }
    // Hands 1 to 3 are slack; the draw gives a fletcher (signal only), which the town has.
    assert!(s.review_jobs(t).is_none());
    assert_eq!(s.town(t).money, 20.0);
}

#[test]
fn the_look_finds_one_idle_hand_a_trade_a_day() {
    let mut s = Society::new(content());
    s.seed(3);
    let t = s.add_town("Inkwell", 100.0);
    s.set_roster(t, &[("merchant", 1), ("none", 1), ("none", 1), ("none", 1)]);
    s.deliver(t, PAPER, 7200.0); // plenty, though its Librarians and Cartographers will buy it
    let idle = |s: &Society| {
        let (r, none) = (s.roster(t), s.content().trade("none").unwrap());
        r.hands.iter().filter(|h| h.trade == none).count()
    };
    let start = s.totals();
    s.advance(DAY - 1);
    assert_eq!(idle(&s), 2, "one a day");
    s.advance(10 * DAY);
    assert_eq!(idle(&s), 0, "and then none idle");
    assert_eq!(stall_paid(&s, t), -18.0, "three stalls");
    s.conserved(&start).unwrap();
    s.solvent().unwrap();
}

#[test]
fn the_same_seed_hires_the_same() {
    let hire = |seed| {
        let mut s = Society::new(content());
        s.seed(seed);
        let t = s.add_town("Inkwell", 100.0);
        s.set_roster(t, &[("merchant", 1), ("none", 1), ("none", 1), ("none", 1), ("none", 1)]);
        s.deliver(t, PAPER, 72.0);
        s.deliver(t, CARROT, 66.0);
        s.advance(10 * DAY);
        s.roster(t).hands.iter().map(|h| h.trade).collect::<Vec<_>>()
    };
    assert_eq!(hire(5), hire(5));
}

#[test]
fn the_dials_are_the_mods() {
    let s = Society::new(content());
    assert_eq!(s.dial("stall_emeralds"), Some(6.0));
    assert_eq!(s.dial("comfort_marbles"), Some(0.2));
    assert_eq!(s.dial("slack_days"), Some(5.0));
    assert_eq!(s.dial("farmer_idle_emeralds"), Some(0.5));
}

#[test]
fn a_town_raising_settlers_keeps_its_idle_hands_for_them() {
    // Grown (8 villagers), fed at twice the caravans' par, no party out: it is raising one, and can't pay for it yet
    // (12 is short of a village's 14), so its idle hand waits (`Territory.grow`), where a smaller town hires.
    let idle_after = |masons: usize, idle: usize| {
        let mut s = Society::new(Content::new(vec![
            Good::new("paper", 1.0 / 24.0, 64),
            Good::new("bread", 1.0 / 3.0, 64).food(5.0),
        ], DAY));
        s.seed(1);
        let t = s.add_town("Inkwell", 12.0);
        let mut hands = vec![("merchant", 1)];
        hands.extend(std::iter::repeat_n(("none", 1), idle));
        hands.extend(std::iter::repeat_n(("mason", 1), masons));
        s.set_roster(t, &hands);
        s.deliver(t, 0, 72.0);
        s.deliver(t, 1, 400.0);
        assert_eq!(s.raising(t), masons + idle + 1 >= 8);
        s.advance(3 * DAY);
        assert_eq!(s.parties().len(), 0, "no party: 12 is short of a village");
        s.roster(t).hands.iter().filter(|h| h.trade == s.content().trade("none").unwrap()).count()
    };
    assert_eq!(idle_after(6, 1), 1, "raising settlers: the hand waits for them");
    assert_eq!(idle_after(2, 1), 0, "not grown: it takes a stall");
    // A party takes two at most: a third waiting hand takes a stall (seed 8675309's Meadowfell held eight).
    assert_eq!(idle_after(5, 3), 2, "two wait for the party, the third is hired");
}

#[test]
fn a_towns_dice_are_its_own() {
    // Town 1's rolls are the same whether town 0 has rolled a hundred times or never: a change in one town doesn't
    // move another's draws.
    let seed = Rng::new(7);
    let mut alone = Dice::default();
    let first: Vec<u64> = (0..5).map(|_| alone.of(1, &seed).next()).collect();
    let mut busy = Dice::default();
    for _ in 0..100 {
        busy.of(0, &seed).next();
    }
    let after: Vec<u64> = (0..5).map(|_| busy.of(1, &seed).next()).collect();
    assert_eq!(first, after);
    // Each town's stream is its own, and the world's seed isn't rolled.
    assert_ne!(Dice::default().of(0, &seed).next(), Dice::default().of(1, &seed).next());
    assert_eq!(seed.0, 7);
}

/// A town with villagers short of beds and land for a home keeps a home's price back from a stall (`home_reserve`),
/// and builds the home with it: a new town spent its founding money on stalls and vendors by day 1 and stayed homeless.
#[test]
fn a_homeless_town_builds_its_home_before_a_stall() {
    let (mut s, t) = town(14.0, 72.0);
    s.set_people(t, 2.0, 0.0, 20.0);
    s.advance(1200); // a touch: the reserve set
    assert!(s.review_jobs(t).is_none() && stall_paid(&s, t) == enterprise_core::Fixed::ZERO,
        "14 emeralds, 10 kept for a home: no stall at 6");
    s.advance(6 * DAY);
    assert_eq!(s.town(t).people.homes_built, 1, "the home, out of what was kept for it");
    let (mut beds, t) = town(14.0, 72.0);
    beds.set_people(t, 2.0, 2.0, 20.0);
    beds.advance(1200);
    beds.review_jobs(t);
    assert_eq!(stall_paid(&beds, t), -6.0, "in beds, the stall");
}

/// What the town down a road buys, a town makes for it: Farfield's Leatherworker buys leather, so a Leatherworker, whose
/// cows give it, is in the bag of the town at the road's other end (a survival network's leatherworkers wanted 480
/// leather that no town made).
#[test]
fn the_trade_that_makes_what_the_town_down_the_road_buys_is_in_the_bag() {
    let (mut s, t) = town(20.0, 0.0);
    let far = s.add_town("Farfield", 0.0);
    s.set_roster(far, &[("leatherworker", 1), ("merchant", 1)]);
    s.set_offers(far, 0, vec![enterprise_core::lockers::Offer::buys(LEATHER, 6, 1, 16, 2, 1.0 / 6.0)]);
    s.switch(far, 0, 0, true);
    let of = |s: &Society, name: &str| s.job_bag(t).iter().filter(|(n, _)| n == name).map(|(_, m)| *m).sum::<f64>();
    let before = of(&s, "leatherworker");
    s.add_route(t, far, 0.25, 0.0, 1.0);
    // 2 x 6 leather x 16 uses a day: four days' is more than a stack, so a stack, at a sixth of an emerald.
    let far_marbles = of(&s, "leatherworker") - before;
    assert!((far_marbles - 64.0 / 6.0).abs() < 1e-9, "a stack of leather's worth: {far_marbles}");
    s.set_roster(t, &[("merchant", 1), ("none", 1), ("leatherworker", 1)]);
    assert!((of(&s, "leatherworker") - 64.0 / 12.0).abs() < 1e-9, "shared with the Leatherworker it has: {}", of(&s, "leatherworker"));
}

/// A crop the town down a road buys is a Farmer's to grow: Farmers only buy crops, so no trade's offer sells one, and a
/// survival network's Meadowfell, with six fields idle and its neighbours' Farmers buying thousands, never drew one.
#[test]
fn a_crop_the_town_down_the_road_buys_puts_a_farmer_in_the_bag() {
    let mut c = content();
    c.goods[CARROT] = c.goods[CARROT].clone().crop();
    let mut s = Society::new(c);
    s.seed(1);
    let t = s.add_town("Inkwell", 20.0);
    s.set_roster(t, &[("merchant", 1), ("none", 1)]);
    let far = s.add_town("Farfield", 0.0);
    s.set_roster(far, &[("farmer", 1), ("merchant", 1)]);
    s.set_offers(far, 0, vec![enterprise_core::lockers::Offer::buys(CARROT, 22, 1, 16, 2, 1.0 / 22.0)]);
    s.switch(far, 0, 0, true);
    let of = |s: &Society| s.job_bag(t).iter().filter(|(n, _)| n == "farmer").map(|(_, m)| *m).sum::<f64>();
    let before = of(&s);
    s.add_route(t, far, 0.25, 0.0, 1.0);
    let far_marbles = of(&s) - before;
    assert!((far_marbles - 64.0 / 22.0).abs() < 1e-9, "a stack of carrots' worth: {far_marbles}");
}

#[test]
fn a_stall_paid_for_is_a_locker_until_the_host_places_it() {
    // A town whose host places its lockers, unloaded: the hand the draw hires has its locker in the books at once.
    let (mut s, t) = town(20.0, 72.0);
    s.host_places_lockers(t);
    let (hand, _, _) = s.review_jobs(t).expect("a stall");
    let roster = s.roster(t);
    let lockers = s.lockers(t).unwrap();
    assert!(lockers.has(s.content(), &roster, hand), "paid for: a locker the Merchant can manage");
    assert!(lockers.stalls.contains(&roster.hands[hand].id));
    // The host puts it up and places it: the books' stall gives way to the real one.
    let id = roster.hands[hand].id;
    let spot = enterprise_core::points::Spot { dim: "minecraft:overworld".into(), x: 1, y: 64, z: 1 };
    s.add_point(enterprise_core::points::Kind::Locker, spot.clone(), t).unwrap();
    assert!(s.tie_locker(id, Some(spot)));
    assert!(s.lockers(t).unwrap().stalls.is_empty());
    assert!(s.lockers(t).unwrap().has(s.content(), &s.roster(t), hand));
}

/// A herd's keeper isn't slack while the town keeps its herd: Dustfell's Leatherworker, its two cows breeding, turned
/// Mason on day 7, and the cows bred to 48 with no one to take their hides. With no herd, it gives way.
#[test]
fn a_keeper_with_a_herd_is_never_slack() {
    let (mut s, t) = slack_town("leatherworker", 8);
    assert!(s.set_herd(t, "cow", 2.0));
    assert!(s.review_jobs(t).is_none(), "breeding its cows is its work");
    let (mut s, t) = slack_town("leatherworker", 8);
    assert!(s.review_jobs(t).expect("no herd: it retrains").2);
}

/// A herd with no keeper is a glut too, at its animals' meat and hides beyond the breeding stock.
#[test]
fn a_herd_with_no_keeper_trains_one() {
    let (mut s, t) = town(20.0, 72.0);
    assert!(s.set_herd(t, "cow", 120.0)); // 118 hides beyond the breeding stock, a sixth each: 19.7 emeralds
    let (_, trade, _) = s.review_jobs(t).expect("a stall");
    assert_eq!(trade, "leatherworker");
}
