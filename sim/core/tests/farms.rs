//! Farms, pinned as the mod's territory, labour, jobs and mood tests pin them: plans, wages, credit and
//! share-cropping, the labour market, fields, stores, and the households' savings on call.

use std::collections::BTreeMap;

use enterprise_core::cash_book::{Account, Reason};
use enterprise_core::farms::{self, Farm, Focus, Store, Wants};
use enterprise_core::state::{Ledger, Stock, Town};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;

/// The mod's crops at their locker prices (`Prices`: wheat 20 to the emerald, carrots 22, potatoes 26, beetroot 15,
/// cane 24 as paper, berries 10), paper, and bread (5 food points).
fn content() -> Content {
    Content::new(vec![
        Good::new("wheat", 1.0 / 20.0, 64).crop(),
        Good::new("carrot", 1.0 / 22.0, 64).crop().food(3.0),
        Good::new("potato", 1.0 / 26.0, 64).crop().food(1.0),
        Good::new("beetroot", 1.0 / 15.0, 64).crop().food(1.0),
        Good::new("sugar_cane", 1.0 / 24.0, 64).crop(),
        Good::new("sweet_berries", 1.0 / 10.0, 64).crop().food(2.0),
        Good::new("paper", 1.0 / 24.0, 64),
        Good::new("bread", 1.0 / 6.0, 64).food(5.0),
    ], DAY)
}

const WHEAT: usize = 0;
const CARROT: usize = 1;
const POTATO: usize = 2;
const BEETROOT: usize = 3;
const CANE: usize = 4;
const BERRIES: usize = 5;
const PAPER: usize = 6;
const BREAD: usize = 7;

/// The mod's test farm (`TerritoryTests.farm`): 20 wheat plots, a level-1 Farmer, and a village that wants wheat, with
/// `money` in the treasury. Its one villager has a bed and its size costs no mood, so its mood holds at 0 while its
/// Farmers are paid (the pace the mod's test reads).
fn farm(money: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.villagers_per_want = f64::INFINITY;
    let t = s.add_town("Meadowworth", money);
    s.set_roster(t, &[("farmer", 1)]);
    s.set_people(t, 1.0, 1.0, 1.0);
    s.set_farm_demand(t, WHEAT, 100.0);
    s.set_plots(t, WHEAT, 20);
    s
}

/// The core looks at its towns every `days`: as often as the mod's test touches its market.
fn touch_every(s: &mut Society, days: u64) {
    s.content_mut().dials.look_ticks = days * DAY;
}

fn the_farm(s: &Society) -> Farm {
    s.farm(0).expect("a farm").farm
}

fn per_day(s: &Society, good: usize) -> f64 {
    the_farm(s).per_day.get(&good).copied().unwrap_or(0.0)
}

/// Takes `n` of a good out of a town's warehouse, as a buyer would.
fn take(s: &mut Society, town: usize, good: usize, n: f64) {
    let mut q = s.world.query::<(&Town, &mut Stock)>();
    for (t, mut stock) in q.iter_mut(&mut s.world) {
        if t.id == town {
            stock.0[good] -= n;
        }
    }
    s.world.resource_mut::<Ledger>().used[good] += n;
}

// parity-partial: TerritoryTests.farmersProduceWhatGrowsInTheirHex
#[test]
fn a_farmer_works_32_plots_a_level_half_an_item_each() {
    let c = content();
    let field = |turned: &[(usize, i64)]| Farm {
        plots: [(WHEAT, 20)].into_iter().collect(),
        turned: turned.iter().copied().collect(),
        ..Default::default()
    };
    let wants = Wants::default();
    let plan = |f: &Farm, farmers: &[u8]| farms::plan(&c, &farms::plots(f), farmers, &wants, 1.0, false, &mut BTreeMap::new()).0;
    let wheat = plan(&field(&[]), &[1]).get(&WHEAT).copied().unwrap_or(0.0);
    assert_eq!(wheat, 20.min(farms::PLOTS_PER_FARMER_LEVEL) as f64 * farms::YIELD_PER_PLOT_PER_DAY, "half a wheat a plot");
    assert!(plan(&field(&[]), &[]).is_empty(), "no farmers, no produce");
    // Fields turned over on the books count as their new crop.
    let turned = plan(&field(&[(WHEAT, -10_000), (BEETROOT, 10)]), &[1]);
    assert!(turned.get(&WHEAT).copied().unwrap_or(0.0) == 0.0 && turned[&BEETROOT] >= 10.0 * farms::YIELD_PER_PLOT_PER_DAY,
        "the wheat turned over to beetroot on the books: {turned:?}");
    // A bigger farm: a level-3 Farmer works 96 of 200 plots, the rest untended.
    let big = Farm { plots: [(WHEAT, 200)].into_iter().collect(), ..Default::default() };
    let mut idle = BTreeMap::new();
    let (p, _) = farms::plan(&c, &farms::plots(&big), &[3], &wants, 1.0, false, &mut idle);
    assert_eq!((p[&WHEAT], idle[&WHEAT]), (48.0, 52.0));
}

// parity-partial: TerritoryTests.produceAndTollsLandInTheMerchantsChest
#[test]
fn produce_lands_in_the_warehouse_on_touch() {
    let mut s = Society::new(content());
    s.content_mut().dials.villagers_per_want = f64::INFINITY;
    // The Farmer's baking (wheat into bread for a hungry town) is the workshop's (tests/workshop.rs): here the
    // wheat stays wheat.
    s.content_mut().conversions.clear();
    let t = s.add_town("Meadowworth", 100.0);
    s.set_roster(t, &[("farmer", 1)]);
    s.set_people(t, 1.0, 1.0, 1.0);
    s.set_plots(t, WHEAT, 20);
    let perday = per_day(&s, WHEAT);
    assert!(perday >= 9.0, "the farmer works the wheat plots, got {perday} a day");
    s.advance(2 * DAY);
    assert_eq!(s.town(t).stock[WHEAT], (perday * 2.0).floor(), "two days of wheat");
}

// parity: TerritoryTests.farmersEarnHalfWhatTheyGrow
#[test]
fn farmers_earn_half_what_they_grow() {
    let s = farm(0.0);
    let c = content();
    let expected: f64 = the_farm(&s).per_day.iter().map(|(g, n)| n * c.goods[*g].value).sum::<f64>() * farms::LABOR_SHARE;
    let wage = s.farm(0).unwrap().wage_per_day;
    assert!((wage - expected).abs() < 1e-9, "wage {wage} is half the value of {} wheat a day", per_day(&s, WHEAT));
    assert!((wage - 0.25).abs() < 1e-12, "10 wheat a day at 20 to the emerald, halved");
}

// parity: TerritoryTests.paidFarmersDeliverTheWholeHarvest
#[test]
fn paid_farmers_deliver_the_whole_harvest() {
    let mut s = farm(10.0);
    touch_every(&mut s, 10);
    let perday = per_day(&s, WHEAT);
    let owed = (s.farm(0).unwrap().wage_per_day * 10.0).floor();
    s.advance(10 * DAY);
    assert_eq!(s.town(0).money, 10.0 - owed, "ten days of wages ({owed} emeralds) paid from the treasury");
    assert_eq!(s.town(0).people.purse, owed, "into the households' purse");
    assert_eq!(s.town(0).stock[WHEAT], (perday * 10.0).floor(), "and the whole ten days' harvest delivered");
    assert_eq!(the_farm(&s).wages_unpaid, 0.0, "nothing unpaid");
    assert!(s.cash_book(0, 20).iter().any(|(_, r, v)| r == "wages" && *v < 0.0), "posted as wages");
}

// parity: TerritoryTests.unpaidFarmersWorkOnCreditThenTakeTheirShare
#[test]
fn unpaid_farmers_work_on_credit_then_take_their_share() {
    let mut s = farm(0.0);
    touch_every(&mut s, 10);
    let perday = per_day(&s, WHEAT);
    let cap = farms::CREDIT_DAYS * perday / 20.0 * farms::LABOR_SHARE;
    s.advance(10 * DAY);
    assert_eq!(s.town(0).stock[WHEAT], (perday * 10.0).floor(), "no coin: the farmers work on credit, the whole harvest in");
    assert!(the_farm(&s).credit >= 1.0, "the town owes them: {}", the_farm(&s).credit);
    for touch in 2..=8 {
        s.advance(touch * 10 * DAY);
    }
    assert!(the_farm(&s).credit <= enterprise_core::Fixed::floor(cap), "credit stops at ten days' customary pay: {} of {cap}", the_farm(&s).credit);
    assert!(the_farm(&s).wages_unpaid >= 1.0, "then the wages go unpaid, recorded");
    let before = s.town(0).stock[WHEAT];
    let worked = farms::yield_factor(s.town(0).people.mood);
    s.advance(90 * DAY);
    let delivered = s.town(0).stock[WHEAT] - before;
    let share = s.farm(0).unwrap().wage_per_day / (perday / 20.0);
    assert!((delivered - perday * 10.0 * worked * (1.0 - share)).abs() <= 2.0,
        "the farmers keep their share of the crop, the town the rest: {delivered} of {} at a share of {share}", perday * 10.0);
}

/// Ten days, one a day; when `selling`, the whole day's harvest sells (`TerritoryTests.tenDays`).
fn ten_days(selling: bool) -> f64 {
    let mut s = farm(5.0 * 64.0);
    touch_every(&mut s, 1);
    let c = content();
    let day_value: f64 = the_farm(&s).per_day.iter().map(|(g, n)| n * c.goods[*g].value).sum();
    for day in 1..=10 {
        if selling {
            s.record_crop_sale(0, day_value);
        }
        s.advance(day * DAY);
    }
    the_farm(&s).labor_share
}

// parity: TerritoryTests.wagesClimbTowardEverythingTheHarvestSellsFor
#[test]
fn wages_climb_toward_everything_the_harvest_sells_for() {
    let share = ten_days(true);
    assert!(share > farms::LABOR_SHARE + 0.05, "farmers' share climbs when the whole harvest sells: {share}");
}

// parity: TerritoryTests.wagesEaseToTheFloorWhenNothingSells
#[test]
fn wages_ease_to_the_floor_when_nothing_sells() {
    let share = ten_days(false);
    assert!((farms::MIN_LABOR_SHARE..0.27).contains(&share), "farmers' share eases toward 25% when nothing sells: {share}");
}

/// The town's own lockers' crop sales are read from its cash book: what its people paid for the wheat they use.
#[test]
fn crop_sales_at_the_towns_lockers_are_read_from_its_cash_book() {
    let mut s = farm(320.0);
    s.set_trade(0, WHEAT, 0.0, 10.0);
    s.deliver(0, WHEAT, 1000.0);
    s.advance(20 * DAY);
    let f = the_farm(&s);
    assert!((0.48..0.51).contains(&f.sales_per_day), "10 wheat a day sold, half an emerald: {}", f.sales_per_day);
    assert!(f.labor_share > farms::LABOR_SHARE, "they sell, so the share climbs: {}", f.labor_share);
}

// parity: TerritoryTests.aGranaryBanksTheWheatTheWarehouseNeedNotKeep
#[test]
fn a_granary_banks_the_wheat_the_warehouse_need_not_keep() {
    // A farm with nothing growing this look (the mod's test touches it with no time gone by).
    let mut s = Society::new(content());
    let t = s.add_town("Meadowworth", 0.0);
    s.build_store(t, Store::Granary);
    s.deliver(t, WHEAT, 6.0 * 64.0);
    s.advance(1200);
    assert_eq!((s.town(t).stock[WHEAT], s.banked(t, WHEAT)), (128.0, 256.0), "two stacks kept loose, the rest banked");
    take(&mut s, t, WHEAT, 100.0);
    s.advance(2400);
    assert_eq!((s.town(t).stock[WHEAT], s.banked(t, WHEAT)), (128.0, 156.0), "drawn on, the granary tops the warehouse up");
    let mut before = s.totals();
    s.advance(DAY);
    s.conserved(&before).unwrap();
    before = s.totals();
    take(&mut s, t, WHEAT, 128.0);
    s.advance(2 * DAY);
    s.conserved(&before).unwrap();
}

// parity: TerritoryTests.aRootCellarIsWantedWhereRootsPileUpAndBanksThem
#[test]
fn a_root_cellar_is_wanted_where_roots_pile_up_and_banks_them() {
    let mut s = Society::new(content());
    let t = s.add_town("Meadowworth", 0.0);
    s.set_farm_demand(t, WHEAT, 100.0);
    s.deliver(t, CARROT, 64.0);
    assert!(!s.wants_store(t, Store::RootCellar), "a stack of carrots: no need of a cellar");
    s.deliver(t, CARROT, 5.0 * 64.0);
    assert!(s.wants_store(t, Store::RootCellar), "six stacks of carrots: a cellar is wanted");
    assert!(!s.wants_store(t, Store::Granary), "but not a granary: no wheat piles up");
    s.advance(1200);
    assert_eq!(the_farm(&s).stores_wanted, vec![Store::RootCellar], "the review notes it wanted");
    s.build_store(t, Store::RootCellar);
    s.advance(2400);
    assert_eq!((s.town(t).stock[CARROT], s.banked(t, CARROT)), (128.0, 256.0), "two stacks kept loose, the rest in the cellar");
    assert_eq!(s.banked(t, WHEAT), 0.0, "the cellar keeps roots, not wheat");
}

/// Produce lands straight in the store beyond what the warehouse keeps loose.
#[test]
fn a_harvest_goes_to_the_store_beyond_what_the_warehouse_keeps_loose() {
    let mut s = farm(100.0);
    s.content_mut().conversions.clear(); // no baking: the wheat stays wheat
    s.build_store(0, Store::Granary);
    s.deliver(0, WHEAT, 120.0);
    s.advance(10 * DAY);
    let (loose, banked) = (s.town(0).stock[WHEAT], s.banked(0, WHEAT));
    assert_eq!(loose, 128.0, "the warehouse topped up to two stacks");
    assert_eq!(loose + banked, 120.0 + 100.0, "the rest of ten days' wheat banked: {banked}");
}

// parity: TerritoryTests.aMerchantBorrowsItsHouseholdsSavingsUpToItsShare
#[test]
fn the_treasury_borrows_its_households_savings_up_to_its_share() {
    let mut s = farm(0.0);
    s.post(Account::Outside, Account::Households(0), 100.0, Reason::PaidIn);
    let on_call = s.deposits_on_call(0);
    assert!(on_call == 100.0 * s.content().dials.deposit_share, "its share of their savings on call: {on_call}");
    let before = s.town(0).money;
    let got = s.borrow_deposits(0, 30.0);
    assert!(got == 30.0 && s.town(0).money - before == 30.0 && s.deposits(0) == 30.0, "30 borrowed, owed them: {got}");
    let more = s.borrow_deposits(0, 100.0);
    assert!(more == 20.0, "no more than its share in all: {more}");
}

/// What it borrowed earns interest, and is repaid from what it can spare beyond its reserve and a keep of 10.
#[test]
fn the_treasury_repays_its_households_with_interest_beyond_its_keep() {
    let mut s = Society::new(content());
    let t = s.add_town("Meadowworth", 0.0);
    s.post(Account::Outside, Account::Households(t), 100.0, Reason::PaidIn);
    assert_eq!(s.borrow_deposits(t, 30.0), 30.0);
    s.advance(1200);
    let i = 1.0 + s.content().dials.deposit_interest_per_day;
    let owed = 30.0 * i.powf(0.05) - 20.0;
    assert!((s.deposits(t) - owed).abs() < 1e-9, "20 repaid at once (30 held, 10 kept): {} owed", s.deposits(t));
    assert!(s.town(t).money == 10.0 && s.town(t).people.purse == 90.0);
    s.advance(10 * DAY);
    let owed = owed * i.powf(9.95);
    assert!((s.deposits(t) - owed).abs() < 1e-9, "the rest gathers interest: {} of {owed}", s.deposits(t));
    s.pay(t, 50.0);
    s.advance(10 * DAY + 1200);
    assert_eq!(s.deposits(t), 0.0, "repaid once there's money to spare");
    assert!(s.town(t).people.purse > 100.0, "with interest: {}", s.town(t).people.purse);
}

// parity: LabourTests.wagesFollowTheLabourMarketAndOwnersTakeMore
#[test]
fn wages_follow_the_labour_market_and_owners_take_more() {
    assert_eq!(farms::tightness(100.0, 100.0, 0.0), 1.0, "every hand busy, no plot idle");
    assert_eq!(farms::tightness(100.0, 100.0, 50.0), 1.5, "plots going untended: the wage rises, up to half again");
    assert_eq!(farms::tightness(100.0, 40.0, 0.0), 0.5, "hands idle: the wage falls, to half at most");
    assert_eq!(farms::share(0.25, 0.0), 0.25, "hands on the estate: the wage");
    assert_eq!(farms::share(0.25, 1.0), 0.75, "owners take three quarters");
    assert_eq!(farms::share(0.9, 1.0), 0.9, "or the wage, if it's more");
}

fn wants(demand: &[(usize, f64)]) -> Wants {
    Wants { demand: demand.iter().copied().collect(), ..Default::default() }
}

// parity: JobsTests.fieldsGrowWhatTheVillageWouldBuy
#[test]
fn fields_grow_what_the_village_would_buy() {
    let c = content();
    let food = 128.0 * 5.0; // 128 bread: fed
    let none = BTreeMap::new();
    let choose = |w: &Wants, grown: &BTreeMap<usize, f64>, food_only: bool| farms::choose(&c, w, grown, food, food_only);
    assert_eq!(choose(&wants(&[]), &none, false), None, "nobody buys a crop, enough food: no field");
    assert_eq!(choose(&wants(&[(PAPER, 48.0)]), &none, false), Some(CANE), "Librarians buy paper: cane");
    let both = wants(&[(PAPER, 48.0), (POTATO, 640.0)]);
    assert_eq!(choose(&both, &none, false), Some(POTATO), "a Farmer buys more in potatoes");
    let potatoes: BTreeMap<usize, f64> = [(POTATO, 640.0)].into_iter().collect();
    assert_eq!(choose(&both, &potatoes, false), Some(CANE), "potatoes grown already: cane next");
    assert_eq!(choose(&wants(&[(BERRIES, 20.0)]), &none, false), Some(BERRIES), "a Master Butcher: berries");
    assert_ne!(choose(&both, &potatoes, true), Some(CANE), "a town still growing: no cane yet");
    let biscuits = wants(&[(POTATO, 640.0), (CANE, 4.0)]);
    assert_eq!(choose(&biscuits, &potatoes, true), Some(CANE), "its workshop's cane: planted while it grows");
    assert_eq!(Focus::of(5.0, 8.0, 2000.0), Focus::Grow, "five villagers: it grows first");
    assert_eq!(Focus::of(9.0, 12.0, 100.0), Focus::Grow, "short of food: it grows");
    assert_eq!(Focus::of(9.0, 9.0, 2000.0), Focus::Build, "fed and grown, every bed taken: it builds");
    assert_eq!(Focus::of(9.0, 12.0, 2000.0), Focus::Trade, "and with room to grow: it trades");
    // Short of food with nothing wanted: carrots.
    assert_eq!(farms::choose(&c, &wants(&[]), &none, 100.0, true), Some(CARROT), "short of food: carrots");
}

// parity: MoodTests.contentFarmersGrowMore
#[test]
fn content_farmers_grow_more() {
    use farms::{culture_factor, yield_factor};
    assert!(yield_factor(0.0) == 1.0 && yield_factor(3.0) > 1.29 && yield_factor(-3.0) < 0.71, "a tenth more or less a point");
    assert!(yield_factor(20.0) == 1.5 && yield_factor(-20.0) == 0.5, "within half either way");
    assert_eq!(culture_factor(2.0), 1.5, "a quarter more culture a point");
}

/// Farmers with hands to spare get a field of what the village would buy, for 4 emeralds of seed and labour.
#[test]
fn farmers_with_hands_to_spare_get_a_field_of_what_sells() {
    let mut s = Society::new(content());
    let t = s.add_town("Meadowworth", 50.0);
    s.set_roster(t, &[("farmer", 1), ("farmer", 1)]);
    s.set_people(t, 2.0, 2.0, 2.0);
    s.set_farm_demand(t, POTATO, 10.0);
    s.advance(1200);
    let f = the_farm(&s);
    assert_eq!((f.plots.get(&POTATO).copied(), f.fields_planted), (Some(80), 1), "a 9 x 9 field round its water");
    assert_eq!(s.town(t).money, 46.0, "for four emeralds");
    s.advance(2 * DAY);
    let f = the_farm(&s);
    assert_eq!(f.fields_planted, 1, "its hands busy now: no more");
    assert_eq!(f.per_day[&POTATO], 32.0, "both Farmers on it: 64 plots");
    assert!(s.cash_book(t, 5).iter().any(|(_, r, v)| r == "a field" && *v == -4.0));
}

/// Fed, plots of a crop no one wants are turned over to one that's wanted, and the rest lie fallow.
#[test]
fn a_fed_town_turns_over_what_no_one_wants_and_leaves_the_rest_fallow() {
    let mut s = Society::new(content());
    let t = s.add_town("Meadowworth", 0.0);
    s.set_roster(t, &[("farmer", 5)]);
    s.set_people(t, 1.0, 1.0, 1.0);
    s.deliver(t, BREAD, 128.0);
    s.set_plots(t, WHEAT, 200);
    s.set_farm_demand(t, CARROT, 20.0);
    s.advance(1200);
    let f = the_farm(&s);
    assert_eq!((f.turned[&WHEAT], f.turned[&CARROT]), (-40, 40), "a field's worth at most, as many as the carrots need");
    assert_eq!(f.per_day[&CARROT], 20.0);
    assert_eq!((f.per_day.get(&WHEAT), f.fallow), (None, 160), "the wheat no one wants lies fallow");
}

/// Owed under contract (the needs), the crop with plots lying idle is turned over, two fields' worth at most.
#[test]
fn a_crop_owed_is_turned_over_from_the_idlest() {
    let c = content();
    let mut f = Farm { plots: [(WHEAT, 300), (CARROT, 100)].into_iter().collect(), ..Default::default() };
    f.idle.insert(CARROT, 30.0);
    f.needs.insert(BEETROOT, 200.0);
    let w = Wants::of(&f, &[0.0; 8]);
    farms::rotate(&c, &mut f, &w, false);
    assert_eq!((f.turned.get(&CARROT).copied(), f.turned.get(&BEETROOT).copied()), (Some(-100), Some(100)),
        "from the carrots lying idle, all of theirs (up to 162)");
}

/// A household with eight emeralds buys a field of its own where the Farmers have hands for it; on it they take at
/// least three quarters.
#[test]
fn a_household_buys_a_field_of_its_own_and_owners_take_more() {
    let mut s = Society::new(content());
    let t = s.add_town("Meadowworth", 0.0);
    s.set_roster(t, &[("farmer", 3)]);
    s.set_people(t, 1.0, 1.0, 1.0);
    s.set_plots(t, WHEAT, 20);
    s.post(Account::Outside, Account::Households(t), 10.0, Reason::PaidIn);
    s.advance(1200);
    let f = the_farm(&s);
    assert_eq!((f.freeholds, f.plots.get(&CARROT).copied()), (80, Some(80)), "nothing wanted, short of food: carrots");
    assert!(s.town(t).people.purse == 2.0 && s.town(t).money == 8.0, "paid the town");
    s.advance(DAY + 1200);
    let v = s.farm(t).unwrap();
    let value = farms::value_per_day(s.content(), &v.farm.per_day);
    assert!(v.wage_per_day > value * v.farm.labor_share, "owners take more: {} of {value}", v.wage_per_day);
}

/// Unpaid farm wages sour the town's people as other unpaid wages do.
#[test]
fn unpaid_farmers_sour_the_mood() {
    let mut s = Society::new(content());
    // No baking: bread from the wheat would feed the town, and its unwanted wheat would lie fallow.
    s.content_mut().conversions.clear();
    let t = s.add_town("Meadowworth", 0.0);
    s.set_roster(t, &[("farmer", 1)]);
    s.set_people(t, 1.0, 1.0, 1.0);
    s.set_plots(t, WHEAT, 20);
    s.advance(60 * DAY);
    assert_eq!(the_farm(&s).paid_share, 0.0, "credit spent, nothing paid");
    assert_eq!(s.town(t).people.paid_share, 0.0, "the town's people count it");
}

/// The farm and the households' deposits survive a save, and run on the same.
#[test]
fn a_farm_saves_and_loads_to_the_bit() {
    let mut s = farm(3.0);
    s.build_store(0, Store::Granary);
    s.post(Account::Outside, Account::Households(0), 40.0, Reason::PaidIn);
    s.borrow_deposits(0, 15.0);
    s.advance(7 * DAY + 600);
    let mut loaded = Society::load(&s.save()).unwrap();
    assert!(s.unsaved().is_empty());
    s.advance(30 * DAY);
    loaded.advance(30 * DAY);
    assert_eq!(format!("{:?}", s.farm(0).unwrap().farm), format!("{:?}", loaded.farm(0).unwrap().farm));
    assert_eq!(s.deposits(0).to_bits(), loaded.deposits(0).to_bits());
}

/// Fields go only where the town has room: the ring of ground round its bell (the mod's site search, counted).
#[test]
fn fields_stop_where_the_town_has_no_more_room() {
    let mut s = Society::new(content());
    let t = s.add_town("Meadowworth", 0.0);
    s.set_roster(t, &[("farmer", 5), ("farmer", 5), ("farmer", 5)]);
    s.set_people(t, 3.0, 3.0, 3.0);
    s.deliver(t, BREAD, 128.0);
    s.set_plots(t, WHEAT, 0);
    s.post(Account::Outside, Account::Households(t), 1000.0, Reason::PaidIn);
    s.advance(100 * DAY);
    let f = the_farm(&s);
    let ground: u32 = f.plots.values().sum();
    assert!(ground as f64 <= farms::farmland(), "{ground} plots in {} blocks", farms::farmland());
    assert_eq!(f.fields_planted, (farms::farmland() / 81.0).floor() as u32, "as many fields as the ring holds");
}

/// Unpaid Farmers who stop working are owed nothing more: the share paid goes back to whole, and the town's people
/// with it, rather than standing at the last settlement's for good.
#[test]
fn farmers_with_no_plan_left_are_not_left_unpaid() {
    let mut s = farm(0.0);
    s.advance(30 * DAY);
    assert!(the_farm(&s).paid_share < 1.0, "past their credit, the Farmers go unpaid: {}", the_farm(&s).paid_share);
    s.set_plots(0, WHEAT, 0);
    s.advance(31 * DAY);
    assert!(the_farm(&s).per_day.is_empty(), "no plots, no plan");
    assert_eq!(the_farm(&s).paid_share, 1.0, "no wage running, nothing unpaid");
    assert_eq!(s.town(0).people.paid_share, 1.0, "and the town's people paid as far as the farm goes");
}
