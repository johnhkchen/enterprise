package dev.b28.enterprise.money;

import dev.b28.enterprise.Dials;
import dev.b28.enterprise.merchant.MerchantStock;
import java.util.HashMap;
import java.util.Map;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.level.ServerLevel;

/**
 * A town's accountant: what it may spend at its own choosing today. Its money beyond the reserve (the wage float and
 * its shelves' stock), less the bills on it falling due in the next {@code budget_days}, is its free money; it spends
 * at most {@code budget_share} of that a day, all its spenders drawing on the one budget (caravans' shopping money,
 * fields, stalls, shop boards, pastures, halls, settlers), so no two of them spend the same spare money. What it owes
 * (wages, bills, goods under contract) isn't the accountant's to hold back. With a counting house its clerks keep
 * better books: they look twice as far ahead for bills, and let it spend more of what's free.
 *
 * Measured before it: towns went broke in their first ten days as caravans, a shop board and new fields each spent
 * the same money beyond the reserve (Cloverby: 52 emeralds of purses out, 12 for a board, 12 for fields; 2 earned).
 */
public final class Accountant {
    private Accountant() {}

    /** Today's budget by market: the day, the budget set at its first look, and what's been spent of it. */
    private static final Map<GlobalPos, double[]> SPENT = new HashMap<>();

    /** The town's free money now: beyond its reserve, less the bills on it due soon. */
    public static double free(ServerLevel level, GlobalPos desk, MerchantStock stock, long now) {
        var server = level.getServer();
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        boolean counting = dev.b28.enterprise.territory.Buildings.has(server, desk,
                dev.b28.enterprise.territory.Buildings.COUNTING_HOUSE);
        long ahead = Math.round(Dials.BUDGET_DAYS.get() * (counting ? 2 : 1) * 24000);
        return Treasury.available(level, desk.pos(), stock, rate)
                - dev.b28.enterprise.territory.Territory.get(server).reserve(desk)
                - dev.b28.enterprise.contract.Bills.get(server).dueBy(desk, now + ahead);
    }

    /** What's left of today's budget: set at the day's first look (so much of the free money then), less what's spent. */
    public static double left(ServerLevel level, GlobalPos desk, MerchantStock stock, long now) {
        double[] day = today(level, desk, stock, now);
        return Math.max(0, day[1] - day[2]);
    }

    /**
     * What's left of today's budget for a seller who last came {@code days} days ago: what the daily looks it missed
     * would have allowed, one after another ({@code 1 - (1 - share)^days} of the free money), less what's spent today.
     * A wagon every few days sells what daily ones would have.
     */
    public static double leftFor(ServerLevel level, GlobalPos desk, MerchantStock stock, double days, long now) {
        double[] day = today(level, desk, stock, now);
        double share = day[3];
        if (days <= 1 || share <= 0 || share >= 1) {
            return Math.max(0, day[1] - day[2]);
        }
        return Math.max(0, day[1] / share * (1 - Math.pow(1 - share, days)) - day[2]);
    }

    /** {day, budget, spent, share} for the market today. */
    private static double[] today(ServerLevel level, GlobalPos desk, MerchantStock stock, long now) {
        double[] day = SPENT.get(desk);
        if (day == null || day[0] != now / 24000) {
            boolean counting = dev.b28.enterprise.territory.Buildings.has(level.getServer(), desk,
                    dev.b28.enterprise.territory.Buildings.COUNTING_HOUSE);
            double share = Math.min(1, Dials.BUDGET_SHARE.get() * (counting ? 1.5 : 1));
            day = new double[] {now / 24000, Math.max(0, share * free(level, desk, stock, now)), 0, share};
            SPENT.put(desk, day);
        }
        return day;
    }

    /**
     * May the town spend {@code emeralds} on {@code what} now? If so it's booked against today's budget; if not, the
     * cash book notes what was held back.
     */
    public static boolean approve(ServerLevel level, GlobalPos desk, MerchantStock stock, String what, double emeralds,
            long now) {
        if (emeralds <= 0) {
            return true;
        }
        if (emeralds > left(level, desk, stock, now) + 1e-9) {
            CashBook.held(desk, what, emeralds);
            return false;
        }
        book(desk, emeralds, now);
        return true;
    }

    /** As much of {@code emeralds} for {@code what} as today's budget allows, booked; the rest noted as held back. */
    public static double allow(ServerLevel level, GlobalPos desk, MerchantStock stock, String what, double emeralds,
            long now) {
        if (emeralds <= 0) {
            return 0;
        }
        double allowed = Math.min(emeralds, left(level, desk, stock, now));
        if (allowed < emeralds - 1e-9) {
            CashBook.held(desk, what, emeralds - allowed);
        }
        book(desk, allowed, now);
        return allowed;
    }

    /** {@code emeralds} spent of today's budget, already allowed for (by {@link #left}): booked against it. */
    public static void spent(GlobalPos desk, double emeralds, long now) {
        book(desk, emeralds, now);
    }

    private static void book(GlobalPos desk, double emeralds, long now) {
        double[] day = SPENT.get(desk);
        if (day != null && day[0] == now / 24000) {
            day[2] += emeralds;
        }
    }

    /** For tests: a fresh day's budget. */
    public static void forgetForTest(GlobalPos desk) {
        SPENT.remove(desk);
    }
}
