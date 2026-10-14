package dev.b28.enterprise.territory;

import dev.b28.enterprise.hex.Hex;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

/**
 * How a town works out whether it holds a chokepoint, and what to charge there. Each caravan that crosses its land
 * shows it what guarding that land would do: post a guard where the caravan crossed, see where it would cross instead,
 * post a guard there too, and so on, noting each time what going around the posts would cost the caravan (extra road
 * food and other towns' tolls). Land nobody can go around cheaply is a chokepoint: a bridge, a pass, the one dry hex.
 * The town picks the toll and posts that bring in the most after paying the guards, asking no more than a share of the
 * detour so caravans keep coming.
 */
public final class Chokepoints {
    /** A town asks at most this share of what going around would cost, so caravans still come through. */
    public static final double DETOUR_SHARE = 0.8;

    /** Tolls below this aren't worth posting a guard for. */
    public static final double MIN_RATE = 0.005;

    /** Posts a town tries per route before giving up: land that takes more to close isn't a chokepoint. */
    public static final int MAX_POSTS_PER_ROUTE = 6;

    /** Guards on these hexes would make a caravan go around at this extra cost, in spurs (infinite: no way). */
    public record Option(List<Hex> posts, double detour) {}

    /** What a town has seen of one caravan route crossing its land. */
    public static final class Crossing {
        /** Ever larger sets of posts, each holding the last one's hexes plus one. */
        final List<Option> options = new ArrayList<>();
        double value; // coins carried past per trip, averaged, in spurs
        double tripsSince;
        double tripsPerDay;
        int quietReviews; // reviews in a row with no trip

        public List<Option> options() {
            return List.copyOf(options);
        }

        public double value() {
            return value;
        }

        public double tripsPerDay() {
            return tripsPerDay;
        }

        /**
         * The most this route would pay at a set of posts, as a share of what it carries: {@code share} of the detour,
         * at most {@code cap}.
         */
        double bearableRate(Option option, double share, double cap) {
            return value <= 0 ? 0 : Math.min(cap, share * option.detour() / value);
        }

        /** The fewest posts that make this route bear {@code rate}, if any. */
        Option fewestPostsFor(double rate, double share, double cap) {
            return options.stream().filter(o -> bearableRate(o, share, cap) >= rate - 1e-9).findFirst().orElse(null);
        }

        double incomePerDay(double rate) {
            return rate * value * tripsPerDay;
        }
    }

    /**
     * The toll and posts a town settles on, and the tolls a day they should bring in before paying the guards: a share
     * of the coins each caravan carries, and a handling fee a caravan, in spurs (what the pass costs whatever's carried:
     * so a caravan with little aboard would sooner hand its goods over there and turn back).
     */
    public record Plan(double rate, Set<Hex> posts, double income, double fee) {
        public static final Plan NONE = new Plan(0, Set.of(), 0, 0);
    }

    /**
     * The fee a caravan on this route would bear at these posts, beyond the share of its coins: what's left of the
     * share of the detour, at most {@code feeCap} (spurs).
     */
    static double bearableFee(Crossing crossing, Option option, double rate, double share, double feeCap) {
        double detourShare = Double.isInfinite(option.detour()) ? Double.POSITIVE_INFINITY : share * option.detour();
        return Math.max(0, Math.min(feeCap, detourShare - rate * crossing.value));
    }

    private Chokepoints() {}

    /**
     * Tries each toll some route would bear. At each, takes the routes richest first, posting the guards each needs
     * where its toll beats the cost of the new guards. Keeps the toll that leaves the most after the guards' pay; no
     * posts if none pays.
     */
    public static Plan bestPlan(Collection<Crossing> crossings, double guardCostPerDay) {
        return bestPlan(crossings, guardCostPerDay, DETOUR_SHARE, Territory.MAX_TOLL_RATE, 0);
    }

    /**
     * As {@link #bestPlan(Collection, double)}, asking up to {@code share} of each detour and at most {@code cap}: more,
     * with guards armed and armoured (see {@link Smithing}).
     */
    public static Plan bestPlan(Collection<Crossing> crossings, double guardCostPerDay, double share, double cap,
            double feeCap) {
        TreeSet<Double> rates = new TreeSet<>();
        crossings.forEach(c -> c.options.forEach(o -> rates.add(c.bearableRate(o, share, cap))));
        List<Crossing> richestFirst = crossings.stream()
                .sorted(Comparator.comparingDouble((Crossing c) -> c.value * c.tripsPerDay).reversed())
                .toList();
        Plan best = Plan.NONE;
        double bestProfit = 0;
        for (double rate : rates) {
            if (rate < MIN_RATE) {
                continue;
            }
            Set<Hex> posts = new LinkedHashSet<>();
            double income = 0;
            double fee = Double.POSITIVE_INFINITY;
            double trips = 0;
            for (Crossing crossing : richestFirst) {
                Option option = crossing.fewestPostsFor(rate, share, cap);
                if (option == null) {
                    continue;
                }
                long added = option.posts().stream().filter(hex -> !posts.contains(hex)).count();
                double bearable = bearableFee(crossing, option, rate, share, feeCap);
                double gain = crossing.incomePerDay(rate) + bearable * crossing.tripsPerDay;
                if (gain > added * guardCostPerDay) {
                    posts.addAll(option.posts());
                    income += crossing.incomePerDay(rate);
                    fee = Math.min(fee, bearable); // what every caravan through will still bear
                    trips += crossing.tripsPerDay;
                }
            }
            fee = Double.isInfinite(fee) ? 0 : fee;
            income += fee * trips;
            double profit = income - posts.size() * guardCostPerDay;
            if (profit > bestProfit + 1e-9) {
                best = new Plan(rate, Set.copyOf(posts), income, fee);
                bestProfit = profit;
            }
        }
        return best;
    }
}
