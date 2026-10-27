package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.settlement.Census;
import dev.b28.enterprise.territory.Fields;
import dev.b28.enterprise.territory.Growth;
import dev.b28.enterprise.territory.Territory;
import dev.b28.enterprise.territory.TownPlans;
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

    /** A cottage built to front a lane to its east: its door in its east wall, facing out, its beds still claimable. */
    @GameTest(template = "floor", skyAccess = true)
    public static void aCottageFacesTheLaneItFronts(GameTestHelper helper) {
        BlockPos corner = helper.absolutePos(new BlockPos(1, 1, 1));
        BlockPos head = Growth.buildHome(helper.getLevel(), corner, net.minecraft.core.Direction.EAST);
        helper.assertTrue(helper.getLevel().getBlockState(head).getBlock() instanceof BedBlock, "a bed inside");
        var door = helper.getLevel().getBlockState(corner.offset(Growth.HOME_SIZE - 1, 1, 2));
        helper.assertTrue(door.getBlock() instanceof net.minecraft.world.level.block.DoorBlock
                && door.getValue(net.minecraft.world.level.block.DoorBlock.FACING) == net.minecraft.core.Direction.EAST,
                "the door in the east wall, facing east: " + door);
        helper.assertTrue(helper.getLevel().getPoiManager().getType(head).filter(type -> type.is(PoiTypes.HOME)).isPresent(),
                "villagers can claim it");
        helper.succeed();
    }

    /**
     * Inside a town's inner wall (John: "lanes"): lanes every ward across, and new homes fronting them and the avenues,
     * none on a street, each door a step from one.
     */
    @GameTest(template = "floor")
    public static void homesFrontTheLanesAndAvenues(GameTestHelper helper) {
        var plan = new TownPlans.Plan(new BlockPos(0, 64, 0), new TownPlans.Rect(-30, -30, 30, 30),
                new TownPlans.Rect(-80, -80, 80, 80), true, true);
        helper.assertTrue(TownPlans.lanes(plan).size() == 4, "a lane each side of each avenue: " + TownPlans.lanes(plan).size());
        var homes = TownPlans.frontages(plan, Growth.HOME_SIZE);
        helper.assertTrue(homes.size() > 20, "rows of homes: " + homes.size());
        java.util.function.BiPredicate<Integer, Integer> street = (x, z) -> Math.abs(x) <= 1 || Math.abs(z) <= 1
                || x >= 18 && x <= 21 || x >= -22 && x <= -19 || z >= 18 && z <= 21 || z >= -22 && z <= -19;
        for (var f : homes) {
            for (int dx = 0; dx < Growth.HOME_SIZE; dx++) {
                for (int dz = 0; dz < Growth.HOME_SIZE; dz++) {
                    int x = f.x() + dx, z = f.z() + dz;
                    helper.assertFalse(street.test(x, z) || TownPlans.onLane(plan, x, z), "a home on a street at " + f);
                }
            }
            int n = Growth.HOME_SIZE;
            int[] out = switch (f.door()) {
                case EAST -> new int[] {f.x() + n + 1, f.z() + 2};
                case WEST -> new int[] {f.x() - 2, f.z() + 2};
                case SOUTH -> new int[] {f.x() + 2, f.z() + n + 1};
                default -> new int[] {f.x() + 2, f.z() - 2};
            };
            helper.assertTrue(street.test(out[0], out[1]), "a doorstep, then the street, from " + f);
        }
        helper.succeed();
    }

    /**
     * A farm track's holdings in a town's gardens (John: "something like a voronoi", "strips in parcels"): fields along
     * it either side in strips running away from it, north-south off a track running east-west, none overlapping, none
     * on the track or the avenue, all between the walls.
     */
    @GameTest(template = "floor")
    public static void aFarmTrackHasStripFieldsEitherSide(GameTestHelper helper) {
        var plan = new TownPlans.Plan(new BlockPos(0, 64, 0), new TownPlans.Rect(-30, -30, 30, 30),
                new TownPlans.Rect(-80, -80, 80, 80), true, true);
        double[][] track = {{2.5, -55.5}, {60.5, -55.5}};
        var fields = dev.b28.enterprise.territory.FieldMaps.fieldsAlong(helper.getLevel(), plan,
                dev.b28.enterprise.territory.FieldMaps.Ring.GARDEN, track, 7);
        helper.assertTrue(fields.size() >= 12, "fields both sides: " + fields.size());
        int s = Fields.SIZE;
        boolean north = false, south = false;
        for (int i = 0; i < fields.size(); i++) {
            int[] a = fields.get(i);
            helper.assertTrue(a[2] == 1, "strips run north-south off an east-west track");
            helper.assertTrue(a[1] + s - 1 < -57 || a[1] > -54, "off the track: " + a[0] + "," + a[1]);
            helper.assertTrue(a[0] > 3, "off the avenue: " + a[0] + "," + a[1]);
            boolean inInner = a[0] <= 34 && a[0] + s - 1 >= -34 && a[1] <= 34 && a[1] + s - 1 >= -34;
            boolean inOuter = a[0] >= -76 && a[0] + s - 1 <= 76 && a[1] >= -76 && a[1] + s - 1 <= 76;
            helper.assertTrue(!inInner && inOuter, "between the walls: " + a[0] + "," + a[1]);
            north |= a[1] < -55;
            south |= a[1] > -55;
            for (int j = 0; j < i; j++) {
                int[] b = fields.get(j);
                helper.assertFalse(a[0] < b[0] + s && b[0] < a[0] + s && a[1] < b[1] + s && b[1] < a[1] + s,
                        "two fields overlap: " + a[0] + "," + a[1] + " and " + b[0] + "," + b[1]);
            }
        }
        helper.assertTrue(north && south, "holdings either side");
        helper.succeed();
    }

    /**
     * The decorative twins (`Props`): a barrel, a cauldron and a lectern placed for real are job sites a villager can
     * take; their twins stand the same and are none.
     */
    @GameTest(template = "floor")
    public static void decorativeTwinsAreNoJobSites(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(new BlockPos(1, 2, 1), Blocks.BARREL);
        helper.assertTrue(level.getPoiManager().getType(helper.absolutePos(new BlockPos(1, 2, 1))).isPresent(),
                "a vanilla barrel is a job site");
        int x = 0;
        for (var twin : dev.b28.enterprise.settlement.Props.TWINS.entrySet()) {
            BlockPos at = new BlockPos(x % 7, 2 + x / 7, 3 + x / 7 % 2);
            helper.setBlock(at, twin.getValue().get());
            helper.assertTrue(level.getPoiManager().getType(helper.absolutePos(at)).isEmpty(),
                    "the twin of " + twin.getKey() + " is no job site");
            x++;
        }
        helper.setBlock(new BlockPos(5, 2, 5), dev.b28.enterprise.settlement.Props.CAULDRON.get().defaultBlockState()
                .setValue(dev.b28.enterprise.settlement.Props.Vat.CONTENTS, dev.b28.enterprise.settlement.Props.Contents.BLUE));
        helper.assertTrue(helper.getBlockState(new BlockPos(5, 2, 5)).getShape(level, helper.absolutePos(new BlockPos(5, 2, 5)))
                .equals(Blocks.CAULDRON.defaultBlockState().getShape(level, helper.absolutePos(new BlockPos(5, 2, 5)))),
                "a dye vat stands as a cauldron");
        helper.succeed();
    }

    /**
     * The placer (`Kits`, the asset studio's structures): a kingdom's home kit found for its slot, laid turned to face
     * its street (east here), its two beds where its metadata says, claimable, and its door in the east wall.
     */
    @GameTest(template = "floor", skyAccess = true)
    public static void aKitIsLaidTurnedToItsStreetWithItsBeds(GameTestHelper helper) {
        var level = helper.getLevel();
        var resources = level.getServer().getResourceManager();
        var kits = dev.b28.enterprise.territory.Kits.kits(resources, "test", "home", 5, 5);
        helper.assertTrue(kits.equals(java.util.List.of("enterprise:test/home/cottage_5x5")), "the test kingdom's home: " + kits);
        helper.assertTrue(dev.b28.enterprise.territory.Kits.kits(resources, "test", "home", 4, 5).isEmpty(),
                "none fits a slot too small");
        BlockPos corner = helper.absolutePos(new BlockPos(1, 3, 1));
        var placed = dev.b28.enterprise.territory.Kits.place(level, kits.get(0), corner, net.minecraft.core.Direction.EAST);
        helper.assertTrue(placed.isPresent(), "laid");
        var box = placed.get().box();
        helper.assertTrue(box.minX() == corner.getX() && box.minZ() == corner.getZ() && box.minY() == corner.getY() - 2,
                "its corner where asked, its foundation two below the ground: " + box);
        helper.assertTrue(placed.get().bedHeads().size() == 2, "two beds declared");
        for (BlockPos head : placed.get().bedHeads()) {
            helper.assertTrue(level.getBlockState(head).getBlock() instanceof BedBlock, "a bed's head at " + head);
            helper.assertTrue(level.getPoiManager().getType(head).filter(type -> type.is(PoiTypes.HOME)).isPresent(),
                    "villagers can claim it");
        }
        var door = level.getBlockState(corner.offset(4, 1, 2));
        helper.assertTrue(door.getBlock() instanceof net.minecraft.world.level.block.DoorBlock
                && door.getValue(net.minecraft.world.level.block.DoorBlock.FACING) == net.minecraft.core.Direction.EAST,
                "the door in the east wall, facing east: " + door);
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
