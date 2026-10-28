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
                new TownPlans.Rect(-80, -80, 80, 80), true, true, 20);
        helper.assertTrue(TownPlans.lanes(plan).size() == 4, "a lane each side of each avenue: " + TownPlans.lanes(plan).size());
        java.util.function.BiPredicate<Integer, Integer> street = (x, z) -> Math.abs(x) <= 1 || Math.abs(z) <= 1
                || x >= 18 && x <= 21 || x >= -22 && x <= -19 || z >= 18 && z <= 21 || z >= -22 && z <= -19;
        // Today's cottage, and the studio's bigger homes (C3: width along the street, depth back from it).
        for (int[] size : new int[][] {{5, 5}, {7, 7}, {9, 6}, {14, 7}}) {
            var homes = TownPlans.frontages(plan, size[0], size[1]);
            helper.assertTrue(homes.size() >= 4, size[0] + " x " + size[1] + ": rows of homes: " + homes.size());
            for (var f : homes) {
                boolean alongZ = f.door().getAxis() == net.minecraft.core.Direction.Axis.X;
                helper.assertTrue(f.sx() == (alongZ ? size[1] : size[0]) && f.sz() == (alongZ ? size[0] : size[1]),
                        "turned to its street: " + f);
                for (int dx = 0; dx < f.sx(); dx++) {
                    for (int dz = 0; dz < f.sz(); dz++) {
                        int x = f.x() + dx, z = f.z() + dz;
                        helper.assertFalse(street.test(x, z) || TownPlans.onLane(plan, x, z), "a home on a street at " + f);
                    }
                }
                int[] out = switch (f.door()) {
                    case EAST -> new int[] {f.x() + f.sx() + 1, f.z() + f.sz() / 2};
                    case WEST -> new int[] {f.x() - 2, f.z() + f.sz() / 2};
                    case SOUTH -> new int[] {f.x() + f.sx() / 2, f.z() + f.sz() + 1};
                    default -> new int[] {f.x() + f.sx() / 2, f.z() - 2};
                };
                helper.assertTrue(street.test(out[0], out[1]), "a doorstep, then the street, from " + f);
            }
        }
        helper.succeed();
    }

    /**
     * A new town's grid (John: "mid density ... multiple floors", "front yard, back yard leeway"): lanes every 39,
     * wards of 33 x 33, plots 16 deep back to back with their yards, as wide as the studio's houses and a side yard
     * each side (11, 14, 17; a terrace of three 23; a whole ward's 33), none on a street.
     */
    @GameTest(template = "floor")
    public static void aNewTownsWardsFrontTheStudiosHouses(GameTestHelper helper) {
        var plan = new TownPlans.Plan(new BlockPos(0, 64, 0), new TownPlans.Rect(-60, -60, 60, 60),
                new TownPlans.Rect(-120, -120, 120, 120), true, true);
        helper.assertTrue(plan.ward() == 39 && plan.cell() == 33 && plan.deepest() == 16 && plan.yards(),
                "wards of 33, plots 16 deep with yards");
        java.util.function.BiPredicate<Integer, Integer> street = (x, z) -> Math.abs(x) <= 2 || Math.abs(z) <= 2
                || x >= 36 && x <= 41 || x >= -42 && x <= -37 || z >= 36 && z <= 41 || z >= -42 && z <= -37;
        for (int width : new int[] {11, 14, 17, 23, 33}) {
            var plots = TownPlans.plots(plan, width);
            helper.assertTrue(plots.size() >= 4, width + " wide: " + plots.size());
            for (var f : plots) {
                helper.assertTrue(Math.max(f.sx(), f.sz()) >= 16 || width >= 16, "16 deep: " + f);
                for (int dx = 0; dx < f.sx(); dx++) {
                    for (int dz = 0; dz < f.sz(); dz++) {
                        helper.assertFalse(street.test(f.x() + dx, f.z() + dz), "a plot on a street: " + f);
                    }
                }
            }
        }
        helper.succeed();
    }

    /**
     * A plot (John: "front yard, back yard leeway"): turned to any street, its house inside it a block in from each side,
     * its front yard between the house and the street, its back yard behind.
     */
    @GameTest(template = "floor")
    public static void aPlotsFrontYardFacesItsStreet(GameTestHelper helper) {
        int w = 11, d = 16;
        for (var door : new net.minecraft.core.Direction[] {net.minecraft.core.Direction.SOUTH, net.minecraft.core.Direction.NORTH,
                net.minecraft.core.Direction.EAST, net.minecraft.core.Direction.WEST}) {
            int[] house = dev.b28.enterprise.territory.Plots.world(0, 0, w, d, door, 1, d - 3 - 9, 9, 9);
            int[] front = dev.b28.enterprise.territory.Plots.world(0, 0, w, d, door, 0, d - 3, w, 3);
            int[] plot = dev.b28.enterprise.territory.Plots.world(0, 0, w, d, door, 0, 0, w, d);
            helper.assertTrue(plot[0] == 0 && plot[1] == 0, "the plot's corner stays put: " + door);
            helper.assertTrue(house[0] >= 0 && house[1] >= 0 && house[0] + house[2] <= plot[2] && house[1] + house[3] <= plot[3],
                    "the house inside its plot: " + door + " " + java.util.Arrays.toString(house));
            int[] street = switch (door) {
                case SOUTH -> new int[] {w / 2, d};
                case NORTH -> new int[] {w / 2, -1};
                case EAST -> new int[] {d, w / 2};
                default -> new int[] {-1, w / 2};
            };
            double fx = front[0] + front[2] / 2.0, fz = front[1] + front[3] / 2.0;
            double hx = house[0] + house[2] / 2.0, hz = house[1] + house[3] / 2.0;
            helper.assertTrue(Math.hypot(fx - street[0], fz - street[1]) < Math.hypot(hx - street[0], hz - street[1]),
                    "the front yard between the house and its street: " + door);
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
                new TownPlans.Rect(-80, -80, 80, 80), true, true, 20);
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
        helper.assertTrue(kits.equals(java.util.List.of("enterprise:test/home/cottage_5x5",
                "enterprise:test/home/cottage_5x5__plain")), "the test kingdom's home and its palette variant: " + kits);
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

    /**
     * Wave-function collapse (`Wfc`, the procedural assembler's solver): a house front of posts and bays, where a post
     * meets only bays and the front ends in posts, comes out alternating, keeps a door pinned where it was put, is the
     * same for a seed and differs between seeds; a front no arrangement can satisfy comes out empty.
     */
    @GameTest(template = "floor")
    public static void wfcAssemblesAFrontThatKeepsItsRules(GameTestHelper helper) {
        var W = dev.b28.enterprise.territory.Wfc.Side.WEST;
        var E = dev.b28.enterprise.territory.Wfc.Side.EAST;
        java.util.function.Function<String, java.util.Map<dev.b28.enterprise.territory.Wfc.Side, String>> bay =
                s -> java.util.Map.of(W, "bay_w", E, "bay_e");
        var modules = java.util.List.of(
                new dev.b28.enterprise.territory.Wfc.Module("post", java.util.Map.of(W, "post_w", E, "post_e"), 1),
                new dev.b28.enterprise.territory.Wfc.Module("plain", bay.apply("plain"), 4),
                new dev.b28.enterprise.territory.Wfc.Module("window", bay.apply("window"), 4),
                new dev.b28.enterprise.territory.Wfc.Module("door", bay.apply("door"), 1));
        var rules = new dev.b28.enterprise.territory.Wfc.Rules(modules,
                java.util.Set.of(java.util.List.of("post_e", "bay_w"), java.util.List.of("bay_e", "post_w"),
                        java.util.List.of("post_w", "end"), java.util.List.of("post_e", "end")),
                java.util.Map.of(W, "end", E, "end"));
        java.util.Map<Integer, Integer> door = java.util.Map.of(3, 3); // the door module in the bay at cell 3
        var a = dev.b28.enterprise.territory.Wfc.solve(rules, 9, 1, 1, (at, m) -> true, door, 42, 5);
        helper.assertTrue(a.isPresent(), "a front of 9");
        var ids = dev.b28.enterprise.territory.Wfc.ids(rules, a.get());
        for (int i = 0; i < 9; i++) {
            helper.assertTrue(ids.get(i).equals("post") == (i % 2 == 0), "posts and bays alternate: " + ids);
        }
        helper.assertTrue(ids.get(3).equals("door"), "the door where it was pinned: " + ids);
        helper.assertTrue(dev.b28.enterprise.territory.Wfc.valid(rules, a.get(), 9, 1, 1), "every join by the rules");
        var again = dev.b28.enterprise.territory.Wfc.solve(rules, 9, 1, 1, (at, m) -> true, door, 42, 5);
        helper.assertTrue(java.util.Arrays.equals(a.get(), again.get()), "the same for a seed");
        boolean differs = false;
        for (long seed = 43; seed < 53 && !differs; seed++) {
            var other = dev.b28.enterprise.territory.Wfc.solve(rules, 9, 1, 1, (at, m) -> true, door, seed, 5);
            differs = !java.util.Arrays.equals(a.get(), other.get());
        }
        helper.assertTrue(differs, "other seeds, other fronts");
        helper.assertTrue(dev.b28.enterprise.territory.Wfc.solve(rules, 8, 1, 1, (at, m) -> true, java.util.Map.of(), 42, 5)
                .isEmpty(), "an even front can't end in posts both ends");
        helper.succeed();
    }

    /**
     * The composer (`Compose`, the studio's module set, thread 0004): each golden house composed from its layout comes out
     * as the studio's own file of it, block for block (its stone palette, the set's own).
     */
    @GameTest(template = "floor")
    public static void theComposerRebuildsTheGoldenHouses(GameTestHelper helper) {
        var level = helper.getLevel();
        var resources = level.getServer().getResourceManager();
        var blocks = dev.b28.enterprise.territory.Compose.blocks(level);
        for (String house : new String[] {"cottage_9x9_b", "townhouse_12x9_b", "terrace_21x9_b"}) {
            var json = resources.getResource(net.minecraft.resources.ResourceLocation.fromNamespaceAndPath("enterprise",
                    "structure/lordaeron/home/" + house + ".json")).orElseThrow();
            com.google.gson.JsonObject meta;
            try (var r = json.openAsReader()) {
                meta = com.google.gson.JsonParser.parseReader(r).getAsJsonObject();
            } catch (java.io.IOException e) {
                throw new IllegalStateException(e);
            }
            var cells = dev.b28.enterprise.territory.Compose.composeAll(resources, blocks, "enterprise:lordaeron/part/timber_relief",
                    meta.getAsJsonObject("golden").getAsJsonArray("layouts"));
            var golden = dev.b28.enterprise.territory.Compose.golden(resources, blocks, "enterprise:lordaeron/home/" + house + "__stone");
            int differ = 0;
            String first = "";
            var keys = new java.util.HashSet<>(golden.keySet());
            keys.addAll(cells.keySet());
            for (var k : keys) {
                if (!java.util.Objects.equals(golden.get(k), cells.get(k))) {
                    if (differ++ == 0) {
                        first = k.toShortString() + ": " + cells.get(k) + " for " + golden.get(k);
                    }
                }
            }
            helper.assertTrue(differ == 0, house + ": " + differ + " of " + golden.size() + " blocks differ, the first " + first);
        }
        helper.succeed();
    }

    /**
     * The wall kit's tiling (`WallKit`, the studio's stone kit): every side closes exactly, corner to corner, a gatehouse
     * centred on each avenue, nothing overlapping (Hearthglen's inner wall: 95 by 59, its gates off-centre).
     */
    @GameTest(template = "floor")
    public static void aWallKitTilesEverySideExactly(GameTestHelper helper) {
        for (int[] side : new int[][] {{95, 55}, {59, 38}, {40, 20}, {130, 61}, {33, 16}}) {
            var pieces = dev.b28.enterprise.territory.WallKit.tileSide(side[0], java.util.List.of(side[1]), 24);
            int x = -3;
            for (var p : pieces) {
                helper.assertTrue(p.at() == x, side[0] + ": a gap or an overlap at " + x + " (" + p + ")");
                x += p.length();
            }
            helper.assertTrue(x == side[0] - 4, side[0] + ": ends at " + x + ", not where the next side's corner begins ("
                    + (side[0] - 4) + ")");
            var gate = pieces.stream().filter(p -> p.piece().startsWith("gatehouse")).findFirst().orElseThrow();
            helper.assertTrue(gate.at() + 5 == side[1], side[0] + ": the gate's passage on the avenue");
        }
        helper.succeed();
    }

    /**
     * The house generator (`Houses`, the studio's plan data): every body class of plan.json at every storey count it
     * allows makes a layout that composes, with the beds its rooms record, and two seeds give two faces.
     */
    @GameTest(template = "floor")
    public static void theGeneratorMakesEveryBodyOfThePlanData(GameTestHelper helper) {
        var resources = helper.getLevel().getServer().getResourceManager();
        var blocks = dev.b28.enterprise.territory.Compose.blocks(helper.getLevel());
        var plan = dev.b28.enterprise.territory.WallKit.json(resources, "town_kits/lordaeron/plan.json").orElseThrow();
        for (var e : plan.getAsJsonObject("bodies").entrySet()) {
            var body = e.getValue().getAsJsonObject();
            int w = body.getAsJsonObject("when").get("W").getAsInt(), d = body.getAsJsonObject("when").get("D").getAsInt();
            var range = body.getAsJsonArray("storeys");
            for (int st = range.get(0).getAsInt(); st <= range.get(1).getAsInt(); st++) {
                String what = e.getKey() + " of " + st;
                java.util.List<String> faces = new java.util.ArrayList<>();
                for (long seed : new long[] {1, 2}) {
                    var layout = dev.b28.enterprise.territory.Houses.layout(
                            new dev.b28.enterprise.territory.Houses.Params(w, d, st, false, false, false, true, false, seed), plan);
                    helper.assertTrue(layout.isPresent(), what + ": no layout");
                    helper.assertTrue(layout.get().getAsJsonArray("beds").size() >= 2, what + ": its beds");
                    var grid = dev.b28.enterprise.territory.Compose.compose(resources, blocks, "enterprise:lordaeron/part/timber_relief",
                            dev.b28.enterprise.territory.Compose.Layout.of(layout.get()));
                    helper.assertTrue(grid.cells.size() > 100, what + ": composed");
                    faces.add(layout.get().getAsJsonArray("shell").toString());
                }
                if (w >= 10) {
                    helper.assertFalse(faces.get(0).equals(faces.get(1)), what + ": two seeds, two faces");
                }
            }
        }
        helper.succeed();
    }

    /**
     * Dalaran's houses (the studio's platform_hall, batch 024): every body of the plan data composed from Dalaran's own
     * parts under one hipped roof, its eave 2 past the relief all round (the template that much wider every side), no
     * chimney through it, and its door where its metadata says.
     */
    @GameTest(template = "floor")
    public static void aDalaranCourtyardHouseIsALotWhole(GameTestHelper helper) {
        // The studio's batch 029: two plots wide, its forecourt its front yard, in a ward's half deep as it is.
        var resources = helper.getLevel().getServer().getResourceManager();
        var plan = new dev.b28.enterprise.territory.TownPlans.Plan(BlockPos.ZERO, null, null, true, true);
        var kit = dev.b28.enterprise.territory.Kits.wholeHome(resources, "dalaran", plan.cell(), plan.deepest(), 1, 0);
        helper.assertTrue(kit.isPresent(), "a Dalaran lot whole within " + plan.cell() + " x " + plan.deepest());
        var m = kit.get();
        helper.assertTrue(m.whole() && m.plots() == 2, m.id() + ": two plots");
        helper.assertTrue(m.width() == 22 && m.depth() == 16 && m.bedHeads().size() == 4, m.id() + ": 22 x 16 with 4 beds, "
                + m.width() + " x " + m.depth() + " with " + m.bedHeads().size());
        // Lordaeron has none: its homes are houses on plots.
        helper.assertTrue(dev.b28.enterprise.territory.Kits.wholeHome(resources, "lordaeron", 99, 99, 1, 0).isEmpty(),
                "no Lordaeron lot whole");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aDalaranHouseStandsUnderOneHippedRoof(GameTestHelper helper) {
        for (int[] body : new int[][] {{7, 7, 1}, {10, 7, 2}, {13, 7, 3}, {10, 10, 2}}) {
            String what = body[0] + "x" + body[1] + " of " + body[2];
            var built = dev.b28.enterprise.territory.Houses.build(helper.getLevel(), "dalaran",
                    new dev.b28.enterprise.territory.Houses.Params(body[0], body[1], body[2], false, false, false, false, false, 7), null);
            helper.assertTrue(built.isPresent(), what + ": built");
            var b = built.get();
            helper.assertTrue(b.sx() == body[0] + 2 + 4 && b.sz() == body[1] + 2 + 4, what + ": the eave all round, " + b.sx() + " x " + b.sz());
            // The roof: purpur at the template's corner (the eave's tip), nothing of a chimney.
            boolean purpur = false, chimney = false;
            for (var e : b.cells().entrySet()) {
                String id = net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(e.getValue().getBlock()).getPath();
                purpur |= id.startsWith("purpur") && (e.getKey().getX() <= 1 || e.getKey().getZ() <= 1);
                chimney |= id.equals("bricks") && e.getKey().getY() > 2 + 5 * body[2];
            }
            helper.assertTrue(purpur, what + ": its hipped roof's eave reaches the template's edge");
            helper.assertFalse(chimney, what + ": no stack through the roof");
            var door = b.meta().entrances().get(0);
            var at = b.cells().get(door);
            helper.assertTrue(at != null && at.getBlock() instanceof net.minecraft.world.level.block.DoorBlock,
                    what + ": the door at " + door.toShortString() + " is " + at);
        }
        helper.succeed();
    }

    /**
     * Yards from tiles (`Yards`, the studio's batch 003): a front yard of every plot width a ward gives is filled exactly,
     * its path tile at the door's column and nowhere else, neighbours' edges meeting, a lamp at most every 5 columns.
     */
    @GameTest(template = "floor")
    public static void yardTilesFillAPlotsWidth(GameTestHelper helper) {
        var tiles = dev.b28.enterprise.territory.Yards.tiles(helper.getLevel().getServer().getResourceManager(), "lordaeron");
        helper.assertFalse(tiles.isEmpty(), "Lordaeron's yard tiles");
        for (int width : new int[] {9, 11, 12, 14, 17, 23}) {
            for (int door = 2; door < width - 2; door += 3) {
                for (long seed = 1; seed <= 3; seed++) {
                    var strip = dev.b28.enterprise.territory.Yards.fill(tiles, "yard:front", width, door,
                            dev.b28.enterprise.territory.Yards.likes("village", "smith"), seed);
                    helper.assertTrue(strip.isPresent(), width + " wide, door at " + door + ": no strip");
                    int x = 0, lamp = -3;
                    dev.b28.enterprise.territory.Yards.Tile last = null;
                    for (var l : strip.get()) {
                        helper.assertTrue(l.at() == x, width + ": a gap at " + x);
                        helper.assertTrue(l.tile().tags().contains("path") == (l.at() == door), width + ": the path at the door");
                        if (last != null) {
                            helper.assertTrue(last.right().equals("*") || l.tile().left().equals("*")
                                    || last.right().equals(l.tile().left()), width + ": edges meet");
                        }
                        int here = l.tile().lamp() ? l.at() + l.tile().width() / 2 : l.at() == door ? door : lamp;
                        helper.assertTrue(here - lamp <= 5, width + ": a lamp in reach");
                        lamp = here;
                        x += l.tile().width();
                        last = l.tile();
                    }
                    helper.assertTrue(x == width, width + ": fills it");
                    helper.assertTrue(strip.get().stream().filter(l -> l.tile().tags().stream()
                            .anyMatch(g -> g.startsWith("trade:"))).count() <= 1, width + ": a trade's tile once");
                    var laid = strip.get();
                    for (int i = 0, run = 0; i < laid.size(); i++) {
                        run = i > 0 && laid.get(i - 1).tile() == laid.get(i).tile() ? run + 1 : 1;
                        int most = laid.get(i).tile().maxRun();
                        helper.assertTrue(most == 0 || run <= most, width + ": " + laid.get(i).tile().id() + " within its run");
                    }
                }
            }
        }
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
