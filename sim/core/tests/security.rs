//! Security (`security.rs`): a town keeps a watch it pays, its defence and protection follow from it, its kit and
//! its walls, and raids, theft and the night take what its protection doesn't keep off, out of the world.

use enterprise_core::security::{defence, needed, protection};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;

/// A town of `villagers` with `money`, a warehouse of wheat and bread.
fn town(villagers: f64, money: f64) -> (Society, usize) {
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 0.05, 64), Good::new("bread", 0.2, 64).food(5.0)], DAY));
    s.content_mut().dials.standing_orders = false;
    let t = s.add_town("Strongholm", money);
    s.set_people(t, villagers, villagers, villagers);
    s.deliver(t, 0, 2000.0);
    s.deliver(t, 1, 640.0);
    (s, t)
}

#[test]
fn protection_is_defence_over_what_a_town_of_its_size_needs() {
    let d = enterprise_core::Dials::default();
    assert_eq!(needed(&d, 40.0), 4.0, "a watchman for ten");
    assert_eq!(protection(0.0, 4.0), 0.0, "undefended");
    assert!((protection(4.0, 4.0) - 0.5).abs() < 1e-9, "half at par");
    // Four unarmed watchmen are half a watch; armed and armoured, half again; a whole wall is a full watch.
    let plain = defence(&d, 40.0, 4, 0, 0.0, 0.0, 0.0, false);
    assert!((plain - 2.0).abs() < 1e-9);
    assert!((defence(&d, 40.0, 4, 0, 1.0, 1.0, 0.0, false) - 6.0).abs() < 1e-9);
    assert!((defence(&d, 40.0, 0, 0, 0.0, 0.0, 1.0, false) - 4.0).abs() < 1e-9);
}

#[test]
fn a_town_keeps_a_watch_it_pays_and_a_hamlet_none() {
    let (mut s, t) = town(30.0, 500.0);
    s.advance(10 * DAY);
    let w = s.watch(t);
    assert_eq!(w.watchmen, 3, "a watchman for each ten villagers: {w:?}");
    assert!(s.cash_book(t, 30).iter().any(|(_, what, n)| what == "watch" && n.to_f64() < 0.0), "paid out of its treasury");
    assert!(w.protection > 0.0);
    let (mut s, t) = town(8.0, 500.0);
    s.advance(10 * DAY);
    assert_eq!(s.watch(t).watchmen, 0, "a hamlet knows its own");
}

#[test]
fn a_rich_undefended_town_is_raided_and_what_s_taken_leaves_the_world() {
    let (mut s, t) = town(30.0, 3000.0);
    s.content_mut().dials.guard_pay = 1e6; // (it can pay no watch)
    s.content_mut().dials.raid_chance = 1.0;
    let before = s.totals();
    let money = s.town(t).money.to_f64();
    s.advance(5 * DAY);
    let w = s.watch(t);
    assert!(w.raids >= 1 && w.raided_of.to_f64() > 0.0, "raided: {w:?}");
    assert!(s.town(t).money.to_f64() < money * 0.8, "of a share of its treasury: {}", s.town(t).money);
    assert!(s.town(t).people.mood < 0.0 || !s.town(t).people.fear.is_empty(), "and frightened");
    s.conserved(&before).unwrap();
}

#[test]
fn thieves_take_less_from_a_watched_town() {
    let stolen = |pay: f64| {
        let (mut s, t) = town(30.0, 2000.0);
        s.content_mut().dials.guard_pay = pay;
        s.content_mut().dials.raid_chance = 0.0;
        s.advance(20 * DAY);
        s.watch(t).stolen.to_f64()
    };
    let (watched, unwatched) = (stolen(0.25), stolen(1e6));
    assert!(unwatched > 0.0 && watched < unwatched, "a watch keeps thieves off: {watched} against {unwatched}");
}

#[test]
fn an_unarmed_watch_draws_a_weaponsmith_an_armorer_and_a_fletcher() {
    let goods = ["wheat", "bread", "iron_sword", "iron_chestplate", "bow", "iron_ingot", "stick", "string"].iter()
        .map(|n| Good::new(n, if *n == "bread" { 0.2 } else { 0.5 }, 64)).collect();
    let mut s = Society::new(Content::new(goods, DAY));
    let t = s.add_town("Strongholm", 500.0);
    s.set_roster(t, &[("merchant", 1), ("farmer", 1), ("none", 1), ("none", 1), ("none", 1), ("none", 1), ("none", 1),
        ("none", 1), ("none", 1), ("none", 1), ("none", 1), ("none", 1), ("none", 1), ("none", 1)]);
    s.deliver(t, 1, 640.0);
    s.content_mut().dials.secure_villagers = 2.0;
    s.content_mut().dials.watch_per_villagers = 2.0;
    s.advance(3 * DAY);
    assert!(s.watch(t).watchmen >= 1, "{:?}", s.watch(t));
    let (bag, roster) = (s.job_bag(t), s.roster(t));
    for trade in ["weaponsmith", "armorer", "fletcher"] {
        let hired = roster.count(s.content().trade(trade).unwrap()) > 0;
        assert!(hired || bag.iter().any(|(n, m)| n == trade && *m > 0.0), "{trade} for its watch's kit: {bag:?}");
    }
}
