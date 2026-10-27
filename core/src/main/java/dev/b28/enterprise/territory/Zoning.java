package dev.b28.enterprise.territory;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;

/**
 * A planned town's next house by its kingdom's zoning (the studio's `town_rules/<kingdom>.json` `zoning`, thread 0012:
 * "the plan comes first, the houses fill it"): the plots taken in the rules' `fill_order` (the avenues near the forum
 * first, then out along them, then the lanes), each street class and band (its distance from the bell) giving its body,
 * kind, storeys, palette and shop; a terrace's houses side by side along their street with party walls between, the last
 * of a run a plain side to its lane. The first free plot of the first entry in order that has one, with its house.
 */
public final class Zoning {
    private Zoning() {}

    /** A plot and the house its zoning gives it. */
    public record Pick(Houses.Built house, Plots.Site site, String street, String band, String palette) {}

    public static Optional<Pick> next(ServerLevel level, long town, TownPlans.Plan plan, String kingdom, long seed) {
        var resources = level.getServer().getResourceManager();
        var rules = WallKit.json(resources, "town_rules/" + kingdom + ".json");
        if (rules.isEmpty() || !rules.get().has("zoning") || !plan.lanes() || !plan.yards()) {
            return Optional.empty();
        }
        JsonObject zoning = rules.get().getAsJsonObject("zoning");
        BlockPos bell = plan.bell();
        int depth = plan.deepest();
        for (JsonElement e : zoning.getAsJsonArray("fill_order")) {
            String[] entry = e.getAsString().split(":");
            String street = entry[0], band = entry.length > 1 ? entry[1] : null;
            if (!street.equals("avenue") && !street.equals("lane") && !street.equals("forum_front")) {
                continue; // ward backs and the fringe: none in this grid, or not houses
            }
            JsonObject rule = rule(zoning, street, band);
            if (rule == null || !rule.has("body")) {
                continue;
            }
            long h = mix(seed, e.getAsString().hashCode());
            String[] body = weighted(rule.get("body"), h).split("x");
            int W = Integer.parseInt(body[0]), D = Integer.parseInt(body[1]);
            boolean terrace = rule.has("kind") && rule.get("kind").getAsString().startsWith("terrace");
            int width = terrace ? W : Plots.width(W);
            List<TownPlans.Frontage> all = new ArrayList<>(TownPlans.frontages(plan, width, depth, 0));
            if (!street.equals("lane")) {
                all.addAll(TownPlans.suburbs(plan, width, depth));
            }
            List<TownPlans.Frontage> fit = all.stream()
                    .filter(f -> street.equals("lane") != TownPlans.onAvenue(plan, f))
                    .filter(f -> street.equals("forum_front") ? distance(f, bell) < FORUM : band == null || band(zoning, distance(f, bell)).equals(band))
                    .sorted(Comparator.comparingDouble((TownPlans.Frontage f) -> distance(f, bell)).thenComparingInt(TownPlans.Frontage::x)
                            .thenComparingInt(TownPlans.Frontage::z))
                    .toList();
            for (TownPlans.Frontage f : fit) {
                Optional<Plots.Site> site = Plots.fits(level, f, bell, depth, W, D, W / 2);
                if (site.isEmpty()) {
                    continue;
                }
                long ph = mix(seed, f.x() * 31L + f.z());
                // Party walls toward the run's next plots (a terrace), a plain side where the run ends at a lane.
                boolean partyWest = false, partyEast = false;
                if (terrace) {
                    boolean prev = all.contains(step(f, -width)), next = all.contains(step(f, width));
                    boolean startIsWest = f.door() == Direction.SOUTH || f.door() == Direction.WEST;
                    partyWest = startIsWest ? prev : next;
                    partyEast = startIsWest ? next : prev;
                }
                int storeys = W < 10 ? 1 : pickInt(rule.get("storeys"), ph); // a 7-wide body is one storey (thread 0008)
                boolean shop = rule.has("shop") && (rule.get("shop").isJsonPrimitive() && rule.get("shop").getAsJsonPrimitive().isBoolean()
                        ? rule.get("shop").getAsBoolean() : Math.floorMod(ph, 2) == 0);
                String palette = rule.has("palette") ? weighted(rule.get("palette"), ph >>> 7) : null;
                // A run's end in the core and inner bands stone (the rules' corner_plots).
                if (terrace && (!partyWest || !partyEast) && rules.get().has("palettes_by_tier")
                        && rules.get().getAsJsonObject("palettes_by_tier").has("corner_plots")
                        && band != null && (band.equals("core") || band.equals("inner"))) {
                    palette = rules.get().getAsJsonObject("palettes_by_tier").get("corner_plots").getAsString();
                }
                var built = Houses.build(level, kingdom, new Houses.Params(W, D, storeys, partyWest, partyEast, shop, false, false,
                        ph), palette);
                if (built.isEmpty()) {
                    continue;
                }
                return Optional.of(new Pick(built.get(), site.get(), street, band, palette));
            }
        }
        return Optional.empty();
    }

    /** A forum front: an avenue plot this near the bell. */
    static final double FORUM = 24;

    /** The first rule for the street and band (a rule with neither matches any). */
    static JsonObject rule(JsonObject zoning, String street, String band) {
        for (JsonElement e : zoning.getAsJsonArray("rules")) {
            JsonObject r = e.getAsJsonObject();
            if (r.has("street") && !r.get("street").getAsString().equals(street)) {
                continue;
            }
            if (r.has("band") && band != null && !matches(r.get("band"), band)) {
                continue;
            }
            if (r.has("band") && band == null && !street.equals("forum_front")) {
                continue;
            }
            return r;
        }
        return null;
    }

    private static boolean matches(JsonElement e, String band) {
        if (e.isJsonArray()) {
            for (JsonElement b : e.getAsJsonArray()) {
                if (b.getAsString().equals(band)) {
                    return true;
                }
            }
            return false;
        }
        return e.getAsString().equals(band);
    }

    /** The band a distance from the bell falls in. */
    static String band(JsonObject zoning, double d) {
        for (JsonElement e : zoning.getAsJsonArray("bands")) {
            JsonObject b = e.getAsJsonObject();
            if ((!b.has("from") || d >= b.get("from").getAsDouble()) && (!b.has("to") || d < b.get("to").getAsDouble())) {
                return b.get("name").getAsString();
            }
        }
        return "fringe";
    }

    static double distance(TownPlans.Frontage f, BlockPos bell) {
        return Math.hypot(f.x() + f.sx() / 2.0 - bell.getX(), f.z() + f.sz() / 2.0 - bell.getZ());
    }

    /** The frontage {@code by} along its street. */
    static TownPlans.Frontage step(TownPlans.Frontage f, int by) {
        return f.door().getAxis() == Direction.Axis.Z ? new TownPlans.Frontage(f.x() + by, f.z(), f.door(), f.sx(), f.sz())
                : new TownPlans.Frontage(f.x(), f.z() + by, f.door(), f.sx(), f.sz());
    }

    /** A value or a weighted choice ({"stone": 2, "brick": 1}) by the hash. */
    static String weighted(JsonElement e, long h) {
        if (!e.isJsonObject()) {
            return e.getAsString();
        }
        double total = 0;
        for (var kv : e.getAsJsonObject().entrySet()) {
            total += kv.getValue().getAsDouble();
        }
        double pick = (Math.floorMod(h, 1_000_003L) / 1_000_003.0) * total;
        String last = null;
        for (var kv : e.getAsJsonObject().entrySet()) {
            last = kv.getKey();
            pick -= kv.getValue().getAsDouble();
            if (pick < 0) {
                return last;
            }
        }
        return last;
    }

    /** A number or a range ([2, 3]) by the hash; 1 if none. */
    static int pickInt(JsonElement e, long h) {
        if (e == null) {
            return 1;
        }
        if (e.isJsonArray()) {
            JsonArray a = e.getAsJsonArray();
            int lo = a.get(0).getAsInt(), hi = a.get(a.size() - 1).getAsInt();
            return lo + (int) Math.floorMod(h, (long) (hi - lo + 1));
        }
        return e.getAsInt();
    }

    static long mix(long a, long b) {
        long h = a * 0x9E3779B97F4A7C15L ^ b * 0xC2B2AE3D27D4EB4FL;
        h ^= h >>> 31;
        h *= 0xBF58476D1CE4E5B9L;
        return h ^ h >>> 29;
    }
}
