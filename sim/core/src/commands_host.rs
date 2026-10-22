//! What the host's commands ask of the core's books (`docs/binding-plan.md` 2, concern 11; task
//! `commands-and-studies`): `Query::CashBook`, `Books`, `Whys`, `Trust` and `Contracts`, the numbers behind
//! `/enterprise cashbook`, `books`, `whys`, `trust` and `contracts`. The rules are the core's own (`cash_book.rs`,
//! `atlas.rs`, `contracts.rs`); this only reads them, in words a command prints, so the studies that drive a server over
//! RCON and `sim/host` read the same numbers. Nothing here changes state, and none of it is saved.
//!
//! **Where the core differs from the mod's commands.** A cash book here has no reconciliation line against the opening
//! balances (the core's books balance by construction: `check.rs` proves every account against the cash book each day);
//! contracts' and rents' history lines are worked out from their state (the core keeps no list of events); a town goes
//! by the core's name for it, not the mod's `Names.label` of its desk, and money is in emeralds, not spurs.
//!
//! The sums a command shows are made here and carried on the answer (a cash book's money in and out and its totals by
//! what for), so a host only sets them out in words.
//!
//! Also here (task `commands-and-studies`, the studies on the core): `Query::Territory` (what `/enterprise territory`
//! prints of a town), `Query::War` (`/enterprise war` and `war tally`) and `Query::RouteBooks` (each route's totals for the
//! studies' per-day rows). All read what the core keeps; none changes it.

use std::collections::BTreeMap;

use crate::atlas::Kind;
use crate::fixed::Fixed;
use crate::messages::TownId;
use crate::society::Society;
use crate::state::Town;
use crate::wire::{no, Answer, BookLine, ContractLine, ForTotal, ForwardLine, HexAt, NameCount, ProjectLine, RentLine,
    RouteBook, SummaryLine, TrustLine, WarTown, WhyView};

impl Society {
    /// A town's name for the commands, "the town" for one that isn't there.
    fn named(&self, town: TownId) -> String {
        self.towns.get(town).and_then(|&e| self.world.get::<Town>(e)).map_or_else(|| "the town".to_string(), |t| t.name.clone())
    }

    fn lines(rows: Vec<(u64, String, Fixed)>) -> Vec<BookLine> {
        rows.into_iter().map(|(day, what, amount)| BookLine { day, what, amount }).collect()
    }

    /// The lines of a book summed by what for (in the order of the words).
    fn for_totals(lines: &[BookLine]) -> Vec<ForTotal> {
        let mut by: BTreeMap<&str, Fixed> = BTreeMap::new();
        for l in lines {
            let e = by.entry(l.what.as_str()).or_insert(Fixed::ZERO);
            *e = e.checked_add(l.amount).unwrap_or(Fixed::MAX);
        }
        by.into_iter().map(|(what, amount)| ForTotal { what: what.to_string(), amount }).collect()
    }

    /// `Query::CashBook`: a town's cash book for the last `days` days, and what was held back from its spenders; with
    /// the money in and out by what for and in all (the causes that came to more than nothing, and those that came to less).
    pub fn cash_book_answer(&self, town: TownId, days: u64) -> Answer {
        let today = self.now() / self.content().day_ticks.max(1);
        let (lines, held) = (Self::lines(self.cash_book(town, days)), Self::lines(self.held_back(town, days)));
        let totals = Self::for_totals(&lines);
        let sum = |keep: fn(Fixed) -> Option<Fixed>| totals.iter().filter_map(|t| keep(t.amount))
            .fold(Fixed::ZERO, |a, v| a.checked_add(v).unwrap_or(Fixed::MAX));
        let received = sum(|v| v.is_positive().then_some(v));
        let paid = sum(|v| (v < Fixed::ZERO).then_some(-v));
        Answer::CashBook { town: no(town), name: self.named(town), today, held_totals: Self::for_totals(&held), totals, lines, held,
            received, paid }
    }

    fn why_views(&self, town: TownId) -> Vec<WhyView> {
        self.whys(town).into_iter().map(|w| WhyView { line: w.line, times: w.times, items: w.items, last: w.last }).collect()
    }

    /// `Query::Whys`: a market's reasons, most often first.
    pub fn whys_answer(&self, town: TownId) -> Answer {
        Answer::Whys { town: no(town), name: self.named(town), whys: self.why_views(town) }
    }

    /// `Query::Books`: a market's dealings summed by kind, counterparty and good (in the order first dealt), with its
    /// reasons, and how many days its books go back.
    pub fn books_answer(&self, town: TownId) -> Answer {
        let content = self.content();
        let mut sums: Vec<(SummaryLine, (u8, Option<TownId>, Option<usize>))> = Vec::new();
        for e in self.books(town) {
            let kind = match e.kind {
                Kind::Bought => "bought",
                Kind::Sold => "sold",
                Kind::Sent => "sent",
                Kind::Home => "home",
                Kind::Shop => "shop",
                Kind::Why => continue,
            };
            let key = (e.kind as u8, e.with, e.good);
            if let Some((line, _)) = sums.iter_mut().find(|(_, k)| *k == key) {
                line.count += e.count;
                line.money = line.money.checked_add(e.money).unwrap_or(Fixed::MAX);
            } else {
                sums.push((SummaryLine { kind: kind.into(), with: e.with.map_or_else(|| "the town".to_string(), |w| self.named(w)),
                    good: e.good.map_or_else(|| "-".to_string(), |g| content.goods[g].name.clone()), count: e.count, money: e.money }, key));
            }
        }
        let e = self.world.entity(self.towns[town]);
        let keeps = crate::atlas::keeps(content, e.get::<crate::roster::Roster>(), e.get::<crate::works::Works>());
        Answer::Books { town: no(town), name: self.named(town), keeps,
            lines: sums.into_iter().map(|(l, _)| l).collect(), whys: self.why_views(town) }
    }

    /// `Query::Trust`: each pair's record that has dealt, as it stands now.
    pub fn trust_answer(&self) -> Answer {
        let (now, day) = (self.now(), self.content().day_ticks.max(1));
        let book = self.world.resource::<crate::contracts::Book>();
        let records: BTreeMap<_, _> = book.records.iter().map(|(&(a, b), _)| ((a, b), book.record(a, b, now, day))).collect();
        Answer::Trust { records: records.into_iter().map(|((a, b), r)| TrustLine { a: no(a), b: no(b),
            parties: format!("{} and {}", self.named(a), self.named(b)), trust: r.trust(), kept: r.kept, missed: r.missed }).collect() }
    }

    /// What a contract has done, as lines of words (`day N: ...`), from its state: when it was drafted and on what terms,
    /// the installments judged and missed, what has been delivered and paid, how it ended. The core keeps no list of events.
    fn contract_history(&self, c: &crate::contracts::Contract, what: &str) -> Vec<String> {
        let day = self.content().day_ticks.max(1);
        let today = self.now() / day;
        let mut out = vec![format!("day {}: drafted, {what} {} time(s) every {} day(s), {}", c.drafted / day, c.installments,
            c.every / day, format!("{:?}", c.incoterm).to_uppercase())];
        let judged = if c.state == crate::contracts::State::Completed { c.installments } else { c.judged };
        if judged > 0 {
            let last = (c.start + u64::from(judged - 1) * c.every) / day;
            out.push(format!("day {last}: {judged} of {} installment(s) judged, {} missed by the seller, {} by the buyer",
                c.installments, c.missed, c.buyer_missed));
        }
        out.push(format!("day {today}: {:.0} delivered, {:.0} on the road, {:.1} emeralds paid, {:.1} up front",
            c.delivered, c.shipped, c.paid.to_f64(), c.advanced.to_f64()));
        match c.state {
            crate::contracts::State::Completed => out.push(format!("day {today}: completed")),
            crate::contracts::State::Broken => out.push(format!("day {today}: broken")),
            _ => {}
        }
        out
    }

    fn rent_history(&self, r: &crate::harvest::Rent, crop: &str) -> Vec<String> {
        let day = self.content().day_ticks.max(1);
        let today = self.now() / day;
        let mut out = vec![format!("day {}: bought for {:.1} emeralds, {} {crop} a season", r.bought / day, r.capital.to_f64(), r.quantity),
            format!("day {today}: {} coupon(s) issued, {:.0} delivered, {:.0} in arrears", r.coupons, r.delivered, r.arrears)];
        match r.state {
            crate::harvest::RentState::Redeemed => out.push(format!("day {today}: redeemed")),
            crate::harvest::RentState::Defaulted => out.push(format!("day {today}: defaulted")),
            crate::harvest::RentState::Active => {}
        }
        out
    }

    /// `Query::Contracts`: every contract, or those the town is a party to, and the rents likewise.
    pub fn contracts_answer(&self, town: Option<TownId>) -> Answer {
        let content = self.content();
        let contracts = self.contracts().into_iter()
            .filter(|c| town.is_none_or(|t| c.seller == t || c.buyer == t))
            .map(|c| {
                let what = match &c.levy {
                    Some(l) => format!("a levy of {} soldiers", l.soldiers),
                    None => format!("{} {}", c.quantity, content.goods[c.good].name),
                };
                ContractLine {
                    id: no(c.id), seller: no(c.seller), buyer: no(c.buyer),
                    seller_name: self.named(c.seller), buyer_name: self.named(c.buyer),
                    state: format!("{:?}", c.state).to_lowercase(), incoterm: format!("{:?}", c.incoterm).to_uppercase(),
                    copies: u8::from(c.known_by_seller) + u8::from(c.known_by_buyer),
                    history: self.contract_history(&c, &what), what,
                    delivered: c.delivered, shipped: c.shipped, paid: c.paid, advanced: c.advanced, missed: c.missed,
                    buyer_missed: c.buyer_missed, trust: self.trust(c.seller, c.buyer),
                }
            }).collect();
        let rents = self.rents().into_iter().filter(|r| town.is_none_or(|t| r.farm == t || r.holder == t))
            .map(|r| {
                let crop = content.goods[r.crop].name.clone();
                RentLine { id: no(r.id), farm: no(r.farm), holder: no(r.holder), farm_name: self.named(r.farm),
                    holder_name: self.named(r.holder), quantity: r.quantity, history: self.rent_history(&r, &crop), crop,
                    capital: r.capital, state: format!("{:?}", r.state).to_lowercase(), coupons: r.coupons, delivered: r.delivered,
                    arrears: r.arrears }
            }).collect();
        Answer::Contracts { contracts, rents }
    }

    fn good_name(&self, good: usize) -> String {
        self.content().goods.get(good).map_or_else(|| "?".to_string(), |g| g.name.clone())
    }

    fn project_line(&self, p: &crate::works::Project) -> ProjectLine {
        use crate::works::What;
        let what = match p.what {
            What::Bridge { hex, .. } => format!("bridge on {},{}", hex.q, hex.r),
            What::Wall => "wall".to_string(),
            What::Hall(kind) => kind.id().to_string(),
        };
        ProjectLine { what, progress: p.progress(), done: p.done, wages: p.wages_paid }
    }

    /// What a town has sold forward of its crops: the contracts it is the seller of, in force, with a harvest term that is
    /// not a rent's coupon, as items a day at their locked price.
    fn sold_forward(&self, town: TownId) -> Vec<ForwardLine> {
        let day = self.content().day_ticks.max(1) as f64;
        let mut out: Vec<ForwardLine> = Vec::new();
        for c in self.contracts().iter().filter(|c| c.seller == town && c.in_force()) {
            if c.harvest.is_none_or(|h| h.carried) {
                continue;
            }
            let per_day = c.quantity * day / (c.every.max(1) as f64);
            let good = self.good_name(c.good);
            match out.iter_mut().find(|l| l.good == good) {
                Some(l) => l.per_day += per_day,
                None => out.push(ForwardLine { good, per_day, price: c.price }),
            }
        }
        out
    }

    /// `Query::Territory`: a town's land, farms, treasury, mood, people, posts and tolls, as `/enterprise territory` tells them.
    pub fn territory_answer(&self, town: TownId) -> Answer {
        let content = self.content();
        let rate = self.mint_rate();
        let v = self.town(town);
        let h = self.holding(town);
        let works = self.works(town);
        let farm = self.farm(town);
        let named = |m: &BTreeMap<usize, f64>| m.iter().filter(|(_, n)| **n > 0.0)
            .map(|(&g, &n)| NameCount { name: self.good_name(g), count: n }).collect::<Vec<_>>();
        let mood = v.people.mood;
        let f = farm.as_ref().map(|f| &f.farm);
        let (bridges, wall, buildings) = (
            works.projects.iter().filter(|p| matches!(p.what, crate::works::What::Bridge { .. })).map(|p| self.project_line(p)).collect(),
            works.projects.iter().find(|p| matches!(p.what, crate::works::What::Wall)).map(|p| self.project_line(p)),
            works.projects.iter().filter(|p| matches!(p.what, crate::works::What::Hall(_))).map(|p| self.project_line(p)).collect(),
        );
        Answer::Territory {
            town: no(town), name: v.name.clone(), rate, hexes: h.hexes.iter().map(|&x| HexAt::of(x)).collect(), culture: h.culture,
            next_hex: self.territory().next_hex_cost(town, content.dials.culture_per_hex),
            farms: f.map_or_else(Vec::new, |f| named(&f.per_day)), idle: f.map_or_else(Vec::new, |f| named(&f.idle)),
            sold_forward: self.sold_forward(town),
            wage_per_day: farm.as_ref().map_or(0.0, |f| f.wage_per_day), labor_share: f.map_or(0.0, |f| f.labor_share),
            crop_sales_per_day: f.map_or(0.0, |f| f.sales_per_day), wages_paid: f.map_or(Fixed::ZERO, |f| f.wages_paid),
            wages_unpaid: f.map_or(Fixed::ZERO, |f| f.wages_unpaid), harvested: f.map_or(0.0, |f| f.harvested),
            deposits: farm.as_ref().map_or(0.0, |f| f.deposits),
            focus: f.map_or_else(String::new, |f| format!("{:?}", f.focus).to_lowercase()),
            purse: v.people.purse, credit: f.map_or(Fixed::ZERO, |f| f.credit), freeholds: f.map_or(0, |f| f.freeholds),
            tightness: f.map_or(1.0, |f| f.tightness), treasury: v.money, reserve: self.emerald_reserve(town), mood,
            villagers: v.people.villagers, beds: v.people.beds,
            comforts: v.people.comfort_met.iter().enumerate().map(|(k, &m)| NameCount {
                name: content.comforts.get(k).cloned().unwrap_or_else(|| format!("comfort {k}")), count: m }).collect(),
            yield_factor: crate::farms::yield_factor(mood), culture_factor: crate::territory::culture_factor(mood),
            bridges, wall, buildings, hunting: h.hunting, armed: h.armed, homes_built: v.people.homes_built, born: v.people.born,
            toll_rate: self.toll_rate(town), passage_fee: h.passage_fee, toll_income: h.toll_before.to_f64().max(0.0) * rate,
            tolls_taken: h.tolls_taken, tolls_owed: Fixed::ZERO, guards: no(self.guards(town)),
            posts: h.posts.iter().map(|&x| HexAt::of(x)).collect(), traffic: h.traffic_per_day, deserted: h.deserted,
            militia: h.militia, trades: self.locker_trades(town),
        }
    }

    /// `Query::War`: the sovereign, its capital's treasury, the contracts that supply it, the army, and each town's part.
    pub fn war_answer(&self) -> Answer {
        let content = self.content();
        let (sov, rate, day) = (self.sovereign(), self.mint_rate(), content.day_ticks.max(1));
        let capital = self.capital();
        let army = self.army();
        let (mut contracts, mut delivered) = (0u32, 0.0);
        if let Some(cap) = capital {
            for c in self.contracts().iter().filter(|c| c.buyer == cap && c.levy.is_none() && c.training.is_none()) {
                contracts += 1;
                delivered += c.delivered;
            }
        }
        let treasury = capital.map_or(Fixed::ZERO, |c| self.town(c).money);
        let depot = capital.map(|c| {
            let stock = self.town(c).stock;
            crate::crown::stockpile(content).into_iter().map(|(g, _)| NameCount { name: self.good_name(g), count: stock[g].floor() }).collect()
        }).unwrap_or_default();
        let bills: f64 = self.bills().iter().filter(|b| b.state != crate::bills::BillState::Paid).fold(0.0, |a, b| a + b.amount.to_f64());
        let tally = self.war_tally();
        let mint = self.mint();
        Answer::War {
            phase: format!("{:?}", sov.phase).to_lowercase(), theatre: format!("{:?}", sov.theatre).to_lowercase(),
            days: sov.phase_since.map_or(0.0, |s| self.now().saturating_sub(s) as f64 / day as f64),
            capital: capital.map(no), treasury, treasury_spurs: treasury.to_f64() * rate, minted: mint.minted(self.now(), day),
            contracts, delivered, serving: no(army.size()), waiting: no(army.waiting()),
            raised: army.records.values().map(|r| r.raised).sum(), fallen: army.records.values().map(|r| r.fallen).sum(), bills,
            rate, debasement: mint.debasement(self.now(), day), levies: sov.levies, hired: sov.hired, awarded: sov.awarded, depot,
            unmet: sov.unmet.iter().map(|&(g, _)| self.good_name(g)).collect(),
            towns: tally.towns.iter().map(|t| WarTown { town: no(t.town), name: self.named(t.town), raised: t.raised,
                fallen: t.fallen, deserted: t.deserted, days: t.days, hungry: t.hungry, strength: t.strength, kit: t.kit,
                earned: t.earned, trust: capital.map_or(0.0, |c| self.trust(t.town, c)) }).collect(),
            issued: tally.issued.iter().map(|(k, &n)| NameCount { name: k.clone(), count: f64::from(n) }).collect(),
            eaten: tally.eaten, heartiness: tally.heartiness,
        }
    }

    /// `Query::RouteBooks`: every route's totals, in spurs at the mint's rate now where the mod counted spurs.
    pub fn route_books_answer(&self) -> Answer {
        use crate::cash_book::{Account, CashBook, Reason};
        let rate = self.mint_rate();
        let book = self.world.resource::<CashBook>();
        let routes = (0..self.route_count()).map(|r| {
            let (a, b) = self.route_ends(r);
            let state = self.route_state(r).1;
            let wagon = book.entries(Account::Wagon(r), 0, u64::MAX);
            let sum = |reason: Reason| wagon.iter().filter(|(_, why, _)| *why == reason).fold(0.0, |a, (_, _, v)| a + v.to_f64());
            RouteBook {
                route: no(r), from: no(a), to: no(b), from_desk: self.desk_of(a), to_desk: self.desk_of(b),
                blocks: self.route_road_blocks(r), trips: state.trips, sold_items: state.sold, sales_spurs: sum(Reason::Sale) * rate,
                service_spurs: state.service_paid.to_f64() * rate, bought_items: state.bought,
                purchase_spurs: state.purchases.to_f64() * rate, food_spurs: state.food_paid.to_f64() * rate,
                tolls_spurs: self.tolls_paid(r).to_f64() * rate, home_spurs: 0.0 - sum(Reason::Takings) * rate,
                food_eaten: state.food_eaten, short_rations: state.short_rations, delivered: state.delivered,
            }
        }).collect();
        Answer::RouteBooks { rate, routes }
    }
}
