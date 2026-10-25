//! The host's census as a town's roster's source, and its villagers as views of the core's hands
//! (`docs/binding-plan.md` 2, concern 5).
//!
//! The mod counts a settlement around its bell (`settlement/CensusTaker.java`): its beds and the villagers living
//! there, kept in a roster of residents by UUID (`Settlements.Resident`). Residents not seen still count (out with a
//! caravan, in a chunk that isn't loaded, wandered off): only a death, or `FORGET_AFTER_TICKS` (10 days) of being
//! looked for and not found, takes one off (`CensusTaker.finish`). Here the host sends what it saw
//! (`Input::Census`), and the town's [`Roster`] follows it ([`Society::census`]):
//!
//! - **The beds** are the world's: `People.beds` is the census's count (a child needs a free bed, `people.rs`).
//! - **A villager the core doesn't know** joins as a hand under the host's tag ([`Society::join`]; a child as a
//!   child, grown `BABY_TICKS` later). One of a trade the core hasn't, or with no tag, isn't counted.
//! - **A villager the core knows** at home here is seen: a profession it took in the world is its hand's trade now
//!   (never `none`: a villager that lost its workstation, or a view not yet written, keeps the core's trade; never the
//!   desk's Merchant's), and a level it earned there is its hand's (never less than the core's). One whose hand is
//!   away (with a caravan, at war), another town's, or with a party of settlers is left as the core has it.
//! - **A villager whose hand is gone** (the rules took it from the world, or it left with settlers) is a view that
//!   should no longer stand: [`Output::HandGone`] again, for the host to take it away.
//! - **A hand at home that no census has seen** for [`FORGET_AFTER_DAYS`] has left: off the roster, as a death is
//!   ([`Society::lose_hand`]). A hand's clock starts at the first census of its town after it joins; a hand with no
//!   tag (no villager stands for it yet) is never forgotten.
//!
//! **Views of hands** ([`Society::hand_outputs`]): what the rules do to hands during an `Advance` comes out as outputs
//! for the host to show: a child born ([`Output::Born`]), a hand come to live at a town from a party of settlers
//! ([`Output::Settled`]), each a hand with no villager that the host spawns and names (`Input::TagHand`); a hand gone
//! with settlers ([`Output::Departed`]: its villager leaves the world and the core forgets its tag, so it settles
//! under a new one); a hand the rules took from the world ([`Output::HandGone`]); a hand's trade, level or XP changed
//! ([`Output::HandLevel`], written to its villager). A hand going away or coming home with a caravan or a war is the
//! wagons' (`wagon-adapter`) and comes out as none of these.
//!
//! **Not as the mod:** no path checks (the core has no free and confined villagers: `Census.value` is the mod's culture,
//! the land's slice), and a census is the host's whenever it can see the whole settlement, not once a day.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::messages::TownId;
use crate::roster::{xp_floor, Hand, HandId, Hands, Roster, TradeId, MAX_LEVEL};
use crate::society::Society;
use crate::state::People;
use crate::wire::{no, Input, Output, Resident};

/// A hand at home that no census has seen for this many days has left (`CensusTaker.FORGET_AFTER_TICKS`, 10 days).
pub const FORGET_AFTER_DAYS: u64 = 10;

/// When each hand was last seen by a census of its town (by id), and the tags of villagers that should no longer
/// stand (a hand gone with settlers: its old villager's tag, with the hand's id).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sightings {
    pub seen: BTreeMap<HandId, u64>,
    pub retired: BTreeMap<String, HandId>,
}

/// Where a hand is and what it is, for the outputs of an `Advance` ([`Society::hand_outputs`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Was {
    /// Its town, or none while it's with a party of settlers.
    pub town: Option<TownId>,
    pub trade: TradeId,
    pub level: u8,
    pub xp: u32,
    pub away: bool,
    pub baby: bool,
}

/// Every hand the society holds with an id, by id: on a roster or with a party on the road.
pub type HandsNow = BTreeMap<HandId, Was>;

/// What a census says of a villager, resolved against the content: its trade's id, whether that's no trade at all or
/// the Merchant's, its level within 1 to 5.
struct Seen<'a> {
    tag: &'a str,
    trade: Option<TradeId>,
    none: bool,
    merchant: bool,
    level: u8,
    baby: bool,
}

impl Society {
    /// A town's census (`Input::Census`): its beds, and the villagers the host saw living there (see the module's
    /// doc). Returns the outputs it makes: a [`Output::HandGone`] for each villager seen whose hand is no more.
    pub fn census(&mut self, town: TownId, beds: u32, residents: &[Resident]) -> Vec<Output> {
        let now = self.now();
        let forget = FORGET_AFTER_DAYS * self.content().day_ticks;
        let e = self.towns[town];
        // The beds it found, and those of the homes it built that the host hasn't laid yet (`land.rs`).
        let unlaid = self.world.resource::<crate::land::LandBook>().unlaid_beds(town);
        self.world.get_mut::<People>(e).expect("a town").beds = beds as f64 + unlaid;
        let seen: Vec<Seen> = {
            let content = self.content();
            let mut tags = BTreeSet::new();
            residents.iter().filter(|r| !r.tag.is_empty() && tags.insert(r.tag.as_str())).map(|r| {
                let trade = content.trade(&r.trade);
                let t = trade.and_then(|t| content.trades.get(t));
                Seen { tag: &r.tag, trade, none: t.is_some_and(|t| t.none), merchant: t.is_some_and(|t| t.merchant),
                    level: r.level.clamp(1, MAX_LEVEL), baby: r.baby }
            }).collect()
        };
        // Every hand held, where it is; and the host's tags, by tag (a living hand's before one the rules took from
        // the world, whose tag is kept for this), and those retired with settlers.
        let held = self.hands_now();
        let tagged: BTreeMap<String, HandId> = {
            let mut by_tag = BTreeMap::new();
            for (id, tag) in &self.world.resource::<Hands>().tags {
                let entry = by_tag.entry(tag.clone()).or_insert(*id);
                if !held.contains_key(entry) && held.contains_key(id) {
                    *entry = *id;
                }
            }
            by_tag
        };
        let retired = self.world.resource::<Sightings>().retired.clone();
        let mut out = Vec::new();
        let mut here: BTreeSet<HandId> = BTreeSet::new();
        for r in &seen {
            if let Some(&id) = tagged.get(r.tag) {
                match held.get(&id) {
                    // Away, or with settlers: off the host's world, the core's to bring back.
                    Some(was) if was.away => {}
                    Some(was) if was.town == Some(town) => {
                        here.insert(id);
                        let (_, i) = self.find_hand(id).expect("a hand held at a town");
                        self.adopt(town, i, r);
                    }
                    // Another town's, seen here: it's still about, and still that town's.
                    Some(_) => {
                        self.world.resource_mut::<Sightings>().seen.insert(id, now);
                    }
                    None => out.push(Output::HandGone { hand: id, tag: r.tag.to_string() }),
                }
            } else if let Some(&id) = retired.get(r.tag) {
                // The villager of a hand gone with settlers: it settles elsewhere under a new villager.
                out.push(Output::HandGone { hand: id, tag: r.tag.to_string() });
            } else if let Some(id) = self.join_seen(town, r) {
                here.insert(id);
            }
        }
        // The unseen: a hand at home with a tag that no census has seen for long enough has left.
        let unseen: Vec<HandId> = {
            let hands = self.world.resource::<Hands>();
            self.world.get::<Roster>(e).map_or_else(Vec::new, |ro| {
                ro.hands.iter().filter(|h| !h.away && h.id != 0 && !here.contains(&h.id) && hands.tag(h.id).is_some())
                    .map(|h| h.id).collect()
            })
        };
        let mut left = Vec::new();
        {
            let mut sightings = self.world.resource_mut::<Sightings>();
            for &id in &here {
                sightings.seen.insert(id, now);
            }
            for id in unseen {
                let last = *sightings.seen.entry(id).or_insert(now);
                if now.saturating_sub(last) > forget {
                    left.push(id);
                }
            }
        }
        for id in left {
            let _ = self.lose_hand(id);
        }
        self.prune_sightings();
        out
    }

    /// The hand at `i` in `town`'s roster as a census saw its villager: a profession taken in the world, a level
    /// earned there (see the module's doc).
    fn adopt(&mut self, town: TownId, i: usize, r: &Seen) {
        let merchant_trade = self.content().trade(crate::roster::MERCHANT);
        let e = self.towns[town];
        let mut roster = self.world.get_mut::<Roster>(e).expect("the hand's roster");
        let desk = roster.merchant == Some(i);
        let h = &mut roster.hands[i];
        if let Some(t) = r.trade.filter(|&t| t != h.trade && !r.none && !desk && !h.is_baby()) {
            h.trade = t;
            if r.merchant && roster.merchant.is_none() {
                roster.merchant = Some(i);
            }
        }
        let h = &mut roster.hands[i];
        if r.level > h.level && !h.is_baby() {
            h.level = r.level;
            h.xp = h.xp.max(xp_floor(merchant_trade == Some(h.trade), r.level));
        }
    }

    /// A villager the core didn't know, seen living at `town`: a hand under its tag (a child as a child). None for one
    /// of a trade the core hasn't.
    fn join_seen(&mut self, town: TownId, r: &Seen) -> Option<HandId> {
        let trade = r.trade?;
        if !r.baby {
            let name = self.content().trades[trade].name.clone();
            return self.join(town, &name, r.level, r.tag).ok();
        }
        let now = self.now();
        let e = self.towns[town];
        let mut roster = self.world.get::<Roster>(e).cloned().unwrap_or_default();
        let id = self.world.resource_mut::<Hands>().next();
        roster.hands.push(Hand { id, ..Hand::baby(self.content(), now) });
        self.world.get_mut::<People>(e).expect("a town").villagers = roster.residents() as f64;
        self.world.entity_mut(e).insert(roster);
        self.world.resource_mut::<Hands>().tags.insert(id, r.tag.to_string());
        Some(id)
    }

    /// A villager now stands for a hand that had none (`Input::TagHand`): the host's tag for it. False (and nothing
    /// done) when the hand isn't at home on a roster, or another living hand has the tag.
    pub fn tag_hand(&mut self, id: HandId, tag: &str) -> bool {
        if tag.is_empty() || self.can_lose_hand(id).is_err() || self.hand_tagged(tag).is_some_and(|other| other != id) {
            return false;
        }
        let now = self.now();
        self.world.resource_mut::<Hands>().tags.insert(id, tag.to_string());
        let mut sightings = self.world.resource_mut::<Sightings>();
        sightings.seen.insert(id, now);
        sightings.retired.remove(tag);
        true
    }

    /// Forgets when hands no longer held were seen.
    fn prune_sightings(&mut self) {
        let living: BTreeSet<HandId> = self.hands_now().into_keys().collect();
        self.world.resource_mut::<Sightings>().seen.retain(|id, _| living.contains(id));
    }

    /// Every hand the society holds with an id, where it is and what it is: before an `Advance`, to tell after it
    /// what the rules did ([`Society::hand_outputs`]).
    pub fn hands_now(&self) -> HandsNow {
        let mut now = HandsNow::new();
        for (t, &e) in self.towns.iter().enumerate() {
            if let Some(roster) = self.world.get::<Roster>(e) {
                for h in roster.hands.iter().filter(|h| h.id != 0) {
                    now.insert(h.id, Was { town: Some(t), trade: h.trade, level: h.level, xp: h.xp, away: h.away,
                        baby: h.is_baby() });
                }
            }
        }
        for p in &self.world.resource::<crate::settlers::Settlers>().parties {
            for h in p.hands.iter().filter(|h| h.id != 0) {
                now.insert(h.id, Was { town: None, trade: h.trade, level: h.level, xp: h.xp, away: true, baby: h.is_baby() });
            }
        }
        now
    }

    /// What the rules did to hands since `before` (see the module's doc), in the order of the hands' ids: a child born,
    /// a hand settled, departed or gone, a hand's trade, level or XP changed. A hand gone with settlers has its tag
    /// forgotten (its villager leaves the world; it settles under a new one).
    pub fn hand_outputs(&mut self, before: &HandsNow) -> Vec<Output> {
        let after = self.hands_now();
        let mut out = Vec::new();
        let mut retire = Vec::new();
        {
            let content = self.content();
            let hands = self.world.resource::<Hands>();
            let tag = |id: HandId| hands.tag(id).unwrap_or_default().to_string();
            for (&id, was) in before {
                match (was.town, after.get(&id)) {
                    (Some(t), Some(now)) if now.town == Some(t) => {
                        let changed = (now.trade, now.level, now.xp) != (was.trade, was.level, was.xp);
                        if changed && !now.away && !now.baby && hands.tag(id).is_some() {
                            out.push(Output::HandLevel { hand: id, tag: tag(id), trade: content.trades[now.trade].name.clone(),
                                level: now.level, xp: now.xp });
                        }
                    }
                    (Some(t), Some(now)) => {
                        if hands.tag(id).is_some() {
                            out.push(Output::Departed { town: no(t), hand: id, tag: tag(id) });
                            retire.push(id);
                        }
                        if let Some(to) = now.town {
                            out.push(Output::Settled { town: no(to), hand: id });
                        }
                    }
                    (Some(_), None) => {
                        if hands.tag(id).is_some() {
                            out.push(Output::HandGone { hand: id, tag: tag(id) });
                        }
                    }
                    (None, Some(now)) => {
                        if let Some(to) = now.town {
                            out.push(Output::Settled { town: no(to), hand: id });
                        }
                    }
                    (None, None) => {}
                }
            }
            for (&id, now) in after.iter().filter(|(id, _)| !before.contains_key(id)) {
                if let Some(to) = now.town {
                    out.push(if now.baby { Output::Born { town: no(to), hand: id } } else { Output::Settled { town: no(to), hand: id } });
                }
            }
        }
        for id in retire {
            if let Some(tag) = self.world.resource_mut::<Hands>().tags.remove(&id) {
                let mut sightings = self.world.resource_mut::<Sightings>();
                sightings.retired.insert(tag, id);
                sightings.seen.remove(&id);
            }
        }
        out
    }

    /// Applies a census's happening (`Input::Census`, `TagHand`): its outputs, or none for any other happening.
    pub(crate) fn apply_census(&mut self, input: &Input) -> Option<Vec<Output>> {
        match input {
            Input::Census { town, beds, residents } => Some(self.census(*town as TownId, *beds, residents)),
            Input::TagHand { hand, tag } => {
                // A hand that isn't at home by now, or a tag another has, is a stale view: ignored.
                let _ = self.tag_hand(*hand, tag);
                Some(Vec::new())
            }
            _ => None,
        }
    }
}
