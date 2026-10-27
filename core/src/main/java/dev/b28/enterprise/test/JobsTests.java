package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.territory.Jobs;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class JobsTests {
    private static double marbles(Map<Jobs.Trade, Double> bag, VillagerProfession profession) {
        return bag.entrySet().stream().filter(e -> e.getKey().profession() == profession)
                .mapToDouble(Map.Entry::getValue).sum();
    }

    @GameTest(template = "floor")
    public static void aMerchantValuesALockerByWhatItsTradesWouldDoForTheTown(GameTestHelper helper) {
        var offers = new net.minecraft.world.item.trading.MerchantOffers();
        offers.add(new net.minecraft.world.item.trading.MerchantOffer(
                new net.minecraft.world.item.trading.ItemCost(net.minecraft.world.item.Items.CARROT, 22),
                new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.EMERALD), 16, 2, 0.05f));
        offers.add(new net.minecraft.world.item.trading.MerchantOffer(
                new net.minecraft.world.item.trading.ItemCost(net.minecraft.world.item.Items.EMERALD, 1),
                new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.BREAD, 6), 16, 1, 0.05f));
        double some = Lockers.worth(offers, Map.of("minecraft:carrot", 5.0), Set.of());
        helper.assertTrue(some == 5, "carrots worth 5 emeralds to sell: worth 5 a restock, not " + some);
        double many = Lockers.worth(offers, Map.of("minecraft:carrot", 100.0), Set.of());
        helper.assertTrue(many == 16, "a glut: as much as its 16 uses pay, not " + many);
        double fed = Lockers.worth(offers, Map.of(), Set.of(net.minecraft.world.item.Items.BREAD));
        helper.assertTrue(fed == 8, "bread the town wants: half what it charges, not " + fed);
        helper.assertTrue(Lockers.worth(offers, Map.of(), Set.of()) == 0, "nothing to sell it, nothing wanted: none");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aComfortTheTownLacksDrawsTheTradeThatMakesIt(GameTestHelper helper) {
        var met = Map.of("furnishings", 0.0, "clothing", 0.5, "treats", 1.0);
        var marbles = Jobs.comfortMarbles(met, 500, Map.of());
        double share = dev.b28.enterprise.Dials.COMFORT_MARBLES.get();
        double shepherd = marbles.entrySet().stream().filter(e -> e.getKey().profession() == VillagerProfession.SHEPHERD)
                .mapToDouble(Map.Entry::getValue).sum();
        double leatherworker = marbles.entrySet().stream()
                .filter(e -> e.getKey().profession() == VillagerProfession.LEATHERWORKER).mapToDouble(Map.Entry::getValue).sum();
        helper.assertTrue(Math.abs(shepherd - 500 * share) < 1e-9, "no furnishings: a Shepherd's share of savings, " + shepherd);
        helper.assertTrue(Math.abs(leatherworker - 0.5 * 500 * share) < 1e-9, "clothing half met: half, " + leatherworker);
        helper.assertTrue(Jobs.comfortMarbles(met, 500, Map.of(VillagerProfession.SHEPHERD, 1)).keySet().stream()
                .noneMatch(t -> t.profession() == VillagerProfession.SHEPHERD), "the town has a Shepherd: none for it");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void anIdleHandDrawsATradeByWhatItWouldMeet(GameTestHelper helper) {
        Map<String, Double> paper = Map.of("minecraft:paper", 3.0, "minecraft:carrot", 1.0);
        var bag = Jobs.bag(paper, Map.of(), Map.of(), 0, Map.of());
        helper.assertTrue(marbles(bag, VillagerProfession.LIBRARIAN) == 3 && marbles(bag, VillagerProfession.FARMER) == 1,
                "paper: 3 marbles a Librarian's, carrots 1 a Farmer's: " + bag);
        bag = Jobs.bag(paper, Map.of(VillagerProfession.LIBRARIAN, 1), Map.of(), 0, Map.of());
        helper.assertTrue(marbles(bag, VillagerProfession.LIBRARIAN) == 1.5,
                "with a Librarian already, a second shares the paper: " + marbles(bag, VillagerProfession.LIBRARIAN));
        helper.assertTrue(Jobs.draw(Jobs.bag(Map.of(), Map.of(), Map.of(), 0, Map.of()), helper.getLevel().random).isEmpty(),
                "nothing to sell: no stall");
        var carrots = Jobs.bag(Map.of("minecraft:carrot", 2.0), Map.of(), Map.of(), 0, Map.of());
        helper.assertTrue(Jobs.draw(carrots, helper.getLevel().random).orElseThrow().profession() == VillagerProfession.FARMER,
                "only carrots: a Farmer");
        // Untended plots: four days of what they'd yield, in a Farmer's marbles.
        helper.assertTrue(marbles(Jobs.bag(Map.of(), Map.of(), Map.of("minecraft:wheat", 40.0), 0, Map.of()),
                VillagerProfession.FARMER) == 8, "40 wheat a day untended: 8 marbles a Farmer's");
        helper.assertTrue(marbles(Jobs.bag(Map.of(), Map.of(), Map.of("minecraft:wheat", 4.0), 0, Map.of()),
                VillagerProfession.FARMER) == 0, "a few plots idle: not worth a hand");
        // A town with halls it wants and could pay for: a Mason's marbles, until it has one.
        helper.assertTrue(marbles(Jobs.bag(Map.of(), Map.of(), Map.of(), 8, Map.of()), VillagerProfession.MASON) == 8,
                "8 emeralds of building wanted: 8 marbles a Mason's");
        helper.assertTrue(marbles(Jobs.bag(Map.of(), Map.of(VillagerProfession.MASON, 1), Map.of(), 8, Map.of()),
                VillagerProfession.MASON) == 0, "one is enough");
        // Drawn often enough, each trade comes up about as often as its share.
        var even = Jobs.bag(paper, Map.of(), Map.of(), 0, Map.of());
        int librarians = 0;
        for (int i = 0; i < 1000; i++) {
            if (Jobs.draw(even, helper.getLevel().random).orElseThrow().profession() == VillagerProfession.LIBRARIAN) {
                librarians++;
            }
        }
        helper.assertTrue(librarians > 330 && librarians < 530,
                "Librarian, Cartographer and Farmer at 3:3:1: about 3 in 7 Librarians, " + librarians + " of 1000");
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true)
    public static void fieldsGrowWhatTheVillageWouldBuy(GameTestHelper helper) {
        helper.setBlock(new BlockPos(3, 2, 3), dev.b28.enterprise.merchant.Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), Blocks.CHEST);
        var stock = dev.b28.enterprise.merchant.MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)));
        stock.insert(new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.BREAD, 128)); // fed
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(Map.of(), Map.of(), stock).isEmpty(),
                "nobody buys a crop, enough food: no field");
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(Map.of("minecraft:paper", 48.0), Map.of(), stock)
                .filter(net.minecraft.world.item.Items.SUGAR_CANE::equals).isPresent(), "Librarians buy paper: cane");
        var both = Map.of("minecraft:paper", 48.0, "minecraft:potato", 640.0);
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(both, Map.of(), stock)
                .filter(net.minecraft.world.item.Items.POTATO::equals).isPresent(), "a Farmer buys more in potatoes");
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(both, Map.of("minecraft:potato", 640.0), stock)
                .filter(net.minecraft.world.item.Items.SUGAR_CANE::equals).isPresent(), "potatoes grown already: cane next");
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(Map.of("minecraft:sweet_berries", 20.0), Map.of(), stock)
                .filter(net.minecraft.world.item.Items.SWEET_BERRIES::equals).isPresent(), "a Master Butcher: berries");
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(both, Map.of("minecraft:potato", 640.0), stock, Map.of(),
                true).filter(net.minecraft.world.item.Items.SUGAR_CANE::equals).isEmpty(), "a town still growing: no cane yet");
        // Its own Farmers' biscuits want cane: a fed town plants it even while it grows, for its own people.
        var biscuits = Map.of("minecraft:potato", 640.0, "minecraft:sugar_cane", 4.0);
        helper.assertTrue(dev.b28.enterprise.territory.Fields.choose(biscuits, Map.of("minecraft:potato", 640.0), stock,
                Map.of(), true).filter(net.minecraft.world.item.Items.SUGAR_CANE::equals).isPresent(),
                "its workshop's cane: planted while it grows");
        helper.assertTrue(dev.b28.enterprise.territory.Focus.of(5, 8, 2000) == dev.b28.enterprise.territory.Focus.GROW,
                "five villagers: it grows first");
        helper.assertTrue(dev.b28.enterprise.territory.Focus.of(9, 12, 100) == dev.b28.enterprise.territory.Focus.GROW,
                "short of food: it grows");
        helper.assertTrue(dev.b28.enterprise.territory.Focus.of(9, 9, 2000) == dev.b28.enterprise.territory.Focus.BUILD,
                "fed and grown, every bed taken: it builds");
        helper.assertTrue(dev.b28.enterprise.territory.Focus.of(9, 12, 2000) == dev.b28.enterprise.territory.Focus.TRADE,
                "and with room to grow: it trades");
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true)
    public static void caneIsPressedIntoPaper(GameTestHelper helper) {
        helper.setBlock(new BlockPos(3, 2, 3), dev.b28.enterprise.merchant.Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), Blocks.CHEST);
        var stock = dev.b28.enterprise.merchant.MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)));
        stock.insert(new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.SUGAR_CANE, 40));
        var pressing = dev.b28.enterprise.territory.Workshop.CONVERSIONS.get(0);
        int made = dev.b28.enterprise.territory.Workshop.runForTest(pressing, 64, stock);
        helper.assertTrue(made == 40 && stock.count(new net.minecraft.world.item.trading.ItemCost(
                net.minecraft.world.item.Items.PAPER, 1)) == 40, "40 cane, 40 paper: " + made);
        stock.insert(new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.WHEAT, 10));
        int bread = dev.b28.enterprise.territory.Workshop.runForTest(dev.b28.enterprise.territory.Workshop.CONVERSIONS.get(1),
                10, stock);
        helper.assertTrue(bread == 3, "10 wheat bake 3 bread, a wheat over: " + bread);
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true)
    public static void townsMakeTheirOwnComforts(GameTestHelper helper) {
        helper.setBlock(new BlockPos(3, 2, 3), dev.b28.enterprise.merchant.Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), Blocks.CHEST);
        var stock = dev.b28.enterprise.merchant.MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)));
        stock.insert(new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.WHEAT, 10));
        stock.insert(new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.SUGAR_CANE, 2));
        java.util.function.Function<net.minecraft.world.item.Item, dev.b28.enterprise.territory.Workshop.Conversion> making =
                out -> dev.b28.enterprise.territory.Workshop.CONVERSIONS.stream().filter(c -> c.output() == out).findFirst()
                        .orElseThrow();
        int biscuits = dev.b28.enterprise.territory.Workshop.runForTest(making.apply(net.minecraft.world.item.Items.COOKIE),
                64, stock);
        helper.assertTrue(biscuits == 8, "10 wheat and 2 cane: two batches of biscuits, the cane runs out first: " + biscuits);
        // A cane earns more in biscuits than pressed into paper: the workshop gives it to the biscuits first.
        double biscuit = dev.b28.enterprise.territory.Workshop.gain(making.apply(net.minecraft.world.item.Items.COOKIE),
                dev.b28.enterprise.money.Prices.emeraldsEach(net.minecraft.world.item.Items.COOKIE));
        double paper = dev.b28.enterprise.territory.Workshop.gain(making.apply(net.minecraft.world.item.Items.PAPER),
                dev.b28.enterprise.money.Prices.emeraldsEach(net.minecraft.world.item.Items.PAPER));
        helper.assertTrue(biscuit > 1.4 * paper, "biscuits " + biscuit + " a emerald of inputs, paper " + paper);
        helper.assertTrue(stock.count(new net.minecraft.world.item.trading.ItemCost(net.minecraft.world.item.Items.WHEAT, 1)) == 6,
                "4 wheat used, 6 left");
        helper.assertTrue(dev.b28.enterprise.territory.Comforts.kindOf(dev.b28.enterprise.trade.Wares.PIPE_TOBACCO.get())
                .map(dev.b28.enterprise.territory.Comforts::name).filter("leisure"::equals).isPresent(), "a comfort: leisure");
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true)
    public static void aStallIsALockerWithAWorker(GameTestHelper helper) {
        Villager idle = helper.spawn(EntityType.VILLAGER, new BlockPos(5, 2, 1));
        var trade = Jobs.TRADES.stream().filter(t -> t.profession() == VillagerProfession.LIBRARIAN).findFirst().orElseThrow();
        BlockPos workstation = Jobs.buildStall(helper.getLevel(), helper.absolutePos(new BlockPos(0, 1, 0)), trade);
        helper.assertTrue(helper.getLevel().getBlockState(workstation).is(Blocks.LECTERN), "a lectern");
        helper.assertFalse(Lockers.touchingContainers(helper.getLevel(), workstation).isEmpty(), "with a chest: a locker");
        var desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(new BlockPos(5, 2, 5)));
        Jobs.hire(helper.getLevel(), idle, workstation, trade, desk);
        helper.assertTrue(idle.getVillagerData().getProfession() == VillagerProfession.LIBRARIAN, "the idle villager is a Librarian");
        helper.runAfterDelay(40, () -> {
            helper.assertTrue(idle.getVillagerData().getProfession() == VillagerProfession.LIBRARIAN, "and stays one");
            helper.assertTrue(idle.getBrain().getMemory(MemoryModuleType.JOB_SITE).map(GlobalPos::pos).filter(workstation::equals)
                    .isPresent(), "working the stall");
            idle.discard();
            helper.succeed();
        });
    }

    /**
     * The core's stall (`Stall`, core mode): its villager is given the trade outright, not only the job site, which the
     * game wipes from a villager with no profession before it gets there (it then takes the nearest free workstation:
     * Wheatfell's Mason stall made a Leatherworker of a village cauldron).
     */
    @GameTest(template = "floor", skyAccess = true)
    public static void theCoresStallGivesItsVillagerTheTrade(GameTestHelper helper) {
        Villager idle = helper.spawn(EntityType.VILLAGER, new BlockPos(5, 2, 1));
        var trade = Jobs.TRADES.stream().filter(t -> t.profession() == VillagerProfession.MASON).findFirst().orElseThrow();
        BlockPos workstation = Jobs.buildStall(helper.getLevel(), helper.absolutePos(new BlockPos(0, 1, 0)), trade);
        dev.b28.enterprise.binding.adapters.PeopleAdapter.takeStall(helper.getLevel(), idle, workstation, VillagerProfession.MASON);
        helper.assertTrue(idle.getVillagerData().getProfession() == VillagerProfession.MASON
                && idle.getVillagerData().getLevel() == 1, "the idle villager is a Mason");
        helper.runAfterDelay(40, () -> {
            helper.assertTrue(idle.getVillagerData().getProfession() == VillagerProfession.MASON, "and stays one: "
                    + idle.getVillagerData().getProfession() + ", site " + idle.getBrain().getMemory(MemoryModuleType.JOB_SITE)
                    + ", poi " + helper.getLevel().getPoiManager().getType(workstation) + ", free "
                    + helper.getLevel().getPoiManager().getFreeTickets(workstation) + ", block " + helper.getLevel().getBlockState(workstation));
            helper.assertTrue(idle.getBrain().getMemory(MemoryModuleType.JOB_SITE).map(GlobalPos::pos).filter(workstation::equals)
                    .isPresent(), "working the stall");
            idle.discard();
            helper.succeed();
        });
    }
}
