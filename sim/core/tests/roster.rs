//! A town's people as a roster of hands, pinned as the mod's locker, levelling and growth tests pin them: trades as
//! content, levels at once at vanilla's thresholds (the Merchant at a player's), lockers by the Merchant's level,
//! babies grown by elapsed time, and villagers that follow the roster.

use enterprise_core::roster::{self, locker_limit, merchant_trade_xp, merchant_xp_for_next_level,
    villager_xp_for_next_level, Hand, BABY_TICKS, MERCHANT_LEVEL_XP};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;

fn content() -> Content {
    let mut goods: Vec<Good> = Content::COMFORTS.iter().enumerate()
        .map(|(k, name)| Good::new(&format!("{name}_good"), 0.05, 64).comfort(k)).collect();
    goods.push(Good::new("wheat", 0.05, 64));
    goods.push(Good::new("bread", 0.2, 64).food(5.0));
    Content::new(goods, DAY)
}

const WHEAT: usize = 5;
const BREAD: usize = 6;

#[test]
fn the_sixteen_trades_are_content() {
    let c = content();
    assert_eq!(c.trades.len(), 16);
    let leads: Vec<&str> = c.trades.iter().filter(|t| t.leads).map(|t| t.name.as_str()).collect();
    assert_eq!(leads, ["merchant", "cartographer", "librarian", "shepherd", "leatherworker", "fisherman"]);
    let squad: Vec<&str> = c.trades.iter().filter(|t| t.squad).map(|t| t.name.as_str()).collect();
    assert_eq!(squad, roster::SQUAD, "the squad's trades, in its order");
    assert!(c.trades.iter().filter(|t| t.leads).all(|t| t.squad), "every lead goes with a squad");
    let none = c.trade("none").unwrap();
    let nitwit = c.trade("nitwit").unwrap();
    assert!(c.trades[none].none && !c.trades[none].squad && c.trades[nitwit].nitwit && !c.trades[nitwit].leads);
    assert!(c.trades[c.trade("merchant").unwrap()].merchant);
    assert_eq!(c.trade("minecraft:librarian"), c.trade("librarian"), "a host's namespaced id reads the same");
    assert_eq!(c.trade("enterprise:merchant"), Some(0));
    assert_eq!(c.trade("wizard"), None);
}

// parity: LockerTests.lockerLimitGrowsWithLevel
#[test]
fn the_merchant_manages_two_lockers_a_level() {
    assert_eq!(locker_limit(1), 2, "Novice manages 2");
    assert_eq!(locker_limit(5), 10, "Master manages 10");
    assert_eq!(locker_limit(9), 10, "levels above Master are capped");
    assert_eq!(locker_limit(0), 2, "below Novice counts as Novice");
    let mut s = Society::new(content());
    let t = s.add_town("Meadowworth", 10.0);
    s.set_roster(t, &[("farmer", 5), ("merchant", 3), ("librarian", 2)]);
    let r = s.roster(t);
    assert_eq!(r.merchant, Some(1), "the desk's Merchant");
    assert_eq!(r.locker_limit(), 6, "a Journeyman Merchant manages 6");
    s.set_roster(t, &[("farmer", 5)]);
    assert_eq!(s.roster(t).locker_limit(), 0, "no Merchant, no lockers managed");
}

// parity: LockerTradingTests.merchantLevelsMatchPlayerLevels
#[test]
fn the_merchant_levels_at_a_players_thresholds() {
    assert_eq!(merchant_xp_for_next_level(1), 315, "Apprentice at 315");
    assert_eq!(merchant_xp_for_next_level(4), 6895, "Master at 6895");
    assert_eq!(merchant_xp_for_next_level(5), 0, "nothing past Master");
    assert_eq!(MERCHANT_LEVEL_XP, [315, 1395, 4020, 6895]);
    let c = content();
    let mut m = Hand::new(&c, c.trade("merchant").unwrap(), 1);
    assert!(!m.reward_merchant_xp(314), "314 is still a Novice");
    assert!(m.reward_merchant_xp(1) && m.level == 2, "315 an Apprentice, at once");
    assert!(m.reward_merchant_xp(10_000) && m.level == 3, "one level a trade, however much XP");
    assert!(m.reward_merchant_xp(0) && m.level == 4, "and the next trade catches up");
    assert!(m.reward_merchant_xp(0) && m.level == 5);
    assert!(!m.reward_merchant_xp(10_000) && m.level == 5, "nothing past Master");
    // What a player would pick up from a trade: 3 to 6, plus 5 when the villager levelled up on it.
    let xps: Vec<u32> = (0..8).map(|roll| merchant_trade_xp(roll, false)).collect();
    assert_eq!(xps, [3, 4, 5, 6, 3, 4, 5, 6]);
    assert_eq!(merchant_trade_xp(3, true), 11);
    // A Merchant set at a level has the XP it took to get there.
    assert_eq!(Hand::new(&c, 0, 3).xp, 1395);
}

// parity-partial: LockerTradingTests.aVillagerLevelsUpAtOnceFromLockerTrades
#[test]
fn a_villager_levels_up_at_once_from_its_trades() {
    assert_eq!([1, 2, 3, 4, 5].map(villager_xp_for_next_level), [10, 70, 150, 250, 0]);
    let c = content();
    let mut librarian = Hand::new(&c, c.trade("librarian").unwrap(), 1);
    // Five trades, ten XP: an Apprentice, there and then.
    let levelled: Vec<bool> = (0..5).map(|_| librarian.reward_trade_xp(2)).collect();
    assert_eq!(levelled, [false, false, false, false, true]);
    assert_eq!((librarian.level, librarian.xp), (2, 10));
    let mut master = Hand::new(&c, c.trade("librarian").unwrap(), 5);
    assert_eq!(master.xp, 250, "a Master has the XP it took");
    assert!(!master.reward_trade_xp(1000) && master.level == 5, "nothing past Master");
}

// parity-partial: GrowthTests.childrenGrowUpWhileAway
#[test]
fn a_child_grows_up_by_the_time_elapsed() {
    let c = content();
    let born = 5 * DAY + 77;
    let mut child = Hand::baby(&c, born);
    assert!(child.is_baby(), "born a baby");
    assert_eq!(child.trade, c.trade("none").unwrap(), "with no trade");
    assert!(!child.grow_up(born + 12_000));
    assert!(child.is_baby() && child.age(born + 12_000) <= -11_000, "half grown after half a day: {}", child.age(born + 12_000));
    assert!(child.grow_up(born + 30_000), "grown after a day away");
    assert!(!child.is_baby() && child.age(born + 30_000) == 0);
    assert_eq!(BABY_TICKS, DAY);
}

#[test]
fn a_child_is_a_resident_not_a_worker() {
    let c = content();
    let farmer = c.trade("farmer").unwrap();
    let mut r = roster::Roster::new(&c, vec![Hand::new(&c, farmer, 2), Hand::new(&c, farmer, 4)]);
    r.add(&c, Hand::baby(&c, 0));
    assert_eq!((r.residents(), r.workers(), r.babies()), (3, 2, 1));
    assert_eq!(r.count(c.trade("none").unwrap()), 0, "a child isn't an idle adult");
    assert!(!r.travels(2), "nor goes with a caravan");
    assert_eq!(r.grow_up(BABY_TICKS), 1);
    assert_eq!((r.residents(), r.workers(), r.babies()), (3, 3, 0));
}

#[test]
fn the_roster_answers_who_is_who() {
    let c = content();
    let mut s = Society::new(c.clone());
    let t = s.add_town("Meadowworth", 10.0);
    s.set_roster(t, &[("merchant", 2), ("none", 1), ("librarian", 3), ("nitwit", 1), ("librarian", 3),
        ("merchant", 4), ("minecraft:librarian", 4)]);
    let r = s.roster(t);
    let librarian = c.trade("librarian").unwrap();
    assert_eq!(r.count(librarian), 3);
    assert_eq!(r.levels(librarian), [3, 3, 4]);
    assert_eq!(r.best(librarian), Some(6), "the most experienced");
    assert_eq!(r.best(c.trade("farmer").unwrap()), None);
    assert_eq!(r.merchant, Some(5), "the desk's Merchant is the most experienced Merchant");
    assert!(!r.travels(5) && r.travels(0), "the desk's Merchant never travels; another Merchant may");
    assert_eq!(r.idle(&c), [3, 1], "idle hands at home, Nitwits first");
    // A hand away isn't idle at home.
    let mut away = r.clone();
    away.hands[1].away = true;
    assert_eq!(away.idle(&c), [3]);
    // One more hand; a Merchant becomes the desk's only if it has none.
    assert_eq!(s.add_hand(t, "merchant", 5), 7);
    assert_eq!(s.roster(t).merchant, Some(5));
    let fresh = s.add_town("Hayford", 0.0);
    assert_eq!(s.add_hand(fresh, "farmer", 1), 0, "a town without a roster starts one");
    assert_eq!(s.add_hand(fresh, "merchant", 1), 1);
    assert_eq!(s.roster(fresh).merchant, Some(1));
    assert_eq!(s.town(fresh).people.villagers, 2.0);
}

#[test]
#[should_panic(expected = "no trade called wizard")]
fn an_unknown_trade_is_refused() {
    let mut s = Society::new(content());
    let t = s.add_town("Meadowworth", 10.0);
    s.set_roster(t, &[("wizard", 1)]);
}

#[test]
fn villagers_follow_the_roster() {
    let mut s = Society::new(content());
    let t = s.add_town("Meadowworth", 10.0);
    s.set_people(t, 3.0, 8.0, 8.0);
    s.set_roster(t, &[("farmer", 1), ("farmer", 2), ("mason", 1), ("cleric", 3), ("nitwit", 1)]);
    assert_eq!(s.town(t).people.villagers, 5.0, "at once");
    s.set_people(t, 9.0, 8.0, 8.0); // a host's count doesn't hold against a roster
    s.advance(DAY);
    assert_eq!(s.town(t).people.villagers, 5.0, "and at every look");
    s.add_hand(t, "toolsmith", 1);
    assert_eq!(s.town(t).people.villagers, 6.0);
    // A town without a roster keeps its count.
    let other = s.add_town("Hayford", 10.0);
    s.set_people(other, 4.0, 4.0, 4.0);
    s.advance(2 * DAY);
    assert_eq!(s.town(other).people.villagers, 4.0);
}

/// A content town with a roster of six, room to grow, every comfort and bread: it has children.
fn growing_town() -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let t = s.add_town("Meadowworth", 60.0);
    s.set_people(t, 6.0, 6.0, 40.0);
    s.set_roster(t, &[("merchant", 1), ("farmer", 2), ("farmer", 1), ("librarian", 1), ("mason", 1), ("cleric", 1)]);
    s.set_trade(t, WHEAT, 128.0, 0.0);
    for k in 0..5 {
        s.deliver(t, k, (6.0 * 0.1 * 60.0 / 0.05f64).ceil());
    }
    s.deliver(t, BREAD, 704.0);
    s
}

#[test]
fn a_child_born_in_a_town_with_a_roster_is_a_baby_hand() {
    let mut s = growing_town();
    let start = s.totals();
    let none = s.content().trade("none").unwrap();
    let mut first_born = None;
    for day in 1..=60 {
        s.advance(day * DAY);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("day {day}: {e}"));
        let (r, p) = (s.roster(0), s.town(0).people);
        assert_eq!(r.residents() as f64, p.villagers, "day {day}: villagers are the roster's residents");
        assert_eq!(r.residents() as u32, 6 + p.born, "day {day}: a hand for every child");
        if first_born.is_none() && p.born > 0 {
            first_born = Some(day);
            assert_eq!(r.babies(), 1, "born a baby");
            assert_eq!(r.hands.last().unwrap().trade, none, "with no trade");
        }
        if let Some(b) = first_born {
            if day >= b + 2 {
                assert!(!r.hands[6].is_baby(), "day {day}: grown up a day after it was born (day {b})");
            }
        }
    }
    let p = s.town(0).people;
    assert!(p.born >= 2 && p.villagers >= 8.0, "it grew as a town without a roster does: {} born", p.born);
    assert_eq!(s.roster(0).workers() as f64 + s.roster(0).babies() as f64, p.villagers);
}
