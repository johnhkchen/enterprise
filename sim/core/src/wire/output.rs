//! Outcomes out: what happened in the society, for the host to show.

use serde::{Deserialize, Serialize};

use super::{GoodNo, HandNo, PackageLine, Place, Point, RouteNo, TownNo, Work};
use crate::fixed::Fixed;

/// What happened, for the host to show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Output {
    WagonLeft { route: RouteNo, stacks: u32, worth: f64 },
    WagonSold { route: RouteNo, sold: f64, takings: Fixed },
    WagonHome { route: RouteNo, takings: Fixed },
    /// A master Merchant filled a player's order: `lots` lots, `count` goods, for `spurs`.
    OrderFilled { town: TownNo, order: u32, good: GoodNo, lots: u32, count: f64, spurs: f64 },
    /// A desk newly posted a want no market it knows holds, at `spurs_each` an item.
    Signal { town: TownNo, good: GoodNo, count: f64, spurs_each: f64 },
    /// A pallet was packed at `at`, of these packages in order: the host shows its crates there.
    PalletPacked { id: u64, at: Place, packages: Vec<PackageLine> },
    /// A pallet went to `to` (loaded, carried, delivered to a hold): its crates move with it.
    PalletMoved { id: u64, to: Place },
    /// A pallet was opened at `at` (by its consignee, or taken apart by its holder): its crates are empty, and it is gone.
    PalletOpened { id: u64, at: Place },
    /// A child was born at a town (`people.rs`): a hand with no villager yet. The host spawns one for it by the town's
    /// bell and names it (`TagHand`).
    Born { town: TownNo, hand: HandNo },
    /// A hand came to live at a town from elsewhere (a party of settlers arriving, or one come home): a hand with no
    /// villager there yet. The host spawns one for it and names it (`TagHand`).
    Settled { town: TownNo, hand: HandNo },
    /// A hand left its town with a party of settlers: its villager (the host's `tag`) leaves the world, and the hand is
    /// off it until it settles (`Settled`, under a new villager). The core forgets the tag.
    Departed { town: TownNo, hand: HandNo, tag: String },
    /// A hand is no more (the rules took it from the world: a soldier fallen), or a census saw the villager of one that
    /// is: its villager (`tag`) leaves the world.
    HandGone { hand: HandNo, tag: String },
    /// The core changed a hand's trade, level or XP (a trade taken up, a locker's trade): written to its villager.
    HandLevel { hand: HandNo, tag: String, trade: String, level: u8, xp: u32 },
    /// Items for the host to put in a desk's or a depot's chests (`count` above nothing) or take out of them (below):
    /// what the core moved in or out of its town's stock (a wagon's sale, its people eating) since the chests were
    /// counted, in whole items, after a stocktake there in the same look. The core takes the chests to hold it from now
    /// on; what the host can't do it sends back as `Unmoved`.
    Moved { point: Point, good: GoodNo, count: i64 },
    /// Money for the host to put in a desk's chests and Banker (above nothing) or take out of them (below), after the
    /// desk's `Cashcount` in the same look: whole `emeralds` in the chests, `coins` spurs of Numismatics coin in them
    /// (the host mints the fewest coins for what the chests hold with it), and `bank` spurs in the Banker's account.
    /// What the core's treasury, the mint and the town's reserve of emeralds worked out since the till was counted (a
    /// wagon's takings, wages paid, surplus emeralds sold to the mint for coin). The core takes the till to hold it from
    /// now on; what the host can't do it sends back as `CashUnmoved`.
    Cash { point: Point, emeralds: i64, coins: i64, bank: i64 },
    /// A town's work for the host to lay in its world, up to `share` of it (0 to 1): a bridge or a hall the masons
    /// finished, a wall a quarter at a time as they lay it, a field the town planted. Given at the end of a look that
    /// surveyed the town, until the host says it's laid (`Built`), so a town whose ground wasn't loaded is shown its works
    /// when it is. Where it goes (a hall's site, a field's) is the host's to find.
    Build { town: TownNo, work: Work, share: f64 },
    /// A hand of a town went away (a wagon's crew or a porter leaving, a soldier to the war): its villager (the host's
    /// `tag`) is off the world until it's back (`HandBack`), kept by the host as it left. Only for a hand the host named.
    HandAway { town: TownNo, hand: HandNo, tag: String },
    /// A hand away is home again (its wagon home, a porter walked back, a soldier from the war): its villager (`tag`; empty
    /// if the host never named it) back in the world by its town's desk, with the trade, level and XP the core has for it
    /// now (it may have earned some on the road). A host with no villager kept for it spawns one and names it (`TagHand`).
    HandBack { town: TownNo, hand: HandNo, tag: String, trade: String, level: u8, xp: u32 },
}
