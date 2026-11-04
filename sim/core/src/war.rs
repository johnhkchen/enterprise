//! War on the engine of people at work (`docs/firms-plan.md` slice 5; John: "a skirmish becomes a crafting contract
//! with similar mechanics like a boss blind"; "Generalize"). The army's soldiers are people as a business's hands are
//! (`persons.rs`): habits, talents, a skill at arms or archery grown by fighting, spirits. Every `skirmish_days` of the
//! campaign the army fights a skirmish, a task against a target with a twist (Balatro's boss blinds):
//!
//! - **Fundamentals** a soldier brings ([`fundamentals`]): its skill at its weapon's work, four more with a talent for
//!   it, four a Perfectionist, its weapon's power past a wooden sword's (unarmed, four less), a fifth of its armour's
//!   points, and its kit's grades (two a grade above Plain, seconds two less, on average: `persons::materials_grade`).
//!   Kit's grade is where the economy meets the war: a bladesmith's Fine swords are a squad's fundamentals.
//! - **Morale:** its soldiers' spirits and its captain's habits (`persons::morale`), less for the hungry. Its captain
//!   is the most skilled soldier, by the crown's mandate (`charters.rs`: to command).
//! - **The target:** `skirmish_target` a soldier, a tenth more for each skirmish won in a row (the next blind is
//!   harder), with a twist drawn for it ([`Twist`]): fog (bows a half), a river (a quarter less all round), night (no
//!   inspiration), or open ground.
//! - **The score:** each soldier's Fundamentals × Morale, give or take its draw (`persons::score`), summed; the knacks
//!   that bend a task bend a fight (Second proof, Glass hands, House style, Unbroken; a Peacemaker serving, no rout).
//! - **Won:** the army's risk is half until the next (`Army::edge`), its spirits rise, and its best soldier makes a
//!   heroic stand (inspired, not at night). **Lost:** its risk half again, its spirits fall; its morale broken (under
//!   `rout_morale`), its soldiers in low spirits rout (never a Steady hand): they walk home, as deserters do.
//! - Each fight teaches: its soldiers more skilled at their weapons' work (half again with a talent or Ambitious, twice
//!   under a Mentor captain).

use serde::{Deserialize, Serialize};

use crate::army::{armour, bare, points, weapon, Army, Soldier, Worn, BOW};
use crate::charters::{Charters, Mandate, Party, Source, Verb};
use crate::content::Content;
use crate::firms::Head;
use crate::persons::{draw, materials_grade, roll, skill_cap, Grade, Habit, Knack, Person, Persons, Work};
use crate::roster::HandId;

/// A skirmish's twist (a boss blind's): what the ground or the hour does to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Twist {
    /// Open ground: nothing.
    Open,
    /// Fog: a bow's fundamentals a half.
    Fog,
    /// A river to cross: the army's score a quarter less.
    River,
    /// Night: no inspiration, and a tenth less morale.
    Night,
}

impl Twist {
    pub const ALL: [Twist; 4] = [Twist::Open, Twist::Fog, Twist::River, Twist::Night];

    pub fn name(self) -> &'static str {
        match self {
            Twist::Open => "open ground",
            Twist::Fog => "fog",
            Twist::River => "a river",
            Twist::Night => "night",
        }
    }

    /// What a player reads of it.
    pub fn tooltip(self) -> &'static str {
        match self {
            Twist::Open => "Open ground: nothing in the way.",
            Twist::Fog => "Fog: archers bring half their fundamentals.",
            Twist::River => "A river to cross: the army's score a quarter less.",
            Twist::Night => "Night: no one inspired, and a tenth less morale.",
        }
    }
}

/// A skirmish fought: the day, the soldiers in it, its twist, its target and the army's score, whether it was won,
/// its captain and its hero (a won fight's best soldier), and how many routed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Skirmish {
    pub day: u64,
    pub soldiers: u32,
    pub twist: Twist,
    pub target: f64,
    pub score: f64,
    pub won: bool,
    pub captain: Option<HandId>,
    pub hero: Option<HandId>,
    pub routed: u32,
}

/// A soldier with no hand of a town's (a realm's levy, a mercenary): a Journeyman at arms and archery, of no habits or
/// talents, in middling spirits.
pub fn stranger() -> Person {
    Person { skills: [(Work::Arms, 9.0), (Work::Archery, 9.0)].into(), spirits: 0.5, ..Default::default() }
}

/// The skirmishes the army keeps (the last so many).
pub const SKIRMISHES_KEPT: usize = 40;

/// Is a soldier's weapon a bow with arrows to shoot (a bow with none is no weapon)?
fn armed_with<'a>(content: &Content, s: &'a Soldier) -> (Option<&'a Worn>, bool) {
    let bow = s.weapon.as_ref().is_some_and(|w| bare(&content.goods[w.good].name) == BOW);
    (s.weapon.as_ref().filter(|_| !bow || s.arrows > 0), bow)
}

/// The work a soldier fights by: archery with a bow, arms with anything else.
pub fn work_of(content: &Content, s: &Soldier) -> Work {
    if armed_with(content, s).1 { Work::Archery } else { Work::Arms }
}

/// What a soldier brings to a fight: its skill at its weapon's work, four more with a talent for it, four a
/// Perfectionist, its weapon's power past a wooden sword's and its edge (unarmed, four less), a fifth of its armour's
/// points, and its kit's grades; a bow's a half in fog.
pub fn fundamentals(content: &Content, s: &Soldier, p: &Person, twist: Twist) -> f64 {
    let (arms, bow) = armed_with(content, s);
    let work = if bow { Work::Archery } else { Work::Arms };
    let name = |w: &Worn| content.goods[w.good].name.as_str();
    let mut f = p.skill(work) + if p.gifted(work) { 4.0 } else { 0.0 } + if p.has(Habit::Perfectionist) { 4.0 } else { 0.0 };
    f += arms.and_then(|w| weapon(name(w)).map(|g| g.power as f64 - 4.0 + w.enchant)).unwrap_or(-4.0);
    let worn: Vec<&Worn> = s.armour.iter().flatten().collect();
    f += points(worn.iter().map(|w| name(w))) / 5.0;
    let mut grades = [0.0; 5];
    for w in arms.into_iter().chain(worn.iter().copied()).filter(|w| armour(name(w)).is_some() || weapon(name(w)).is_some()) {
        grades[w.grade.unwrap_or(Grade::Plain).index()] += 1.0;
    }
    f += materials_grade(&grades);
    if bow && twist == Twist::Fog {
        f *= 0.5;
    }
    f.max(0.0)
}

/// An army's soldiers as people: drawn from their ids the first time, as a business's hands are.
fn as_people(army: &Army, persons: &mut Persons) {
    for s in &army.soldiers {
        if let Some(h) = &s.hand {
            let m = persons.hands.entry(h.id).or_insert_with(|| draw(h.id, h.level, &[]));
            for w in Work::ALL {
                m.skills.entry(w).or_insert(3.0 * h.level.clamp(1, 5) as f64);
            }
        }
    }
}

fn id(s: &Soldier) -> Option<HandId> {
    s.hand.as_ref().map(|h| h.id)
}

/// An army's captain, by the crown's mandate to command: the most skilled at its work; another once it's gone.
fn appoint(content: &Content, army: &mut Army, persons: &Persons, charters: &mut Charters, now: u64) {
    let serving: Vec<HandId> = army.soldiers.iter().filter_map(id).collect();
    if army.captain.is_some_and(|c| !serving.contains(&c.hand)) {
        let c = army.captain.take().unwrap();
        if let Some(m) = charters.mandates.iter_mut().find(|m| m.id == c.mandate) {
            m.until = Some(now);
        }
    }
    if army.captain.is_none() {
        let skill = |s: &Soldier| id(s).and_then(|h| persons.hands.get(&h)).map_or(0.0, |m| m.skill(work_of(content, s)));
        if let Some(best) = army.soldiers.iter().filter(|s| s.hand.is_some())
                .max_by(|a, b| skill(a).total_cmp(&skill(b)).then(id(b).cmp(&id(a)))) {
            let hand = id(best).unwrap();
            let mandate = charters.next_mandate;
            charters.next_mandate += 1;
            charters.mandates.push(Mandate { id: mandate, principal: Party::Crown, agent: hand, scope: vec![Verb::Command],
                source: Source::Appointment, since: now, term: None, until: None });
            army.captain = Some(Head { hand, mandate });
        }
    }
}

/// How an army fought: its score, its morale, and its best soldier's (a hand of a town's) score.
pub struct Fought {
    pub score: f64,
    pub morale: f64,
    pub best: Option<(f64, HandId)>,
}

/// An army's score in a fight (the `no`th it's fought, under `twist`): each soldier's Fundamentals × Morale, give or
/// take its draw, summed; see the module.
fn fight(content: &Content, army: &Army, persons: &Persons, twist: Twist, no: u64, now: u64) -> Fought {
    let d = &content.dials;
    let night = twist == Twist::Night;
    let captain = army.captain.and_then(|c| persons.hands.get(&c.hand).cloned().map(|m| (c.hand, m)));
    // Morale: its spirits, its captain's habits, less for the hungry and at night.
    let spirits = army.soldiers.iter().map(|s| id(s).and_then(|h| persons.hands.get(&h)).map_or(0.5, |m| m.spirits))
        .sum::<f64>() / army.soldiers.len().max(1) as f64;
    let morale = crate::persons::morale(spirits, captain.as_ref().map(|c| &c.1)) - 0.3 * army.unfed
        - if night { 0.1 } else { 0.0 };
    let masters: Vec<(HandId, Vec<crate::persons::Talent>)> = army.soldiers.iter()
        .filter(|s| s.hand.as_ref().is_some_and(|h| h.level >= 5))
        .filter_map(|s| id(s).and_then(|h| persons.hands.get(&h).map(|m| (h, m.talents.clone())))).collect();
    let taskmaster = captain.as_ref().is_some_and(|c| c.1.has(Habit::Taskmaster));
    let mut score = 0.0;
    let mut best: Option<(f64, HandId)> = None;
    let nobody = stranger();
    for (i, s) in army.soldiers.iter().enumerate() {
        // (A soldier of no town's roster fights as a stranger: its draw by its place in the army.)
        let h = id(s);
        let m = h.and_then(|h| persons.hands.get(&h)).unwrap_or(&nobody);
        let h = h.unwrap_or(u32::MAX - i as u32);
        let k = m.knack;
        let mut f = fundamentals(content, s, m, twist);
        if k == Some(Knack::GlassHands) {
            f *= 2.0;
        }
        let inspired = if m.inspired_until.is_some_and(|u| now < u) && !night { 0.5 } else { 0.0 };
        let sulking = if m.sulking_until.is_some_and(|u| now < u) { 0.3 } else { 0.0 };
        let lift = match k {
            Some(Knack::HouseStyle) => (0.1 * masters.iter().filter(|(o, ts)| *o != h && ts.iter().any(|t| m.talents.contains(t)))
                .count() as f64).min(0.5),
            Some(Knack::Unbroken) => 0.05 * army.won_in_row.min(10) as f64,
            _ => 0.0,
        };
        let spread = d.grade_spread * if m.has(Habit::Experimenter) { 2.0 } else { 1.0 };
        let mut one = crate::persons::score(f, morale + inspired - sulking + lift, spread, roll(h as u64, no, 73));
        if k == Some(Knack::SecondProof) {
            one = one.max(crate::persons::score(f, morale + inspired - sulking + lift, spread, roll(h as u64, no, 74)));
        }
        // (Under a Taskmaster, its crew a quarter more.)
        if taskmaster && captain.as_ref().is_some_and(|c| c.0 != h) {
            one *= 1.25;
        }
        score += one;
        if id(s).is_some() && best.is_none_or(|b| one > b.0) {
            best = Some((one, h));
        }
    }
    if twist == Twist::River {
        score *= 0.75;
    }
    Fought { score, morale, best }
}

/// What a fight did to an army: its risk till the next, its spirits, its hero, its soldiers' skill, and its routs
/// (the soldiers who routed, taken out of it).
fn after(content: &Content, army: &mut Army, persons: &mut Persons, fought: &Fought, won: bool, twist: Twist, now: u64)
        -> (Option<HandId>, Vec<Soldier>) {
    let d = &content.dials;
    let day = content.day_ticks.max(1);
    let night = twist == Twist::Night;
    let captain = army.captain.and_then(|c| persons.hands.get(&c.hand).cloned().map(|m| (c.hand, m)));
    army.won_in_row = if won { army.won_in_row + 1 } else { 0 };
    army.edge = if won { -0.5 } else { 0.5 };
    let hero = fought.best.filter(|_| won && !night).map(|b| b.1);
    // Spirits: each a third of the way to its mark (a little over a half, up for a win and down for a loss, less
    // hungry; a Beloved captain's up, a Taskmaster's down); its hero inspired; each more skilled for the fight.
    let mentor = captain.as_ref().is_some_and(|c| c.1.has(Habit::Mentor));
    for s in &army.soldiers {
        let Some(h) = id(s) else { continue };
        let level = s.hand.as_ref().map_or(1, |x| x.level);
        let work = work_of(content, s);
        let m = persons.hands.get_mut(&h).unwrap();
        let mut mark: f64 = 0.55 + if won { 0.15 } else { -0.2 } - 0.3 * army.unfed;
        if let Some((_, cm)) = captain.as_ref().filter(|c| c.0 != h) {
            mark += if cm.has(Habit::Beloved) { 0.2 } else { 0.0 } - if cm.has(Habit::Taskmaster) { 0.2 } else { 0.0 };
        }
        m.spirits += (mark.clamp(0.0, 1.0) - m.spirits) / 3.0;
        m.low_days = if m.spirits < 0.35 { m.low_days + 1 } else { 0 };
        if Some(h) == hero {
            m.inspired_until = Some(now + d.skirmish_days as u64 * day);
        }
        // (Experience at arms, whatever its trade's level: the newest learn most; John: "let combat grant real
        // experience, particularly for new soldiers".)
        let _ = level;
        let master = skill_cap(5);
        let lift = if m.gifted(work) { 1.5 } else { 1.0 } * if m.has(Habit::Ambitious) { 1.5 } else { 1.0 }
            * if mentor { 2.0 } else { 1.0 };
        let skill = m.skills.entry(work).or_insert(0.0);
        let gain = d.combat_skill * ((master - *skill) / master).max(0.0) * lift;
        *skill = (*skill + gain).min(master.max(*skill));
    }
    // Routed: lost with its morale broken (no Peacemaker serving), its soldiers in low spirits, never a Steady hand.
    let peace = army.soldiers.iter().any(|s| id(s).and_then(|h| persons.hands.get(&h)).is_some_and(|m| m.knack == Some(Knack::Peacemaker)));
    let mut routed = Vec::new();
    if !won && fought.morale < d.rout_morale && !peace {
        let mut i = 0;
        while i < army.soldiers.len() {
            let runs = id(&army.soldiers[i]).and_then(|h| persons.hands.get(&h))
                .is_some_and(|m| m.spirits < 0.35 && !m.has(Habit::SteadyHand));
            if runs {
                let s = army.soldiers.remove(i);
                army.records.entry(s.home).or_default().deserted += 1;
                routed.push(s);
            } else {
                i += 1;
            }
        }
    }
    (hero, routed)
}

/// Keeps a skirmish in an army's record (the last [`SKIRMISHES_KEPT`]).
fn keep(army: &mut Army, k: Skirmish) {
    army.skirmishes.push(k);
    if army.skirmishes.len() > SKIRMISHES_KEPT {
        army.skirmishes.remove(0);
    }
}

/// The army's skirmish today, against the land's target (`skirmish_target` a soldier, a tenth more for each won in
/// a row): see the module. The soldiers who routed, taken out of the army (they walk home).
pub fn skirmish(content: &Content, army: &mut Army, persons: &mut Persons, charters: &mut Charters, now: u64)
        -> Vec<Soldier> {
    if army.soldiers.is_empty() {
        return Vec::new();
    }
    as_people(army, persons);
    appoint(content, army, persons, charters, now);
    let no = army.skirmishes_fought;
    army.skirmishes_fought += 1;
    let twist = Twist::ALL[((roll(no, 0, 71) * 4.0) as usize).min(3)];
    let fought = fight(content, army, persons, twist, no, now);
    let target = content.dials.skirmish_target * army.soldiers.len() as f64 * 1.1f64.powi(army.won_in_row.min(30) as i32);
    let won = fought.score >= target;
    let soldiers = army.soldiers.len() as u32;
    let (hero, routed) = after(content, army, persons, &fought, won, twist, now);
    keep(army, Skirmish { day: now / content.day_ticks.max(1), soldiers, twist, target, score: fought.score, won,
        captain: army.captain.map(|c| c.hand), hero, routed: routed.len() as u32 });
    routed
}

/// Two armies meet (`docs/firms-plan.md` slice 5; John: "Faction armies, army vs army"): each's score is the other's
/// target, under one twist (drawn from `no`, the battle's fight). Whether the first won, and each's routed.
pub fn clash(content: &Content, a: &mut Army, b: &mut Army, persons: &mut Persons, charters: &mut Charters, no: u64,
        now: u64) -> (bool, Vec<Soldier>, Vec<Soldier>) {
    clash_lifted(content, a, b, persons, charters, no, now, [1.0, 1.0])
}

/// [`clash`], each army's score times its `lift` (a town's defenders at home, behind its walls: `rules/battle.rs`).
#[allow(clippy::too_many_arguments)]
pub fn clash_lifted(content: &Content, a: &mut Army, b: &mut Army, persons: &mut Persons, charters: &mut Charters,
        no: u64, now: u64, lift: [f64; 2]) -> (bool, Vec<Soldier>, Vec<Soldier>) {
    for army in [&mut *a, &mut *b] {
        as_people(army, persons);
        appoint(content, army, persons, charters, now);
        army.skirmishes_fought += 1;
    }
    let twist = Twist::ALL[((roll(no, 1, 71) * 4.0) as usize).min(3)];
    let (mut fa, mut fb) = (fight(content, a, persons, twist, 2 * no, now), fight(content, b, persons, twist, 2 * no + 1, now));
    fa.score *= lift[0];
    fb.score *= lift[1];
    // (A tie, by the toss of a coin: never the first army's.)
    let a_won = fa.score > fb.score || fa.score == fb.score && roll(no, 2, 71) < 0.5;
    let day = now / content.day_ticks.max(1);
    let (na, nb) = (a.soldiers.len() as u32, b.soldiers.len() as u32);
    let (ha, ra) = after(content, a, persons, &fa, a_won, twist, now);
    let (hb, rb) = after(content, b, persons, &fb, !a_won, twist, now);
    keep(a, Skirmish { day, soldiers: na, twist, target: fb.score, score: fa.score, won: a_won,
        captain: a.captain.map(|c| c.hand), hero: ha, routed: ra.len() as u32 });
    keep(b, Skirmish { day, soldiers: nb, twist, target: fa.score, score: fb.score, won: !a_won,
        captain: b.captain.map(|c| c.hand), hero: hb, routed: rb.len() as u32 });
    (a_won, ra, rb)
}
