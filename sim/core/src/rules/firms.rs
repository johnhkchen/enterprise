//! Firms' rules (`firms.rs`, `persons.rs`). At each look, before the town's workshop, each firm works what its town
//! wants of what it makes, each batch graded by Fundamentals × Morale, and sells it to the town. Once a day its hands'
//! spirits move and what they bring befalls them (inspiration; walking out, quarrels, sulks; an Ambitious hand long low
//! leaves), its owners appoint its head, it hires and lets go, pays its hands and its owners, or is wound up; and
//! where none of a kind is, a household founds one if enough people live within its range, the town wants what it
//! makes and a hand of its trade would make more than it earns now.

use std::collections::BTreeSet;

use bevy_ecs::prelude::*;

use crate::cash_book::{post, Account, CashBook, Reason, Side};
use crate::charters::{Charters, Mandate, Party, Source, Verb};
use crate::content::Content;
use crate::firms::{added, graded_wanted, ingredients, kinds, Firm, Firms, Head, Kind};
use crate::fixed::Fixed;
use crate::households::{earners, shares, Homes, HouseholdId, Households};
use crate::persons::{draw, fundamentals, roll, skill_cap, Event, Grade, Habit, Person, Persons};
use crate::roster::{Hand, HandId, Roster};
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

/// A firm's head no longer, its mandate ended.
fn end_head(firm: &mut Firm, charters: &mut Charters, now: u64) {
    if let Some(h) = firm.head.take() {
        if let Some(m) = charters.mandates.iter_mut().find(|m| m.id == h.mandate) {
            m.until = Some(now);
        }
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

/// A hand as a person: its make-up drawn from its id the first time a firm looks at it.
fn person<'a>(persons: &'a mut Persons, hand: &Hand, all: &[String]) -> &'a mut Person {
    persons.hands.entry(hand.id).or_insert_with(|| draw(hand.id, hand.level, all))
}

/// The batches a hand makes a day for its firm: its rank's, a quarter fewer a Perfectionist's or a Mentor head's own,
/// a quarter more under a Taskmaster head.
fn pace(d: &crate::content::Dials, hand: &Hand, m: &Person, head: Option<(HandId, &Person)>) -> f64 {
    let mut n = hand.level as f64 * d.firm_batches_per_level;
    if m.has(Habit::Perfectionist) {
        n *= 0.75;
    }
    match head {
        Some((h, hm)) if h == hand.id && hm.has(Habit::Mentor) => n *= 0.75,
        Some((h, hm)) if h != hand.id && hm.has(Habit::Taskmaster) => n *= 1.25,
        _ => {}
    }
    n
}

/// Once a day (and at each look, its work), each town's firms, then any kind it has none of founded where it pays.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn run(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, mut firms: ResMut<Firms>,
        mut households: ResMut<Households>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>,
        mut charters: ResMut<Charters>, mut persons: ResMut<Persons>,
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
    let persons = &mut *persons;
    // Every kind's ingredients, an ingredient talent's choice.
    let mut all: Vec<String> = kinds.iter().flat_map(|k| ingredients(&content, &k.conversion)).collect();
    all.sort();
    all.dedup();
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
            let Some(kind) = kinds.iter().find(|k| k.name == firm.name) else { continue };
            work(&content, kind, firm, persons, &all, roster, people, stock, money, hooks, &mut ledger, &mut book, now);
            if matches!(firm.reckoned, Some(k) if now < k + day) {
                continue;
            }
            firm.reckoned = Some(now);
            let c = &kind.conversion;
            let ing = ingredients(&content, c);
            let ours = |id: HandId| grown(id).filter(|x| x.trade == kind.trade);
            firm.employees.retain(|&(id, _)| ours(id).is_some());
            // A head gone from its hands, or at its term's end, is no longer: its owners appoint again below.
            let term = (d.firm_head_term_days * day as f64) as u64;
            let since = |m: crate::charters::MandateId| charters.mandates.iter().find(|x| x.id == m).map_or(0, |x| x.since);
            if firm.head.is_some_and(|h| !firm.employees.iter().any(|e| e.0 == h.hand) || now >= since(h.mandate) + term) {
                end_head(firm, &mut charters, now);
            }
            spirits(firm, persons, &all, &ours, homes, now, today, day);
            // What its town wanted of what it makes since yesterday, its average moved a fifth of the way: it hires
            // for a quarter more than that (a batch at least), not for the whole of the town's stock at once.
            firm.used += (firm.eaten - firm.used) * 0.2;
            firm.eaten = 0.0;
            let hire_for = (firm.used * 1.25 / c.out).ceil().max(1.0);
            // Its hands: as many as that takes, one at least while it's open; the last taken on let go first.
            let paced = |firm: &Firm, persons: &Persons, id: HandId| {
                let head = firm.head.and_then(|h| persons.hands.get(&h.hand).map(|m| (h.hand, m)));
                ours(id).zip(persons.hands.get(&id)).map_or(0.0, |(x, m)| pace(d, x, m, head))
            };
            let mut capacity: f64 = firm.employees.iter().map(|e| paced(firm, persons, e.0)).sum();
            while firm.employees.len() > 1 && capacity - paced(firm, persons, firm.employees.last().unwrap().0) >= hire_for {
                let (id, _) = firm.employees.pop().unwrap();
                capacity -= paced(firm, persons, id);
                employed.remove(&id);
            }
            // The hand its owners would take on next: of its trade and free, the best at the work as they see it
            // (skill and talents), its kin first under a Kin-minded head; and only while what it would make of a
            // batch at the grade it works at pays more than it would be offered.
            let candidate = |firm: &Firm, persons: &mut Persons, employed: &BTreeSet<HandId>| -> Option<HandId> {
                let kin = firm.head.filter(|h| persons.hands.get(&h.hand).is_some_and(|m| m.has(Habit::KinMinded)))
                    .and_then(|h| home_of(homes, h.hand));
                let mut best: Option<(bool, i64, u8, std::cmp::Reverse<HandId>)> = None;
                for x in roster.hands.iter()
                        .filter(|x| x.trade == kind.trade && x.baby_until.is_none() && !employed.contains(&x.id)) {
                    let m = person(persons, x, &all);
                    let w = fundamentals(m, kind.work, &ing);
                    let pays = added(&content, c, Grade::of(w)) * pace(d, x, m, None) > offer.to_f64();
                    let key = (kin.is_some() && home_of(homes, x.id) == kin, (w * 1000.0) as i64, x.level,
                        std::cmp::Reverse(x.id));
                    if pays && best.as_ref().is_none_or(|b| key > *b) {
                        best = Some(key);
                    }
                }
                best.map(|b| b.3 .0)
            };
            // Past the hands its owners run themselves (or to take on another who'd come), they appoint the most
            // skilled at its process of theirs its head: a mandate of the firm's, to hire and command its hands, at
            // more pay.
            let hands = firm.employees.len() as f64;
            let more = capacity < hire_for && hands >= d.firm_hands_unled && candidate(firm, persons, &employed).is_some();
            if firm.head.is_none() && (hands > d.firm_hands_unled || more) {
                let skill = |id: HandId| persons.hands.get(&id).map_or(0.0, |m| m.skill(kind.work));
                if let Some(i) = (0..firm.employees.len()).max_by(|&a, &b| {
                    let (x, y) = (firm.employees[a].0, firm.employees[b].0);
                    skill(x).total_cmp(&skill(y)).then(y.cmp(&x))
                }) {
                    let hand = firm.employees[i].0;
                    let mandate = charters.next_mandate;
                    charters.next_mandate += 1;
                    charters.mandates.push(Mandate { id: mandate, principal: Party::Firm(firm.id), agent: hand,
                        scope: vec![Verb::Hire, Verb::Command], source: Source::Appointment, since: now, term: Some(term),
                        until: None });
                    firm.head = Some(Head { hand, mandate });
                    firm.employees[i].1 = Fixed::ceil(firm.employees[i].1.to_f64() * (1.0 + d.firm_head_premium));
                    firm.note(today, hand, Event::Appointed);
                    capacity = firm.employees.iter().map(|e| paced(firm, persons, e.0)).sum();
                }
            }
            let most = if firm.head.is_some() { d.firm_hands_most } else { d.firm_hands_unled.max(1.0) };
            while (firm.employees.is_empty() || capacity < hire_for) && (firm.employees.len() as f64) < most {
                let Some(id) = candidate(firm, persons, &employed) else { break };
                firm.employees.push((id, offer));
                employed.insert(id);
                capacity += paced(firm, persons, id);
            }
            // A head let go, or over no more hands than its owners run themselves, is a hand again (paid as the
            // others).
            if let Some(h) = firm.head.filter(|h| !firm.employees.iter().any(|e| e.0 == h.hand)
                    || firm.employees.len() as f64 <= d.firm_hands_unled) {
                end_head(firm, &mut charters, now);
                if let Some(e) = firm.employees.iter_mut().find(|e| e.0 == h.hand) {
                    e.1 = offer;
                }
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
                end_head(firm, &mut charters, now);
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
        // would make more of a batch, at the grade it works at, than it would be offered.
        for kind in &kinds {
            if firms.firms.iter().any(|f| f.town == t && f.name == kind.name) {
                continue;
            }
            let near: f64 = places.iter().filter(|&&(u, at, _)| u == t || within(town.at, at, kind.range))
                .map(|p| p.2).sum();
            // (Nor where a bigger town within its range has one: the central place serves the country round it.)
            let bigger = |u: usize| places.iter().any(|&(v, at, n)| v == u && n > people.villagers && within(town.at, at, kind.range));
            if near < kind.threshold || firms.firms.iter().any(|f| f.name == kind.name && f.town != t && bigger(f.town)) {
                continue;
            }
            let c = &kind.conversion;
            let par = hooks.map_or(0.0, |h| h.par_of(c.output));
            if graded_wanted(&content, people, c.output, par) < c.out {
                continue;
            }
            let ing = ingredients(&content, c);
            let pays = roster.hands.iter().filter(|x| x.trade == kind.trade && x.baby_until.is_none()
                && !employed.contains(&x.id)).any(|x| {
                let m = person(persons, x, &all);
                added(&content, c, Grade::of(fundamentals(m, kind.work, &ing))) * pace(d, x, m, None) > offer.to_f64()
            });
            if !pays {
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

/// A firm's work since its last look: what its town wants of its graded pieces (`graded_wanted`; noted, for its hiring), its hands'
/// batches for the time passed (a day's at most carried; none of a hand out for the day), as far as the town wants
/// them, its inputs go (not what's set aside for forward sales) and the treasury pays; each batch graded by its maker's
/// Fundamentals × the shop's Morale, give or take the batch's own draw, and its maker the more skilled for it. The
/// pieces are sold to the town at their grades' worth and the inputs bought of it at theirs.
#[allow(clippy::too_many_arguments)]
fn work(content: &Content, kind: &Kind, firm: &mut Firm, persons: &mut Persons, all: &[String], roster: &Roster,
        people: &mut People, stock: &mut Stock, money: &mut Money, hooks: Option<&Workshop>, ledger: &mut Ledger,
        book: &mut CashBook, now: u64) {
    let d = &content.dials;
    let day = content.day_ticks.max(1);
    let c = &kind.conversion;
    let t = firm.town;
    let value = content.goods[c.output].value;
    let par = hooks.map_or(0.0, |h| h.par_of(c.output));
    let wanted = graded_wanted(content, people, c.output, par);
    firm.eaten += wanted;
    let since = firm.worked.map_or(0.0, |w| now.saturating_sub(w) as f64 / day as f64);
    firm.worked = Some(now);
    // Its hands at work, and how fast each goes.
    let head = firm.head.and_then(|h| persons.hands.get(&h.hand).map(|m| (h.hand, m.clone())));
    let crew: Vec<(HandId, f64)> = firm.employees.iter().filter_map(|&(id, _)| {
        let x = roster.hands.iter().find(|x| x.id == id && x.trade == kind.trade && x.baby_until.is_none())?;
        let m = person(persons, x, all);
        m.out_until.is_none_or(|u| now >= u).then(|| (id, pace(d, x, m, head.as_ref().map(|(h, m)| (*h, m)))))
    }).collect();
    let capacity: f64 = crew.iter().map(|c| c.1).sum();
    firm.progress = (firm.progress + capacity * since).min(capacity.max(1.0));
    let inputs = c.inputs.iter().map(|&(g, n)| {
        let aside = hooks.map_or(0.0, |h| h.aside_of(g));
        ((stock.0[g] - aside).max(0.0) / n + 1e-9).floor()
    }).fold(f64::INFINITY, f64::min);
    let best = Grade::Masterwork.worth(d);
    let affords = (money.0.at_least_zero().to_f64() / (c.out * value * best).max(1e-9)).floor();
    let n = (wanted / c.out - 1e-9).ceil().max(0.0).min(inputs).min(affords).min(firm.progress.floor());
    if n < 1.0 || crew.is_empty() {
        return;
    }
    // The shop's morale: its crew's spirits, its head's habits.
    let spirits: f64 = crew.iter().filter_map(|c| persons.hands.get(&c.0)).map(|m| m.spirits).sum::<f64>() / crew.len() as f64;
    let morale = crate::persons::morale(spirits, head.as_ref().map(|h| &h.1));
    let mentor = head.as_ref().is_some_and(|h| h.1.has(Habit::Mentor));
    let ing = ingredients(content, c);
    let mut given = vec![0.0; crew.len()];
    let mut made = [0.0; 5];
    for b in 0..n as usize {
        // Each batch to the hand furthest behind its share.
        let i = (0..crew.len()).max_by(|&x, &y| {
            let behind = |k: usize| crew[k].1 * (b + 1) as f64 / capacity - given[k];
            behind(x).total_cmp(&behind(y)).then(y.cmp(&x))
        }).unwrap();
        given[i] += 1.0;
        let (id, _) = crew[i];
        let level = roster.hands.iter().find(|x| x.id == id).map_or(1, |x| x.level);
        let m = persons.hands.get_mut(&id).unwrap();
        let spread = d.grade_spread * if m.has(Habit::Experimenter) { 2.0 } else { 1.0 };
        let draw = roll(firm.id as u64, firm.batches, 11);
        let score = crate::persons::score(fundamentals(m, kind.work, &ing), morale + crate::persons::own_form(m, now), spread, draw);
        let mut grade = Grade::of(score);
        if m.has(Habit::SteadyHand) {
            grade = grade.max(Grade::Plain);
        }
        made[grade.index()] += c.out;
        firm.batches += 1;
        let gain = d.skill_per_batch * if m.gifted(kind.work) { 1.5 } else { 1.0 }
            * if m.has(Habit::Ambitious) { 1.5 } else { 1.0 } * if mentor { 2.0 } else { 1.0 };
        let skill = m.skills.entry(kind.work).or_insert(0.0);
        *skill = (*skill + gain).min(skill_cap(level).max(*skill));
    }
    let pieces = crate::workshop::run(c, n * c.out, stock, ledger);
    let graded = people.graded.entry(c.output).or_insert([0.0; 5]);
    for g in Grade::ALL.into_iter().filter(|&g| g != Grade::Plain) {
        graded[g.index()] += made[g.index()];
    }
    for (a, b) in firm.grades.iter_mut().zip(made) {
        *a += b;
    }
    firm.made += pieces;
    let sales = Fixed::floor(Grade::ALL.iter().map(|g| made[g.index()] * value * g.worth(d)).sum::<f64>());
    post(ledger, book, Side::treasury(t, &mut money.0), Side::firm(firm.id, &mut firm.treasury), sales,
        Reason::Other("firm sales".into()), now, day);
    let bought = Fixed::ceil(n * crate::workshop::inputs_worth(content, c)).min(firm.treasury);
    post(ledger, book, Side::firm(firm.id, &mut firm.treasury), Side::treasury(t, &mut money.0), bought,
        Reason::Other("firm inputs".into()), now, day);
}

/// A firm's hands' spirits for the day, and what they bring. Each heads a third of the way toward its mark: a little
/// over a half, more at work it has a talent for, more under a Beloved head and less under a Taskmaster (its head's own
/// not), more among its kin under a Kin-minded one, and much less while its wages go unpaid. High spirits now and then bring an inspiration (three days of
/// better work; twice as often a Hothead's, half a Steady hand's); low ones an off-form spell: it walks out for the
/// day (never a Homebody), quarrels with another of the crew (both the lower for it; a Hothead's likelier), or sulks
/// (two days of worse work). An Ambitious hand ten days in low spirits leaves.
#[allow(clippy::too_many_arguments)]
fn spirits<'r>(firm: &mut Firm, persons: &mut Persons, all: &[String], ours: &dyn Fn(HandId) -> Option<&'r Hand>,
        homes: &Homes, now: u64, today: u64, day: u64) {
    let head = firm.head.and_then(|h| persons.hands.get(&h.hand).map(|m| (h.hand, m.clone())));
    let crew: Vec<HandId> = firm.employees.iter().map(|e| e.0).collect();
    let work = crate::firms::KINDS.iter().find(|k| k.0 == firm.name).map(|k| k.3);
    let mut left = Vec::new();
    for &id in &crew {
        let Some(x) = ours(id) else { continue };
        let kin = head.as_ref().is_some_and(|(h, m)| *h != id && m.has(Habit::KinMinded)
            && home_of(homes, *h).is_some() && home_of(homes, *h) == home_of(homes, id));
        let m = person(persons, x, all);
        let mut mark: f64 = 0.55 + if work.is_some_and(|w| m.gifted(w)) { 0.15 } else { 0.0 };
        if let Some((_, hm)) = head.as_ref().filter(|(h, _)| *h != id) {
            mark += if hm.has(Habit::Beloved) { 0.2 } else { 0.0 } - if hm.has(Habit::Taskmaster) { 0.2 } else { 0.0 };
        }
        if kin {
            mark += 0.2;
        }
        // (Wages unpaid, spirits fall.)
        if firm.broke_days > 0 {
            mark -= 0.4;
        }
        m.spirits += (mark.clamp(0.0, 1.0) - m.spirits) / 3.0;
        let hot = m.has(Habit::Hothead);
        if m.inspired_until.is_none_or(|u| now >= u) {
            let p = (m.spirits - 0.6).max(0.0) * 0.25 * if hot { 2.0 } else { 1.0 }
                * if m.has(Habit::SteadyHand) { 0.5 } else { 1.0 };
            if roll(id as u64, today, 21) < p {
                m.inspired_until = Some(now + 3 * day);
                firm.note(today, id, Event::Inspired);
            }
        }
        let p = (0.4 - m.spirits).max(0.0) * 0.5 * if hot { 1.5 } else { 1.0 };
        if roll(id as u64, today, 22) < p {
            let others: Vec<HandId> = crew.iter().copied().filter(|&o| o != id).collect();
            let walk = if m.has(Habit::Homebody) { 0.0 } else { 1.0 };
            let quarrel = if others.is_empty() { 0.0 } else if hot { 2.0 } else { 1.0 };
            let r = roll(id as u64, today, 23) * (walk + quarrel + 1.0);
            if r < walk {
                m.out_until = Some(now + day);
                firm.note(today, id, Event::WalkedOut);
            } else if r < walk + quarrel {
                let other = others[((roll(id as u64, today, 24) * others.len() as f64) as usize).min(others.len() - 1)];
                m.spirits = (m.spirits - 0.2).max(0.0);
                if let Some(o) = persons.hands.get_mut(&other) {
                    o.spirits = (o.spirits - 0.2).max(0.0);
                }
                firm.note(today, id, Event::Quarrelled(other));
            } else {
                m.sulking_until = Some(now + 2 * day);
                firm.note(today, id, Event::Sulked);
            }
        }
        let m = persons.hands.get_mut(&id).unwrap();
        m.low_days = if m.spirits < 0.35 { m.low_days + 1 } else { 0 };
        if m.has(Habit::Ambitious) && m.low_days >= 10 {
            m.low_days = 0;
            left.push(id);
        }
    }
    for id in left {
        firm.employees.retain(|e| e.0 != id);
        firm.note(today, id, Event::Left);
    }
}

/// Is `b` within `range` blocks of `a`? (A town not yet placed is near nothing.)
fn within(a: Option<(f64, f64)>, b: Option<(f64, f64)>, range: f64) -> bool {
    matches!((a, b), (Some(a), Some(b)) if (a.0 - b.0).hypot(a.1 - b.1) <= range)
}
