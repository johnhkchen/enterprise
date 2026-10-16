package dev.b28.enterprise.market;

import dev.b28.enterprise.Dials;
import dev.b28.enterprise.contract.Bills;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.territory.Buildings;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.MinecraftServer;

/**
 * A hub's discount desk: the bills it buys, at what price, and how many. The hub earns from the spread and the
 * throughput, so it wants steady, well-secured paper, not the most of it. What it pays is the bill's face less:
 * <ul>
 *   <li>its fee, and so much a day to the bill's due day (as a bank's);</li>
 *   <li>more for a bill whose goods it can't see yet (crop not yet shipped) than for one on goods delivered or in
 *   its hands, and more again for one due past its season ({@code discount_long_days} out);</li>
 *   <li>and, as a throttle, more as its warehouse fills and its book of paper nears its limit: the flow slows,
 *   no one is turned away.</li>
 * </ul>
 * Its book is capped: all the paper it holds at most {@code discount_book_share} of its means, and no more than
 * {@code discount_drawee_share} of that on one drawee, nor {@code discount_foreign_share} on markets of foreign
 * realms. It buys on its discount days (every {@code discount_days}), best-secured paper first; with a counting
 * house its clerks keep the book every day, at half the fee, and it carries twice the paper.
 */
public final class Discounts {
    private Discounts() {}

    /** A price for a bill: the share of its face paid, or why not. */
    public record Quote(double share, Optional<String> refused) {
        static Quote no(String why) {
            return new Quote(0, Optional.of(why));
        }

        public boolean taken() {
            return refused.isEmpty();
        }
    }

    public static boolean counting(MinecraftServer server, GlobalPos hub) {
        return Buildings.has(server, hub, Buildings.COUNTING_HOUSE);
    }

    /** Is the desk open today? Every day with a counting house; else on its discount days. */
    public static boolean open(MinecraftServer server, GlobalPos hub, long now) {
        int every = Math.max(1, Dials.DISCOUNT_DAYS.getInt());
        return counting(server, hub) || (now / 24000) % every == 0;
    }

    /** Emeralds of face the hub holds in other markets' bills, outstanding: its book. */
    public static double book(MinecraftServer server, GlobalPos hub) {
        return Bills.get(server).all().stream().filter(b -> b.state() == Bills.State.OUTSTANDING
                && b.payee().equals(hub) && !b.drawee().equals(hub)).mapToDouble(Bills.Bill::emeralds).sum();
    }

    /** As {@link #book}, of bills on {@code drawee}. */
    static double onDrawee(MinecraftServer server, GlobalPos hub, GlobalPos drawee) {
        return Bills.get(server).all().stream().filter(b -> b.state() == Bills.State.OUTSTANDING
                && b.payee().equals(hub) && b.drawee().equals(drawee)).mapToDouble(Bills.Bill::emeralds).sum();
    }

    /** As {@link #book}, of bills on foreign markets. */
    static double foreign(MinecraftServer server, GlobalPos hub) {
        return Bills.get(server).all().stream().filter(b -> b.state() == Bills.State.OUTSTANDING
                && b.payee().equals(hub) && isForeign(server, b.drawee())).mapToDouble(Bills.Bill::emeralds).sum();
    }

    static boolean isForeign(MinecraftServer server, GlobalPos market) {
        return dev.b28.enterprise.trade.Realms.get(server).isForeign(market)
                || dev.b28.enterprise.merchant.OffMap.get(server).is(market);
    }

    /** The most paper the hub will hold, in emeralds of face: so much of its means, twice with a counting house. */
    public static double limit(MinecraftServer server, GlobalPos hub, double means) {
        return Math.max(0, means) * Dials.DISCOUNT_BOOK_SHARE.get() * (counting(server, hub) ? 2 : 1);
    }

    /** Is a bill backed by goods the hub can see: delivered, or crated and on their way, under its contract? */
    static boolean backed(MinecraftServer server, Bills.Bill bill) {
        if (bill.contract() < 0) {
            return false;
        }
        return ContractBook.get(server).contract(bill.contract())
                .map(c -> c.delivered() + c.shipped() > 0 || c.state() == dev.b28.enterprise.contract.Contract.State.COMPLETED)
                .orElse(false);
    }

    /**
     * What the hub would pay for {@code bill} now, as a share of its face; {@code backed} if its goods are known to be
     * in hand (crates bought with it, say), else read from its contract. {@code taking} is face already taken on in this
     * sitting (not yet in the ledger), {@code fill} the warehouse's fill (0 empty, 1 full), {@code means} the hub's
     * spare money. Refused past its limits.
     */
    public static Quote quote(MinecraftServer server, GlobalPos hub, Bills.Bill bill, boolean backed, double taking,
            double fill, double means, long now) {
        double limit = limit(server, hub, means);
        double held = book(server, hub) + taking;
        if (held + bill.emeralds() > limit) {
            return Quote.no("its book is full: " + Math.round(held) + " of " + Math.round(limit) + " emeralds");
        }
        if (onDrawee(server, hub, bill.drawee()) + bill.emeralds() > Dials.DISCOUNT_DRAWEE_SHARE.get() * limit) {
            return Quote.no("it holds its fill of that market's paper");
        }
        if (isForeign(server, bill.drawee())
                && foreign(server, hub) + bill.emeralds() > Dials.DISCOUNT_FOREIGN_SHARE.get() * limit) {
            return Quote.no("it holds its fill of foreign paper");
        }
        boolean counting = counting(server, hub);
        double days = Math.max(0, (bill.due() - now) / 24000.0);
        double off = Freight.DISCOUNT_FEE * (counting ? 0.5 : 1)
                + (bill.due() < now ? Dials.STALE_DISCOUNT.get() : Freight.DISCOUNT_PER_DAY * days);
        if (!(backed || backed(server, bill))) {
            off += Dials.DISCOUNT_UNBACKED.get(); // crop not yet in anyone's hands
        }
        if (days > Dials.DISCOUNT_LONG_DAYS.get()) {
            off += Dials.DISCOUNT_LONG.get(); // due past the season
        }
        // The throttle: dearer as its warehouse fills and its book nears the limit.
        off += Dials.DISCOUNT_THROTTLE.get() * (Math.max(0, Math.min(1, fill)) + Math.min(1, held / Math.max(1, limit)));
        return new Quote(Math.max(0, 1 - off), Optional.empty());
    }

    /** The warehouse's fill: the share of its loose slots taken. */
    public static double fill(MerchantStock stock) {
        return stock.slots() <= 0 ? 1 : 1 - stock.freeSlots() / (double) stock.slots();
    }
}
