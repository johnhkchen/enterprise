//! A rostered town's lockers through the look (`lockers.rs`; every town has them from `add_town`, empty until it
//! keeps a roster): following its roster and setting aside the uses its switched-on offers take over (Touch, before
//! the town's people buy at their worth); replaying the restocks missed since its clock, two a whole day (Work); the
//! Merchant's review at the day's first look (Review).

use bevy_ecs::prelude::*;

use super::Step;
use crate::cash_book::CashBook;
use crate::content::Content;
use crate::lockers::{catch_up, review as review_town, Books, Lockers};
use crate::state::*;
use crate::workshop::Workshop;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        follow.in_set(Step::Touch).before(super::touch::catch_up),
        // After the farm and the workshop: the Merchant touches the town's stock (its harvest in) before its lockers trade
        // (`LockerTrading.trade`).
        restock.in_set(Step::Work).after(super::farms::work).after(super::workshop::work),
        // After settlers: the hands that left are off its roster, and the stock they took gone, before it reviews.
        // (After the farm's review too, as main ran them before `realms` registered systems and moved an ambiguous order.)
        // Before the works' review, as main ran them before `post` registered systems and moved an ambiguous order.
        // (After the territory's, as main ran them before `orders` registered systems and moved an ambiguous order.)
        review.in_set(Step::Review).after(super::settlers::review).after(super::farms::review)
            .after(super::territory::review).before(super::works::review),
    ));
}

/// Every town's lockers follow its roster, and the uses of goods its switched-on offers buy are set aside: its income
/// from them is its lockers', its par for them their capacity, and its workshop makes them up to that par.
pub fn follow(content: Res<Content>, mut towns: Query<(&Roster, &mut Lockers, &mut Trades, Option<&mut Workshop>)>) {
    for (roster, mut lockers, mut trades, workshop) in &mut towns {
        lockers.sync(&content, roster);
        lockers.set_aside(&content, &mut trades, workshop.map(|w| w.into_inner()));
    }
}

/// Every town's missed restocks replayed, its managed lockers trading at each.
#[allow(clippy::type_complexity)]
pub fn restock(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>,
        seed: Res<Rng>, mut dice: ResMut<Dice>, market: Res<crate::market::Market>,
        mut towns: Query<(&Town, &mut Roster, &mut Lockers, &mut Stock, &mut Money, &People)>) {
    let mut order: Vec<_> = towns.iter_mut().collect();
    order.sort_by_key(|(t, ..)| t.id);
    for (town, mut roster, mut lockers, mut stock, mut money, people) in order {
        lockers.workshop_keeps = crate::workshop::keeps(&content, &roster, people, &stock);
        // And what the town bids more for than its lockers pay (`market.rs`): the paper its books would fetch more for.
        if content.dials.market {
            for (g, keep) in lockers.workshop_keeps.iter_mut().enumerate() {
                if market.bid(town.id, g).is_some_and(|(each, _)| each > content.goods[g].value) {
                    *keep = keep.max(stock.0[g]);
                }
            }
        }
        let mut books = Books { ledger: &mut ledger, book: &mut book, rng: dice.of(town.id, &seed) };
        catch_up(&content, &mut books, clock.now, town.id, &mut roster, &mut lockers, &mut stock, &mut money);
        lockers.workshop_keeps.clear(); // worked out at each restock, never kept
    }
}

/// The Merchant's review of its lockers, once a day at the day's first look. The goods the town wants (for its
/// lockers' selling offers): those its people use.
pub fn review(clock: Res<Clock>, content: Res<Content>, mut towns: Query<(&Roster, &mut Lockers, &Stock, &Trades)>) {
    let today = clock.now / content.day_ticks.max(1);
    for (roster, mut lockers, stock, trades) in &mut towns {
        if lockers.reviewed.is_some_and(|d| d >= today) {
            continue;
        }
        lockers.reviewed = Some(today);
        let wanted: Vec<bool> = trades.uses.iter().map(|u| *u > 0.0).collect();
        review_town(&content, clock.now, roster, &mut lockers, stock, &wanted);
    }
}
