//! War on the engine of people at work (`war.rs`, `docs/firms-plan.md` slice 5): a soldier's fundamentals, its kit's
//! grades among them; a skirmish every few campaign days against a target with a twist, its captain by the crown's
//! mandate; won, the army's risk less; lost with its morale broken, its low-spirited rout.

use enterprise_core::army::{self, FOODS};
use enterprise_core::persons::{Grade, Person, Work};
use enterprise_core::war::{fundamentals, Twist};
use enterprise_core::{Content, Good, Hex, Society};

const DAY: u64 = 24_000;

fn content() -> Content {
    let mut goods: Vec<Good> = FOODS.iter().map(|f| army::food_good(f.0, 64)).collect();
    for g in army::WEAPONS.iter().chain(army::ARMOUR.iter().flatten()).chain([&army::SHIELD]) {
        goods.push(Good::priced(g.name, 1));
    }
    goods.push(Good::priced("arrow", 64));
    Content::new(goods, DAY)
}

fn good(c: &Content, name: &str) -> usize {
    c.good(name).unwrap_or_else(|| panic!("no good called {name}"))
}

/// A town of `n` idle hands (level 3) and its Merchant, and the front off the map, its army enlisted and at the
/// front with `swords` iron swords (all at `grade`, if any) and bread for the campaign.
fn campaign(s: &mut Society, n: usize, swords: f64, grade: Option<Grade>) -> usize {
    let home = s.add_town("Home", 0.0);
    let mut hands = vec![("merchant", 1)];
    hands.extend(std::iter::repeat_n(("none", 3), n));
    s.set_roster(home, &hands);
    let hex = Hex::new(0, 0);
    s.place_town(home, hex.center_x(), hex.center_z());
    let front = s.add_off_map("Front", hex.center_x(), hex.center_z() - 1500.0, 0.0);
    let (sword, bread) = (good(s.content(), "iron_sword"), good(s.content(), "bread"));
    s.deliver(front, sword, swords);
    if let Some(g) = grade {
        s.set_graded(front, sword, g, swords);
    }
    s.deliver(front, bread, 2000.0);
    assert_eq!(s.enlist(home, n), n);
    s.set_front(front);
    front
}

#[test]
fn a_soldier_brings_its_skill_its_kit_and_its_kit_s_grade() {
    let c = content();
    let sword = good(&c, "iron_sword");
    let bow = good(&c, "bow");
    let p = Person { skills: [(Work::Arms, 9.0), (Work::Archery, 9.0)].into(), ..Default::default() };
    let unarmed = army::Soldier::default();
    let mut armed = army::Soldier { weapon: Some(army::Worn { good: sword, ..Default::default() }), ..Default::default() };
    assert_eq!(fundamentals(&c, &unarmed, &p, Twist::Open), 5.0, "its skill, four less unarmed");
    assert_eq!(fundamentals(&c, &armed, &p, Twist::Open), 11.0, "an iron sword's 6, two past a wooden one's");
    armed.weapon.as_mut().unwrap().grade = Some(Grade::Fine);
    assert_eq!(fundamentals(&c, &armed, &p, Twist::Open), 15.0, "a Fine sword: four more");
    let archer = army::Soldier { weapon: Some(army::Worn { good: bow, ..Default::default() }), arrows: 10, ..Default::default() };
    assert_eq!(fundamentals(&c, &archer, &p, Twist::Fog), fundamentals(&c, &archer, &p, Twist::Open) / 2.0, "fog");
    assert!(Twist::ALL.iter().all(|t| !t.tooltip().is_empty()));
}

#[test]
fn every_few_days_the_army_fights_a_skirmish_under_its_captain_by_the_crown_s_mandate() {
    let mut s = Society::new(content());
    campaign(&mut s, 10, 10.0, None);
    s.advance(11 * DAY);
    let a = s.army().clone();
    assert_eq!(a.skirmishes.len(), 2, "on campaign days 5 and 10: {:?}", a.skirmishes);
    let first = &a.skirmishes[0];
    assert_eq!(first.target, s.content().dials.skirmish_target * first.soldiers as f64);
    let captain = a.captain.expect("a captain");
    let m = s.charters().mandates.iter().find(|m| m.id == captain.mandate).cloned().unwrap();
    assert_eq!((m.principal, m.agent, m.until), (enterprise_core::charters::Party::Crown, captain.hand, None));
    assert!(a.soldiers.iter().any(|x| x.hand.as_ref().is_some_and(|h| h.id == captain.hand)));
}

#[test]
fn a_won_skirmish_halves_the_army_s_risk_and_a_lost_one_raises_it() {
    let edge = |target: f64| {
        let mut s = Society::new(content());
        s.content_mut().dials.skirmish_target = target;
        campaign(&mut s, 10, 10.0, None);
        s.advance(6 * DAY);
        let a = s.army().clone();
        (a.skirmishes[0].won, a.edge)
    };
    assert_eq!(edge(0.1), (true, -0.5));
    assert_eq!(edge(1000.0), (false, 0.5));
}

#[test]
fn fine_swords_fight_better_than_plain_ones() {
    let score = |grade: Option<Grade>| {
        let mut s = Society::new(content());
        campaign(&mut s, 10, 10.0, grade);
        s.advance(6 * DAY);
        let a = s.army().clone();
        assert!(a.soldiers.iter().all(|x| x.weapon.as_ref().is_some_and(|w| w.grade == grade)), "{:?}", a.soldiers[0].weapon);
        a.skirmishes[0].score
    };
    let (plain, fine) = (score(None), score(Some(Grade::Fine)));
    assert!(fine > plain * 1.2, "{plain} plain, {fine} Fine");
}

#[test]
fn an_army_whose_morale_breaks_routs_its_low_spirited_but_never_a_steady_hand() {
    use enterprise_core::persons::Habit;
    let mut s = Society::new(content());
    s.content_mut().dials.skirmish_target = 1000.0;
    s.content_mut().dials.rout_morale = 10.0;
    campaign(&mut s, 20, 20.0, None);
    s.advance(40 * DAY);
    let a = s.army().clone();
    let routed: u32 = a.skirmishes.iter().map(|k| k.routed).sum();
    assert!(routed > 0 && a.skirmishes.iter().all(|k| !k.won), "{:?}", a.skirmishes);
    for x in &a.soldiers {
        let m = s.person(x.hand.as_ref().unwrap().id).unwrap();
        assert!(m.spirits >= 0.35 || m.has(Habit::SteadyHand) || m.knack.is_some(), "{m:?}");
    }
}
