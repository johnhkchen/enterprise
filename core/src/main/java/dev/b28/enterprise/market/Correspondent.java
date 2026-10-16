package dev.b28.enterprise.market;

import dev.b28.enterprise.Dials;
import dev.b28.enterprise.caravan.Caravan;
import dev.b28.enterprise.caravan.Route;
import dev.b28.enterprise.cargo.Cargo;
import dev.b28.enterprise.cargo.Crates;
import dev.b28.enterprise.cargo.Papers;
import dev.b28.enterprise.contract.Bills;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.territory.Territory;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.ItemStack;

/**
 * A hub's correspondent at the capital: a hub with a counting house keeps an account there, and when it runs short of
 * money it doesn't sell its grain cheap or stop buying paper: its caravan to the capital carries its best bills (the
 * soonest due), and the capital rediscounts them, as a bank would, for coin that rides home. Nothing crosses distance
 * without a carrier: the bills go and the coin comes back on the hub's own run.
 */
public final class Correspondent {
    private Correspondent() {}

    /** The capital, if the run from {@code hub} goes there and the hub keeps an account (a counting house). */
    static Optional<GlobalPos> capitalFor(MinecraftServer server, GlobalPos hub, GlobalPos to) {
        return Sovereign.get(server).desk().filter(to::equals).filter(c -> !c.equals(hub))
                .filter(c -> Discounts.counting(server, hub));
    }

    /**
     * As the hub's caravan loads for the capital: the bills it sends to rediscount, out of its files, the soonest due
     * first, as many as cover what it's short of its {@code correspondent_floor}. Returns the bundles to carry.
     */
    public static List<ItemStack> send(ServerLevel level, Route route, MerchantStock stock, long now) {
        MinecraftServer server = level.getServer();
        GlobalPos hub = route.from();
        if (capitalFor(server, hub, route.to()).isEmpty()) {
            return List.of();
        }
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        double spare = Treasury.available(level, hub.pos(), stock, rate) - Territory.get(server).reserve(hub);
        double shortBy = Dials.CORRESPONDENT_FLOOR.get() - spare;
        if (shortBy <= 0) {
            return List.of();
        }
        Bills ledger = Bills.get(server);
        List<ItemStack> bundles = stock.removeLoose(s -> s.is(Cargo.DOCUMENTS.get()) && !Bills.ids(s).isEmpty());
        List<Papers.Document> all = new ArrayList<>();
        for (ItemStack bundle : bundles) {
            all.addAll(bundle.getOrDefault(Cargo.PAPERS.get(), Papers.NONE).documents());
        }
        all.sort(Comparator.comparingLong((Papers.Document d) -> ledger.bill(d.data().getInt("bill"))
                .map(Bills.Bill::due).orElse(Long.MAX_VALUE)));
        List<Papers.Document> going = new ArrayList<>();
        List<Papers.Document> kept = new ArrayList<>();
        double raised = 0;
        for (Papers.Document doc : all) {
            var bill = ledger.bill(doc.data().getInt("bill")).orElse(null);
            boolean sendable = bill != null && bill.state() == Bills.State.OUTSTANDING && bill.payee().equals(hub)
                    && !bill.drawee().equals(hub) && bill.due() > now;
            if (sendable && raised < shortBy) {
                going.add(doc);
                raised += bill.emeralds() * price(bill, now);
            } else {
                kept.add(doc);
            }
        }
        if (!kept.isEmpty()) {
            stock.insert(Crates.documents(new Papers(kept)));
        }
        return going.isEmpty() ? List.of() : List.of(Crates.documents(new Papers(going)));
    }

    /** The capital's rediscount: a bank's fee, and so much a day to the bill's due day. */
    static double price(Bills.Bill bill, long now) {
        double days = Math.max(0, (bill.due() - now) / 24000.0);
        return Math.max(0, 1 - Bank.DISCOUNT_FEE - Bank.DISCOUNT_PER_DAY * days);
    }

    /**
     * At the capital: it rediscounts the hub's bills the caravan brought, from its treasury beyond its reserve, the coin
     * into the caravan's purse; a bill on the capital itself is redeemed, any other endorsed to it and filed. What it
     * can't pay for rides home again. Returns spurs paid.
     */
    public static int rediscount(ServerLevel level, Route route, MerchantStock stock, Caravan caravan, long now) {
        MinecraftServer server = level.getServer();
        GlobalPos hub = route.from();
        GlobalPos capital = route.to();
        if (capitalFor(server, hub, capital).isEmpty()) {
            return 0;
        }
        Bills ledger = Bills.get(server);
        Mint mint = Mint.get(server);
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        double spare = Treasury.available(level, capital.pos(), stock, rate) - Territory.get(server).reserve(capital);
        int paid = 0;
        List<Papers.Document> bought = new ArrayList<>();
        for (var it = caravan.cargo().listIterator(); it.hasNext(); ) {
            ItemStack bundle = it.next();
            if (!bundle.is(Cargo.DOCUMENTS.get()) || Bills.ids(bundle).isEmpty()) {
                continue;
            }
            List<Papers.Document> kept = new ArrayList<>();
            for (Papers.Document doc : bundle.getOrDefault(Cargo.PAPERS.get(), Papers.NONE).documents()) {
                var bill = ledger.bill(doc.data().getInt("bill")).orElse(null);
                double cost = bill == null ? 0 : bill.emeralds() * price(bill, now);
                if (bill == null || bill.state() != Bills.State.OUTSTANDING || !bill.payee().equals(hub)
                        || bill.due() <= now || cost <= 0 || spare < cost) {
                    kept.add(doc);
                    continue;
                }
                spare -= cost;
                Treasury.Withdrawal taken = Treasury.withdraw(level, capital.pos(), stock, cost, mint, now);
                int spurs = taken.spurs() + mint.sell(Mint.DEFAULT_FACTION, taken.emeralds(), now);
                caravan.receive(spurs);
                paid += spurs;
                if (bill.drawee().equals(capital)) {
                    ledger.redeem(bill); // its own paper: settled now
                } else {
                    ledger.endorse(bill, capital);
                    bought.add(doc);
                }
            }
            it.remove();
            if (!kept.isEmpty()) {
                it.add(Crates.documents(new Papers(kept)));
            }
        }
        if (!bought.isEmpty()) {
            stock.insert(Crates.documents(new Papers(bought))); // the capital presents them in its turn
        }
        if (paid > 0) {
            dev.b28.enterprise.Enterprise.LOGGER.info("{}'s correspondent at {} rediscounts its bills for {} spurs",
                    dev.b28.enterprise.settlement.Names.label(hub), dev.b28.enterprise.settlement.Names.label(capital),
                    paid);
        }
        return paid;
    }
}
