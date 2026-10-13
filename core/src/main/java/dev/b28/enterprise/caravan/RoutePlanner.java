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
import net.minecraft.core.GlobalPos;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.Level;

/**
 * The cheapest hex path for a caravan: each hex costs its road food both ways, and entering another town's land costs
 * that town's toll on the coins the caravan expects to carry home. A town that prices its toll above the detour loses
 * the traffic; that is how towns compete. A* over the hex grid, within a margin around the straight line.
 */
public final class RoutePlanner {
    /** Hexes either side of the straight line the search may stray. */
    private static final int MARGIN = 6;

    private RoutePlanner() {}

    /**
     * @param perHex cost of crossing one hex (food both ways), in spurs
     * @param value coins the caravan expects to carry home, in spurs
     * @param ends the towns at either end of the route (they charge their own caravans nothing)
     */
    public static List<Hex> plan(Territory territory, ResourceKey<Level> dimension, Hex start, Hex goal,
            Set<GlobalPos> ends, double perHex, double value) {
        int direct = start.distance(goal);
        record Node(Hex hex, double f) {}
        PriorityQueue<Node> open = new PriorityQueue<>((a, b) -> Double.compare(a.f, b.f));
        Map<Hex, Double> best = new HashMap<>();
        Map<Hex, Hex> cameFrom = new HashMap<>();
        best.put(start, 0.0);
        open.add(new Node(start, direct * perHex));
        while (!open.isEmpty()) {
            Node node = open.poll();
            Hex hex = node.hex();
            if (hex.equals(goal)) {
                return walkBack(cameFrom, goal);
            }
            double g = best.get(hex);
            if (node.f() > g + hex.distance(goal) * perHex + 1e-9) {
                continue; // stale entry
            }
            Optional<GlobalPos> here = territory.owner(dimension, hex);
            for (Hex next : hex.neighbors()) {
                if (next.distance(start) + next.distance(goal) > direct + 2 * MARGIN) {
                    continue;
                }
                double cost = g + perHex + entryToll(territory, dimension, here, next, ends, value);
                if (cost < best.getOrDefault(next, Double.MAX_VALUE)) {
                    best.put(next, cost);
                    cameFrom.put(next, hex);
                    open.add(new Node(next, cost + next.distance(goal) * perHex));
                }
            }
        }
        return Hex.line(start, goal); // unreachable within the margin: go straight
    }

    /** The toll for stepping into {@code next}: charged on entering a town's land, not within it. */
    static double entryToll(Territory territory, ResourceKey<Level> dimension, Optional<GlobalPos> here, Hex next,
            Set<GlobalPos> ends, double value) {
        Optional<GlobalPos> there = territory.owner(dimension, next);
        if (there.isEmpty() || there.equals(here) || ends.contains(there.get())) {
            return 0;
        }
        return territory.effectiveToll(there.get()) * value;
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
