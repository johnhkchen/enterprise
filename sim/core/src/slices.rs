//! Every slice's lines in the lists the slices share: what the save keeps (`saved!`, see `save.rs`) and the systems
//! that join the look (`register`, see `rules::look_schedule`).
//!
//! **Add lines only, one per thing.** Git merges this file by union (`.gitattributes`), so two slices that each add a
//! line never conflict here; a line changed or taken out would come back beside its new self. Changes to lines already
//! here are the integrator's.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::bills::Bills;
use crate::content::Content;
use crate::contracts::Book;
use crate::state::*;

crate::saved! {
    resources {
        content: Content,
        clock: Clock,
        ledger: Ledger,
        outbox: Outbox,
        book: Book = Book::default(),
        bills: Bills = Bills::default(),
        mint: crate::mint::Mint = crate::mint::Mint::default(),
        map: crate::map::Map = crate::map::Map::default(),
        cash_book: crate::cash_book::CashBook = crate::cash_book::CashBook::default(),
        territory: crate::territory::Territory = crate::territory::Territory::default(),
        settlers: crate::settlers::Settlers = crate::settlers::Settlers::default(),
        rng: crate::state::Rng = crate::state::Rng::default(),
        rents: crate::harvest::Rents = crate::harvest::Rents::default(),
        signals: crate::jobs::Signals = crate::jobs::Signals::default(),
        hiring: crate::jobs::Hiring = crate::jobs::Hiring::default(),
        takings: crate::chokepoints::Takings = crate::chokepoints::Takings::default(),
        tenders: crate::training::Tenders = crate::training::Tenders::default(),
        apprenticeships: crate::training::Apprenticeships = crate::training::Apprenticeships::default(),
        squads: crate::squad::Squads = crate::squad::Squads::default(),
        banks: crate::bank::Banks = crate::bank::Banks::default(),
        crown: crate::realms::Crown = crate::realms::Crown::default(),
        atlas: crate::atlas::Atlas = crate::atlas::Atlas::default(),
        stocktakes: crate::atlas::Stocktakes = crate::atlas::Stocktakes::default(),
        books: crate::atlas::Books = crate::atlas::Books::default(),
        whys: crate::atlas::Whys = crate::atlas::Whys::default(),
        listings: crate::desires::Listings = crate::desires::Listings::default(),
        service_ledger: crate::services::ServiceLedger = crate::services::ServiceLedger::default(),
        freight: crate::freight::Freight = crate::freight::Freight::default(),
        sourcing: crate::sourcing::Sourcing = crate::sourcing::Sourcing::default(),
        exchange: crate::sourcing::MetalExchange = crate::sourcing::MetalExchange::default(),
        mail: crate::post::Mail = crate::post::Mail::default(),
        forwarder: crate::post::Forwarder = crate::post::Forwarder::default(),
        army: crate::army::Army = crate::army::Army::default(),
        orders: crate::orders::Orders = crate::orders::Orders::default(),
        posted: crate::orders::Signals = crate::orders::Signals::default(),
        sovereign: crate::crown::Sovereign = crate::crown::Sovereign::default(),
        pallets: crate::pallet::Pallets = crate::pallet::Pallets::default(),
        points: crate::points::Points = crate::points::Points::default(),
        hands: crate::roster::Hands = crate::roster::Hands::default(),
        sightings: crate::census::Sightings = crate::census::Sightings::default(),
        former_desks: crate::chests::FormerDesks = crate::chests::FormerDesks::default(),
        tills: crate::till::Tills = crate::till::Tills::default(),
        land_book: crate::land::LandBook = crate::land::LandBook::default(),
        stowage: crate::pallet_host::Stowage = crate::pallet_host::Stowage::default(),
        vendors: crate::shop_host::Vendors = crate::shop_host::Vendors::default(),
        crown_shop: crate::crown_host::CrownShop = crate::crown_host::CrownShop::default(),
    }
    town {
        town: Town,
        money: Money,
        stock: Stock,
        trades: Trades,
        touched: Touched,
        budget: Budget,
        people: People,
        roster: crate::roster::Roster,
        locker_capacity: crate::bargain::LockerCapacity,
        workshop: crate::workshop::Workshop,
        farm: crate::farms::Farm,
        deposits: crate::farms::Deposits,
        herds: crate::herds::Herds,
        lockers: crate::lockers::Lockers,
        shipments: crate::cargo::Shipments,
        paperwork: crate::cargo::Paperwork,
        works: crate::works::Works,
        off_map: crate::realms::OffMap,
        realm: crate::realms::Realm,
        hub: crate::freight::Hub,
        kits: crate::kit::Kits,
    }
    route {
        route: Route,
        state: RouteState,
        path: RoutePath,
    }
}

/// Every module's systems, each joining its steps of the look.
pub fn register(schedule: &mut Schedule) {
    crate::rules::touch::register(schedule);
    crate::people::register(schedule);
    crate::rules::contracts::register(schedule);
    crate::rules::bills::register(schedule);
    crate::rules::advances::register(schedule);
    crate::rules::arrive::register(schedule);
    crate::rules::plan::register(schedule);
    crate::rules::roster::register(schedule);
    crate::rules::territory::register(schedule);
    crate::rules::workshop::register(schedule);
    crate::rules::settlers::register(schedule);
    crate::rules::farms::register(schedule);
    crate::rules::herds::register(schedule);
    crate::rules::lockers::register(schedule);
    crate::rules::road::register(schedule);
    crate::rules::harvest::register(schedule);
    crate::rules::jobs::register(schedule);
    crate::rules::chokepoints::register(schedule);
    crate::rules::cargo::register(schedule);
    crate::rules::works::register(schedule);
    crate::rules::training::register(schedule);
    crate::rules::squad::register(schedule);
    crate::rules::bank::register(schedule);
    crate::rules::realms::register(schedule);
    crate::rules::atlas::register(schedule);
    crate::rules::desires::register(schedule);
    crate::rules::services::register(schedule);
    crate::rules::freight::register(schedule);
    crate::rules::sourcing::register(schedule);
    crate::rules::kit::register(schedule);
    crate::rules::post::register(schedule);
    crate::rules::army::register(schedule);
    crate::rules::orders::register(schedule);
    crate::rules::stranded::register(schedule);
    crate::rules::crown::register(schedule);
    crate::rules::pallet_messages::register(schedule);
}
