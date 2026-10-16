package dev.b28.enterprise.caravan;

import dev.b28.enterprise.Dials;
import dev.b28.enterprise.contract.Advance;
import dev.b28.enterprise.contract.Contract;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.contract.Delivery;
import dev.b28.enterprise.contract.Harvest;
import dev.b28.enterprise.contract.Incoterm;
import dev.b28.enterprise.contract.Rents;
import dev.b28.enterprise.cargo.Crates;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.territory.Territory;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.trading.ItemCost;

/**
 * Forward sales of crops, as abbeys sold their wool and farms their standing grain: a market whose lockers buy a crop
 * its own fields don't grow enough of offers the farm town whose caravan has come so much a round trip, ten times, at
 * today's price, a third paid up front (see {@link Advance}), each delivery within a harvest window with shortfalls
 * rolled over (see {@link Harvest}). The seller's Farmers sow for it before anything else. Drafted here, it's in force
 * once the caravan has brought its copy home.
 */
public final class ForwardSales {
    // Its settings are dials (see Dials): the harvest window, the advance, the sowing lead, the largest lot.

    private ForwardSales() {}

    /**
     * At the buyer's market, a farm town's caravan having come. It first buys back rents its town owes here, with the
     * capital it brought (see {@link Caravans}' purse); then the town here buys what it lacks of the crops a farm town
     * grows: a rent, where the farm is short of money and the town here has savings to put by (it pays the capital into
     * the caravan's purse, to take home); else forward contracts.
     */
    static void offer(MinecraftServer server, net.minecraft.server.level.ServerLevel level, Route route,
            dev.b28.enterprise.merchant.MerchantStock stock, Caravan caravan, dev.b28.enterprise.money.Mint mint,
            double rate, long now) {
        Rents rents = Rents.get(server);
        int keep = (int) Math.ceil(Caravans.foodNeeded(route.distance()) * Math.max(1, caravan.crew.size())
                * Caravans.PURSE_PER_FOOD_POINT * rate);
        for (Rents.Rent rent : rents.owed(route.from(), route.to())) {
            int spurs = (int) Math.ceil(rent.capital() * rate);
            if (caravan.spurs - keep >= spurs) {
                caravan.spurs -= spurs;
                dev.b28.enterprise.money.Treasury.deposit(level, route.to().pos(), stock, spurs);
                rents.redeem(rent, ContractBook.get(server), now);
            }
        }
        ContractBook book = ContractBook.get(server);
        if (book.trust(route.from(), route.to(), now) < ContractBook.MIN_TRUST) {
            return;
        }
        Territory territory = Territory.get(server);
        Optional<GlobalPos> farm = territory.bellOf(route.from());
        Optional<GlobalPos> market = territory.bellOf(route.to());
        if (farm.isEmpty() || market.isEmpty() || territory.farmPlan(farm.get()).isEmpty()) {
            return; // only a town with farms sells its crop forward
        }
        Map<String, Double> demand = territory.demand(market.get());
        Map<String, Double> grown = territory.farmPlan(market.get());
        long every = Math.max(Caravans.DEPARTURE_INTERVAL, 2 * Caravans.travelTicks(route.distance()));
        for (Item crop : Territory.ROTATION) {
            String id = BuiltInRegistries.ITEM.getKey(crop).toString();
            // What it lacks of what its lockers use (at least a lot a round trip, to start a trade it has no crop for),
            // not all its villagers could buy.
            double used = Math.max(dev.b28.enterprise.merchant.Warehouse.get(server).perDay(route.to(), crop, now),
                    Caravans.CONTRACT_MIN_ITEMS * 24000.0 / every);
            double lacking = Math.min(demand.getOrDefault(id, 0.0), used) - grown.getOrDefault(id, 0.0)
                    - boughtForward(book, route.to(), crop);
            // No more than the farm could grow beyond what it has promised already.
            double promised = promised(book, route.from());
            lacking = Math.min(lacking, territory.couldGrow(farm.get(), crop) - promised);
            if (lacking <= 0 || book.covers(route.from(), route.to(), crop)) {
                continue;
            }
            // A rent, where the farm needs money and the town here has it to put by.
            int season = (int) Math.min(2 * Dials.FORWARD_MAX.get(), Math.floor(lacking * Rents.seasonTicks() / 24000.0 / 16) * 16);
            double capital = Rents.capital(season, Prices.emeraldsEach(crop));
            double farmSpare = territory.treasury(farm.get()) - territory.reserve(route.from());
            double savings = dev.b28.enterprise.money.Treasury.available(level, route.to().pos(), stock, rate)
                    - territory.reserve(route.to());
            if (season >= Caravans.CONTRACT_MIN_ITEMS && farmSpare < capital && savings >= 2 * capital) {
                var paid = dev.b28.enterprise.money.Treasury.withdraw(level, route.to().pos(), stock, capital, mint, now);
                caravan.spurs += paid.spurs() + mint.sell(dev.b28.enterprise.money.Mint.DEFAULT_FACTION, paid.emeralds(), now);
                rents.sell(route.from(), route.to(), crop, season, capital, book, now);
                continue;
            }
            int quantity = (int) Math.min(Dials.FORWARD_MAX.get(), Math.floor(lacking * every / 24000.0 / 16) * 16);
            if (quantity < Caravans.CONTRACT_MIN_ITEMS) {
                continue;
            }
            Contract contract = book.draft(route.from(), route.to(), List.of(
                            new Delivery(crop, quantity, Prices.emeraldsEach(crop)), new Harvest(Dials.HARVEST_WINDOW.getInt()),
                            new Advance(Dials.ADVANCE_SHARE.get())),
                    Incoterm.DDP, now + every + Dials.SOW_DAYS.ticks(), every, Dials.FORWARD_INSTALLMENTS.getInt(), now,
                    route.to(), false);
            caravan.drafts.add(contract.id());
        }
    }

    /**
     * What the farm keeps back of a crop for its forward sales and rents' coupons: what falls due by the end of the
     * next harvest window, not yet delivered or on the road. Its caravans sell, and its workshop uses, only the rest.
     */
    public static int setAside(MinecraftServer server, GlobalPos farm, Item crop, MerchantStock stock, long now) {
        int kept = 0;
        for (Contract c : ContractBook.get(server).of(farm)) {
            if (!c.seller().equals(farm) || c.harvest().isEmpty() || !c.inForce()) {
                continue;
            }
            if (c.delivery().filter(d -> d.item() == crop).isPresent()) {
                // Less what's crated for it already: out of the stock's reach.
                kept += Math.max(0, c.owed(now + c.harvest().get().windowTicks()) - c.shipped()
                        - earmarked(stock, c.id(), crop));
            }
        }
        return kept;
    }

    /** Is this one of the stock's crates earmarked under the contract (its bill of lading ships under it)? */
    static boolean earmarkedFor(ItemStack stack, int contract) {
        return Crates.isCrate(stack) && Crates.papers(stack).contract().filter(id -> id == contract).isPresent();
    }

    static int goodsIn(ItemStack crate) {
        return Crates.goods(crate).stream().mapToInt(ItemStack::getCount).sum();
    }

    /** Of a crop, what's crated in the stock under the contract. */
    public static int earmarked(MerchantStock stock, int contract, Item crop) {
        return stock.looseStacks().stream().filter(s -> earmarkedFor(s, contract))
                .flatMap(s -> Crates.goods(s).stream()).filter(g -> g.is(crop)).mapToInt(ItemStack::getCount).sum();
    }

    /** Takes the crates earmarked under the contract out of the stock, as many as cover {@code most} items. */
    public static List<ItemStack> takeEarmarked(MerchantStock stock, int contract, int most) {
        int[] left = {most};
        return stock.removeLoose(s -> {
            if (left[0] <= 0 || !earmarkedFor(s, contract)) {
                return false;
            }
            left[0] -= goodsIn(s);
            return true;
        });
    }

    /**
     * The farm earmarks its harvest for what it owes: as soon as the crop is in, what falls due under its forward
     * sales and rents' coupons by the end of the next harvest window is crated, a bill of lading made out to the buyer
     * and a copy of the contract in each crate. Crated, it's a shipment, out of reach of the lockers, the bakers and
     * the caravans' spot sales; the next caravan to the buyer, or the buyer's own collecting, takes it first. A crate
     * whose contract has closed, or gone to another buyer, goes back on the shelf.
     */
    public static void earmark(net.minecraft.server.level.ServerLevel level, GlobalPos desk, MerchantStock stock, long now) {
        MinecraftServer server = level.getServer();
        ContractBook book = ContractBook.get(server);
        List<ItemStack> stale = stock.removeLoose(s -> Crates.isCrate(s) && Crates.papers(s).contract()
                .flatMap(book::contract)
                .filter(c -> c.seller().equals(desk) && c.harvest().isPresent()
                        && (!c.inForce() || !Crates.papers(s).consignee().equals(Optional.of(c.buyer()))))
                .isPresent());
        // And any crate it shipped under a contract no longer in force (broken, say): unless it was paid for by a bill
        // still to be collected, the goods are its own again. (A crate paid for is its buyer's: see market.Stranded.)
        var bills = dev.b28.enterprise.contract.Bills.get(server);
        stale.addAll(stock.removeLoose(s -> Crates.isCrate(s) && Crates.papers(s).shipper().filter(desk::equals).isPresent()
                && Crates.papers(s).consignee().filter(c -> !c.equals(desk)).isPresent()
                && Crates.papers(s).contract().flatMap(book::contract).filter(c -> !c.inForce()).isPresent()
                && !(Crates.papers(s).settled() && bills.all().stream().anyMatch(b -> b.state()
                        == dev.b28.enterprise.contract.Bills.State.OUTSTANDING && b.payee().equals(desk)
                        && Crates.papers(s).consignee().filter(b.drawee()::equals).isPresent()))));
        for (ItemStack crate : stale) {
            Crates.goods(crate).forEach(stock::insert);
            stock.insert(new ItemStack(dev.b28.enterprise.cargo.Cargo.CRATE_ITEM.get()));
        }
        bills.tidy(stock, desk); // and paper no longer its to collect, thrown out
        for (Contract c : book.of(desk)) {
            if (!c.seller().equals(desk) || c.harvest().isEmpty() || !c.inForce() || c.delivery().isEmpty()) {
                continue;
            }
            Item crop = c.delivery().get().item();
            int want = c.owed(now + c.harvest().get().windowTicks()) - c.shipped() - earmarked(stock, c.id(), crop);
            if (want <= 0) {
                continue;
            }
            List<ItemStack> goods = new java.util.ArrayList<>(stock.take(new ItemCost(crop, 1), want));
            if (goods.isEmpty()) {
                continue;
            }
            for (ItemStack packed : Crates.pack(level, desk, stock, goods,
                    Crates.shipping(c, desk, c.buyer(), goods, -1, now, false), now)) {
                ItemStack rest = stock.insert(packed);
                if (!rest.isEmpty()) {
                    (Crates.isCrate(rest) ? Crates.goods(rest) : List.of(rest)).forEach(stock::insert); // no room
                }
            }
        }
    }

    /** Items a day of every crop the farm has sold forward, drafted or in force (contracts and rents' coupons). */
    static double promised(ContractBook book, GlobalPos farm) {
        return book.of(farm).stream()
                .filter(c -> c.seller().equals(farm) && c.harvest().isPresent()
                        && (c.state() == Contract.State.ACTIVE || c.state() == Contract.State.DRAFTED))
                .flatMap(c -> c.delivery().stream().map(d -> d.quantity() * 24000.0 / c.every()))
                .mapToDouble(Double::doubleValue).sum();
    }

    /** Items a day of a crop the buyer has bought forward (contracts and rents' coupons), from every farm. */
    static double boughtForward(ContractBook book, GlobalPos buyer, Item crop) {
        return book.of(buyer).stream()
                .filter(c -> c.buyer().equals(buyer) && c.harvest().isPresent()
                        && (c.state() == Contract.State.ACTIVE || c.state() == Contract.State.DRAFTED))
                .flatMap(c -> c.delivery().filter(d -> d.item() == crop).stream()
                        .map(d -> d.quantity() * 24000.0 / c.every()))
                .mapToDouble(Double::doubleValue).sum();
    }

    /**
     * What the town has sold forward, in force: item id -> {items a day, emeralds each}. Its Farmers sow for these
     * first, after the herds' feed.
     */
    public static Map<String, double[]> sold(MinecraftServer server, GlobalPos seller) {
        Map<String, double[]> sold = new HashMap<>();
        for (Contract c : ContractBook.get(server).of(seller)) {
            if (!c.seller().equals(seller) || c.harvest().isEmpty() || !c.inForce()) {
                continue;
            }
            c.delivery().ifPresent(d -> sold.merge(BuiltInRegistries.ITEM.getKey(d.item()).toString(),
                    new double[] {d.quantity() * 24000.0 / c.every(), Math.max(d.unitPrice(), Prices.emeraldsEach(d.item()))},
                    (a, b) -> new double[] {a[0] + b[0], Math.max(a[1], b[1])}));
        }
        return sold;
    }
}
