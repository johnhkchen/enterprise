package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.locker.Locker;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.merchant.Merchant;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.ChestBlock;
import net.minecraft.world.level.block.state.properties.ChestType;
import net.minecraft.core.Direction;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

// Arena coordinates: floor at y=1, so blocks stand at y=2. Desk in the middle at (3, 2, 3).
// Compare positions in world space: helper.relativePos does not invert absolutePos for rotated tests.
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class LockerTests {
    private static final BlockPos DESK = new BlockPos(3, 2, 3);
    // Tests run side by side; a wider search would find the neighbouring tests' workstations.
    private static final int RANGE = 4;

    @GameTest(template = "floor")
    public static void findsContainersTouchingWorkstations(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(new BlockPos(1, 2, 1), Blocks.LECTERN);    // locker: chest beside it
        helper.setBlock(new BlockPos(0, 2, 1), Blocks.CHEST);
        helper.setBlock(new BlockPos(5, 2, 5), Blocks.COMPOSTER);  // locker: barrel on top
        helper.setBlock(new BlockPos(5, 3, 5), Blocks.BARREL);
        helper.setBlock(new BlockPos(5, 2, 1), Blocks.FLETCHING_TABLE); // no container: not a locker
        helper.setBlock(new BlockPos(1, 2, 5), Blocks.CHEST);       // container with no workstation: ignored

        List<Locker> lockers = Lockers.near(helper.getLevel(), helper.absolutePos(DESK), RANGE, 10);
        List<BlockPos> sites = lockers.stream().map(Locker::workstation).toList();
        helper.assertTrue(sites.size() == 2, "expected 2 lockers, found " + sites);
        helper.assertTrue(sites.contains(helper.absolutePos(new BlockPos(1, 2, 1))), "lectern with a chest is a locker");
        helper.assertTrue(
                sites.contains(helper.absolutePos(new BlockPos(5, 2, 5))), "composter with a barrel on top is a locker");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void otherDesksAreNotLockers(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(new BlockPos(5, 2, 5), Merchant.DESK.get());
        helper.setBlock(new BlockPos(6, 2, 5), Blocks.CHEST);

        helper.assertTrue(
                Lockers.near(helper.getLevel(), helper.absolutePos(DESK), RANGE, 10).isEmpty(),
                "a Merchant's Desk with a chest is stock, not a locker");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void deskStockIsTouchingContainers(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), Blocks.BARREL);
        helper.setBlock(new BlockPos(3, 2, 4), Blocks.CHEST);
        helper.setBlock(new BlockPos(1, 2, 3), Blocks.CHEST); // one block away: not touching

        List<BlockPos> stock = Lockers.touchingContainers(helper.getLevel(), helper.absolutePos(DESK));
        helper.assertTrue(stock.size() == 2, "expected 2 stock containers, found " + stock.size());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void doubleChestCountsOnceWithBothHalves(GameTestHelper helper) {
        BlockPos lectern = new BlockPos(2, 2, 2);
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(lectern, Blocks.LECTERN);
        // A double chest north of the lectern, running east-west; only one half touches the lectern.
        helper.setBlock(new BlockPos(2, 2, 1), Blocks.CHEST.defaultBlockState()
                .setValue(ChestBlock.FACING, Direction.NORTH).setValue(ChestBlock.TYPE, ChestType.LEFT));
        helper.setBlock(new BlockPos(3, 2, 1), Blocks.CHEST.defaultBlockState()
                .setValue(ChestBlock.FACING, Direction.NORTH).setValue(ChestBlock.TYPE, ChestType.RIGHT));

        List<BlockPos> containers = Lockers.touchingContainers(helper.getLevel(), helper.absolutePos(lectern));
        helper.assertTrue(containers.size() == 1, "double chest should appear once, found " + containers.size());
        int slots = Lockers.inventory(helper.getLevel(), containers).getSlots();
        helper.assertTrue(slots == 54, "double chest locker should have 54 slots, has " + slots);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void limitKeepsNearestLockers(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(new BlockPos(2, 2, 3), Blocks.LECTERN);          // 1 block away
        helper.setBlock(new BlockPos(1, 2, 3), Blocks.CHEST);
        helper.setBlock(new BlockPos(3, 2, 1), Blocks.COMPOSTER);        // 2 blocks away
        helper.setBlock(new BlockPos(3, 2, 0), Blocks.CHEST);
        helper.setBlock(new BlockPos(6, 2, 6), Blocks.FLETCHING_TABLE);  // farthest
        helper.setBlock(new BlockPos(6, 2, 5), Blocks.CHEST);

        int limit = Merchant.lockerLimit(1);
        List<BlockPos> sites = Lockers.near(helper.getLevel(), helper.absolutePos(DESK), RANGE, limit).stream()
                .map(Locker::workstation)
                .toList();
        helper.assertTrue(
                sites.equals(List.of(helper.absolutePos(new BlockPos(2, 2, 3)), helper.absolutePos(new BlockPos(3, 2, 1)))),
                "a Novice Merchant should keep the 2 nearest lockers, kept " + sites);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void limitPrefersLockersWithTradesSwitchedOn(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(new BlockPos(2, 2, 3), Blocks.LECTERN);          // nearest, nothing switched on
        helper.setBlock(new BlockPos(1, 2, 3), Blocks.CHEST);
        helper.setBlock(new BlockPos(3, 2, 1), Blocks.COMPOSTER);        // next, nothing switched on
        helper.setBlock(new BlockPos(3, 2, 0), Blocks.CHEST);
        helper.setBlock(new BlockPos(5, 2, 5), Blocks.FLETCHING_TABLE);  // farthest, a trade switched on
        helper.setBlock(new BlockPos(6, 2, 5), Blocks.CHEST);
        dev.b28.enterprise.merchant.MerchantBook.get(helper.getLevel().getServer()).setEnabled(
                net.minecraft.core.GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(DESK)),
                helper.absolutePos(new BlockPos(5, 2, 5)), "minecraft:stick+>minecraft:emerald", true);

        List<BlockPos> sites = Lockers.near(helper.getLevel(), helper.absolutePos(DESK), RANGE, 2).stream()
                .map(Locker::workstation)
                .toList();
        helper.assertTrue(sites.equals(List.of(helper.absolutePos(new BlockPos(5, 2, 5)), helper.absolutePos(new BlockPos(2, 2, 3)))),
                "the locker with a trade on comes first, then the nearest; kept " + sites);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void lockerLimitGrowsWithLevel(GameTestHelper helper) {
        helper.assertTrue(Merchant.lockerLimit(1) == 2, "Novice manages 2");
        helper.assertTrue(Merchant.lockerLimit(5) == 10, "Master manages 10");
        helper.assertTrue(Merchant.lockerLimit(9) == 10, "levels above Master are capped");
        helper.succeed();
    }
}
