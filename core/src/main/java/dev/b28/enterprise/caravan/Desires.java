package dev.b28.enterprise.caravan;

import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.territory.Comforts;
import dev.b28.enterprise.territory.Territory;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.item.Item;

/**
 * What towns want, in money: the value a Merchant puts on bringing a good to a market. A town short of a comfort pays
 * more for it the less of that kind its villagers have had (up to {@link #MOST} times its worth when none), as far as
 * its households and its spare money can pay for what it's short of; a town holding its fill takes more only cheap.
 * A Merchant who supplies other towns (the capital, a freight hub) buys for them too: a good it can carry on and sell
 * for more than it pays, after the road (its share of the road food a stack of cargo costs), by {@link #MARGIN} or
 * better. What it knows of other towns is what its caravans last saw there.
 */
public final class Desires {
    /** A kind none of the town's villagers has had: they pay up to this many times its worth. */
    public static final double MOST = 2.0;
    /** What a town with its fill pays for more: this share of its worth. */
    public static final double GLUT = 0.5;
    /** The margin a Merchant wants over the landed cost before it buys to sell on. */
    public static final double MARGIN = 0.15;

    private Desires() {}

    /** What the market at {@code desk} would pay for one of {@code item} on arrival at {@code at}, in emeralds. */
    public static double willPay(MinecraftServer server, GlobalPos desk, Item item, long at) {
        double value = Prices.emeraldsEach(item);
        Territory territory = Territory.get(server);
        var kind = Comforts.kindOf(item);
        Optional<GlobalPos> bell = territory.bellOf(desk);
        if (kind.isEmpty() || bell.isEmpty()) {
            return value;
        }
        String name = Comforts.name(kind.get());
        double shortOf = Caravans.comfortWants(server, territory, desk, at).getOrDefault(name, 0.0);
        if (shortOf <= 0) {
            return value * GLUT;
        }
        double met = territory.comfortsMet(bell.get()).getOrDefault(name, 0.0);
        double eager = 1 + (MOST - 1) * (1 - Math.max(0, Math.min(1, met)));
        double money = territory.households(bell.get())[0]
                + Math.max(0, territory.treasury(bell.get()) - territory.reserve(desk));
        double afford = Math.min(1, money / shortOf);
        return value * Math.max(GLUT, eager * afford);
    }

    /** Units of a comfort the market is short of on arrival: its shortfall in that kind, at the good's worth. */
    public static int shortOf(MinecraftServer server, GlobalPos desk, Item item, long at) {
        var kind = Comforts.kindOf(item);
        if (kind.isEmpty()) {
            return 0;
        }
        double worth = Caravans.comfortWants(server, Territory.get(server), desk, at)
                .getOrDefault(Comforts.name(kind.get()), 0.0);
        return (int) Math.ceil(worth / Prices.emeraldsEach(item));
    }

    /** The road's cost of carrying one of {@code item} on a route, both ways: road food spread over the cargo space. */
    public static double haulEach(Route route, Item item) {
        double food = 2.0 * Caravans.foodNeeded(route.distance()) * Math.max(1, route.lastCrew());
        double perStack = food * Caravans.PURSE_PER_FOOD_POINT / Math.max(1, route.lastCapacity());
        return perStack / item.getDefaultMaxStackSize();
    }

    /** Where a Merchant at {@code home} could sell a good on, and how much: the best of its routes' far ends. */
    public record Outlet(GlobalPos market, double each, int units) {}

    /**
     * The best outlet for a comfort among the markets {@code home}'s caravans supply (other than {@code except}): what
     * it would pay, less the road; and how many it's short of in all, across them.
     */
    public static Optional<Outlet> outlet(MinecraftServer server, GlobalPos home, GlobalPos except, Item item, long at) {
        if (Comforts.kindOf(item).isEmpty()) {
            return Optional.empty();
        }
        GlobalPos best = null;
        double bestEach = 0;
        int units = 0;
        for (Route route : CaravanLedger.get(server).routes()) {
            if (!route.from().equals(home) || route.to().equals(except) || route.stopping) {
                continue;
            }
            int short_ = shortOf(server, route.to(), item, at);
            if (short_ <= 0) {
                continue;
            }
            units += short_;
            double each = willPay(server, route.to(), item, at) - haulEach(route, item);
            if (each > bestEach) {
                bestEach = each;
                best = route.to();
            }
        }
        return best == null ? Optional.empty() : Optional.of(new Outlet(best, bestEach, units));
    }

    /** Is a price worth paying to sell on there: the outlet's price, less the margin, covers it? */
    public static boolean worthBuying(double price, Outlet outlet) {
        return price <= outlet.each() * (1 - MARGIN);
    }
}
