//! Property (`property.rs`, `rules/property.rs`): fields are one household's, bought, worked, let, handed on, sold and
//! homesteaded; owners, tenants and labourers in one village, and the disputes left open.

use enterprise_core::cash_book::{Account, Reason};
use enterprise_core::charters::Party;
use enterprise_core::fixed::Fixed;
use enterprise_core::households::{gather, pay, Household, HouseholdId};
use enterprise_core::property::{Acquired, Contest, Standing, Target};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;
const WHEAT: usize = 0;

fn content() -> Content {
    Content::new(vec![Good::new("wheat", 1.0 / 20.0, 64).crop(), Good::new("bread", 1.0 / 6.0, 64).food(5.0)], DAY)
}

/// A town of these hands (level 3) in `beds` beds, 20 plots of wheat, its size costing no mood, no earnings from outside.
fn village(hands: &[&str], beds: f64) -> (Society, usize) {
    let mut s = Society::new(content());
    s.content_mut().dials.villagers_per_want = f64::INFINITY;
    s.content_mut().dials.hand_wage = 0.0;
    let t = s.add_town("Hearthglen", 0.0);
    let roster: Vec<(&str, u8)> = hands.iter().map(|h| (*h, 3)).collect();
    s.set_roster(t, &roster);
    let v = s.town(t).people.villagers;
    s.set_people(t, v, beds, beds);
    s.set_plots(t, WHEAT, 20);
    s.set_farm_demand(t, WHEAT, 200.0); // (wanted: its fields are tended, not left fallow, and stay freeholds)
    s.deliver(t, 1, 640.0);
    (s, t)
}

/// Runs `days` more days.
fn run(s: &mut Society, days: u64) {
    let now = s.now();
    s.advance(now + days * DAY);
}

/// The hands' ids, in the roster's order.
fn ids(s: &Society, t: usize) -> Vec<u32> {
    s.roster(t).hands.iter().map(|h| h.id).collect()
}

/// The household a hand lives in.
fn home_of(s: &Society, t: usize, hand: u32) -> Household {
    s.households(t).into_iter().find(|h| h.members.contains(&hand)).expect("housed")
}

fn standing(s: &Society, t: usize, h: HouseholdId) -> Standing {
    s.standings(t).into_iter().find(|(x, _)| *x == h).map(|(_, st)| st).expect("a household")
}

fn fields_of(s: &Society, owner: HouseholdId) -> Vec<(u32, Acquired)> {
    let r = s.land_register();
    r.fields.keys().filter_map(|f| r.claims.get(&Target::Field(*f)).filter(|c| c.owner == Party::Household(owner))
        .map(|c| (*f, c.how))).collect()
}

#[test]
fn shares_move_between_households_and_still_add_up() {
    let h = |purse: i64| Household { members: vec![1], purse: Fixed::emeralds(purse), ..Default::default() };
    let mut list = vec![h(10), h(30), h(0)];
    assert_eq!(gather(&mut list, 2, Fixed::emeralds(8)), Fixed::emeralds(8));
    assert_eq!(list.iter().map(|h| h.purse).collect::<Vec<_>>(), vec![Fixed::emeralds(8), Fixed::emeralds(24), Fixed::emeralds(8)],
        "out of the others by what each holds");
    assert_eq!(pay(&mut list, 2, 0, Fixed::emeralds(100)), Fixed::emeralds(8), "no more than it holds");
    assert_eq!(list.iter().map(|h| h.purse).sum::<Fixed>(), Fixed::emeralds(40));
}

#[test]
fn each_farmer_household_that_can_pay_buys_a_field_of_its_own_and_pays_for_it_itself() {
    let (mut s, t) = village(&["farmer", "farmer"], 4.0);
    run(&mut s, 1);
    s.post(Account::Outside, Account::Households(t), 20.0, Reason::PaidIn);
    run(&mut s, 4);
    // The richer first (they were alike: the first), then the other, now the richer: each paid its own price.
    let [a, b] = ids(&s, t)[..] else { panic!() };
    let (ha, hb) = (home_of(&s, t, a), home_of(&s, t, b));
    assert_eq!(fields_of(&s, ha.id).iter().map(|x| x.1).collect::<Vec<_>>(), vec![Acquired::Bought]);
    assert_eq!(fields_of(&s, hb.id).iter().map(|x| x.1).collect::<Vec<_>>(), vec![Acquired::Bought]);
    assert_eq!(s.cash_book(t, 10).iter().filter(|x| x.1 == "land").count(), 2, "no more than its Farmer works");
    // (Ten each, eight of it paid, and their fields' wages since, out of the sixteen the town took for them.)
    assert_eq!(ha.purse, hb.purse, "alike: each paid its own");
    assert!(ha.purse < Fixed::emeralds(10));
    assert_eq!((standing(&s, t, ha.id), standing(&s, t, hb.id)), (Standing::Owner, Standing::Owner));
}

/// [Merchant, Farmer, Farmer] in two homes: the Merchant and the second Farmer share one.
fn bought_by_the_shared_household() -> (Society, usize, [u32; 3]) {
    let (mut s, t) = village(&["merchant", "farmer", "farmer"], 4.0);
    run(&mut s, 1);
    s.post(Account::Outside, Account::Households(t), 20.0, Reason::PaidIn);
    run(&mut s, 3);
    let [m, f1, f2] = ids(&s, t)[..] else { panic!() };
    assert_eq!(home_of(&s, t, m).id, home_of(&s, t, f2).id, "the Merchant and the second Farmer at home together");
    assert_eq!(fields_of(&s, home_of(&s, t, m).id).len(), 1, "theirs the richer: their field");
    (s, t, [m, f1, f2])
}

#[test]
fn a_household_with_no_farmer_for_its_field_lets_it_to_a_labourer_s() {
    let (mut s, t, [m, f1, f2]) = bought_by_the_shared_household();
    s.lose_hand(f2).unwrap();
    run(&mut s, 2);
    let (owner, tenant) = (home_of(&s, t, m).id, home_of(&s, t, f1).id);
    let r = s.land_register();
    let (f, _) = fields_of(&s, owner)[0];
    assert_eq!(r.leases.get(&f).map(|l| l.tenant), Some(tenant), "let to the Farmer's household: {:?}", r.leases);
    assert_eq!((standing(&s, t, owner), standing(&s, t, tenant)), (Standing::Landlord, Standing::Tenant));
}

#[test]
fn a_household_dying_out_with_no_kin_leaves_its_field_to_the_town_and_its_tenant_a_dispute() {
    let (mut s, t, [m, f1, f2]) = bought_by_the_shared_household();
    let freeholds = s.farm(t).unwrap().farm.freeholds;
    s.lose_hand(f2).unwrap();
    run(&mut s, 2);
    let tenant = home_of(&s, t, f1).id;
    s.lose_hand(m).unwrap();
    run(&mut s, 2);
    let r = s.land_register();
    assert!(r.fields.is_empty(), "back in the estate: {:?}", r.fields);
    assert!(s.farm(t).unwrap().farm.freeholds < freeholds);
    let d = r.disputes.last().expect("a dispute");
    assert_eq!((d.why, d.claimants.clone()), (Contest::Inheritance, vec![Party::Household(tenant), Party::Town(t)]));
}

#[test]
fn a_field_its_household_can_t_work_is_sold_to_its_tenant_once_it_can_pay() {
    let (mut s, t, [m, f1, f2]) = bought_by_the_shared_household();
    s.content_mut().dials.idle_sale_days = 2.0;
    let seller = home_of(&s, t, m).id;
    s.lose_hand(f1).unwrap();
    s.lose_hand(f2).unwrap();
    run(&mut s, 4);
    assert!(s.land_register().fields.values().all(|f| f.worked_by.is_none()), "no one to work it");
    s.add_hand(t, "farmer", 3);
    run(&mut s, 1);
    let buyer = s.households(t).into_iter().find(|h| h.id != seller).expect("the new Farmer's").id;
    assert_eq!(standing(&s, t, buyer), Standing::Tenant, "it rents it while it can't buy");
    s.post(Account::Outside, Account::Households(t), 200.0, Reason::PaidIn);
    run(&mut s, 2);
    assert_eq!(fields_of(&s, buyer).iter().map(|x| x.1).collect::<Vec<_>>(), vec![Acquired::Purchased]);
    assert!(fields_of(&s, seller).is_empty());
    assert_eq!(standing(&s, t, buyer), Standing::Owner);
}

/// The fields homesteaded and the town's disputes, `farmers` Farmers with `plots` plots after six days.
fn homesteading(farmers: usize, plots: u32) -> (Society, Vec<Party>) {
    let (mut s, t) = village(&vec!["farmer"; farmers], 2.0 * farmers as f64);
    s.content_mut().dials.homestead_days = 2.0;
    s.set_plots(t, WHEAT, plots);
    s.set_farm_demand(t, WHEAT, 1000.0);
    run(&mut s, 6);
    let r = s.land_register();
    let owners = r.claims.iter().filter(|(k, c)| matches!(k, Target::Field(_)) && c.how == Acquired::Homesteaded)
        .map(|(_, c)| c.owner).collect();
    (s, owners)
}

#[test]
fn estate_land_enough_for_each_is_homesteaded_in_turn_with_no_dispute() {
    let (s, owners) = homesteading(3, 600);
    assert!(owners.len() >= 2, "idle: {:?}", s.farm(0).unwrap().farm.idle);
    assert!(s.land_register().disputes.is_empty(), "{:?}", s.land_register().disputes);
}

#[test]
fn estate_land_too_little_for_all_that_could_take_it_is_disputed() {
    let (s, owners) = homesteading(6, 800);
    let r = s.land_register();
    let d = r.disputes.iter().find(|d| d.why == Contest::Homestead).expect("a dispute");
    assert!(d.claimants.len() >= 2 && owners.iter().any(|o| d.claimants.contains(o)), "{d:?}");
    assert!(s.farm(0).unwrap().farm.freeholds >= r.fields.values().map(|f| f.plots).sum::<u32>());
}
