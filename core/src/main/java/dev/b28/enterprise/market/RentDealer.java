package dev.b28.enterprise.market;

import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.contract.Rents;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.territory.Territory;
import java.util.List;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;

/**
 * A freight hub's Merchant deals in rents. Every road crosses its town, so it meets the towns with savings and those
 * short of coin: it buys a rent from a holder who needs coin, or whose rent has risen, a tenth under its price, and
 * sells to a town with savings to put by a tenth over. A rent it holds it collects itself: its caravans call at the
 * farm for each season's coupon, so a remote farm with no caravan of its own still keeps its rent.
 */
public final class RentDealer {
    /** It buys this much under its price and sells this much over. */
    public static final double SPREAD = 0.1;
    /** A holder sells when the dealer bids this many times what it paid. */
    public static final double TAKE_PROFIT = 1.25;
    /** A rent changes hands at most once in this long, and never back to who sold it within it. */
    public static final long HOLD_TICKS = Rents.SEASON_TICKS;

    private RentDealer() {}

    /**
     * A rent's price at the hub now, in emeralds: its seasons' purchase at the crop's worth, times how short the
     * markets the hub's caravans reach are of the crop (twice, with none of it bought ahead; half, with all of it).
     */
    public static double price(MinecraftServer server, GlobalPos hub, Rents.Rent rent) {
        Territory territory = Territory.get(server);
        String id = BuiltInRegistries.ITEM.getKey(rent.crop()).toString();
        double wanted = territory.marketWants(server, hub).getOrDefault(id, List.of()).stream()
                .mapToDouble(m -> m[0]).sum();
        double ahead = Rents.get(server).all().stream().filter(r -> r.state() == Rents.State.ACTIVE && r.crop() == rent.crop())
                .mapToDouble(r -> r.quantity() * 24000.0 / Rents.SEASON_TICKS).sum();
        double share = wanted + ahead <= 0 ? 0.5 : wanted / (wanted + ahead);
        return Rents.capital(rent.quantity(), Prices.emeraldsEach(rent.crop())) * (0.5 + 1.5 * share);
    }

    /**
     * A caravan from {@code visitor} at the hub: the hub buys the rents its town would sell (short of coin, or for a
     * profit), paying into the caravan's purse; and sells it rents the hub holds, if its town has savings to put by,
     * paid from the purse or by a bill on its town.
     */
    public static void deal(MinecraftServer server, ServerLevel level, GlobalPos hub, GlobalPos visitor,
            MerchantStock stock, dev.b28.enterprise.caravan.Caravan caravan, Mint mint, double rate, long now) {
        if (!Freight.isHub(server, hub) || visitor.equals(hub)) {
            return;
        }
        RentDeeds.quote(level, hub, stock, mint, rate, now); // its shops: rents to players
        Rents rents = Rents.get(server);
        ContractBook book = ContractBook.get(server);
        Territory territory = Territory.get(server);
        CaravanLedger ledger = CaravanLedger.get(server);
        double visitorSpare = territory.bellOf(visitor).map(territory::treasury).orElse(0.0) - territory.reserve(visitor);
        for (Rents.Rent rent : rents.heldBy(visitor)) {
            if (ledger.route(hub, rent.farm()).isEmpty()) {
                continue; // it can't collect what it can't reach
            }
            double bid = price(server, hub, rent) * (1 - SPREAD);
            double spare = Treasury.available(level, hub.pos(), stock, rate) - territory.reserve(hub);
            if (now - rent.traded() < HOLD_TICKS) {
                continue;
            }
            if ((visitorSpare < 0 || bid >= TAKE_PROFIT * rent.paid()) && spare >= bid) {
                Treasury.Withdrawal paid = Treasury.withdraw(level, hub.pos(), stock, bid, mint, now);
                caravan.addSpurs(paid.spurs() + mint.sell(Mint.DEFAULT_FACTION, paid.emeralds(), now));
                // The farm delivers where its caravans go; the dealer's collect only where they don't.
                rents.assign(rent, hub, ledger.route(rent.farm(), hub).isEmpty(), bid, book, now);
                visitorSpare += bid;
            }
        }
        if (book.trust(hub, visitor, now) < ContractBook.MIN_TRUST) {
            return;
        }
        for (Rents.Rent rent : rents.heldBy(hub)) {
            if (rent.bearer()) {
                continue; // a player's, in its keeping
            }
            double ask = price(server, hub, rent) * (1 + SPREAD);
            if (rent.farm().equals(visitor) || visitorSpare < 2 * ask || now - rent.traded() < HOLD_TICKS
                    && rent.previous().filter(visitor::equals).isPresent()) {
                continue;
            }
            // Only to a town the coupons can reach: the farm's caravans deliver there, or its own collect at the farm.
            boolean delivered = ledger.route(rent.farm(), visitor).isPresent();
            boolean collected = ledger.route(visitor, rent.farm()).isPresent();
            if (!delivered && !collected) {
                continue;
            }
            int spurs = (int) Math.min(caravan.spurs(), Math.ceil(ask * rate));
            caravan.addSpurs(-spurs);
            Treasury.deposit(level, hub.pos(), stock, spurs);
            double rest = ask - spurs / rate;
            if (rest > 0.01) {
                dev.b28.enterprise.contract.Bills.file(stock, dev.b28.enterprise.contract.Bills.get(server)
                        .draw(visitor, hub, rest, now)); // the rest on its town's treasury
            }
            rents.assign(rent, visitor, !delivered, ask, book, now);
            visitorSpare -= ask;
        }
    }
}
