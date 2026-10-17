//! Touch: towns brought up to date, their people making and using goods for the time since (computed on touch,
//! never per tick).

use bevy_ecs::prelude::*;

use super::Step;
use crate::content::Content;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(catch_up.in_set(Step::Touch));
}

/// A town brought up to date: its people make and use goods for the time since, paying for what they use.
pub fn catch_up_town(now: u64, content: &Content, ledger: &mut Ledger, stock: &mut Stock, money: &mut Money,
        trades: &Trades, touched: &mut Touched) {
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
        money.0 += paid;
        ledger.income += paid;
    }
}

/// Every town brought up to date (others in the step run after it).
pub fn catch_up(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>,
        mut towns: Query<(&mut Stock, &mut Money, &Trades, &mut Touched)>) {
    for (mut stock, mut money, trades, mut touched) in &mut towns {
        catch_up_town(clock.now, &content, &mut ledger, &mut stock, &mut money, trades, &mut touched);
    }
}
