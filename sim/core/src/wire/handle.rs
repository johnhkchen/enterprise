//! A request in, a response out: the core's one entry point for a host.

use std::collections::{BTreeMap, BTreeSet};

use super::{Answer, Command, Input, Point, Query, Refusal, Request, Response, SaveFile, TownPoint, ABI, SCHEMA};
use crate::content::Content;
use crate::fixed::Fixed;
use crate::points::{Kind, Spot};
use crate::society::Society;

/// Runs a request over the society a host keeps (`None` until it creates or loads one). A request built against
/// another ABI or schema is refused first; then a command needing a society when there's none; then any happening or
/// question naming what isn't there, before anything is applied. A refused `Create` or `Load` leaves the society as
/// it was.
pub fn handle(society: &mut Option<Society>, request: Request) -> Response {
    if request.abi != ABI {
        return refused(Refusal::Abi { core: ABI, request: request.abi });
    }
    if request.schema != SCHEMA {
        return refused(Refusal::Schema { core: SCHEMA, request: request.schema });
    }
    match request.command {
        Command::Create { pack, seed } => match Content::from_pack(&pack) {
            Ok(content) => {
                let mut s = Society::new(content);
                s.seed(seed);
                *society = Some(s);
                done(Vec::new(), Vec::new())
            }
            Err(why) => refused(Refusal::Pack { why }),
        },
        Command::Load { save } => {
            // A format this core can't bring forward (a newer core's, or older than its chain goes) is refused by
            // number, so a host can tell its player which; the blob's own header must say the same format.
            if !crate::save::migrate::reads(save.format) {
                return refused(Refusal::SaveFormat { core: crate::save::FORMAT, save: save.format });
            }
            match crate::save::format_of(&save.blob) {
                Ok(f) if f != save.format => {
                    return refused(Refusal::Save { why: format!("a save filed as format {} whose blob says {f}", save.format) });
                }
                Err(why) => return refused(Refusal::Save { why }),
                Ok(_) => {}
            }
            match Society::load(&save.blob) {
                Ok(s) => {
                    *society = Some(s);
                    done(Vec::new(), Vec::new())
                }
                Err(why) => refused(Refusal::Save { why }),
            }
        }
        Command::Save => match society {
            Some(s) => Response::Saved { save: SaveFile::of(s) },
            None => refused(Refusal::NoSociety),
        },
        Command::Apply { inputs } => {
            let Some(s) = society else {
                return refused(Refusal::NoSociety);
            };
            if let Err(why) = check_inputs(s, &inputs) {
                return refused(why);
            }
            let mut outputs: Vec<super::Output> = inputs.iter().flat_map(|input| s.apply(input)).collect();
            // The desks and depots counted in this look show their towns' stock: what moved since, put in or taken out
            // (`chests.rs`).
            let counted: Vec<Spot> = inputs.iter()
                .filter_map(|i| match i {
                    Input::Stocktake { point, .. } => Some(point.spot()),
                    _ => None,
                })
                .collect();
            let moved = s.show_stock(&counted);
            outputs.extend(moved.iter().map(|m| super::Output::Moved {
                point: Point::of(&m.at, m.kind),
                good: super::no(m.good),
                count: m.count,
            }));
            // And each desk counted with its money shows its town's: the treasury put in or taken out of its chests and
            // Banker, the surplus sold to the mint (`till.rs`), in the room the stock's moves left.
            let cashed: Vec<Spot> = inputs.iter()
                .filter_map(|i| match i {
                    Input::Cashcount { point, .. } => Some(point.spot()),
                    _ => None,
                })
                .collect();
            outputs.extend(s.show_cash_after(&cashed, &moved).into_iter().map(|m| super::Output::Cash {
                point: Point::of(&m.at, Kind::Desk),
                emeralds: m.emeralds,
                coins: m.coins,
                bank: m.bank,
            }));
            // The towns surveyed in this look are shown their works to lay (`land.rs`).
            let surveyed: Vec<usize> = inputs.iter()
                .filter_map(|i| match i {
                    Input::Survey { town, .. } => Some(*town as usize),
                    _ => None,
                })
                .collect();
            outputs.extend(s.show_land(&surveyed));
            // The towns a host's settlers founded and the host hasn't built yet (`settlers.rs`).
            outputs.extend(s.show_founded());
            // The towns' shops: the boards' shelves kept for the towns counted in this look, and the vendors counted told
            // what they show (`shop_host.rs`).
            let read: Vec<Spot> = inputs.iter()
                .filter_map(|i| match i {
                    Input::Vendor { point, .. } => Some(point.spot()),
                    _ => None,
                })
                .collect();
            let shopped: Vec<usize> = counted.iter().chain(read.iter())
                .filter_map(|at| s.point(at).map(|p| p.town))
                .collect();
            outputs.extend(s.show_shops(&shopped, &read));
            // The crown's gold shop, asked for again while it's wanted and none stands (`crown_host.rs`).
            outputs.extend(s.want_crown_shop(&shopped));
            done(outputs, Vec::new())
        }
        Command::Ask { queries } => {
            let Some(s) = society else {
                return refused(Refusal::NoSociety);
            };
            let mut answers = Vec::with_capacity(queries.len());
            for (at, query) in queries.iter().enumerate() {
                match answer(s, query) {
                    Ok(a) => answers.push(a),
                    Err(why) => return refused(Refusal::Query { at: at as u32, why }),
                }
            }
            done(Vec::new(), answers)
        }
    }
}

/// [`handle`] on postcard bytes: a [`Request`]'s in, a [`Response`]'s out (`Refusal::Unreadable` for bytes that aren't
/// a request). What the WebAssembly module's `ent_call` runs.
pub fn handle_bytes(society: &mut Option<Society>, bytes: &[u8]) -> Vec<u8> {
    let response = match postcard::from_bytes::<Request>(bytes) {
        Ok(request) => handle(society, request),
        Err(e) => refused(Refusal::Unreadable { why: e.to_string() }),
    };
    postcard::to_allocvec(&response).expect("a response serializes")
}

fn done(outputs: Vec<super::Output>, answers: Vec<Answer>) -> Response {
    Response::Done { outputs, answers }
}

fn refused(why: Refusal) -> Response {
    Response::Refused { why }
}

fn check_town(s: &Society, town: u32) -> Result<(), String> {
    if town as usize >= s.town_count() { Err(format!("no town {town}")) } else { Ok(()) }
}

fn answer(s: &Society, query: &Query) -> Result<Answer, String> {
    Ok(match query {
        &Query::Stock { town } => {
            if town as usize >= s.town_count() {
                return Err(format!("no town {town}"));
            }
            Answer::Stock { town, stock: s.town(town as usize).stock }
        }
        Query::Points => Answer::Points {
            points: s.points().0.iter().map(|(spot, p)| TownPoint { point: Point::of(spot, p.kind), town: super::no(p.town) })
                .collect(),
        },
        Query::Goods => Answer::Goods { goods: s.content().goods.iter().map(|g| g.name.clone()).collect() },
        Query::Dials => Answer::Dials { dials: s.dial_views() },
        &Query::Hands { town } => {
            if town as usize >= s.town_count() {
                return Err(format!("no town {town}"));
            }
            let lockers = s.roster(town as usize).locker_limit();
            Answer::Hands { town, lockers, hands: s.hand_views(town as usize) }
        }
        Query::Cash { point } => {
            let spot = point.spot();
            same_kind(s.point(&spot).map(|p| p.kind), point)?;
            let c = s.cash(&spot).ok_or_else(|| format!("{:?} at {spot:?} isn't a desk", point.kind))?;
            let (bank, bank_worth) = c.till.bank.map_or((None, Fixed::ZERO), |b| (Some(b.spurs), b.worth));
            Answer::Cash { point: point.clone(), town: super::no(c.town), treasury: c.treasury, emeralds: c.till.emeralds,
                coins: c.till.coins.spurs, coin_worth: c.till.coins.worth, bank, bank_worth, reserve: c.reserve }
        }
        Query::Mint => {
            let (mint, now, day) = (s.mint(), s.now(), s.content().day_ticks);
            Answer::Mint { rate: mint.rate(now, day), pressure: mint.pressure(now, day), minted: mint.minted(now, day),
                debased: mint.debasement(now, day) }
        }
        Query::Chests { point } => {
            let spot = point.spot();
            same_kind(s.point(&spot).map(|p| p.kind), point)?;
            let c = s.chests(&spot).expect("a point that stands");
            Answer::Chests { point: point.clone(), town: super::no(c.town), held: c.held, free: c.free, short: c.short }
        }
        &Query::Land { town } => {
            if town as usize >= s.town_count() {
                return Err(format!("no town {town}"));
            }
            s.land_view(town as usize)
        }
        Query::Ground { hexes } => Answer::Ground { ground: hexes.iter().map(|&h| s.ground_view(h)).collect() },
        &Query::Lockers { town } => {
            if town as usize >= s.town_count() {
                return Err(format!("no town {town}"));
            }
            let (limit, placed, lockers) = s.locker_views(town as usize);
            Answer::Lockers { town, limit, placed, lockers }
        }
        &Query::Routes { town } => {
            if town as usize >= s.town_count() {
                return Err(format!("no town {town}"));
            }
            s.route_lines(town as usize)
        }
        &Query::Pallets { town } => {
            if town as usize >= s.town_count() {
                return Err(format!("no town {town}"));
            }
            s.pallets_view(town as usize)
        }
        &Query::Pallet { id } => Answer::Pallet { id, pallet: s.pallets().get(id).map(|p| s.pallet_view(p)) },
        Query::Stow { point, free } => {
            if point.kind() != Kind::Desk {
                return Err(format!("pallets stowed at a {:?}", point.kind));
            }
            same_kind(s.point(&point.spot()).map(|p| p.kind), point)?;
            s.stow_view(point, *free).expect("a desk that stands")
        }
        &Query::Manifest { route } => {
            if route as usize >= s.route_count() {
                return Err(format!("no route {route}"));
            }
            s.manifest_view(route as usize)
        }
        &Query::CashBook { town, days } => {
            check_town(s, town)?;
            s.cash_book_answer(town as usize, u64::from(days))
        }
        &Query::Books { town } => {
            check_town(s, town)?;
            s.books_answer(town as usize)
        }
        &Query::Whys { town } => {
            check_town(s, town)?;
            s.whys_answer(town as usize)
        }
        Query::Trust => s.trust_answer(),
        &Query::Contracts { town } => {
            if let Some(t) = town {
                check_town(s, t)?;
            }
            s.contracts_answer(town.map(|t| t as usize))
        }
        &Query::Territory { town } => {
            check_town(s, town)?;
            s.territory_answer(town as usize)
        }
        Query::War => s.war_answer(),
        Query::RouteBooks => s.route_books_answer(),
        Query::Realms => s.realms_answer(),
        Query::Metals => s.metals_answer(),
        &Query::Shops { town } => {
            if town as usize >= s.town_count() {
                return Err(format!("no town {town}"));
            }
            s.shop_lines(town as usize)
        }
        &Query::Deeds { town } => {
            if town as usize >= s.town_count() {
                return Err(format!("no town {town}"));
            }
            s.deed_lines(town as usize)
        }
    })
}

/// The longest an `Advance` may reach past now, in days (a hundred years): a host's tick far in the future (a bug, a
/// `u64::MAX`) would otherwise keep one call catching up day by day without end.
pub const LONGEST_ADVANCE_DAYS: u64 = 36_500;

/// Checks a look's happenings in order, as they'd be applied (a town added is there for the happenings after it, a
/// point removed isn't): the first naming a town, good or point that isn't there (or a point of another kind than the
/// one standing there), a count or sum that isn't a number at or above nothing, or an advance past
/// [`LONGEST_ADVANCE_DAYS`] from now, is refused.
fn check_inputs(s: &Society, inputs: &[Input]) -> Result<(), Refusal> {
    let goods = s.content().goods.len();
    let mut towns = s.town_count();
    let mut now = s.now();
    let longest = LONGEST_ADVANCE_DAYS.saturating_mul(s.content().day_ticks);
    // The points as they'd stand, by place, with their kind: only those the happenings touch.
    let mut points: BTreeMap<Spot, Option<Kind>> = BTreeMap::new();
    // Places where a town's desk stood as they'd be (a desk there founds no town: it's that town's again), where the
    // happenings change them: a desk removed leaves one, any point placed takes it.
    let mut former: BTreeMap<Spot, bool> = BTreeMap::new();
    // The hands the happenings take on (by their tags) and lose.
    let mut tags: BTreeSet<&str> = BTreeSet::new();
    let mut lost: BTreeSet<u32> = BTreeSet::new();
    let stands = |points: &BTreeMap<Spot, Option<Kind>>, at: &Spot| {
        points.get(at).copied().unwrap_or_else(|| s.point(at).map(|p| p.kind))
    };
    for (at, input) in inputs.iter().enumerate() {
        let wrong = |why: String| Refusal::Input { at: at as u32, why };
        let town = |t: u32| if (t as usize) < towns { Ok(()) } else { Err(wrong(format!("no town {t}"))) };
        let good = |g: u32| if (g as usize) < goods { Ok(()) } else { Err(wrong(format!("no good {g}"))) };
        let amount = |what: &str, x: f64| if x.is_finite() && x >= 0.0 { Ok(()) } else { Err(wrong(format!("{what} {x}"))) };
        match input {
            Input::Advance { to } => {
                if *to > now && *to - now > longest {
                    return Err(wrong(format!("an advance to tick {to}, more than {LONGEST_ADVANCE_DAYS} days past {now}")));
                }
                now = now.max(*to);
            }
            Input::Deliver { town: t, good: g, count } | Input::Buy { town: t, good: g, count } => {
                town(*t)?;
                good(*g)?;
                amount("a count of", *count)?;
            }
            Input::Pay { town: t, money } => {
                town(*t)?;
                if money.is_negative() {
                    return Err(wrong(format!("a payment of {money}")));
                }
            }
            Input::Order { town: t, good: g, lot, price, purse } => {
                town(*t)?;
                good(*g)?;
                amount("a lot of", *lot)?;
                amount("a price of", *price)?;
                amount("a purse of", *purse)?;
            }
            Input::Stall { town: t, good: g, lot, price, stock } => {
                town(*t)?;
                good(*g)?;
                amount("a lot of", *lot)?;
                amount("a price of", *price)?;
                amount("a stock of", *stock)?;
            }
            Input::OwnShop { town: t } => town(*t)?,
            Input::AddTown { desk, .. } => {
                if desk.kind() != Kind::Desk {
                    return Err(wrong(format!("a town's desk that's a {:?}", desk.kind)));
                }
                let spot = desk.spot();
                if stands(&points, &spot).is_some() {
                    return Err(wrong(format!("a point already stands at {spot:?}")));
                }
                let again = former.get(&spot).copied().unwrap_or_else(|| s.former_desks().0.contains_key(&spot));
                if !again {
                    towns += 1;
                }
                former.insert(spot.clone(), false);
                points.insert(spot, Some(Kind::Desk));
            }
            Input::AddPoint { point, town: t } => {
                town(*t)?;
                let spot = point.spot();
                if stands(&points, &spot).is_some() {
                    return Err(wrong(format!("a point already stands at {spot:?}")));
                }
                former.insert(spot.clone(), false);
                points.insert(spot, Some(point.kind()));
            }
            Input::RemovePoint { point } => {
                let spot = point.spot();
                same_kind(stands(&points, &spot), point).map_err(wrong)?;
                if point.kind() == Kind::Desk {
                    former.insert(spot.clone(), true);
                }
                points.insert(spot, None);
            }
            Input::PalletBrought { town: t, .. } | Input::PackageBrought { town: t, .. } => town(*t)?,
            Input::SetDial { name, value } => {
                match value {
                    Some(v) => crate::dials::Dials::check(name, *v),
                    None if crate::dials::Dials::names().contains(&name.as_str()) => Ok(()),
                    None => Err(format!("no dial called {name}")),
                }
                .map_err(wrong)?;
            }
            Input::AddHand { town: t, trade, level, tag } => {
                town(*t)?;
                s.can_be_hand(trade, *level, "").map_err(wrong)?;
                if !tag.is_empty() {
                    // A name a living hand has is taken, unless that hand is lost earlier in this look.
                    if let Some(other) = s.hand_tagged(tag).filter(|id| !lost.contains(id)) {
                        return Err(wrong(format!("hand {other} is called {tag} already")));
                    }
                    if !tags.insert(tag.as_str()) {
                        return Err(wrong(format!("two hands called {tag}")));
                    }
                }
            }
            Input::HandLost { hand } => {
                s.can_lose_hand(*hand).map_err(wrong)?;
                if !lost.insert(*hand) {
                    return Err(wrong(format!("hand {hand} lost twice")));
                }
            }
            // A count of the world: lenient (`census.rs`), but of a town that's there.
            Input::Census { town: t, .. } => town(*t)?,
            Input::TagHand { tag, .. } => {
                if tag.is_empty() {
                    return Err(wrong("a hand named with no tag".into()));
                }
            }
            // A pallet that isn't there, or isn't in a warehouse, is a stale view: ignored, not refused.
            Input::PackageLost { .. } => {}
            Input::Stocktake { point, counts, .. } => {
                same_kind(stands(&points, &point.spot()), point).map_err(wrong)?;
                for c in counts {
                    good(c.good)?;
                    amount("a count of", c.count)?;
                }
            }
            // A point gone (or another kind there) by the time the host says what it couldn't move is a stale view.
            Input::Unmoved { good: g, .. } => good(*g)?,
            Input::Cashcount { point, .. } => {
                if point.kind() != Kind::Desk {
                    return Err(wrong(format!("a till counted at a {:?}", point.kind)));
                }
                same_kind(stands(&points, &point.spot()), point).map_err(wrong)?;
            }
            Input::CashUnmoved { .. } => {}
            Input::Ground { water, relief, height, .. } => {
                amount("a share under water of", *water)?;
                amount("a relief of", *relief)?;
                if let Some(h) = height.filter(|h| !h.is_finite()) {
                    return Err(wrong(format!("a ground height of {h}")));
                }
            }
            Input::Crops { counts, .. } => {
                for c in counts {
                    good(c.good)?;
                    amount("a count of", c.count)?;
                }
            }
            Input::Survey { town: t, .. } | Input::Port { town: t, .. } => town(*t)?,
            // A work the town hasn't is a stale view (ignored), but of a town that's there, a good that is, and a share.
            Input::Built { town: t, work, share } => {
                town(*t)?;
                amount("a share of", *share)?;
                match work {
                    super::Work::Field { crop } => good(*crop)?,
                    super::Work::Hall { kind } if !Society::hall_named(kind) => {
                        return Err(wrong(format!("no hall called {kind}")));
                    }
                    _ => {}
                }
            }
            // A hand gone, or an offer changed, by the time a switch applies is a stale view (`locker_host.rs`).
            Input::Switch { good: g, .. } => good(*g)?,
            Input::Route { from, to, .. } => {
                town(*from)?;
                town(*to)?;
                if from == to {
                    return Err(wrong(format!("a route from town {from} to itself")));
                }
            }
            Input::Pack { town: t, consignee, goods: lines } => {
                town(*t)?;
                town(*consignee)?;
                if t == consignee {
                    return Err(wrong(format!("goods packed at town {t} for itself")));
                }
                for c in lines {
                    good(c.good)?;
                    amount("a count of", c.count)?;
                    if c.count.fract() != 0.0 {
                        return Err(wrong(format!("{} of good {} packed: only whole items are", c.count, c.good)));
                    }
                }
            }
            Input::Stowed { point, .. } => {
                if point.kind() != Kind::Desk {
                    return Err(wrong(format!("a pallet stowed at a {:?}", point.kind)));
                }
                same_kind(stands(&points, &point.spot()), point).map_err(wrong)?;
            }
            Input::Vendor { point, read } => {
                if point.kind() != Kind::Shop {
                    return Err(wrong(format!("a vendor counted at a {:?}", point.kind)));
                }
                same_kind(stands(&points, &point.spot()), point).map_err(wrong)?;
                if let Some(g) = read.good {
                    good(g)?;
                }
            }
            Input::Board { point } => {
                if point.kind() != Kind::Shop {
                    return Err(wrong(format!("a board's vendor that's a {:?}", point.kind)));
                }
                same_kind(stands(&points, &point.spot()), point).map_err(wrong)?;
            }
            Input::DeedShop { rent, point } => {
                if *rent as usize >= s.rents().len() {
                    return Err(wrong(format!("no rent {rent}")));
                }
                if let Some(point) = point {
                    if point.kind() != Kind::Shop {
                        return Err(wrong(format!("a deed's shop that's a {:?}", point.kind)));
                    }
                    same_kind(stands(&points, &point.spot()), point).map_err(wrong)?;
                }
            }
            Input::DeedBrought { town: t, .. } => town(*t)?,
            Input::OffMap { at, realm, partners, .. } => {
                if at.kind() != Kind::Market {
                    return Err(wrong(format!("a market off the map that's a {:?}", at.kind)));
                }
                let spot = at.spot();
                if stands(&points, &spot).is_some() {
                    return Err(wrong(format!("a point already stands at {spot:?}")));
                }
                if !matches!(realm.as_str(), "" | "coast" | "march") {
                    return Err(wrong(format!("no realm called {realm}")));
                }
                for p in partners {
                    town(*p)?;
                }
                towns += 1;
                former.insert(spot.clone(), false);
                points.insert(spot, Some(Kind::Market));
            }
            Input::Capital { town: t, .. } | Input::Mobilise { town: t, .. } => town(*t)?,
            Input::Declare { theatre } => {
                if crate::crown::Theatre::named(theatre).is_none() {
                    return Err(wrong(format!("no theatre called {theatre}")));
                }
            }
            Input::Peace => {}
            Input::CrownShop { point } => {
                if point.kind() != Kind::Shop {
                    return Err(wrong(format!("the crown's shop that's a {:?}", point.kind)));
                }
                same_kind(stands(&points, &point.spot()), point).map_err(wrong)?;
            }
            Input::Locker { at, .. } => {
                if let Some(point) = at {
                    if point.kind() != Kind::Locker {
                        return Err(wrong(format!("a hand's locker at a {:?}", point.kind)));
                    }
                    same_kind(stands(&points, &point.spot()), point).map_err(wrong)?;
                }
            }
        }
    }
    Ok(())
}

/// A point named by a happening is the one standing there, of the same kind.
fn same_kind(standing: Option<Kind>, point: &Point) -> Result<(), String> {
    match standing {
        None => Err(format!("no point at {:?}", point.spot())),
        Some(kind) if kind != point.kind() => {
            Err(format!("a {:?} at {:?}, where a {kind:?} stands", point.kind, point.spot()))
        }
        Some(_) => Ok(()),
    }
}
