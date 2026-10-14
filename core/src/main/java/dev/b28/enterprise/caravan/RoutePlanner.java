package dev.b28.enterprise.caravan;

import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.territory.Territory;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.PriorityQueue;
import java.util.Set;
import java.util.function.ToDoubleFunction;
import net.minecraft.core.GlobalPos;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.Level;

/**
 * The cheapest hex path for a caravan: each hex costs its road food both ways, times how hard it is to cross (water
 * to skirt; a hex mostly under water can't be crossed), and passing a town's toll post costs that town's toll on the
 * coins the caravan expects to carry home. A town that prices its toll above the detour around its post loses the
 * traffic; that is how towns compete. A* over the hex grid, within a margin around the straight line.
 */
public final class RoutePlanner {
    /** Hexes either side of the straight line the search may stray. */
    private static final int MARGIN = 6;

    /** Open ground everywhere. */
    public static final ToDoubleFunction<Hex> FLAT = hex -> 1;

    /** A path and what it costs, in spurs. An empty path (infinite cost) when there is no way through. */
    public record Plan(List<Hex> path, double cost) {
        public boolean found() {
            return !path.isEmpty();
        }
    }

    private RoutePlanner() {}

    /** As {@link #plan(Territory, ResourceKey, Hex, Hex, Set, double, ToDoubleFunction, double, Set)} on open ground. */
    public static List<Hex> plan(Territory territory, ResourceKey<Level> dimension, Hex start, Hex goal,
            Set<GlobalPos> free, double perHex, double value) {
        return plan(territory, dimension, start, goal, free, perHex, FLAT, value, Set.of()).path();
    }

    /**
     * @param free towns that charge this caravan nothing (the route's own ends)
     * @param perHex cost of crossing one hex of open ground (food both ways), in spurs
     * @param factor how much harder each hex is to cross than open ground (at least 1; infinite to forbid it)
     * @param value coins the caravan expects to carry home, in spurs
     * @param avoid hexes treated as impassable (the start and goal never are)
     */
    public static Plan plan(Territory territory, ResourceKey<Level> dimension, Hex start, Hex goal,
            Set<GlobalPos> free, double perHex, ToDoubleFunction<Hex> factor, double value, Set<Hex> avoid) {
        int direct = start.distance(goal);
        record Node(Hex hex, double f) {}
        PriorityQueue<Node> open = new PriorityQueue<>((a, b) -> Double.compare(a.f, b.f));
        Map<Hex, Double> best = new HashMap<>();
        Map<Hex, Hex> cameFrom = new HashMap<>();
        Map<Hex, Double> factors = new HashMap<>();
        best.put(start, 0.0);
        open.add(new Node(start, direct * perHex));
        while (!open.isEmpty()) {
            Node node = open.poll();
            Hex hex = node.hex();
            if (hex.equals(goal)) {
                return new Plan(walkBack(cameFrom, goal), best.get(goal));
            }
            double g = best.get(hex);
            if (node.f() > g + hex.distance(goal) * perHex + 1e-9) {
                continue; // stale entry
            }
            for (Hex next : hex.neighbors()) {
                if (next.distance(start) + next.distance(goal) > direct + 2 * MARGIN) {
                    continue;
                }
                boolean end = next.equals(goal) || next.equals(start);
                if (!end && avoid.contains(next)) {
                    continue;
                }
                double hard = end ? 1 : factors.computeIfAbsent(next, factor::applyAsDouble);
                if (Double.isInfinite(hard)) {
                    continue;
                }
                double cost = g + perHex * hard + postToll(territory, dimension, hex, next, free, value);
                if (cost < best.getOrDefault(next, Double.MAX_VALUE)) {
                    best.put(next, cost);
                    cameFrom.put(next, hex);
                    open.add(new Node(next, cost + next.distance(goal) * perHex));
                }
            }
        }
        return new Plan(List.of(), Double.POSITIVE_INFINITY);
    }

    /**
     * The toll for stepping into {@code next}: charged on reaching a town's toll post, not again at a post of the same
     * town next to it.
     */
    static double postToll(Territory territory, ResourceKey<Level> dimension, Hex here, Hex next, Set<GlobalPos> free,
            double value) {
        Optional<GlobalPos> post = territory.postOwner(dimension, next);
        if (post.isEmpty() || free.contains(post.get()) || territory.postOwner(dimension, here).equals(post)) {
            return 0;
        }
        return territory.tollRate(post.get()) * value + territory.passageFee(post.get());
    }

    private static List<Hex> walkBack(Map<Hex, Hex> cameFrom, Hex goal) {
        List<Hex> path = new ArrayList<>();
        for (Hex hex = goal; hex != null; hex = cameFrom.get(hex)) {
            path.add(hex);
        }
        Collections.reverse(path);
        return path;
    }
}
