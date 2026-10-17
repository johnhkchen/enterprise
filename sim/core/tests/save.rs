//! The state as one blob (`save.rs`): saved, loaded and run on, a society comes out as if never saved, to the bit;
//! dials set by name are saved with the world (`Dials.java`); a contract's whole state survives.

mod common;

use common::{world, DAY};
use enterprise_core::{Content, Dials, Good, Incoterm, Society, State};

/// Everything a host can see of a society, exactly (`{:?}` of an `f64` is its shortest exact form).
fn seen(s: &Society) -> String {
    let towns: Vec<String> = (0..s.town_count()).map(|t| format!("{:?}", s.town(t))).collect();
    let routes: Vec<String> = (0..s.route_count()).map(|r| format!("{:?}", s.route(r))).collect();
    format!("{towns:?}\n{routes:?}\n{:?}\n{:?}", s.contracts(), s.bills())
}

#[test]
fn load_then_run_equals_never_saved_to_the_bit() {
    for seed in [0, 3, 7, 42, 77, 99, 123, 256, 1234, 4321] {
        let mut kept = world(seed);
        kept.advance(50 * DAY);
        let blob = kept.save();
        let mut loaded = Society::load(&blob).unwrap();
        assert_eq!(loaded.world.resource::<enterprise_core::state::Towns>().0,
            kept.world.resource::<enterprise_core::state::Towns>().0, "seed {seed}: the towns' entities line up");
        assert_eq!(seen(&loaded), seen(&kept), "seed {seed}: loaded as saved");
        for day in 51..=100 {
            let (a, b) = (kept.advance(day * DAY), loaded.advance(day * DAY));
            assert_eq!(a, b, "seed {seed}, day {day}: the same wagons");
        }
        for t in 0..kept.town_count() {
            assert_eq!(kept.town(t).money.to_bits(), loaded.town(t).money.to_bits(), "seed {seed}, town {t}");
        }
        assert_eq!(seen(&loaded), seen(&kept), "seed {seed}: the same world at day 100");
        assert!(loaded.save() == kept.save(), "seed {seed}: the same blob at day 100");
    }
}

// parity: ContractTests.aDialIsSetForTheWorldAndSavedWithIt
#[test]
fn a_dial_is_set_for_the_world_and_saved_with_it() {
    let mut s = world(5);
    s.set_dial("shelf_days", 6.0).unwrap();
    assert_eq!(s.content().dials.shelf_days, 6.0);
    let mut loaded = Society::load(&s.save()).unwrap();
    assert_eq!(loaded.dial("shelf_days"), Some(6.0), "saved as 6");
    loaded.reset_dial("shelf_days").unwrap();
    assert_eq!(loaded.content().dials.shelf_days, Dials::default().shelf_days, "reset to the mod's 5");
    assert_eq!(loaded.dial("shelf_days"), Some(5.0));
}

#[test]
fn dials_by_name_round_whole_numbers_and_refuse_strangers() {
    let mut d = Dials::default();
    d.set("wagon_slots", 26.6).unwrap();
    assert_eq!(d.wagon_slots, 27, "rounded, as the mod's getInt");
    d.set("standing_orders", 0.0).unwrap();
    assert!(!d.standing_orders);
    assert_eq!(d.get("standing_orders"), Some(0.0));
    assert!(d.set("no_such_dial", 1.0).is_err());
    assert!(d.reset("no_such_dial").is_err());
    assert!(d.set("wagon_slots", -1.0).is_err(), "not a count");
    assert!(d.set("shelf_days", f64::NAN).is_err());
}

#[test]
fn every_dial_has_a_name() {
    let json = serde_json::to_value(Dials::default()).unwrap();
    let mut fields: Vec<&str> = json.as_object().unwrap().keys().map(|k| k.as_str()).collect();
    let mut names = Dials::names();
    fields.sort();
    names.sort();
    assert_eq!(names, fields, "every field of Dials is listed in dials_by_name! (content.rs)");
    for name in Dials::names() {
        let mut d = Dials::default();
        let v = d.get(name).unwrap();
        d.set(name, if v == 1.0 { 0.0 } else { v + 1.0 }).unwrap();
        assert_ne!(d.get(name), Some(v), "{name} is set by its name");
        d.reset(name).unwrap();
        assert_eq!(d.get(name), Some(v), "{name} resets to its default");
    }
}

// parity-partial: ContractTests.aBrokenContractOwesBackItsUnearnedAdvance
#[test]
fn a_broken_contracts_state_survives_save_and_load() {
    let mut s = Society::new(Content::new(vec![Good::new("iron_sword", 1.0, 1)], DAY));
    let seller = s.add_town("Forge", 10.0);
    let buyer = s.add_town("Keep", 10.0);
    let t = 10 * DAY;
    let id = s.draft(seller, buyer, 0, 3.0, 1.0, t, DAY, 4, Incoterm::Dap, true);
    s.advance(t + 3 * DAY + 2 * DAY + 1); // all four judged, none delivered
    s.review();
    let c = s.contract(id);
    assert_eq!(c.state, State::Broken, "broken: {c:?}");
    let mut loaded = Society::load(&s.save()).unwrap();
    assert_eq!(format!("{:?}", loaded.contract(id)), format!("{c:?}"), "saved and loaded whole");
    assert_eq!(loaded.trust(seller, buyer).to_bits(), s.trust(seller, buyer).to_bits(), "and the pair's record");
    s.advance(t + 30 * DAY);
    loaded.advance(t + 30 * DAY);
    assert_eq!(loaded.contract(id).state, State::Broken);
    assert_eq!(loaded.trust(seller, buyer).to_bits(), s.trust(seller, buyer).to_bits(), "forgiven alike");
}

#[test]
fn a_blob_that_isnt_a_save_is_refused() {
    assert!(Society::load(b"").is_err());
    assert!(Society::load(b"not a save at all").is_err());
    let mut blob = world(1).save();
    blob[4] = blob[4].wrapping_add(1); // another format
    assert!(Society::load(&blob).err().unwrap().contains("format"));
    let mut blob = world(1).save();
    blob.truncate(blob.len() / 2);
    assert!(Society::load(&blob).is_err(), "cut short");
}
