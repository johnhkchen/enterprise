//! The crown (`crown.rs`, `rules/crown.rs`), pinned as the mod's war and caravan tests pin it: bonus clauses pay for
//! quality without requiring it; mobilising mints into the treasury (debasing the spur) and stockpiles kit and the
//! army's food put by; a requisition takes the best offer for its whole lot; a caravan brings mercenaries hired over the
//! sea. And the plan's numbers: levies, the procurement agent's day, tenders, the front's orders, peace and the tally.

use enterprise_core::army::{self};
use enterprise_core::crown::{self, Bonus, Enchant, Theatre};
use enterprise_core::sourcing::Phase;
use enterprise_core::{Content, Good, Hex, Incoterm, Society, State};

const DAY: u64 = 24_000;

/// The mod's breads and meats (with their saturation), its weapons, armour and shield, and arrows.
fn content() -> Content {
    let mut goods: Vec<Good> = ["bread", "cooked_beef", "cooked_porkchop"].iter().map(|n| army::food_good(n, 64)).collect();
    for g in army::WEAPONS.iter().chain(army::ARMOUR.iter().flatten()).chain([&army::SHIELD]) {
        goods.push(Good::priced(g.name, 1));
    }
    goods.push(Good::priced("arrow", 64));
    Content::new(goods, DAY)
}

fn good(s: &Society, name: &str) -> usize {
    s.content().good(name).unwrap_or_else(|| panic!("no good called {name}"))
}

// parity: WarTests.bonusClausesPayForQualityWithoutRequiringIt
#[test]
fn bonus_clauses_pay_for_quality_without_requiring_it() {
    let mut s = Society::new(content());
    let (a, b) = (s.add_town("A", 0.0), s.add_town("B", 0.0));
    let sword = good(&s, "iron_sword");
    let id = s.draft(a, b, sword, 4.0, 2.0, 0, DAY, 1, Incoterm::Exw, true);
    s.world.resource_mut::<enterprise_core::contracts::Book>().contracts[id].bonuses =
        vec![Bonus::new(&[(Enchant::Unbreaking, 1), (Enchant::Sharpness, 2)], 25.0)];
    // A sharp one: Sharpness 3 and Unbreaking 1, as the seller's kit holds it (its edge suited to a faction's men).
    assert!(s.add_kit(a, "iron_sword", 1, 3.0, 1));
    assert_eq!(s.town(a).stock[sword], 1.0, "any iron sword counts, enchanted or not: it's an iron sword in stock");
    let sharp = crown::enchantments(s.content(), sword, 3.0, 1, Theatre::Faction);
    assert_eq!(sharp, vec![(Enchant::Sharpness, 3), (Enchant::Unbreaking, 1)]);
    let c = s.contract(id);
    assert_eq!((c.price_of(&[]), c.price_of(&sharp)), (2.0, 2.5), "a plain one at the price, a sharp one 25% on top");
    assert_eq!(c.price_of(&[(Enchant::Sharpness, 1), (Enchant::Unbreaking, 1)]), 2.0, "every enchantment at its level");
}

#[test]
fn the_crowns_clauses_name_what_suits_each_piece_and_the_enemy() {
    let s = Society::new(content());
    let pct = |name: &str, theatre: Theatre| -> Vec<f64> {
        crown::bonuses(s.content(), good(&s, name), theatre).iter().map(|b| b.percent).collect()
    };
    assert_eq!(pct("diamond_sword", Theatre::Faction), vec![25.0]);
    assert_eq!(pct("iron_sword", Theatre::Undead), vec![25.0, 30.0], "and Smite against the undead");
    assert_eq!(pct("iron_chestplate", Theatre::Faction), vec![25.0, 15.0]);
    assert_eq!(crown::bonuses(s.content(), good(&s, "iron_boots"), Theatre::Nether)[0].at_least,
        vec![(Enchant::FireProtection, 2)], "fire protection against the Nether");
    assert_eq!(pct("bow", Theatre::Faction), vec![25.0, 10.0]);
    assert_eq!(pct("shield", Theatre::Faction), vec![10.0]);
    assert!(pct("bread", Theatre::Faction).is_empty());
}

/// A capital (town 0) with its Merchant, placed, and a second town.
fn realm() -> Society {
    let mut s = Society::new(content());
    let capital = s.add_town("Capital", 100.0);
    s.set_roster(capital, &[("merchant", 3), ("none", 1)]);
    let hex = Hex::new(0, 0);
    s.place_town(capital, hex.center_x(), hex.center_z());
    let other = s.add_town("Other", 10.0);
    s.place_town(other, hex.center_x() + 300.0, hex.center_z());
    s
}

// parity-partial: WarTests.mobilisingTheSovereignMintsAndBuysGoldThroughAShop (the vendor block buying gold ingots is Minecraft's: the minted treasury, debasement, swords and ration store are pinned)
#[test]
fn mobilising_the_sovereign_mints_and_stockpiles() {
    let mut s = realm();
    let t = s.now();
    let before = s.mint().debasement(t, DAY);
    let rate = s.mint_rate();
    let money = s.town(0).money;
    s.mobilise(0, 1000);
    assert_eq!(s.sovereign().phase, Phase::Mobilising, "mobilising");
    // The coin's worth at the rate, rounded down to the thousandth (`fixed.rs`).
    assert!(s.town(0).money - money == enterprise_core::Fixed::floor(1000.0 / rate),
        "a treasury with a day's minting in it: {} more", s.town(0).money - money);
    assert!(s.mint().debasement(t, DAY) > before, "the minting debases the spur");
    let sword = good(&s, "diamond_sword");
    assert_eq!(s.targets().iter().find(|(g, _)| *g == sword).map(|(_, n)| *n), Some(16), "it stockpiles diamond swords too");
    assert_eq!(s.food_reserve(0), 384.0 * 5.0 + 1920.0, "the capital keeps the army's ration store: 3,840 food points");
    assert_eq!(s.food_reserve(1), 0.0, "no other town does");
    s.peace();
    assert_eq!(s.food_reserve(0), 0.0, "at peace it keeps none");
    assert_eq!(s.sovereign().phase, Phase::Peace);
}

#[test]
fn the_stockpile_is_for_a_planned_army_of_24() {
    let s = Society::new(content());
    let n = |name: &str| crown::stockpile(s.content()).into_iter().find(|(g, _)| *g == good(&s, name)).map(|(_, n)| n);
    assert_eq!((n("diamond_sword"), n("iron_sword")), (Some(16), Some(8)));
    assert_eq!((n("diamond_helmet"), n("iron_leggings")), (Some(8), Some(16)), "a third diamond, two thirds iron");
    assert_eq!((n("chainmail_chestplate"), n("leather_chestplate"), n("shield")), (Some(6), Some(6), Some(12)));
    assert_eq!((n("bow"), n("arrow"), n("bread")), (Some(8), Some(256), Some(384)));
    assert_eq!(n("stone_sword"), None);
}

#[test]
fn a_named_capital_mints_its_revenue_daily_into_its_treasury() {
    let mut s = realm();
    s.name_capital(0, 160);
    assert_eq!(s.capital(), Some(0));
    assert!(s.exchange().overseas.is_some(), "its port's exchange opens, with the market across the sea");
    s.advance(DAY / 2);
    let minted: enterprise_core::Fixed = s.cash_book(0, 1).iter().filter(|(_, why, _)| why == "minted").map(|(_, _, n)| n).sum();
    assert!(minted == 160.0 / 16.0, "today's revenue into its treasury, at 16 spurs an emerald: {minted}");
    s.advance(DAY + DAY / 2);
    assert_eq!(s.sovereign().minted, 320.0, "a day later, another day's");
}

// parity: CaravanTests.aRequisitionTakesTheBestOfferForTheWholeLot
#[test]
fn a_requisition_takes_the_best_offer_for_the_whole_lot() {
    let mut s = Society::new(content());
    let (hub, dest) = (s.add_town("Hub", 100.0), s.add_town("Dest", 100.0));
    s.place_town(hub, 0.0, 0.0);
    s.place_town(dest, 300.0, 0.0);
    let bread = good(&s, "bread");
    s.deliver(dest, bread, 256.0); // beyond what its own caravans eat
    s.record_stocktake(dest, &[(bread, 256.0)], Some(0));
    s.learn(hub, dest);
    s.add_route(hub, dest, 0.25, 0.5, 1.0);
    let id = s.post_requisition(hub, &[bread], false, 40.0, 1.5);
    assert_eq!(s.read_requisitions(dest, hub), 1, "the town's Merchant bids");
    assert_eq!(s.read_requisitions(dest, hub), 0, "once");
    let wanted = s.requisitions().into_iter().find(|r| r.id == id).unwrap();
    let bid = wanted.bids[0].clone();
    let ceiling = wanted.ceiling(s.content(), bread);
    let worth = s.content().goods[bread].value;
    assert!((ceiling - worth * 1.1 * 1.1 * 1.5).abs() < 1e-12, "the crown's price: worth, margin, carriage, premium");
    assert!(bid.each < ceiling, "under the crown's price: {}", bid.each);
    let cost = worth * 1.1; // Ex Works, from its stock
    assert!((bid.each - (cost + 0.5 * (ceiling - cost))).abs() < 1e-12, "its cost and half the way to the crown's price");
    assert!(s.award_requisitions(hub).is_empty(), "not while bids are taken");
    s.advance(2 * DAY);
    let drafted = s.award_requisitions(hub);
    assert_eq!(drafted.len(), 1, "the best offer taken");
    let c = s.contract(drafted[0]);
    let wanted = s.requisitions().into_iter().find(|r| r.id == id).unwrap();
    assert!(c.seller == dest && c.buyer == hub && c.good == bread && c.installments == 1 && c.quantity == bid.count
        && wanted.open == 40.0 - bid.count, "one contract for the lot: {}, open {}", bid.count, wanted.open);
    assert_eq!(c.incoterm, Incoterm::Exw, "the hub's wagons go there: they collect");
    assert_eq!(c.state, State::Drafted, "its copy goes back with the next wagon");
}

// parity: CaravanTests.aCaravanBringsMercenariesHiredOverTheSea
#[test]
fn a_caravan_brings_mercenaries_hired_over_the_sea() {
    let mut s = Society::new(content());
    let hub = s.add_town("Hub", 100.0);
    s.place_town(hub, 0.0, 0.0);
    let bread = good(&s, "bread");
    s.deliver(hub, bread, 192.0);
    let sea = s.add_off_map("the market across the sea", -64.0, 0.0, 0.0);
    s.set_off_map_sells(sea, true);
    s.deliver(sea, bread, 20.0); // the wagons' food home
    let route = s.add_route(hub, sea, 64.0 / 1200.0, 0.1, 1.0);
    let hire = s.draft_levy(sea, hub, 3, 1.0, true);
    let before = s.army().waiting();
    s.advance(8000);
    let levy = s.contract(hire).levy.unwrap();
    assert_eq!(levy.raised, 3, "three hired: {:?}, the wagon {:?}", s.contract(hire), s.route(route));
    assert_eq!(s.army().waiting() - before, 3, "and waiting in the barracks");
    assert!(s.army().barracks.iter().all(|r| r.home == sea && r.hand.is_none() && r.pay == 1.0 && r.levy == Some(hire)),
        "their home that market, at a mercenary's pay: {:?}", s.army().barracks);
}

#[test]
fn a_levy_asks_idle_hands_and_a_fifth_of_the_rest_one_to_four() {
    let s = Society::new(content());
    let roster = |hands: &[(&str, u8)]| {
        let mut t = Society::new(content());
        t.add_town("T", 0.0);
        t.set_roster(0, hands);
        t.roster(0)
    };
    let c = s.content();
    assert_eq!(crown::levy_size(c, &roster(&[("merchant", 1), ("farmer", 1), ("mason", 1)])), 1, "at least one");
    assert_eq!(crown::levy_size(c, &roster(&[("none", 1), ("farmer", 1), ("farmer", 1), ("mason", 1)])), 2,
        "an idle hand and a fifth of three, rounded");
    let many: Vec<(&str, u8)> = std::iter::repeat_n(("none", 1), 3).chain(std::iter::repeat_n(("farmer", 2), 10)).collect();
    assert_eq!(crown::levy_size(c, &roster(&many)), 4, "at most four");
}

#[test]
fn a_levy_takes_idle_hands_first_then_a_trade_it_has_others_of_and_the_needed_last() {
    let mut t = Society::new(content());
    t.add_town("T", 0.0);
    t.set_roster(0, &[("merchant", 5), ("weaponsmith", 1), ("farmer", 3), ("farmer", 2), ("mason", 1), ("none", 1)]);
    let roster = t.roster(0);
    let needed = crown::trained_trades(t.content());
    let order = crown::levy_pick(t.content(), &roster, &needed, 6);
    let trades: Vec<String> = order.iter().map(|&i| t.content().trades[roster.hands[i].trade].name.clone()).collect();
    assert_eq!(trades, ["none", "farmer", "farmer", "mason", "weaponsmith"],
        "idle, the farmers (least skilled first), the mason, the crown's smith last; never the Merchant");
    assert_eq!(roster.hands[order[1]].level, 2);
}

#[test]
fn a_levy_keeps_the_only_hand_of_a_trade_its_town_delivers_by_last() {
    let mut t = Society::new(content());
    t.add_town("T", 0.0);
    t.add_town("U", 0.0);
    t.set_roster(0, &[("merchant", 3), ("farmer", 1), ("mason", 1)]);
    let roster = t.roster(0);
    let name = |book: &enterprise_core::contracts::Book| {
        let needed = crown::needed_trades(t.content(), book, 0);
        let i = crown::levy_pick(t.content(), &roster, &needed, 1)[0];
        t.content().trades[roster.hands[i].trade].name.clone()
    };
    let mut book = enterprise_core::contracts::Book::default();
    assert_eq!(name(&book), "farmer", "equals: the roster's order");
    let bread = good(&t, "bread");
    let id = book.draft(0, 1, bread, 8.0, 1.0, DAY, DAY, 4, Incoterm::Exw, false, false);
    assert_eq!(name(&book), "farmer", "a contract the seller hasn't its copy of yet needs nobody");
    book.contracts[id].state = State::Active;
    assert_eq!(name(&book), "mason", "the farmer sells the bread it's bound to deliver (`Apprentices.neededTrades`)");
}

/// A capital with its Merchant, crew and bread, a road to a town of six (two idle), and its port's exchange; named.
fn kingdom() -> Society {
    let mut s = Society::new(content());
    let capital = s.add_town("Capital", 400.0);
    s.set_roster(capital, &[("merchant", 3), ("none", 1), ("none", 1), ("none", 1)]);
    let town = s.add_town("Town", 50.0);
    s.set_roster(town, &[("merchant", 2), ("none", 1), ("none", 1), ("farmer", 2), ("farmer", 1), ("mason", 1)]);
    let hex = Hex::new(0, 0);
    s.place_town(capital, hex.center_x(), hex.center_z());
    s.place_town(town, hex.center_x() + 240.0, hex.center_z());
    let bread = good(&s, "bread");
    s.deliver(capital, bread, 960.0);
    s.deliver(town, bread, 320.0);
    s.add_route(capital, town, 0.2, 0.5, 1.0);
    s.name_capital(capital, 0);
    s
}

#[test]
fn at_war_the_towns_the_capitals_wagons_reach_are_levied_and_their_recruits_reach_the_front() {
    let mut s = kingdom();
    s.mobilise(0, 1000);
    s.advance(DAY);
    assert_eq!(s.declare(Theatre::Faction), 1, "one levy: the town on its road (not the front, nor the sea)");
    let levy = s.contracts().into_iter().find(|c| c.levy.is_some()).unwrap();
    assert_eq!((levy.seller, levy.buyer, levy.levy.unwrap().soldiers), (1, 0, 3),
        "two idle hands and a fifth of four, at most four");
    assert_eq!(levy.levy.unwrap().pay_per_day, 0.25);
    let front = s.army().front.expect("the front opens");
    assert!(s.is_off_map(front));
    let (c, f) = (s.town_hex(0).unwrap(), s.town_hex(front).unwrap());
    assert!(((c.center_z() - f.center_z()) - 1500.0).abs() < 80.0, "1,500 blocks north of the capital");
    // The capital's wagon carries the copy and brings the recruits home; the next to the front takes them on.
    let mut at_front = 0;
    for day in 2..=12 {
        s.advance(day * DAY);
        at_front = s.army().size();
        if at_front == 3 {
            break;
        }
    }
    assert_eq!(at_front, 3, "three soldiers at the front: {:?}", s.army());
    assert_eq!(s.roster(1).hands.iter().filter(|h| h.away).count(), 3, "away from their town, on its roster");
    assert_eq!(s.contracts().into_iter().find(|c| c.levy.is_some()).unwrap().levy.unwrap().raised, 3);
    let tally = s.war_tally();
    let town = tally.towns.iter().find(|t| t.town == 1).expect("the town in the tally");
    assert_eq!(town.raised, 3, "the tally counts the soldiers it raised");
    assert_eq!(tally.levies, 1);
    assert!(tally.minted >= 1000.0);
    // Their pay, a quarter a soldier-day, is owed the town under the levy, and the capital's wagons carry it there.
    let day = s.now() / DAY;
    s.advance((day + 6) * DAY);
    let c = s.contracts().into_iter().find(|c| c.levy.is_some()).unwrap();
    assert!(c.paid > 0.0, "levy pay carried to the town: {} paid, {} owing", c.paid, c.owing);
    assert!((c.paid + c.owing).millis() % 250 == 0, "in soldier-days at 0.25: {}", c.paid + c.owing);
    // Peace: they walk home, the minting falls back to the revenue, the levy closes.
    let serving = s.army().size() + s.army().waiting();
    assert_eq!(s.peace(), serving);
    assert_eq!(s.sovereign().mint_per_day, 0);
    assert!(s.contracts().iter().filter(|c| c.levy.is_some()).all(|c| c.state == State::Completed));
    s.advance(20 * DAY);
    assert_eq!(s.roster(1).hands.iter().filter(|h| h.away).count(), 0, "home again");
}

#[test]
fn mobilising_the_agent_requisitions_what_the_stockpile_lacks_for_the_period() {
    let mut s = kingdom();
    s.mobilise(0, 1000);
    s.advance(DAY + 1200);
    let reqs = s.requisitions();
    let sword = good(&s, "diamond_sword");
    let r = reqs.iter().find(|r| r.goods == [sword]).expect("diamond swords requisitioned");
    assert_eq!((r.wanted, r.by_points, r.buyer), (16.0, false, 0));
    assert_eq!(r.due - r.posted, 5 * DAY, "for the period, five days");
    assert!((r.ceiling(s.content(), sword) - s.content().goods[sword].value * 1.1 * 1.1 * 1.5).abs() < 1e-9,
        "at the crown's price, mobilising's premium");
    let meat = reqs.iter().find(|r| r.by_points).expect("meat, one requisition of a kind");
    assert_eq!(meat.goods, [good(&s, "cooked_beef"), good(&s, "cooked_porkchop")]);
    assert_eq!(meat.wanted, 1920.0, "in food points: half the twenty days' food for 24");
    assert!(reqs.iter().all(|r| r.goods != [good(&s, "bread")]), "bread: the capital holds its 384 and more");
    // What's under contract counts: a sword drafted for the capital is one fewer to requisition.
    assert!(s.sovereign().shortfall.iter().any(|&(g, n)| g == sword && n == 16.0));
}

#[test]
fn the_crown_tenders_for_the_masters_its_realm_lacks() {
    let mut s = realm();
    s.mobilise(0, 1000);
    s.advance(DAY + 1200);
    let tenders = s.board(0);
    let smith = s.content().trade("weaponsmith").unwrap();
    let t = tenders.iter().find(|t| t.work.trade == smith).expect("a tender for master weaponsmiths");
    assert_eq!((t.open, t.work.level, t.work.offtake), (2, 5, 1.0));
    assert!(t.work.bonus == 24.0 * 1.5, "24 a master, at mobilising's premium: {}", t.work.bonus);
    assert_eq!(t.span, 15 * DAY, "due in fifteen days");
    let butcher = s.content().trade("butcher").unwrap();
    assert!(tenders.iter().any(|t| t.work.trade == butcher && t.work.level == 2 && t.open == 2), "and Butchers");
    // Untaken, it lapses, and is posted again for a quarter more.
    s.advance(16 * DAY);
    let again = s.board(0).into_iter().find(|t| t.work.trade == smith).expect("posted again");
    assert!(again.work.bonus == 24.0 * 1.5 * 1.25, "a quarter more: {}", again.work.bonus);
}

#[test]
fn short_of_its_planned_army_the_crown_hires_mercenaries_one_contract_at_a_time() {
    let mut s = kingdom();
    s.mobilise(0, 1000);
    s.declare(Theatre::Faction);
    let sea = s.exchange().overseas.unwrap();
    assert_eq!(s.hire_mercenaries(), 8, "24 planned, 3 levied: 21 short, at most 8 a contract");
    assert_eq!(s.hire_mercenaries(), 0, "one open at a time");
    let hire = s.contracts().into_iter().find(|c| c.seller == sea && c.levy.is_some()).unwrap();
    assert_eq!((hire.levy.unwrap().soldiers, hire.levy.unwrap().pay_per_day, hire.buyer), (8, 1.0, 0));
}

#[test]
fn the_fronts_quartermaster_orders_hearty_food_first_and_kit_from_the_capital() {
    let mut s = kingdom();
    let (beef, bread, sword) = (good(&s, "cooked_beef"), good(&s, "bread"), good(&s, "iron_sword"));
    s.deliver(0, beef, 64.0);
    s.deliver(0, sword, 20.0);
    s.record_stocktake(0, &[(beef, 64.0), (bread, 960.0), (sword, 20.0)], Some(0));
    s.mobilise(0, 1000);
    s.declare(Theatre::Faction);
    s.enlist(1, 2); // two of the town's hands at the front at once
    s.advance(DAY + 1200);
    let front = s.army().front.unwrap();
    let orders: Vec<_> = s.contracts().into_iter().filter(|c| c.seller == 0 && c.buyer == front).collect();
    let beef_order = orders.iter().find(|c| c.good == beef).expect("hearty food first");
    assert_eq!(beef_order.state, State::Active, "the crown's own order: in force at once");
    assert_eq!((beef_order.price, beef_order.incoterm), (0.0, Incoterm::Dap));
    let swords = orders.iter().find(|c| c.good == sword).expect("a sword each");
    assert_eq!(swords.quantity, 2.0);
    let rations = s.army().wants(s.content(), s.front_ration_days());
    let want = rations.iter().find(|(g, _)| *g == bread).unwrap().1 as f64 * 5.0;
    let ordered: f64 = orders.iter().filter(|c| s.content().goods[c.good].food > 0.0)
        .map(|c| c.quantity * s.content().goods[c.good].food).sum();
    assert!(ordered >= want && ordered < want + 8.0 + 5.0, "rations for the round trips: {ordered} of {want} points");
}

#[test]
fn the_war_tally_counts_soldiers_kit_and_food() {
    let mut s = kingdom();
    s.mobilise(0, 1000);
    s.declare(Theatre::Faction);
    let front = s.army().front.unwrap();
    let bread = good(&s, "bread");
    s.deliver(front, bread, 40.0);
    s.enlist(1, 2);
    s.advance(3 * DAY);
    let tally = s.war_tally();
    let town = tally.towns.iter().find(|t| t.town == 1).unwrap();
    let levied = s.contracts().into_iter().find(|c| c.levy.is_some()).unwrap().levy.unwrap().raised;
    assert_eq!(town.raised, 2 + levied, "the two sent at once, and those its levy raised that reached the front");
    assert!(town.days >= 4 && town.fallen + town.deserted <= town.raised);
    assert!(tally.eaten > 0.0, "food eaten: {}", tally.eaten);
    assert!(tally.issued.is_empty() || tally.issued.values().all(|n| *n > 0));
    assert!(tally.rate > 16.0, "the spur's rate, debased: {}", tally.rate);
}
