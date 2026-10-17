//! Touch: towns brought up to date, their people making and using goods for the time since (computed on touch,
//! never per tick).

use bevy_ecs::prelude::*;

use super::Step;
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(catch_up.in_set(Step::Touch));
}

/// A town brought up to date: its people make and use goods for the time since, paying for what they use.
#[allow(clippy::too_many_arguments)]
pub fn catch_up_town(now: u64, content: &Content, ledger: &mut Ledger, book: &mut CashBook, town: usize, stock: &mut Stock,
        money: &mut Money, trades: &Trades, touched: &mut Touched) {
    let days = now.saturating_sub(touched.0) as f64 / content.day_ticks as f64;
    touched.0 = now;
    if days <= 0.0 {
        return;
    }
    for g in 0..stock.0.len() {
        let made = trades.makes[g] * days;
        stock.0[g] += made;
        ledger.made[g] += made;
        let used = (trades.uses[g] * days).min(stock.0[g]);
        stock.0[g] -= used;
        ledger.used[g] += used;
        let paid = used * content.goods[g].value;
        post(ledger, book, Side::outside(), Side::treasury(town, &mut money.0), paid, Reason::Sold(g), now,
            content.day_ticks);
    }
}

/// Every town brought up to date (others in the step run after it).
pub fn catch_up(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>,
        mut towns: Query<(&Town, &mut Stock, &mut Money, &Trades, &mut Touched)>) {
    for (town, mut stock, mut money, trades, mut touched) in &mut towns {
        catch_up_town(clock.now, &content, &mut ledger, &mut book, town.id, &mut stock, &mut money, trades, &mut touched);
    }
}
