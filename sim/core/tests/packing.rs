//! Packing (`packing`, `docs/pallets.md` sections 1 "One contract to a bill" and "Handling", 2 "Earmarks" and "Ex works"):
//! packages of each good's own kind (crate, barrel, bundle, or loose) on a pallet by the grouping key; owed goods that can't
//! be crated ride loose on their pallet under its bill, out of the wagon's cargo; a farm's earmark tops up its open pallet to
//! its kind's caps, a bill for the lots that fit; an Ex Works collection takes the seller's whole pallets and re-pallets a
//! part one at the seller's with a new bill.

use enterprise_core::cargo::{self, How, Lading, LADING};
use enterprise_core::pallet::{At, Handling, Payment, BARREL, BUNDLE, CRATE, LOOSE};
use enterprise_core::{Content, Good, Incoterm, Society};

const DAY: u64 = 24_000;

fn look(s: &mut Society) {
    let next = s.now() + s.content().dials.look_ticks;
    s.advance(next);
}

fn until(s: &mut Society, most: u32, done: impl Fn(&Society) -> bool) -> bool {
    for _ in 0..most {
        if done(s) {
            return true;
        }
        look(s);
    }
    done(s)
}

/// Goods of 64 to a stack, each packed as `kinds` say (crate, barrel, bundle, loose).
fn goods(names: &[(&str, usize)]) -> Content {
    let mut content = Content::new(names.iter().map(|(n, _)| Good::new(n, 0.05, 64)).collect(), DAY);
    for (g, (_, kind)) in names.iter().enumerate() {
        content.goods[g].package = *kind;
    }
    content
}

fn bill(shipper: usize, consignee: usize, contract: Option<usize>) -> Lading {
    Lading { contract, payment: Payment::Owed, copy: contract.is_some(), ..Lading::new(shipper, consignee, 0) }
}

/// A town with money, crates and goods to pack.
fn shelf(content: Content, money: f64, empties: u32) -> Society {
    let mut s = Society::new(content);
    s.add_town("Haygate", money);
    s.add_town("Cloverby", money);
    s.set_dial("empties_kept", 1000.0).unwrap();
    for _ in 0..empties {
        // (An empty crate comes to a town as a pallet of nothing made out to it.)
        s.deliver_pallet(0, bill(1, 0, None), cargo::crated(s.content(), 0, 0.0));
    }
    s
}

// ---- a pallet by the grouping key, packages of each good's own kind

#[test]
fn a_barrel_good_and_a_crate_good_for_one_buyer_share_a_pallet() {
    let mut s = shelf(goods(&[("ale", BARREL), ("paper", CRATE)]), 5.0, 4);
    s.deliver(0, 0, 100.0);
    s.deliver(0, 1, 64.0);
    let before = s.totals();
    let ids = s.pack_pallets(0, bill(0, 1, None), &[(0, 100.0), (1, 64.0)], How::OWED);
    assert_eq!(ids.len(), 1, "one buyer, one bill: one pallet");
    let p = s.pallets().get(ids[0]).unwrap();
    assert_eq!(p.packages.iter().map(|pk| (pk.kind, pk.good, pk.count)).collect::<Vec<_>>(),
        vec![(BARREL, 0, 100.0), (CRATE, 1, 64.0)], "a barrel of ale, a crate of paper, on one pallet");
    assert_eq!(p.goods(), vec![(0, 100.0), (1, 64.0)]);
    assert!(p.seal.is_none() && p.lading.consignee == 1 && p.at == At::Town(0));
    assert_eq!(s.paperwork(0).by_kind.get(LADING), Some(&1), "one bill of lading for it");
    assert_eq!(s.shipments(0).empties, 2, "two of the town's four empty crates used");
    assert_eq!((s.town(0).stock[0], s.town(0).stock[1]), (0.0, 0.0), "out of the stock");
    s.conserved(&before).unwrap();
}

#[test]
fn a_host_packs_only_its_own_goods_and_under_a_contract_only_the_contracts_good() {
    let mut s = Society::new(goods(&[("carrot", CRATE), ("paper", CRATE)]));
    s.add_town("Cloverby", 10.0);
    s.add_town("Far Market", 0.0);
    s.deliver(0, 0, 64.0);
    s.deliver(0, 1, 64.0);
    let before = s.totals();
    let c = s.draft_harvest(0, 1, 0, 64.0, 0.04, 0, 10 * DAY, 3, Incoterm::Ddp, true, false);
    assert!(s.pack_pallets(1, bill(0, 1, None), &[(0, 64.0)], How::OWED).is_empty(), "not the shipper: refused");
    assert!(s.pack_pallets(0, bill(0, 1, Some(c)), &[(0, 64.0), (1, 64.0)], How::OWED).is_empty(),
        "another good under the contract: refused");
    assert_eq!((s.town(0).stock[0], s.town(0).stock[1]), (64.0, 64.0), "nothing left the stock");
    assert_eq!(s.pack_pallets(0, bill(0, 1, Some(c)), &[(0, 64.0)], How::OWED).len(), 1, "the contract's good: packed");
    s.conserved(&before).unwrap();
}

#[test]
fn goods_of_a_key_fill_a_pallet_and_the_next_begins_a_new_one() {
    // Nine stacks to a pallet: nine of ale fill one; the paper begins another.
    let mut s = shelf(goods(&[("ale", BARREL), ("paper", CRATE)]), 5.0, 0);
    s.deliver(0, 0, 9.0 * 64.0);
    s.deliver(0, 1, 64.0);
    let ids = s.pack_pallets(0, bill(0, 1, None), &[(0, 9.0 * 64.0), (1, 64.0)], How::OWED);
    assert_eq!(ids.len(), 2);
    let counts: Vec<Vec<(usize, f64)>> = ids.iter().map(|&id| s.pallets().get(id).unwrap().goods()).collect();
    assert_eq!(counts, vec![vec![(0, 576.0)], vec![(1, 64.0)]]);
    assert_eq!(s.paperwork(0).by_kind.get(LADING), Some(&2), "a bill each");
    assert!(s.town(0).money == 5.0 - 2.0 * 0.125, "two new crates bought: {}", s.town(0).money);
    // Three stacks of each fit one: a part-full pallet takes the next good as far as its room goes.
    let mut s = shelf(goods(&[("ale", BARREL), ("paper", CRATE)]), 5.0, 0);
    s.deliver(0, 0, 3.0 * 64.0);
    s.deliver(0, 1, 8.0 * 64.0);
    let ids = s.pack_pallets(0, bill(0, 1, None), &[(0, 3.0 * 64.0), (1, 8.0 * 64.0)], How::OWED);
    let counts: Vec<Vec<(usize, f64)>> = ids.iter().map(|&id| s.pallets().get(id).unwrap().goods()).collect();
    assert_eq!(counts, vec![vec![(0, 192.0), (1, 384.0)], vec![(1, 128.0)]], "six stacks of paper beside the ale, two more behind");
}

#[test]
fn goods_that_need_care_never_share_a_pallet_with_goods_that_dont() {
    let mut c = goods(&[("milk", CRATE), ("paper", CRATE)]);
    c.goods[0].handling = Handling { cool: true, fragile: false };
    let mut s = shelf(c, 5.0, 4);
    s.deliver(0, 0, 64.0);
    s.deliver(0, 1, 64.0);
    let ids = s.pack_pallets(0, bill(0, 1, None), &[(0, 64.0), (1, 64.0)], How::OWED);
    assert_eq!(ids.len(), 2, "keep cool is part of the key");
    let milk = s.pallets().get(ids[0]).unwrap();
    assert!(milk.lading.handling.cool && milk.good() == 0, "{milk:?}");
    let paper = s.pallets().get(ids[1]).unwrap();
    assert!(!paper.lading.handling.cool && paper.good() == 1, "{paper:?}");
    assert_ne!(milk.key(), paper.key());
}

#[test]
fn packages_of_a_goods_own_kind_a_bundle_costs_no_crate_and_comes_back_no_empty() {
    let mut s = shelf(goods(&[("arrows", BUNDLE)]), 0.0, 0);
    s.deliver(0, 0, 3.0 * 64.0);
    let ids = s.pack_pallets(0, bill(0, 1, None), &[(0, 3.0 * 64.0)], How::CRATED);
    assert_eq!(ids.len(), 1, "bundled with no crate and no money");
    let p = s.pallets().get(ids[0]).unwrap();
    assert!(p.packages.len() == 1 && p.packages[0].kind == BUNDLE, "{p:?}");
    assert_eq!(p.empties(s.content()), 0, "a bundle is cut and gone");
    assert!(s.town(0).money == 0.0, "and costs nothing: {}", s.town(0).money);
    // Opened at its consignee's it leaves no empty behind.
    s.deliver_pallet(1, bill(0, 1, None), p.packages.clone());
    assert_eq!(s.shipments(1).empties, 0);
}

#[test]
fn a_good_that_isnt_packed_lies_loose_on_its_pallet_with_no_crate() {
    let mut s = shelf(goods(&[("gravel", LOOSE)]), 5.0, 0);
    s.deliver(0, 0, 11.0 * 64.0);
    let ids = s.pack_pallets(0, bill(0, 1, None), &[(0, 11.0 * 64.0)], How::CRATED);
    assert_eq!(ids.len(), 2, "nine stacks and two");
    for (&id, count) in ids.iter().zip([576.0, 128.0]) {
        let p = s.pallets().get(id).unwrap();
        assert!(p.packages.len() == 1 && p.packages[0].kind == LOOSE && p.count() == count && p.crates() == 0, "{p:?}");
    }
    assert!(s.town(0).money == 5.0, "no crate bought: {}", s.town(0).money);
    assert_eq!(s.paperwork(0).by_kind.get(LADING), Some(&2), "a bill each");
}

// ---- owed goods that can't be crated ride loose on their pallet

/// Haygate (with `money`) and Cloverby a day apart, the wagon every thirty days.
fn hub_and_town(money: f64) -> Society {
    let mut s = Society::new(goods(&[("paper", CRATE), ("flint", CRATE)]));
    s.add_town("Haygate", money);
    s.add_town("Cloverby", 5.0);
    s.add_route(0, 1, 1.0, 0.0, 30.0);
    s
}

// packing: the mod's wagon carries these loose in its cargo with no papers; the core's ride on pallets under bills
#[test]
fn a_shipper_with_no_crates_and_no_money_sends_its_owed_goods_on_pallets_a_bill_each() {
    let mut s = hub_and_town(0.0);
    s.deliver(0, 0, 20.0 * 64.0);
    let id = s.draft(0, 1, 0, 20.0 * 64.0, 0.05, 0, DAY, 1, Incoterm::Ddp, true);
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road));
    let (aboard, lots, _) = s.wagon_pallets(0);
    assert!(lots.is_empty());
    assert_eq!(aboard.iter().map(|p| p.count()).collect::<Vec<_>>(), vec![576.0, 576.0, 128.0], "three pallets, nine stacks each");
    assert!(aboard.iter().all(|p| p.crates() == 0 && p.packages[0].kind == LOOSE && p.lading.contract == Some(id)
        && p.lading.copy && p.seal.is_some()), "loose on their pallets, each under its own bill: {aboard:?}");
    assert_eq!(s.paperwork(0).by_kind.get(LADING), Some(&3), "three bills");
    assert_eq!(s.wagon_cargo(0).unwrap()[0], 0.0, "none left in the cargo");
    assert!(until(&mut s, 30, |s| s.contract(id).delivered > 0.0));
    assert_eq!(s.contract(id).delivered, 20.0 * 64.0, "delivered, from the pallets the far end opened");
}

#[test]
fn what_the_empties_cover_goes_crated_and_the_rest_loose_on_pallets_after_it() {
    // One empty crate and no money: the first pallet crated, the others loose.
    let mut s = shelf(goods(&[("paper", CRATE)]), 0.0, 1);
    s.deliver(0, 0, 20.0 * 64.0);
    let before = s.totals();
    let ids = s.pack_pallets(0, bill(0, 1, None), &[(0, 20.0 * 64.0)], How::OWED);
    let kinds: Vec<Vec<usize>> = ids.iter().map(|&id| s.pallets().get(id).unwrap().packages.iter().map(|pk| pk.kind).collect()).collect();
    assert_eq!(kinds, vec![vec![CRATE], vec![LOOSE], vec![LOOSE]]);
    assert_eq!(s.paperwork(0).by_kind.get(LADING), Some(&3), "a bill each");
    assert_eq!(s.town(0).stock[0], 0.0, "all of it on pallets");
    s.conserved(&before).unwrap();
    // Only what can be crated goes when asked (a forwarder's goods, the offtake): the rest stays in stock.
    let mut s = shelf(goods(&[("paper", CRATE)]), 0.0, 1);
    s.deliver(0, 0, 20.0 * 64.0);
    let ids = s.pack_pallets(0, bill(0, 1, None), &[(0, 20.0 * 64.0)], How::CRATED);
    assert_eq!(ids.len(), 1);
    assert_eq!(s.town(0).stock[0], 20.0 * 64.0 - 576.0, "the rest stays in stock");
    assert_eq!(s.paperwork(0).by_kind.get(LADING), Some(&1));
}

// A porter run carries one pallet; the plan counts the goods owed as the pallets they'll go on, loose or crated.
#[test]
fn the_plan_counts_loose_pallets_as_it_counts_crated_ones() {
    const LOOK: u64 = 1_200;
    let goes_owed = |crate_emeralds: f64, owe: &dyn Fn(&mut Society, u64)| -> Society {
        let content = Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0)], DAY);
        let mut s = Society::new(content);
        s.content_mut().dials.standing_orders = false;
        s.set_dial("crate_emeralds", crate_emeralds).unwrap();
        let hub = s.add_town("Hub", 5.0);
        let dest = s.add_town("Dest", 12.0);
        s.record_capacity(dest, 1, &[(0, 64.0 / 5.0)]);
        s.set_trade(dest, 0, 0.0, 1e-9);
        s.deliver(hub, 0, 128.0);
        s.deliver(hub, 1, 8.0);
        s.deliver(dest, 1, 2.0);
        s.set_roster(hub, &[("merchant", 1), ("librarian", 2)]);
        s.set_offers(hub, 1, Vec::new());
        s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
        s.advance(LOOK);
        s.advance(4 * LOOK);
        let t = s.now();
        owe(&mut s, t);
        for look in 1..=(4 * DAY / LOOK) {
            s.advance(t + 2 * DAY + look * LOOK);
            if s.route(0).on_the_road {
                return s;
            }
        }
        panic!("it never went");
    };
    // No crate is to be had (a hundred emeralds each): half a stack owed is one loose pallet, on a porter's back; two
    // contracts owed are two pallets, more than a porter carries, so the wagon goes.
    let one = goes_owed(100.0, &|s, t| {
        s.draft(0, 1, 0, 32.0, 0.5, t + 2 * DAY, DAY, 1, Incoterm::Dap, true);
    });
    let (aboard, ..) = one.wagon_pallets(0);
    assert!(one.crew(0).porter && aboard.len() == 1 && aboard[0].packages[0].kind == LOOSE, "{:?} {aboard:?}", one.crew(0));
    let two = goes_owed(100.0, &|s, t| {
        s.draft(0, 1, 0, 16.0, 0.5, t + 2 * DAY, DAY, 1, Incoterm::Dap, true);
        s.draft(0, 1, 0, 16.0, 0.5, t + 2 * DAY, DAY, 1, Incoterm::Dap, true);
    });
    let (aboard, ..) = two.wagon_pallets(0);
    assert!(!two.crew(0).porter && aboard.len() == 2 && aboard.iter().all(|p| p.packages[0].kind == LOOSE), "{aboard:?}");
    assert_eq!(two.wagon_cargo(0).unwrap()[0], 0.0);
}

// ---- a farm's earmark tops up its open pallet

/// A farm owing a far buyer `each` of a crop every ten days, three times; no wagon yet.
fn farm(each: f64, held: f64) -> (Society, usize) {
    let mut s = Society::new(goods(&[("carrot", CRATE)]));
    s.add_town("Cloverby", 10.0);
    s.add_town("Far Market", 0.0);
    s.deliver(0, 0, held);
    let id = s.draft_harvest(0, 1, 0, each, 0.04, 0, 10 * DAY, 3, Incoterm::Ddp, true, false);
    (s, id)
}

#[test]
fn a_farms_second_lot_tops_up_its_open_pallet_one_bill_and_once_full_a_new_one() {
    let (mut s, id) = farm(400.0, 2000.0);
    look(&mut s);
    assert_eq!(s.earmarked(0, id), 400.0, "the first installment crated at once");
    let first = s.town_pallets(0)[0].clone();
    assert_eq!(s.paperwork(0).documents, 1);
    // The second installment falls due within the window six days on: its lot tops up the pallet (576 a pallet).
    assert!(until(&mut s, 12 * 20, |s| s.earmarked(0, id) >= 800.0), "the second lot");
    let held = s.town_pallets(0);
    assert_eq!(held.iter().map(|p| p.count()).collect::<Vec<_>>(), vec![576.0, 224.0], "topped up to its caps, then a new pallet");
    assert_eq!(held[0].id, first.id, "the same pallet, one bill");
    assert_eq!(held[0].lading.issued, first.lading.issued);
    assert!(held[0].seal.is_none() && held[1].seal.is_none(), "open at the farm");
    assert_eq!(s.paperwork(0).documents, 2, "two bills for 800 (it was 1 + 1: not 2 + 1)");
    // The third tops up the second pallet.
    assert!(until(&mut s, 12 * 20, |s| s.earmarked(0, id) >= 1200.0), "the third lot");
    let held = s.town_pallets(0);
    assert_eq!(held.iter().map(|p| p.count()).collect::<Vec<_>>(), vec![576.0, 576.0, 48.0]);
    assert_eq!(s.paperwork(0).documents, 3);
    // The crates: a part-full crate is filled before a new one is begun.
    assert_eq!(held.iter().map(|p| p.packages.len()).collect::<Vec<_>>(), vec![1, 1, 1], "a crate a pallet");
}

#[test]
fn a_farms_lot_that_fills_the_part_crate_needs_no_new_crate() {
    let (mut s, id) = farm(64.0, 2000.0);
    s.set_dial("empties_kept", 0.0).unwrap();
    look(&mut s);
    assert_eq!(s.earmarked(0, id), 64.0);
    assert!(until(&mut s, 12 * 20, |s| s.earmarked(0, id) >= 128.0));
    let held = s.town_pallets(0);
    assert_eq!(held.len(), 1, "one pallet");
    assert_eq!(held[0].packages.iter().map(|p| p.count).collect::<Vec<_>>(), vec![128.0], "the same crate, fuller");
    let spent: f64 = s.cash_book(0, 400).iter().filter(|(_, why, _)| why.contains("crates")).map(|(_, _, n)| n.to_f64()).sum();
    assert!((spent.abs() - 0.125).abs() < 1e-9, "one crate bought, not two: {spent}");
    assert_eq!(s.paperwork(0).documents, 1);
}

#[test]
fn a_pallet_that_has_left_is_not_topped_up() {
    let (mut s, id) = farm(400.0, 2000.0);
    s.add_route(0, 1, 1.0, 0.0, 30.0);
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road));
    let (aboard, ..) = s.wagon_pallets(0);
    assert!(aboard.len() == 1 && aboard[0].seal.is_some() && aboard[0].count() == 400.0, "{aboard:?}");
    // Later lots are new pallets: the sealed one on the road is never opened to add to.
    assert!(until(&mut s, 12 * 20, |s| s.earmarked(0, id) > 0.0));
    let (aboard, ..) = s.wagon_pallets(0);
    assert!(aboard.iter().all(|p| p.count() == 400.0), "unchanged: {aboard:?}");
}

// ---- Ex Works: whole pallets taken, a part pallet re-palleted

#[test]
fn an_ex_works_collection_opens_a_part_pallet_and_re_pallets_it_with_a_new_bill() {
    let mut s = Society::new(goods(&[("carrot", CRATE)]));
    s.add_town("Cloverby", 10.0);
    s.add_town("Haygate", 100.0);
    s.deliver(0, 0, 3000.0);
    let id = s.draft_harvest(0, 1, 0, 400.0, 0.04, 0, 10 * DAY, 3, Incoterm::Exw, true, false);
    assert!(until(&mut s, 12 * 20, |s| s.earmarked(0, id) >= 800.0), "two installments earmarked on pallets of 576 and 224");
    let held = s.town_pallets(0);
    assert_eq!(held.iter().map(|p| (p.id, p.count())).collect::<Vec<_>>(), vec![(0, 576.0), (1, 224.0)]);
    assert_eq!(s.paperwork(0).documents, 2);
    s.add_route(1, 0, 1.0, 0.0, 30.0);
    let totals = s.totals();
    assert!(until(&mut s, 40, |s| s.contract(id).delivered > 0.0), "collected");
    assert_eq!(s.contract(id).delivered, 400.0);
    s.conserved(&totals).unwrap();
    let (aboard, ..) = s.wagon_pallets(0);
    assert_eq!(aboard.iter().map(|p| (p.id, p.count(), p.lading.payment)).collect::<Vec<_>>(),
        vec![(1, 224.0, Payment::Paid), (2, 176.0, Payment::Paid)],
        "the 224 pallet taken whole (its own bill, now paid), 176 of the 576 packed new for the buyer");
    assert!(aboard.iter().all(|p| p.consigned_to(1) && p.seal.is_some_and(|seal| seal.by == 0)), "sealed as they left the seller");
    let left = s.town_pallets(0);
    assert_eq!(left.iter().map(|p| (p.count(), p.lading.payment)).collect::<Vec<_>>(), vec![(400.0, Payment::Owed)],
        "the rest of the part pallet re-palleted for the contract");
    assert!(left[0].id > 2 && left[0].seal.is_none() && left[0].consigned_to(1), "a new pallet, open at the seller's: {left:?}");
    assert_eq!(s.paperwork(0).documents, 4, "two more bills: the 176 for the buyer, the 400 re-palleted (the 224 kept its own)");
    assert!(until(&mut s, 40, |s| s.route(0).trips >= 1));
    assert_eq!(s.town(1).stock[0], 400.0, "home and stored");
    assert_eq!(s.contract(id).owing, 0.0, "paid for at the seller's: no debt booked at home");
}

#[test]
fn an_ex_works_collection_of_what_the_seller_has_loose_packs_it_loose_when_it_cant_crate() {
    let mut s = Society::new(goods(&[("carrot", CRATE)]));
    s.add_town("Cloverby", 0.0);
    s.add_town("Haygate", 100.0);
    s.deliver(0, 0, 3000.0);
    let id = s.draft_harvest(0, 1, 0, 300.0, 0.04, 0, 10 * DAY, 3, Incoterm::Exw, true, false);
    s.set_dial("crate_room", 0.0).unwrap(); // the seller never earmarks: nothing is crated for the buyer ahead
    s.add_route(1, 0, 1.0, 0.0, 30.0);
    let totals = s.totals();
    assert!(until(&mut s, 40, |s| s.contract(id).delivered > 0.0), "collected");
    s.conserved(&totals).unwrap();
    assert_eq!(s.contract(id).delivered, 300.0);
    let (aboard, ..) = s.wagon_pallets(0);
    assert_eq!(aboard.iter().map(|p| (p.count(), p.lading.payment)).collect::<Vec<_>>(), vec![(300.0, Payment::Paid)]);
    assert!(aboard[0].packages.iter().all(|p| p.kind == LOOSE), "a seller with no money for crates: loose on its pallet: {:?}", aboard[0]);
    assert_eq!(s.wagon_cargo(0).unwrap()[0], 0.0, "none of it loose in the wagon's cargo");
    assert!(until(&mut s, 40, |s| s.route(0).trips >= 1));
    assert_eq!(s.town(1).stock[0], 300.0);
}
