//! What the commands ask of the books (`commands_host.rs`): the cash book, the books and reasons, trust and contracts,
//! read from a society that has run a contract (a mill's paper to a town of scribes) and is asked as a host would.

use enterprise_core::wire::Answer;
use enterprise_core::{Content, Good, Incoterm, Society};

const DAY: u64 = 24_000;

fn society() -> Society {
    let content = Content::new(vec![Good::new("paper", 0.04, 64)], DAY);
    let mut s = Society::new(content);
    let mill = s.add_town("Wheatfield", 20.0);
    let scribes = s.add_town("Hayford", 40.0);
    s.set_trade(mill, 0, 128.0, 0.0);
    s.set_trade(scribes, 0, 0.0, 32.0);
    s.add_route(mill, scribes, 1.0, 0.5, 1.0);
    s.deliver(mill, 0, 640.0);
    s.draft(mill, scribes, 0, 64.0, 0.05, 0, DAY, 3, Incoterm::Ddp, true);
    s.advance(4 * DAY);
    s
}

#[test]
fn a_contract_is_listed_with_its_parties_and_what_it_has_done() {
    let s = society();
    let Answer::Contracts { contracts, .. } = s.contracts_answer(None) else { panic!() };
    assert_eq!(contracts.len(), 1);
    let c = &contracts[0];
    assert_eq!((c.seller_name.as_str(), c.buyer_name.as_str(), c.incoterm.as_str()), ("Wheatfield", "Hayford", "DDP"));
    assert_eq!(c.what, "64 paper");
    assert!(c.delivered >= 64.0 && c.paid.millis() > 0, "{c:?}");
    // Asked by either party it's there; by a town that's none of it, nothing.
    for t in [0, 1] {
        assert!(matches!(s.contracts_answer(Some(t)), Answer::Contracts { contracts, .. } if contracts.len() == 1));
    }
}

#[test]
fn trust_is_the_pairs_record_as_the_contract_left_it() {
    let s = society();
    let Answer::Trust { records } = s.trust_answer() else { panic!() };
    assert_eq!(records.len(), 1);
    assert_eq!((records[0].a, records[0].b), (0, 1));
    assert!((records[0].trust - s.trust(0, 1)).abs() < 1e-12, "{records:?}");
    assert!(records[0].kept > 0.0, "{records:?}");
}

#[test]
fn the_cash_book_shows_the_money_a_trade_moved_and_the_books_the_dealing() {
    let s = society();
    let Answer::CashBook { name, lines, .. } = s.cash_book_answer(0, 10) else { panic!() };
    assert_eq!(name, "Wheatfield");
    assert!(lines.iter().any(|l| l.amount.millis() > 0), "the mill was paid: {lines:?}");
    let ours: i64 = s.cash_book(0, 10).iter().map(|(_, _, v)| v.millis()).sum();
    assert_eq!(lines.iter().map(|l| l.amount.millis()).sum::<i64>(), ours, "the answer is the core's book, line for line");
    // The books and the reasons alone agree on the reasons.
    let (Answer::Books { whys: in_books, keeps, .. }, Answer::Whys { whys, .. }) = (s.books_answer(0), s.whys_answer(0)) else { panic!() };
    assert_eq!(in_books, whys);
    assert!(keeps > 0);
}

#[test]
fn the_cash_books_sums_are_the_cores_not_the_hosts() {
    let s = society();
    let Answer::CashBook { lines, totals, received, paid, held_totals, held, .. } = s.cash_book_answer(0, 10) else { panic!() };
    let net: i64 = lines.iter().map(|l| l.amount.millis()).sum();
    assert_eq!(received.millis() - paid.millis(), net, "money in less money out is what the lines come to");
    assert_eq!(totals.iter().map(|t| t.amount.millis()).sum::<i64>(), net);
    assert!(totals.windows(2).all(|w| w[0].what < w[1].what), "by what for, in the order of the words: {totals:?}");
    assert!(received.millis() > 0 && paid.millis() >= 0);
    assert_eq!(held_totals.iter().map(|t| t.amount.millis()).sum::<i64>(), held.iter().map(|l| l.amount.millis()).sum::<i64>());
}

#[test]
fn a_contract_tells_its_history_from_its_state_and_a_rent_is_listed_with_its_farm() {
    let mut s = society();
    let Answer::Contracts { contracts, rents } = s.contracts_answer(None) else { panic!() };
    assert!(rents.is_empty());
    let h = &contracts[0].history;
    assert!(h[0].starts_with("day 0: drafted, 64 paper") && h[0].ends_with("DDP"), "{h:?}");
    assert!(h.iter().any(|l| l.contains(&format!("{:.0} delivered", contracts[0].delivered))), "{h:?}");
    assert!(h.last().unwrap().ends_with("completed"), "{h:?}");
    // A rent: the scribes buy a season's paper from the mill; both towns are parties to it.
    s.pay(1, 100.0);
    let id = s.sell_rent(0, 1, 0, 8.0, 0.04).expect("the holder can pay");
    s.advance(DAY);
    for (town, n) in [(Some(0), 1), (Some(1), 1), (None, 1)] {
        let Answer::Contracts { rents, .. } = s.contracts_answer(town) else { panic!() };
        assert_eq!(rents.len(), n, "{town:?}");
        assert_eq!((rents[0].id as usize, rents[0].farm_name.as_str(), rents[0].holder_name.as_str()), (id, "Wheatfield", "Hayford"));
        assert!(rents[0].history[0].starts_with("day ") && rents[0].coupons >= 1, "{:?}", rents[0]);
    }
}

#[test]
fn a_towns_territory_is_what_the_core_keeps_of_it() {
    let s = society();
    let Answer::Territory { name, treasury, villagers, mood, trades, hexes, toll_rate, rate, guards, tolls_owed, .. } =
        s.territory_answer(0) else { panic!() };
    assert_eq!(name, "Wheatfield");
    assert_eq!(treasury, s.town(0).money);
    assert_eq!((villagers, mood), (s.town(0).people.villagers, s.town(0).people.mood));
    assert_eq!(trades, s.locker_trades(0));
    assert_eq!(hexes.len(), s.holding(0).hexes.len());
    assert_eq!((toll_rate, rate, guards, tolls_owed.millis()), (s.toll_rate(0), s.mint_rate(), 0, 0));
}

#[test]
fn the_war_is_the_crowns_and_the_armys_state() {
    let mut s = society();
    let Answer::War { phase, capital, serving, contracts, rate, .. } = s.war_answer() else { panic!() };
    assert_eq!((phase.as_str(), capital, serving, contracts), ("peace", None, 0, 0));
    assert_eq!(rate, s.mint_rate());
    s.name_capital(0, 500);
    s.advance(DAY);
    let Answer::War { capital, treasury, treasury_spurs, contracts, delivered, .. } = s.war_answer() else { panic!() };
    assert_eq!(capital, Some(0));
    assert_eq!(treasury, s.town(0).money);
    assert!((treasury_spurs - treasury.to_f64() * s.mint_rate()).abs() < 1e-9);
    // The mill's paper contract is with a town that isn't the capital: none of the crown's.
    assert_eq!((contracts, delivered), (0, 0.0));
}

#[test]
fn a_routes_books_are_its_states_and_its_wagons_book() {
    let s = society();
    let Answer::RouteBooks { rate, routes } = s.route_books_answer() else { panic!() };
    assert_eq!((rate, routes.len()), (s.mint_rate(), 1));
    let r = &routes[0];
    assert_eq!((r.from, r.to, r.trips), (0, 1, s.route(0).trips));
    assert_eq!(r.sold_items, s.route(0).sold);
    assert!(r.blocks > 0 || s.route_road_blocks(0) == 0);
    assert!((r.tolls_spurs - s.tolls_paid(0).to_f64() * rate).abs() < 1e-9);
}
