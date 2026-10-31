//! Crops as data (`docs/markets-plan.md` 5): the crops a field can grow come with the host's pack, one a file; a pack
//! with none has the mod's six; a crop added by file is tended and planted for a bid as cane is for paper.

use std::collections::BTreeMap;

use enterprise_core::farms::{allowed, choose, field_crops, plan, rotation, sold_as, Wants};
use enterprise_core::map::Land;
use enterprise_core::wire::{ContentPack, PackCrop, PackFit, PackGood, PackMade};
use enterprise_core::Content;

const DAY: u64 = 24_000;

fn goods() -> Vec<PackGood> {
    vec![PackGood::new("minecraft:wheat", 0.05, 64), PackGood::new("minecraft:sugar_cane", 1.0 / 24.0, 64),
        PackGood::new("minecraft:paper", 1.0 / 24.0, 64), PackGood::new("cotton:cotton", 0.05, 64),
        PackGood::new("cotton:cloth", 0.4, 64)]
}

fn file(good: &str) -> PackCrop {
    PackCrop { good: good.into(), rotation: false, sold_as: None, water: 1, cash: false, fit: vec![], made_into: vec![] }
}

/// Cotton, as its mod's file would have it: grown to sell, on the plains and hardly on the sand, woven into cloth.
fn cotton() -> PackCrop {
    PackCrop { cash: true, fit: vec![PackFit { land: "plains".into(), fit: 1.0 }, PackFit { land: "desert".into(), fit: 0.1 }],
        made_into: vec![PackMade { good: "cotton:cloth".into(), from: 4.0, makes: 1.0 }], ..file("cotton:cotton") }
}

#[test]
fn a_pack_with_no_crop_files_has_the_mod_s_crops_for_its_goods_and_one_with_files_has_those() {
    let c = Content::from_pack(&ContentPack::new(DAY, goods())).unwrap();
    assert_eq!(field_crops(&c), vec![0, 1], "wheat and cane, the mod's crops the pack has goods for");
    assert_eq!(rotation(&c), vec![0]);
    assert_eq!(sold_as(&c, 1), 2, "cane sells as paper");
    let mut pack = ContentPack::new(DAY, goods());
    pack.crops = vec![PackCrop { rotation: true, ..file("minecraft:wheat") }, cotton()];
    let c = Content::from_pack(&pack).unwrap();
    assert_eq!(field_crops(&c), vec![0, 3], "the files' crops, in their order: no cane without its file");
    assert_eq!(rotation(&c), vec![0]);
    assert_eq!(sold_as(&c, 3), 3, "cotton sells as itself");
}

#[test]
fn a_crop_added_by_file_is_tended_and_planted_for_a_bid_where_its_land_suits_it() {
    let mut pack = ContentPack::new(DAY, goods());
    pack.crops = vec![PackCrop { rotation: true, ..file("minecraft:wheat") }, cotton()];
    let c = Content::from_pack(&pack).unwrap();
    let (wheat, cotton, cloth) = (0, 3, 4);
    // Wheat its people buy at its worth; a jeans works down the road bids 0.2 for 16 cotton a day after the haul.
    let mut wants = Wants { demand: [(wheat, 16.0)].into(), draws: [(cotton, (16.0, 0.2))].into(), ..Default::default() };
    wants.land = Some(Land::Plains);
    assert_eq!(choose(&c, &wants.all(), &BTreeMap::new(), 1000.0, false), Some(cotton), "the plains: cotton");
    wants.land = Some(Land::Desert);
    assert_eq!(choose(&c, &wants.all(), &BTreeMap::new(), 1000.0, false), Some(wheat), "the sand: wheat, cotton hardly grows");
    // Fields of both in the ground: the Farmers tend the cotton the bid pays for.
    let plots: BTreeMap<usize, u32> = [(wheat, 64), (cotton, 64)].into();
    let (tended, _) = plan(&c, &plots, &[1], &wants, 1.0, true, &mut BTreeMap::new());
    assert_eq!(tended.get(&cotton).copied(), Some(16.0), "{tended:?}");
    // A town still growing, short of food, doesn't plant what it can't eat on land that suits wheat as well.
    wants.land = Some(Land::Plains);
    assert!(!allowed(&c, cotton, &wants.all(), 0.0, true));
    // And cloth bid for is cotton's use too: four cotton to the cloth, priced a quarter of the cloth's bid.
    let wants = Wants { demand: [(wheat, 16.0)].into(), draws: [(cloth, (4.0, 1.2))].into(), land: Some(Land::Plains),
        ..Default::default() };
    assert_eq!(choose(&c, &wants.all(), &BTreeMap::new(), 1000.0, false), Some(cotton), "cotton for the cloth");
}

#[test]
fn a_crop_file_the_core_cannot_read_is_refused_with_what_is_wrong() {
    let mut pack = ContentPack::new(DAY, goods());
    pack.crops = vec![file("cotton:silk"), PackCrop { sold_as: Some("cotton:yarn".into()), water: 82,
        fit: vec![PackFit { land: "moon".into(), fit: 2.0 }], ..file("minecraft:wheat") }, file("minecraft:wheat")];
    let why = Content::from_pack(&pack).unwrap_err();
    for wrong in ["no good called that", "\"cotton:yarn\" to sell it as", "more than a field", "no land called \"moon\"",
        "not a number from 0 to 1", "a crop another file is"] {
        assert!(why.contains(wrong), "{wrong:?} in {why}");
    }
}
