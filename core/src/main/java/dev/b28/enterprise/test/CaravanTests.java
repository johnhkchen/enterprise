package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.caravan.Caravan;
import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.caravan.Route;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.MoneyChanger;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.entity.EntityType;
import java.util.List;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** A hub and a destination 4 blocks apart: 80 ticks each way, 1 food each way. Each test keeps its own ledger. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class CaravanTests {
    private static final BlockPos HUB = new BlockPos(1, 2, 3);
    private static final BlockPos HUB_CHEST = new BlockPos(0, 2, 3);
    private static final BlockPos DEST = new BlockPos(5, 2, 3);
    private static final BlockPos DEST_CHEST = new BlockPos(6, 2, 3);
    private static final ItemCost PAPER = new ItemCost(Items.PAPER, 1);

    private record Network(GameTestHelper helper, CaravanLedger ledger, Route route,
            dev.b28.enterprise.territory.Territory territory) {
        MerchantStock hub() {
            return MerchantStock.at(helper.getLevel(), helper.absolutePos(HUB));
        }

        MerchantStock dest() {
            return MerchantStock.at(helper.getLevel(), helper.absolutePos(DEST));
        }

        void poll(long at) {
            // Its own territory: other tests' censuses claim real hexes, and their tolls would leak in.
            Caravans.poll(helper.getLevel().getServer(), ledger, territory, at);
        }
    }

    private static Network network(GameTestHelper helper, int hubBread) {
        return network(helper, hubBread, 5, 192);
    }

    /** As below, with a squad of one Librarian at the hub. */
    private static Network network(GameTestHelper helper, int hubBread, int hubEmeralds, int destSpurs) {
        return network(helper, hubBread, hubEmeralds, destSpurs, List.of(VillagerProfession.LIBRARIAN));
    }

    /** A trained villager of {@code trade} by the hub, ready to go with a caravan. */
    private static Villager squadMember(GameTestHelper helper, VillagerProfession trade, BlockPos at) {
        Villager member = helper.spawn(EntityType.VILLAGER, at);
        member.setVillagerData(member.getVillagerData().setProfession(trade).setLevel(2));
        member.setVillagerXp(1); // trained: keeps its trade without a workstation
        return member;
    }

    /**
     * A hub with paper, bread, {@code hubEmeralds} and a squad of {@code squad}; a destination holding
     * {@code destSpurs} in coins and bread.
     */
    private static Network network(GameTestHelper helper, int hubBread, int hubEmeralds, int destSpurs,
            List<VillagerProfession> squad) {
        helper.setBlock(HUB, Merchant.DESK.get());
        helper.setBlock(HUB_CHEST, Blocks.CHEST);
        helper.setBlock(DEST, Merchant.DESK.get());
        helper.setBlock(DEST_CHEST, Blocks.CHEST);
        Villager merchant = helper.spawn(EntityType.VILLAGER, new BlockPos(1, 2, 4));
        merchant.setVillagerData(merchant.getVillagerData().setProfession(Merchant.PROFESSION.get()));
        LockerTradingTests.claim(helper, merchant, HUB);
        for (int i = 0; i < squad.size(); i++) {
            squadMember(helper, squad.get(i), new BlockPos(0, 2, 5 + i % 2).east(i / 2));
        }

        var dimension = helper.getLevel().dimension();
        GlobalPos hub = GlobalPos.of(dimension, helper.absolutePos(HUB));
        GlobalPos dest = GlobalPos.of(dimension, helper.absolutePos(DEST));
        // The destination's librarian buys paper.
        MerchantBook.get(helper.getLevel().getServer()).setEnabled(
                dest, helper.absolutePos(new BlockPos(5, 2, 6)), "minecraft:paper+>minecraft:emerald", true);
        CaravanLedger ledger = new CaravanLedger();
        Network network = new Network(helper, ledger, ledger.addRoute(hub, dest), new dev.b28.enterprise.territory.Territory());
        network.hub().insert(new ItemStack(Items.PAPER, 64));
        network.hub().insert(new ItemStack(Items.PAPER, 64));
        if (hubBread > 0) {
            network.hub().insert(new ItemStack(Items.BREAD, hubBread));
        }
        if (hubEmeralds > 0) {
            network.hub().insert(new ItemStack(Items.EMERALD, hubEmeralds)); // funds the caravan's purse
        }
        MoneyChanger.coins(destSpurs).forEach(network.dest()::insert); // the destination Merchant's money
        network.dest().insert(new ItemStack(Items.BREAD, 2));
        return network;
    }

    @GameTest(template = "floor")
    public static void noSquadNoCaravan(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of());
        net.poll(helper.getLevel().getGameTime());
        helper.assertTrue(net.ledger().caravan(net.route().id()).isEmpty(), "nobody to send, nobody goes");
        helper.assertTrue(net.route().status().contains("squad"), "the route says so: " + net.route().status());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theSquadLeavesTheWorldAndComesBack(GameTestHelper helper) {
        // 70 bread (350 food points): food to spare past the caravans' 320-point par, for a second member.
        Network net = network(helper, 70, 5, 192, List.of(VillagerProfession.CARTOGRAPHER, VillagerProfession.LIBRARIAN));
        var squad = helper.getLevel().getEntitiesOfClass(Villager.class,
                new net.minecraft.world.phys.AABB(helper.absolutePos(HUB)).inflate(8),
                v -> v.getVillagerData().getProfession() != Merchant.PROFESSION.get());
        helper.assertTrue(squad.size() == 2, "two to send: " + squad.size());
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        Caravan out = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(out.crew().size() == 2, "both go: no books on that town yet, its road unmapped");
        helper.assertTrue(squad.stream().allMatch(v -> helper.getLevel().getEntity(v.getUUID()) == null),
                "both are off the world while on the road");
        helper.assertTrue(net.hub().foodNutrition() == 345, "both eat: 2 food points, one bread");
        net.poll(t + 80);
        net.poll(t + 160);
        helper.assertTrue(net.ledger().caravan(net.route().id()).isEmpty(), "home");
        helper.assertTrue(squad.stream().allMatch(v -> helper.getLevel().getEntity(v.getUUID()) instanceof Villager back
                        && back.getVillagerData().getProfession() == v.getVillagerData().getProfession()),
                "both are back, still in their trades");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void extraMembersGoOnlyWhereTheyHelp(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of(VillagerProfession.CARTOGRAPHER, VillagerProfession.SHEPHERD));
        var desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(HUB));
        var noNeed = new dev.b28.enterprise.caravan.Squad.Needs(false, false, false);
        helper.assertTrue(dev.b28.enterprise.caravan.Squad.muster(helper.getLevel(), desk, 16, noNeed, 3).size() == 1,
                "nothing to map, no animals: the lead goes alone");
        var animals = new dev.b28.enterprise.caravan.Squad.Needs(false, false, true);
        var squad = dev.b28.enterprise.caravan.Squad.muster(helper.getLevel(), desk, 16, animals, 3);
        helper.assertTrue(squad.size() == 2 && dev.b28.enterprise.caravan.Squad.trade(squad.get(1)).equals("minecraft:shepherd"),
                "animals to drive: the Shepherd comes too");
        helper.assertTrue(dev.b28.enterprise.caravan.Squad.muster(helper.getLevel(), desk, 16, animals, 0).size() == 1,
                "unless she's another route's lead");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void babiesAndTheDesksMerchantStayHome(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of());
        Villager baby = squadMember(helper, VillagerProfession.CARTOGRAPHER, new BlockPos(0, 2, 5));
        baby.setAge(-24000);
        Villager untrained = helper.spawn(EntityType.VILLAGER, new BlockPos(0, 2, 6));
        var desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(HUB));
        helper.assertFalse(dev.b28.enterprise.caravan.Squad.eligible(baby, desk), "no babies on the road");
        helper.assertFalse(dev.b28.enterprise.caravan.Squad.eligible(untrained, desk), "nor the untrained");
        helper.assertTrue(dev.b28.enterprise.caravan.Squad.available(helper.getLevel(), desk, 16).isEmpty(),
                "and not the Merchant who works the desk");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aShepherdSellsSheepWhereThePastureIsShort(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192, List.of(VillagerProfession.SHEPHERD));
        net.territory().afterCensus(helper.getLevel(), helper.absolutePos(new BlockPos(6, 2, 4)),
                new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, List.of());
        for (int i = 0; i < 3; i++) {
            helper.spawn(EntityType.SHEEP, new BlockPos(2, 2, 1 + i));
        }
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        helper.assertTrue(net.ledger().caravan(net.route().id()).orElseThrow().herd().size() == 3, "she drives 3 sheep");
        net.poll(t + 80);
        helper.assertTrue(net.route().animalsSold() == 3, "the town buys all three: " + net.route().animalsSold());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aLibrarianBringsBackTheBooks(GameTestHelper helper) {
        Network net = network(helper, 70, 5, 192, List.of(VillagerProfession.CARTOGRAPHER));
        var warehouse = dev.b28.enterprise.merchant.Warehouse.get(helper.getLevel().getServer());
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(warehouse.stocktakeTime(net.route().to()) < 0, "no Librarian: nobody counted their shelves");
        net.poll(t + 160);
        Villager librarian = squadMember(helper, VillagerProfession.LIBRARIAN, new BlockPos(0, 2, 5));
        net.poll(t + 160 + 24000);
        net.poll(t + 240 + 24000);
        helper.assertTrue(warehouse.stocktakeTime(net.route().to()) >= 0, "with one, the books come home");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void caravanSellsItsCargoAndBuysItsFood(GameTestHelper helper) {
        Network net = network(helper, 2);
        long t = helper.getLevel().getGameTime();

        net.poll(t);
        Caravan out = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(out.leg() == Caravan.Leg.OUTBOUND && out.items() == 128, "leaves with the 128 paper");
        helper.assertTrue(out.spurs() > 0, "and a purse for the road");
        helper.assertTrue(net.hub().count(PAPER) == 0 && net.hub().foodNutrition() == 5, "paper gone, one bread eaten");
        int purse = out.spurs();

        int destMoney = MoneyChanger.value(net.dest().looseStacks());
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(PAPER) == dev.b28.enterprise.merchant.Warehouse.MIN_PAR,
                "the destination bought paper up to its par of one stack, bought " + net.dest().count(PAPER));
        Caravan back = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(back.leg() == Caravan.Leg.RETURNING, "heads home");
        int destSpent = destMoney - MoneyChanger.value(net.dest().looseStacks());
        // It paid the bargained price for 64 paper and was paid back for a bread of road food.
        helper.assertTrue(net.route().salesSpurs() > 0 && net.route().foodSpurs() > 0, "a sale and a food purchase");
        helper.assertTrue(destSpent == net.route().salesSpurs() - net.route().foodSpurs(),
                "the destination's money moved by sales less food: " + destSpent);
        helper.assertTrue(back.spurs() == purse + net.route().salesSpurs() - net.route().foodSpurs(),
                "the caravan carries its purse plus sales less food");

        int carried = back.spurs();
        int hubBefore = MoneyChanger.value(net.hub().looseStacks()); // includes change from funding the purse
        net.poll(t + 160);
        helper.assertTrue(net.ledger().caravan(net.route().id()).isEmpty(), "the caravan is home");
        helper.assertTrue(MoneyChanger.value(net.hub().looseStacks()) - hubBefore == carried, "the hub banks what it carried");
        helper.assertTrue(net.route().trips() == 1 && net.route().itemsDelivered() == 64, "one trip, 64 sold");
        helper.assertTrue(net.hub().count(PAPER) == 64, "the unsold 64 paper came home to the hub's warehouse");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void townsBuyComfortsUpToTheirPar(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        // The destination is a town of five: it wants 4 days of treats, 5 x 0.1 x 4 = 2 emeralds' worth (36 cookies).
        net.territory().afterCensus(helper.getLevel(), helper.absolutePos(new BlockPos(6, 2, 4)),
                new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, java.util.List.of());
        net.hub().insert(new ItemStack(Items.COOKIE, 64));
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        int aboard = net.ledger().caravan(net.route().id()).orElseThrow().cargo().stream()
                .filter(stack -> stack.is(Items.COOKIE)).mapToInt(ItemStack::getCount).sum();
        helper.assertTrue(aboard == 54, "the caravan takes half again what the town wants: " + aboard);
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(new ItemCost(Items.COOKIE, 1)) == 36,
                "the town buys 36 cookies: " + net.dest().count(new ItemCost(Items.COOKIE, 1)));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void caravansAllowForComfortsUsedSinceTheStocktake(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        var territory = net.territory();
        territory.afterCensus(helper.getLevel(), helper.absolutePos(new BlockPos(6, 2, 4)),
                new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, java.util.List.of());
        var dest = net.route().to();
        long t = helper.getLevel().getGameTime();
        // At the last visit the town was full: 36 cookies, its whole 2-emerald par of treats.
        dev.b28.enterprise.merchant.Warehouse.get(helper.getLevel().getServer())
                .recordStocktake(dest, java.util.Map.of("minecraft:cookie", 36), t);
        var now = Caravans.comfortWantsForTest(helper.getLevel().getServer(), territory, dest, t);
        helper.assertTrue(!now.containsKey("treats"), "just after: it wants no treats " + now);
        var later = Caravans.comfortWantsForTest(helper.getLevel().getServer(), territory, dest, t + 2 * 24000L);
        // Two days on, 5 villagers have eaten 1 emerald's worth: it wants that back.
        helper.assertTrue(Math.abs(later.getOrDefault("treats", 0.0) - 1.0) < 1e-6, "two days on: " + later);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void townsKeepTheirTradingCapital(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192); // the town holds 192 spurs, 12 emeralds
        net.territory().afterCensus(helper.getLevel(), helper.absolutePos(new BlockPos(6, 2, 4)),
                new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, java.util.List.of());
        // Its lockers sell 240 paper a day: two days' stock is 480 paper, 20 emeralds, more than it has.
        long start = helper.getLevel().getGameTime();
        var warehouse = dev.b28.enterprise.merchant.Warehouse.get(helper.getLevel().getServer());
        for (int day = 0; day <= 30; day++) {
            warehouse.recordUse(net.route().to(), Items.PAPER, 240, start + day * 24000L);
        }
        long t = start + 30 * 24000L + 100; // a month of trade later
        net.hub().insert(new ItemStack(Items.COOKIE, 64));
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(new ItemCost(Items.COOKIE, 1)) == 0, "no cookies: the money is for paper");
        helper.assertTrue(net.dest().count(PAPER) > 0, "and it bought paper");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aTownShortOfClothesBuysAPairOfBoots(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        net.territory().afterCensus(helper.getLevel(), helper.absolutePos(new BlockPos(6, 2, 4)),
                new dev.b28.enterprise.settlement.Census(5, 5, 5, 5, 0), 1, java.util.List.of());
        for (int i = 0; i < 3; i++) {
            net.hub().insert(new ItemStack(Items.LEATHER_BOOTS)); // 4 emeralds each: more than the 2-emerald par
        }
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(new ItemCost(Items.LEATHER_BOOTS, 1)) == 1, "one pair bought");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void caravansBuyATownsSurplusForTheTripHome(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        // Another town on the hub's routes buys carrots: the travelling Merchant has somewhere to sell them.
        var dimension = helper.getLevel().dimension();
        GlobalPos elsewhere = GlobalPos.of(dimension, helper.absolutePos(new BlockPos(3, 2, 6)));
        net.ledger().addRoute(GlobalPos.of(dimension, helper.absolutePos(HUB)), elsewhere);
        MerchantBook.get(helper.getLevel().getServer()).setEnabled(
                elsewhere, helper.absolutePos(new BlockPos(3, 2, 0)), "minecraft:carrot+>minecraft:emerald", true);
        for (int i = 0; i < 4; i++) {
            net.dest().insert(new ItemStack(Items.CARROT, 64)); // nobody there wants carrots: all surplus past food par
        }
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        int destMoney = MoneyChanger.value(net.dest().looseStacks());
        net.poll(t + 80);
        helper.assertTrue(net.route().itemsBought() > 0, "the caravan bought carrots");
        int carrotsAboard = net.ledger().caravan(net.route().id()).orElseThrow().cargo().stream()
                .filter(stack -> stack.is(Items.CARROT)).mapToInt(ItemStack::getCount).sum();
        helper.assertTrue(carrotsAboard > 0 && carrotsAboard <= net.route().itemsBought(),
                "carrots ride home (with any other surplus bought): " + carrotsAboard);
        helper.assertTrue(MoneyChanger.value(net.dest().looseStacks()) > destMoney - net.route().salesSpurs(),
                "the town was paid for its surplus");
        helper.assertTrue(net.dest().foodNutrition() >= dev.b28.enterprise.merchant.Warehouse.FOOD_PAR_POINTS,
                "the town keeps its food par: " + net.dest().foodNutrition());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void caravansDontBuySurplusNobodyWants(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 192);
        for (int i = 0; i < 4; i++) {
            net.dest().insert(new ItemStack(Items.CARROT, 64));
        }
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(net.route().itemsBought() == 0, "no town on the hub's routes wants carrots: none bought");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aDestinationThatCantPayGetsNothing(GameTestHelper helper) {
        Network net = network(helper, 2, 5, 0);
        long t = helper.getLevel().getGameTime();
        net.poll(t);
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(PAPER) == 0, "no money, no paper");
        helper.assertTrue(net.ledger().caravan(net.route().id()).orElseThrow().cargo().stream()
                .filter(stack -> stack.is(Items.PAPER)).mapToInt(ItemStack::getCount).sum() == 128, "the paper rides home");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aHubThatCantFundThePurseStaysHome(GameTestHelper helper) {
        Network net = network(helper, 2, 0, 192);
        net.poll(helper.getLevel().getGameTime());
        helper.assertTrue(net.ledger().caravan(net.route().id()).isEmpty(), "no purse, no caravan");
        helper.assertTrue(net.route().status().contains("fund"), "the route says the hub can't fund it");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void noFoodNoCaravan(GameTestHelper helper) {
        Network net = network(helper, 0);
        net.poll(helper.getLevel().getGameTime());
        helper.assertTrue(net.ledger().caravan(net.route().id()).isEmpty(), "without food, nobody leaves");
        helper.assertTrue(net.hub().count(PAPER) == 128, "the goods stay at the hub");
        helper.assertTrue(net.route().status().contains("food"), "the route says it's waiting for food");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void foodCapsALegAt3200Blocks(GameTestHelper helper) {
        helper.assertTrue(Caravans.inRange(3200), "3,200 blocks takes a full stack of bread");
        helper.assertFalse(Caravans.inRange(3210), "a little further is out of range");
        helper.assertTrue(Caravans.travelTicks(1200) == 24000, "1,200 blocks is a day on the road");
        helper.succeed();
    }
}
