package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.caravan.Route;
import dev.b28.enterprise.caravan.RoutePlanner;
import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.territory.Terrain;
import dev.b28.enterprise.territory.Territory;
import java.util.List;
import java.util.Set;
import java.util.function.ToDoubleFunction;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

// Pure ledger tests: each keeps its own Territory; hexes are far from any test arena.
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class TollTests {
    private static final Hex START = new Hex(0, 0);
    private static final Hex GOAL = new Hex(12, 0);
    private static final double PER_HEX = 8.5; // road food both ways at bread prices
    private static final double VALUE = 1000; // coins a caravan carries home
    private static final Hex BRIDGE = new Hex(6, 0);
    /** A river along q = 6, crossable only at the bridge: every path from START to GOAL crosses q = 6. */
    private static final ToDoubleFunction<Hex> RIVER =
            hex -> hex.q() == 6 && !hex.equals(BRIDGE) ? Double.POSITIVE_INFINITY : 1;

    /** A town holding its hex and two rings around the bridge hex: 19 hexes across the road. */
    private static GlobalPos townAcrossTheRoad(GameTestHelper helper, Territory territory, double toll) {
        GlobalPos town = GlobalPos.of(helper.getLevel().dimension(), new BlockPos(999_000, 64, 999_000));
        for (int dq = -2; dq <= 2; dq++) {
            for (int dr = -2; dr <= 2; dr++) {
                Hex hex = new Hex(BRIDGE.q() + dq, BRIDGE.r() + dr);
                if (hex.distance(BRIDGE) <= 2) {
                    territory.claim(town, hex);
                }
            }
        }
        territory.setTollRate(town, toll);
        territory.setGuards(town, 19); // a post on every hex
        return town;
    }

    private static List<Hex> plan(GameTestHelper helper, Territory territory) {
        return RoutePlanner.plan(territory, helper.getLevel().dimension(), START, GOAL, Set.of(), PER_HEX, VALUE);
    }

    /** A route from START's centre to GOAL's centre. */
    private static Route route(GameTestHelper helper) {
        var dimension = helper.getLevel().dimension();
        return new CaravanLedger().addRoute(
                GlobalPos.of(dimension, BlockPos.containing(START.centerX(), 64, START.centerZ())),
                GlobalPos.of(dimension, BlockPos.containing(GOAL.centerX(), 64, GOAL.centerZ())));
    }

    @GameTest(template = "floor")
    public static void withNoTollsThePathIsStraight(GameTestHelper helper) {
        List<Hex> path = plan(helper, new Territory());
        helper.assertTrue(path.size() == START.distance(GOAL) + 1, "straight: " + path.size() + " hexes");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aPriceyTownIsGoneAround(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos town = townAcrossTheRoad(helper, territory, 0.25); // 250 spurs a crossing
        List<Hex> path = plan(helper, territory);
        var dimension = helper.getLevel().dimension();
        helper.assertTrue(path.stream().noneMatch(hex -> territory.owner(dimension, hex).filter(town::equals).isPresent()),
                "the path skirts the town: " + path);
        helper.assertTrue(path.size() > START.distance(GOAL) + 1, "taking a few more hexes");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aCheapTollIsPaid(GameTestHelper helper) {
        Territory territory = new Territory();
        townAcrossTheRoad(helper, territory, 0.01); // 10 spurs a crossing, less than any detour
        List<Hex> path = plan(helper, territory);
        helper.assertTrue(path.size() == START.distance(GOAL) + 1, "straight through: " + path.size() + " hexes");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void onlyPostsCollect(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos town = townAcrossTheRoad(helper, territory, 0.25);
        territory.setGuards(town, 0);
        helper.assertTrue(plan(helper, territory).size() == START.distance(GOAL) + 1, "no posts: straight through");
        helper.assertTrue(Caravans.payTolls(territory, route(helper), 1000) == 0, "and nothing paid");
        territory.setPosts(town, List.of(BRIDGE));
        List<Hex> path = plan(helper, territory);
        helper.assertTrue(!path.contains(BRIDGE), "one post on open ground is stepped around: " + path);
        helper.assertTrue(path.size() <= START.distance(GOAL) + 2, "for a hex at most: " + path.size());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void waterCantBeCrossed(GameTestHelper helper) {
        Territory territory = new Territory();
        var dimension = helper.getLevel().dimension();
        var plan = RoutePlanner.plan(territory, dimension, START, GOAL, Set.of(), PER_HEX, RIVER, VALUE, Set.of());
        helper.assertTrue(plan.found() && plan.path().contains(BRIDGE), "the way over is the bridge: " + plan.path());
        var blocked = RoutePlanner.plan(territory, dimension, START, GOAL, Set.of(), PER_HEX, RIVER, VALUE, Set.of(BRIDGE));
        helper.assertTrue(!blocked.found(), "without it there is no way: " + blocked.path());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aTownOnOpenGroundPostsNoGuard(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos town = townAcrossTheRoad(helper, territory, Territory.DEFAULT_TOLL_RATE);
        territory.setGuards(town, 0);
        Route route = route(helper);
        long t = 1_000_000;
        territory.reviewTolls(town, t, 16);
        for (int day = 0; day < 3; day++) {
            Caravans.payTolls(territory, route, 1000, PER_HEX, RoutePlanner.FLAT);
        }
        var seen = territory.crossings(town).get(route.id());
        helper.assertTrue(seen != null && seen.options().get(0).detour() <= PER_HEX + 1e-6,
                "one post can be gone around for a hex's food at most: " + (seen == null ? null : seen.options()));
        territory.reviewTolls(town, t + Territory.TOLL_REVIEW_TICKS, 16);
        helper.assertTrue(territory.guards(town) == 0, "no chokepoint, so no guard: " + territory.posts(town));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aTownHoldingTheOnlyCrossingTollsIt(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos town = townAcrossTheRoad(helper, territory, Territory.DEFAULT_TOLL_RATE);
        territory.setGuards(town, 0);
        Route route = route(helper);
        long t = 1_000_000;
        territory.reviewTolls(town, t, 16);
        for (int day = 0; day < 3; day++) {
            Caravans.payTolls(territory, route, 1000, PER_HEX, RIVER);
        }
        territory.reviewTolls(town, t + Territory.TOLL_REVIEW_TICKS, 16);
        helper.assertTrue(territory.posts(town).equals(Set.of(BRIDGE)), "a guard on the bridge: " + territory.posts(town));
        helper.assertTrue(territory.tollRate(town) == Territory.MAX_TOLL_RATE,
                "no way around, so the most it may ask: " + territory.tollRate(town));
        int paid = Caravans.payTolls(territory, route, 1000, PER_HEX, RIVER);
        helper.assertTrue(paid == 250, "the next caravan pays 25%: " + paid);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aTownTollsUpToTheDetour(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos town = townAcrossTheRoad(helper, territory, Territory.DEFAULT_TOLL_RATE);
        territory.setGuards(town, 0);
        Route route = route(helper);
        // The river has a second, far crossing, 6 hexes off the road: going around the bridge costs a long detour.
        Hex ford = new Hex(6, -6);
        ToDoubleFunction<Hex> twoCrossings = hex -> hex.q() == 6 && !hex.equals(BRIDGE) && !hex.equals(ford)
                ? Double.POSITIVE_INFINITY : 1;
        long t = 1_000_000;
        territory.reviewTolls(town, t, 16);
        for (int day = 0; day < 3; day++) {
            Caravans.payTolls(territory, route, 1000, PER_HEX, twoCrossings);
        }
        var option = territory.crossings(town).get(route.id()).options().get(0);
        helper.assertTrue(option.posts().equals(List.of(BRIDGE)), "first try: a post on the bridge: " + option);
        double detour = option.detour();
        territory.reviewTolls(town, t + Territory.TOLL_REVIEW_TICKS, 16);
        helper.assertTrue(territory.posts(town).equals(Set.of(BRIDGE)), "the bridge is posted: " + territory.posts(town));
        double expected = 0.8 * detour / 1000;
        helper.assertTrue(Math.abs(territory.tollRate(town) - expected) < 1e-9 && expected < Territory.MAX_TOLL_RATE,
                "toll at 80% of the " + detour + "-spur detour: " + territory.tollRate(town));
        var path = RoutePlanner.plan(territory, helper.getLevel().dimension(), START, GOAL, Set.of(), PER_HEX,
                twoCrossings, 1000, Set.of()).path();
        helper.assertTrue(path.contains(BRIDGE), "so caravans still take the bridge: " + path);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void twoBridgesTakeTwoPosts(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos town = townAcrossTheRoad(helper, territory, Territory.DEFAULT_TOLL_RATE);
        territory.setGuards(town, 0);
        Route route = route(helper);
        Hex second = new Hex(6, 1); // next to the bridge, also the town's
        ToDoubleFunction<Hex> twoBridges = hex -> hex.q() == 6 && !hex.equals(BRIDGE) && !hex.equals(second)
                ? Double.POSITIVE_INFINITY : 1;
        long t = 1_000_000;
        territory.reviewTolls(town, t, 16);
        for (int day = 0; day < 3; day++) {
            Caravans.payTolls(territory, route, 1000, PER_HEX, twoBridges);
        }
        var options = territory.crossings(town).get(route.id()).options();
        helper.assertTrue(options.size() == 2 && options.get(0).detour() <= PER_HEX + 1e-6
                && Double.isInfinite(options.get(1).detour()), "one bridge is gone around, both aren't: " + options);
        territory.reviewTolls(town, t + Territory.TOLL_REVIEW_TICKS, 16);
        for (int day = 0; day < 3; day++) {
            Caravans.payTolls(territory, route, 1000, PER_HEX, twoBridges);
        }
        territory.reviewTolls(town, t + 2 * Territory.TOLL_REVIEW_TICKS, 16); // a guard hired per review
        helper.assertTrue(territory.posts(town).equals(Set.of(BRIDGE, second)), "both posted: " + territory.posts(town));
        helper.assertTrue(territory.tollRate(town) == Territory.MAX_TOLL_RATE, "at the most it may ask");
        helper.succeed();
    }

    @GameTest(template = "floor", skyAccess = true) // no barrier ceiling over the arena: the survey reads the surface
    public static void surveysSeeWater(GameTestHelper helper) {
        // Find a survey column inside the arena and flood its floor.
        BlockPos column = null;
        for (int x = 1; x <= 5 && column == null; x++) {
            for (int z = 1; z <= 5 && column == null; z++) {
                BlockPos abs = helper.absolutePos(new BlockPos(x, 1, z));
                if ((abs.getX() & 15) % 4 == 2 && (abs.getZ() & 15) % 4 == 2) {
                    column = new BlockPos(x, 1, z);
                }
            }
        }
        helper.assertTrue(column != null, "a survey column in the arena");
        var chunk = helper.getLevel().getChunkAt(helper.absolutePos(column));
        int before = Terrain.survey(chunk);
        helper.setBlock(column, Blocks.WATER);
        helper.assertTrue(Terrain.survey(chunk) == before + 1, "one more wet column: " + before + " -> " + Terrain.survey(chunk));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aFloodedHexIsImpassable(GameTestHelper helper) {
        Terrain terrain = new Terrain();
        var level = helper.getLevel();
        Hex lake = new Hex(5000, 5000);
        helper.assertTrue(terrain.crossingFactor(level, lake) == 1, "dry land by default");
        for (long chunk : Terrain.chunksOf(lake)) {
            terrain.record(level, chunk, Terrain.SAMPLES);
        }
        helper.assertTrue(Double.isInfinite(terrain.crossingFactor(level, lake)), "flooded: impassable");
        helper.succeed();
    }
}
