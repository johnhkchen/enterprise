//! Training as a contract, and the tenders that find it work, pinned as the mod's game tests pin them
//! (`LogisticsTests`, `LockerTradingTests`): a town with a weaponsmith at some level, a capital far away that buys the
//! training, and the numbers of the Java fixtures (a bonus of 24 with a third up front, a level-2 smith to 5 is three levels,
//! six emeralds, eight days; forty in twenty days pays; an offtake crates twelve axes for the buyer and draws a bill).

use enterprise_core::bills::Held;
use enterprise_core::contracts::{Book, State};
use enterprise_core::lockers::Offer;
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;
const COAL: usize = 0;
const AXE: usize = 1;

fn content() -> Content {
    // An axe is one to a stack, a worth of half an emerald.
    Content::new(vec![Good::new("coal", 0.1, 64), Good::new("iron_axe", 0.5, 1)], DAY)
}

/// A smithy with `hands` and 20 emeralds, and a capital (town 1) far off with nothing but a name.
fn world(hands: &[(&str, u8)], money: f64) -> Society {
    let mut s = Society::new(content());
    let t = s.add_town("Smithy", 0.0);
    s.add_town("Capital", 0.0);
    s.set_roster(t, hands);
    if money > 0.0 {
        s.pay(t, money);
    }
    s
}

/// The Java fixture's weaponsmith: coal 15 for an emerald (16 uses, 2 XP), an axe for an emerald (12 uses, 1 XP).
fn smith_offers(s: &mut Society, buys: bool) {
    let mut offers = Vec::new();
    if buys {
        offers.push(Offer::buys(COAL, 15, 1, 16, 2, 0.05));
    }
    offers.push(Offer::sells(AXE, 1, 1, 12, 1, 0.05));
    s.set_offers(0, 0, offers);
}

fn open(s: &Society) -> Vec<usize> {
    s.world.resource::<Book>().open.clone()
}

// parity: LogisticsTests.aTrainingContractIsPaidWhenTheCensusShowsTheMaster
#[test]
fn a_training_contract_is_paid_when_the_census_shows_the_master() {
    let mut s = world(&[("weaponsmith", 3)], 0.0);
    let id = s.draft_training(0, 1, "weaponsmith", 5, 24.0, 0.0, 10 * DAY, true, true);
    assert!(s.contract(id).advance_due() == 8.0, "a third of the bonus up front: {}", s.contract(id).advance_due());
    s.world.resource_mut::<Book>().advanced(id, 8.0);
    s.advance(DAY);
    assert!(s.contract(id).in_force(), "a Journeyman yet: under way");
    s.set_roster(0, &[("weaponsmith", 5)]);
    s.advance(2 * DAY);
    let c = s.contract(id);
    assert!(c.state == State::Completed && c.owing == 16.0, "a master: done, the rest of the bonus owed: {}", c.owing);
    assert!(c.recouped == 8.0, "the advance worked off against the bonus");
    // A second contract wants a second master, not the one it has.
    let another = s.draft_training(0, 1, "weaponsmith", 5, 24.0, 0.0, 10 * DAY, false, true);
    assert_eq!(s.contract(another).training.unwrap().already, 1);
    s.advance(3 * DAY);
    assert!(s.contract(another).in_force(), "a second contract wants a second master, not the one it has");
    s.add_hand(0, "weaponsmith", 5);
    s.advance(4 * DAY);
    assert_eq!(s.contract(another).state, State::Completed, "and is done with the second");
}

#[test]
fn a_training_contract_late_is_a_miss_and_as_long_again_breaks_it() {
    // Due by day 4 (two days' grace to day 6), drafted on day 0: a miss after day 6, broken after day 10.
    let mut s = world(&[("weaponsmith", 3)], 0.0);
    let id = s.draft_training(0, 1, "weaponsmith", 5, 24.0, 0.0, 4 * DAY, true, true);
    s.world.resource_mut::<Book>().advanced(id, 8.0);
    s.advance(6 * DAY);
    assert_eq!((s.contract(id).missed, s.contract(id).state), (0, State::Active), "still within its grace");
    s.advance(7 * DAY);
    assert_eq!((s.contract(id).missed, s.contract(id).state), (1, State::Active), "late once");
    s.advance(10 * DAY);
    assert_eq!(s.contract(id).state, State::Active, "as long again as it was given");
    s.advance(11 * DAY);
    let c = s.contract(id);
    assert_eq!(c.state, State::Broken, "broken, still untrained");
    assert!(c.refund == 8.0, "and the advance it hadn't worked off is owed back: {}", c.refund);
}

// parity: LogisticsTests.aMerchantTakesATenderWorthItsWhile
#[test]
fn a_merchant_takes_a_tender_worth_its_while() {
    let mut s = world(&[("weaponsmith", 2)], 20.0);
    let cheap = s.tender(1, "weaponsmith", 5, 3.0, 20 * DAY);
    // Three levels at two emeralds is six: three won't pay for it.
    assert!(s.tender_worth(0, cheap).is_some_and(|v| v < 0.0), "three emeralds won't pay for three levels of coal and iron");
    let rushed = s.tender(1, "weaponsmith", 5, 40.0, 3 * DAY);
    assert!(s.tender_worth(0, rushed).is_none(), "and three days is too soon (eight are wanted)");
    let fair = s.tender(1, "weaponsmith", 5, 40.0, 20 * DAY);
    assert!(s.tender_worth(0, fair).is_some_and(|v| v > 0.0), "forty in twenty days pays");
    assert_eq!(s.tender_worth(0, fair), Some(34.0), "the bonus less six emeralds");
    assert_eq!(s.read_board(0, 1), 1, "the town's Merchant takes the one that pays...");
    let open = |id: usize| s.tenders().iter().find(|t| t.id == id).unwrap().open;
    assert_eq!((open(fair), open(cheap)), (0, 1), "...and leaves the others");
    let book = s.world.resource::<Book>();
    let c = book.contracts.iter().find(|c| c.seller == 0 && c.training.is_some()).expect("a training contract");
    assert!(c.buyer == 1 && c.known_by_seller && !c.known_by_buyer, "a contract with the buyer, its copy to ride home with the wagon");
    assert_eq!(c.start, 20 * DAY, "due the tender's day");
    assert!((c.advance_share - 1.0 / 3.0).abs() < 1e-9, "a third up front where trusted");
    assert_eq!(c.training.unwrap().already, 0);
}

// parity: LogisticsTests.aMerchantWithNoOneToTrainCarriesTheTenderOn
#[test]
fn a_merchant_with_no_one_to_train_carries_the_tender_on() {
    // A hub with no smith but a road to a town of its own; the capital's tender is read there.
    let mut s = Society::new(content());
    let hub = s.add_town("Hub", 0.0);
    let town = s.add_town("Smithy", 0.0);
    let capital = s.add_town("Capital", 0.0);
    s.set_roster(town, &[("weaponsmith", 2)]);
    s.add_route(hub, town, 1.0, 0.5, 2.0);
    let tender = s.tender(capital, "weaponsmith", 5, 40.0, 20 * DAY);
    assert_eq!(s.read_board(hub, capital), 1, "the hub can't train, but carries it on");
    let first = s.tenders().into_iter().find(|t| t.id == tender).unwrap();
    assert_eq!(first.open, 0);
    let onward = s.board(hub);
    assert!(onward.len() == 1 && onward[0].work.bonus < 40.0 && onward[0].due < first.due,
        "posted on its own board, for a little less, a little sooner");
    assert!(onward[0].work.bonus == 34.0, "15% less");
    assert_eq!(onward[0].due, 18 * DAY, "two days sooner");
    assert_eq!(s.tenders_open_for(capital, "weaponsmith"), 1, "the capital's want is still one, wherever it is posted");
    // Nobody to carry it to, and nobody read it twice.
    assert_eq!(s.read_board(hub, capital), 0);
}

#[test]
fn a_tender_carried_on_is_taken_by_a_town_of_the_hubs_and_done_when_that_town_is() {
    let mut s = Society::new(content());
    let hub = s.add_town("Hub", 0.0);
    let town = s.add_town("Smithy", 0.0);
    let capital = s.add_town("Capital", 0.0);
    s.set_roster(town, &[("weaponsmith", 2)]);
    s.pay(town, 20.0);
    s.add_route(hub, town, 1.0, 0.5, 2.0);
    s.tender(capital, "weaponsmith", 5, 40.0, 20 * DAY);
    s.read_board(hub, capital);
    assert_eq!(s.read_board(town, hub), 1, "the smithy reads the hub's board and takes the tender worth 34 - 6 to it");
    let book = s.world.resource::<Book>();
    let down = book.contracts.iter().find(|c| c.seller == town).expect("the town's contract, with the hub");
    let up = book.contracts.iter().find(|c| c.seller == hub).expect("the hub's own, with the capital");
    assert_eq!((down.buyer, up.buyer), (hub, capital));
    assert_eq!(up.training.unwrap().via, Some(down.id), "carried out by the town's");
    assert!(down.training.unwrap().bonus == 34.0 && up.training.unwrap().bonus == 40.0);
    let (down, up) = (down.id, up.id);
    // The copies ride: the town's to the hub, the hub's to the capital, on wagons; here by hand.
    for (id, party) in [(down, hub), (up, capital)] {
        s.world.resource_mut::<Book>().deliver_copy(id, party);
    }
    assert!(s.contract(down).in_force() && s.contract(up).in_force());
    s.advance(DAY);
    s.set_roster(town, &[("weaponsmith", 5)]);
    s.advance(2 * DAY);
    assert_eq!((s.contract(down).state, s.contract(up).state), (State::Completed, State::Completed),
        "the capital's contract is done when the town's is");
    assert!(s.contract(up).owing == 40.0 || s.contract(up).owing == enterprise_core::Fixed::emeralds(40) - enterprise_core::Fixed::ceil(40.0 / 3.0),
        "{}", s.contract(up).owing);
}

#[test]
fn a_town_already_training_or_drafting_takes_no_second_tender() {
    let mut s = world(&[("weaponsmith", 2), ("weaponsmith", 1)], 40.0);
    s.tender_for(1, "weaponsmith", 5, 40.0, 0.0, 2, 20 * DAY);
    assert_eq!(s.read_board(0, 1), 1, "one taken; a second isn't, while the first is drafted");
    assert_eq!(s.board(1)[0].open, 1);
}

#[test]
fn a_tender_nobody_takes_lapses_and_is_posted_again_for_a_quarter_more() {
    let mut s = world(&[("weaponsmith", 2)], 20.0);
    let id = s.tender(1, "weaponsmith", 5, 8.0, 10 * DAY);
    s.advance(7 * DAY);
    assert_eq!(s.board(1).len(), 1, "open until two days before its day");
    s.advance(8 * DAY);
    let lapsed = s.tenders();
    assert!(lapsed.iter().all(|t| t.id != id), "gone from the boards");
    let again = s.board(1);
    assert_eq!(again.len(), 1);
    assert!(again[0].work.bonus == 10.0, "a quarter more: {}", again[0].work.bonus);
    assert_eq!(again[0].due, 18 * DAY, "as long as it was given, from today");
}

#[test]
fn a_tender_carried_on_that_no_town_takes_goes_back_to_the_first() {
    let mut s = Society::new(content());
    let hub = s.add_town("Hub", 0.0);
    let town = s.add_town("Smithy", 0.0);
    let capital = s.add_town("Capital", 0.0);
    s.set_roster(town, &[("weaponsmith", 2)]);
    s.add_route(hub, town, 1.0, 0.5, 2.0);
    let first = s.tender(capital, "weaponsmith", 5, 40.0, 20 * DAY);
    s.read_board(hub, capital);
    assert_eq!(s.tenders().iter().find(|t| t.id == first).unwrap().open, 0);
    s.advance(16 * DAY);
    s.lapse_tenders();
    assert_eq!(s.tenders().iter().find(|t| t.id == first).map(|t| t.open), Some(1), "the carrier gives it back");
    assert!(s.board(hub).is_empty());
}

// parity: LockerTradingTests.theMerchantTrainsASmithOnlyUnderATrainingContract
#[test]
fn the_merchant_trains_a_smith_only_under_a_training_contract() {
    let mut s = world(&[("weaponsmith", 1), ("merchant", 1)], 20.0);
    smith_offers(&mut s, true);
    s.restock(0);
    let on = |s: &Society, k: usize| s.lockers(0).unwrap().hands[0].offers[k].on;
    let flagged = |s: &Society, k: usize| s.lockers(0).unwrap().hands[0].offers[k].training;
    assert!(!on(&s, 0), "no contract to train it: the Merchant leaves it be");
    s.draft_training(0, 1, "weaponsmith", 5, 24.0, 0.0, 10 * DAY, false, true);
    s.restock(0);
    assert!(on(&s, 0) && flagged(&s, 0), "under contract: what it learns on goes on");
    assert!(on(&s, 1) && flagged(&s, 1), "and with no coal in the warehouse, its work is bought instead");
    assert!(s.roster(0).hands[0].xp > 0, "it learns");
    assert!(s.lockers(0).unwrap().needed.contains(&s.content().trade("weaponsmith").unwrap()), "the Merchant's first care");
    // A master: the training stops.
    s.set_roster(0, &[("weaponsmith", 5), ("merchant", 1)]);
    s.restock(0);
    assert!(!on(&s, 0) && !on(&s, 1), "a master: the training stops");
    assert!(!flagged(&s, 0) && !flagged(&s, 1));
}

#[test]
fn what_the_merchant_switched_on_before_the_contract_stays_on_and_coal_in_the_warehouse_is_bought_first() {
    let mut s = world(&[("weaponsmith", 1), ("merchant", 1)], 20.0);
    smith_offers(&mut s, true);
    s.switch(0, 0, 0, true); // the coal trade was on when it hired the villager
    s.deliver(0, COAL, 15.0);
    s.draft_training(0, 1, "weaponsmith", 5, 24.0, 0.0, 10 * DAY, false, true);
    s.restock(0);
    let hand = &s.lockers(0).unwrap().hands[0];
    assert!(hand.offers[0].on && !hand.offers[0].training, "on already: not the training's to switch off");
    assert!(!hand.offers[1].on, "coal to learn on in the warehouse: no work bought");
    // The contract gone (a master): the one the training switched on goes off, the one it found stays.
    s.set_roster(0, &[("weaponsmith", 5), ("merchant", 1)]);
    s.restock(0);
    assert!(s.lockers(0).unwrap().hands[0].offers[0].on);
}

#[test]
fn the_pupil_is_the_smith_furthest_along() {
    let s = world(&[("weaponsmith", 2), ("weaponsmith", 4), ("weaponsmith", 5), ("merchant", 1)], 20.0);
    let roster = s.roster(0);
    let smith = s.content().trade("weaponsmith").unwrap();
    assert_eq!(enterprise_core::training::pupil(&roster, smith, 5), Some(1), "the level 4, not the 5 (done) nor the 2");
    assert_eq!(enterprise_core::training::pupil(&roster, smith, 2), None, "all at the level");
}

#[test]
fn no_money_to_spare_no_lesson_bought() {
    // The reserve and the advances held are the work's budget: broke, the pupil learns nothing it must pay for.
    let mut s = world(&[("weaponsmith", 1), ("merchant", 1)], 0.0);
    smith_offers(&mut s, false);
    s.draft_training(0, 1, "weaponsmith", 5, 24.0, 0.0, 10 * DAY, true, true);
    s.restock(0);
    assert!(!s.lockers(0).unwrap().hands[0].offers[0].on, "nothing to pay with");
    // The advance came: a lesson at least.
    let id = open(&s)[0];
    s.world.resource_mut::<Book>().advanced(id, 8.0);
    s.pay(0, 8.0);
    s.restock(0);
    assert!(s.lockers(0).unwrap().hands[0].offers[0].on, "paid up front: the work may spend it");
}

// parity: LockerTradingTests.underAnOfftakeThePupilsWorkIsCratedForTheBuyerWithABill
#[test]
fn under_an_offtake_the_pupils_work_is_crated_for_the_buyer_with_a_bill() {
    let mut s = world(&[("weaponsmith", 1), ("merchant", 1)], 40.0);
    smith_offers(&mut s, false); // a Novice who only sells
    s.draft_training(0, 1, "weaponsmith", 5, 24.0, 1.0, 10 * DAY, false, true);
    // The buyer's wagons call here: it will collect the work, and pay for it.
    s.add_route(1, 0, 1.0, 0.0, 2.0);
    s.restock(0);
    assert_eq!(s.town(0).stock[AXE], 0.0, "no axes left loose to clog it");
    let held = s.town_pallets(0);
    let crated: Vec<_> = held.iter().filter(|c| c.consigned_to(1)).collect();
    assert_eq!(crated.len(), 1, "one pallet");
    assert_eq!(crated[0].count(), 12.0);
    assert_eq!(held.iter().map(|p| p.crates()).sum::<u32>(), 2, "twelve axes crated for the buyer: two crates");
    assert!(crated[0].lading.prepaid() && crated[0].lading.contract.is_some(), "paid for by the bill drawn with them");
    let bills = s.bills();
    assert_eq!(bills.len(), 1);
    assert_eq!(bills[0].pallets, vec![crated[0].id], "the bill names its pallet");
    assert!(bills[0].drawee == 1 && bills[0].payee == 0, "a bill drawn on the buyer for them");
    assert!(bills[0].amount >= 12.0 / 3.0, "worth at least a third of an emerald each: {}", bills[0].amount);
    assert!(bills[0].amount == 12.0 * 0.5 * 1.02, "twelve axes at half an emerald, plus the bill's two per cent");
    assert_eq!(bills[0].held, Held::Town(0), "filed in the seller's warehouse");
    assert_eq!(s.paperwork(0).by_kind.get("bill of lading"), Some(&1), "with a bill of lading");
}

#[test]
fn with_no_wagon_of_the_buyers_calling_the_work_is_the_towns() {
    let mut s = world(&[("weaponsmith", 1), ("merchant", 1)], 40.0);
    smith_offers(&mut s, false);
    s.draft_training(0, 1, "weaponsmith", 5, 24.0, 1.0, 10 * DAY, false, true);
    s.restock(0);
    assert_eq!(s.town(0).stock[AXE], 12.0, "no caravan of the buyer's calls, and no hub: the work is the town's");
    assert!(s.bills().is_empty() && s.town_pallets(0).is_empty());
}

#[test]
fn without_an_offtake_the_work_is_bought_only_where_it_has_somewhere_to_go() {
    // No offtake, nobody wants axes: it's bought only while the warehouse has room for a restock's worth.
    let mut s = world(&[("weaponsmith", 1), ("merchant", 1)], 40.0);
    smith_offers(&mut s, false);
    s.draft_training(0, 1, "weaponsmith", 5, 24.0, 0.0, 10 * DAY, false, true);
    s.restock(0);
    assert_eq!(s.town(0).stock[AXE], 12.0, "room for twelve axes: bought");
    let mut s = world(&[("weaponsmith", 1), ("merchant", 1)], 40.0);
    smith_offers(&mut s, false);
    s.set_dial("warehouse_slots", 14.0).unwrap(); // twelve axes would leave 2 free of the 5 it keeps
    s.draft_training(0, 1, "weaponsmith", 5, 24.0, 0.0, 10 * DAY, false, true);
    s.restock(0);
    assert_eq!(s.town(0).stock[AXE], 0.0, "no room: no work bought");
    assert!(!s.lockers(0).unwrap().hands[0].offers[0].on);
}

#[test]
fn a_town_wants_what_its_people_use_so_its_work_has_somewhere_to_go() {
    let mut s = world(&[("weaponsmith", 1), ("merchant", 1)], 40.0);
    smith_offers(&mut s, false);
    s.set_dial("warehouse_slots", 14.0).unwrap();
    s.set_trade(0, AXE, 0.0, 1.0);
    s.draft_training(0, 1, "weaponsmith", 5, 24.0, 0.0, 10 * DAY, false, true);
    s.restock(0);
    assert!(s.lockers(0).unwrap().hands[0].offers[0].on, "the town wants axes: bought, room or not");
}

#[test]
fn a_training_contract_in_the_looks_trains_the_pupil_to_a_master_and_is_paid() {
    // The whole of it through the looks: contract in force, the pupil bought work, learned, became a master.
    let mut s = world(&[("weaponsmith", 1), ("merchant", 1)], 200.0);
    s.content_mut().dials.look_ticks = DAY / 2;
    s.seed(7);
    smith_offers(&mut s, true);
    s.set_dial("warehouse_slots", 5000.0).unwrap();
    let id = s.draft_training(0, 1, "weaponsmith", 3, 24.0, 0.0, 30 * DAY, true, true);
    let start = s.totals();
    for day in 1..=30 {
        s.advance(day * DAY);
        s.conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
        if s.contract(id).state == State::Completed {
            break;
        }
    }
    assert_eq!(s.contract(id).state, State::Completed, "level {}", s.roster(0).hands[0].level);
    assert!(s.roster(0).hands[0].level >= 3);
}

#[test]
fn a_wagon_carries_the_board_and_the_copy_and_the_contract_comes_into_force() {
    // The capital's wagon reaches the smithy and its Merchant reads the tender it carries from the capital's board;
    // the contract's copy rides home with the wagon.
    let mut s = Society::new(content());
    let smithy = s.add_town("Smithy", 0.0);
    let capital = s.add_town("Capital", 100.0);
    s.set_roster(smithy, &[("weaponsmith", 2), ("merchant", 1)]);
    s.pay(smithy, 20.0);
    s.set_trade(capital, COAL, 200.0, 0.0);
    s.set_trade(smithy, COAL, 0.0, 100.0);
    s.add_route(capital, smithy, 1.0, 0.1, 1.0);
    s.tender(capital, "weaponsmith", 5, 40.0, 30 * DAY);
    let mut seen = false;
    for day in 1..=12 {
        s.advance(day * DAY);
        if let Some(c) = s.contracts().iter().find(|c| c.training.is_some() && c.seller == smithy) {
            seen = true;
            if c.known_by_buyer && c.known_by_seller {
                assert_eq!(c.state, State::Active);
                return;
            }
        }
    }
    panic!("seen: {seen}; contracts {:?}", s.contracts().iter().map(|c| (c.state, c.known_by_seller, c.known_by_buyer)).collect::<Vec<_>>());
}

#[test]
fn the_offtake_crates_are_fetched_by_the_buyers_wagon_and_the_bill_is_honoured() {
    // The buyer's wagon calls at the smithy: it fetches the crates made out to it and presents... the bill rides to the
    // buyer on the smithy's wagon; here the smithy has a wagon to the capital too.
    let mut s = world(&[("weaponsmith", 1), ("merchant", 1)], 40.0);
    s.pay(1, 100.0);
    smith_offers(&mut s, false);
    s.add_route(1, 0, 1.0, 0.0, 1.0);
    s.add_route(0, 1, 1.0, 0.0, 1.0);
    s.draft_training(0, 1, "weaponsmith", 5, 24.0, 1.0, 20 * DAY, false, true);
    s.set_trade(1, COAL, 100.0, 0.0);
    s.set_trade(0, COAL, 0.0, 100.0);
    let start = s.totals();
    s.restock(0);
    for day in 1..=10 {
        s.advance(day * DAY);
        s.conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
    assert!(s.town_pallets(0).iter().all(|c| !c.consigned_to(1)), "the buyer's wagon fetched what was made out to it");
    assert!(s.town(1).stock[AXE] >= 12.0, "the axes are at the buyer: {}", s.town(1).stock[AXE]);
    assert!(s.bills().iter().all(|b| b.state != enterprise_core::bills::BillState::Dishonoured));
}
