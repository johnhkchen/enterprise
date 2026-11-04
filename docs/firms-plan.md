# Businesses, their masters and their houses

The plan for firms past step 3 of "How to Add Society Systems to the Sim" (John, 2026-11-04). It replaces step 4's
first try (foremen with hidden competence and loyalty who skimmed the takings: "Magic production and way too easy to
blackball embezzlement seems boring"). Its models: RimWorld's colonists (habits, talents, inspiration, moods that
break) and gacha rosters (characters collected and invested in, drawn as rare arrivals), with Balatro's scoring as its
spine: what a piece is worth is a sum times a multiplier, shown, and the masters' knacks bend its rules and stack.

> "A famous clothing brand might involve excellent in house tanners, sewing experts, and another firm that specializes
> in rabbit hide nearby. This sort of bonus stacking should be the foundation of more advanced play." (John)

## Glossary

The words a player reads. The code follows them where a player sees it (a firm is `firm` in code); each word's
tooltip is `persons::GLOSSARY`, each habit's, knack's, grade's and work's its `tooltip()`.

| Word | What it is |
|---|---|
| **Business** | A firm: a household's capital put into a workshop, its hands on wages (`firms.rs`). |
| **Head** | The hand its owners put in charge of the others, by a mandate (`charters.rs`): head baker, head tanner. |
| **Habit** | One of a hand's two ways that cut both ways (Perfectionist: finer work, slower). |
| **Talent** | A person's gift for a kind of work (tanning, baking) or a material (rabbit hide, cocoa). |
| **Skill** | What a person has learnt of a kind of work by doing it; its rank the vanilla one (Novice to Master). |
| **Knack** | A Master's own way of doing things, which bends a rule (Balatro's jokers). One each, unlocked at Master. |
| **Inspiration** | A run of great work, from good spirits. |
| **Off form** | What low spirits do: walks out, quarrels, sulks. Each one seen, with a cause that can be put right. |
| **Fundamentals** | What a person brings to a task: its skill, its talents, the grade of its materials or kit. |
| **Morale** | How the shop is running: its head, its crew and their spirits. Not only crafting's: a squad has morale too. |
| **Grade** | A piece's worth: Seconds, Plain, Good, Fine, Masterwork (Fundamentals × Morale sets it). |
| **Steps** | The works a business does in turn: a bootmaker's tanning, then its sewing. |
| **Suppliers** | The businesses a business buys its materials of. |
| **Reputation** | A house's fame: the basket of what its name does (its premium, its customers' loyalty, its draw...). |
| **Draw** | The part of reputation that brings masters to it: who comes seeking work. |
| **House** | A business whose reputation has made it a name: the House of Garrick. |

## The four layers of a hand

1. **Habits** (two, fixed; in time some come of its history: a tenant years on the land, a raid lived through).
2. **Talents** (one or two): for a process or an ingredient. Working at its talent a hand learns faster, works in
   better spirits and adds to the fundamentals.
3. **Skills** by process, grown by doing and fading a little unused: a tanner and a sewer of the same rank are
   different hands.
4. **A knack** at Master: a rule bent, not a number added. Knacks stack, and their order matters.

Habits (twelve to start; cut as they prove dull):

| Habit | For | Against |
|---|---|---|
| Perfectionist | More fundamentals | Makes less |
| Steady hand | Never makes seconds | Rarely inspired |
| Experimenter | Its grades spread wide (masterworks and seconds) | Its grades spread wide |
| Tinkerer | The station it works breaks down half as often | None yet |
| Mentor | As head: its crew learn twice as fast | Makes less itself |
| Taskmaster | As head: its crew make a quarter more | Its crew's spirits fall |
| Beloved | As head: its crew's spirits rise | Soft on poor work: less morale |
| Kin-minded | As head: its kin among the crew in better spirits | Takes on kin before the more skilled |
| Showman | Its pieces earn its business half again the reputation | Its off-form spells are talked of: each costs a twentieth of its name |
| Hothead | Inspired twice as often | Quarrels twice as often |
| Ambitious | Learns half again as fast | Long in low spirits, it leaves |
| Homebody | Never walks out | Won't run a branch |

Knacks (a first ten, slice 2, and Patron with reputation, slice 3; the tooltip a player reads is `Knack::tooltip`, and a test keeps this table to them):

| Knack | The rule it bends |
|---|---|
| House Style | +0.1 morale for each other Master of its business who shares a talent with it |
| Supply Whisperer | Materials bought of another business count a grade higher |
| Copycat | Has the knack of a Master at the business it buys its materials of most |
| Unbroken | +0.05 morale for each day in a row it has made no seconds, up to +0.5; seconds start it over |
| Second Proof | Every third batch is graded twice, the better kept |
| Glass Hands | Twice the fundamentals; one batch in ten ruined |
| Scrap Artist | Its seconds aren't sold: worked back in, +4 fundamentals to its next batch |
| Rabbit's Foot | Whatever it makes of rabbit hide comes out a grade higher |
| The Regular | +0.05 morale for each day in a row its town has bought of its business, up to +0.5 |
| Peacemaker | No quarrels in its shop: whoever would quarrel sulks instead |
| Patron | Its pieces sold in its home town earn its business twice the reputation (slice 3) |

A hand finds its knack the day it's a Master in a business (drawn from its id, `persons::knack_of`).

## One engine: a person's task

Every kind of work runs on the same engine (`sim/core/src/persons.rs`, John: "Generalize"): a person's make-up (habits,
talents for a kind of work or a material, skill at each work, spirits), and a task scored as **Fundamentals × Morale**,
give or take a draw of its own. The score sets a piece's grade; against a target, it's whether a contract or a fight is
won. A batch at a bench is a task; so, later, is a skirmish's round and a works' heat of steel.

| Piece | Workshop | War | Industry |
|---|---|---|---|
| Fundamentals | skill, talents, the materials' grade | skill, talents, the kit's grade | skill, capped by the station |
| Morale | its head, its crew's spirits | its captain, its spirits, its supplies | at a works' scale: heads over heads |
| The leader | head (a mandate: hire, command) | captain (the same mandate) | heads over heads |
| A good spell | inspiration | a heroic stand | a record run |
| A bad spell | walks out, quarrels, sulks | routs, deserts, loots | strikes |
| The test | its town's wants | a skirmish: a target and a twist (Balatro's boss blinds) | big orders: 1,000 Good rails by day 60 |

**War.** Soldiers are the same people (a Hothead charges early, a Steady hand never routs; talents for a weapon or a
ground; skill at archery, the pike, riding). Kit's grade is where the economy meets the war: a steel house's
masterwork blades in a smith's swords are a squad's fundamentals. A skirmish is fought in rounds against a target
with a twist (fog: ranged knacks off; a river: half pace; night: no inspiration); morale broken, it routs. The
wounded are the Clerics' work. It builds on `army.rs`, `squad.rs`, garrisons and raids.

**Industry, the Carnegie way.** Ore's grade, coke's, iron's and steel's flow down the chain, so owning the mine and the
coke ovens lets their masters' knacks reach the furnace. A process is capital: a station (a Bessemer converter, an
open hearth; in the game a Create machine the player builds) with a throughput, a grade ceiling, upkeep and
breakdowns: capital buys the ceiling, masters reach it. Scale or grade is a real fork (bulk rails, or crucible
blades). Big orders are boss blinds. A works' heads over heads make morale its politics: a Taskmaster's output, and
the strike it risks. The fun is the combination that breaks the curve, shown in every breakdown.

## Grading a batch

Fundamentals = the hand's skill at the work + four with a talent for it + two for each material it has a talent for +
four a Perfectionist + its materials' grades (two for each grade a piece is above Plain, a Masterwork's six; seconds
two less; on average, `persons::materials_grade`). Morale = 1 + a third of how far its crew's spirits are
from a half, less a tenth under a Beloved head, + a half inspired or less three tenths sulking. The score, give or
take `grade_spread` (0.3; twice an Experimenter's), sets the grade: Seconds under 6, Plain under 12, Good under 18,
Fine under 30, Masterwork beyond (a Master's capped skill, 20, is Fine; a masterwork takes more).
A piece's worth is its good's value times its grade's: Seconds 0.75, Plain 1, Good 1.25, Fine `fine_quality` (1.5),
Masterwork 2; in comfort the same (a grade above Plain lifts the mood as the finer tier does; seconds take from it).
The finer tier the lockers buy of villagers' selling trades is Fine.

## Slices

1. **Hands and grades** (replaces step 4's first try): each hand's habits, talents and skills; grades in a town's
   stock; a business's batches graded by Fundamentals × Morale; a head appointed (the most skilled in the process, as
   its owners see it); its hands' spirits, inspirations and off-form spells. The trace shows each hand's make-up, each
   business's batches by grade and its events.
2. **Suppliers and knacks**: processes in steps within a business (tanning, then sewing); materials' grades in the
   fundamentals; businesses buying of businesses; the first ten knacks. Done (2026-11-04): a **tannery** cures its
   town's hides (a cow's leather, plain) into graded leather and tans rabbit hides into it, and sells its town the
   leather; a **bootmaker** (a step-3 tannery, renamed) tans what leather it can't buy graded of its town, then sews
   boots, its hands each at the step they bring most to. A business's materials are its own earlier step's first, then
   its town's graded pieces, best first, then plain; graded pieces another business of its town made are bought of it
   (its suppliers, `Firm::suppliers`), at what a grade of it adds to their own pieces if more than its grade's worth
   (John: "Buyer bids its gain", `firms::bids`). Each batch spends its hands' pace (slice 1 never did: a business made all its
   town wanted at each look).
3. **Houses**: reputation and draw; grades carried by wagon; masters coming seeking work; branches. Reputation,
   Houses, their premium and draw done (2026-11-04): a piece sold earns its worth beyond a plain one's, fading 1% a
   day; at 200 a business is a House with a name; its pieces sell at up to 0.3 more; the hands newly of its trade in
   its town come gifted at its draw. Grades by wagon done: a wagon's goods carry their grades out, are sold by grade at
   the far end (each grade's worth of the price), and its sale's grades go to the host. Branches done: a House saves a
   branch's capital and working days, sends them by its town's next wagon down a road to a town with no business of
   its kind that wants its pieces and has a hand (not a Homebody) it would pay; the branch is founded when the money
   gets there, under the House's name with half its reputation, its head by the House's mandate, and its takings ride
   home with the House's wagons. Slice 3 done but for masters moving between towns (the host's villagers).
4. **Stations and chains** (moved up from 3, industry's heart and "capital investment for higher quality"): a
   process's station, its throughput and grade ceiling; multi-step chains (ore to steel). Stations done: each step
   works at a station, its first holding its pieces to Fine, a second letting them be masterworks, each tier a quarter
   faster and its kind's capital again; it breaks down 2% of days a tier (half where a Tinkerer works it), repaired
   the next day at a tenth of its price; a business saves for and buys the next tier where it pays for itself in 60
   days. The iron chain done: an ironworks (Armorers) smelts raw iron and coal or charcoal into graded ingots, a
   bladesmith (Weaponsmiths) and a toolmaker (Toolsmiths) forge them into swords and hoes, a town wanting two of each
   for its wearers; graded kit counts as its grade, as enchanted kit does (John: "Count like enchantment").
5. **War**: a skirmish as a task against a target with a twist, on the same engine. Done (2026-11-04, `war.rs`):
   every 5 campaign days the army fights a skirmish, its soldiers' Fundamentals (skill at arms or archery, talents,
   weapon, armour and their grades) × Morale (spirits, hunger, its captain by the crown's mandate) against 9 a soldier,
   a tenth more for each won in a row, with a twist (open ground, fog: bows a half, a river: a quarter less, night: no
   inspiration). Won, the army's risk is half till the next and its best makes a heroic stand; lost, half again, and
   with its morale broken its low-spirited rout (never a Steady hand, nor under a Peacemaker). Second proof, Glass
   hands, House style and Unbroken bend a fight as they do a batch.
