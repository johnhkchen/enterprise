//! Crates and their papers (`cargo.rs`), pinned as the mod's game tests pin them (`CargoTests`, `CaravanTests`,
//! `MarketTests`, `CountingHouseTests`), in milliseconds.

use enterprise_core::cargo::{self, crated, Lading, Lot, Payment, LADING};
use enterprise_core::contracts::Book;
use enterprise_core::{Content, Good, Incoterm, Society};

const DAY: u64 = 24_000;

/// One look on.
fn look(s: &mut Society) {
    let next = s.now() + s.content().dials.look_ticks;
    s.advance(next);
}

/// Looks on until `done` holds (at most `most` looks); whether it did.
fn until(s: &mut Society, most: u32, done: impl Fn(&Society) -> bool) -> bool {
    for _ in 0..most {
        if done(s) {
            return true;
        }
        look(s);
    }
    done(s)
}

fn goods(names: &[&str]) -> Content {
    Content::new(names.iter().map(|n| Good::new(n, 0.05, 64)).collect(), DAY)
}

/// A bill of lading from `shipper` to `consignee`, under `contract`, paid as `payment`, with a copy or not.
fn bill(shipper: usize, consignee: usize, contract: Option<usize>, payment: Payment, copy: bool) -> Lading {
    Lading { contract, payment, copy, ..Lading::new(shipper, consignee, 0) }
}

// parity-partial: CargoTests.aCrateTakesTheSpaceOfWhatsInIt (the core's package is one good, not mixed stacks; the item stack sizes are Minecraft's)
#[test]
fn a_crate_takes_the_space_of_whats_in_it() {
    // (The mod's crate holds three stacks of mixed goods; the core's package is one good, three stacks of it.)
    let content = goods(&["paper", "bread"]);
    let crate_of = |count| Lot { packages: crated(&content, 0, count) };
    assert_eq!(cargo::loose_stacks(&content, &[0.0, 1.0]) + crate_of(129.0).slots(&content), 4,
        "three stacks crated, one loose: four");
    assert_eq!(crate_of(0.0).slots(&content), 1, "an empty crate takes one");
    assert_eq!(crate_of(64.0 * 9.0).crates(), 1, "nine stacks to a crate");
    assert_eq!(crate_of(64.0 * 9.0 + 1.0).crates(), 2);
    assert_eq!(content.dials.empties_kept, 16, "a stack of empties kept");
}

/// A market with stationery and money.
fn desk(paper: f64, ink: f64, money: f64) -> Society {
    let mut s = Society::new(goods(&["paper", "ink_sac", "feather"]));
    s.add_town("Haygate", money);
    s.deliver(0, 0, paper);
    s.deliver(0, 1, ink);
    s
}

// parity: CargoTests.documentsUseStationeryOrAScribe
#[test]
fn documents_use_stationery_or_a_scribe() {
    let mut s = desk(8.0, 1.0, 10.0);
    s.draw_papers(0, 8, LADING);
    look(&mut s);
    assert_eq!(s.paperwork(0).book(), [8.0, 8.0, 1.0, 0.0, 0.0], "eight papers, an ink sac, no scribe");
    assert_eq!(s.town(0).stock[..2], [0.0, 0.0], "used up");
    s.draw_papers(0, 8, LADING);
    look(&mut s);
    let b = s.paperwork(0).book();
    assert!((b[4] - 0.8).abs() < 1e-9, "out of paper: a scribe paid a tenth a document: {b:?}");
    assert!(s.town(0).money == 9.2, "from the treasury: {}", s.town(0).money);
    assert_eq!(s.paperwork(0).by_kind.get(LADING), Some(&16));
}

#[test]
fn with_no_stationery_a_scribe_draws_up_each_document_for_a_tenth() {
    // (The scribe's half of `CountingHouseTests.aCountingHousesClerksDrawUpPapersCheaply`; `tests/works.rs` has it whole.)
    let mut s = desk(0.0, 0.0, 10.0);
    s.draw_papers(0, 10, LADING);
    look(&mut s);
    assert!(s.paperwork(0).scribe_fees == 1.0, "a scribe at 0.1 a document: {}", s.paperwork(0).scribe_fees);
    assert_eq!(cargo::CLERK_EMERALDS, 0.02);
    assert_eq!(cargo::COUNTING_STATIONERY, 0.5);
}

#[test]
fn a_scribe_is_paid_only_what_the_treasury_holds() {
    let mut s = desk(0.0, 0.0, 0.25);
    s.draw_papers(0, 10, LADING);
    look(&mut s);
    assert_eq!(s.town(0).money, 0.0, "it pays what it has");
    assert!(s.paperwork(0).scribe_fees == 1.0, "the fee booked in full, as the mod's");
}

/// A hub and a town a day apart, the hub's wagon going every `every` days; paper and flint and axes.
fn hub_and_town(hub_money: f64, town_money: f64, every: f64) -> Society {
    let mut s = Society::new(goods(&["paper", "flint", "iron_axe"]));
    s.add_town("Haygate", hub_money);
    s.add_town("Cloverby", town_money);
    s.add_route(0, 1, 1.0, 0.0, every);
    s
}

// parity: CaravanTests.contractedGoodsShipInCratesWithTheirPapers
#[test]
fn contracted_goods_ship_in_crates_with_their_papers() {
    let mut s = hub_and_town(5.0, 5.0, 30.0);
    s.deliver(0, 0, 192.0);
    let id = s.draft(0, 1, 0, 64.0, 0.05, 0, DAY, 1, Incoterm::Ddp, true);
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road), "the wagon left");
    let (aboard, lots, _) = s.wagon_pallets(0);
    assert!(aboard.len() == 1 && lots.is_empty(), "the paper rides on a pallet, in a crate: {aboard:?}");
    let crate_ = &aboard[0];
    assert!(crate_.consigned_to(1) && crate_.lading.contract == Some(id) && crate_.count() == 64.0 && crate_.lading.copy,
        "its bill of lading names the town and the contract: {crate_:?}");
    assert_eq!(crate_.crates(), 1, "one crate");
    assert!(crate_.seal.is_some_and(|seal| seal.by == 0), "sealed as it left home");
    assert_eq!(s.paperwork(0).documents, 1, "one bill of lading drawn");
    assert!(s.town(0).money == (5.0 - 0.125), "a new crate bought: {}", s.town(0).money);
    assert!(until(&mut s, 30, |s| s.contract(id).delivered > 0.0));
    assert_eq!(s.contract(id).delivered, 64.0, "opened and delivered under the contract");
    assert!(s.shipments(1).empties >= 1, "the empty crate stays, to ship with");
}

// parity: CaravanTests.whatAPlayerBroughtIsPaidForByTheNextCaravan
#[test]
fn what_a_player_brought_is_paid_for_by_the_next_wagon() {
    let mut s = hub_and_town(5.0, 10.0, 30.0);
    let id = s.draft(0, 1, 1, 16.0, 0.25, 0, DAY, 1, Incoterm::Dap, true);
    // A player carried a crate from the hub and put it in the town's warehouse.
    let packages = crated(s.content(), 1, 16.0);
    assert_eq!(s.deliver_pallet(1, bill(0, 1, Some(id), Payment::Owed, true), packages), None, "received, not held");
    assert!(s.contract(id).delivered == 16.0 && s.town(1).stock[1] == 16.0, "the Merchant opened it and took delivery");
    assert!(s.contract(id).owing == 4.0, "4 emeralds owed to the hub: {}", s.contract(id).owing);
    assert_eq!(s.shipments(1).empties, 1, "and keeps the crate");
    assert!(until(&mut s, 60, |s| s.route(0).trips >= 1));
    let c = s.contract(id);
    assert!(c.owing < 0.01 && c.paid >= 3.9, "the hub's wagon collected it: owing {}, paid {}", c.owing, c.paid);
    assert!(s.town(0).money == 9.0, "home with it: {}", s.town(0).money);
}

#[test]
fn the_buyers_wagon_carries_what_its_town_owes() {
    // The town's own wagon to the hub carries the money, not only the hub's coming for it (`settleOwing`).
    let mut s = Society::new(goods(&["paper", "flint"]));
    s.add_town("Haygate", 5.0);
    s.add_town("Cloverby", 10.0);
    s.add_route(1, 0, 1.0, 0.0, 1.0);
    let id = s.draft(0, 1, 1, 16.0, 0.25, 0, DAY, 1, Incoterm::Dap, true);
    let packages = crated(s.content(), 1, 16.0);
    s.deliver_pallet(1, bill(0, 1, Some(id), Payment::Owed, true), packages);
    assert!(until(&mut s, 60, |s| s.contract(id).owing < 0.01), "paid: {}", s.contract(id).owing);
    assert!(s.town(0).money == 9.0, "the hub has its 4: {}", s.town(0).money);
    assert!(until(&mut s, 60, |s| s.route(0).trips >= 1));
    assert!(s.town(1).money == 6.0, "the town paid 4: {}", s.town(1).money);
}

#[test]
fn a_settled_crate_is_received_without_a_debt() {
    let mut s = hub_and_town(5.0, 10.0, 30.0);
    let id = s.draft(0, 1, 1, 16.0, 0.25, 0, DAY, 1, Incoterm::Dap, true);
    let packages = crated(s.content(), 1, 16.0);
    s.deliver_pallet(1, bill(0, 1, Some(id), Payment::Paid, true), packages);
    assert_eq!(s.town(1).stock[1], 16.0);
    assert_eq!(s.contract(id).owing, 0.0, "paid for when crated");
    assert_eq!(s.contract(id).delivered, 0.0, "settled elsewhere: not booked here");
}

// parity: CaravanTests.aCaravanFetchesHomeCratesMadeOutToIt
#[test]
fn a_wagon_fetches_home_crates_made_out_to_it() {
    let mut s = hub_and_town(5.0, 5.0, 30.0);
    let packages = crated(s.content(), 2, 9.0);
    let id = s.deliver_pallet(1, bill(1, 0, None, Payment::Paid, false), packages).expect("held");
    assert_eq!(s.town_pallets(1).len(), 1, "it waits at the town, crated");
    assert_eq!(s.town(1).stock[2], 0.0, "not the town's stock");
    assert!(until(&mut s, 40, |s| s.wagon_pallets(0).0.iter().any(|p| p.id == id)), "the wagon fetched it, the same pallet");
    assert!(s.wagon_pallets(0).0[0].seal.is_some_and(|seal| seal.by == 1), "sealed as it left the town's");
    assert!(until(&mut s, 60, |s| s.route(0).trips >= 1));
    assert_eq!(s.town(0).stock[2], 9.0, "home and opened");
    assert!(s.town_pallets(1).is_empty() && s.pallets().get(id).is_none(), "opened: out of the registry");
    assert_eq!(s.shipments(0).empties, 1, "the crate kept at home");
}

// parity-partial: CaravanTests.goodsForSaleTravelCratedAndAreStoredAsTheyCome (a crate's slots are Minecraft's: the buyer keeping the crate stands for 'one slot, not two')
#[test]
fn goods_for_sale_travel_crated_and_the_buyer_keeps_the_crate() {
    // (A crate's slots are Minecraft's: "stored as it came, one slot, not two" is here the buyer keeping the crate.)
    let mut s = hub_and_town(5.0, 1000.0, 30.0);
    s.deliver(0, 0, 640.0);
    s.set_trade(1, 0, 0.0, 128.0 / s.content().dials.shelf_days); // its shelf takes 128 paper
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road));
    let (aboard, lots, _) = s.wagon_pallets(0);
    assert!(aboard.is_empty() && lots.iter().any(|c| c.good() == 0 && c.count() == 128.0),
        "the paper goes in a crate of its own, an open lot with no bill: {lots:?}");
    assert_eq!(s.paperwork(0).documents, 0, "goods for sale have no papers");
    assert!(until(&mut s, 30, |s| s.route(0).sold > 0.0));
    assert_eq!(s.town(1).stock[0], 128.0, "sold, all of it");
    assert_eq!(s.shipments(1).empties, 1, "the lot bought whole: its crate stays with the buyer");
    assert!(s.wagon_pallets(0).1.is_empty(), "nothing left crated aboard");
}

#[test]
fn a_lot_partly_sold_rides_on_crated() {
    let mut s = hub_and_town(5.0, 1000.0, 30.0);
    s.deliver(0, 0, 640.0);
    s.set_trade(1, 0, 0.0, 128.0 / s.content().dials.shelf_days);
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road));
    // The town gets paper from elsewhere meanwhile: it buys only what its shelf has room for.
    s.deliver(1, 0, 100.0);
    assert!(until(&mut s, 30, |s| s.route(0).sold > 0.0));
    let sold = s.route(0).sold;
    let (_, lots, _) = s.wagon_pallets(0);
    assert!(sold < 128.0 && lots.len() == 1 && lots[0].count() == 128.0 - sold, "the rest crated aboard: {sold} sold, {lots:?}");
    assert_eq!(s.shipments(1).empties, 0, "the crate stays with the lot");
    assert!(until(&mut s, 60, |s| s.route(0).trips >= 1));
    assert!(s.shipments(0).empties >= 1, "home, its lot back on the shelf, the crate kept");
}

#[test]
fn a_shipper_uses_its_own_empties_first_and_has_spare_ones_back() {
    let mut s = hub_and_town(5.0, 5.0, 30.0);
    s.deliver(0, 0, 640.0);
    // The hub holds an empty crate; the town more than the stack it keeps.
    let empty = crated(s.content(), 0, 0.0);
    s.deliver_pallet(0, bill(1, 0, None, Payment::Owed, false), empty.clone());
    for _ in 0..20 {
        s.deliver_pallet(1, bill(0, 1, None, Payment::Owed, false), empty.clone());
    }
    assert_eq!((s.shipments(0).empties, s.shipments(1).empties), (1, 20));
    let id = s.draft(0, 1, 0, 64.0, 0.05, 0, DAY, 1, Incoterm::Ddp, true);
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road));
    assert_eq!(s.shipments(0).empties, 0, "its own crate used");
    assert!(s.town(0).money == 5.0, "nothing paid for it: {}", s.town(0).money);
    assert!(until(&mut s, 30, |s| s.contract(id).delivered > 0.0));
    let (_, _, empties) = s.wagon_pallets(0);
    assert_eq!(empties, 4, "the town keeps sixteen of its 20 and hands the rest back, before the crate is opened");
    assert_eq!(s.shipments(1).empties, 17, "then the opened crate's kept too");
    assert!(until(&mut s, 60, |s| s.route(0).trips >= 1));
    assert_eq!(s.shipments(0).empties, 4, "returnable crates home");
}

#[test]
fn with_no_crate_and_no_money_goods_go_loose_without_papers_crated() {
    let mut s = hub_and_town(0.0, 5.0, 30.0);
    s.deliver(0, 0, 192.0);
    let id = s.draft(0, 1, 0, 64.0, 0.05, 0, DAY, 1, Incoterm::Ddp, true);
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road));
    assert!(s.wagon_pallets(0).0.is_empty(), "no crate to be had: it goes loose");
    assert!(until(&mut s, 30, |s| s.contract(id).delivered > 0.0));
    assert_eq!(s.contract(id).delivered, 64.0, "and is delivered all the same");
}

/// A farm owing a far buyer 40 of a crop every ten days, three times, judged four days after each day.
fn farm(crop: &str, held: f64) -> (Society, usize) {
    let mut s = Society::new(goods(&[crop]));
    s.add_town("Cloverby", 10.0);
    s.add_town("Far Market", 0.0);
    s.deliver(0, 0, held);
    let id = s.draft_harvest(0, 1, 0, 40.0, 0.04, 0, 10 * DAY, 3, Incoterm::Ddp, true, false);
    assert_eq!(s.content().dials.harvest_window_days, 4);
    (s, id)
}

// parity: MarketTests.aFarmCratesOwedCropInLotsNotAsItTrickles
#[test]
fn a_farm_crates_owed_crop_in_lots_not_as_it_trickles() {
    let (mut s, id) = farm("carrot", 10.0);
    for _ in 0..20 {
        look(&mut s); // touched as the harvest trickles in
    }
    assert!(s.paperwork(0).documents == 0 && s.earmarked(0, id) == 0.0, "10 of 40 owed: not yet a lot, no crate and no papers");
    s.deliver(0, 0, 30.0);
    look(&mut s);
    assert_eq!(s.earmarked(0, id), 40.0, "all 40 in: crated at once");
    assert_eq!(s.paperwork(0).documents, 1, "one bill of lading");
    let crate_ = &s.town_pallets(0)[0];
    assert!(crate_.consigned_to(1) && crate_.lading.contract == Some(id) && crate_.lading.copy && crate_.seal.is_none(),
        "made out to the buyer, open at the farm: {crate_:?}");
}

// parity-partial: MarketTests.aFarmWithNoRoomForCratesDrawsNoPapers (the room is the dial crate_room, not the chests' free slots (Minecraft's))
#[test]
fn a_farm_with_no_room_for_crates_draws_no_papers() {
    // (The mod's room is its chests' free slots; the core's is the dial `crate_room`.)
    let (mut s, _) = farm("potato", 64.0);
    s.set_dial("crate_room", 0.0).unwrap();
    for _ in 0..20 {
        look(&mut s); // touched again and again
    }
    assert_eq!(s.paperwork(0).documents, 0, "no room for a crate, no bill of lading drawn");
    assert_eq!(s.town(0).stock[0], 64.0, "the potatoes still in stock");
    assert!((s.set_aside_beyond_crates(0, 0) - 40.0).abs() < 1e-9, "still set aside");
}

// parity: MarketTests.aFarmCratesWhatItOwesOutOfReachUntilItShips
#[test]
fn a_farm_crates_what_it_owes_out_of_reach_until_it_ships() {
    let (mut s, id) = farm("potato", 64.0);
    look(&mut s);
    assert!(s.earmarked(0, id) == 40.0 && s.town(0).stock[0] == 24.0, "the 40 owed crated, out of reach: {} crated, {} loose",
        s.earmarked(0, id), s.town(0).stock[0]);
    assert_eq!(s.set_aside_beyond_crates(0, 0), 0.0, "crated, nothing more set aside");
    assert_eq!(s.set_aside_for_harvest(0, 0), 0.0);
    s.world.resource_mut::<Book>().close(id);
    look(&mut s);
    assert_eq!(s.town(0).stock[0], 64.0, "its contract closed, back on the shelf");
    assert!(s.town_pallets(0).is_empty() && s.shipments(0).empties == 1, "the crate kept empty");
}

#[test]
fn a_farms_crates_ride_first_to_its_buyer() {
    let (mut s, id) = farm("potato", 64.0);
    look(&mut s);
    assert_eq!(s.earmarked(0, id), 40.0);
    s.add_route(0, 1, 1.0, 0.0, 30.0);
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road));
    let (aboard, ..) = s.wagon_pallets(0);
    assert!(aboard.len() == 1 && aboard[0].count() == 40.0 && aboard[0].lading.contract == Some(id), "the crate as it was: {aboard:?}");
    assert_eq!(s.town(0).stock[0], 24.0, "nothing loose went with it");
    assert_eq!(s.paperwork(0).documents, 1, "no second bill of lading");
    assert!(until(&mut s, 40, |s| s.contract(id).delivered > 0.0));
    assert_eq!(s.contract(id).delivered, 40.0, "delivered from its crate");
}

#[test]
fn ex_works_the_buyer_takes_what_the_seller_crated_for_it_first() {
    let mut s = Society::new(goods(&["potato"]));
    s.add_town("Cloverby", 10.0);
    s.add_town("Haygate", 100.0);
    s.deliver(0, 0, 64.0);
    let id = s.draft_harvest(0, 1, 0, 40.0, 0.04, 0, 10 * DAY, 3, Incoterm::Exw, true, false);
    look(&mut s);
    assert_eq!(s.earmarked(0, id), 40.0);
    s.add_route(1, 0, 1.0, 0.0, 30.0);
    assert!(until(&mut s, 40, |s| s.contract(id).delivered > 0.0));
    assert_eq!(s.contract(id).delivered, 40.0, "collected");
    assert_eq!(s.town(0).stock[0], 24.0, "from the crate, the loose ones left");
    let (aboard, ..) = s.wagon_pallets(0);
    assert!(aboard.len() == 1 && aboard[0].consigned_to(1) && aboard[0].lading.payment == Payment::Paid,
        "crated again, made out home, paid for: {aboard:?}");
    assert_eq!(s.paperwork(0).documents, 2, "a bill for the earmark, one for the collection");
    assert!(until(&mut s, 40, |s| s.route(0).trips >= 1));
    assert_eq!(s.town(1).stock[0], 40.0, "home and stored");
    assert_eq!(s.contract(id).owing, 0.0, "paid for: no debt booked at home");
}
