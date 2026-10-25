//! Wagons through the wire (`docs/binding-plan.md` 2, concern 8; task `wagon-adapter`): what a host needs of the core's
//! caravans to show them in its world. The rules are the core's own (`squad.rs`, `rules/plan.rs`, `rules/arrive.rs`,
//! `atlas.rs`, `road.rs`): this module only lets a host start and stop a town's wagons, tells it when a hand goes away
//! and comes back, and answers what its screens show.
//!
//! - **Routes started and stopped** ([`Society::host_route`], `Input::Route`): the Merchant's screen
//!   (`merchant/CommissionPayload.java` `commission`) and `/enterprise route add`/`remove`
//!   (`EnterpriseCommands.addRoute`/`removeRoute`). Started, a route opens as [`Society::open_route`] opens a commissioned
//!   one, if the far end is within a leg's food (`Caravans.inRange`, desk to desk as the command measures it) and there's
//!   a way; a route from the same town to the same market that was wound down opens again on the same food terms (the
//!   mod removes the route and adds a new one: here it keeps its number, its books and its road). Stopped, an open route
//!   is wound down
//!   ([`Society::close_route`]: its wagon on the road comes home and stays, the mod's `removeRoute`). Unlike the screen's
//!   commission ([`Society::commission`]) the far end needn't be a market the town knows: the screen shows only those,
//!   and the command never asked.
//! - **Hands away and back** ([`Society::away_outputs`], around a wire `Advance`, beside `census.rs`'s
//!   [`Society::hand_outputs`]): a hand the host named that leaves its town's roster as away (a wagon's crew or porter,
//!   a soldier: `squad::stamp`, `army.rs`) is `Output::HandAway`, its villager stowed by the host (`Squad.stow`); one away
//!   that's home again is `Output::HandBack` with its trade, level and XP now (`Squad.release` by the desk, as the mod's
//!   `walkersHome` and `arriveHome`: the trade a hand was hired into, a level earned on the road, aren't in the stowed
//!   villager and no `HandLevel` comes while it's away). A hand back is seen by the census from then (`Sightings`): its
//!   trip was no absence, and a census that comes before its villager is out isn't its ten days unseen. A hand that
//!   goes and comes back within one `Advance` (a host's long catch-up) never left the host's world.
//! - **What the screen shows** ([`Society::route_lines`], `Query::Routes`): the town's routes from it and to it, its
//!   wagons on the road and their crews by hand, the markets it knows with how far, whether a leg's food reaches them
//!   and whether it sends wagons there (`MerchantView.caravans`), and its squad at home (`Squad.available`).
//!
//! **Not here** (John, 2026-10-18: pallets live in the core only, `docs/decisions.md`): the crates a wagon carries and
//! their papers (the bills of lading, the superbills) are the core's pallets (`pallet.rs`); the mod draws its crate items
//! and blocks and its documents from the pallet outputs (`PalletPacked`, `PalletMoved`, `PalletOpened`) in task
//! `pallet-views`, with no crate or paper rules in Java.

use crate::census::{HandsNow, Sightings};
use crate::messages::{RouteId, TownId};
use crate::roster::Hands;
use crate::society::Society;
use crate::state::{Leg, Route, RouteState};
use crate::wire::{no, Answer, MarketLine, Output, Point, RouteLine, WagonLine};

impl Society {
    /// Starts (`send`) or stops wagons from `from` to `to`, as `Input::Route` (see the module's doc): the route open now
    /// from one to the other, if any. Never fails for a far end too far or with no way there (nothing opens).
    pub fn host_route(&mut self, from: TownId, to: TownId, send: bool) -> Option<RouteId> {
        let existing = self.route_from_to(from, to);
        if !send {
            self.close_route(from, to);
            return None;
        }
        if let Some(route) = existing {
            // Open already, or wound down: opened again (`retire` dropped its plan; the next look plans one, and until
            // then it's home, as a wagon just back is), if a leg's food reaches it now, as a new one must (the food
            // dial may have been lowered since it was wound down).
            if !self.route_open(route)
                && !self.blocks_between(from, to).is_some_and(|b| crate::road::in_range(self.content(), b))
            {
                return None;
            }
            if let Some(mut state) = self.world.get_mut::<RouteState>(self.routes[route]) {
                if state.freight.wound_down.take().is_some() && state.wagon.is_none() && state.stayed.is_none() {
                    state.stayed = Some(crate::road::Stay::Home);
                }
            }
            return Some(route);
        }
        let blocks = self.blocks_between(from, to)?;
        if !crate::road::in_range(self.content(), blocks) {
            return None;
        }
        self.open_route(from, to)
    }

    /// The first route from `from` to `to`, open or wound down.
    pub(crate) fn route_from_to(&self, from: TownId, to: TownId) -> Option<RouteId> {
        self.routes.iter().position(|&e| self.world.get::<Route>(e).is_some_and(|r| r.from_id == from && r.to_id == to))
    }

    /// Is the route open (not wound down)?
    fn route_open(&self, route: RouteId) -> bool {
        self.world.get::<RouteState>(self.routes[route]).is_none_or(|s| s.freight.wound_down.is_none())
    }

    /// What went away and came back since `before` (see the module's doc), in the order of the hands' ids: a hand named
    /// by the host gone away from its town (`HandAway`), a hand away home again (`HandBack`, seen by the census now).
    pub fn away_outputs(&mut self, before: &HandsNow) -> Vec<Output> {
        let after = self.hands_now();
        let now = self.now();
        let mut out = Vec::new();
        let mut back = Vec::new();
        {
            let content = self.content();
            let hands = self.world.resource::<Hands>();
            for (&id, was) in before {
                let (Some(t), Some(is)) = (was.town, after.get(&id)) else { continue };
                if is.town != Some(t) || was.away == is.away {
                    continue;
                }
                let tag = hands.tag(id);
                if is.away {
                    if let Some(tag) = tag {
                        out.push(Output::HandAway { town: no(t), hand: id, tag: tag.to_string() });
                    }
                } else {
                    out.push(Output::HandBack { town: no(t), hand: id, tag: tag.unwrap_or_default().to_string(),
                        trade: content.trades[is.trade].name.clone(), level: is.level, xp: is.xp });
                    back.push(id);
                }
            }
        }
        let mut sightings = self.world.resource_mut::<Sightings>();
        for id in back {
            sightings.seen.insert(id, now);
        }
        out
    }

    /// A town's routes, the markets it knows and its squad at home (`Query::Routes`; see the module's doc).
    pub fn route_lines(&self, town: TownId) -> Answer {
        let content = self.content();
        let (now, day) = (self.now(), content.day_ticks.max(1));
        let whys = self.world.resource::<crate::atlas::Whys>();
        let mut routes = Vec::new();
        for (id, &e) in self.routes.iter().enumerate() {
            let Some(r) = self.world.get::<Route>(e) else { continue };
            if r.from_id != town && r.to_id != town {
                continue;
            }
            let state = self.world.get::<RouteState>(e);
            let open = self.route_open(id);
            let wagon = state.and_then(|s| s.wagon.as_ref()).map(|w| WagonLine {
                out: w.leg == Leg::Out,
                arrives: w.arrives,
                crew: w.crew.iter().map(|h| h.id).collect(),
                stacks: w.slots,
                porter: w.porter,
            });
            let status = match &wagon {
                Some(w) if w.out && w.arrives > now => "on the way out".to_string(),
                Some(w) if w.out => "at the far end".to_string(),
                Some(_) => "on the way home".to_string(),
                None if !open => "wound down".to_string(),
                None => {
                    // Why no wagon left on it at its last try, if a try today sent none (`poll` 183's status): the
                    // route's own reason, not the first of the day's in the tally's order.
                    let today = now / day;
                    let tried_today = whys.tally.get(&r.from_id).into_iter().flatten()
                        .any(|((with, _, why), t)| *with == Some(r.to_id) && t.last == today
                            && matches!(why, crate::atlas::Why::NoTrip(_)));
                    match state.and_then(|s| s.stayed.map(|stay| (stay, s))) {
                        Some((stay, s)) if tried_today => format!("home: {}", crate::rules::atlas::stay_words(stay, s)),
                        _ => "home".to_string(),
                    }
                }
            };
            routes.push(RouteLine {
                route: no(id),
                from: no(r.from_id),
                to: no(r.to_id),
                blocks: r.blocks,
                days: r.travel_ticks as f64 / day as f64,
                every_days: r.every_ticks as f64 / day as f64,
                open,
                on_call: r.on_call,
                trips: state.map_or(0, |s| s.trips),
                sold: state.map_or(0.0, |s| s.sold),
                wagon,
                status,
            });
        }
        let markets = self.known(town).into_iter().map(|other| {
            let blocks = self.blocks_between(town, other);
            MarketLine {
                town: no(other),
                desk: self.desk_of(other),
                blocks,
                settlement: self.holding(other).bell.is_some(),
                reachable: blocks.is_some_and(|b| crate::road::in_range(content, b)),
                routed: self.route_from_to(town, other).is_some_and(|r| self.route_open(r)),
            }
        }).collect();
        let roster = self.roster(town);
        let lockers = self.lockers(town);
        let squad = crate::squad::available(content, &roster, lockers.as_ref()).into_iter()
            .map(|i| content.trades[roster.hands[i].trade].name.clone()).collect();
        Answer::Routes { town: no(town), routes, markets, squad }
    }

    /// How far apart two towns are in blocks (`EnterpriseCommands.addRoute`, `MerchantView.caravans`): desk to desk,
    /// rounded up, when the host placed both; else as the atlas reckons it (hex to hex, or a route's road between them).
    fn blocks_between(&self, a: TownId, b: TownId) -> Option<u32> {
        let at = |t: TownId| self.world.get::<crate::state::Town>(self.towns[t]).and_then(|t| t.at);
        match (at(a), at(b)) {
            (Some((ax, az)), Some((bx, bz))) => Some(((ax - bx).powi(2) + (az - bz).powi(2)).sqrt().ceil() as u32),
            _ => self.places().distance(a, b),
        }
    }

    /// The desk a town's host placed for it (the first desk point of the town's; a market off the map's point if it has no
    /// desk), if any.
    pub(crate) fn desk_of(&self, town: TownId) -> Option<Point> {
        self.points().0.iter().find(|(_, p)| p.town == town && p.kind == crate::points::Kind::Desk)
            .or_else(|| self.points().0.iter().find(|(_, p)| p.town == town && p.kind == crate::points::Kind::Market))
            .map(|(spot, p)| Point::of(spot, p.kind))
    }
}
