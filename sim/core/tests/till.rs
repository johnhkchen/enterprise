//! A town's money in its desk's chests and Blaze Banker (`till.rs`, task `money-adapter`): counted at each look and
//! booked as money paid in from outside the society or taken out; the treasury shown in them as whole emeralds, the
//! surplus sold to the mint for coin, the working reserve kept, and a spend taken out emeralds first, then coin, then the
//! Banker, then an emerald changed at the mint. A host here is a purse (`Purse`): a chest of so many slots holding
//! emeralds and coin (stacks of 64, coin in the fewest coins), and a Banker's balance, written as the mod's adapter
//! writes them.

use crate::common;

use common::DAY;
use enterprise_core::cash_book::{Account, Reason};
use enterprise_core::lockers::Offer;
use enterprise_core::mint::Mint;
use enterprise_core::points::Spot;
use enterprise_core::wire::*;
use enterprise_core::{Content, Fixed, Good, Society};

const WHEAT: usize = 0;

/// No founding purse: these count a desk's money from nothing (`a_new_town_is_given_its_founding_purse` has the purse).
fn pack() -> ContentPack {
    let mut pack = ContentPack::new(DAY, vec![PackGood::new("minecraft:wheat", 0.05, 64), PackGood::new("minecraft:bread", 0.2, 64)]);
    pack.dials = vec![PackDial { name: "founding_purse".into(), value: 0.0 }];
    pack
}

fn desk() -> Point {
    Point { kind: PointKind::Desk, dim: "minecraft:overworld".into(), x: 0, y: -60, z: 0 }
}

fn spot(p: &Point) -> Spot {
    Spot { dim: p.dim.clone(), x: p.x, y: p.y, z: p.z }
}

fn call(s: &mut Option<Society>, command: Command) -> Response {
    handle(s, Request::new(command))
}

fn created() -> Option<Society> {
    let mut s = None;
    assert_eq!(call(&mut s, Command::Create { pack: pack(), seed: 7 }), Response::Done { outputs: vec![], answers: vec![] });
    call(&mut s, Command::Apply { inputs: vec![Input::AddTown { name: "Ashford".into(), desk: desk() }] });
    s
}

fn apply(s: &mut Option<Society>, inputs: Vec<Input>) -> Vec<Output> {
    match call(s, Command::Apply { inputs }) {
        Response::Done { outputs, .. } => outputs,
        r => panic!("applied: {r:?}"),
    }
}

fn society(s: &mut Option<Society>) -> &mut Society {
    s.as_mut().unwrap()
}

/// The coins of a sum of spurs, fewest first by largest: spur, bevel (8), sprocket (16), cog (64), crown (512), sun (4096).
fn coin_stacks(spurs: i64) -> u32 {
    let mut left = spurs;
    let mut stacks = 0;
    for coin in [4096, 512, 64, 16, 8, 1] {
        stacks += ((left / coin) as u32).div_ceil(64);
        left %= coin;
    }
    stacks
}

/// A desk's chests and the Banker touching it, as a host holds them.
#[derive(Clone, Debug)]
struct Purse {
    slots: u32,
    junk: u32,
    emeralds: i64,
    coins: i64,
    bank: Option<i64>,
    writes: bool,
}

impl Purse {
    fn new(emeralds: i64, coins: i64, bank: Option<i64>) -> Purse {
        Purse { slots: 27, junk: 0, emeralds, coins, bank, writes: true }
    }

    fn used(&self) -> u32 {
        (self.emeralds as u32).div_ceil(64) + coin_stacks(self.coins) + self.junk
    }

    fn free(&self) -> u32 {
        self.slots.saturating_sub(self.used())
    }

    /// Room for a fresh set of coin: every slot the coin has now and the empty ones, six kinds of coin.
    fn coin_room(&self) -> bool {
        self.free() + coin_stacks(self.coins) >= 6
    }

    fn count(&self) -> Vec<Input> {
        vec![
            Input::Stocktake { point: desk(), counts: vec![], free: self.free() },
            Input::Cashcount { point: desk(), emeralds: self.emeralds as u64, coins: self.coins as u64,
                bank: self.bank.map(|b| b as u64), coin_room: self.coin_room(), writes: self.writes },
        ]
    }

    /// The core's move written as the adapter writes it: emeralds as far as there's room, coins re-minted as the fewest
    /// coins (all or nothing, and only if they fit), the Banker's balance. What couldn't be done is returned.
    fn write(&mut self, emeralds: i64, coins: i64, bank: i64) -> Option<Input> {
        let mut left = (0, 0, 0);
        if emeralds >= 0 {
            let room = (if self.emeralds % 64 > 0 { 64 - self.emeralds % 64 } else { 0 }) + self.free() as i64 * 64;
            let put = emeralds.min(room);
            self.emeralds += put;
            left.0 = emeralds - put;
        } else {
            let take = (-emeralds).min(self.emeralds);
            self.emeralds -= take;
            left.0 = emeralds + take;
        }
        let was = self.coins;
        let now = (was + coins).max(0);
        let mut after = self.clone();
        after.coins = now;
        if coins > 0 && after.used() > self.slots {
            left.1 = coins; // no room for the coin
        } else {
            self.coins = now;
            left.1 = if was + coins < 0 { was + coins } else { 0 };
        }
        match (&mut self.bank, bank) {
            (Some(b), bank) => {
                let now = (*b + bank).max(0);
                left.2 = if *b + bank < 0 { *b + bank } else { 0 };
                *b = now;
            }
            (None, bank) => left.2 = bank,
        }
        (left != (0, 0, 0)).then(|| Input::CashUnmoved { point: desk(), emeralds: left.0, coins: left.1, bank: left.2 })
    }

    /// A look as the host runs one: what it couldn't do, then the counts, an advance; then the cash moves written.
    fn look(&mut self, s: &mut Option<Society>, unmoved: &mut Vec<Input>, to: Option<u64>) -> Vec<Output> {
        let mut inputs = std::mem::take(unmoved);
        inputs.extend(self.count());
        inputs.extend(to.map(|to| Input::Advance { to }));
        let outputs = apply(s, inputs);
        for o in &outputs {
            if let Output::Cash { emeralds, coins, bank, .. } = o {
                if self.writes {
                    unmoved.extend(self.write(*emeralds, *coins, *bank));
                } else {
                    unmoved.push(Input::CashUnmoved { point: desk(), emeralds: *emeralds, coins: *coins, bank: *bank });
                }
            }
        }
        outputs
    }
}

fn cash(outputs: &[Output]) -> Vec<(i64, i64, i64)> {
    outputs.iter().filter_map(|o| match o {
        Output::Cash { emeralds, coins, bank, .. } => Some((*emeralds, *coins, *bank)),
        _ => None,
    }).collect()
}

fn treasury(s: &Option<Society>) -> Fixed {
    s.as_ref().unwrap().balance(Account::Treasury(0)).unwrap()
}

fn view(s: &mut Option<Society>) -> (Fixed, u64, u64, Fixed, Option<u64>, Fixed) {
    match call(s, Command::Ask { queries: vec![Query::Cash { point: desk() }] }) {
        Response::Done { answers, .. } => match &answers[0] {
            Answer::Cash { treasury, emeralds, coins, coin_worth, bank, bank_worth, .. } => {
                (*treasury, *emeralds, *coins, *coin_worth, *bank, *bank_worth)
            }
            a => panic!("{a:?}"),
        },
        r => panic!("{r:?}"),
    }
}

/// A lockers' trade switched on that costs emeralds: the Merchant keeps its reserve.
fn keep_a_reserve(s: &mut Option<Society>) {
    let soc = society(s);
    soc.set_roster(0, &[("librarian", 1)]);
    soc.set_offers(0, 0, vec![Offer::sells(WHEAT, 1, 4, 16, 2, 0.05)]);
    soc.switch(0, 0, 0, true);
}

fn books(s: &Option<Society>) -> Vec<(String, Fixed)> {
    let mut by: std::collections::BTreeMap<String, Fixed> = Default::default();
    for (_, what, v) in s.as_ref().unwrap().cash_book(0, 60) {
        *by.entry(what).or_default() += v;
    }
    by.into_iter().collect()
}

#[test]
fn what_a_desks_chests_hold_is_money_paid_in_and_what_leaves_them_money_taken_out() {
    let mut s = created();
    let mut purse = Purse::new(10, 0, None);
    purse.writes = false; // only counting
    let mut unmoved = Vec::new();
    purse.look(&mut s, &mut unmoved, None);
    assert_eq!(treasury(&s), Fixed::emeralds(10), "ten emeralds in the chest are the town's");
    // A player takes four out: booked to the outside, by what for.
    purse.emeralds = 6;
    purse.look(&mut s, &mut unmoved, None);
    assert_eq!(treasury(&s), Fixed::emeralds(6));
    assert_eq!(books(&s), vec![("paid in".to_string(), Fixed::emeralds(10)), ("taken out".to_string(), Fixed::emeralds(-4))]);
    // Counted again with nothing changed, nothing is booked.
    purse.look(&mut s, &mut unmoved, None);
    assert_eq!(treasury(&s), Fixed::emeralds(6));
    // The cash book is the treasury's own: income through a rule is noted beside the player's.
    society(&mut s).post(Account::Outside, Account::Treasury(0), 2, Reason::Other("lockers: sold wheat".into()));
    assert_eq!(books(&s).len(), 3);
    assert!(unmoved.iter().all(|u| matches!(u, Input::CashUnmoved { .. })), "a host that doesn't write sends every move back");
}

#[test]
fn spurs_are_counted_in_at_the_rate_of_the_day_and_keep_the_worth_they_came_in_at() {
    let mut s = created();
    let mut purse = Purse::new(0, 4096, Some(1600));
    purse.writes = false;
    let mut unmoved = Vec::new();
    purse.look(&mut s, &mut unmoved, None);
    // At 16 spurs an emerald: 4096 spurs of coin are worth 256 and the Banker's 1600 are worth 100.
    assert_eq!(treasury(&s), Fixed::emeralds(356));
    let (_, _, coins, coin_worth, bank, bank_worth) = view(&mut s);
    assert_eq!((coins, coin_worth, bank, bank_worth), (4096, Fixed::emeralds(256), Some(1600), Fixed::emeralds(100)));
    // The mint's rate halves: the same coin counted again books nothing (the worth it came in at stands), and 800 more
    // spurs are worth 100 at 8 spurs an emerald.
    society(&mut s).sell_emeralds(1728);
    assert!((society(&mut s).mint_rate() - 8.0).abs() < 1e-9);
    purse.look(&mut s, &mut unmoved, None);
    assert_eq!(treasury(&s), Fixed::emeralds(356), "the rate moving makes no money");
    purse.coins += 800;
    purse.look(&mut s, &mut unmoved, None);
    assert_eq!(treasury(&s), Fixed::emeralds(456), "spurs paid in come in at the rate then");
    // Taking half the coin out takes half its worth: 2448 of 4896 spurs.
    purse.coins -= 2448;
    purse.look(&mut s, &mut unmoved, None);
    let (_, _, coins, coin_worth, ..) = view(&mut s);
    assert_eq!((coins, coin_worth), (2448, Fixed::emeralds(178)));
    assert_eq!(treasury(&s), Fixed::emeralds(456 - 178));
}

// MoneyTests.surplusEmeraldsBecomeCoinsInStock in the core: the sale and its quote (the game test's chests and re-minting are the host's,
// CoreBindingTests.aDesksEmeraldsAreSoldToTheMintForCoinsInItsChest)
#[test]
fn emeralds_above_the_reserve_are_sold_to_the_mint_for_coins_in_the_chest() {
    let mut s = created();
    keep_a_reserve(&mut s);
    let expected = Mint::default().quote(36, 0, DAY) as i64;
    let mut purse = Purse::new(100, 0, None);
    let mut unmoved = Vec::new();
    let out = purse.look(&mut s, &mut unmoved, None);
    assert_eq!(cash(&out), vec![(-36, expected, 0)], "36 emeralds sold, the 64-emerald reserve kept");
    assert_eq!((purse.emeralds, purse.coins), (64, expected), "coins worth {expected} spurs in the chest");
    assert!(unmoved.is_empty());
    assert_eq!(treasury(&s), Fixed::emeralds(100), "selling at the mint moves worth, not money");
    let (_, emeralds, coins, coin_worth, ..) = view(&mut s);
    assert_eq!((emeralds, coins, coin_worth), (64, expected as u64, Fixed::emeralds(36)));
    // The mint took the selling pressure; the rate fell.
    assert!(society(&mut s).mint_rate() < 16.0);
    assert!((society(&mut s).mint().pressure(0, DAY) - 36.0).abs() < 1e-9);
    // And the next look, the chests as written, sells nothing more.
    assert!(cash(&purse.look(&mut s, &mut unmoved, None)).is_empty());
    assert_eq!(treasury(&s), Fixed::emeralds(100));
}

#[test]
fn a_blaze_bankers_account_takes_the_coin_and_the_chest_gets_none() {
    let mut s = created();
    keep_a_reserve(&mut s);
    let expected = Mint::default().quote(100, 0, DAY) as i64;
    let mut purse = Purse::new(164, 0, Some(0));
    let mut unmoved = Vec::new();
    let out = purse.look(&mut s, &mut unmoved, None);
    assert_eq!(cash(&out), vec![(-100, 0, expected)]);
    assert_eq!((purse.emeralds, purse.coins, purse.bank), (64, 0, Some(expected)), "no loose coins, the balance grew");
    let (.., bank, bank_worth) = view(&mut s);
    assert_eq!((bank, bank_worth), (Some(expected as u64), Fixed::emeralds(100)));
}

#[test]
fn with_no_trade_that_costs_emeralds_all_of_them_are_sold() {
    let mut s = created();
    let mut purse = Purse::new(10, 0, None);
    let out = purse.look(&mut s, &mut Vec::new(), None);
    assert_eq!(cash(&out)[0].0, -10);
    assert_eq!(purse.emeralds, 0);
}

#[test]
fn under_the_reserve_nothing_is_sold() {
    let mut s = created();
    keep_a_reserve(&mut s);
    let mut purse = Purse::new(50, 0, None);
    let out = purse.look(&mut s, &mut Vec::new(), None);
    assert!(cash(&out).is_empty(), "{out:?}");
    assert_eq!(purse.emeralds, 50, "all 50 stay");
    assert_eq!(society(&mut s).mint_rate(), 16.0);
}

#[test]
fn chests_with_no_room_for_coin_keep_their_emeralds_for_now() {
    let mut s = created();
    let mut purse = Purse::new(100, 0, None);
    purse.junk = 21; // 27 slots: 2 for the emeralds, 21 of junk: 4 free, a set of coin needs 6
    assert!(!purse.coin_room());
    let out = purse.look(&mut s, &mut Vec::new(), None);
    assert!(cash(&out).is_empty(), "{out:?}");
    assert_eq!(society(&mut s).mint_rate(), 16.0, "nothing sold");
    purse.junk = 5;
    assert!(cash(&purse.look(&mut s, &mut Vec::new(), None)).len() == 1, "room: sold");
}

#[test]
fn emeralds_are_put_only_in_the_room_the_stocks_moves_leave() {
    let mut s = created();
    keep_a_reserve(&mut s); // nothing sold: the emeralds stay in the chest
    let mut purse = Purse::new(0, 0, None);
    purse.slots = 1; // one empty slot at the count
    let mut unmoved = Vec::new();
    purse.look(&mut s, &mut unmoved, None);
    society(&mut s).deliver(0, WHEAT, 64.0);
    society(&mut s).post(Account::Outside, Account::Treasury(0), Fixed::emeralds(10), Reason::Takings);
    let out = purse.look(&mut s, &mut unmoved, None);
    assert!(out.iter().any(|o| matches!(o, Output::Moved { count: 64, .. })), "a stack of wheat takes the slot: {out:?}");
    assert!(cash(&out).iter().all(|c| c.0 == 0), "no emerald promised the slot the wheat took: {out:?}");
    assert_eq!(treasury(&s), Fixed::emeralds(10), "the ten wait on the books");
}

#[test]
fn income_is_put_in_as_whole_emeralds_and_fractions_wait_on_the_books() {
    let mut s = created();
    keep_a_reserve(&mut s);
    let mut purse = Purse::new(64, 0, None);
    let mut unmoved = Vec::new();
    purse.look(&mut s, &mut unmoved, None);
    // A wagon's takings: 7.4 emeralds.
    society(&mut s).post(Account::Outside, Account::Treasury(0), Fixed::from(7.4), Reason::Takings);
    let out = purse.look(&mut s, &mut unmoved, None);
    // Seven in, then the surplus over the reserve sold to the mint at once, as the mod settles after its income.
    assert_eq!(cash(&out)[0].0, 0, "seven emeralds put in and seven sold: {out:?}");
    assert_eq!(treasury(&s), Fixed::from(71.4));
    let (_, emeralds, _, coin_worth, ..) = view(&mut s);
    assert_eq!((emeralds, coin_worth), (64, Fixed::emeralds(7)));
    // A further 0.6 makes the eighth whole emerald.
    society(&mut s).post(Account::Outside, Account::Treasury(0), Fixed::from(0.6), Reason::Takings);
    purse.look(&mut s, &mut unmoved, None);
    let (_, emeralds, _, coin_worth, ..) = view(&mut s);
    assert_eq!((emeralds, coin_worth), (64, Fixed::emeralds(8)));
    assert_eq!(treasury(&s), Fixed::emeralds(72));
}

#[test]
fn a_spend_takes_emeralds_first_then_coin_then_the_banker() {
    let mut s = created();
    keep_a_reserve(&mut s);
    // 70 emeralds (kept: only 6 over the reserve would sell, so set the reserve up high by keeping 64) and coin and bank.
    let mut purse = Purse::new(64, 160, Some(320)); // 64 + 10 + 20 = 94 emeralds
    let mut unmoved = Vec::new();
    purse.look(&mut s, &mut unmoved, None);
    assert_eq!(treasury(&s), Fixed::emeralds(94));
    // Four emeralds spent on a field: the emeralds go (then bought back from coin at the mint, up to the reserve).
    society(&mut s).post(Account::Treasury(0), Account::Outside, 4, Reason::Other("a field".into()));
    let rate = society(&mut s).mint_rate();
    let out = purse.look(&mut s, &mut unmoved, None);
    let (emeralds, coins, bank) = cash(&out)[0];
    assert_eq!(emeralds, 0, "four taken and four bought back to the reserve: {out:?}");
    assert!(coins < 0, "paid for in coin, the coin first: {out:?}");
    assert_eq!(bank, 0);
    assert_eq!(purse.emeralds, 64);
    assert!(treasury(&s) == Fixed::emeralds(90));
    // The coin paid more than 4 emeralds' worth at the mint's price: the spread shows in the spur's worth, not the treasury.
    assert!(-coins as f64 >= 4.0 * rate, "{coins} spurs for four emeralds at {rate}");
    // Nothing made or lost: what the till holds is the treasury.
    let (t, e, _, cw, _, bw) = view(&mut s);
    assert_eq!(Fixed::emeralds(e as i64) + cw + bw, t, "the till shows the whole treasury");
}

#[test]
fn a_part_emerald_is_paid_from_coin_and_if_there_is_none_an_emerald_is_changed_at_the_mint() {
    let mut s = created();
    let mut purse = Purse::new(3, 0, None);
    let mut unmoved = Vec::new();
    purse.writes = false;
    purse.look(&mut s, &mut unmoved, None); // counted in: 3 emeralds, nothing else to show
    unmoved.clear();
    purse.writes = true;
    purse.emeralds = 3;
    // The town pays half an emerald out (with no reserve the look has also sold what it holds: so keep it back).
    let soc = society(&mut s);
    soc.set_roster(0, &[("librarian", 1)]);
    soc.set_offers(0, 0, vec![Offer::sells(WHEAT, 1, 4, 16, 2, 0.05)]);
    soc.switch(0, 0, 0, true); // a reserve of 64 emeralds: the 3 stay emeralds
    soc.post(Account::Treasury(0), Account::Outside, Fixed::from(0.5), Reason::Other("wages".into()));
    let out = purse.look(&mut s, &mut unmoved, None);
    // No coin and no Banker: an emerald is changed at the mint, half its worth paid, the change back as coin.
    let (e, c, b) = cash(&out)[0];
    assert_eq!((e, b), (-1, 0), "{out:?}");
    let sold = Mint::default().quote(1, 0, DAY) as i64;
    assert_eq!(c, sold - (sold as f64 * 0.5).ceil() as i64, "the other half back as coin");
    assert_eq!((purse.emeralds, purse.coins), (2, c));
    assert_eq!(treasury(&s), Fixed::from(2.5));
    let (t, em, _, cw, ..) = view(&mut s);
    assert_eq!(Fixed::emeralds(em as i64) + cw, t, "two emeralds and half an emerald of coin");
}

#[test]
fn what_the_host_couldnt_write_is_the_treasurys_still_and_is_shown_again() {
    let mut s = created();
    let mut purse = Purse::new(0, 0, None);
    let mut unmoved = Vec::new();
    purse.look(&mut s, &mut unmoved, None);
    // 20 emeralds of takings, but a hopper filled the chest: no slot for them.
    society(&mut s).post(Account::Outside, Account::Treasury(0), 20, Reason::Takings);
    purse.junk = 27;
    let out = purse.look(&mut s, &mut unmoved, None);
    assert!(cash(&out).is_empty(), "no room counted, nothing moved: {out:?}");
    assert_eq!(treasury(&s), Fixed::emeralds(20), "the money is the town's all the same");
    // One slot: 20 emeralds go in at the next look; with the reserve they stay emeralds.
    purse.junk = 26;
    let soc = society(&mut s);
    soc.set_roster(0, &[("librarian", 1)]);
    soc.set_offers(0, 0, vec![Offer::sells(WHEAT, 1, 4, 16, 2, 0.05)]);
    soc.switch(0, 0, 0, true);
    let out = purse.look(&mut s, &mut unmoved, None);
    assert_eq!(cash(&out), vec![(20, 0, 0)]);
    assert_eq!(purse.emeralds, 20);
    // A hopper fills the slot between the count and the write: nothing goes in, and the core is told.
    society(&mut s).post(Account::Outside, Account::Treasury(0), 5, Reason::Takings);
    let outputs = apply(&mut s, purse.count());
    assert_eq!(cash(&outputs), vec![(5, 0, 0)]);
    unmoved.push(Input::CashUnmoved { point: desk(), emeralds: 5, coins: 0, bank: 0 });
    purse.junk = 27 - 1;
    let out = purse.look(&mut s, &mut unmoved, None);
    assert_eq!(cash(&out), vec![(5, 0, 0)], "shown again once the chest counted shows room");
    assert_eq!(purse.emeralds, 25);
    assert_eq!(treasury(&s), Fixed::emeralds(25));
}

#[test]
fn a_host_that_doesnt_write_is_only_counted_and_the_mint_is_left_alone() {
    let mut s = created();
    let mut purse = Purse::new(500, 0, None);
    purse.writes = false;
    let mut unmoved = Vec::new();
    for day in 0..3 {
        let out = purse.look(&mut s, &mut unmoved, Some(day * DAY));
        assert!(cash(&out).is_empty(), "day {day}: {out:?}");
    }
    assert_eq!(society(&mut s).mint_rate(), 16.0, "the core sold nothing to the mint");
    assert_eq!(treasury(&s), Fixed::emeralds(500));
}

#[test]
fn a_desk_broken_takes_its_money_and_one_put_back_counts_it_afresh() {
    let mut s = created();
    let mut purse = Purse::new(30, 0, Some(160));
    purse.writes = false;
    let mut unmoved = Vec::new();
    purse.look(&mut s, &mut unmoved, None);
    assert_eq!(treasury(&s), Fixed::emeralds(40));
    apply(&mut s, vec![Input::RemovePoint { point: desk() }]);
    assert_eq!(treasury(&s), Fixed::ZERO, "the chests' emeralds and the Banker's account leave with the desk");
    apply(&mut s, vec![Input::AddTown { name: "again".into(), desk: desk() }]);
    purse.look(&mut s, &mut unmoved, None);
    assert_eq!(treasury(&s), Fixed::emeralds(40), "counted in once: the same town, the same money");
    assert_eq!(society(&mut s).town_count(), 1);
}

#[test]
fn a_banker_removed_takes_its_balance_and_a_new_one_is_counted_in() {
    let mut s = created();
    let mut purse = Purse::new(0, 0, Some(160));
    purse.writes = false;
    let mut unmoved = Vec::new();
    purse.look(&mut s, &mut unmoved, None);
    assert_eq!(treasury(&s), Fixed::emeralds(10));
    purse.bank = None;
    purse.look(&mut s, &mut unmoved, None);
    assert_eq!(treasury(&s), Fixed::ZERO);
    purse.bank = Some(32);
    purse.look(&mut s, &mut unmoved, None);
    assert_eq!(treasury(&s), Fixed::emeralds(2));
}

#[test]
fn a_cash_count_must_name_a_desk_that_stands() {
    let mut s = created();
    let locker = Point { kind: PointKind::Locker, ..desk() };
    let bad = Input::Cashcount { point: locker, emeralds: 1, coins: 0, bank: None, coin_room: false, writes: false };
    assert!(matches!(call(&mut s, Command::Apply { inputs: vec![bad] }), Response::Refused { .. }));
    let elsewhere = Point { x: 99, ..desk() };
    let bad = Input::Cashcount { point: elsewhere.clone(), emeralds: 1, coins: 0, bank: None, coin_room: false, writes: false };
    assert!(matches!(call(&mut s, Command::Apply { inputs: vec![bad] }), Response::Refused { .. }));
    // What the host couldn't do at a desk that has gone is a stale view, ignored.
    let stale = Input::CashUnmoved { point: elsewhere, emeralds: 1, coins: 0, bank: 0 };
    assert!(matches!(call(&mut s, Command::Apply { inputs: vec![stale] }), Response::Done { .. }));
    assert_eq!(spot(&desk()).x, 0);
}

#[test]
fn the_mint_is_asked_on_the_wire() {
    let mut s = created();
    let mint = |s: &mut Option<Society>| match call(s, Command::Ask { queries: vec![Query::Mint] }) {
        Response::Done { answers, .. } => match answers[0] {
            Answer::Mint { rate, pressure, minted, debased } => (rate, pressure, minted, debased),
            ref a => panic!("{a:?}"),
        },
        r => panic!("{r:?}"),
    };
    assert_eq!(mint(&mut s), (16.0, 0.0, 0.0, 1.0));
    society(&mut s).sell_emeralds(1728);
    let (rate, pressure, ..) = mint(&mut s);
    assert!((rate - 8.0).abs() < 1e-9 && (pressure - 1728.0).abs() < 1e-9, "{rate} {pressure}");
    let _ = (Content::new(vec![Good::new("w", 1.0, 64)], DAY), WHEAT);
}

/// Nothing made or lost over a month of takings, spending, a player's doing and the mint: the books balance to the
/// thousandth and the till shows what the treasury holds, give or take the fractions it keeps on its books.
#[test]
fn a_month_of_takings_and_spending_through_a_till_conserves_the_money() {
    let mut s = created();
    keep_a_reserve(&mut s);
    let start = society(&mut s).totals();
    let mut purse = Purse::new(80, 0, Some(0));
    let mut unmoved = Vec::new();
    for day in 0..30u64 {
        if day % 3 == 0 {
            society(&mut s).post(Account::Outside, Account::Treasury(0), Fixed::from(10.37 + day as f64), Reason::Takings);
        }
        if day % 4 == 1 {
            let held = treasury(&s);
            let spend = Fixed::from(6.18).min(held);
            society(&mut s).post(Account::Treasury(0), Account::Outside, spend, Reason::Other("wages".into()));
        }
        if day % 7 == 5 {
            purse.emeralds = (purse.emeralds - 9).max(0); // a player takes some
        }
        purse.look(&mut s, &mut unmoved, Some((day + 1) * DAY));
        society(&mut s).conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
        let (t, e, _, cw, _, bw) = view(&mut s);
        let shown = Fixed::emeralds(e as i64) + cw + bw;
        assert!(shown <= t, "day {day}: the till shows {shown} of a treasury of {t}");
        assert!(t - shown < Fixed::from(3.0) || purse.free() == 0, "day {day}: {} of {t} waits on the books", t - shown);
        assert_eq!((purse.emeralds as u64, purse.coins as u64), (e, view(&mut s).2), "day {day}: the till is what the host holds");
    }
}

#[test]
fn a_new_town_is_given_its_founding_purse() {
    let mut pack = pack();
    pack.dials.clear();
    let mut s = None;
    call(&mut s, Command::Create { pack, seed: 7 });
    call(&mut s, Command::Apply { inputs: vec![Input::AddTown { name: "Ashford".into(), desk: desk() }] });
    assert_eq!(treasury(&s), Fixed::emeralds(64), "64 emeralds from outside the society");
    assert_eq!(society(&mut s).cash_book(0, 1), vec![(0, "founding purse".to_string(), Fixed::emeralds(64))]);
    // Shown in the desk's chests at its looks, as the treasury always is.
    let mut purse = Purse::new(0, 0, None);
    let mut unmoved = Vec::new();
    purse.look(&mut s, &mut unmoved, None);
    purse.look(&mut s, &mut unmoved, None);
    let (held, emeralds, _, coin_worth, _, _) = view(&mut s);
    assert_eq!((held, Fixed::emeralds(emeralds as i64) + coin_worth), (Fixed::emeralds(64), Fixed::emeralds(64)));
    // The desk broken with the purse in its chests (taken out with them) and put back where it stood: the same town,
    // no second purse.
    apply(&mut s, vec![Input::RemovePoint { point: desk() }]);
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk() }]);
    assert_eq!((society(&mut s).town_count(), treasury(&s)), (1, Fixed::ZERO));
}
