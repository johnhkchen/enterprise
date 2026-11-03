//! Households and the registers (`households.rs`, `property.rs`, `charters.rs`): a town's hands live in households a
//! home each, whose purses split the households' purse; its hexes, halls and homes are claims; the crown charters it,
//! and its Merchant runs its trade by mandate.

use enterprise_core::charters::{Grant, Party, Verb};
use enterprise_core::fixed::Fixed;
use enterprise_core::households::{allot, Household};
use enterprise_core::property::{Acquired, Target};
use enterprise_core::works::{Kind, What};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;

/// A town of a Merchant and `farmers` Farmers in `beds` beds, with bread to eat.
fn town(farmers: usize, beds: f64) -> (Society, usize) {
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 0.05, 64), Good::new("bread", 0.2, 64).food(5.0)], DAY));
    s.content_mut().dials.standing_orders = false;
    let t = s.add_town("Hearthglen", 500.0);
    let mut hands = vec![("merchant", 2)];
    hands.extend(std::iter::repeat_n(("farmer", 1), farmers));
    s.set_roster(t, &hands);
    let v = s.town(t).people.villagers;
    s.set_people(t, v, beds, beds);
    s.deliver(t, 1, 640.0);
    (s, t)
}

fn purses(hs: &[Household]) -> Fixed {
    hs.iter().map(|h| h.purse).sum()
}

#[test]
fn each_hand_lives_in_one_household_a_home_each_and_their_purses_are_the_town_s() {
    let (mut s, t) = town(9, 10.0);
    s.advance(3 * DAY);
    let hs = s.households(t);
    assert_eq!(hs.len(), 5, "ten in ten beds: five homes of two: {hs:?}");
    let mut ids: Vec<_> = hs.iter().flat_map(|h| h.members.clone()).collect();
    ids.sort();
    let mut hands: Vec<_> = s.roster(t).hands.iter().map(|h| h.id).collect();
    hands.sort();
    assert_eq!(ids, hands, "each hand in exactly one");
    assert!(s.town(t).people.purse.is_positive(), "they've earned");
    assert_eq!(purses(&hs), s.town(t).people.purse, "their purses are the households' purse");
}

#[test]
fn a_household_s_share_of_a_gain_follows_its_earners_and_of_a_loss_what_it_holds() {
    let h = |members: Vec<u32>, purse: i64| Household { members, purse: Fixed::emeralds(purse), ..Default::default() };
    let list = [h(vec![1, 2], 10), h(vec![3], 30), h(vec![4], 0)];
    // A gain of 9: two earners, one, and a child.
    let got = allot(&list, &[2, 1, 0], Fixed::emeralds(49));
    assert_eq!(got, vec![Fixed::emeralds(16), Fixed::emeralds(33), Fixed::ZERO]);
    // A loss of 20: out of what each holds, a quarter and three quarters.
    let got = allot(&list, &[2, 1, 0], Fixed::emeralds(20));
    assert_eq!(got, vec![Fixed::emeralds(5), Fixed::emeralds(15), Fixed::ZERO]);
    // Thousandths that don't divide still add up.
    let got = allot(&list, &[1, 1, 1], Fixed::from_millis(40_001));
    assert_eq!(got.iter().copied().sum::<Fixed>(), Fixed::from_millis(40_001));
}

#[test]
fn a_hand_lost_leaves_its_household_and_one_left_empty_passes_what_it_held_to_the_rest() {
    let (mut s, t) = town(3, 4.0);
    s.advance(3 * DAY);
    let hs = s.households(t);
    assert_eq!(hs.len(), 2);
    let gone = hs[1].members.clone();
    for id in gone {
        s.lose_hand(id).unwrap();
    }
    s.advance(4 * DAY);
    let hs = s.households(t);
    assert_eq!(hs.len(), 1, "its household gone: {hs:?}");
    assert_eq!(hs[0].purse, s.town(t).people.purse, "and the rest hold all the purse");
}

#[test]
fn a_child_joins_a_household_and_takes_no_home_till_it_s_grown() {
    use enterprise_core::wire::Resident;
    let seen = |tag: &str, trade: &str, baby| Resident { tag: tag.into(), trade: trade.into(), level: 1, baby };
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64).food(5.0)], DAY));
    let t = s.add_town("Ashford", 10.0);
    // Three homes, a grown hand and two children.
    s.census(t, 6, &[seen("m", "enterprise:merchant", false), seen("c", "minecraft:none", true), seen("d", "minecraft:none", true)]);
    s.advance(100);
    let hs = s.households(t);
    assert_eq!(hs.iter().map(|h| h.members.len()).collect::<Vec<_>>(), vec![3], "the children take no home: {hs:?}");
    // Grown, one moves out to a home still empty: a home is two beds.
    s.advance(3 * DAY);
    let hs = s.households(t);
    assert_eq!(hs.iter().map(|h| h.members.len()).collect::<Vec<_>>(), vec![2, 1], "{hs:?}");
}

#[test]
fn a_town_s_hexes_halls_and_homes_are_claims_and_the_crown_charters_it() {
    let (mut s, t) = town(3, 4.0);
    let home = enterprise_core::Hex::new(0, 0);
    s.place_town(t, home.center_x(), home.center_z());
    s.claim_hex(t, home);
    s.start_project(t, What::Hall(Kind::Square));
    s.finish_project(t, What::Hall(Kind::Square));
    s.advance(2 * DAY);
    let r = s.land_register();
    let hex = r.claims.get(&Target::Hex(home)).expect("its hex claimed");
    assert_eq!((hex.owner, hex.how), (Party::Town(t), Acquired::Founding));
    assert_eq!(r.claims.get(&Target::Hall(t, Kind::Square)).map(|c| c.owner), Some(Party::Town(t)));
    for h in s.households(t) {
        assert_eq!(r.claims.get(&Target::Home(h.id)).map(|c| c.owner), Some(Party::Household(h.id)), "its home");
    }
    let c = s.charters();
    let charter = c.held_by(Party::Town(t)).expect("chartered");
    assert_eq!(charter.grantor, Party::Crown);
    assert!(charter.grants.contains(&Grant::Market) && charter.grants.contains(&Grant::SelfGovernment));
    assert_eq!(c.charters.len(), 1, "once");
}

#[test]
fn the_merchant_runs_its_town_s_trade_by_mandate_and_a_new_merchant_takes_it_over() {
    let (mut s, t) = town(3, 4.0);
    s.advance(2 * DAY);
    let roster = s.roster(t);
    let merchant = roster.hands[roster.merchant.unwrap()].id;
    let m: Vec<_> = s.charters().of(Party::Town(t)).cloned().collect();
    assert_eq!(m.len(), 1);
    assert_eq!((m[0].agent, m[0].scope.clone()), (merchant, vec![Verb::Trade]));
    s.lose_hand(merchant).unwrap();
    s.add_hand(t, "merchant", 1);
    s.advance(4 * DAY);
    let c = s.charters();
    let now: Vec<_> = c.of(Party::Town(t)).collect();
    assert_eq!(now.len(), 1);
    assert_ne!(now[0].agent, merchant, "the new Merchant's");
    assert!(c.mandates.iter().any(|m| m.agent == merchant && m.until.is_some()), "the old one's ended, and kept");
}
