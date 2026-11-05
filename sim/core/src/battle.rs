//! A battle between factions for a hex (John: "seed a factional battle between horde and alliance, requiring them to
//! hold a hex 30 days before settling is allowed"; "One hex, seeded", "Faction armies, army vs army", "30 days held,
//! holder only"). Factions are peoples grouped (`docs/decisions.md`: "Factions are races"): the Horde the orcs,
//! tauren and goblins; the Alliance the kingdoms of men (Stormwind, Lordaeron, Dalaran), the dwarves and the night
//! elves ([`Faction::of`]).
//!
//! - **Seeded** (`battle_day`, a dial: none at 0): that day the hex between the nearest Horde town and the nearest
//!   Alliance town, the middle of the line between them or the nearest of it that's dry and no town's, is contested.
//! - **Each side's army** (an [`Army`], as the crown's: its soldiers eat and draw their kit at its camp, its town
//!   nearest the hex, and run its risks): each of its faction's towns within `battle_reach` hexes levies
//!   `battle_levy` of its residents (a levy's order: idle hands first), who march to the hex, the road's time at a
//!   wagon's pace; every `battle_reinforce_days` its towns make up their levies.
//! - **They meet** every `skirmish_days` (`war::clash`: each side's score the other's target, one twist for both); the
//!   winner holds the hex, as does a side whose enemy has no one there. The fallen are their towns' grief; the routed
//!   and deserters walk home.
//! - **Held `battle_hold_days` in a row,** the hex is its holder's: the fighting ends, both armies walk home, and only
//!   the holder's settlers may found a town on it ([`Battles::may_settle`]); none may before.

use serde::{Deserialize, Serialize};

use bevy_ecs::prelude::Resource;

use crate::army::{Army, Recruit};
use crate::hex::Hex;
use crate::messages::TownId;

/// A side in the war of peoples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Faction {
    Horde,
    Alliance,
}

impl Faction {
    /// The faction of a people (a town's kingdom, `Input::People`), if it has one.
    pub fn of(people: &str) -> Option<Faction> {
        match people.rsplit(':').next().unwrap_or(people).to_ascii_lowercase().replace(['_', ' '], "").as_str() {
            "orc" | "tauren" | "goblin" | "troll" | "undead" | "forsaken" | "bloodelf" => Some(Faction::Horde),
            "stormwind" | "lordaeron" | "dalaran" | "human" | "dwarf" | "nightelf" | "gnome" | "draenei" => Some(Faction::Alliance),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Faction::Horde => "the Horde",
            Faction::Alliance => "the Alliance",
        }
    }

    /// Its capital off the map, whose war chest backs it (John: "both sides will get a backing capital (Orgrimmar,
    /// Theramore) with its own mercenary and war chest").
    pub fn capital(self) -> &'static str {
        match self {
            Faction::Horde => "Orgrimmar",
            Faction::Alliance => "Theramore",
        }
    }

    /// Its war chest, in emeralds (a dial).
    pub fn chest(self, d: &crate::Dials) -> f64 {
        match self {
            Faction::Horde => d.war_chest_horde,
            Faction::Alliance => d.war_chest_alliance,
        }
    }

    pub const BOTH: [Faction; 2] = [Faction::Horde, Faction::Alliance];
}

/// A recruit on its way to the battle: there, it's a soldier of its side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Marching {
    pub side: usize,
    pub recruit: Recruit,
    pub arrives: u64,
}

/// A battle for a hex: the hex, the factions and their armies, each side's camp (its town nearest the hex: its
/// warehouse feeds and kits its army) and levying towns with each one's levy, those marching to it, who holds it and
/// since when, whose it became (the battle over), when it began, its days fought and its fights.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Battle {
    pub hex: Hex,
    pub factions: [Faction; 2],
    pub armies: [Army; 2],
    pub camps: [TownId; 2],
    pub levies: [Vec<(TownId, usize)>; 2],
    pub marching: Vec<Marching>,
    pub holder: Option<usize>,
    pub held_since: Option<u64>,
    pub won_by: Option<Faction>,
    pub began: u64,
    pub days: u64,
    pub fights: u64,
    /// The day it was last fought (once a day).
    #[serde(default)]
    pub reckoned: Option<u64>,
    /// When its levies march (`battle_mobilise_days` after it's seeded: till then its towns arm), and have they.
    #[serde(default)]
    pub marches: u64,
    #[serde(default)]
    pub marched: bool,
    /// The town it's fought for, if it's a town's hex, and the side attacking it (`docs/war-plan.md`): held
    /// `raze_hold_days` by the attackers, the town is razed; held `battle_hold_days` by its defenders, the attack is
    /// beaten off. And whether what came of it has been seen to.
    #[serde(default)]
    pub town: Option<TownId>,
    #[serde(default)]
    pub attacker: Option<usize>,
    #[serde(default)]
    pub settled: bool,
    /// A battle where squads on the march met (`Squad`): over when one side has no one left on its hex.
    #[serde(default)]
    pub field: bool,
    /// A raid on a town away from the enemy's main force (`rules::battle::council`), fought beside the campaign's.
    #[serde(default)]
    pub raid: bool,
    /// The side that held the hex's ground when it began (a field battle's: the side waiting there).
    #[serde(default)]
    pub ground_side: Option<usize>,
}

impl Battle {
    /// The days its holder has held it, today.
    pub fn held_days(&self, now: u64, day: u64) -> u64 {
        self.held_since.map_or(0, |s| now.saturating_sub(s) / day.max(1))
    }
}

/// The war for the region (`docs/war-plan.md`): declared and when Theramore lands, its base (the Alliance's port
/// nearest the Horde), the capitals' soldiers in the field (Theramore's marines, Orgrimmar's mercenaries), when
/// Theramore withdrew, the towns razed (by whom), the refugees taken in and those with nowhere to go, and whose it was.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Campaign {
    pub declared: u64,
    pub lands: u64,
    pub base: Option<TownId>,
    pub landed: bool,
    pub marines: u32,
    pub mercenaries: u32,
    pub hired: Option<u64>,
    pub withdrew: Option<u64>,
    pub razed: Vec<(u64, TownId, Faction)>,
    pub refugees: u32,
    pub lost: u32,
    pub won_by: Option<(u64, Faction)>,
    /// The day it was last reckoned (once a day).
    #[serde(default)]
    pub reckoned: Option<u64>,
    /// When Theramore's last ships came with marines (each `convoy_days` at most).
    #[serde(default)]
    pub sailed: Option<u64>,
    /// The day each side's heroes came (Orgrimmar's, Theramore's), how many still lead, and the day one last fell.
    #[serde(default)]
    pub heroes: [Option<u64>; 2],
    #[serde(default)]
    pub heroes_alive: [u32; 2],
    #[serde(default)]
    pub heroes_fell: [Option<u64>; 2],
}

/// A squad of the war for the region (`docs/war-plan.md`, squads on the hexes; John: "hex-based squad movement"): a
/// town's levy marching from home, or a capital's soldiers (Theramore's marines, Orgrimmar's mercenaries; their home
/// their camp). Its soldiers are its own while it marches; in a battle they're its side's army's, each marked with
/// the squad's id, and `battle` is that battle. It goes `squad_hexes_per_day` a day toward its goal, and home once
/// it's done.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Squad {
    pub id: u32,
    pub side: usize,
    pub home: TownId,
    pub capital: bool,
    pub soldiers: Vec<crate::army::Soldier>,
    pub hex: Hex,
    pub goal: Option<Hex>,
    pub homeward: bool,
    pub battle: Option<usize>,
    /// The town battle it was raised for, and whether it waits at its goal (a post its council gave it: a chokepoint,
    /// the road to the enemy's camp, a hex round its battle).
    #[serde(default)]
    pub bound: Option<usize>,
    #[serde(default)]
    pub waits: bool,
}

impl Squad {
    /// Its soldiers, marching or in its battle's army.
    pub fn size(&self, battles: &[Battle]) -> usize {
        self.soldiers.len() + self.battle.and_then(|i| battles.get(i))
            .map_or(0, |b| b.armies[self.side].soldiers.iter().filter(|s| s.squad == Some(self.id)).count())
    }
}

/// The battles for hexes, and whether `battle_day`'s was seeded.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Battles {
    pub battles: Vec<Battle>,
    pub seeded: bool,
    /// The day the barracks towns last drilled (`works::Kind::Barracks`).
    #[serde(default)]
    pub drilled: Option<u64>,
    /// The day its towns last procured their levies' kit (`rules/battle.rs` `procure`).
    #[serde(default)]
    pub procured: Option<u64>,
    /// What each faction's capital has spent of its war chest (`Faction::chest`), and the day it ran dry.
    #[serde(default)]
    pub spent: std::collections::BTreeMap<Faction, crate::fixed::Fixed>,
    #[serde(default)]
    pub dry: std::collections::BTreeMap<Faction, u64>,
    /// The war for the region, once declared, and the towns razed: ruins, no one's.
    #[serde(default)]
    pub campaign: Option<Campaign>,
    #[serde(default)]
    pub ruins: std::collections::BTreeSet<TownId>,
    /// The contracts and tenders its capital has paid for (its towns' procurement: `rules/battle.rs` `procure`), by id:
    /// each once, its worth when let.
    #[serde(default)]
    pub funded_contracts: std::collections::BTreeSet<usize>,
    #[serde(default)]
    pub funded_tenders: std::collections::BTreeSet<usize>,
    /// What each capital owes its towns for what they procured and haven't been paid yet (town, faction, owed), paid
    /// at its daily pace ([`Battles::spend_public`]); and what each paid of its pace today (day, paid).
    #[serde(default)]
    pub owed: Vec<(TownId, Faction, crate::fixed::Fixed)>,
    #[serde(default)]
    pub paid_today: std::collections::BTreeMap<Faction, (u64, crate::fixed::Fixed)>,
    /// The war's squads on the hexes, and the next one's id (`docs/war-plan.md`, squads on the hexes).
    #[serde(default)]
    pub squads: Vec<Squad>,
    #[serde(default)]
    pub next_squad: u32,
    #[serde(default)]
    pub squads_moved: Option<u64>,
}

impl Battles {
    /// What a faction's capital will pay of `want` out of its war chest, what's left of it at most, counted spent; the
    /// day it's spent, it's dry.
    pub fn draw(&mut self, d: &crate::Dials, faction: Faction, want: crate::fixed::Fixed, day: u64) -> crate::fixed::Fixed {
        use crate::fixed::Fixed;
        let spent = self.spent.entry(faction).or_default();
        let left = (Fixed::floor(faction.chest(d)) - *spent).at_least_zero();
        let paid = want.at_least_zero().min(left);
        *spent += paid;
        if paid < want && !self.dry.contains_key(&faction) {
            self.dry.insert(faction, day);
        }
        paid
    }

    /// What's left of a faction's war chest.
    pub fn chest_left(&self, d: &crate::Dials, faction: Faction) -> f64 {
        (faction.chest(d) - self.spent.get(&faction).map_or(0.0, |s| s.to_f64())).max(0.0)
    }

    /// A capital's daily pace of public spending (its towns' barracks and procurement): what its chest holds past its
    /// reserve (`chest_reserve`, kept for its soldiers in the field), over `chest_pace_days`.
    pub fn pace(&self, d: &crate::Dials, faction: Faction) -> f64 {
        faction.chest(d) * (1.0 - d.chest_reserve.clamp(0.0, 1.0)) / d.chest_pace_days.max(1.0)
    }

    /// What a capital may still spend on public works today: what's left of its pace, its chest past its reserve at
    /// most.
    pub fn public_left(&self, d: &crate::Dials, faction: Faction, today: u64) -> crate::fixed::Fixed {
        use crate::fixed::Fixed;
        let paid = self.paid_today.get(&faction).filter(|p| p.0 == today).map_or(Fixed::ZERO, |p| p.1);
        let pace = (Fixed::floor(self.pace(d, faction)) - paid).at_least_zero();
        let above = self.chest_left(d, faction) - faction.chest(d) * d.chest_reserve.clamp(0.0, 1.0);
        pace.min(Fixed::floor(above.max(0.0)))
    }

    /// What a capital pays of `want` on public works today, [`Battles::public_left`] at most, counted spent.
    pub fn spend_public(&mut self, d: &crate::Dials, faction: Faction, want: crate::fixed::Fixed, today: u64)
            -> crate::fixed::Fixed {
        let paid = want.at_least_zero().min(self.public_left(d, faction, today));
        let paid = self.draw(d, faction, paid, today);
        let p = self.paid_today.entry(faction).or_insert((today, crate::fixed::Fixed::ZERO));
        if p.0 != today {
            *p = (today, crate::fixed::Fixed::ZERO);
        }
        p.1 += paid;
        paid
    }

    /// What a capital owes its towns, all told.
    pub fn backlog(&self, faction: Faction) -> f64 {
        self.owed.iter().filter(|o| o.1 == faction).map(|o| o.2.to_f64()).sum()
    }

    /// May a town of this faction found a town on `hex`? Not on a hex fought for until a side has held it
    /// `battle_hold_days`; then only the side that did.
    pub fn may_settle(&self, hex: Hex, faction: Option<Faction>) -> bool {
        // (A town's hex beaten off is still its town's; razed, it's the razers' to settle.)
        self.battles.iter().filter(|b| b.hex == hex && !(b.town.is_some() && b.won_by.is_some() && b.attacker
            .is_some_and(|a| b.won_by != Some(b.factions[a])))).all(|b| b.won_by.is_some() && b.won_by == faction)
    }
}

impl crate::society::Society {
    pub fn battles(&self) -> &Battles {
        self.world.resource::<Battles>()
    }
}
