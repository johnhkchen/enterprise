//! Storage (`storage.rs`): what a town holds beyond what it wants goes off, food first, out of the world; durable
//! goods keep, and a Shepherd shears no more once the town holds its fill of wool.

use enterprise_core::kit;
use enterprise_core::storage::{keeping, store_of, Keeping, Stores, STORE_SLOTS};
use enterprise_core::works::Kind;
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;

fn content() -> Content {
    Content::new(vec![Good::new("beef", 0.1, 64).food(3.0), Good::new("bread", 0.2, 64).food(5.0),
        Good::new("white_wool", 0.1, 64), Good::new("iron_ingot", 1.0, 64)], DAY)
}

#[test]
fn raw_meat_goes_off_fastest_then_food_then_wool_and_iron_keeps() {
    let c = content();
    assert_eq!([0, 1, 2, 3].map(|g| keeping(&c, g)), [Keeping::Raw, Keeping::Food, Keeping::Fibre, Keeping::Keeps]);
    let mut s = Society::new(content());
    s.content_mut().dials.meal_points = 0.0; // (none of it eaten)
    let t = s.add_town("Larderby", 100.0);
    for g in 0..4 {
        s.deliver(t, g, 1064.0);
    }
    let before = s.totals();
    s.advance(11 * DAY);
    let held = s.town(t).stock;
    let gone: Vec<f64> = (0..4).map(|g| 1064.0 - held[g]).collect();
    assert!(gone[0] > gone[1] && gone[1] > gone[2] && gone[2] > 0.0 && gone[3] == 0.0, "{gone:?}");
    // A tenth of the thousand beyond its keep, raw meat at a twentieth a day, for ten days: about 400 gone.
    assert!((gone[0] - 1000.0 * (1.0 - 0.95f64.powi(10))).abs() < 30.0, "{}", gone[0]);
    assert!(s.stored(t).spoiled > 0.0);
    s.conserved(&before).unwrap();
}

#[test]
fn what_a_town_wants_of_a_good_keeps() {
    let mut s = Society::new(content());
    s.content_mut().dials.meal_points = 0.0;
    let t = s.add_town("Larderby", 100.0);
    s.deliver(t, 0, 60.0);
    s.advance(10 * DAY);
    assert_eq!(s.town(t).stock[0], 60.0, "below its keep, none goes off");
}

#[test]
fn each_storehouse_holds_its_kind_of_goods() {
    let c = Content::new(["iron_sword", "bread", "white_wool", "white_carpet", "leather_chestplate", "iron_ingot"].iter()
        .map(|n| if *n == "bread" { Good::new(n, 0.2, 64).food(5.0) } else { Good::new(n, 0.5, 64) }).collect(), DAY);
    let kinds: Vec<Option<Kind>> = (0..6).map(|g| store_of(&c, g)).collect();
    assert_eq!(kinds, [Some(Kind::Armory), Some(Kind::Granary), Some(Kind::Storehouse), Some(Kind::Storehouse),
        Some(Kind::Armory), None]);
}

#[test]
fn a_storehouse_holds_its_kind_out_of_the_warehouse_up_to_its_slots() {
    let c = content();
    let mut stock = vec![0.0; c.goods.len()];
    stock[2] = 64.0 * 40.0; // forty stacks of wool
    let none = kit::free_slots(&c, &stock, 0, Stores::default());
    let held = kit::free_slots(&c, &stock, 0, Stores { storehouse: true, ..Default::default() });
    assert_eq!(held - none, STORE_SLOTS as i64, "the storehouse holds 27 of them");
    let other = kit::free_slots(&c, &stock, 0, Stores { armory: true, granary: true, ..Default::default() });
    assert_eq!(other, none, "the armory and granary hold no wool");
}

