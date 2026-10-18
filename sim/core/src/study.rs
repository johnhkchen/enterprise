//! The host's reads the network study needs (`sim/host`, the report and CSV of `dev/experiments/network_study.py`),
//! found missing when the host built the study's world: plain reads of what the slices keep. (A route's whole state
//! is the sweep's `Society::route_state`.)

use crate::cash_book::{Account, CashBook, Reason};
use crate::fixed::Fixed;
use crate::messages::{RouteId, TownId};
use crate::society::Society;

impl Society {
    /// The tolls a route's wagons have paid, all told: its wagon's purse book's tolls (`rules/territory.rs` posts each
    /// as "toll", out of the purse to the post's owner; the mod's `Route` keeps no such sum, `payTolls` pays them as it
    /// goes).
    pub fn tolls_paid(&self, route: RouteId) -> Fixed {
        let toll = Reason::Other("toll".into());
        -self.world.resource::<CashBook>().entries(Account::Wagon(route), 0, u64::MAX).into_iter()
            .filter(|(_, r, _)| *r == toll).map(|(_, _, v)| v).sum::<Fixed>()
    }

    /// The trades a town's managed lockers have made, all told (`LockerTrading.trade`; the study's `trades`): 0 for a
    /// town with no roster.
    pub fn locker_trades(&self, town: TownId) -> u64 {
        self.world.get::<crate::lockers::Lockers>(self.towns[town]).map_or(0, |l| l.trades)
    }
}
