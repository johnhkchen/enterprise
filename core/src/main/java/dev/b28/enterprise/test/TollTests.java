package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.caravan.RoutePlanner;
import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.territory.Territory;
import java.util.List;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
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

    /** A town holding its hex and two rings around a hex on the straight line: 19 hexes across the road. */
    private static GlobalPos townAcrossTheRoad(GameTestHelper helper, Territory territory, double toll) {
        GlobalPos town = GlobalPos.of(helper.getLevel().dimension(), new BlockPos(999_000, 64, 999_000));
        Hex centre = new Hex(6, 0);
        for (int dq = -2; dq <= 2; dq++) {
            for (int dr = -2; dr <= 2; dr++) {
                Hex hex = new Hex(centre.q() + dq, centre.r() + dr);
                if (hex.distance(centre) <= 2) {
                    territory.claim(town, hex);
                }
            }
        }
        territory.setTollRate(town, toll);
        territory.setGuards(town, 19); // fully garrisoned: the whole toll is collected
        return town;
    }

    private static List<Hex> plan(GameTestHelper helper, Territory territory) {
        return RoutePlanner.plan(territory, helper.getLevel().dimension(), START, GOAL, Set.of(), PER_HEX, VALUE);
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
    public static void anUnguardedTownCollectsNothing(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos town = townAcrossTheRoad(helper, territory, 0.25);
        territory.setGuards(town, 0);
        helper.assertTrue(territory.effectiveToll(town) == 0, "no guards, no toll");
        List<Hex> path = plan(helper, territory);
        helper.assertTrue(path.size() == START.distance(GOAL) + 1, "so caravans go straight through");
        territory.setGuards(town, 10);
        helper.assertTrue(Math.abs(territory.effectiveToll(town) - 0.25 * 10 / 19.0) < 1e-9,
                "10 guards on 19 hexes collect that share of the toll: " + territory.effectiveToll(town));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void townsHireGuardsWhereTrafficPays(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos town = townAcrossTheRoad(helper, territory, 0.05);
        territory.setGuards(town, 0);
        double guardCost = Territory.guardCostPerDay(16); // spurs a day
        // Traffic worth 19 hexes x a guard's cost / 5%, a day: each guard's hex pays for itself 1.5 times over.
        territory.recordTraffic(town, (int) Math.ceil(1.5 * 19 * guardCost / 0.05 * 3));
        territory.reviewGarrison(town, 3, 16);
        helper.assertTrue(territory.guards(town) == 1, "busy road: a guard is hired");
        territory.reviewGarrison(town, 3, 16); // three quiet days
        helper.assertTrue(territory.guards(town) == 0, "quiet road: the guard is let go");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void townsChaseTollIncome(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos town = GlobalPos.of(helper.getLevel().dimension(), new BlockPos(999_000, 64, 999_000));
        long t = 1_000_000;
        territory.reviewTolls(town, t); // starts the clock at 5%
        territory.oweToll(town, 100);
        territory.reviewTolls(town, t + Territory.TOLL_REVIEW_TICKS); // first day's income: keep raising
        helper.assertTrue(Math.abs(territory.tollRate(town) - 0.06) < 1e-9, "up to 6%: " + territory.tollRate(town));
        territory.oweToll(town, 150);
        territory.reviewTolls(town, t + 2 * Territory.TOLL_REVIEW_TICKS); // more income: keep going
        helper.assertTrue(Math.abs(territory.tollRate(town) - 0.07) < 1e-9, "up to 7%: " + territory.tollRate(town));
        territory.oweToll(town, 50);
        territory.reviewTolls(town, t + 3 * Territory.TOLL_REVIEW_TICKS); // income fell: reverse
        helper.assertTrue(Math.abs(territory.tollRate(town) - 0.06) < 1e-9, "back to 6%: " + territory.tollRate(town));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aTownWithNoTrafficLowersItsToll(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos town = GlobalPos.of(helper.getLevel().dimension(), new BlockPos(998_000, 64, 999_000));
        long t = 1_000_000;
        territory.reviewTolls(town, t);
        territory.reviewTolls(town, t + Territory.TOLL_REVIEW_TICKS);
        territory.reviewTolls(town, t + 2 * Territory.TOLL_REVIEW_TICKS);
        helper.assertTrue(territory.tollRate(town) < Territory.DEFAULT_TOLL_RATE, "lowered: " + territory.tollRate(town));
        helper.succeed();
    }
}
