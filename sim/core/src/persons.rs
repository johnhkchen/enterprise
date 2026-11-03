//! People at work: what each hand is (`docs/firms-plan.md`), and how a task it does with others comes out. One engine
//! for every kind of work: a business's batch now, a squad's skirmish and a works' heat of steel later.
//!
//! - **A person's make-up:** two habits that cut both ways, its talents (for a kind of work, or a material), its skill
//!   at each work it has done, its spirits and what they bring (inspiration; off-form spells). Drawn from its id when
//!   it's first looked at ([`roll`]): the same hand is the same person whatever else happens, and no town's dice are
//!   rolled (`state::Dice`), so the draw doesn't re-roll a town's other draws.
//! - **A task:** what a person brings to it ([`fundamentals`]: its fundamentals: at a bench, in a fight) times its group's
//!   [`morale`] (its leader's habits, the group's spirits), give or take a draw of the task's own ([`score`]). The score
//!   sets a piece's [`Grade`]; against a target, it's whether a contract or a fight is won.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::roster::HandId;

/// A piece's grade, worst to best (a piece with none is Plain).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Grade {
    Seconds,
    Plain,
    Good,
    Fine,
    Masterwork,
}

impl Grade {
    pub const ALL: [Grade; 5] = [Grade::Seconds, Grade::Plain, Grade::Good, Grade::Fine, Grade::Masterwork];
    /// Those a town keeps count of (a piece not counted is Plain), best first: the order they're used in.
    pub const COUNTED: [Grade; 4] = [Grade::Masterwork, Grade::Fine, Grade::Good, Grade::Seconds];

    /// What a piece of it is worth, a plain one's being 1 (Fine: `fine_quality`, the finer tier the lockers buy).
    pub fn worth(self, d: &crate::content::Dials) -> f64 {
        match self {
            Grade::Seconds => 0.75,
            Grade::Plain => 1.0,
            Grade::Good => 1.25,
            Grade::Fine => d.fine_quality,
            Grade::Masterwork => 2.0,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Grade::Seconds => "seconds",
            Grade::Plain => "plain",
            Grade::Good => "good",
            Grade::Fine => "fine",
            Grade::Masterwork => "masterwork",
        }
    }

    /// The grade Fundamentals × Morale comes to (a Master at its skill's cap, 20, is Fine; a masterwork takes more: a
    /// talent and a habit for it, or good spirits and an inspiration).
    pub fn of(score: f64) -> Grade {
        match score {
            s if s < 6.0 => Grade::Seconds,
            s if s < 12.0 => Grade::Plain,
            s if s < 18.0 => Grade::Good,
            s if s < 30.0 => Grade::Fine,
            _ => Grade::Masterwork,
        }
    }
}

/// A kind of work a person has skill at and may have a talent for (in time a fighting art: archery, the pike).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Work {
    Baking,
    Weaving,
    Leatherwork,
}

impl Work {
    pub const ALL: [Work; 3] = [Work::Baking, Work::Weaving, Work::Leatherwork];

    pub fn name(self) -> &'static str {
        match self {
            Work::Baking => "baking",
            Work::Weaving => "weaving",
            Work::Leatherwork => "leatherwork",
        }
    }
}

/// A hand's ways, each cutting both ways (`docs/firms-plan.md`). Tinkerer and Showman wait on stations and reputation
/// (slice 3): they do nothing yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Habit {
    Perfectionist,
    SteadyHand,
    Experimenter,
    Tinkerer,
    Mentor,
    Taskmaster,
    Beloved,
    KinMinded,
    Showman,
    Hothead,
    Ambitious,
    Homebody,
}

impl Habit {
    pub const ALL: [Habit; 12] = [Habit::Perfectionist, Habit::SteadyHand, Habit::Experimenter, Habit::Tinkerer,
        Habit::Mentor, Habit::Taskmaster, Habit::Beloved, Habit::KinMinded, Habit::Showman, Habit::Hothead,
        Habit::Ambitious, Habit::Homebody];

    pub fn name(self) -> &'static str {
        match self {
            Habit::Perfectionist => "perfectionist",
            Habit::SteadyHand => "steady hand",
            Habit::Experimenter => "experimenter",
            Habit::Tinkerer => "tinkerer",
            Habit::Mentor => "mentor",
            Habit::Taskmaster => "taskmaster",
            Habit::Beloved => "beloved",
            Habit::KinMinded => "kin-minded",
            Habit::Showman => "showman",
            Habit::Hothead => "hothead",
            Habit::Ambitious => "ambitious",
            Habit::Homebody => "homebody",
        }
    }

    /// Two habits no hand has both of.
    fn clashes(self, other: Habit) -> bool {
        let pair = |a, b| (self == a && other == b) || (self == b && other == a);
        pair(Habit::Taskmaster, Habit::Beloved) || pair(Habit::SteadyHand, Habit::Experimenter)
            || pair(Habit::Ambitious, Habit::Homebody)
    }
}

/// A person's gift: for a kind of work, or for a material it works (a good, by name).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Talent {
    Work(Work),
    Material(String),
}

/// What befalls a hand of a business, and its business's record of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// Inspired: its batches lifted for days.
    Inspired,
    /// Walked out for the day: it made nothing.
    WalkedOut,
    /// Quarrelled with another of the crew: both in lower spirits.
    Quarrelled(HandId),
    /// Sulking: its batches worse for days.
    Sulked,
    /// Left the business, its spirits long low.
    Left,
    /// Appointed its head.
    Appointed,
}

impl Event {
    pub fn name(&self) -> String {
        match self {
            Event::Inspired => "inspired".into(),
            Event::WalkedOut => "walked out".into(),
            Event::Quarrelled(h) => format!("quarrelled with {h}"),
            Event::Sulked => "sulked".into(),
            Event::Left => "left".into(),
            Event::Appointed => "appointed head".into(),
        }
    }
}

/// A hand as a person at work.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub habits: Vec<Habit>,
    pub talents: Vec<Talent>,
    /// Its skill at each work it has done (0 to its rank's cap: [`skill_cap`]).
    pub skills: BTreeMap<Work, f64>,
    /// Its spirits, 0 to 1 (a half: neither up nor down).
    pub spirits: f64,
    /// Inspired, sulking or out until these ticks.
    pub inspired_until: Option<u64>,
    pub sulking_until: Option<u64>,
    pub out_until: Option<u64>,
    /// The days in a row its spirits have been low.
    pub low_days: u32,
}

impl Person {
    pub fn has(&self, h: Habit) -> bool {
        self.habits.contains(&h)
    }

    pub fn skill(&self, p: Work) -> f64 {
        self.skills.get(&p).copied().unwrap_or(0.0)
    }

    /// Does it have a talent for this work?
    pub fn gifted(&self, p: Work) -> bool {
        self.talents.contains(&Talent::Work(p))
    }

    /// Of these materials (by name), how many it has a talent for.
    pub fn materials(&self, materials: &[String]) -> usize {
        self.talents.iter().filter(|t| matches!(t, Talent::Material(g) if materials.contains(g))).count()
    }
}

/// Every hand that has been looked at as a person at work.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Persons {
    pub hands: BTreeMap<HandId, Person>,
}

/// The most skill a hand of a rank has at a work: four a level (a Master's 20).
pub fn skill_cap(level: u8) -> f64 {
    4.0 * level.clamp(1, 5) as f64
}

/// A draw in [0, 1), the same for the same `a`, `b` and salt whatever else happens (splitmix64).
pub fn roll(a: u64, b: u64, salt: u64) -> f64 {
    let mut z = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xD1B5_4A32_D192_ED03)
        ^ salt.wrapping_mul(0x8CB9_2BA7_2F3D_8DD7);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

/// A hand's make-up, drawn from its id: two habits that don't clash; a talent for a kind of work, or for one of
/// `materials` (four in ten), and a second one time in four; its skill at each work three a level (a Journeyman's
/// 9), its spirits a half.
pub fn draw(hand: HandId, level: u8, materials: &[String]) -> Person {
    let h = hand as u64;
    let pick = |salt: u64, n: usize| ((roll(h, 0, salt) * n as f64) as usize).min(n - 1);
    let first = Habit::ALL[pick(1, Habit::ALL.len())];
    let others: Vec<Habit> = Habit::ALL.into_iter().filter(|&x| x != first && !x.clashes(first)).collect();
    let habits = vec![first, others[pick(2, others.len())]];
    let talent = |salt: u64| -> Talent {
        if materials.is_empty() || roll(h, 0, salt) >= 0.4 {
            Talent::Work(Work::ALL[pick(salt + 1, Work::ALL.len())])
        } else {
            Talent::Material(materials[pick(salt + 2, materials.len())].clone())
        }
    };
    let mut talents = vec![talent(3)];
    if roll(h, 0, 6) < 0.25 {
        let second = talent(7);
        if !talents.contains(&second) {
            talents.push(second);
        }
    }
    let skills = Work::ALL.into_iter().map(|p| (p, 3.0 * level.clamp(1, 5) as f64)).collect();
    Person { habits, talents, skills, spirits: 0.5, ..Default::default() }
}

/// What a person brings to a task of a work with these materials: its skill at the work, four more with a talent for
/// it, two for each of the materials it has a talent for, four more a Perfectionist.
pub fn fundamentals(p: &Person, work: Work, materials: &[String]) -> f64 {
    p.skill(work) + if p.gifted(work) { 4.0 } else { 0.0 } + 2.0 * p.materials(materials) as f64
        + if p.has(Habit::Perfectionist) { 4.0 } else { 0.0 }
}

/// A group's morale at a task: 1, and a third of how far its people's spirits are from a half (more in good spirits,
/// less in low), less a tenth under a Beloved leader (soft on poor work).
pub fn morale(spirits: f64, leader: Option<&Person>) -> f64 {
    1.0 + (spirits - 0.5) * 0.6 - if leader.is_some_and(|l| l.has(Habit::Beloved)) { 0.1 } else { 0.0 }
}

/// A person's own lift at a task: half again inspired, less sulking.
pub fn own_form(p: &Person, now: u64) -> f64 {
    let inspired = if p.inspired_until.is_some_and(|u| now < u) { 0.5 } else { 0.0 };
    inspired - if p.sulking_until.is_some_and(|u| now < u) { 0.3 } else { 0.0 }
}

/// A task's score: what it was brought × the morale it was done in, give or take `spread` of it by its draw (in
/// [0, 1)): its grade (`Grade::of`), or against a target, whether it was done.
pub fn score(brought: f64, morale: f64, spread: f64, draw: f64) -> f64 {
    brought * morale * (1.0 + spread * (2.0 * draw - 1.0))
}

impl crate::society::Society {
    /// A hand's make-up as a person, if it has been looked at.
    pub fn person(&self, hand: HandId) -> Option<Person> {
        self.world.resource::<Persons>().hands.get(&hand).cloned()
    }
}
