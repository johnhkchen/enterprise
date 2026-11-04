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

    /// The grade above (a Masterwork's own).
    pub fn up(self) -> Grade {
        Grade::ALL[(self.index() + 1).min(4)]
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

    /// What a player reads of it.
    pub fn tooltip(self) -> &'static str {
        match self {
            Grade::Seconds => "Flawed: worth three quarters of a plain piece, and a town in want of better grumbles.",
            Grade::Plain => "Sound work: what a town's own workshop makes.",
            Grade::Good => "A quarter more than a plain piece, and a town likes it the better.",
            Grade::Fine => "Half as much again as a plain piece: what a town's finer tier is.",
            Grade::Masterwork => "Twice a plain piece's worth: a Master's best, with all going its way.",
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
    Tanning,
    Leatherwork,
}

impl Work {
    pub const ALL: [Work; 4] = [Work::Baking, Work::Weaving, Work::Tanning, Work::Leatherwork];

    pub fn name(self) -> &'static str {
        match self {
            Work::Baking => "baking",
            Work::Weaving => "weaving",
            Work::Tanning => "tanning",
            Work::Leatherwork => "leatherwork",
        }
    }

    /// What a player reads of it.
    pub fn tooltip(self) -> &'static str {
        match self {
            Work::Baking => "Baking: wheat into bread.",
            Work::Weaving => "Weaving: wool into carpets.",
            Work::Tanning => "Tanning: hides cured into leather, rabbit hides tanned into it.",
            Work::Leatherwork => "Leatherwork: leather cut and sewn into boots.",
        }
    }
}

/// A Master's own way of doing things, a rule it bends (Balatro's jokers: `docs/firms-plan.md`). One each, drawn from
/// its id when it reaches Master ([`knack_of`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Knack {
    /// More morale for each other Master of its business who shares a talent with it.
    HouseStyle,
    /// Materials bought of another business count a grade higher.
    SupplyWhisperer,
    /// Has the knack of a Master of the business it buys its materials of most.
    Copycat,
    /// More morale each day in a row it has made no seconds.
    Unbroken,
    /// Every third batch of its graded twice, the better kept.
    SecondProof,
    /// Twice the fundamentals; one batch in ten ruined.
    GlassHands,
    /// Its seconds go back in, to its next batch's fundamentals.
    ScrapArtist,
    /// What it makes of rabbit hide is graded a grade higher.
    RabbitsFoot,
    /// More morale for each day in a row its town has bought of its business.
    TheRegular,
    /// No quarrels in its shop: its crew sulk instead.
    Peacemaker,
    /// Its pieces sold in its home town earn its business twice the reputation.
    Patron,
}

impl Knack {
    pub const ALL: [Knack; 11] = [Knack::HouseStyle, Knack::SupplyWhisperer, Knack::Copycat, Knack::Unbroken,
        Knack::SecondProof, Knack::GlassHands, Knack::ScrapArtist, Knack::RabbitsFoot, Knack::TheRegular,
        Knack::Peacemaker, Knack::Patron];

    pub fn name(self) -> &'static str {
        match self {
            Knack::HouseStyle => "house style",
            Knack::SupplyWhisperer => "supply whisperer",
            Knack::Copycat => "copycat",
            Knack::Unbroken => "unbroken",
            Knack::SecondProof => "second proof",
            Knack::GlassHands => "glass hands",
            Knack::ScrapArtist => "scrap artist",
            Knack::RabbitsFoot => "rabbit's foot",
            Knack::TheRegular => "the regular",
            Knack::Peacemaker => "peacemaker",
            Knack::Patron => "patron",
        }
    }

    /// What a player reads of it: the rule it bends, with its numbers.
    pub fn tooltip(self) -> &'static str {
        match self {
            Knack::HouseStyle => "+0.1 morale for each other Master of its business who shares a talent with it.",
            Knack::SupplyWhisperer => "Materials bought of another business count a grade higher.",
            Knack::Copycat => "Has the knack of a Master at the business it buys its materials of most.",
            Knack::Unbroken => "+0.05 morale for each day in a row it has made no seconds, up to +0.5. Seconds start it over.",
            Knack::SecondProof => "Every third batch is graded twice, and the better grade kept.",
            Knack::GlassHands => "Twice the fundamentals. One batch in ten is ruined.",
            Knack::ScrapArtist => "Its seconds aren't sold: they're worked back in, +4 fundamentals to its next batch.",
            Knack::RabbitsFoot => "Whatever it makes of rabbit hide comes out a grade higher.",
            Knack::TheRegular => "+0.05 morale for each day in a row its town has bought of its business, up to +0.5.",
            Knack::Peacemaker => "No quarrels in its shop: whoever would quarrel sulks instead.",
            Knack::Patron => "Its pieces sold in its home town earn its business twice the reputation.",
        }
    }
}

/// The knack a hand has once it's a Master, drawn from its id.
pub fn knack_of(hand: HandId) -> Knack {
    Knack::ALL[((roll(hand as u64, 0, 31) * Knack::ALL.len() as f64) as usize).min(Knack::ALL.len() - 1)]
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

    /// What a player reads of it: what it's good for, and what it costs, with their numbers.
    pub fn tooltip(self) -> &'static str {
        match self {
            Habit::Perfectionist => "+4 fundamentals. Makes a quarter fewer batches.",
            Habit::SteadyHand => "Never makes seconds. Inspired half as often.",
            Habit::Experimenter => "Its grades spread twice as wide: more masterworks, and more seconds.",
            Habit::Tinkerer => "The stations it works will break down less (once businesses have stations).",
            Habit::Mentor => "As head, its crew learn twice as fast. Makes a quarter fewer batches itself.",
            Habit::Taskmaster => "As head, its crew make a quarter more. Their spirits fall for it.",
            Habit::Beloved => "As head, its crew's spirits rise. Soft on poor work: 0.1 less morale.",
            Habit::KinMinded => "As head, its kin among the crew are in better spirits. Takes on kin before the more skilled.",
            Habit::Showman => "Its pieces earn its business half again the reputation. Its off-form spells are talked of: each costs a twentieth of its business's name.",
            Habit::Hothead => "Inspired twice as often. Off form half again as often, and quarrels twice as often.",
            Habit::Ambitious => "Learns half again as fast. Ten days in low spirits, it leaves.",
            Habit::Homebody => "Never walks out. Won't run a branch: a House never founds one round it, nor makes it a branch's head.",
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

impl Talent {
    pub fn name(&self) -> String {
        match self {
            Talent::Work(w) => w.name().into(),
            Talent::Material(g) => g.replace('_', " "),
        }
    }

    /// What a player reads of it.
    pub fn tooltip(&self) -> String {
        match self {
            Talent::Work(w) => format!("A gift for {}: +4 fundamentals at it, learns it half again as fast, and in better spirits doing it.", w.name()),
            Talent::Material(_) => format!("Loves working {}: +2 fundamentals at any batch of it.", self.name()),
        }
    }
}

/// The words a player reads of people at work and their businesses (`docs/firms-plan.md`'s glossary), each with what it
/// means.
pub const GLOSSARY: [(&str, &str); 16] = [
    ("Business", "A household's money put into a workshop, its hands on wages."),
    ("Head", "The hand the owners put in charge of the others: head baker, head tanner."),
    ("Habit", "One of a hand's two ways. Each cuts both ways."),
    ("Talent", "A gift for a kind of work or a material: better work, faster learning, better spirits."),
    ("Skill", "What a hand has learnt of a kind of work by doing it. A Master's goes to 20."),
    ("Knack", "A Master's own way of doing things, a rule it bends. One each."),
    ("Inspiration", "A run of great work, from good spirits."),
    ("Off form", "What low spirits do: walks out, quarrels, sulks. Each one seen, with a cause that can be put right."),
    ("Fundamentals", "What a hand brings to a batch: its skill, its talents, the grade of its materials."),
    ("Morale", "How the shop is running: its head, its crew and their spirits."),
    ("Grade", "A piece's worth, Seconds to Masterwork: Fundamentals × Morale sets it."),
    ("Steps", "The works a business does in turn: a bootmaker's tanning, then its sewing."),
    ("Suppliers", "The businesses a business buys its materials of."),
    ("Reputation", "A house's fame: what its name does for it."),
    ("Draw", "The part of its reputation that brings masters seeking work."),
    ("House", "A business whose reputation has made it a name: the House of Garrick."),
];

/// A House's name, drawn from its business's id (a household's name, as the town would know it), the first in its
/// drawn order no other House has (`taken`).
pub fn house_name(id: u64, taken: &[String]) -> String {
    const NAMES: [&str; 32] = ["Garrick", "Ashby", "Thorne", "Marlow", "Hale", "Wren", "Corbin", "Fenwick", "Holt",
        "Brannock", "Pell", "Lisle", "Aldous", "Ferris", "Kettering", "Moss", "Radcliffe", "Sallow", "Tarrant", "Vane",
        "Whitlock", "Yardley", "Bramble", "Cobb", "Draycott", "Ember", "Gale", "Harrow", "Ives", "Lark", "Oakes", "Penn"];
    let start = ((roll(id, 0, 41) * NAMES.len() as f64) as usize).min(NAMES.len() - 1);
    (0..NAMES.len()).map(|i| format!("House of {}", NAMES[(start + i) % NAMES.len()]))
        .find(|n| !taken.contains(n))
        .unwrap_or_else(|| format!("House of {} {}", NAMES[start], taken.len()))
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
    /// Became a Master: its knack.
    Mastered(Knack),
    /// Ruined a batch (Glass hands).
    Ruined,
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
            Event::Mastered(k) => format!("mastered: {}", k.name()),
            Event::Ruined => "ruined a batch".into(),
        }
    }

    /// What a player reads of it.
    pub fn tooltip(&self) -> &'static str {
        match self {
            Event::Inspired => "Inspired: half again the morale at its batches for three days.",
            Event::WalkedOut => "Walked out in low spirits: it made nothing that day.",
            Event::Quarrelled(_) => "Quarrelled with another of the crew: both in lower spirits for it.",
            Event::Sulked => "Sulking: 0.3 less morale at its batches for two days.",
            Event::Left => "Left the business, its spirits long low.",
            Event::Appointed => "Appointed head by the owners: it hires and commands the crew, at half again the pay.",
            Event::Mastered(_) => "Became a Master, and found its knack.",
            Event::Ruined => "Ruined a batch: its materials lost, nothing made.",
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
    /// Its knack, a Master's.
    pub knack: Option<Knack>,
    /// The batches it has made, all told (Second proof's third, Glass hands' tenth).
    pub batches: u64,
    /// The day it last made seconds, or became a Master (Unbroken's run).
    pub seconds_on: u64,
    /// Fundamentals its seconds put back in, for its next batch (Scrap artist).
    pub scrap: f64,
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

/// What its materials' grades bring a task, by how many pieces of each grade went into it: two for each grade a piece is
/// above Plain (a Masterwork's six), two less for seconds, on average.
pub fn materials_grade(took: &[f64; 5]) -> f64 {
    let n: f64 = took.iter().sum();
    if n <= 0.0 {
        return 0.0;
    }
    Grade::ALL.iter().map(|g| took[g.index()] * 2.0 * (g.index() as f64 - 1.0)).sum::<f64>() / n
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
    /// Counts `n` of a town's pieces of good `g` at `grade` (a host's, or a test's: what its pieces are).
    pub fn set_graded(&mut self, town: crate::messages::TownId, g: usize, grade: Grade, n: f64) {
        let e = self.towns[town];
        let mut people = self.world.get_mut::<crate::state::People>(e).expect("a town");
        people.graded.entry(g).or_insert([0.0; 5])[grade.index()] = n;
    }

    /// A hand's make-up as a person, if it has been looked at.
    pub fn person(&self, hand: HandId) -> Option<Person> {
        self.world.resource::<Persons>().hands.get(&hand).cloned()
    }
}
