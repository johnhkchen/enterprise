//! Property's rules (`property.rs`): each day a town's fields pass to their heirs, are bought, worked, let, sold and
//! homesteaded, and what they earn moves the households' shares.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use crate::charters::Party;
use crate::content::Content;
use crate::farms::{Farm, FREEHOLD_SHARE, YIELD_PER_PLOT_PER_DAY};
use crate::fixed::Fixed;
use crate::households::{gather, pay, Household, HouseholdId, Households};
use crate::property::{Acquired, Claim, Contest, Dispute, Field, Lease, Register, Target};
use crate::roster::Roster;
use crate::rules::Step;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    // (after the farm's review, which buys the households' fields, and the claims' register; before the jobs' reviews,
    // and so the rest of the reviews)
    schedule.add_systems(fields.in_set(Step::Review).after(super::registers::register_claims).after(super::farms::review)
        .before(super::jobs::relocate).before(super::jobs::review));
}

/// The plots of a field of `crop` (a 9 x 9 less its water).
fn field_size(content: &Content, crop: usize) -> u32 {
    crate::farms::field_plots(content, crop).max(1)
}

/// What a field earns its Farmers a day, as a freehold: its plots' yield at its crop's worth, their share of it.
pub fn earns(content: &Content, f: &Field) -> f64 {
    f.plots as f64 * YIELD_PER_PLOT_PER_DAY * content.goods.get(f.crop).map_or(0.0, |g| g.value) * FREEHOLD_SHARE
}

/// The grown Farmers of each household.
fn farmers(content: &Content, roster: &Roster, list: &[Household]) -> Vec<usize> {
    let Some(farmer) = content.trade("farmer") else { return vec![0; list.len()] };
    list.iter().map(|h| h.members.iter()
        .filter(|&&id| roster.hands.iter().any(|x| x.id == id && x.trade == farmer && x.baby_until.is_none())).count())
        .collect()
}

/// Once a day, each town with a farm and households: its fields handed on, bought, worked or let, paid for, sold and
/// homesteaded (`property.rs`).
#[allow(clippy::too_many_arguments)]
pub fn fields(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, mut households: ResMut<Households>,
        mut register: ResMut<Register>,
        mut q: Query<(&Town, &mut Farm, &Roster), (Without<crate::realms::Realm>, Without<crate::realms::OffMap>)>) {
    let (now, day) = (clock.now, content.day_ticks.max(1));
    let d = &content.dials;
    let register = &mut *register;
    for &e in &towns.0 {
        let Ok((town, mut farm, roster)) = q.get_mut(e) else { continue };
        let t = town.id;
        let estate = register.estates.entry(t).or_default();
        if matches!(estate.reckoned, Some(k) if now < k + day) {
            continue;
        }
        // (The register's first day: no wages read before it.)
        let first = estate.reckoned.is_none();
        estate.reckoned = Some(now);
        let paid = if first { Fixed::ZERO } else { farm.wages_paid - estate.wages_seen };
        estate.wages_seen = farm.wages_paid;
        let Some(homes) = households.towns.get_mut(&t) else { continue };
        let gone = std::mem::take(&mut homes.gone);
        let hid: Vec<HouseholdId> = homes.list.iter().map(|h| h.id).collect();
        let at: BTreeMap<HouseholdId, usize> = hid.iter().enumerate().map(|(i, h)| (*h, i)).collect();
        let owner_of = |register: &Register, f: u32| match register.field_owner(f) {
            Some(Party::Household(h)) => at.get(&h).copied(),
            _ => None,
        };

        // Inherited: a household gone hands its fields to its kin; with none, the town takes them into its estate,
        // and a tenant working one is left with a dispute. A tenant gone gives its lease up.
        for (h, kin) in gone {
            register.leases.retain(|_, l| l.tenant != h);
            let held: Vec<u32> = register.fields_of(t).map(|(f, _)| *f)
                .filter(|f| register.field_owner(*f) == Some(Party::Household(h))).collect();
            for f in held {
                if let Some(k) = kin.filter(|k| at.contains_key(k)) {
                    register.claims.insert(Target::Field(f), Claim { id: register.next, owner: Party::Household(k),
                        how: Acquired::Inherited, since: now });
                    register.next += 1;
                    continue;
                }
                if let Some(l) = register.leases.remove(&f) {
                    register.disputes.push(Dispute { id: register.next_dispute, town: t, target: Target::Field(f),
                        claimants: vec![Party::Household(l.tenant), Party::Town(t)], why: Contest::Inheritance, since: now });
                    register.next_dispute += 1;
                }
                let plots = register.fields.remove(&f).map_or(0, |x| x.plots);
                register.claims.remove(&Target::Field(f));
                farm.freeholds = farm.freeholds.saturating_sub(plots);
            }
        }

        // The freeholds as the farm has them: those it no longer has struck out, newest first; each it bought for a
        // household (`rules/farms.rs`) that household's; any else (a host's, a save's from before the register) the
        // Farmer household's with room for it that has most.
        let bought = std::mem::take(&mut homes.bought);
        let list = &mut homes.list;
        let fs = farmers(&content, roster, list);
        let held_by = |register: &Register, i: usize| register.fields_of(t)
            .filter(|(f, _)| register.field_owner(**f) == Some(Party::Household(hid[i]))).count();
        let mut registered: u32 = register.fields_of(t).map(|(_, f)| f.plots).sum();
        while registered > farm.freeholds {
            let Some(f) = register.fields_of(t).map(|(f, _)| *f).last() else { break };
            registered -= register.fields.remove(&f).map_or(0, |x| x.plots);
            register.claims.remove(&Target::Field(f));
            register.leases.remove(&f);
        }
        let mut new = Vec::new();
        for (h, crop, plots) in bought {
            if registered + plots <= farm.freeholds {
                new.push((at.get(&h).copied(), crop, plots));
                registered += plots;
            }
        }
        let crop = farm.plots.iter().max_by_key(|(_, n)| **n).map(|(g, _)| *g);
        while let Some(crop) = crop.filter(|_| registered < farm.freeholds) {
            let plots = (farm.freeholds - registered).min(field_size(&content, crop));
            new.push((None, crop, plots));
            registered += plots;
        }
        for (owner, crop, plots) in new {
            let Some(o) = owner.or_else(|| (0..list.len())
                .filter(|&i| (fs[i] as f64 * d.fields_per_farmer) > held_by(register, i) as f64)
                .max_by_key(|&i| (list[i].purse, std::cmp::Reverse(i)))
                .or_else(|| (0..list.len()).max_by_key(|&i| (list[i].purse, std::cmp::Reverse(i))))) else { break };
            let f = register.next_field;
            register.next_field += 1;
            register.fields.insert(f, Field { town: t, crop, plots, worked_by: None, idle_since: None });
            register.claims.insert(Target::Field(f), Claim { id: register.next, owner: Party::Household(hid[o]),
                how: Acquired::Bought, since: now });
            register.next += 1;
        }

        // Worked or let: a household works as many of its fields as it has Farmers, and lets the rest to a Farmer
        // household with hands to spare, the one working fewest; a field neither lies idle.
        let mut working = vec![0usize; list.len()];
        let ids: Vec<u32> = register.fields_of(t).map(|(f, _)| *f).collect();
        let mut unworked = Vec::new();
        for &f in &ids {
            let Some(o) = owner_of(register, f) else { continue };
            if working[o] < fs[o] {
                working[o] += 1;
                register.leases.remove(&f);
                let field = register.fields.get_mut(&f).unwrap();
                field.worked_by = Some(list[o].id);
                field.idle_since = None;
            } else {
                register.fields.get_mut(&f).unwrap().idle_since.get_or_insert(now);
                unworked.push((f, o));
            }
        }
        unworked.retain(|&(f, _)| {
            let tenant = register.leases.get(&f).and_then(|l| at.get(&l.tenant).copied());
            match tenant {
                Some(i) if working[i] < fs[i] => {
                    working[i] += 1;
                    register.fields.get_mut(&f).unwrap().worked_by = Some(list[i].id);
                    false
                }
                _ => {
                    register.leases.remove(&f);
                    true
                }
            }
        });
        for &(f, o) in &unworked {
            let field = register.fields.get_mut(&f).unwrap();
            let tenant = (0..list.len()).filter(|&i| i != o && working[i] < fs[i]).min_by_key(|&i| (working[i], i));
            if let Some(i) = tenant {
                working[i] += 1;
                field.worked_by = Some(list[i].id);
                register.leases.insert(f, Lease { tenant: list[i].id, share: d.lease_share, since: now });
            } else {
                field.worked_by = None;
            }
        }

        // Paid: of the Farmers' wages the town paid since, each field's share by its plots of those tended moves to the
        // household working it, and of that a tenant's rent to the owner.
        let tended = farm.per_day.values().sum::<f64>() / YIELD_PER_PLOT_PER_DAY;
        if paid.is_positive() && tended > 0.0 {
            for &f in &ids {
                let field = &register.fields[&f];
                let Some(w) = field.worked_by.and_then(|h| at.get(&h).copied()) else { continue };
                let earned = gather(list, w, paid.mul_floor((field.plots as f64 / tended).min(1.0)));
                if let (Some(l), Some(o)) = (register.leases.get(&f), owner_of(register, f)) {
                    pay(list, w, o, earned.mul_floor(l.share));
                }
            }
        }

        // Sold: a field its household hasn't worked for long enough goes to a Farmer household with room for it that
        // can pay for it, its tenant first.
        for &f in &ids {
            let field = &register.fields[&f];
            if !field.idle_since.is_some_and(|s| now >= s + (d.idle_sale_days * day as f64) as u64) {
                continue;
            }
            let Some(o) = owner_of(register, f) else { continue };
            let price = Fixed::ceil(earns(&content, field) * d.land_price_days);
            let tenant = register.leases.get(&f).and_then(|l| at.get(&l.tenant).copied());
            let buyer = (0..list.len()).filter(|&i| i != o && list[i].purse >= price
                    && (working[i] < fs[i] || Some(i) == tenant)
                    && (fs[i] as f64 * d.fields_per_farmer) > held_by(register, i) as f64)
                .max_by_key(|&i| (Some(i) == tenant, list[i].purse, std::cmp::Reverse(i)));
            if let Some(b) = buyer {
                pay(list, b, o, price);
                if Some(b) != tenant {
                    working[b] += 1;
                    if let Some(i) = tenant {
                        working[i] -= 1;
                    }
                }
                register.leases.remove(&f);
                let field = register.fields.get_mut(&f).unwrap();
                field.worked_by = Some(list[b].id);
                field.idle_since = None;
                register.claims.insert(Target::Field(f), Claim { id: register.next, owner: Party::Household(list[b].id),
                    how: Acquired::Purchased, since: now });
                register.next += 1;
            }
        }

        // Homesteaded: a field's worth of the estate's plots untended for long enough is taken by the Farmer household
        // with hands to spare and room for it that holds least; the others that could have are left with a dispute.
        let (idle_crop, untended) = farm.idle.iter().max_by(|a, b| a.1.total_cmp(b.1))
            .map_or((None, 0.0), |(g, n)| (Some(*g), n / YIELD_PER_PLOT_PER_DAY));
        let estate = register.estates.get_mut(&t).unwrap();
        let Some(crop) = idle_crop.filter(|&g| untended >= field_size(&content, g) as f64) else {
            estate.untended_since = None;
            continue;
        };
        let since = *estate.untended_since.get_or_insert(now);
        if now < since + (d.homestead_days * day as f64) as u64 {
            continue;
        }
        let mut could: Vec<usize> = (0..list.len()).filter(|&i| working[i] < fs[i]
            && (fs[i] as f64 * d.fields_per_farmer) > held_by(register, i) as f64).collect();
        could.sort_by_key(|&i| (held_by(register, i), list[i].purse, i));
        let Some(&taker) = could.first() else { continue };
        register.estates.get_mut(&t).unwrap().untended_since = None;
        let plots = field_size(&content, crop);
        farm.freeholds += plots;
        let f = register.next_field;
        register.next_field += 1;
        register.fields.insert(f, Field { town: t, crop, plots, worked_by: Some(list[taker].id), idle_since: None });
        register.claims.insert(Target::Field(f), Claim { id: register.next, owner: Party::Household(list[taker].id),
            how: Acquired::Homesteaded, since: now });
        register.next += 1;
        if could.len() > 1 {
            let claimants = could.iter().map(|&i| Party::Household(list[i].id)).collect();
            register.disputes.push(Dispute { id: register.next_dispute, town: t, target: Target::Field(f), claimants,
                why: Contest::Homestead, since: now });
            register.next_dispute += 1;
        }
    }
}
