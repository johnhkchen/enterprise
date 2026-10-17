//! Settings a study tunes, as in the mod's `Dials` (`Dials.java`): one catalogue, set by name for a world (`set`,
//! `/enterprise dial <name> <value>`, or `reset`), saved with it in its [`crate::Content`].
//!
//! **A dial is one line** in the list at the end, `name: type = the mod's default,`, under its doc comment. A slice
//! adds its dials as a block of lines under a comment naming it, at the end of the list. Git merges this file by
//! union (`.gitattributes`): two slices adding dials never conflict here; a line changed or taken out would come back
//! beside its new self, so changes to lines already here are the integrator's.

use serde::{Deserialize, Serialize};

/// A dial's value as the mod keeps every dial: a number. Whole-number dials round it (the mod's `Dial.getInt`), and a
/// switch is on for anything but 0.
pub trait DialValue: Sized {
    fn from_f64(value: f64) -> Result<Self, String>;
    fn to_f64(&self) -> f64;
}

impl DialValue for f64 {
    fn from_f64(value: f64) -> Result<Self, String> {
        if value.is_finite() { Ok(value) } else { Err(format!("{value} isn't a number")) }
    }
    fn to_f64(&self) -> f64 {
        *self
    }
}

impl DialValue for u32 {
    fn from_f64(value: f64) -> Result<Self, String> {
        let n = value.round();
        if n.is_finite() && (0.0..=u32::MAX as f64).contains(&n) { Ok(n as u32) } else { Err(format!("{value} isn't a count")) }
    }
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

impl DialValue for u64 {
    fn from_f64(value: f64) -> Result<Self, String> {
        let n = value.round();
        if n.is_finite() && (0.0..=u64::MAX as f64).contains(&n) { Ok(n as u64) } else { Err(format!("{value} isn't a count")) }
    }
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

impl DialValue for bool {
    fn from_f64(value: f64) -> Result<Self, String> {
        if value.is_finite() { Ok(value != 0.0) } else { Err(format!("{value} isn't a switch")) }
    }
    fn to_f64(&self) -> f64 {
        if *self { 1.0 } else { 0.0 }
    }
}

/// The list made into `Dials`: the struct (in the list's order, which is the save's), its defaults, and dials by name.
macro_rules! dials {
    ($($(#[$doc:meta])* $name:ident: $ty:ty = $default:expr,)*) => {
        /// Settings a study tunes, as in the mod's `Dials`.
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct Dials {
            $($(#[$doc])* pub $name: $ty,)*
        }

        impl Default for Dials {
            fn default() -> Self {
                Dials { $($name: $default,)* }
            }
        }

        impl Dials {
            /// Every dial's name, in the order they're declared.
            pub fn names() -> Vec<&'static str> {
                vec![$(stringify!($name)),*]
            }

            /// A dial's value, as a number.
            pub fn get(&self, name: &str) -> Option<f64> {
                match name {
                    $(stringify!($name) => Some(DialValue::to_f64(&self.$name)),)*
                    _ => None,
                }
            }

            /// Sets a dial for this world.
            pub fn set(&mut self, name: &str, value: f64) -> Result<(), String> {
                match name {
                    $(stringify!($name) => self.$name = DialValue::from_f64(value)?,)*
                    _ => return Err(format!("no dial called {name}")),
                }
                Ok(())
            }

            /// Puts a dial back to the mod's default.
            pub fn reset(&mut self, name: &str) -> Result<(), String> {
                let initial = Dials::default();
                match name {
                    $(stringify!($name) => self.$name = initial.$name,)*
                    _ => return Err(format!("no dial called {name}")),
                }
                Ok(())
            }
        }
    };
}

dials! {
    /// Days of what its people use a town's warehouse aims to hold of each good (its shelf).
    shelf_days: f64 = 5.0,
    /// Share of its free money a town will spend in a day.
    budget_share: f64 = 0.5,
    /// A buyer allows a seller who's been away at most this many days' budget at once.
    max_budget_days: f64 = 6.0,
    /// Share of its slots a wagon fills before it goes early.
    wagon_full: f64 = 0.75,
    /// Slots in a wagon, a stack each.
    wagon_slots: u32 = 27,
    /// How often the core looks at its wagons and towns, in ticks.
    look_ticks: u64 = 1200,
    /// A wagon that sold a good well drafts a standing order for it.
    standing_orders: bool = true,
    /// Days of what its people use, at its worth, a town keeps back from spending: its trading capital.
    reserve_days: f64 = 2.0,
    /// Bills on a town falling due this many days ahead aren't free money.
    budget_days: f64 = 3.0,
    /// What each villager uses of each kind of comfort a day, in emeralds' worth; days of it a town buys up to, and
    /// keeps when others would buy it.
    ration: f64 = 0.1,
    comfort_days: f64 = 6.0,
    comfort_keep_days: f64 = 4.0,
    /// Mood moves halfway to where it's heading in this many days; every so many villagers cost a point of it;
    /// each villager without a bed half a point; unpaid wages, up to two points.
    mood_half_life_days: f64 = 3.0,
    villagers_per_want: f64 = 8.0,
    crowding_mood: f64 = 0.5,
    unpaid_mood: f64 = 2.0,
    /// The households' wages: this share of the worth of what the town makes.
    labour_share: f64 = 0.5,
    /// A home (two beds) once beds are full and mood is at least `home_mood`, after `home_days`, for `home_cost`.
    home_mood: f64 = 0.5,
    home_days: f64 = 4.0,
    home_cost: f64 = 10.0,
    /// A child once a bed is free and mood is at least `birth_mood`, after `birth_days`, for `birth_cost` and
    /// `birth_food` food points.
    birth_mood: f64 = 1.0,
    birth_days: f64 = 4.0,
    birth_cost: f64 = 3.0,
    birth_food: f64 = 36.0,
    /// Territory (`territory-growth`): culture a hex costs per hex already held, and how many rings out a town may grow;
    /// a guard's daily food (points) and pay, days of pay a guard on a tolling post waits for; the toll a town asks
    /// until it sets its own.
    culture_per_hex: f64 = 5.0,
    max_rings: u32 = 2,
    guard_food: f64 = 6.0,
    guard_pay: f64 = 0.5,
    guard_credit_days: f64 = 3.0,
    default_toll_rate: f64 = 0.05,
    // Advances and refunds (`contract/Advance.java`).
    /// The share of a contract's worth the buyer pays up front, for a draft that asks for an advance (a third).
    advance_share: f64 = 1.0 / 3.0,
    // bargain-and-par
    /// Merchants bargain by level at the far end (`Bargain.java`); off, every good sells at its full value.
    bargain: bool = true,
    // Settlers (`caravan/Expeditions.java`, `Dials.java`).
    /// Emeralds a travelling Merchant carries at most to start a market in a village nobody trades with; and the
    /// settlers' to start a new town, if their town can spare twice that.
    settling_capital: f64 = 30.0,
    founding_capital: f64 = 42.0,
    /// Days between a town's parties of settlers, and the hexes out a village or open ground must be (from every
    /// market, and at most from home).
    days_between: f64 = 8.0,
    min_hexes: u32 = 4,
    max_hexes: u32 = 8,
    // farms-and-wages (`farms.rs`)
    /// Share of its households' savings a town's treasury may borrow, to honour its bills or buy paper.
    deposit_share: f64 = 0.5,
    /// What the treasury pays its households a day on what it has borrowed of their savings.
    deposit_interest_per_day: f64 = 0.002,
    /// Emeralds beyond its reserve the treasury keeps before it repays its households.
    deposit_keep: f64 = 10.0,
    /// Plots turned over to another crop a review.
    rotate_plots: u32 = 81,
    /// Times as many plots a review for a crop owed under contract.
    rotate_owed_fields: u32 = 2,
    // herds (`territory/Husbandry.java`)
    /// Animals a pasture holds.
    pasture_animals: u32 = 16,
    /// The share a fed herd grows by a day, while there's room.
    breed_per_day: f64 = 0.15,
    /// The share a hungry herd dwindles by a day.
    starve_per_day: f64 = 0.1,
    /// A Butcher culls a kind beyond this share of the room, divided among the kinds kept.
    cull_share: f64 = 0.75,
    /// A Butcher or Leatherworker always keeps this many of a kind to breed from.
    breeding_stock: f64 = 4.0,
    /// The share of the surplus a Butcher culls a day.
    slaughter_per_day: f64 = 0.5,
    /// Wool a shorn sheep gives a day.
    wool_per_sheep_day: f64 = 0.5,
    // road-food (`road.rs`, `Caravans.java`)
    /// The most food points a wagon carries a leg (`FOOD_CAPACITY`, a stack of bread): its longest leg.
    food_capacity: u32 = 320,
    /// A mouth eats a food point every so many blocks (`BLOCKS_PER_FOOD`).
    blocks_per_food: u32 = 10,
    /// Money a wagon takes per food point of the road home, to buy it at the far end; and what a point bought on the
    /// way costs (`PURSE_PER_FOOD_POINT`: bread is 30 points to the emerald, half again for dearer food).
    purse_per_food_point: f64 = 1.5 / 30.0,
    /// Ticks a wagon waits at the far end for food before it sets off home on short rations (`MAX_WAIT_TICKS`).
    max_wait_ticks: u64 = 12000,
    /// Animals in a wagon's team, each eating a mouth's food in fodder there and back (`WAGON_TEAM`).
    wagon_team: u32 = 1,
    // Forward crop sales and rents (`Dials.java`: `contract/Harvest.java`, `caravan/ForwardSales.java`, `contract/Rents.java`).
    /// Days after an installment's day a farm may still deliver it.
    harvest_window_days: u32 = 4,
    /// Installments a farm may fall behind before a shortfall is a miss.
    harvest_arrears: f64 = 2.0,
    /// A shortfall rolled over costs this much of a miss in trust.
    harvest_rolled_weight: f64 = 0.25,
    /// Days a farm has to sow before its first installment falls due.
    forward_sow_days: f64 = 4.0,
    /// At most this many of a crop an installment; installments in a forward sale.
    forward_max_quantity: f64 = 256.0,
    forward_installments: u32 = 10,
    /// The share of a forward sale's worth the buyer pays up front (`Dials.ADVANCE_SHARE`, a third).
    forward_advance_share: f64 = 1.0 / 3.0,
    /// Markets offer farm towns forward sales and rents of the crops they lack (`ForwardSales.offer`).
    forward_sales: bool = true,
    /// Share of what a farm grows of a crop it may promise more of.
    could_grow_own_share: f64 = 0.5,
    /// Share of what it grows of its other field crops it may promise of one (they can be turned over).
    could_grow_other_share: f64 = 0.25,
    /// A rent's season (a coupon falls due each), the seasons' crop it sells for, and the arrears (in seasons' crop)
    /// past which it is a default.
    rent_season_days: u32 = 10,
    rent_purchase_seasons: f64 = 8.0,
    rent_arrears_seasons: f64 = 2.0,
    /// A dealer buys this much under its price and sells this much over.
    dealer_spread: f64 = 0.1,
    // jobs (`territory/Jobs.java`)
    /// A stall's workstation, chest and labour, in emeralds.
    stall_emeralds: f64 = 6.0,
    /// The share of its households' savings a comfort the town lacks puts in the job draw, for the trade that makes it.
    comfort_marbles: f64 = 0.2,
    /// Days a villager's trades do nothing for the town before it takes up a trade the town needs.
    slack_days: u32 = 5,
    /// Idle plots must yield at least this much a day, in emeralds, to put Farmers' marbles in the bag.
    farmer_idle_emeralds: f64 = 0.5,
    /// Chokepoints (`chokepoints`): towns review their tolls and posts themselves (off: they keep what the host sets),
    /// every so many days; they ask at most this share of a route's detour, at most this toll, and a passage fee of
    /// at most so many emeralds; they try at most so many posts a route, and forget a route unseen for so many reviews.
    toll_reviews: bool = true,
    toll_review_days: f64 = 3.0,
    detour_share: f64 = 0.8,
    max_toll_rate: f64 = 0.25,
    passage_fee_cap: f64 = 3.0,
    max_posts_per_route: u32 = 6,
    forget_after_reviews: u32 = 4,
    // crates-and-papers (`cargo.rs`: `cargo/Crates.java`, `cargo/Paperwork.java`)
    /// What a new crate costs (`Crates.CRATE_EMERALDS`: a log's worth of wood and the work).
    crate_emeralds: f64 = 0.125,
    /// Empty crates a town keeps to ship with; the rest go back with a wagon (`Crates.EMPTIES_KEPT`).
    empties_kept: u32 = 16,
    /// Crates of shipments a town's warehouse has room for (the mod counts its chests' free slots): a farm with none
    /// left crates nothing and draws no papers.
    crate_room: u32 = 27,
    /// What a scribe charges for a document the town lacks the stationery for (`Paperwork.SCRIBE_EMERALDS`).
    scribe_emeralds: f64 = 0.1,
    /// Ink sacs and feathers a document takes (`Paperwork.INK_PER_DOCUMENT`, `QUILL_PER_DOCUMENT`).
    ink_per_document: f64 = 1.0 / 8.0,
    quill_per_document: f64 = 1.0 / 32.0,
    // buildings-and-works (`works.rs`: `territory/Works.java`, `service/BuildServices.java`)
    /// Masons on a project, each paid `mason_pay` a day and laying `blocks_per_mason_day` blocks (`Works.MASONS`,
    /// `MASON_PAY_PER_DAY`, `BLOCKS_PER_MASON_DAY`).
    masons: u32 = 2,
    mason_pay: f64 = 1.0,
    blocks_per_mason_day: f64 = 32.0,
    /// Stone each hex a town holds supplies a day (`Works.STONE_PER_HEX_DAY`).
    stone_per_hex_day: f64 = 16.0,
    /// A town builds a bridge if its tolls should repay the masons' wages within this many days (`Works.PAYBACK_DAYS`).
    payback_days: f64 = 30.0,
    /// A wall's blocks: its ring of 252 columns, 3 high (`Works.ring`, `WALL_HEIGHT`); and the emeralds a town needs to
    /// spare to start one (`Works.WALL_START_EMERALDS`).
    wall_blocks: u32 = 756,
    wall_start_emeralds: f64 = 12.0,
    /// A town builds a hall only with this many times its whole cost to spare beyond its reserve
    /// (`BuildServices.SAVINGS`), with builders of at most this level when it has no Mason (`HIRED_SKILL`).
    build_savings: f64 = 2.0,
    hired_skill: u32 = 3,
    /// Documents a day a market draws up before it wants a counting house (`Dials.COUNTING_DOCUMENTS`).
    counting_house_documents: f64 = 3.0,
    // training-and-tenders (`training.rs`: `market/Tenders.java`, `market/Apprentices.java`, `Sovereign.TRAINING_BONUS`)
    /// What a master is worth to a buyer who tenders for one, at the top level (`Sovereign.TRAINING_BONUS`).
    training_bonus: f64 = 24.0,
    /// A Merchant carrying a tender on keeps this share of its price (`Tenders.RELAY_MARGIN`).
    relay_margin: f64 = 0.15,
    /// What a Merchant reckons training costs its town a level: coal, iron and the rest bought in (`Tenders.LEVEL_COST`).
    level_cost: f64 = 2.0,
    /// The days a level takes, with the coal and iron to hand, and the days to get them there (`DAYS_PER_LEVEL`, `SUPPLY_DAYS`).
    days_per_level: f64 = 2.0,
    supply_days: f64 = 2.0,
    /// Slots in a town's warehouse, and the free ones it keeps (`MerchantStock.FREE_SLOTS`): the core has no chests, so
    /// a stack of a good is a slot and the warehouse holds this many (a double chest's worth).
    warehouse_slots: u32 = 54,
    free_slots: u32 = 5,
    // squads (`squad.rs`, `caravan/Squad.java`, `Commissions.java`, `Caravans.java`)
    /// What the desk's Merchant pays a hand to sign on as a travelling Merchant (`Squad.SIGNING_EMERALDS`).
    signing_emeralds: f64 = 2.0,
    /// What training a hand for a squad costs (`Commissions.TRAINING_EMERALDS`).
    training_emeralds: f64 = 4.0,
    /// Stacks a Leatherworker's packs, or a porter's back, carry beyond the wagon (`Squad.PACK_STACKS`).
    pack_stacks: u32 = 9,
    /// A saddled wagon's pace: a third faster (`Caravans.SADDLED_PACE`).
    saddled_pace: f64 = 4.0 / 3.0,
    /// The share of a saddle a trip wears out (`Caravans.SADDLE_WEAR_PER_TRIP`).
    saddle_wear: f64 = 0.1,
    // bank-and-discounts (`bank.rs`: `market/Bank.java`, `market/Discounts.java`, `contract/Bills.java`)
    /// A bank buys a bill not yet due for its face less this, and so much a day to its due day (`Bank.DISCOUNT_FEE`,
    /// `DISCOUNT_PER_DAY`).
    discount_fee: f64 = 0.01,
    discount_per_day: f64 = 0.01,
    /// A bank lends a town up to this share of what the goods still to come under a contract are worth, at this much
    /// over the principal (`Bank.LOAN_SHARE`, `LOAN_INTEREST`).
    loan_share: f64 = 0.5,
    loan_interest: f64 = 0.03,
    /// A bank's wagon carries this much more for its dealings, as far as its market can spare (`Bank.FLOAT`).
    bank_float: f64 = 48.0,
    /// Days a drawee has to pay a bill from when it's first put to it, and a day more for every so many emeralds it
    /// owes the same holder (0: none) (`bill_grace_days`, `bill_grace_emeralds_per_day`).
    bill_grace_days: f64 = 8.0,
    bill_grace_emeralds_per_day: f64 = 100.0,
    /// Days past its day a bill no one can carry to its drawee lapses (`Bills.LAPSE_TICKS`).
    lapse_days: f64 = 12.0,
    /// A hub's discount desk: the most paper it holds as a share of its spare money; the most of that on one market's
    /// paper, and on foreign markets'; more off a bill whose goods it can't see yet; a bill due further out than so many
    /// days is past its season, and costs so much more; more off at a full warehouse, and again at a full book; more
    /// off a bill past its day (`Dials.java`).
    discount_book_share: f64 = 1.0,
    discount_drawee_share: f64 = 0.35,
    discount_foreign_share: f64 = 0.5,
    discount_unbacked: f64 = 0.04,
    discount_long_days: f64 = 12.0,
    discount_long: f64 = 0.05,
    discount_throttle: f64 = 0.03,
    stale_bill_discount: f64 = 0.1,
    // realms (`realms.rs`, `trade/Realms.java`, `Caravans.java`)
    /// The crown's share of what a foreign trader sells inside the kingdom (`Realms.CUSTOMS`).
    customs: f64 = 0.1,
    /// What a realm's people use a day of each good it wants (`Realms.USES_PER_DAY`).
    realm_uses_per_day: u32 = 64,
    /// A foreign realm's Merchants haggle at this level, and this many of its people go with each caravan
    /// (`Caravans.FOREIGN_MERCHANT_LEVEL`, `FOREIGN_CREW`).
    foreign_merchant_level: u32 = 4,
    foreign_crew: u32 = 2,
    // atlas-and-books (`atlas.rs`: `caravan/Atlas.java`, `Commissions.java`, `Caravans.STALE_BOOKS_TICKS`)
    /// Markets this close (in blocks) know of each other without anyone travelling (`Atlas.WORD_OF_MOUTH`).
    word_of_mouth: f64 = 400.0,
    /// A town commissions a caravan of its own only with this much to spare beyond its reserve (`COMMISSION_SPARE`).
    commission_spare: f64 = 8.0,
    /// A far end's books are stale this many days after a Librarian last copied them (`STALE_BOOKS_TICKS`).
    stale_books_days: f64 = 3.0,
}
