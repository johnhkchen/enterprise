//! Firms' rules (`firms.rs`, `persons.rs`). At each look, before the town's workshop, each firm works what its town
//! wants of what it makes, each batch graded by Fundamentals × Morale, and sells it to the town. Once a day its hands'
//! spirits move and what they bring befalls them (inspiration; walking out, quarrels, sulks; an Ambitious hand long low
//! leaves), its owners appoint its head, it hires and lets go, pays its hands and its owners, or is wound up; and
//! where none of a kind is, a household founds one if enough people live within its range, the town wants what it
//! makes and a hand of its trade would make more than it earns now.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;

use crate::cash_book::{post, Account, CashBook, Reason, Side};
use crate::charters::{Charters, Mandate, Party, Source, Verb};
use crate::content::{Content, GoodId};
use crate::firms::{added, bids, graded_wanted_for, ingredients, kinds, price, Carried, Errand, Firm, FirmId, Firms, Head, Kind, Stage};
use crate::fixed::Fixed;
use crate::households::{earners, shares, Homes, HouseholdId, Households};
use crate::persons::{draw, fundamentals, knack_of, roll, skill_cap, Event, Grade, Habit, Knack, Person, Persons, Work};
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
    let m = persons.hands.entry(hand.id).or_insert_with(|| draw(hand.id, hand.level, all));
    // (A work new since it was drawn, it knows as its draw would have.)
    for w in Work::ALL {
        m.skills.entry(w).or_insert(3.0 * hand.level.clamp(1, 5) as f64);
    }
    m
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
        mut charters: ResMut<Charters>, mut persons: ResMut<Persons>, routes: Query<&crate::state::Route>,
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
    // (A tannery of an older save made boots: it's a bootmaker now.)
    for f in firms.firms.iter_mut().filter(|f| f.name == "tannery" && f.recipe.as_deref().is_some_and(|r| r.ends_with("leather_boots"))) {
        f.name = "bootmaker".into();
    }
    // Every kind's materials, a material talent's choice.
    let mut all: Vec<String> = kinds.iter().flat_map(|k| k.steps.iter().flat_map(|s| materials_of(&content, s))).collect();
    all.sort();
    all.dedup();
    // Each business's knack a Copycat buying of it copies: its most skilled Master's, not a Copycat's own.
    // The Houses' names, none twice.
    let mut houses: Vec<String> = firms.firms.iter().chain(firms.wound_up.iter().map(|w| &w.0))
        .filter_map(|f| f.house.clone()).collect();
    let mut masters: BTreeMap<FirmId, Knack> = BTreeMap::new();
    for f in &firms.firms {
        let work = kinds.iter().find(|k| k.name == f.name).map(|k| k.work);
        let best = f.employees.iter().filter_map(|e| persons.hands.get(&e.0).map(|m| (e.0, m)))
            .filter(|(_, m)| m.knack.is_some_and(|k| k != Knack::Copycat))
            .max_by(|a, b| work.map_or(0.0, |w| a.1.skill(w)).total_cmp(&work.map_or(0.0, |w| b.1.skill(w))).then(b.0.cmp(&a.0)));
        if let Some((_, m)) = best {
            masters.insert(f.id, m.knack.unwrap());
        }
    }
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
        // The town's businesses by what they sell it, and what of each material they'd use of it: two days' of what
        // their town wants of them, a tannery's leather kept at what its bootmaker would sew.
        let mut makers: BTreeMap<GoodId, FirmId> = BTreeMap::new();
        let mut needs: BTreeMap<GoodId, f64> = BTreeMap::new();
        let bids = bids(&content, firms.firms.iter().filter(|f| f.town == t)
            .filter_map(|f| kinds.iter().find(|k| k.name == f.name)));
        for f in firms.firms.iter().filter(|f| f.town == t) {
            let Some(kind) = kinds.iter().find(|k| k.name == f.name) else { continue };
            makers.insert(kind.steps[kind.steps.len() - 1].output, f.id);
            for &(g, n) in &kind.conversion.inputs {
                *needs.entry(g).or_insert(0.0) += 2.0 * f.used / kind.conversion.out.max(1e-9) * n;
            }
        }
        // A House's draw: of the hands newly of its trade in its town (none of its businesses has looked at yet), its
        // draw's share come with a talent for its work, as those who come seeking work at a House do.
        for f in firms.firms.iter().filter(|f| f.town == t && f.house.is_some()) {
            let Some(kind) = kinds.iter().find(|k| k.name == f.name) else { continue };
            let share = f.draw(d);
            let new: Vec<&Hand> = roster.hands.iter().filter(|x| x.trade == kind.trade && x.baby_until.is_none()
                && !persons.hands.contains_key(&x.id)).collect();
            for x in new {
                let m = person(persons, x, &all);
                if roll(x.id as u64, 0, 51) < share && !m.gifted(kind.work) {
                    m.talents.push(crate::persons::Talent::Work(kind.work));
                }
            }
        }
        let mut done = Vec::new();
        for firm in firms.firms.iter_mut().filter(|f| f.town == t) {
            let Some(kind) = kinds.iter().find(|k| k.name == firm.name) else { continue };
            let copied = firm.main_supplier().and_then(|f| masters.get(&f).copied());
            let output = kind.steps[kind.steps.len() - 1].output;
            // (Not its own use: a tannery cures the town's hides for others.)
            let own: f64 = kind.conversion.inputs.iter().filter(|i| i.0 == output)
                .map(|i| 2.0 * firm.used / kind.conversion.out.max(1e-9) * i.1).sum();
            let needed = needs.get(&output).copied().unwrap_or(0.0) - own;
            work(&content, kind, firm, persons, &all, roster, people, stock, money, hooks, &mut ledger, &mut book, now,
                needed, &makers, &bids, copied);
            if matches!(firm.reckoned, Some(k) if now < k + day) {
                continue;
            }
            firm.reckoned = Some(now);
            let c = &kind.conversion;
            let ours = |id: HandId| grown(id).filter(|x| x.trade == kind.trade);
            firm.employees.retain(|&(id, _)| ours(id).is_some());
            // A head gone from its hands, or at its term's end, is no longer: its owners appoint again below.
            let term = (d.firm_head_term_days * day as f64) as u64;
            let since = |m: crate::charters::MandateId| charters.mandates.iter().find(|x| x.id == m).map_or(0, |x| x.since);
            if firm.head.is_some_and(|h| !firm.employees.iter().any(|e| e.0 == h.hand) || now >= since(h.mandate) + term) {
                end_head(firm, &mut charters, now);
            }
            spirits(kind, firm, persons, &all, &ours, homes, copied, now, today, day);
            firm.regular = if firm.sold > 0.0 { firm.regular + 1 } else { 0 };
            firm.sold = 0.0;
            // Its name fades a little each day; made great enough, it's a House.
            firm.reputation *= 1.0 - d.reputation_fade;
            if firm.house.is_none() && firm.reputation >= d.house_reputation {
                firm.house = Some(crate::persons::house_name(firm.id as u64, &houses));
                houses.extend(firm.house.clone());
            }
            // What its town wanted of what it makes since yesterday, its average moved a fifth of the way: it hires
            // for a quarter more than that (a batch at least), not for the whole of the town's stock at once.
            firm.used += (firm.eaten - firm.used) * 0.2;
            firm.eaten = 0.0;
            let hire_for = (firm.used * 1.25 / c.out).ceil().max(1.0);
            // Its hands: as many as that takes, one at least while it's open; the last taken on let go first.
            // (At its last step's station's pace.)
            let paced = |firm: &Firm, persons: &Persons, id: HandId| {
                let head = firm.head.and_then(|h| persons.hands.get(&h.hand).map(|m| (h.hand, m)));
                let speed = crate::firms::throughput(d, firm.station(kind.steps.len() - 1));
                ours(id).zip(persons.hands.get(&id)).map_or(0.0, |(x, m)| pace(d, x, m, head) * speed)
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
                    let w = best_step(&content, kind, m).0;
                    let pays = added(&content, &bids, c, Grade::of(w)) * pace(d, x, m, None) > offer.to_f64();
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
            // (A branch has its head always, by its House's mandate: never a Homebody, who won't run one.)
            let branch = firm.branch_of;
            if firm.head.is_none() && (hands > d.firm_hands_unled || more || branch.is_some()) {
                let skill = |id: HandId| persons.hands.get(&id)
                    .filter(|m| branch.is_none() || !m.has(Habit::Homebody))
                    .map_or(-1.0, |m| kind.steps.iter().map(|s| m.skill(s.work)).fold(0.0, f64::max));
                if let Some(i) = (0..firm.employees.len()).filter(|&i| skill(firm.employees[i].0) >= 0.0).max_by(|&a, &b| {
                    let (x, y) = (firm.employees[a].0, firm.employees[b].0);
                    skill(x).total_cmp(&skill(y)).then(y.cmp(&x))
                }) {
                    let hand = firm.employees[i].0;
                    let mandate = charters.next_mandate;
                    charters.next_mandate += 1;
                    charters.mandates.push(Mandate { id: mandate, principal: Party::Firm(branch.unwrap_or(firm.id)), agent: hand,
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
                    || (firm.employees.len() as f64 <= d.firm_hands_unled && branch.is_none())) {
                end_head(firm, &mut charters, now);
                if let Some(e) = firm.employees.iter_mut().find(|e| e.0 == h.hand) {
                    e.1 = offer;
                }
            }
            // Its hands' wages, each its household's own, as far as its treasury goes.
            let mut short = false;
            for &(id, wage) in &firm.employees {
                let paid = wage.min((firm.treasury - firm.pledged).at_least_zero());
                short |= paid < wage;
                post(&mut ledger, &mut book, Side::firm(firm.id, &mut firm.treasury), Side::households(t, &mut people.purse),
                    paid, Reason::Other("firm wages".into()), now, day);
                credit(homes, home_of(homes, id), paid);
            }
            // (A business with no hands can't work: it's wound up as one that can't pay them.)
            short |= firm.employees.is_empty();
            firm.broke_days = if short { firm.broke_days + 1 } else { 0 };
            if firm.broke_days as f64 >= d.firm_broke_days {
                // Wound up: what's left of its treasury its owners' (a branch's, its town's: its House is far off),
                // its hands back to the town's wages; what it was sending, not sent.
                let left = firm.treasury;
                if branch.is_some() {
                    post(&mut ledger, &mut book, Side::firm(firm.id, &mut firm.treasury), Side::treasury(t, &mut money.0),
                        left.at_least_zero(), Reason::Other("firm wound up".into()), now, day);
                } else {
                    pay_owners(firm, homes, people, &mut ledger, &mut book, left, "firm wound up", now, day);
                }
                firm.pledged = Fixed::ZERO;
                end_head(firm, &mut charters, now);
                for (id, _) in firm.employees.drain(..) {
                    employed.remove(&id);
                }
                done.push(firm.id);
                continue;
            }
            let short_handed = capacity < hire_for && firm.employees.len() as f64 >= most;
            let saving = stations(&content, kind, firm, persons, &bids, offer, short_handed.then_some((hire_for, capacity)),
                &mut ledger, &mut book, &mut money.0, now);
            // Beyond the days of wages it keeps (and what it's sending), its takings are its owners': a branch's, its
            // House's, due for the House's next wagon to call.
            let bill: Fixed = firm.employees.iter().map(|e| e.1).sum();
            let mut keep = Fixed::ceil(bill.to_f64() * d.firm_working_days) + firm.pledged + saving;
            // (A House keeps back a branch's capital and half again its working days at home's wages, saving for one: a
            // town down its roads may pay more.)
            if firm.house.is_some() && branch.is_none() {
                keep += Fixed::ceil(kind.capital) + Fixed::ceil(1.5 * offer.to_f64() * d.firm_working_days);
            }
            if firm.treasury > keep {
                let beyond = firm.treasury - keep;
                if branch.is_some() {
                    firm.due = beyond;
                } else {
                    pay_owners(firm, homes, people, &mut ledger, &mut book, beyond, "dividends", now, day);
                }
            }
        }
        for id in done {
            let i = firms.firms.iter().position(|f| f.id == id).unwrap();
            let f = firms.firms.remove(i);
            firms.wound_up.push((f, now));
            firms.carried.retain(|c| c.firm != id);
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
            if graded_wanted_for(&content, people, c.output, par, needs.get(&c.output).copied().unwrap_or(0.0)) < c.out {
                continue;
            }
            // (The hand it's founded round, the best of those it would pay, it takes on: another kind founded today
            // needs another.)
            let mut first: Option<(f64, HandId)> = None;
            for x in roster.hands.iter().filter(|x| x.trade == kind.trade && x.baby_until.is_none() && !employed.contains(&x.id)) {
                let m = person(persons, x, &all);
                let w = best_step(&content, kind, m).0;
                if added(&content, &bids, c, Grade::of(w)) * pace(d, x, m, None) > offer.to_f64()
                        && first.is_none_or(|f| w > f.0) {
                    first = Some((w, x.id));
                }
            }
            let Some((_, hand)) = first else { continue };
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
            firm.employees.push((hand, offer));
            employed.insert(hand);
            firms.firms.push(firm);
        }
    }
    let routes: Vec<(usize, usize, usize, u64)> = routes.iter().map(|r| (r.seq, r.from_id, r.to_id, r.travel_ticks)).collect();
    branches(&content, &kinds, firms, persons, &all, &mut ledger, &mut book, &mut q, &towns.0, &routes, now);
}

/// A business's stations, once a day (`docs/firms-plan.md` slice 4): one broken yesterday is repaired, at
/// `station_repair_share` of its price, paid to its town (whose hands mend it); each breaks down `station_breakdown`
/// of days for each tier (half where a Tinkerer works it); and the business buys a step's next tier, up to
/// `station_tier_most`, where it has the money to spare beyond its working days and it would pay for itself in
/// `station_payback_days`: a second for the masterworks its first held back, a third (its last step's) for the
/// batches it's short of at its most hands. What it saves for one that would pay but it can't yet afford, its owners
/// are paid only beyond.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn stations(content: &Content, kind: &Kind, firm: &mut Firm, persons: &Persons, bids: &BTreeMap<GoodId, f64>,
        offer: Fixed, short: Option<(f64, f64)>, ledger: &mut Ledger, book: &mut CashBook, treasury: &mut Fixed,
        now: u64) -> Fixed {
    let d = &content.dials;
    let day = content.day_ticks.max(1);
    let today = now / day;
    let n = kind.steps.len();
    firm.stations.resize(n, 1);
    firm.broken.resize(n, false);
    firm.held_back.resize(n, 0.0);
    firm.held_today.resize(n, 0.0);
    let t = firm.town;
    // Its hands at each step (as at its work: where each brings most).
    let mut saving = Fixed::ZERO;
    let at: Vec<(HandId, usize)> = firm.employees.iter()
        .filter_map(|e| persons.hands.get(&e.0).map(|m| (e.0, best_step(content, kind, m).1))).collect();
    for s in 0..n {
        let tier = firm.station(s);
        let work = kind.steps[s].work;
        let cost = crate::firms::station_price(kind, tier);
        let hand = at.iter().find(|a| a.1 == s).map(|a| a.0).or(firm.head.map(|h| h.hand))
            .or(firm.employees.first().map(|e| e.0));
        if firm.broken[s] {
            let repair = Fixed::ceil(cost * d.station_repair_share).min(firm.treasury.at_least_zero());
            post(ledger, book, Side::firm(firm.id, &mut firm.treasury), Side::treasury(t, treasury), repair,
                Reason::Other("station repaired".into()), now, day);
            firm.broken[s] = false;
        }
        firm.held_back[s] += (firm.held_today[s] - firm.held_back[s]) * 0.2;
        firm.held_today[s] = 0.0;
        let tinkered = at.iter().any(|a| a.1 == s && persons.hands.get(&a.0).is_some_and(|m| m.has(Habit::Tinkerer)));
        let p = d.station_breakdown * tier as f64 * if tinkered { 0.5 } else { 1.0 };
        if at.iter().any(|a| a.1 == s) && roll(firm.id as u64 * 8 + s as u64, today, 61) < p {
            firm.broken[s] = true;
            if let Some(h) = hand {
                firm.note(today, h, Event::BrokeDown(work));
            }
            continue;
        }
        // Its next tier, if it would pay for itself.
        if (tier as f64) >= d.station_tier_most {
            continue;
        }
        let next = crate::firms::station_price(kind, tier + 1);
        let bill: Fixed = firm.employees.iter().map(|e| e.1).sum();
        let spare = firm.treasury - firm.pledged - Fixed::ceil(bill.to_f64() * d.firm_working_days) - Fixed::ceil(offer.to_f64());
        let output = kind.steps[s].output;
        let gain = match short {
            _ if tier < 2 => firm.held_back[s] * (price(content, bids, output, Grade::Masterwork)
                - price(content, bids, output, Grade::Fine)),
            // (A batch a day more for each its hands make at this station's pace, as far as it's short.)
            Some((wanted, capacity)) if s + 1 == n => {
                let more = (wanted - capacity).min(capacity * d.station_throughput / crate::firms::throughput(d, tier));
                more * added(content, bids, &kind.conversion, Grade::Fine).max(0.0)
            }
            _ => 0.0,
        };
        if gain * d.station_payback_days < next {
            continue;
        }
        if spare.to_f64() >= next {
            post(ledger, book, Side::firm(firm.id, &mut firm.treasury), Side::treasury(t, treasury), Fixed::ceil(next),
                Reason::Other("station bought".into()), now, day);
            firm.stations[s] = tier + 1;
            if let Some(h) = hand {
                firm.note(today, h, Event::Upgraded(work, tier + 1));
            }
        } else {
            saving = saving.max(Fixed::ceil(next));
        }
    }
    saving
}

/// A town's businesses' bids for their materials (`firms::bids`).
fn bids_in(content: &Content, kinds: &[Kind], firms: &Firms, t: usize) -> BTreeMap<GoodId, f64> {
    bids(content, firms.firms.iter().filter(|f| f.town == t).filter_map(|f| kinds.iter().find(|k| k.name == f.name)))
}

/// What a hand of town `t` would be offered by a business: more than it earns of the town's wages now.
fn offer_in(content: &Content, book: &CashBook, firms: &Firms, roster: &Roster, t: usize, today: u64) -> Fixed {
    let employed = firms.employed(t);
    let free = roster.hands.iter().filter(|x| x.baby_until.is_none() && !employed.contains(&x.id)).count();
    let d = &content.dials;
    Fixed::ceil(earned(book, t, free, today).max(d.hand_wage) * (1.0 + d.firm_wage_premium))
}

/// The hand of town `t` a House's branch there would be founded round: of its trade and free, not a Homebody (who
/// won't run a branch), the best at its work of those whose batches, at the grade they work at and the House's name's
/// premium there (`premium`), pay more than they'd be offered.
#[allow(clippy::too_many_arguments)]
fn branch_hand(content: &Content, kind: &Kind, persons: &mut Persons, all: &[String], roster: &Roster,
        employed: &BTreeSet<HandId>, bids: &BTreeMap<GoodId, f64>, premium: f64, offer: Fixed) -> Option<HandId> {
    let mut best: Option<(f64, HandId)> = None;
    for x in roster.hands.iter().filter(|x| x.trade == kind.trade && x.baby_until.is_none() && !employed.contains(&x.id)) {
        let m = person(persons, x, all);
        if m.has(Habit::Homebody) {
            continue;
        }
        let w = best_step(content, kind, m).0;
        if added(content, bids, &kind.conversion, Grade::of(w)) * (1.0 + premium) * pace(&content.dials, x, m, None)
                > offer.to_f64() && best.is_none_or(|b| w > b.0) {
            best = Some((w, x.id));
        }
    }
    best.map(|b| b.1)
}

/// Two firms of the list, by id, both to change.
fn two(firms: &mut [Firm], a: FirmId, b: FirmId) -> Option<(&mut Firm, &mut Firm)> {
    let i = firms.iter().position(|f| f.id == a)?;
    let j = firms.iter().position(|f| f.id == b)?;
    match i.cmp(&j) {
        std::cmp::Ordering::Less => { let (x, y) = firms.split_at_mut(j); Some((&mut x[i], &mut y[0])) }
        std::cmp::Ordering::Greater => { let (x, y) = firms.split_at_mut(i); Some((&mut y[0], &mut x[j])) }
        std::cmp::Ordering::Equal => None,
    }
}

/// The days money a business sent by wagon may ride before it's taken as lost on the road (its pledge freed).
const LOST_DAYS: u64 = 60;

/// Branches (`docs/firms-plan.md` slice 3; John: "Rides the wagon"): the money businesses send by wagon, and the
/// Houses that send for a branch. Money that's there: a House's capital founds its branch in the town it went to (its
/// House's name, half its reputation, owned by it; its capital the town's, which builds its workshop, its working days
/// the branch's own; round the hand it's founded on, whose head it is by the House's mandate), or comes back to it if
/// there's no founding it now; a branch's takings home are its House's. Money riding longer than [`LOST_DAYS`] is
/// taken as lost on the road, its pledge freed. Then each House reckoned today, not a branch, with no branch on its
/// way and the capital and a branch's working days to spare beyond its own, sends for one in the nearest town down its
/// roads with no business of its kind, that wants its pieces and has a hand who'd be founded on, by its route's next
/// wagon.
#[inline(never)]
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn branches(content: &Content, kinds: &[Kind], firms: &mut Firms, persons: &mut Persons, all: &[String],
        ledger: &mut Ledger, book: &mut CashBook,
        q: &mut Query<(&Town, &mut People, &mut Stock, &mut Money, &Roster, Option<&Workshop>),
            (Without<crate::realms::Realm>, Without<crate::realms::OffMap>)>,
        towns: &[Entity], routes: &[(usize, usize, usize, u64)], now: u64) {
    let d = &content.dials;
    let day = content.day_ticks.max(1);
    let today = now / day;
    let entity = |t: usize, q: &Query<(&Town, &mut People, &mut Stock, &mut Money, &Roster, Option<&Workshop>),
            (Without<crate::realms::Realm>, Without<crate::realms::OffMap>)>| {
        towns.iter().copied().find(|&e| q.get(e).is_ok_and(|x| x.0.id == t))
    };
    let mut kept = Vec::new();
    for c in std::mem::take(&mut firms.carried) {
        let unpledge = |firms: &mut Firms| if let Some(f) = firms.get_mut(c.firm) { f.pledged -= c.amount; };
        match (c.stage, c.errand) {
            (Stage::There, Errand::Home) => {
                let house = firms.firms.iter().find(|f| f.id == c.firm).and_then(|f| f.branch_of);
                match house.and_then(|h| two(&mut firms.firms, c.firm, h)) {
                    Some((branch, house)) => {
                        branch.pledged -= c.amount;
                        let amount = c.amount.min(branch.treasury.at_least_zero());
                        post(ledger, book, Side::firm(branch.id, &mut branch.treasury), Side::firm(house.id, &mut house.treasury),
                            amount, Reason::Other("branch takings".into()), now, day);
                    }
                    None => unpledge(firms),
                }
            }
            (Stage::There, Errand::Branch) => {
                let Some(h) = firms.firms.iter().find(|f| f.id == c.firm).cloned() else { continue };
                let kind = kinds.iter().find(|k| k.name == h.name);
                let e = entity(c.town, q);
                let taken = firms.firms.iter().any(|f| f.town == c.town && f.name == h.name);
                let founded = match (kind, e, taken) {
                    (Some(kind), Some(e), false) => {
                        let bids = bids_in(content, kinds, firms, c.town);
                        let employed = firms.employed(c.town);
                        let Ok((_, _, _, mut money, roster, _)) = q.get_mut(e) else { continue };
                        let offer = offer_in(content, book, firms, roster, c.town, today);
                        let branch = Firm { house: h.house.clone(), reputation: h.reputation / 2.0, ..Default::default() };
                        let hand = branch_hand(content, kind, persons, all, roster, &employed, &bids, branch.premium(d), offer);
                        let capital = Fixed::ceil(kind.capital).min(c.amount);
                        match hand {
                            Some(hand) => {
                                let id = firms.next;
                                firms.next += 1;
                                let mut b = Firm { id, town: c.town, name: h.name.clone(), recipe: h.recipe.clone(),
                                    owners: vec![(Party::Firm(h.id), 1.0)], founded: now, reckoned: Some(now),
                                    branch_of: Some(h.id), employees: vec![(hand, offer)], ..branch };
                                let house = firms.get_mut(h.id).unwrap();
                                house.pledged -= c.amount;
                                post(ledger, book, Side::firm(house.id, &mut house.treasury), Side::treasury(c.town, &mut money.0),
                                    capital, Reason::Other("branch capital".into()), now, day);
                                post(ledger, book, Side::firm(house.id, &mut house.treasury), Side::firm(id, &mut b.treasury),
                                    c.amount - capital, Reason::Other("branch founded".into()), now, day);
                                firms.firms.push(b);
                                true
                            }
                            None => false,
                        }
                    }
                    _ => false,
                };
                if !founded {
                    unpledge(firms);
                }
            }
            (_, _) if now >= c.since + LOST_DAYS * day => unpledge(firms),
            _ => kept.push(c),
        }
    }
    kept.append(&mut firms.carried);
    firms.carried = kept;
    // Each House reckoned today sends for a branch where it would found one.
    let houses: Vec<FirmId> = firms.firms.iter().filter(|f| f.house.is_some() && f.branch_of.is_none()
        && f.reckoned == Some(now) && !firms.carried.iter().any(|c| c.firm == f.id && c.errand == Errand::Branch))
        .map(|f| f.id).collect();
    for h in houses {
        let f = firms.firms.iter().find(|f| f.id == h).unwrap().clone();
        let Some(kind) = kinds.iter().find(|k| k.name == f.name) else { continue };
        let bill: Fixed = f.employees.iter().map(|e| e.1).sum();
        let spare = f.treasury - f.pledged - Fixed::ceil(bill.to_f64() * d.firm_working_days);
        let mut down: Vec<&(usize, usize, usize, u64)> = routes.iter().filter(|r| r.1 == f.town && r.2 != f.town).collect();
        down.sort_by_key(|r| (r.3, r.0));
        for &&(route, _, to, _) in &down {
            if firms.firms.iter().any(|x| x.town == to && x.name == f.name)
                || firms.carried.iter().any(|c| c.town == to && c.errand == Errand::Branch) {
                continue;
            }
            let Some(e) = entity(to, q) else { continue };
            let bids = bids_in(content, kinds, firms, to);
            let employed = firms.employed(to);
            let Ok((_, people, _, _, roster, hooks)) = q.get(e) else { continue };
            let c = &kind.conversion;
            let par = hooks.map_or(0.0, |h| h.par_of(c.output));
            if graded_wanted_for(content, people, c.output, par, 0.0) < c.out {
                continue;
            }
            let offer = offer_in(content, book, firms, roster, to, today);
            let amount = Fixed::ceil(kind.capital) + Fixed::ceil(offer.to_f64() * d.firm_working_days);
            if spare < amount {
                break;
            }
            let branch = Firm { house: f.house.clone(), reputation: f.reputation / 2.0, ..Default::default() };
            if branch_hand(content, kind, persons, all, roster, &employed, &bids, branch.premium(d), offer).is_none() {
                continue;
            }
            firms.carried.push(Carried { errand: Errand::Branch, firm: h, town: to, route, amount, stage: Stage::Waiting,
                since: now });
            firms.get_mut(h).unwrap().pledged += amount;
            break;
        }
    }
}

/// The names of a step's materials, bare (a material talent's).
fn materials_of(content: &Content, step: &crate::firms::Step) -> Vec<String> {
    let mut all: Vec<String> = step.recipes.iter().flat_map(|c| ingredients(content, c)).collect();
    all.sort();
    all.dedup();
    all
}

/// What a hand brings to its firm at its best step, and which step that is (the later on a tie: a bootmaker's hands
/// sew unless they're tanners at heart).
fn best_step(content: &Content, kind: &Kind, m: &Person) -> (f64, usize) {
    kind.steps.iter().enumerate().map(|(i, s)| (fundamentals(m, s.work, &materials_of(content, s)), i))
        .fold((f64::NEG_INFINITY, 0), |a, b| if b.0 >= a.0 { b } else { a })
}

/// A hand's knack at work: its own, or a Copycat's, the knack it copies (`copied`).
fn knack(m: &Person, copied: Option<Knack>) -> Option<Knack> {
    match m.knack {
        Some(Knack::Copycat) => copied,
        k => k,
    }
}

/// Where a batch's materials came from: its own firm's earlier step, its town's graded pieces (a business's), or its
/// town's plain stock.
fn take_materials(people: &mut People, stock: &Stock, pool: &mut BTreeMap<GoodId, [f64; 5]>, g: GoodId, n: f64,
        plain_only: bool) -> ([f64; 5], [f64; 5]) {
    let mut own = [0.0; 5];
    let mut left = n;
    if let Some(of) = pool.get_mut(&g).filter(|_| !plain_only) {
        for grade in [Grade::Masterwork, Grade::Fine, Grade::Good, Grade::Plain, Grade::Seconds] {
            let t = of[grade.index()].min(left);
            of[grade.index()] -= t;
            own[grade.index()] += t;
            left -= t;
        }
    }
    let mut bought = [0.0; 5];
    if left > 1e-9 {
        if plain_only {
            bought[Grade::Plain.index()] = left;
        } else {
            let pooled: f64 = pool.get(&g).map_or(0.0, |of| of.iter().sum());
            bought = crate::people::take_graded(&mut people.graded, g, stock.0[g] - pooled, left);
        }
    }
    (own, bought)
}

/// A firm's work since its last look, step by step: what its town wants of its graded pieces (`graded_wanted_for`;
/// noted, for its hiring) its last step makes, and each earlier step what the next will work of its output that its
/// town's businesses haven't made (bought of them first), as far as each step's hands' batches for the time passed go
/// (a day's at most carried; none of a hand out for the day), the inputs (not what's set aside for forward sales) and
/// the town's treasury pays. Each batch is graded by its maker's Fundamentals (with its materials' grades) × the shop's
/// Morale, give or take the batch's own draw, as its maker's knack bends it, and its maker the more skilled for it.
/// Its pieces are sold to the town at their grades' worth, and the materials it buys of the town paid for at theirs.
// (Not inlined into `run`: together they're too big a function for the JVM to compile the wasm of.)
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn work(content: &Content, kind: &Kind, firm: &mut Firm, persons: &mut Persons, all: &[String], roster: &Roster,
        people: &mut People, stock: &mut Stock, money: &mut Money, hooks: Option<&Workshop>, ledger: &mut Ledger,
        book: &mut CashBook, now: u64, needed: f64, makers: &BTreeMap<GoodId, FirmId>, bids: &BTreeMap<GoodId, f64>,
        copied: Option<Knack>) {
    let d = &content.dials;
    let day = content.day_ticks.max(1);
    let today = now / day;
    let t = firm.town;
    let last = kind.steps.len() - 1;
    let output = kind.steps[last].output;
    let par = hooks.map_or(0.0, |h| h.par_of(output));
    let wanted = graded_wanted_for(content, people, output, par, needed);
    firm.eaten += wanted;
    let since = firm.worked.map_or(0.0, |w| now.saturating_sub(w) as f64 / day as f64);
    firm.worked = Some(now);
    // Its hands at work, each at the step it brings most to (the last step never without one), and how fast each goes.
    let head = firm.head.and_then(|h| persons.hands.get(&h.hand).map(|m| (h.hand, m.clone())));
    let mut crew: Vec<(HandId, usize, f64, u8)> = firm.employees.iter().filter_map(|&(id, _)| {
        let x = roster.hands.iter().find(|x| x.id == id && x.trade == kind.trade && x.baby_until.is_none())?;
        let m = person(persons, x, all);
        m.out_until.is_none_or(|u| now >= u)
            .then(|| (id, best_step(content, kind, m).1, pace(d, x, m, head.as_ref().map(|(h, m)| (*h, m))), x.level))
    }).collect();
    if !crew.is_empty() && !crew.iter().any(|c| c.1 == last) {
        let at_last = |id: HandId| persons.hands.get(&id)
            .map_or(0.0, |m| fundamentals(m, kind.steps[last].work, &materials_of(content, &kind.steps[last])));
        let i = (0..crew.len()).max_by(|&a, &b| at_last(crew[a].0).total_cmp(&at_last(crew[b].0)).then(crew[b].0.cmp(&crew[a].0)))
            .unwrap();
        crew[i].1 = last;
    }
    firm.progress.resize(kind.steps.len(), 0.0);
    // (Each step at its station's pace; none at a broken one's.)
    let capacity: Vec<f64> = (0..=last).map(|s| if firm.is_broken(s) { 0.0 } else {
        crew.iter().filter(|c| c.1 == s).map(|c| c.2).sum::<f64>() * crate::firms::throughput(d, firm.station(s))
    }).collect();
    firm.held_today.resize(kind.steps.len(), 0.0);
    for (p, &c) in firm.progress.iter_mut().zip(&capacity) {
        *p = (*p + c * since).min(c.max(1.0));
    }
    if crew.is_empty() {
        return;
    }
    // The shop's morale: its crew's spirits, its head's habits.
    let spirits: f64 = crew.iter().filter_map(|c| persons.hands.get(&c.0)).map(|m| m.spirits).sum::<f64>() / crew.len() as f64;
    let morale = crate::persons::morale(spirits, head.as_ref().map(|h| &h.1));
    let mentor = head.as_ref().is_some_and(|h| h.1.has(Habit::Mentor));
    // (House style: the other Masters of the crew who share a talent with each.)
    let masters: Vec<(HandId, Vec<crate::persons::Talent>)> = crew.iter().filter(|c| c.3 >= 5)
        .filter_map(|c| persons.hands.get(&c.0).map(|m| (c.0, m.talents.clone()))).collect();
    // How many batches each step makes: the last what its town wants, as far as its progress and the town's treasury
    // go; an earlier one what the next would work of its output beyond the graded pieces its town holds.
    let mut target = vec![0.0; kind.steps.len()];
    let out_last = kind.steps[last].recipes.iter().map(|c| c.out).fold(f64::INFINITY, f64::min);
    let best = price(content, bids, output, Grade::Masterwork) * (1.0 + firm.premium(d));
    let affords = (money.0.at_least_zero().to_f64() / (out_last * best).max(1e-9)).floor();
    target[last] = (wanted / out_last - 1e-9).ceil().max(0.0).min(affords).min(firm.progress[last].floor());
    for s in (0..last).rev() {
        let g = kind.steps[s].output;
        let per = kind.steps[s + 1].recipes[0].inputs.iter().find(|i| i.0 == g).map_or(0.0, |i| i.1);
        let held: f64 = people.graded.get(&g).map_or(0.0, |of| of.iter().sum());
        let out = kind.steps[s].recipes[0].out.max(1e-9);
        target[s] = ((target[s + 1] * per - held) / out - 1e-9).ceil().max(0.0).min(firm.progress[s].floor());
    }
    let mut pool: BTreeMap<GoodId, [f64; 5]> = BTreeMap::new();
    let mut made = [0.0; 5];
    let mut cost = 0.0;
    let mut fame = 0.0;
    for s in 0..=last {
        let step = &kind.steps[s];
        let ing = materials_of(content, step);
        let hands: Vec<(HandId, f64, u8)> = crew.iter().filter(|c| c.1 == s).map(|c| (c.0, c.2, c.3)).collect();
        let cap: f64 = hands.iter().map(|h| h.1).sum();
        let mut given = vec![0.0; hands.len()];
        for b in 0..target[s] as usize {
            if hands.is_empty() {
                break;
            }
            // Its recipe: the first its inputs are there for (a hide cured only of the plain ones: the graded are
            // cured already; its own step's pieces count, as do its town's).
            let avail = |g: GoodId, pool: &BTreeMap<GoodId, [f64; 5]>| {
                let aside = hooks.map_or(0.0, |h| h.aside_of(g));
                let graded: f64 = people.graded.get(&g).map_or(0.0, |of| of.iter().sum());
                let pooled: f64 = pool.get(&g).map_or(0.0, |of| of.iter().sum());
                if g == step.output { stock.0[g] - aside - graded - pooled } else { stock.0[g] - aside }
            };
            let Some(c) = step.recipes.iter().find(|c| c.inputs.iter().all(|&(g, n)| avail(g, &pool) >= n - 1e-9)) else { break };
            // Each batch to the hand furthest behind its share.
            let i = (0..hands.len()).max_by(|&x, &y| {
                let behind = |k: usize| hands[k].1 * (b + 1) as f64 / cap - given[k];
                behind(x).total_cmp(&behind(y)).then(hands[y].0.cmp(&hands[x].0))
            }).unwrap();
            given[i] += 1.0;
            let (id, _, level) = hands[i];
            let mut took = [0.0; 5];
            for &(g, n) in &c.inputs {
                let (own, bought) = take_materials(people, stock, &mut pool, g, n, g == step.output);
                stock.0[g] -= n;
                ledger.used[g] += n;
                cost += Grade::ALL.iter().map(|gr| bought[gr.index()] * price(content, bids, g, *gr)).sum::<f64>();
                // (A business's graded pieces: its supplier's, and a Supply whisperer's a grade higher.)
                let theirs: f64 = Grade::COUNTED.iter().map(|gr| bought[gr.index()]).sum();
                if let Some(&f) = makers.get(&g).filter(|&&f| f != firm.id && theirs > 0.0) {
                    match firm.suppliers.iter_mut().find(|x| x.0 == f) {
                        Some(x) => x.1 += theirs,
                        None => firm.suppliers.push((f, theirs)),
                    }
                }
                let m = persons.hands.get(&id).unwrap();
                // (A graded piece is a business's: of this town, or carried in.)
                let whispers = knack(m, copied) == Some(Knack::SupplyWhisperer);
                for gr in Grade::ALL {
                    took[gr.index()] += own[gr.index()];
                    let to = if whispers && gr != Grade::Plain { gr.up() } else { gr };
                    took[to.index()] += bought[gr.index()];
                }
            }
            let m = persons.hands.get_mut(&id).unwrap();
            let k = knack(m, copied);
            let mut brought = fundamentals(m, step.work, &ing) + crate::persons::materials_grade(&took) + m.scrap;
            m.scrap = 0.0;
            if k == Some(Knack::GlassHands) {
                brought *= 2.0;
            }
            let mut lift = crate::persons::own_form(m, now);
            lift += match k {
                Some(Knack::HouseStyle) => 0.1 * masters.iter().filter(|(o, ts)| *o != id
                    && ts.iter().any(|x| m.talents.contains(x))).count() as f64,
                Some(Knack::Unbroken) => 0.05 * today.saturating_sub(m.seconds_on).min(10) as f64,
                Some(Knack::TheRegular) => 0.05 * firm.regular.min(10) as f64,
                _ => 0.0,
            };
            let spread = d.grade_spread * if m.has(Habit::Experimenter) { 2.0 } else { 1.0 };
            let mut grade = Grade::of(crate::persons::score(brought, morale + lift, spread, roll(firm.id as u64, firm.batches, 11)));
            if k == Some(Knack::SecondProof) && m.batches % 3 == 2 {
                grade = grade.max(Grade::of(crate::persons::score(brought, morale + lift, spread,
                    roll(firm.id as u64, firm.batches, 12))));
            }
            if m.has(Habit::SteadyHand) {
                grade = grade.max(Grade::Plain);
            }
            if k == Some(Knack::RabbitsFoot) && c.inputs.iter().any(|i| content.goods[i.0].name.ends_with("rabbit_hide")) {
                grade = grade.up();
            }
            // (No better than its station lets it be: what it held back, noted.)
            let top = crate::firms::ceiling(firm.station(s));
            if grade > top {
                firm.held_today[s] += c.out;
                grade = top;
            }
            let ruined = k == Some(Knack::GlassHands) && m.batches % 10 == 9;
            let scrapped = !ruined && grade == Grade::Seconds && k == Some(Knack::ScrapArtist);
            if grade == Grade::Seconds {
                m.seconds_on = today;
            }
            if scrapped {
                m.scrap += 4.0;
            }
            m.batches += 1;
            firm.batches += 1;
            firm.progress[s] -= 1.0;
            let gain = d.skill_per_batch * if m.gifted(step.work) { 1.5 } else { 1.0 }
                * if m.has(Habit::Ambitious) { 1.5 } else { 1.0 } * if mentor { 2.0 } else { 1.0 };
            let skill = m.skills.entry(step.work).or_insert(0.0);
            *skill = (*skill + gain).min(skill_cap(level).max(*skill));
            if ruined {
                firm.note(today, id, Event::Ruined);
                continue;
            }
            if scrapped {
                continue;
            }
            stock.0[step.output] += c.out;
            ledger.made[step.output] += c.out;
            if s < last {
                pool.entry(step.output).or_insert([0.0; 5])[grade.index()] += c.out;
            } else {
                made[grade.index()] += c.out;
                // Its name, by what its pieces are worth beyond plain ones (seconds take from it): twice a Patron's, sold
                // in its home town; half again a Showman's.
                let m = persons.hands.get(&id).unwrap();
                fame += c.out * content.goods[step.output].value * (grade.worth(d) - 1.0)
                    * if k == Some(Knack::Patron) { 2.0 } else { 1.0 } * if m.has(Habit::Showman) { 1.5 } else { 1.0 };
            }
        }
    }
    firm.reputation += fame;
    // What its own earlier steps made beyond what the next worked is sold to the town with the rest; what it makes,
    // a House's, at its name's premium.
    let mut sales = 0.0;
    let premium = firm.premium(d);
    for (g, of) in pool.iter().chain(std::iter::once((&output, &made))) {
        let graded = people.graded.entry(*g).or_insert([0.0; 5]);
        for gr in Grade::COUNTED {
            graded[gr.index()] += of[gr.index()];
        }
        let lift = if *g == output { 1.0 + premium } else { 1.0 };
        sales += Grade::ALL.iter().map(|gr| of[gr.index()] * price(content, bids, *g, *gr) * lift).sum::<f64>();
    }
    people.graded.retain(|_, of| of.iter().any(|x| *x > 0.0));
    for (a, b) in firm.grades.iter_mut().zip(made) {
        *a += b;
    }
    let pieces: f64 = made.iter().sum();
    firm.made += pieces;
    firm.sold += pieces;
    post(ledger, book, Side::treasury(t, &mut money.0), Side::firm(firm.id, &mut firm.treasury), Fixed::floor(sales),
        Reason::Other("firm sales".into()), now, day);
    let bought = Fixed::ceil(cost).min(firm.treasury);
    post(ledger, book, Side::firm(firm.id, &mut firm.treasury), Side::treasury(t, &mut money.0), bought,
        Reason::Other("firm inputs".into()), now, day);
}

/// A firm's hands' spirits for the day, and what they bring. Each heads a third of the way toward its mark: a little
/// over a half, more at work it has a talent for, more under a Beloved head and less under a Taskmaster (its head's own
/// not), more among its kin under a Kin-minded one, and much less while its wages go unpaid. High spirits now and then bring an inspiration (three days of
/// better work; twice as often a Hothead's, half a Steady hand's); low ones an off-form spell: it walks out for the
/// day (never a Homebody), quarrels with another of the crew (both the lower for it; a Hothead's likelier), or sulks
/// (two days of worse work). An Ambitious hand ten days in low spirits leaves.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn spirits<'r>(kind: &Kind, firm: &mut Firm, persons: &mut Persons, all: &[String],
        ours: &dyn Fn(HandId) -> Option<&'r Hand>, homes: &Homes, copied: Option<Knack>, now: u64, today: u64, day: u64) {
    let head = firm.head.and_then(|h| persons.hands.get(&h.hand).map(|m| (h.hand, m.clone())));
    let crew: Vec<HandId> = firm.employees.iter().map(|e| e.0).collect();
    // A hand become a Master finds its knack.
    for &id in &crew {
        let Some(x) = ours(id).filter(|x| x.level >= 5) else { continue };
        let m = person(persons, x, all);
        if m.knack.is_none() {
            let k = knack_of(id);
            m.knack = Some(k);
            m.seconds_on = today;
            firm.note(today, id, Event::Mastered(k));
        }
    }
    // (A Peacemaker in the shop: no quarrels.)
    let peace = crew.iter().any(|id| persons.hands.get(id).is_some_and(|m| knack(m, copied) == Some(Knack::Peacemaker)));
    let mut left = Vec::new();
    for &id in &crew {
        let Some(x) = ours(id) else { continue };
        let kin = head.as_ref().is_some_and(|(h, m)| *h != id && m.has(Habit::KinMinded)
            && home_of(homes, *h).is_some() && home_of(homes, *h) == home_of(homes, id));
        let m = person(persons, x, all);
        let mut mark: f64 = 0.55 + if kind.steps.iter().any(|s| m.gifted(s.work)) { 0.15 } else { 0.0 };
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
            // (A Showman's off-form spells are talked of.)
            if m.has(Habit::Showman) {
                firm.reputation -= 0.05 * firm.reputation.abs();
            }
            let others: Vec<HandId> = crew.iter().copied().filter(|&o| o != id).collect();
            let walk = if m.has(Habit::Homebody) { 0.0 } else { 1.0 };
            let quarrel = if others.is_empty() || peace { 0.0 } else if hot { 2.0 } else { 1.0 };
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
