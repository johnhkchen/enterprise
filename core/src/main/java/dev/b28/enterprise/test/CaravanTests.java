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
import net.minecraft.world.entity.npc.Villager;
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
        helper.setBlock(HUB, Merchant.DESK.get());
        helper.setBlock(HUB_CHEST, Blocks.CHEST);
        helper.setBlock(DEST, Merchant.DESK.get());
        helper.setBlock(DEST_CHEST, Blocks.CHEST);
        Villager merchant = helper.spawn(EntityType.VILLAGER, new BlockPos(1, 2, 4));
        merchant.setVillagerData(merchant.getVillagerData().setProfession(Merchant.PROFESSION.get()));
        LockerTradingTests.claim(helper, merchant, HUB);

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
        MoneyChanger.coins(192).forEach(network.dest()::insert); // the destination Merchant's earnings
        network.dest().insert(new ItemStack(Items.BREAD, 2));
        return network;
    }

    @GameTest(template = "floor")
    public static void caravanDeliversGoodsAndBringsCoinsHome(GameTestHelper helper) {
        Network net = network(helper, 2);
        long t = helper.getLevel().getGameTime();

        net.poll(t);
        Caravan out = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(out.leg() == Caravan.Leg.OUTBOUND && out.items() == 128, "leaves with the 128 paper");
        helper.assertTrue(net.hub().count(PAPER) == 0, "the paper left the hub");
        helper.assertTrue(net.hub().foodNutrition() == 5, "the leg's food was eaten: one bread of two left");

        net.poll(t + 79);
        helper.assertTrue(net.dest().count(PAPER) == 0, "not there yet at 79 ticks");
        net.poll(t + 80);
        helper.assertTrue(net.dest().count(PAPER) == 128, "delivers the paper on arrival");
        helper.assertTrue(MoneyChanger.value(net.dest().looseStacks()) == 0, "collects the destination's coins");
        Caravan back = net.ledger().caravan(net.route().id()).orElseThrow();
        helper.assertTrue(back.leg() == Caravan.Leg.RETURNING && back.spurs() == 192, "heads home with 192 spurs");

        net.poll(t + 160);
        helper.assertTrue(net.ledger().caravan(net.route().id()).isEmpty(), "the caravan is home");
        helper.assertTrue(MoneyChanger.value(net.hub().looseStacks()) == 192, "the hub has the 192 spurs in coins");
        helper.assertTrue(net.route().trips() == 1 && net.route().itemsDelivered() == 128
                && net.route().spursCollected() == 192 && net.route().foodEaten() == 2, "the route's totals add up");
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
