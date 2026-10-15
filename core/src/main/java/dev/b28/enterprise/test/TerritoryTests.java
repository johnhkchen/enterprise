package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.caravan.Route;
import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.MoneyChanger;
import dev.b28.enterprise.settlement.Census;
import dev.b28.enterprise.territory.Territory;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

// Each test keeps its own Territory, so tests side by side don't claim each other's hexes.
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class TerritoryTests {
    private static final BlockPos BELL = new BlockPos(3, 2, 3);
    private static final Census FIVE_FREE = new Census(5, 5, 5, 5, 0); // value 5

    @GameTest(template = "floor")
    public static void homeHexFirstThenGrowthWithCulture(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL));
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), FIVE_FREE, 1, List.of());
        // Home hex free; 5 culture buys the next hex (5 x 1 held).
        helper.assertTrue(territory.hexes(bell).size() == 2, "home plus one, got " + territory.hexes(bell).size());
        helper.assertTrue(territory.hexes(bell).contains(Hex.at(helper.absolutePos(BELL))), "the home hex is the bell's");
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), FIVE_FREE, 2, List.of());
        // 10 more culture buys a third hex (5 x 2 held).
        helper.assertTrue(territory.hexes(bell).size() == 3, "a third hex after two more days, got " + territory.hexes(bell).size());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void growthStopsAtTwoRings(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL));
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), FIVE_FREE, 1000, List.of());
        Hex home = Hex.at(helper.absolutePos(BELL));
        helper.assertTrue(territory.hexes(bell).size() == 19, "19 hexes at most, got " + territory.hexes(bell).size());
        helper.assertTrue(territory.hexes(bell).stream().allMatch(hex -> hex.distance(home) <= 2), "all within two rings");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aHeldHexStaysWithItsOwner(GameTestHelper helper) {
        Territory territory = new Territory();
        var dimension = helper.getLevel().dimension();
        Hex home = Hex.at(helper.absolutePos(BELL));
        GlobalPos rival = GlobalPos.of(dimension, helper.absolutePos(BELL).offset(500, 0, 0));
        for (Hex hex : home.neighbors()) {
            territory.claim(rival, hex); // the rival already holds the whole first ring
        }
        GlobalPos bell = GlobalPos.of(dimension, helper.absolutePos(BELL));
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), FIVE_FREE, 1000, List.of());
        helper.assertTrue(territory.hexes(bell).equals(List.of(home)), "hemmed in, it keeps only its home hex");
        helper.succeed();
    }

    /** 20 wheat plots on farmland in the arena's corner. Wheat placed on stone would pop off at once. */
    private static void wheatField(GameTestHelper helper) {
        for (int x = 0; x < 5; x++) {
            for (int z = 0; z < 4; z++) {
                helper.setBlock(new BlockPos(x, 1, z), Blocks.FARMLAND);
                helper.setBlock(new BlockPos(x, 2, z), Blocks.WHEAT);
            }
        }
    }

    @GameTest(template = "floor")
    public static void farmersProduceWhatGrowsInTheirHex(GameTestHelper helper) {
        wheatField(helper);
        Hex hex = Hex.at(helper.absolutePos(BELL));
        int plots = 0;
        for (long chunk : dev.b28.enterprise.territory.Crops.chunksOf(hex)) {
            var counted = dev.b28.enterprise.territory.Crops.countChunk(
                    helper.getLevel(), chunk, hex, helper.absolutePos(BELL).getY());
            plots += counted == null ? 0 : counted.getOrDefault(Items.WHEAT, 0);
        }
        helper.assertTrue(plots >= 10, "the field counts its wheat plots (neighbouring tests may trample a few), counted " + plots);
        var plan = new Territory().plan(helper.getLevel(), helper.absolutePos(BELL).getY(), List.of(hex), List.of(1));
        double wheat = plan.getOrDefault("minecraft:wheat", 0.0);
        // Other tests' fields can share this hex; one level-1 farmer works at most 32 plots of it.
        int worked = Math.min(plots, Territory.PLOTS_PER_FARMER_LEVEL);
        helper.assertTrue(wheat == worked * Territory.YIELD_PER_PLOT_PER_DAY,
                "half a wheat per worked plot per day: " + worked * Territory.YIELD_PER_PLOT_PER_DAY + ", got " + wheat);
        helper.assertTrue(new Territory().plan(helper.getLevel(), helper.absolutePos(BELL).getY(), List.of(hex), List.of())
                .isEmpty(), "no farmers, no produce");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void produceAndTollsLandInTheMerchantsChest(GameTestHelper helper) {
        helper.setBlock(new BlockPos(5, 2, 5), Merchant.DESK.get());
        helper.setBlock(new BlockPos(6, 2, 5), Blocks.CHEST);
        wheatField(helper);
        Territory territory = new Territory();
        GlobalPos bell = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL));
        long t = helper.getLevel().getGameTime();
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), FIVE_FREE, 1, List.of(1));
        double perDay = territory.farmPlan(bell).getOrDefault("minecraft:wheat", 0.0);
        helper.assertTrue(perDay >= 9, "the farmer works the wheat plots, got " + perDay + " a day");
        territory.oweToll(bell, 100);

        MerchantStock stock = MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(5, 2, 5)));
        territory.touch(helper.getLevel(), helper.absolutePos(new BlockPos(5, 2, 5)), stock, t + 2 * 24000);
        int wheat = stock.count(new ItemCost(Items.WHEAT, 1));
        helper.assertTrue(wheat == (int) Math.floor(perDay * 2), "two days of wheat: " + perDay * 2 + ", got " + wheat);
        helper.assertTrue(MoneyChanger.value(stock.looseStacks()) == 100, "the 100-spur toll landed as coins");
        helper.assertTrue(territory.tollsOwed(bell) == 0, "and is no longer owed");
        helper.succeed();
    }

    /** A wheat farm with a level-1 farmer, its Merchant's desk and chest; returns the farm's settlement. */
    private static GlobalPos farm(GameTestHelper helper, Territory territory) {
        helper.setBlock(new BlockPos(5, 2, 5), Merchant.DESK.get());
        helper.setBlock(new BlockPos(6, 2, 5), Blocks.CHEST);
        wheatField(helper);
        // The village wants wheat, so its farmer tends the wheat first, whatever a neighbouring test grows in the hex.
        territory.recordDemand(GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL)),
                java.util.Map.of("minecraft:wheat", 100.0));
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), FIVE_FREE, 1, List.of(1));
        return GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL));
    }

    private static MerchantStock farmStock(GameTestHelper helper) {
        return MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(5, 2, 5)));
    }

    @GameTest(template = "floor")
    public static void farmersEarnHalfWhatTheyGrow(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = farm(helper, territory);
        double wheat = territory.farmPlan(bell).get("minecraft:wheat");
        double expected = territory.farmPlan(bell).entrySet().stream() // wheat sells 20 to the emerald
                .mapToDouble(e -> e.getValue() / dev.b28.enterprise.money.Prices.itemsPerEmerald(e.getKey())).sum()
                * Territory.LABOR_SHARE;
        helper.assertTrue(Math.abs(territory.wagePerDay(bell) - expected) < 1e-9,
                "wage " + territory.wagePerDay(bell) + " is half the value of " + wheat + " wheat a day");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void paidFarmersDeliverTheWholeHarvest(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = farm(helper, territory);
        farmStock(helper).insert(new ItemStack(Items.EMERALD, 10));
        long t = helper.getLevel().getGameTime();
        double perDay = territory.farmPlan(bell).get("minecraft:wheat");
        int owed = (int) Math.floor(territory.wagePerDay(bell) * 10);

        territory.touch(helper.getLevel(), helper.absolutePos(new BlockPos(5, 2, 5)), farmStock(helper), t + 10 * 24000);
        helper.assertTrue(farmStock(helper).count(new ItemCost(Items.EMERALD, 1)) == 10 - owed,
                "ten days of wages (" + owed + " emeralds) paid from the chest");
        helper.assertTrue(farmStock(helper).count(new ItemCost(Items.WHEAT, 1)) == (int) Math.floor(perDay * 10),
                "and the whole ten days' harvest delivered");
        helper.assertTrue(territory.wagesUnpaid(bell) == 0, "nothing unpaid");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void unpaidFarmersWorkOnCreditThenTakeTheirShare(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = farm(helper, territory);
        long t = helper.getLevel().getGameTime();
        BlockPos desk = helper.absolutePos(new BlockPos(5, 2, 5));
        double perDay = territory.farmPlan(bell).get("minecraft:wheat");
        double cap = dev.b28.enterprise.territory.Households.CREDIT_DAYS * perDay / 20 * Territory.LABOR_SHARE; // 20 to the emerald
        territory.touch(helper.getLevel(), desk, farmStock(helper), t + 10 * 24000L);
        helper.assertTrue(farmStock(helper).count(new ItemCost(Items.WHEAT, 1)) == (int) Math.floor(perDay * 10),
                "no coin in the chest: the farmers work on credit, the whole harvest in");
        helper.assertTrue(territory.households(bell)[1] >= 1, "the town owes them: " + territory.households(bell)[1]);
        for (int touch = 2; touch <= 8; touch++) {
            territory.touch(helper.getLevel(), desk, farmStock(helper), t + touch * 10 * 24000L);
        }
        helper.assertTrue(territory.households(bell)[1] <= cap + 1e-9,
                "credit stops at ten days' customary pay: " + territory.households(bell)[1] + " of " + cap);
        helper.assertTrue(territory.wagesUnpaid(bell) >= 1, "then the wages go unpaid, recorded");
        int before = farmStock(helper).count(new ItemCost(Items.WHEAT, 1));
        double worked = Territory.yieldFactor(territory.mood(bell)); // unpaid, they're unhappy and work less
        territory.touch(helper.getLevel(), desk, farmStock(helper), t + 9 * 10 * 24000L);
        int delivered = farmStock(helper).count(new ItemCost(Items.WHEAT, 1)) - before;
        double share = territory.wagePerDay(bell) / (perDay / 20);
        helper.assertTrue(Math.abs(delivered - perDay * 10 * worked * (1 - share)) <= 2,
                "and the farmers keep their share of the crop, the town the rest: " + delivered + " of "
                        + perDay * 10 + " at a share of " + share);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aGranaryBanksTheWheatTheWarehouseNeedNotKeep(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = farm(helper, territory);
        var works = dev.b28.enterprise.territory.Works.get(helper.getLevel().getServer());
        works.finish(works.startBuilding(bell, dev.b28.enterprise.territory.Buildings.GRANARY,
                helper.absolutePos(new BlockPos(1, 1, 1))));
        BlockPos desk = helper.absolutePos(new BlockPos(5, 2, 5));
        long t = helper.getLevel().getGameTime();
        for (int i = 0; i < 6; i++) {
            farmStock(helper).insert(new ItemStack(Items.WHEAT, 64));
        }
        territory.touch(helper.getLevel(), desk, farmStock(helper), t);
        int loose = farmStock(helper).count(new ItemCost(Items.WHEAT, 1));
        helper.assertTrue(loose == 128 && territory.grain(bell) == 256,
                "two stacks kept loose, the rest banked: " + loose + " loose, " + territory.grain(bell) + " banked");
        farmStock(helper).take(new ItemCost(Items.WHEAT, 1), 100);
        territory.touch(helper.getLevel(), desk, farmStock(helper), t);
        loose = farmStock(helper).count(new ItemCost(Items.WHEAT, 1));
        helper.assertTrue(loose == 128 && territory.grain(bell) == 156,
                "drawn on, the granary tops the warehouse up: " + loose + " loose, " + territory.grain(bell) + " banked");
        works.forgetForTest(bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void farmersTakeCoinsAtTheRegionsRate(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = farm(helper, territory);
        MoneyChanger.coins(200).forEach(farmStock(helper)::insert);
        long t = helper.getLevel().getGameTime();
        double perDay = territory.farmPlan(bell).get("minecraft:wheat");

        territory.touch(helper.getLevel(), helper.absolutePos(new BlockPos(5, 2, 5)), farmStock(helper), t + 10 * 24000);
        int left = MoneyChanger.value(farmStock(helper).looseStacks());
        helper.assertTrue(left < 200 && left > 0, "wages came out of the coins, with change: " + left + " spurs left");
        helper.assertTrue(farmStock(helper).count(new ItemCost(Items.WHEAT, 1)) == (int) Math.floor(perDay * 10),
                "paid in coins, the farmers delivered the whole harvest");
        helper.succeed();
    }

    /** Ten days of touches, one a day; when {@code selling}, the whole day's harvest sells for emeralds. */
    private static double tenDays(GameTestHelper helper, boolean selling) {
        Territory territory = new Territory();
        GlobalPos bell = farm(helper, territory);
        BlockPos desk = helper.absolutePos(new BlockPos(5, 2, 5));
        GlobalPos deskPos = GlobalPos.of(helper.getLevel().dimension(), desk);
        for (int i = 0; i < 5; i++) {
            farmStock(helper).insert(new ItemStack(Items.EMERALD, 64)); // enough to pay every wage
        }
        // The whole plan's worth a day (a neighbouring test's crops may share the hex and take the farmer).
        double dayValue = territory.farmPlan(bell).entrySet().stream()
                .mapToDouble(e -> e.getValue() / dev.b28.enterprise.money.Prices.itemsPerEmerald(e.getKey())).sum();
        long t = helper.getLevel().getGameTime();
        for (int day = 1; day <= 10; day++) {
            if (selling) {
                territory.recordCropSale(deskPos, dayValue);
            }
            territory.touch(helper.getLevel(), desk, farmStock(helper), t + day * 24000L);
        }
        return territory.laborShare(bell);
    }

    @GameTest(template = "floor")
    public static void wagesClimbTowardEverythingTheHarvestSellsFor(GameTestHelper helper) {
        double share = tenDays(helper, true);
        // Toward what sells (all of it), less for the hands this little farm leaves idle (the labour market).
        helper.assertTrue(share > Territory.LABOR_SHARE + 0.05,
                "farmers' share climbs when the whole harvest sells: " + share);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void wagesEaseToTheFloorWhenNothingSells(GameTestHelper helper) {
        double share = tenDays(helper, false);
        helper.assertTrue(share < 0.27 && share >= Territory.MIN_LABOR_SHARE,
                "farmers' share eases toward 25% when nothing sells: " + share);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void fedAndPaidGuardsStay(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = farm(helper, territory);
        territory.setGuards(bell, 2);
        farmStock(helper).insert(new ItemStack(Items.BREAD, 64));
        farmStock(helper).insert(new ItemStack(Items.EMERALD, 64));
        BlockPos desk = helper.absolutePos(new BlockPos(5, 2, 5));
        long t = helper.getLevel().getGameTime();
        territory.touch(helper.getLevel(), desk, farmStock(helper), t); // starts the upkeep clock
        territory.touch(helper.getLevel(), desk, farmStock(helper), t + 5 * 24000L);
        helper.assertTrue(territory.deserted(bell) == 0, "fed and paid, nobody deserts");
        // 2 guards x 6 food x 5 days = 60 food points, 12 bread.
        helper.assertTrue(farmStock(helper).count(new ItemCost(Items.BREAD, 1)) == 64 - 12,
                "they ate 12 bread, " + (64 - farmStock(helper).count(new ItemCost(Items.BREAD, 1))) + " eaten");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void unfedGuardsDesert(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = farm(helper, territory);
        territory.setGuards(bell, 4);
        farmStock(helper).insert(new ItemStack(Items.EMERALD, 64)); // paid, but no food
        BlockPos desk = helper.absolutePos(new BlockPos(5, 2, 5));
        long t = helper.getLevel().getGameTime();
        territory.touch(helper.getLevel(), desk, farmStock(helper), t);
        territory.touch(helper.getLevel(), desk, farmStock(helper), t + 3 * 24000L);
        helper.assertTrue(territory.guards(bell) < 4 && territory.deserted(bell) > 0,
                "hungry guards desert: " + territory.guards(bell) + " left");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void caravansPayTollsOnlyToLandTheyCross(GameTestHelper helper) {
        var dimension = helper.getLevel().dimension();
        CaravanLedger ledger = new CaravanLedger();
        // A route running 1,000 blocks east; two other settlements hold hexes on the way.
        Route route = ledger.addRoute(GlobalPos.of(dimension, new BlockPos(0, 64, 0)),
                GlobalPos.of(dimension, new BlockPos(1000, 64, 0)));
        Territory territory = new Territory();
        GlobalPos first = GlobalPos.of(dimension, new BlockPos(300, 64, 300));
        GlobalPos second = GlobalPos.of(dimension, new BlockPos(700, 64, 300));
        territory.claim(first, Hex.at(new BlockPos(300, 64, 0)));
        territory.claim(first, Hex.at(new BlockPos(380, 64, 0))); // two hexes, one toll
        territory.claim(second, Hex.at(new BlockPos(700, 64, 0)));
        territory.claim(GlobalPos.of(dimension, new BlockPos(500, 64, 900)), Hex.at(new BlockPos(500, 64, 900)));
        territory.setGuards(first, 2); // every hex they hold is guarded: the full 5% is collected
        territory.setGuards(second, 1);

        int paid = Caravans.payTolls(territory, route, 1000);
        helper.assertTrue(paid == 50 + 47, "5% to the first (50), then 5% of the rest to the second (47); paid " + paid);
        helper.assertTrue(territory.tollsOwed(first) == 50 && territory.tollsOwed(second) == 47, "each owner is owed its toll");
        helper.succeed();
    }
}
