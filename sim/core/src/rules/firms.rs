//! Firms' rules (`firms.rs`): once a day each firm hires and lets go, makes its finer pieces of the town's goods, sells
//! them to the town and pays its hands and its owners, or is wound up; and where none of a kind is, a household founds
//! one if enough people live within its range, the town wants what it makes and a hand makes more than it earns now.

use bevy_ecs::prelude::*;

use crate::cash_book::{post, Account, CashBook, Reason, Side};
use crate::charters::Party;
use crate::content::Content;
use crate::firms::{added, fine_wanted, kinds, Firm, Firms};
use crate::fixed::Fixed;
use crate::households::{earners, shares, Homes, HouseholdId, Households};
use crate::roster::{HandId, Roster};
use crate::rules::Step;
use crate::state::*;
use crate::workshop::Workshop;

/// The days of a town's wages a hand reckons what it earns by.
const WAGE_MEMORY_DAYS: u64 = 10;

pub fn register(schedule: &mut Schedule) {
    // (after the day's harvest, herds and kit and what's earmarked for forward sales; before the town's own workshop,
    // which makes plain what's still wanted)
    schedule.add_systems(run.in_set(Step::Work).after(super::cargo::earmark).before(super::workshop::work));
}

/// What a grown hand of the town earns a day of its wages, by the last [`WAGE_MEMORY_DAYS`] of them (the town's and
/// its own work's, `Reason::Wages` and `Reason::Earned`), shared among the hands a firm doesn't employ.
fn earned(book: &CashBook, t: usize, hands: usize, today: u64) -> f64 {
    let from = today.saturating_sub(WAGE_MEMORY_DAYS - 1);
    let paid: f64 = book.entries(Account::Households(t), from, today).into_iter()
        .filter(|(_, r, v)| matches!(r, Reason::Wages | Reason::Earned) && v.is_positive()).map(|(_, _, v)| v.to_f64()).sum();
    paid / (today + 1 - from) as f64 / hands.max(1) as f64
}

/// The household a hand lives in.
fn home_of(homes: &Homes, hand: HandId) -> Option<HouseholdId> {
    homes.list.iter().find(|h| h.members.contains(&hand)).map(|h| h.id)
}

/// Money a firm paid into the households' purse that's one household's own at the next share-out (a credit).
fn credit(homes: &mut Homes, h: Option<HouseholdId>, amount: Fixed) {
    if let Some(h) = h.filter(|_| amount.is_positive()) {
        homes.charges.push((h, -amount));
    }
}

/// Pays a firm's owners `amount` out of its treasury, by their shares (the last the remainder), each its own.
#[allow(clippy::too_many_arguments)]
fn pay_owners(firm: &mut Firm, homes: &mut Homes, people: &mut People, ledger: &mut Ledger, book: &mut CashBook,
        amount: Fixed, why: &str, now: u64, day: u64) {
    let amount = amount.min(firm.treasury.at_least_zero());
    let owners: Vec<(HouseholdId, f64)> = firm.owners.iter()
        .filter_map(|&(p, s)| match p { Party::Household(h) => Some((h, s)), _ => None }).collect();
    let mut left = amount;
    for (i, &(h, s)) in owners.iter().enumerate() {
        let part = if i + 1 == owners.len() { left } else { Fixed::floor(amount.to_f64() * s).min(left) };
        left -= part;
        post(ledger, book, Side::firm(firm.id, &mut firm.treasury), Side::households(firm.town, &mut people.purse), part,
            Reason::Other(why.into()), now, day);
        credit(homes, homes.list.iter().any(|x| x.id == h).then_some(h), part);
    }
}

/// Once a day, each town's firms at work, then any kind it has none of founded where it pays.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn run(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, mut firms: ResMut<Firms>,
        mut households: ResMut<Households>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>,
        mut q: Query<(&Town, &mut People, &mut Stock, &mut Money, &Roster, Option<&Workshop>),
            (Without<crate::realms::Realm>, Without<crate::realms::OffMap>)>) {
    let kinds = kinds(&content);
    if kinds.is_empty() {
        return;
    }
    let (now, day) = (clock.now, content.day_ticks.max(1));
    let today = now / day;
    let d = &content.dials;
    let firms = &mut *firms;
    // Where each town stands and how many live there, for the kinds' ranges.
    let places: Vec<(usize, Option<(f64, f64)>, f64)> = q.iter().map(|(t, p, ..)| (t.id, t.at, p.villagers)).collect();
    for &e in &towns.0 {
        let Ok((town, mut people, mut stock, mut money, roster, hooks)) = q.get_mut(e) else { continue };
        let t = town.id;
        let Some(homes) = households.towns.get_mut(&t) else { continue };
        let (people, stock, money) = (&mut *people, &mut *stock, &mut *money);
        let grown = |id: HandId| roster.hands.iter().find(|x| x.id == id && x.baby_until.is_none());
        // What a hand would be offered: more than it earns of the town's wages now.
        let mut employed = firms.employed(t);
        let free = roster.hands.iter().filter(|x| x.baby_until.is_none() && !employed.contains(&x.id)).count();
        let earns = earned(&book, t, free, today).max(d.hand_wage);
        let offer = Fixed::ceil(earns * (1.0 + d.firm_wage_premium));
        let mut done = Vec::new();
        for firm in firms.firms.iter_mut().filter(|f| f.town == t) {
            if matches!(firm.reckoned, Some(k) if now < k + day) {
                continue;
            }
            firm.reckoned = Some(now);
            let Some(kind) = kinds.iter().find(|k| k.name == firm.name) else { continue };
            let c = &kind.conversion;
            let (value, fine) = (content.goods[c.output].value, d.fine_quality);
            firm.employees.retain(|&(id, _)| grown(id).is_some_and(|x| x.trade == kind.trade));
            // The finer pieces the town wants of it; of the inputs, what isn't set aside for its forward sales; and
            // what the treasury can pay for.
            let par = hooks.map_or(0.0, |h| h.par_of(c.output));
            let wanted = (fine_wanted(&content, people, c.output, par) / c.out - 1e-9).ceil().max(0.0);
            let inputs = c.inputs.iter().map(|&(g, n)| {
                let aside = hooks.map_or(0.0, |h| h.aside_of(g));
                ((stock.0[g] - aside).max(0.0) / n + 1e-9).floor()
            }).fold(f64::INFINITY, f64::min);
            let affords = (money.0.at_least_zero().to_f64() / (c.out * value * fine).max(1e-9)).floor();
            let batches = wanted.min(inputs).min(affords);
            // Its hands: as many as those batches take (one at least, while it's open), the best of its trade first,
            // each while it pays more than the hand earns now; the last taken on let go first.
            let per_hand = |id: HandId| grown(id).map_or(0.0, |x| x.level as f64 * d.firm_batches_per_level);
            let mut capacity: f64 = firm.employees.iter().map(|e| per_hand(e.0)).sum();
            while firm.employees.len() > 1 && capacity - per_hand(firm.employees.last().unwrap().0) >= batches {
                let (id, _) = firm.employees.pop().unwrap();
                capacity -= per_hand(id);
                employed.remove(&id);
            }
            while (firm.employees.is_empty() || capacity < batches) && (firm.employees.len() as f64) < d.firm_hands_most {
                let Some(hand) = roster.hands.iter().filter(|x| x.trade == kind.trade && x.baby_until.is_none()
                        && !employed.contains(&x.id)).max_by_key(|x| (x.level, std::cmp::Reverse(x.id))) else { break };
                if added(&content, c) * per_hand(hand.id) <= offer.to_f64() {
                    break;
                }
                firm.employees.push((hand.id, offer));
                employed.insert(hand.id);
                capacity += per_hand(hand.id);
            }
            // Its day's work: the finer pieces, sold to the town at their worth, its inputs bought of it at theirs.
            let ran = batches.min(capacity.floor());
            let made = if ran > 0.0 { crate::workshop::run(c, ran * c.out, stock, &mut ledger) } else { 0.0 };
            if made > 0.0 {
                *people.fine.entry(c.output).or_insert(0.0) += made;
                firm.made += made;
                let sales = Fixed::floor(made * value * fine);
                post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::firm(firm.id, &mut firm.treasury), sales,
                    Reason::Other("firm sales".into()), now, day);
                let bought = Fixed::ceil(made / c.out * crate::workshop::inputs_worth(&content, c)).min(firm.treasury);
                post(&mut ledger, &mut book, Side::firm(firm.id, &mut firm.treasury), Side::treasury(t, &mut money.0), bought,
                    Reason::Other("firm inputs".into()), now, day);
            }
            // Its hands' wages, each its household's own, as far as its treasury goes.
            let mut short = false;
            for &(id, wage) in &firm.employees {
                let paid = wage.min(firm.treasury.at_least_zero());
                short |= paid < wage;
                post(&mut ledger, &mut book, Side::firm(firm.id, &mut firm.treasury), Side::households(t, &mut people.purse),
                    paid, Reason::Other("firm wages".into()), now, day);
                credit(homes, home_of(homes, id), paid);
            }
            firm.broke_days = if short { firm.broke_days + 1 } else { 0 };
            if firm.broke_days as f64 >= d.firm_broke_days {
                // Wound up: what's left of its treasury its owners', its hands back to the town's wages.
                let left = firm.treasury;
                pay_owners(firm, homes, people, &mut ledger, &mut book, left, "firm wound up", now, day);
                for (id, _) in firm.employees.drain(..) {
                    employed.remove(&id);
                }
                done.push(firm.id);
                continue;
            }
            // Beyond the days of wages it keeps, its takings are its owners'.
            let bill: Fixed = firm.employees.iter().map(|e| e.1).sum();
            let keep = Fixed::ceil(bill.to_f64() * d.firm_working_days);
            if firm.treasury > keep {
                let beyond = firm.treasury - keep;
                pay_owners(firm, homes, people, &mut ledger, &mut book, beyond, "dividends", now, day);
            }
        }
        for id in done {
            let i = firms.firms.iter().position(|f| f.id == id).unwrap();
            let f = firms.firms.remove(i);
            firms.wound_up.push((f, now));
        }
        // Founded: of each kind the town has none of, by the household that can pay its capital and its working days,
        // the richest (or the two richest together, partners), where enough people live within its range and no
        // bigger town within it has one, the town wants a batch of what it makes, and a hand of its trade not taken on
        // makes more than it would be offered.
        for kind in &kinds {
            if firms.firms.iter().any(|f| f.town == t && f.name == kind.name) {
                continue;
            }
            let Some(hand) = roster.hands.iter().filter(|x| x.trade == kind.trade && x.baby_until.is_none()
                && !employed.contains(&x.id)).max_by_key(|x| x.level) else { continue };
            let near: f64 = places.iter().filter(|&&(u, at, _)| u == t || within(town.at, at, kind.range))
                .map(|p| p.2).sum();
            // (Nor where a bigger town within its range has one: the central place serves the country round it.)
            let bigger = |u: usize| places.iter().any(|&(v, at, n)| v == u && n > people.villagers && within(town.at, at, kind.range));
            if near < kind.threshold || firms.firms.iter().any(|f| f.name == kind.name && f.town != t && bigger(f.town)) {
                continue;
            }
            let c = &kind.conversion;
            let par = hooks.map_or(0.0, |h| h.par_of(c.output));
            if fine_wanted(&content, people, c.output, par) < c.out
                || added(&content, c) * hand.level as f64 * d.firm_batches_per_level <= offer.to_f64() {
                continue;
            }
            let capital = Fixed::ceil(kind.capital);
            let working = Fixed::ceil(offer.to_f64() * d.firm_working_days);
            let cost = capital + working;
            let weights: Vec<usize> = homes.list.iter().map(|h| earners(roster, h, &employed)).collect();
            let purses = shares(&homes.list, &homes.charges, &weights, people.purse);
            let mut rich: Vec<usize> = (0..homes.list.len()).collect();
            rich.sort_by_key(|&i| (std::cmp::Reverse(purses[i]), i));
            let founders: Vec<(usize, Fixed)> = match rich[..] {
                [a, ..] if purses[a] >= cost => vec![(a, cost)],
                [a, b, ..] if purses[a] + purses[b] >= cost => vec![(a, purses[a]), (b, cost - purses[a])],
                _ => continue,
            };
            let id = firms.next;
            firms.next += 1;
            let mut firm = Firm { id, town: t, name: kind.name.into(), recipe: Some(content.goods[c.output].name.clone()),
                owners: founders.iter().map(|&(i, put)| (Party::Household(homes.list[i].id), put.ratio(cost))).collect(),
                founded: now, reckoned: Some(now), ..Default::default() };
            // Its capital the town's, which builds its workshop; its working days its own.
            post(&mut ledger, &mut book, Side::households(t, &mut people.purse), Side::treasury(t, &mut money.0), capital,
                Reason::Other("firm capital".into()), now, day);
            post(&mut ledger, &mut book, Side::households(t, &mut people.purse), Side::firm(id, &mut firm.treasury), working,
                Reason::Other("firm founded".into()), now, day);
            for &(i, put) in &founders {
                homes.charges.push((homes.list[i].id, put));
            }
            firms.firms.push(firm);
        }
    }
}

/// Is `b` within `range` blocks of `a`? (A town not yet placed is near nothing.)
fn within(a: Option<(f64, f64)>, b: Option<(f64, f64)>, range: f64) -> bool {
    matches!((a, b), (Some(a), Some(b)) if (a.0 - b.0).hypot(a.1 - b.1) <= range)
}
