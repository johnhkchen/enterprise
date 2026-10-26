//! Mines and kit (`kit.rs`, `rules/kit.rs`), pinned as the mod's metal, master, labour and building tests pin them:
//! a Toolsmith's mine, kit wearing out (mended, Unbreaking), the deep mine, a Nitwit gathering what the land gives,
//! a Fletcher hunting with a bow, spare bows arming the guards. Goods are the mod's, by name.

use enterprise_core::kit::{self, Crew, Kits, Wear};
use enterprise_core::map::Land;
use enterprise_core::{Content, Good, Hex, Society};

const DAY: u64 = 24_000;
const LOOK: u64 = 1_200;

const NAMES: [&str; 34] = ["raw_iron", "coal", "raw_gold", "lapis_lazuli", "iron_ingot", "iron_hoe", "iron_pickaxe",
    "iron_sword", "iron_chestplate", "bow", "stick", "string", "feather", "oak_log", "wheat_seeds", "dandelion", "poppy",
    "flint", "sand", "clay_ball", "cobblestone", "beef", "porkchop", "chicken", "iron_axe", "leather", "rabbit",
    "rabbit_hide", "sugar_cane", "apple", "sweet_berries", "spruce_log", "cactus", "ink_sac"];

fn content() -> Content {
    Content::new(NAMES.iter().map(|n| Good::new(n, 0.1, 64)).collect(), DAY)
}

fn good(c: &Content, name: &str) -> usize {
    c.good(name).unwrap_or_else(|| panic!("no good called {name}"))
}

fn stock(s: &Society, t: usize, name: &str) -> f64 {
    s.town(t).stock[good(s.content(), name)]
}

/// A town of these hands, placed on a hex of its own.
fn town(s: &mut Society, hands: &[(&str, u8)]) -> usize {
    let t = s.add_town("Town", 0.0);
    s.set_roster(t, hands);
    let hex = Hex::new(3, 3);
    s.place_town(t, hex.center_x(), hex.center_z());
    t
}

// parity: MetalTests.aToolsmithsMineBringsUpOre
#[test]
fn a_toolsmiths_mine_brings_up_ore() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("toolsmith", 2)]);
    s.set_mine(t, false);
    s.advance(DAY);
    assert_eq!(stock(&s, t, "raw_iron"), 4.0, "a level-2 Toolsmith brings up 4 raw iron a day");
    assert_eq!(stock(&s, t, "coal"), 1.0, "and a coal");
    assert_eq!(stock(&s, t, "raw_gold"), 0.0, "and half a gold, owed");
    assert_eq!(stock(&s, t, "lapis_lazuli"), 0.0, "no lapis while the mine isn't deep");
    s.advance(3 * DAY);
    assert_eq!((stock(&s, t, "raw_iron"), stock(&s, t, "coal"), stock(&s, t, "raw_gold")), (12.0, 3.0, 1.0));
}

#[test]
fn a_town_without_a_mine_brings_up_no_ore() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("toolsmith", 2)]);
    s.advance(3 * DAY);
    assert_eq!(stock(&s, t, "raw_iron"), 0.0);
    assert!(s.mine(t).is_none(), "and with no emeralds spare digs none");
}

#[test]
fn a_toolsmith_digs_a_mine_for_six_emeralds_beyond_the_reserve() {
    let mut s = Society::new(content());
    let poor = town(&mut s, &[("toolsmith", 2)]);
    s.pay(poor, 5.0);
    let rich = town(&mut s, &[("toolsmith", 2)]);
    s.pay(rich, 10.0);
    let none = town(&mut s, &[("farmer", 2)]);
    s.pay(none, 10.0);
    s.advance(DAY);
    assert!(s.mine(poor).is_none(), "5 emeralds aren't enough");
    assert_eq!(s.mine(rich), Some(kit::Mine { deep: false }), "10 are");
    assert_eq!(s.town(rich).money, 4.0, "and a mine costs 6");
    assert!(s.mine(none).is_none(), "and a town needs a Toolsmith");
    assert_eq!(stock(&s, rich, "raw_iron"), 4.0, "the mine is worked from then");
}

#[test]
fn pickaxes_bring_up_half_again_as_much_ore() {
    let mut plain = Society::new(content());
    let a = town(&mut plain, &[("toolsmith", 2)]);
    plain.set_mine(a, false);
    let mut tooled = Society::new(content());
    let b = town(&mut tooled, &[("toolsmith", 2)]);
    tooled.set_mine(b, false);
    tooled.add_kit(b, "iron_pickaxe", 1, 0.0, 0);
    plain.advance(DAY);
    tooled.advance(DAY);
    assert_eq!(tooled.kit_share(b, "iron_pickaxe"), 1.0, "a pickaxe for the only miner");
    assert_eq!((stock(&plain, a, "raw_iron"), stock(&tooled, b, "raw_iron")), (4.0, 6.0));
}

// parity: MetalTests.anArmorerSmeltsAndAToolsmithForgesHoes
#[test]
fn an_armorer_smelts_and_a_toolsmith_forges_hoes() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("toolsmith", 2), ("armorer", 2), ("farmer", 2), ("farmer", 2)]);
    s.deliver(t, good(s.content(), "raw_iron"), 16.0);
    s.deliver(t, good(s.content(), "coal"), 2.0);
    s.advance(2 * LOOK);
    assert_eq!(stock(&s, t, "iron_hoe"), 2.0, "a hoe for each farmer");
    assert_eq!(stock(&s, t, "iron_ingot"), 12.0, "of the 16 iron smelted");
    assert_eq!(stock(&s, t, "iron_pickaxe"), 0.0, "no mine, no miners: no pickaxes");
    s.advance(3 * LOOK);
    assert_eq!(s.kit_share(t, "iron_hoe"), 1.0, "every farmer has tools");
}

// parity: MetalTests.toolsWearOutSlowerWhenMended
#[test]
fn tools_wear_out_slower_when_mended() {
    let c = content();
    let hoe = good(&c, "iron_hoe");
    let mut stock = vec![0.0; c.goods.len()];
    stock[hoe] = 4.0;
    let mut kits = Kits::default();
    let crew = Crew { farmers: 2, ..Default::default() };
    let used = kits.wear(&c, &mut stock, 16.0, crew, Wear::mended(false));
    assert_eq!(used, vec![(hoe, 2)], "two farmers wear out a hoe each in 16 days");
    assert_eq!(stock[hoe], 2.0);
    let used = kits.wear(&c, &mut stock, 16.0, crew, Wear::mended(true));
    assert_eq!(used, vec![(hoe, 1)], "half that while mended");
}

// parity: MasterTests.unbreakingKitLastsLonger
#[test]
fn unbreaking_kit_lasts_longer() {
    let c = content();
    let hoe = good(&c, "iron_hoe");
    let mut stock = vec![0.0; c.goods.len()];
    stock[hoe] = 2.0;
    let mut kits = Kits::default();
    kits.enchant(hoe, 2, 0.0, 3);
    let crew = Crew { farmers: 2, ..Default::default() };
    let used = kits.wear(&c, &mut stock, 16.0, crew, Wear::mended(false));
    assert!(used.is_empty(), "Unbreaking III: four times as long, {used:?} used in 16 days");
    assert_eq!(kits.wear[&hoe], 0.5, "a half a hoe owed");
    let used = kits.wear(&c, &mut stock, 16.0, crew, Wear::mended(false));
    assert_eq!(used, vec![(hoe, 1)], "and the next 16 days take one of the two (the first in use)");
    assert_eq!(kits.pieces[0].count, 1, "an enchanted one");
    assert_eq!(kits.wear[&hoe], 0.0);
}

#[test]
fn kit_wears_out_in_a_town_as_the_days_pass() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("farmer", 1), ("farmer", 1)]);
    s.deliver(t, good(s.content(), "iron_hoe"), 4.0);
    s.advance(16 * DAY);
    assert_eq!(stock(&s, t, "iron_hoe"), 2.0, "two farmers, 16 days: a hoe each");
    let mut mended = Society::new(content());
    let m = town(&mut mended, &[("farmer", 1), ("farmer", 1)]);
    mended.deliver(m, good(mended.content(), "iron_hoe"), 4.0);
    mended.mend(m, 20 * DAY);
    mended.advance(16 * DAY);
    assert_eq!(stock(&mended, m, "iron_hoe"), 3.0, "half that while a Toolsmith mends them");
}

#[test]
fn enchanted_kit_is_worth_more() {
    let c = content();
    let sword = good(&c, "iron_sword");
    let mut kits = Kits::default();
    kits.enchant(sword, 1, 3.0, 0); // quality 1.45
    kits.enchant(sword, 1, 10.0, 0); // quality 1.5 at most
    assert_eq!(kits.share(sword, 4.0, 4), (1.5 + 1.45 + 2.0) / 4.0, "the best first, two plain");
    assert_eq!(kits.share(sword, 4.0, 2), (1.5 + 1.45) / 2.0, "two wearers have the two best");
    assert_eq!(kits.share(sword, 4.0, 1), 1.5, "one has the best: up to half again, no more");
    assert_eq!(kits.share(sword, 4.0, 0), 0.0, "no wearers, no share");
    assert_eq!(kits.share(sword, 1.0, 1), 1.45, "an enchanted piece lost to a sale isn't counted: the last in the list goes first");
}

// parity: MasterTests.aMasterToolsmithSinksTheMineDeep
#[test]
fn a_master_toolsmith_sinks_the_mine_deep() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("toolsmith", 5), ("mason", 3)]);
    s.set_mine(t, false);
    s.pay(t, 30.0);
    s.advance(LOOK);
    assert_eq!(s.mine(t), Some(kit::Mine { deep: true }), "sunk deep, the Mason shoring it");
    assert_eq!(s.town(t).money, 22.0, "for 8 emeralds");
    s.advance(DAY);
    assert_eq!(stock(&s, t, "lapis_lazuli"), 2.0, "a master brings up 2.5 lapis a day");
    assert_eq!(stock(&s, t, "raw_iron"), 10.0, "and 10 raw iron");
}

#[test]
fn a_deep_mine_needs_a_master_a_mason_and_the_emeralds() {
    for (hands, money) in [(vec![("toolsmith", 4), ("mason", 3)], 30.0), (vec![("toolsmith", 5), ("mason", 2)], 30.0),
        (vec![("toolsmith", 5)], 30.0), (vec![("toolsmith", 5), ("mason", 3)], 7.0)] {
        let mut s = Society::new(content());
        let t = town(&mut s, &hands);
        s.set_mine(t, false);
        s.pay(t, money);
        s.advance(DAY);
        assert_eq!(s.mine(t), Some(kit::Mine { deep: false }), "{hands:?} with {money}");
        assert_eq!(stock(&s, t, "lapis_lazuli"), 0.0);
    }
}

// parity: LabourTests.aNitwitGathersWhatTheLandGives
#[test]
fn a_nitwit_gathers_what_the_land_gives() {
    for land in [Land::Plains, Land::Woods, Land::Taiga, Land::Desert, Land::Shore, Land::Hills, Land::Water] {
        let mut s = Society::new(content());
        let t = town(&mut s, &[("nitwit", 1)]);
        s.set_land(s.town_hex(t).unwrap(), land);
        s.advance(DAY);
        let hauls = kit::hauls(land);
        let (first, per_day) = hauls[0];
        assert_eq!(stock(&s, t, first), (per_day * 1.5).floor(), "a day of a Nitwit's gathering on {land:?}: {first}");
        for &(name, per_day) in hauls {
            assert_eq!(stock(&s, t, name), (per_day * 1.5).floor(), "{land:?}: {name}");
        }
    }
}

#[test]
fn another_idle_hand_gathers_a_third_less_and_each_works_a_hex() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("nitwit", 1), ("none", 1), ("farmer", 1)]);
    let home = s.town_hex(t).unwrap();
    s.set_land(home, Land::Hills);
    s.advance(DAY);
    // The Nitwit works the home hex (16 cobblestone, 1.5 times); the other hand, on the same hex, finds the 16 kept of
    // a good the town doesn't use already owed, and gathers none (`Labour.room` counts what's owed).
    assert_eq!(s.holding(t).hexes.len(), 1);
    assert_eq!(stock(&s, t, "cobblestone"), 24.0);
    assert_eq!(stock(&s, t, "flint"), 7.0, "(flint: 3 for the Nitwit, 4.5, and 3 more for the other, the 4.5 owed being under 16)");
}

#[test]
fn hands_work_the_hexes_by_q_then_r_not_as_claimed() {
    // Home (3, 3) on plains is claimed first, then (2, 3) on hills: the one Nitwit works (2, 3), the first by q.
    let mut s = Society::new(content());
    let t = town(&mut s, &[("nitwit", 1)]);
    let home = s.town_hex(t).unwrap();
    assert!(s.claim(t));
    assert!(s.claim_hex(t, Hex::new(2, 3)));
    s.set_land(Hex::new(2, 3), Land::Hills);
    assert_eq!(s.holding(t).hexes[0], home, "(claimed first)");
    s.advance(DAY);
    assert_eq!(stock(&s, t, "cobblestone"), 24.0, "the hills' 16, by 1.5");
    assert_eq!(stock(&s, t, "wheat_seeds"), 0.0, "and none of the plains'");
}

#[test]
fn hands_stop_gathering_at_sixteen_of_a_good_the_town_does_not_use_and_sixty_four_of_one_it_or_a_neighbour_does() {
    let c = content();
    let sticks = good(&c, "stick");
    let mut trades = enterprise_core::state::Trades { makes: vec![0.0; c.goods.len()], uses: vec![0.0; c.goods.len()],
        capacity: vec![0.0; c.goods.len()], ..Default::default() };
    let mut held = vec![0.0; c.goods.len()];
    held[sticks] = 15.0;
    assert!(kit::room(&c, &trades, &held, 0, sticks, 0.0, false), "15 of 16: room");
    assert!(!kit::room(&c, &trades, &held, 0, sticks, 1.0, false), "15 and 1 owed: none");
    held[sticks] = 16.0;
    assert!(!kit::room(&c, &trades, &held, 0, sticks, 0.0, false), "16: enough");
    assert!(kit::room(&c, &trades, &held, 0, sticks, 0.0, true), "16, and the towns down its roads buy them: room for 64");
    trades.capacity[sticks] = 5.0; // its lockers take sticks
    held[sticks] = 63.0;
    assert!(kit::room(&c, &trades, &held, 0, sticks, 0.0, false), "63 of the 64 it keeps of what it uses");
    held[sticks] = 64.0;
    assert!(!kit::room(&c, &trades, &held, 0, sticks, 0.0, false), "64: enough");
    // The warehouse keeps its 5 free slots and 4 more for the goods it trades (54 slots in all).
    held[sticks] = 0.0;
    trades.capacity[sticks] = 0.0;
    assert!(kit::room(&c, &trades, &held, 44, sticks, 0.0, false), "10 slots free: room");
    assert!(!kit::room(&c, &trades, &held, 45, sticks, 0.0, false), "9 free: none to spare");
    assert_eq!(kit::free_slots(&c, &held, 45), 9);
}

#[test]
fn what_hands_gather_stops_at_what_the_town_keeps() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("nitwit", 1)]);
    s.advance(10 * DAY);
    // Plains: 3 sticks a day (2 * 1.5): up to the 16 kept, the last day's gathering over (3, 6, ... 15, 18).
    assert_eq!(stock(&s, t, "stick"), 18.0);
}

#[test]
fn tools_make_hands_gather_half_again_and_wear_out_in_sixteen_days() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("nitwit", 1)]);
    s.set_land(s.town_hex(t).unwrap(), Land::Hills);
    s.give_tools(t, 1);
    s.advance(DAY);
    // 16 * 1.5 * 1.5 = 36
    assert_eq!(stock(&s, t, "cobblestone"), 36.0, "a tool for the only hand: half as much again");
    assert!((s.kit(t).hand_tools - (1.0 - 1.0 / 16.0)).abs() < 1e-12, "and a day's wear: {}", s.kit(t).hand_tools);
    s.advance(16 * DAY);
    // (a tool wears by the share of the hands it equips: what is left wears by its own share, so it dwindles)
    assert!((s.kit(t).hand_tools - (15.0f64 / 16.0).powi(16)).abs() < 1e-9, "16 days on: {}", s.kit(t).hand_tools);
}

#[test]
fn hands_without_a_tool_are_given_an_axe_from_the_warehouse() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("nitwit", 1), ("none", 1)]);
    s.deliver(t, good(s.content(), "iron_axe"), 1.0);
    s.advance(DAY);
    assert_eq!(stock(&s, t, "iron_axe"), 0.0, "taken");
    assert!(s.kit(t).hand_tools > 0.9, "by a hand: {}", s.kit(t).hand_tools);
}

// parity: BuildingTests.aFletcherHuntsWithABow
#[test]
fn a_fletcher_hunts_with_a_bow() {
    // As the mod's: `LabourTests.town`'s Fletcher is a Novice, and the bow a plain one in the warehouse.
    let mut s = Society::new(content());
    let t = town(&mut s, &[("fletcher", 1)]);
    s.deliver(t, good(s.content(), "bow"), 1.0);
    s.advance(DAY);
    let feathers = stock(&s, t, "feather");
    assert!(s.hunting(t) == 1.0 && feathers >= 1.0, "a day's hunt with a bow: {feathers} feathers");
    assert_eq!(feathers, 2.0, "on plains, 2 feathers a level");
}

#[test]
fn a_journeyman_fletcher_hunts_a_level_each() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("fletcher", 2)]);
    s.add_kit(t, "bow", 1, 0.0, 0);
    s.advance(DAY);
    let feathers = stock(&s, t, "feather");
    assert!(s.hunting(t) == 1.0 && feathers >= 1.0, "a day's hunt with a bow: {feathers} feathers");
    // On plains (temperate): 2 feathers, 1 beef, pork and chicken, half a leather, a level each (2 levels).
    assert_eq!((feathers, stock(&s, t, "beef"), stock(&s, t, "porkchop"), stock(&s, t, "chicken")), (4.0, 2.0, 2.0, 2.0));
}

#[test]
fn a_fletcher_without_a_bow_sets_snares_for_a_quarter() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("fletcher", 2)]);
    s.advance(DAY);
    assert_eq!(s.hunting(t), 0.0);
    assert_eq!(stock(&s, t, "feather"), 1.0, "2 levels * 0.25 * 2 feathers");
    assert_eq!(stock(&s, t, "beef"), 0.0, "and half a beef owed");
}

#[test]
fn a_master_fletcher_leads_a_hunting_party_and_game_depends_on_the_land() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("fletcher", 5)]);
    s.add_kit(t, "bow", 1, 0.0, 0);
    s.advance(DAY);
    assert_eq!(stock(&s, t, "feather"), 20.0, "5 levels * 2 * 2 feathers");
    for (land, feathers, rabbit) in [(Land::Desert, 2.0, 4.0), (Land::Taiga, 2.0, 4.0), (Land::Water, 2.0, 0.0), (Land::Woods, 4.0, 0.0)] {
        let mut s = Society::new(content());
        let t = town(&mut s, &[("fletcher", 2)]);
        s.set_land(s.town_hex(t).unwrap(), land);
        s.add_kit(t, "bow", 1, 0.0, 0);
        s.advance(DAY);
        assert_eq!(stock(&s, t, "feather"), feathers, "{land:?}");
        assert_eq!(stock(&s, t, "rabbit"), rabbit, "{land:?}");
    }
}

// parity: BuildingTests.sparesBowsArmTheGuards
#[test]
fn spare_bows_arm_the_guards() {
    // 2 bows and a sword, 2 guards and a hunter: a sword and the bow the hunter doesn't need arm both guards.
    assert_eq!(kit::armed(1, 2, 2, 1, 0.0), 1.0);
    // Or half of four, at the watchtower's floor of a half.
    assert_eq!(kit::armed(1, 2, 4, 1, 0.5), 0.5);
    assert_eq!(kit::armed(1, 2, 4, 1, 0.0), 0.5, "(a sword and a spare bow of four)");
    assert_eq!(kit::armed(0, 0, 0, 0, 0.5), 0.0, "no guards, none armed");
    assert_eq!(kit::armed(9, 0, 3, 0, 0.0), 1.0, "and never more than all");
    assert_eq!(kit::hunting(1, 2), 0.5);
    assert_eq!(kit::hunting(3, 2), 1.0);
    assert_eq!(kit::hunting(1, 0), 0.0);
}

#[test]
fn a_town_keeps_its_kit_shares_as_it_looks() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("farmer", 1), ("farmer", 1), ("fletcher", 1), ("toolsmith", 1)]);
    s.add_kit(t, "iron_hoe", 1, 0.0, 0);
    s.add_kit(t, "bow", 1, 0.0, 0);
    s.advance(2 * LOOK);
    assert_eq!(s.kit_share(t, "iron_hoe"), 0.5, "a hoe for one of two farmers");
    assert_eq!(s.kit_share(t, "iron_pickaxe"), 0.0, "no mine, no miners, no share");
    assert_eq!(s.hunting(t), 1.0, "a bow for the hunter");
    assert_eq!(s.armed(t), 0.0, "and none for guards that aren't posted");
    s.set_mine(t, false);
    s.add_kit(t, "iron_pickaxe", 1, 0.0, 0);
    s.advance(4 * LOOK);
    assert_eq!(s.kit_share(t, "iron_pickaxe"), 1.0, "now it has a miner");
}

#[test]
fn a_toolsmith_forges_pickaxes_for_its_miners_once_it_has_a_mine() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("toolsmith", 2), ("toolsmith", 1)]);
    s.deliver(t, good(s.content(), "iron_ingot"), 20.0);
    s.advance(LOOK);
    assert_eq!(stock(&s, t, "iron_pickaxe"), 0.0, "no mine, no miners");
    s.set_mine(t, false);
    s.advance(2 * LOOK);
    assert_eq!(stock(&s, t, "iron_pickaxe"), 2.0, "a pickaxe for each of its two miners");
    assert_eq!(stock(&s, t, "iron_ingot"), 14.0, "of 3 iron each");
}

#[test]
fn the_kit_is_saved_with_the_society() {
    let mut s = Society::new(content());
    let t = town(&mut s, &[("farmer", 1), ("toolsmith", 3), ("nitwit", 1)]);
    s.set_mine(t, true);
    s.add_kit(t, "iron_hoe", 3, 2.0, 1);
    s.give_tools(t, 2);
    s.mend(t, 5 * DAY);
    s.advance(3 * DAY);
    let bytes = s.save();
    let mut loaded = Society::load(&bytes).unwrap();
    assert_eq!(loaded.kit(t), s.kit(t));
    assert_eq!(loaded.mine(t), s.mine(t));
    s.advance(6 * DAY);
    loaded.advance(6 * DAY);
    assert_eq!(loaded.kit(t), s.kit(t), "and goes on as if it had never been");
    assert_eq!(loaded.town(t).stock, s.town(t).stock);
}
