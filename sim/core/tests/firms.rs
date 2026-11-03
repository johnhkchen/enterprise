//! Firms (`firms.rs`, `rules/firms.rs`): a household's capital put into a bakery, a weaver's or a tannery that makes
//! the finer tier of the town's goods with hands it hires away from the town's wages, where enough people live within
//! its range; and wound up when it can't pay them.

use enterprise_core::cash_book::{Account, Reason};
use enterprise_core::charters::Party;
use enterprise_core::fixed::Fixed;
use enterprise_core::persons::{draw, Grade, Habit, Talent, Work};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;
const WHEAT: usize = 0;
const BREAD: usize = 1;

fn content() -> Content {
    Content::new(vec![Good::new("wheat", 0.05, 64).crop(), Good::new("bread", 0.2, 64).food(5.0)], DAY)
}

/// Runs `days` more days.
fn run(s: &mut Society, days: u64) {
    let now = s.now();
    s.advance(now + days * DAY);
}

/// A town of `villagers` people at `at` (its Merchant and a hand away with its wagon count for none): these hands
/// (level 3), then nitwits, in a home for each of these hands and no
/// land to build more, with wheat to bake and `purse` emeralds in its households' purse.
fn town(s: &mut Society, name: &str, villagers: usize, at: (f64, f64), hands: &[&str], purse: f64) -> usize {
    town_of(s, name, villagers, at, hands, 3, purse)
}

/// As [`town`], its hands of this level.
fn town_of(s: &mut Society, name: &str, villagers: usize, at: (f64, f64), hands: &[&str], level: u8, purse: f64) -> usize {
    let t = s.add_town(name, 500.0);
    let mut roster: Vec<(&str, u8)> = hands.iter().map(|h| (*h, level)).collect();
    roster.resize(villagers, ("nitwit", 1));
    s.set_roster(t, &roster);
    let beds = 2.0 * hands.len() as f64;
    s.set_people(t, villagers as f64, beds, beds);
    s.place_town(t, at.0, at.1);
    s.deliver(t, WHEAT, 600.0);
    s.post(Account::Outside, Account::Households(t), purse, Reason::PaidIn);
    t
}

fn society() -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.villagers_per_want = f64::INFINITY;
    s
}

fn bakeries(s: &Society, t: usize) -> usize {
    s.firms().iter().filter(|f| f.town == t && f.name == "bakery").count()
}

#[test]
fn a_bakery_opens_where_enough_people_live_within_its_range_and_not_in_the_small_town_alone() {
    let mut s = society();
    let big = town(&mut s, "Stormwind", 44, (0.0, 0.0), &["farmer", "merchant"], 200.0);
    let small = town(&mut s, "Goldshire", 20, (5000.0, 0.0), &["farmer", "merchant"], 200.0);
    // As small, but a village of 25 within 150 blocks: 43 within its range.
    let near = town(&mut s, "Lakeshire", 20, (10_000.0, 0.0), &["farmer", "merchant"], 200.0);
    town(&mut s, "Darkshire", 25, (10_100.0, 0.0), &["merchant"], 0.0);
    run(&mut s, 3);
    assert_eq!((bakeries(&s, big), bakeries(&s, small), bakeries(&s, near)), (1, 0, 1), "{:?}", s.firms());
}

#[test]
fn a_bakery_hires_a_farmer_bakes_the_finer_bread_its_town_eats_and_pays_its_hand_s_household() {
    let mut s = society();
    let t = town(&mut s, "Stormwind", 44, (0.0, 0.0), &["farmer", "farmer", "merchant"], 300.0);
    let before = s.totals();
    // (Its wheat as a farm brings it in, a day's at a time: what's left of a delivery the workshop bakes or sells.)
    let mut lift: f64 = 0.0;
    for _ in 0..6 {
        s.deliver(t, WHEAT, 60.0);
        run(&mut s, 1);
        lift = lift.max(s.town(t).people.meal_fine);
    }
    s.conserved(&before).unwrap();
    let firm = s.firms().into_iter().next().expect("a bakery");
    assert_eq!(firm.owners.len(), 1);
    assert!(firm.made > 0.0 && !firm.employees.is_empty(), "{firm:?}");
    let hand = firm.employees[0].0;
    // Its Farmer earns its wage, not a share of the town's.
    let theirs = s.households(t).into_iter().find(|h| h.members.contains(&hand)).unwrap();
    assert!(theirs.purse.is_positive());
    // Its capital the town's, its bread sold to the town, its wheat bought of it; its wages into the households' purse.
    let (book, theirs) = (s.cash_book(t, 7), s.account_book(Account::Households(t), 7));
    for (why, of) in [("firm capital", &book), ("firm sales", &book), ("firm inputs", &book), ("firm wages", &theirs)] {
        assert!(of.iter().any(|x| x.1 == why), "{why}: {of:?}");
    }
    // The finer bread is eaten first, lifts the mood as a comfort's finer tier does, and is paid for so (this town has
    // no other comforts).
    assert!(lift > 0.0);
    assert!(book.iter().any(|x| x.1 == "comforts"), "{book:?}");
    assert_eq!(s.households(t).iter().map(|h| h.purse).sum::<Fixed>(), s.town(t).people.purse, "the shares add up");
}

#[test]
fn two_households_neither_of_which_can_pay_alone_found_it_together() {
    let mut s = society();
    let t = town(&mut s, "Stormwind", 44, (0.0, 0.0), &["farmer", "farmer"], 60.0);
    run(&mut s, 3);
    let firm = s.firms().into_iter().find(|f| f.town == t).expect("a bakery");
    assert_eq!(firm.owners.len(), 2, "{firm:?}");
    assert!((firm.owners.iter().map(|o| o.1).sum::<f64>() - 1.0).abs() < 1e-9);
    assert!(firm.owners.iter().all(|o| matches!(o.0, Party::Household(_))));
}

#[test]
fn no_firm_where_a_hand_earns_more_of_the_town_than_its_work_there_would_pay() {
    let mut s = society();
    s.content_mut().dials.hand_wage = 5.0;
    let t = town(&mut s, "Stormwind", 44, (0.0, 0.0), &["farmer", "merchant"], 200.0);
    run(&mut s, 3);
    assert_eq!(bakeries(&s, t), 0);
}

#[test]
fn a_bakery_with_no_wheat_left_can_t_pay_its_hand_and_is_wound_up() {
    let mut s = society();
    let t = town(&mut s, "Stormwind", 44, (0.0, 0.0), &["farmer", "merchant"], 200.0);
    run(&mut s, 2);
    let id = s.firms().first().expect("a bakery").id;
    let wheat = s.town(t).stock[WHEAT];
    s.deliver(t, WHEAT, -wheat);
    let bread = s.town(t).stock[BREAD];
    s.deliver(t, BREAD, -bread);
    let before = s.totals();
    run(&mut s, 40);
    s.conserved(&before).unwrap();
    let (gone, _) = s.firms_wound_up().into_iter().find(|(f, _)| f.id == id).expect("wound up");
    assert!(gone.broke_days as f64 >= s.content().dials.firm_broke_days);
    assert!(s.firms().iter().all(|f| f.id != id));
}



#[test]
fn a_village_beside_a_bigger_town_with_a_bakery_opens_none_of_its_own() {
    let mut s = society();
    let big = town(&mut s, "Stormwind", 44, (0.0, 0.0), &["farmer", "merchant"], 200.0);
    let village = town(&mut s, "Goldshire", 10, (100.0, 0.0), &["farmer", "merchant"], 200.0);
    run(&mut s, 3);
    assert_eq!((bakeries(&s, big), bakeries(&s, village)), (1, 0), "{:?}", s.firms());
}

/// A bakery in a big town (60 eat 24 loaves a day, two hands' baking) whose owners run one hand themselves, fed its
/// wheat for `days` days: it takes on more.
fn busy_bakery(s: &mut Society, days: u64) -> usize {
    s.content_mut().dials.firm_hands_unled = 1.0;
    let t = town(s, "Stormwind", 60, (0.0, 0.0), &["farmer", "farmer", "farmer", "farmer", "farmer", "merchant"], 400.0);
    for _ in 0..days {
        s.deliver(t, WHEAT, 120.0);
        run(s, 1);
    }
    t
}

#[test]
fn a_person_s_make_up_is_its_own_two_habits_that_don_t_clash_and_skill_by_rank() {
    let all = vec!["wheat".to_string(), "leather".to_string()];
    for h in 1..300 {
        let p = draw(h, 3, &all);
        assert_eq!(p, draw(h, 3, &all), "the same hand, the same person");
        assert_eq!(p.habits.len(), 2);
        assert_ne!(p.habits[0], p.habits[1]);
        assert!(!(p.has(Habit::Taskmaster) && p.has(Habit::Beloved)) && !(p.has(Habit::Ambitious) && p.has(Habit::Homebody)));
        assert!(!p.talents.is_empty() && p.talents.iter().all(|t| matches!(t, Talent::Work(_))
            || matches!(t, Talent::Material(m) if all.contains(m))));
        assert!(Work::ALL.iter().all(|&w| p.skill(w) == 9.0));
    }
    let habits: std::collections::BTreeSet<_> = (1..300).flat_map(|h| draw(h, 3, &all).habits).collect();
    assert_eq!(habits.len(), Habit::ALL.len(), "every habit turns up");
}

#[test]
fn a_task_scores_fundamentals_times_morale_and_the_score_sets_the_grade() {
    use enterprise_core::persons::{morale, score};
    assert_eq!(score(12.0, 1.0, 0.3, 0.5), 12.0);
    assert_eq!(Grade::of(score(12.0, 1.0, 0.3, 0.5)), Grade::Good);
    assert_eq!(Grade::of(score(12.0, 1.5, 0.3, 0.5)), Grade::Fine, "in good heart, finer");
    assert!(morale(0.9, None) > morale(0.5, None) && morale(0.5, None) > morale(0.1, None));
    let d = Content::new(vec![], DAY).dials;
    let worths: Vec<f64> = Grade::ALL.iter().map(|g| g.worth(&d)).collect();
    assert!(worths.windows(2).all(|w| w[0] < w[1]) && Grade::Plain.worth(&d) == 1.0);
}

#[test]
fn graded_pieces_are_used_best_first_and_seconds_last() {
    use enterprise_core::people::take_graded;
    let mut graded = std::collections::BTreeMap::new();
    graded.insert(1, [2.0, 0.0, 1.0, 0.0, 1.0]); // 2 seconds, 1 good, 1 masterwork, of 10 held: 6 plain
    let took = take_graded(&mut graded, 1, 10.0, 8.0);
    assert_eq!(took, [0.0, 6.0, 1.0, 0.0, 1.0]);
    assert_eq!(take_graded(&mut graded, 1, 2.0, 2.0), [2.0, 0.0, 0.0, 0.0, 0.0]);
    assert!(graded.is_empty());
}

#[test]
fn a_master_baker_s_bread_grades_finer_than_a_novice_s() {
    let average = |level: u8| {
        let mut s = society();
        s.content_mut().dials.hand_wage = 0.01;
        let t = town_of(&mut s, "Stormwind", 44, (0.0, 0.0), &["farmer", "merchant"], level, 200.0);
        for _ in 0..10 {
            s.deliver(t, WHEAT, 120.0);
            run(&mut s, 1);
        }
        let f = s.firms().into_iter().next().expect("a bakery");
        let pieces: f64 = f.grades.iter().sum();
        assert!(pieces > 0.0);
        Grade::ALL.iter().map(|g| f.grades[g.index()] * g.index() as f64).sum::<f64>() / pieces
    };
    let (novice, master) = (average(1), average(5));
    assert!(master > novice + 1.0, "{novice} {master}");
}

#[test]
fn past_the_hands_its_owners_run_a_firm_appoints_its_most_skilled_head_by_mandate_and_grows() {
    let mut s = society();
    busy_bakery(&mut s, 12);
    let firm = s.firms().into_iter().next().expect("a bakery");
    let head = firm.head.expect("a head");
    assert!(firm.employees.len() as f64 > s.content().dials.firm_hands_unled, "{firm:?}");
    let c = s.charters();
    let m = c.mandates.iter().find(|m| m.id == head.mandate).unwrap();
    assert_eq!((m.principal, m.agent, m.scope.clone(), m.until), (Party::Firm(firm.id), head.hand,
        vec![enterprise_core::charters::Verb::Hire, enterprise_core::charters::Verb::Command], None));
    let paid = firm.employees.iter().find(|e| e.0 == head.hand).unwrap().1;
    assert!(firm.employees.iter().all(|e| e.1 <= paid) && firm.employees.iter().any(|e| e.1 < paid), "paid more");
    assert!(firm.events.iter().any(|e| e.1 == head.hand && e.2 == enterprise_core::persons::Event::Appointed));
}

#[test]
fn a_head_s_term_ends_and_its_owners_appoint_again() {
    let mut s = society();
    s.content_mut().dials.firm_head_term_days = 2.0;
    busy_bakery(&mut s, 14);
    let firm = s.firms().into_iter().next().expect("a bakery");
    let c = s.charters();
    let of: Vec<_> = c.mandates.iter().filter(|m| m.principal == Party::Firm(firm.id)).collect();
    assert!(of.len() >= 2 && of.iter().filter(|m| m.until.is_none()).count() <= 1, "{of:?}");
}

#[test]
fn hands_kept_in_low_spirits_go_off_form_and_it_shows_in_the_business_s_record() {
    let mut s = society();
    let t = busy_bakery(&mut s, 4);
    for _ in 0..30 {
        let hands: Vec<u32> = s.firms().iter().flat_map(|f| f.employees.iter().map(|e| e.0)).collect();
        for h in hands {
            if let Some(p) = s.world.resource_mut::<enterprise_core::persons::Persons>().hands.get_mut(&h) {
                p.spirits = 0.0;
            }
        }
        s.deliver(t, WHEAT, 120.0);
        run(&mut s, 1);
    }
    let firms: Vec<_> = s.firms().into_iter().chain(s.firms_wound_up().into_iter().map(|x| x.0)).collect();
    let events: Vec<_> = firms.iter().flat_map(|f| f.events.iter().map(|e| e.2.clone())).collect();
    use enterprise_core::persons::Event;
    assert!(events.iter().any(|e| matches!(e, Event::WalkedOut | Event::Quarrelled(_) | Event::Sulked)), "{events:?}");
    assert!(!events.contains(&Event::Inspired), "not in those spirits");
}
