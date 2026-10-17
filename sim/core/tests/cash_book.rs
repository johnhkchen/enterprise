//! The cash book and the Accountant: every transfer a posting between accounts, noted on both sides by day and by
//! what for, and the town's day's budget shared by all its spenders.

mod common;

use enterprise_core::cash_book::{Account, Reason, KEPT_DAYS};
use enterprise_core::state::Money;
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;

fn wheat_world() -> Society {
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 0.05, 64)], DAY));
    s.content_mut().dials.standing_orders = false;
    s
}

/// What a book says, by what for, summed over its days.
fn by_label(book: &[(u64, String, f64)], label: &str) -> f64 {
    book.iter().filter(|(_, l, _)| l == label).map(|(_, _, v)| v).sum()
}

// parity: CountingHouseTests.theCashBookTalliesMoneyByWhatItWasFor
#[test]
fn the_cash_book_tallies_money_by_what_it_was_for() {
    let mut s = wheat_world();
    let town = s.add_town("Ledgerton", 10.0);
    s.advance(DAY / 2);
    // 4 out to the households as wages, 2 in from the wheat its people bought.
    s.post(Account::Treasury(town), Account::Households(town), 4.0, Reason::Wages);
    s.post(Account::Outside, Account::Treasury(town), 2.0, Reason::Sold(0));
    let book = s.cash_book(town, 1);
    assert!((by_label(&book, "wages") + 4.0).abs() < 1e-9, "4 out, booked to what took it: {book:?}");
    assert!((by_label(&book, "lockers: sold wheat") - 2.0).abs() < 1e-9, "2 in from wheat: {book:?}");
    assert_eq!(book.len(), 2, "nothing else: {book:?}");
    assert!((s.town(town).money - 8.0).abs() < 1e-9, "and the money moved");
    // The other side of each: the households' purse got the wages; the outside paid the 2.
    let households = s.account_book(Account::Households(town), 1);
    assert!((by_label(&households, "wages") - 4.0).abs() < 1e-9, "{households:?}");
    assert!((s.town(town).people.purse - 4.0).abs() < 1e-9);
    let outside = s.account_book(Account::Outside, 1);
    assert!((by_label(&outside, "lockers: sold wheat") + 2.0).abs() < 1e-9, "{outside:?}");
}

// parity: CountingHouseTests.theAccountantKeepsTwoSpendersOffTheSameMoney
#[test]
fn the_accountant_keeps_two_spenders_off_the_same_money() {
    let mut s = wheat_world();
    let town = s.add_town("Cloverby", 40.0);
    let lender = s.add_town("Lender", 0.0);
    s.content_mut().dials.budget_share = 0.5; // half of the 40 free, 20, today
    assert!((s.free(town) - 40.0).abs() < 1e-9, "no trading capital, no bills: all 40 free");
    assert!(s.approve(town, "a field", 12.0), "40 free: so much of it today; a field for 12, yes");
    assert!(!s.approve(town, "a shop", 12.0), "and a shop for 12 more, not today: 8 left");
    let shopping = s.allow(town, "caravan shopping", 20.0);
    assert!((shopping - 8.0).abs() < 1e-9, "the caravan gets what's left, 8: {shopping}");
    assert!(s.budget_left(town) < 1e-9, "nothing left today");
    let held = s.held_back(town, 1);
    assert_eq!(held, vec![(0, "a shop".to_string(), 12.0), (0, "caravan shopping".to_string(), 12.0)],
        "what was held back is noted");
    assert!((s.town(town).money - 40.0).abs() < 1e-9, "approving isn't spending: no money moved");
    // A new day, a new budget: 20 again.
    s.advance(DAY);
    assert!((s.budget_left(town) - 20.0).abs() < 1e-9, "{}", s.budget_left(town));
    // A bill of 30 (30.6 with its interest) falling due within budget_days: only 9.4 of the 40 free.
    let now = s.now();
    let mut bills = s.world.resource_mut::<enterprise_core::bills::Bills>();
    let id = bills.draw(town, lender, 30.0, now, DAY);
    bills.bills[id].due = now + 2 * DAY;
    let free = s.free(town);
    assert!(free < 10.0 + 1e-9, "a bill of 30 falling due: only 10 of the 40 free, {free}");
    assert!((free - (40.0 - 30.6)).abs() < 1e-9, "{free}");
    // Falling due beyond budget_days, it doesn't count.
    s.world.resource_mut::<enterprise_core::bills::Bills>().bills[id].due = now + 4 * DAY;
    assert!((s.free(town) - 40.0).abs() < 1e-9);
}

/// A farm making wheat and a mill using it, a day apart, the farm's wagon every two days.
fn farm_and_mill() -> Society {
    let mut s = wheat_world();
    let farm = s.add_town("Wheatfield", 20.0);
    let mill = s.add_town("Hayford", 30.0);
    s.set_trade(farm, 0, 128.0, 0.0);
    s.set_trade(mill, 0, 0.0, 64.0);
    s.add_route(farm, mill, 1.0, 0.5, 2.0);
    s
}

#[test]
fn a_towns_book_explains_every_emerald_of_its_treasury() {
    let mut s = farm_and_mill();
    let start = [s.town(0).money, s.town(1).money];
    s.advance(40 * DAY);
    for t in 0..2 {
        let book = s.cash_book(t, 41);
        let booked: f64 = book.iter().map(|(_, _, v)| v).sum();
        let moved = s.town(t).money - start[t];
        assert!((booked - moved).abs() < 1e-6, "{}: moved {moved}, booked {booked}", s.town(t).name);
    }
    let farm = s.cash_book(0, 41);
    assert!(by_label(&farm, "road food") < 0.0, "the farm buys its wagon's road food: {farm:?}");
    assert!(by_label(&farm, "takings") > 0.0, "and its wagon brings takings home");
    let mill = s.cash_book(1, 41);
    assert!(by_label(&mill, "sale") < 0.0, "the mill pays the wagon for its wheat: {mill:?}");
    assert!(by_label(&mill, "lockers: sold wheat") > 0.0, "and its people buy the wheat");
    // The wagon's purse passes everything it took on to home: what it holds now is what's not home yet.
    let wagon = s.account_book(Account::Wagon(0), 41);
    let sales = by_label(&wagon, "sale");
    let on_the_road = s.balance(Account::Wagon(0)).unwrap_or(0.0);
    assert!(sales > 0.0 && (sales + by_label(&wagon, "takings") - on_the_road).abs() < 1e-9, "{wagon:?}");
}

#[test]
fn a_book_keeps_sixty_days_and_its_running_totals_for_ever() {
    let mut s = farm_and_mill();
    s.advance(100 * DAY);
    let book = s.cash_book(1, 1000);
    let first = book.iter().map(|(d, _, _)| *d).min().unwrap();
    assert!(first + KEPT_DAYS >= 100, "entries from day {first} on, of 100");
    assert!(first + KEPT_DAYS <= 101, "and as far back as that: {first}");
    let today = s.cash_book(1, 1);
    assert!(today.iter().all(|(d, _, _)| *d == 100), "{today:?}");
}

#[test]
fn money_moved_off_the_books_is_caught() {
    let mut s = farm_and_mill();
    s.advance(5 * DAY);
    let start = s.totals();
    s.advance(6 * DAY);
    s.conserved(&start).expect("on the books, it balances");
    // Money from one town to another without a posting: the world's money is the same, but the accounts aren't.
    let towns = s.world.resource::<enterprise_core::state::Towns>().0.clone();
    s.world.get_mut::<Money>(towns[0]).unwrap().0 -= 3.0;
    s.world.get_mut::<Money>(towns[1]).unwrap().0 += 3.0;
    let e = s.conserved(&start).expect_err("moved off the books");
    assert!(e.contains("Treasury"), "{e}");
}

#[test]
fn money_paid_in_from_outside_is_on_the_books() {
    let mut s = farm_and_mill();
    let start = s.totals();
    s.pay(1, 5.0);
    s.conserved(&start).expect("paid in by the outside");
    assert!((by_label(&s.cash_book(1, 1), "paid in") - 5.0).abs() < 1e-9);
}

#[test]
fn households_are_paid_wages_and_pay_for_comforts() {
    let mut content = Content::new(vec![Good::new("wheat", 0.05, 64), Good::new("bread", 0.2, 64).comfort(0)], DAY);
    content.dials.standing_orders = false;
    let mut s = Society::new(content);
    let town = s.add_town("Breadby", 50.0);
    s.set_people(town, 6.0, 8.0, 8.0);
    s.set_trade(town, 0, 64.0, 0.0);
    s.deliver(town, 1, 100.0);
    s.advance(5 * DAY);
    let h = s.account_book(Account::Households(town), 10);
    assert!(by_label(&h, "wages") > 0.0, "{h:?}");
    assert!(by_label(&h, "comforts") < 0.0, "{h:?}");
    let t = s.cash_book(town, 10);
    assert!((by_label(&t, "wages") + by_label(&h, "wages")).abs() < 1e-9, "both sides of the wages");
    assert!((by_label(&t, "comforts") + by_label(&h, "comforts")).abs() < 1e-9, "both sides of the comforts");
}

#[test]
fn each_towns_book_explains_its_treasury_in_random_worlds() {
    use common::{world, DAY};
    for seed in 0..100 {
        let mut s = world(seed);
        let mut at_90 = Vec::new();
        for day in 1..=120 {
            if day == 90 {
                // The last look of day 89 (a look is every 1,200 ticks, so this adds none).
                s.advance(day * DAY - 1200);
                at_90 = (0..s.town_count()).map(|t| s.town(t).money).collect();
            }
            s.advance(day * DAY);
            common::slices::daily(&mut s, seed, day);
        }
        // Days 90 to 120's entries are what each town's money moved since day 89.
        for t in 0..s.town_count() {
            let booked: f64 = s.cash_book(t, 31).iter().map(|(_, _, v)| v).sum();
            // A town founded since (settlers) had nothing at day 89.
            let moved = s.town(t).money - at_90.get(t).copied().unwrap_or(0.0);
            assert!((booked - moved).abs() < 1e-6 * moved.abs().max(1.0), "seed {seed}, town {t}: {booked} {moved}");
        }
    }
}
