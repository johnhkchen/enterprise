//! The farms' steps (`farms.rs`; `territory/Territory.java` `touch`, `payWages`, `bank`, `settleDeposits`,
//! `afterCensus`, `buyLand`; `territory/Fields.java` `consider`):
//!
//! - **Work:** each farm accrues its produce and its Farmers' wages for the time since, and reads its crop sales.
//! - **Pay:** the wages settled (the treasury pays what it can, owes the rest as credit as far as the Farmers will
//!   wait, and beyond that they take their share of the crop), the produce handed over (to the store, where there is
//!   one, beyond what the warehouse keeps loose), the stores banking and topping up; then the treasury's debts to its
//!   households' savings, with interest, repaid beyond its reserve.
//! - **Review** (each day's first look): the crop rotation, the plan made again, the town's focus, the stores it
//!   would build, a field when its Farmers have hands to spare, and a field of their own for households that can pay.
//!
//! **Not yet as the mod:** a household's freehold crop is chosen from all the town's wants (demand, trades' use,
//! draws) where `Fields.freehold` reads the villagers' demand with the herds' feed; and `Fields.consider` reads only the
//! feed as needs where this reads [`Farm::needs`] with [`Farm::feed`] (forward sales and feed). The herds fill
//! `feed`; the review sets `needs` to what the town has sold forward (`ForwardSales.sold`, `harvest.rs`). Share-cropping cuts each look's crop by the share paid at the last settlement, so it lags
//! a recovery by up to one settlement.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use super::Step;
use crate::bills::Bills;
use crate::cash_book::{post, Accountant, CashBook, Reason, Side};
use crate::content::Content;
use crate::fixed::Fixed;
use crate::farms::*;
use crate::people::food_points;
use crate::roster::Roster;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        work.in_set(Step::Work),
        pay.in_set(Step::Pay),
        settle_deposits.in_set(Step::Pay).after(pay),
        // In the order it ran unpinned: it and the settlers' review both spend the treasury, territory's reads its
        // people (a new system elsewhere could reorder them: bank-and-discounts pinned it).
        review.in_set(Step::Review).after(super::settlers::review).before(super::territory::review),
    ));
}

/// Each farm accrues for the time since, with the crop sales its cash book shows since it last looked.
pub fn work(clock: Res<Clock>, content: Res<Content>, book: Res<CashBook>, mut farms: Query<(&Town, &mut Farm, &People)>) {
    for (town, mut farm, people) in &mut farms {
        if !farm.set_up {
            continue;
        }
        let sold = crop_sales(&content, &book, town.id, &mut farm, clock.now);
        farm.sales_since += sold;
        accrue(&content, &mut farm, people.mood, clock.now);
    }
}

/// The reserve a town keeps back before repaying its households or planting: its trading capital and its wage float
/// (`Territory.reserve`).
pub fn reserve(content: &Content, trades: &Trades, farm: Option<&Farm>) -> Fixed {
    super::trading_capital(content, trades) + farm.map_or(Fixed::ZERO, |f| wage_float(content, f))
}

/// Wages settled, produce handed over, stores banking (`Territory.payWages`, `touch`, `bank`).
#[allow(clippy::type_complexity)]
pub fn pay(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>,
        mut farms: Query<(&Town, &mut Farm, &mut Money, &mut People, &mut Stock, &Trades)>) {
    let (now, day) = (clock.now, content.day_ticks);
    for (town, mut farm, mut money, mut people, mut stock, trades) in &mut farms {
        if !farm.set_up {
            continue;
        }
        let t = town.id;
        let farm = &mut *farm;
        // The whole emeralds owed: what the treasury has, then credit as far as the Farmers will wait.
        let whole = farm.wage_owed.floor();
        if whole > 0.0 {
            let emeralds = Fixed::emeralds(whole as i64);
            let paid = emeralds.min(money.0.at_least_zero());
            post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::households(t, &mut people.purse), paid,
                Reason::Wages, now, day);
            let room = (Fixed::floor(CREDIT_DAYS * value_per_day(&content, &farm.per_day) * LABOR_SHARE) - farm.credit)
                .at_least_zero();
            let credited = (emeralds - paid).min(room);
            farm.credit += credited;
            farm.wage_owed -= whole;
            farm.wages_paid += paid;
            farm.wages_unpaid += emeralds - paid - credited;
            farm.paid_share = (paid + credited).ratio(emeralds);
        } else if wage_per_day(&content, farm) <= 0.0 {
            // No one at work, no wage running: nothing is left unpaid (the mod's 1 with nothing whole owed). Without
            // this a share of 0 from the last settlement would stand for good once the plan empties.
            farm.paid_share = 1.0;
        }
        // Unpaid, the Farmers take their wage in kind: their share of the crop. The town gets the rest.
        if farm.paid_share < 1.0 {
            let value = value_per_day(&content, &farm.per_day);
            let share = if value <= 0.0 { 1.0 } else { (wage_per_day(&content, farm) / value).min(1.0) };
            let kept = 1.0 - (1.0 - farm.paid_share) * share;
            for (g, fresh) in &farm.fresh {
                if let Some(p) = farm.pending.get_mut(g) {
                    *p = (*p - fresh * (1.0 - kept)).max(0.0);
                }
            }
        }
        farm.fresh.clear();
        // The produce handed over as whole items: what the warehouse needn't keep loose straight to its store.
        let goods: Vec<usize> = farm.pending.keys().copied().collect();
        for g in goods {
            let whole = farm.pending[&g].floor();
            if whole <= 0.0 {
                continue;
            }
            let mut left = whole;
            if let Some(store) = store_for(&content, farm, g) {
                let short = (loose(&content, &trades.uses, g) - stock.0[g]).max(0.0);
                let banked = (left - short).min(store.holds() - in_store(&content, farm, store)).max(0.0).floor();
                *farm.banked.entry(g).or_insert(0.0) += banked;
                left -= banked;
            }
            stock.0[g] += left;
            ledger.made[g] += whole;
            *farm.pending.get_mut(&g).unwrap() -= whole;
            farm.harvested += whole;
        }
        bank(&content, farm, &mut stock.0, &trades.uses);
    }
}

/// Interest on what the treasury owes its households, and what it can spare beyond its reserve and its keep repaid
/// them (`Territory.settleDeposits`).
#[allow(clippy::type_complexity)]
fn settle_deposits(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>,
        mut towns: Query<(&Town, &mut Deposits, &mut Money, &mut People, &Trades, Option<&Farm>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    for (town, mut deposits, mut money, mut people, trades, farm) in &mut towns {
        if deposits.owed <= 0.01 {
            continue;
        }
        let days = now.saturating_sub(deposits.last) as f64 / day as f64;
        deposits.last = now;
        deposits.owed *= crate::fixed::pow(1.0 + content.dials.deposit_interest_per_day, days.min(MAX_DAYS));
        let spare = money.0 - reserve(&content, trades, farm) - Fixed::ceil(content.dials.deposit_keep);
        // What's owed rounded up to the thousandth (the treasury's debt), as far as it can spare.
        let repay = Fixed::ceil(deposits.owed).min(spare);
        if repay > Fixed::from_millis(10) {
            post(&mut ledger, &mut book, Side::treasury(town.id, &mut money.0), Side::households(town.id, &mut people.purse),
                repay, Reason::Other("deposits repaid".into()), now, day);
            deposits.owed = (deposits.owed - repay.to_f64()).max(0.0);
        }
    }
}

/// Each day's first look, the farm's census (`Territory.afterCensus`): rotation, the plan again, the focus, the
/// stores it would build, a new field (`Fields.consider`) and a household's own (`buyLand`).
#[allow(clippy::type_complexity)]
pub fn review(clock: Res<Clock>, content: Res<Content>, bills: Res<Bills>, contracts: Res<crate::contracts::Book>,
        mut ledger: ResMut<Ledger>,
        mut book: ResMut<CashBook>,
        mut farms: Query<(&Town, &mut Farm, &mut Money, &mut People, &Stock, &Trades, &mut Budget, Option<&Roster>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let today = now / day;
    for (town, mut farm, mut money, mut people, stock, trades, mut budget, roster) in &mut farms {
        if !farm.set_up || farm.reviewed == Some(today) {
            continue;
        }
        farm.reviewed = Some(today);
        let t = town.id;
        let farm = &mut *farm;
        // What it has sold forward comes first: a contract to keep (`Territory.afterCensus`; the herds' feed is not
        // here yet).
        farm.needs = crate::harvest::sold(&content, &contracts, t).into_iter().map(|(g, (n, _))| (g, n)).collect();
        let farmers = farmers(&content, roster);
        let food = food_points(&content, stock);
        let fed = food >= 2.0 * FOOD_PAR_POINTS;
        rotate(&content, farm, &Wants::of(farm, &trades.uses), fed);
        replan(&content, farm, &farmers, &trades.uses, fed);
        farm.focus = Focus::of(people.villagers, people.beds, food);
        farm.stores_wanted = Store::ALL.into_iter()
            .filter(|s| !farm.stores.contains(s) && wants_store(&content, farm, &stock.0, &trades.uses, *s)).collect();
        let wants = Wants::of(farm, &trades.uses).all();
        // A field, where the Farmers have hands to spare, or for a crop the town would buy and doesn't grow.
        if let Some(crop) = field_wanted(&content, farm, &farmers, &wants, food) {
            let free = super::free(&content, trades, &money, &bills, t, now);
            let field = Fixed::ceil(FIELD_EMERALDS);
            if money.0 - reserve(&content, trades, Some(farm)) >= field
                && Accountant::approve(&content, &mut book, t, now, free, &mut budget, Reason::Other("a field".into()),
                    field) {
                *farm.plots.entry(crop).or_insert(0) += field_plots(&content, crop);
                farm.fields_planted += 1;
                post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::outside(), field,
                    Reason::Other("a field".into()), now, day);
            }
        }
        // A household with the price of a field in purse and credit buys one of its own, where there are hands for it.
        let land = Fixed::ceil(LAND_EMERALDS);
        if people.purse + farm.credit >= land && farm.spare_plots >= FIELD_SIZE * FIELD_SIZE / 3
            && room_for_a_field(farm) {
            let crop = choose(&content, &Wants { needs: BTreeMap::new(), ..wants.clone() }, &farm.per_day, food,
                farm.focus.food_only()).or_else(|| content.good("wheat"));
            if let Some(crop) = crop {
                let from_credit = farm.credit.min(land);
                farm.credit -= from_credit;
                let paid = (land - from_credit).min(people.purse);
                post(&mut ledger, &mut book, Side::households(t, &mut people.purse), Side::treasury(t, &mut money.0), paid,
                    Reason::Other("land".into()), now, day);
                let plots = field_plots(&content, crop);
                farm.freeholds += plots;
                farm.fields_planted += 1;
                *farm.plots.entry(crop).or_insert(0) += plots;
            }
        }
    }
}

/// The crop of the field the town would plant now, if any (`Fields.consider`, before the money): its Farmers have
/// room for [`SPARE_PLOTS`] more plots, or there's a crop it would buy and grows none of (or feed it's short of) and
/// it has planted fewer than two fields a Farmer level; and there's room for it.
fn field_wanted(content: &Content, farm: &Farm, farmers: &[u8], wants: &Wants, food: f64) -> Option<usize> {
    if farmers.is_empty() || !room_for_a_field(farm) {
        return None;
    }
    let capacity = capacity(farmers, 1.0);
    let levels: u32 = farmers.iter().map(|l| (*l).max(1) as u32).sum();
    let worked = farm.per_day.values().sum::<f64>() / YIELD_PER_PLOT_PER_DAY;
    let food_only = farm.focus.food_only();
    let grown = |g: usize| farm.per_day.get(&g).copied().unwrap_or(0.0);
    let all = field_crops(content);
    let mut unmet = all.iter().filter(|&&c| allowed(content, c, wants, food, food_only)).any(|&crop| {
        wanted(content, crop, wants, food_only) > 0.0
            && all.iter().filter(|&&c| sold_as(content, c) == sold_as(content, crop)).all(|&c| grown(c) <= 0.0)
    });
    unmet |= wants.needs.iter().any(|(g, n)| *n > grown(*g));
    let spare = capacity - worked >= SPARE_PLOTS as f64;
    if !(spare || unmet && farm.fields_planted < 2 * levels) {
        return None;
    }
    choose(content, wants, &farm.per_day, food, food_only)
}
