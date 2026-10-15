package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.settlement.Census;
import dev.b28.enterprise.territory.Growth;
import dev.b28.enterprise.territory.Territory;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.entity.ai.village.poi.PoiTypes;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.BedBlock;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.phys.AABB;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class GrowthTests {
    private static final BlockPos BELL = new BlockPos(3, 2, 3);
    private static final BlockPos DESK = new BlockPos(5, 2, 5);

    @GameTest(template = "floor")
    public static void fieldsRingTheTownAndCutNoOneOff(GameTestHelper helper) {
        BlockPos bell = new BlockPos(0, 64, 0);
        BlockPos home = new BlockPos(10, 64, 0);
        java.util.List<BlockPos> town = java.util.List.of(bell, home);
        helper.assertFalse(Growth.zoned(bell, new BlockPos(2, 64, -4), 9, town), "not over a home");
        helper.assertFalse(Growth.zoned(bell, new BlockPos(12, 64, -4), 9, town), "not in its yard");
        helper.assertTrue(Growth.zoned(bell, new BlockPos(14, 64, -4), 9, town), "beyond it, out from the bell: yes");
        helper.assertTrue(Growth.zoned(bell, new BlockPos(-22, 64, -4), 9, town), "or well away on the far side");
        helper.assertFalse(Growth.zoned(bell, new BlockPos(-4, 64, 4), 9, java.util.List.of(bell, new BlockPos(0, 64, 16))),
                "never between a home and the bell");
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true) // no barrier ceiling: the cottage needs open sky over the floor
    public static void aCottageFitsOnOpenGroundAndHasABed(GameTestHelper helper) {
        BlockPos corner = helper.absolutePos(new BlockPos(1, 1, 1));
        helper.assertTrue(Growth.fits(helper.getLevel(), corner), "flat, open floor with a block around: fits");
        BlockPos head = Growth.buildHome(helper.getLevel(), corner);
        helper.assertTrue(helper.getLevel().getBlockState(head).getBlock() instanceof BedBlock, "a bed inside");
        helper.assertTrue(helper.getLevel().getBlockState(head.east(2)).getBlock() instanceof BedBlock, "two: a family's");
        helper.assertTrue(helper.getLevel().getPoiManager().getType(head).filter(type -> type.is(PoiTypes.HOME)).isPresent(),
                "villagers can claim it");
        helper.assertFalse(Growth.fits(helper.getLevel(), corner), "and nothing else fits on top of it");
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true)
    public static void fieldsAndWaterAreNoPlaceForAHome(GameTestHelper helper) {
        BlockPos corner = helper.absolutePos(new BlockPos(1, 1, 1));
        helper.setBlock(new BlockPos(3, 1, 3), Blocks.FARMLAND);
        helper.assertFalse(Growth.fits(helper.getLevel(), corner), "not on a field");
        helper.setBlock(new BlockPos(3, 1, 3), Blocks.WATER);
        helper.assertFalse(Growth.fits(helper.getLevel(), corner), "not in water");
        helper.succeed();
    }

    private static GlobalPos town(GameTestHelper helper, Territory territory, Census census, double mood) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        MerchantStock stock = MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
        stock.insert(new ItemStack(Items.BREAD, 64));
        stock.insert(new ItemStack(Items.BREAD, 64));
        stock.insert(new ItemStack(Items.EMERALD, 10));
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), census, 0, List.of());
        GlobalPos bell = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL));
        territory.setMood(bell, mood);
        return bell;
    }

    private static long babiesNearBell(GameTestHelper helper) {
        return helper.getLevel().getEntitiesOfClass(Villager.class, new AABB(helper.absolutePos(BELL)).inflate(2),
                Villager::isBaby).size();
    }

    @GameTest(template = "floor")
    public static void aContentTownWithASpareBedHasAChild(GameTestHelper helper) {
        Territory territory = new Territory();
        Census threeInFour = new Census(3, 3, 4, 3, 0);
        GlobalPos bell = town(helper, territory, threeInFour, 2);
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), threeInFour, Growth.DAYS_PER_BIRTH, List.of());
        helper.assertTrue(territory.born(bell) == 1 && babiesNearBell(helper) == 1, "a child by the bell");
        MerchantStock stock = MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
        helper.assertTrue(stock.count(new ItemCost(Items.EMERALD, 1)) == 7, "3 emeralds paid");
        helper.assertTrue(stock.count(new ItemCost(Items.BREAD, 1)) <= 128 - 8, "and its food eaten");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void anUnhappyTownHasNoChildren(GameTestHelper helper) {
        Territory territory = new Territory();
        Census threeInFour = new Census(3, 3, 4, 3, 0);
        GlobalPos bell = town(helper, territory, threeInFour, 0);
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), threeInFour, 2 * Growth.DAYS_PER_BIRTH, List.of());
        helper.assertTrue(territory.born(bell) == 0 && babiesNearBell(helper) == 0, "so-so: no children");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void masonsBuildABridgeForWages(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, new Census(3, 3, 3, 3, 0), 0);
        var works = new dev.b28.enterprise.territory.Works();
        var hex = new dev.b28.enterprise.hex.Hex(5000, 5000);
        var bridge = works.start(bell, helper.getLevel().dimension(), hex, new dev.b28.enterprise.hex.Hex(4999, 5000),
                new dev.b28.enterprise.hex.Hex(5001, 5000));
        MerchantStock stock = MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
        stock.insert(new ItemStack(Items.EMERALD, 54)); // 64 with the town's 10
        long t = helper.getLevel().getGameTime();
        BlockPos desk = helper.absolutePos(DESK);
        works.work(helper.getLevel(), desk, stock, bell, 4, t); // starts the clock
        works.work(helper.getLevel(), desk, stock, bell, 4, t + 3 * 24000L);
        // 4 hexes of stone a day (64 blocks) for 2 masons at 32 a day: 192 blocks in 3 days, 6 emeralds of wages.
        helper.assertTrue(Math.abs(bridge.progress() * dev.b28.enterprise.territory.Works.deck(hex,
                        new dev.b28.enterprise.hex.Hex(4999, 5000), new dev.b28.enterprise.hex.Hex(5001, 5000)).size() - 192) < 1,
                "192 blocks laid: " + bridge.progress());
        helper.assertTrue(stock.count(new ItemCost(Items.EMERALD, 1)) == 58, "6 emeralds of wages paid");
        works.work(helper.getLevel(), desk, stock, bell, 4, t + 10 * 24000L);
        helper.assertTrue(bridge.done() && works.bridged(helper.getLevel().dimension(), hex), "finished within 10 days");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aWallRingsTheBellWithFourGates(GameTestHelper helper) {
        BlockPos centre = new BlockPos(0, 0, 0);
        var ring = dev.b28.enterprise.territory.Works.ring(centre, dev.b28.enterprise.territory.Works.WALL_RADIUS);
        helper.assertTrue(ring.size() > 200 && ring.size() < 300, "about 2 pi x 40 columns: " + ring.size());
        helper.assertTrue(ring.stream().noneMatch(c -> Math.abs(c.getX()) <= 1 || Math.abs(c.getZ()) <= 1),
                "gates at the four points");
        helper.assertTrue(ring.stream().allMatch(c -> Math.abs(Math.hypot(c.getX(), c.getZ()) - 40) < 0.75), "on the ring");
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true)
    public static void wallsRiseOnGroundAndLeaveFieldsOpen(GameTestHelper helper) {
        // A ring of radius 3 round the arena's middle, on grass, with farmland under one column.
        for (int x = 0; x <= 6; x++) {
            for (int z = 0; z <= 6; z++) {
                helper.setBlock(new BlockPos(x, 1, z), Blocks.GRASS_BLOCK);
            }
        }
        BlockPos centre = helper.absolutePos(new BlockPos(3, 1, 3));
        var ring = dev.b28.enterprise.territory.Works.ring(centre, 3);
        BlockPos field = ring.get(0);
        helper.getLevel().setBlock(new BlockPos(field.getX(), centre.getY(), field.getZ()), Blocks.FARMLAND.defaultBlockState(), 3);
        var works = new dev.b28.enterprise.territory.Works();
        var wall = works.startWall(GlobalPos.of(helper.getLevel().dimension(), centre), 3);
        dev.b28.enterprise.territory.Works.raiseForTest(helper.getLevel(), wall, ring);
        int raised = 0;
        for (BlockPos column : ring) {
            if (helper.getLevel().getBlockState(new BlockPos(column.getX(), centre.getY() + 3, column.getZ())).is(Blocks.OAK_LOG)) {
                raised++;
            }
        }
        helper.assertTrue(raised == ring.size() - 1, "every column but the field's, 3 high: " + raised + " of " + ring.size());
        helper.assertTrue(helper.getLevel().getBlockState(new BlockPos(field.getX(), centre.getY() + 1, field.getZ())).isAir(),
                "the field is left open");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aWalledTownFeelsSafe(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, new Census(3, 3, 3, 3, 0), 0);
        var works = dev.b28.enterprise.territory.Works.get(helper.getLevel().getServer());
        var wall = works.startWall(bell, dev.b28.enterprise.territory.Works.WALL_RADIUS);
        works.finish(wall);
        long t = helper.getLevel().getGameTime();
        BlockPos desk = helper.absolutePos(DESK);
        MerchantStock stock = MerchantStock.at(helper.getLevel(), desk);
        territory.touch(helper.getLevel(), desk, stock, t);
        territory.touch(helper.getLevel(), desk, stock, t + 30 * 24000L);
        // No comforts, 3 villagers: -3/8 for size, +1 for the wall.
        helper.assertTrue(Math.abs(territory.mood(bell) - (1 - 3 / 8.0)) < 0.01, "walled: " + territory.mood(bell));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void childrenGrowUpWhileAway(GameTestHelper helper) {
        Villager child = Growth.bear(helper.getLevel(), helper.absolutePos(BELL));
        long now = helper.getLevel().getGameTime();
        helper.assertTrue(child.isBaby(), "born a baby");
        Growth.growUp(child, now + 12000);
        helper.assertTrue(child.isBaby() && child.getAge() <= -11000, "half grown after half a day: " + child.getAge());
        Growth.growUp(child, now + 30000);
        helper.assertFalse(child.isBaby(), "grown after a day away");
        child.discard();
        helper.succeed();
    }
}
