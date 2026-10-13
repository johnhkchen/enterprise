package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Bargain;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.item.Items;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class BargainTests {
    @GameTest(template = "floor")
    public static void noDealAtPar(GameTestHelper helper) {
        helper.assertTrue(Bargain.price(1.0, 0, 3, 3) < 0, "a full shelf buys nothing");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void shorterShelvesPayMore(GameTestHelper helper) {
        double half = Bargain.price(1.0, 0.5, 1, 1);
        double empty = Bargain.price(1.0, 1.0, 1, 1);
        helper.assertTrue(empty > half && half > Bargain.SELLER_FLOOR, "empty " + empty + " > half-full " + half);
        helper.assertTrue(empty <= 1.0, "never above the local value");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theHigherLevelMerchantTakesMoreOfTheGap(GameTestHelper helper) {
        double even = Bargain.price(1.0, 1.0, 1, 1);
        double masterSelling = Bargain.price(1.0, 1.0, 5, 1);
        double masterBuying = Bargain.price(1.0, 1.0, 1, 5);
        helper.assertTrue(Math.abs(even - 0.75) < 1e-9, "equals split the gap: " + even);
        helper.assertTrue(Math.abs(masterSelling - (0.5 + 0.5 * 5 / 6.0)) < 1e-9, "a Master seller takes 5/6: " + masterSelling);
        helper.assertTrue(masterBuying < even && masterSelling > even, "levels move the price their way");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void parIsTwoDaysOfWhatTheLockersCanTake(GameTestHelper helper) {
        Warehouse warehouse = new Warehouse();
        GlobalPos desk = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(new BlockPos(3, 2, 3)));
        long t = helper.getLevel().getGameTime();
        helper.assertTrue(warehouse.par(desk, Items.PAPER, true, t) == Warehouse.MIN_PAR, "a wanted good starts at a stack");
        helper.assertTrue(warehouse.par(desk, Items.CARROT, false, t) == 0, "an unwanted good has no par");
        // A librarian and a cartographer each buy 24 paper 16 times a restock.
        warehouse.recordCapacity(desk, 1L, java.util.Map.of(Items.PAPER, 384));
        warehouse.recordCapacity(desk, 2L, java.util.Map.of(Items.PAPER, 384));
        helper.assertTrue(warehouse.par(desk, Items.PAPER, true, t) == 1536, "two days of 768: " + warehouse.par(desk, Items.PAPER, true, t));
        warehouse.recordCapacity(desk, 2L, java.util.Map.of()); // the cartographer's trade switched off
        helper.assertTrue(warehouse.par(desk, Items.PAPER, true, t) == 768, "a session replaces that locker's figure");
        helper.succeed();
    }
}
