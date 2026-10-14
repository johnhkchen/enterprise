package dev.b28.enterprise.market;

import dev.b28.enterprise.caravan.Atlas;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.service.Services;
import dev.b28.enterprise.territory.Territory;
import dev.ithundxr.createnumismatics.content.coins.CoinItem;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.Holder;
import net.minecraft.core.component.DataComponentPredicate;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.EnchantmentTags;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.enchantment.Enchantment;
import net.minecraft.world.item.enchantment.EnchantmentHelper;
import net.minecraft.world.item.enchantment.ItemEnchantments;
import net.minecraft.world.item.trading.ItemCost;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * A master Merchant's brokering with the players' shops round its desk (Numismatics vendors), as agent and market
 * maker, every {@link #TICKS} while its market is loaded:
 * <ul>
 *   <li>A player's buying shop is a standing order: the Merchant fills it at the shop's price from what its warehouse
 *   can spare; what it can't fill stays an order its region's caravans bring the goods for.</li>
 *   <li>An order for enchanted goods is a commission: with a master Librarian and a Cleric in its town, the Merchant
 *   has the book enchanted, copied and combined to the order's levels and laid on, and hands the good over.</li>
 *   <li>A player's selling shop is supply: the Merchant buys what its town or its orders want, at up to the good's
 *   worth, more for goods it has posted as wanted.</li>
 *   <li>What its town wants that no market it knows has is posted as a signal, with the premium it will pay: work for
 *   players who can bring it from another region, or make it.</li>
 *   <li>A shop touching the desk is the town's own: the Merchant sells its surplus there, a little over its worth,
 *   and banks the takings.</li>
 * </ul>
 */
public final class Broker {
    public static final int MASTER = 5;
    public static final long TICKS = 1200;

    /** The Merchant fills orders priced at no less than this share of the goods' worth. */
    public static final double AGENT_FLOOR = 0.9;
    /** It buys from players' shops at up to the goods' worth, and half again for goods it has posted as wanted. */
    public static final double MAKER_CEILING = 1.0;
    public static final double SIGNAL_PREMIUM = 1.5;
    /** Its own shop sells surplus at this much over its worth, in lots of up to this many. */
    public static final double OWN_MARKUP = 1.1;
    public static final int OWN_LOT = 16;

    /** A commission's work costs an emerald (bottles o' enchanting) for each book combined and each enchantment laid on. */
    public static final double XP_EMERALDS = 1;
    /** Steps of commission work a Merchant gets through each time. */
    public static final int STEPS = 3;

    private Broker() {}

    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        if (server.overworld().getGameTime() % TICKS != 0) {
            return;
        }
        for (GlobalPos desk : Atlas.markets(server)) { // desks with lockers, settlements' desks, caravans' ends
            ServerLevel level = server.getLevel(desk.dimension());
            if (level != null && level.isLoaded(desk.pos())) {
                broker(level, desk, Territory.get(server), level.getGameTime());
            }
        }
    }

    /** The desk's Merchant's level, if one works it. */
    static int merchantLevel(ServerLevel level, BlockPos desk) {
        return Merchant.workingAt(level, desk, Lockers.MERCHANT_RANGE).map(m -> m.getVillagerData().getLevel()).orElse(0);
    }

    /** One look at the shops round the desk. Returns false if no master Merchant works it. */
    public static boolean broker(ServerLevel level, GlobalPos desk, Territory territory, long now) {
        if (merchantLevel(level, desk.pos()) < MASTER) {
            return false;
        }
        MinecraftServer server = level.getServer();
        MerchantStock stock = MerchantStock.at(level, desk.pos());
        dev.b28.enterprise.cargo.Crates.receive(server, level, desk, stock, now); // crates consigned here
        Procurement.procure(level, desk, stock); // what its own villagers sell
        Exchange exchange = Exchange.get(server);
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        List<VendorBlockEntity> shops = Shops.near(level, desk.pos());
        Map<Item, Exchange.Order> open = new LinkedHashMap<>();
        Set<BlockPos> commissioned = new LinkedHashSet<>();
        for (VendorBlockEntity shop : shops) {
            if (Shops.own(shop, desk.pos()) || shop.getFilterItem().isEmpty()
                    || shop.getMode() != VendorBlockEntity.Mode.BUY
                    || Sovereign.get(server).shops().contains(shop.getBlockPos())
                    || MetalExchange.get(server).shops().contains(shop.getBlockPos())
                    || Requisitions.get(server).isShop(shop.getBlockPos())
                    || Freight.get(server).isShop(shop.getBlockPos())) {
                continue; // the sovereign's and the exchange's own shops buy from players, not from its Merchant
            }
            if (Shops.enchanted(shop.getFilterItem())) {
                commissioned.add(shop.getBlockPos());
                commission(level, desk, territory, stock, shop, now);
            } else {
                fill(level, desk, territory, stock, shop, rate, now, open);
            }
        }
        exchange.jobs(desk).forEach((pos, job) -> {
            if (!commissioned.contains(pos)) { // the shop is gone, or wants something plain now
                if (!job.ready.isEmpty()) {
                    stock.insert(job.ready);
                }
                exchange.endJob(desk, pos);
            }
        });
        exchange.setOrders(desk, open);
        List<Exchange.Signal> signals = signals(level, desk, territory, stock, shops, rate, now);
        exchange.setSignals(desk, signals);
        for (VendorBlockEntity shop : shops) {
            if (!Shops.own(shop, desk.pos()) && shop.getMode() == VendorBlockEntity.Mode.SELL
                    && !shop.getFilterItem().isEmpty() && !Freight.get(server).isShop(shop.getBlockPos())) {
                buy(level, desk, territory, stock, shop, signals, rate, now);
            }
        }
        List<VendorBlockEntity> own = shops.stream().filter(s -> Shops.own(s, desk.pos())).toList();
        sellSurplus(level, desk, stock, own, rate, now);
        return true;
    }

    /** What the warehouse can spare of an item: beyond its par. */
    static int spare(MinecraftServer server, GlobalPos desk, MerchantStock stock, Item item, Set<Item> wanted, long now) {
        return Math.max(0, stock.count(new ItemCost(item, 1))
                - Warehouse.get(server).par(desk, item, wanted.contains(item), now));
    }

    /** A contract with a player's shop runs this many installments at most, one a day (commissions, every two). */
    public static final int SHOP_INSTALLMENTS = 10;
    public static final long SHOP_EVERY = 24000;
    public static final long COMMISSION_EVERY = 2 * 24000;

    /** A supplier town asks this much over the goods' worth, before carriage and tolls. */
    public static final double SUPPLIER_MARGIN = 0.1;

    /**
     * The contract with a player's shop for what it orders: drafted in person at the shop (it's at this desk's market),
     * so in force at once; Ex Works (the shop is at the seller's); as many installments as the shop's purse would pay
     * for, up to {@link #SHOP_INSTALLMENTS}, at the shop's price, which is then locked. None if the shop offers less
     * than {@link #AGENT_FLOOR} of the goods' worth, or can't pay a lot, or trust has run out.
     */
    static Optional<dev.b28.enterprise.contract.Contract> contractFor(ServerLevel level, GlobalPos desk,
            VendorBlockEntity shop, double rate, long now) {
        var book = dev.b28.enterprise.contract.ContractBook.get(level.getServer());
        ItemStack lot = shop.getFilterItem();
        GlobalPos buyer = GlobalPos.of(level.dimension(), shop.getBlockPos());
        int price = shop.getTotalPrice();
        var delivery = dev.b28.enterprise.contract.Delivery.of(lot, lot.getCount(), price / (double) lot.getCount() / rate);
        var existing = book.open(desk, buyer, delivery).stream().filter(c -> c.inForce()).findFirst();
        if (existing.isPresent()) {
            return existing;
        }
        double worth = Prices.emeraldsEach(lot.getItem()) * lot.getCount() * rate;
        int lots = Shops.purse(shop).map(p -> price <= 0 ? 0 : p.getBalance() / price).orElse(0);
        if (price < AGENT_FLOOR * worth || lots < 1
                || book.trust(desk, buyer, now) < dev.b28.enterprise.contract.ContractBook.MIN_TRUST) {
            return Optional.empty();
        }
        var contract = book.draft(desk, buyer, delivery, dev.b28.enterprise.contract.Incoterm.EXW, now,
                Shops.enchanted(lot) ? COMMISSION_EVERY : SHOP_EVERY, Math.min(SHOP_INSTALLMENTS, lots), now, desk, true);
        Exchange.get(level.getServer()).note(desk, now, "contract #%d with the shop at %s: %s".formatted(contract.id(),
                shop.getBlockPos().toShortString(), contract.history().get(0)));
        return Optional.of(contract);
    }

    /** Hands a lot to a shop under its contract, at the locked price; false if the shop couldn't take or pay for it. */
    static boolean deliver(ServerLevel level, GlobalPos desk, MerchantStock stock, VendorBlockEntity shop,
            dev.b28.enterprise.contract.Contract contract, ItemStack goods, double rate, long now) {
        var book = dev.b28.enterprise.contract.ContractBook.get(level.getServer());
        var delivery = contract.delivery().orElseThrow();
        // The locked price, at today's rate, but never more than the shop now asks for that many: the Merchant won't
        // charge a player above its own posted price.
        int asked = (int) Math.floor(shop.getTotalPrice() * goods.getCount() / (double) shop.getFilterItem().getCount());
        int price = Math.min(asked, (int) Math.round(delivery.unitPrice() * goods.getCount() * rate));
        boolean affordable = Shops.purse(shop).map(p -> p.getBalance() >= price).orElse(false);
        int paid = affordable ? Shops.sellTo(shop, goods, price) : -1;
        if (paid < 0) {
            if (!affordable) {
                book.unpaid(contract, now);
            }
            return false;
        }
        Treasury.deposit(level, desk.pos(), stock, paid);
        book.settle(contract, goods.getCount(), paid / rate, now);
        Exchange.Tally tally = Exchange.get(level.getServer()).tally(desk);
        tally.lotsSold++;
        tally.spursFromSales += paid;
        return true;
    }

    /**
     * Fills a player's order under its contract, from what the warehouse can spare, lot by lot as installments fall
     * due; what the contract still wants beyond the warehouse stays open for the region, and is sourced from a
     * supplier town.
     */
    private static void fill(ServerLevel level, GlobalPos desk, Territory territory, MerchantStock stock,
            VendorBlockEntity shop, double rate, long now, Map<Item, Exchange.Order> open) {
        MinecraftServer server = level.getServer();
        ItemStack lot = shop.getFilterItem();
        Item item = lot.getItem();
        var contract = contractFor(level, desk, shop, rate, now);
        if (contract.isEmpty()) {
            return; // priced under its worth, nothing to pay with, or no trust left: the Merchant won't take it on
        }
        Exchange exchange = Exchange.get(server);
        Set<Item> wanted = Caravans.wantedAt(server, desk);
        int sold = 0;
        while (contract.get().owed(now) >= lot.getCount() && Shops.roomFor(shop) > 0
                && spare(server, desk, stock, item, wanted, now) >= lot.getCount()) {
            ItemStack goods = merge(stock.take(new ItemCost(item, 1), lot.getCount()));
            if (!deliver(level, desk, stock, shop, contract.get(), goods, rate, now)) {
                stock.insert(goods);
                break;
            }
            sold++;
        }
        if (sold > 0) {
            exchange.note(desk, now, "delivered %d lots of %d %s to the shop at %s under contract #%d".formatted(sold,
                    lot.getCount(), path(item), shop.getBlockPos().toShortString(), contract.get().id()));
        }
        int remaining = contract.get().installments() * lot.getCount() - contract.get().delivered();
        int short_ = remaining - spare(server, desk, stock, item, wanted, now);
        if (short_ > 0) {
            open.merge(item, new Exchange.Order(item, short_, contract.get().delivery().get().unitPrice() * rate,
                    shop.getBlockPos()), (a, b) -> new Exchange.Order(item, a.outstanding() + b.outstanding(),
                            Math.max(a.spursEach(), b.spursEach()), a.shop()));
            source(level, desk, territory, item, short_, now);
        }
    }

    /** A supplier that can make or sell goods, but doesn't hold them, is asked for at most this many at a time. */
    public static final int MAKER_LOT = 16;
    /** And given this long to make them before the first is due, in this many installments a day apart. */
    public static final long MAKER_LEAD = 3 * 24000L;
    public static final int MAKER_INSTALLMENTS = 4;
    /** Work to make is paid this share up front (an advance), where the two trust each other. */
    public static final double MAKER_ADVANCE = 1 / 3.0;
    /** Where they don't, they still deal, on trial terms: this many, in one lot, nothing up front. */
    public static final int TRIAL_LOT = 4;

    /**
     * Contracts suppliers for goods this desk is short of, until what's contracted covers the need: markets it knows
     * with a road between them, nearest first, that held the goods at their last stocktake, or (failing those) whose
     * people include a trade that makes or sells them (a Weaponsmith for swords; see {@link Procurement#suppliers}).
     * Ex Works if this desk's caravans go there (they collect), else Delivered At Place. One open contract per good
     * per supplier. Drafted here, each is in force once a caravan between the two carries its copy. Returns true if
     * the need is covered.
     */
    public static boolean source(ServerLevel level, GlobalPos desk, Territory territory, Item item, int need, long now) {
        return source(level, desk, territory, item, need, now, 1, List.of());
    }

    /**
     * As above, paying {@code premium} times the price (a sovereign at war), with {@code extra} terms (bonus clauses)
     * in each contract.
     */
    public static boolean source(ServerLevel level, GlobalPos desk, Territory territory, Item item, int need, long now,
            double premium, List<dev.b28.enterprise.contract.Term> extra) {
        MinecraftServer server = level.getServer();
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        var plain = new dev.b28.enterprise.contract.Delivery(item, 1, 0);
        Warehouse warehouse = Warehouse.get(server);
        var ledger = dev.b28.enterprise.caravan.CaravanLedger.get(server);
        List<GlobalPos> markets = Atlas.get(server).known(server, desk);
        int covered = coming(server, desk, item);
        if (covered >= need) {
            return true;
        }
        var crown = Sovereign.get(server);
        for (boolean holders : List.of(true, false)) {
            for (GlobalPos supplier : markets) {
                // One order open at a time with a supplier; but the crown's front orders from its capital day by day,
                // each order what's short beyond those on the way, so every caravan out brings the latest.
                boolean crownOrders = crown.isCrown(supplier) && crown.isCrown(desk);
                if (covered >= need || !crownOrders && !book.open(supplier, desk, plain).isEmpty()) {
                    continue;
                }
                Optional<Means> found = means(level, desk, supplier, item, now);
                if (found.isEmpty() || (holders ? found.get().spare() <= 0 : !found.get().makes())) {
                    continue;
                }
                Means means = found.get();
                // The crown doesn't put its own army on trial: its capital supplies its front whatever their record.
                boolean trusted = means.trusted() || crownOrders;
                var incoterm = means.incoterm();
                var route = means.route();
                // (food is made by the stack: a Butcher's restock is dozens)
                int quantity = Math.min(need - covered, holders ? means.spare() : makerLot(item));
                if (!trusted) {
                    quantity = Math.min(quantity, TRIAL_LOT); // a trial lot, to rebuild the record
                }
                // Work to make comes in small installments a day apart, the first after a lead: steady deliveries
                // build trust, and the seller isn't asked for everything at once.
                int installments = holders || !trusted ? 1 : Math.min(MAKER_INSTALLMENTS, quantity);
                int each = (quantity + installments - 1) / installments;
                var delivery = new dev.b28.enterprise.contract.Delivery(item, each,
                        landed(territory, item, means, premium));
                List<dev.b28.enterprise.contract.Term> terms = new ArrayList<>();
                terms.add(delivery);
                terms.addAll(extra);
                if (!holders && trusted) {
                    terms.add(new dev.b28.enterprise.contract.Advance(MAKER_ADVANCE)); // money to start the work with
                }
                // Delivered at the buyer's, the first installment is due once the seller's next caravan can be there.
                long due = holders ? now : now + MAKER_LEAD;
                if (incoterm == dev.b28.enterprise.contract.Incoterm.DAP) {
                    due = Math.max(due, ledger.nextDeparture(route, now)) + (long) (means.roadDays() * 24000);
                }
                var contract = book.draft(supplier, desk, terms, incoterm, due,
                        Caravans.DEPARTURE_INTERVAL, installments, now, desk, false);
                covered += each * installments;
                Exchange.get(server).note(desk, now, "sourced %d %s from %s, %s, %s (contract #%d)".formatted(
                        each * installments, path(item), supplier.pos().toShortString(),
                        holders ? "from its stock" : trusted ? "to make, a third up front" : "on trial", incoterm.name(),
                        contract.id()));
            }
        }
        return covered >= need;
    }

    /**
     * What a supplier can do for a desk's order: what it can spare of the good (beyond its par, half its kit unless it's
     * an armoury, and its caravans' food), whether its people make or sell it, the road and terms it would go by (Ex
     * Works if the desk's caravans go there, else Delivered At Place), and whether the two trust each other.
     */
    public record Means(int spare, boolean makes, dev.b28.enterprise.caravan.Route route,
            dev.b28.enterprise.contract.Incoterm incoterm, boolean trusted, Optional<Freight.Leg> onward) {
        /** Days the goods are on the road: the route, and the forwarded leg beyond a hub (and a day there). */
        public double roadDays() {
            double days = Caravans.travelTicks(route.distance()) / 24000.0;
            return onward.map(l -> days + 1 + Caravans.travelTicks(l.route().distance()) / 24000.0).orElse(days);
        }
    }

    /** {@link Means} of {@code supplier} for {@code desk}, if there's a road between them. */
    public static Optional<Means> means(ServerLevel level, GlobalPos desk, GlobalPos supplier, Item item, long now) {
        MinecraftServer server = level.getServer();
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        Warehouse warehouse = Warehouse.get(server);
        var ledger = dev.b28.enterprise.caravan.CaravanLedger.get(server);
        Territory territory = Territory.get(server);
        var collect = ledger.route(desk, supplier);
        var bring = ledger.route(supplier, desk);
        // No road between them, but the supplier's caravans go to a freight hub with a carrier's run to the desk: its
        // goods go there and are forwarded, delivered at the desk.
        Optional<Freight.Leg> onward = collect.isEmpty() && bring.isEmpty()
                ? Freight.via(server, supplier, desk) : Optional.empty();
        if (collect.isEmpty() && bring.isEmpty() && onward.isEmpty()) {
            return Optional.empty(); // no road between them: a gap a player could fill
        }
        if (onward.isPresent()) {
            GlobalPos hub = onward.get().from();
            bring = ledger.route(supplier, hub);
        }
        boolean kit = dev.b28.enterprise.territory.Smithing.KITS.stream().anyMatch(k -> k.item() == item);
        boolean trusted = book.trust(supplier, desk, now) >= dev.b28.enterprise.contract.ContractBook.MIN_TRUST;
        int held = warehouse.onHand(supplier, item);
        // A town keeps half its kit for its own hands; an armoury's goods are there to be sent on.
        boolean keepsHalf = kit && !dev.b28.enterprise.merchant.Depots.get(server)
                .has(supplier, dev.b28.enterprise.merchant.Depots.Kind.ARMOURY);
        int spare = (keepsHalf ? held / 2 : held) - warehouse.par(supplier, item, false, now);
        var food = item.getDefaultInstance().get(net.minecraft.core.component.DataComponents.FOOD);
        if (food != null && food.nutrition() > 0) {
            // And the food its caravans need, as far as its other food doesn't cover it.
            int otherPoints = warehouse.stocktake(supplier).entrySet().stream()
                    .filter(e -> !e.getKey().equals(BuiltInRegistries.ITEM.getKey(item).toString()))
                    .mapToInt(e -> e.getValue() * dev.b28.enterprise.army.Army.nutrition(
                            BuiltInRegistries.ITEM.get(ResourceLocation.parse(e.getKey()))))
                    .sum();
            spare -= Math.max(0, 2 * Warehouse.FOOD_PAR_POINTS - otherPoints) / food.nutrition();
        }
        // It can make them if a villager of it sells them (at the level), or a trade of it makes them from inputs
        // it holds or this desk does (and its caravans would bring): no gold without a mine's raw gold.
        int least = Procurement.sellerLevel(item);
        var residents = territory.bellOf(supplier).map(bell -> Services.residents(level, bell)).orElse(Map.of());
        // (and, where its Merchant has seen them restock, one of them does offer it: a trade's offers vary)
        boolean sells = residents.entrySet().stream().anyMatch(e -> Procurement.SELLERS.getOrDefault(item,
                Set.of()).contains(e.getKey()) && e.getValue() >= least)
                && warehouse.offers(supplier, item).orElse(true);
        boolean crafts = dev.b28.enterprise.service.Crafts.recipesFor(item).stream()
                .anyMatch(r -> residents.containsKey(r.trade()) && r.inputs().keySet().stream()
                        .allMatch(in -> warehouse.onHand(supplier, in) > 0 || warehouse.onHand(desk, in) > 0));
        var incoterm = collect.isPresent() ? dev.b28.enterprise.contract.Incoterm.EXW
                : dev.b28.enterprise.contract.Incoterm.DAP;
        return Optional.of(new Means(Math.max(0, spare), sells || crafts, collect.orElseGet(bring::get), incoterm,
                trusted, onward));
    }

    /** A lot to make: food by the stack (a Butcher's restock is dozens), else {@link #MAKER_LOT}. */
    public static int makerLot(Item item) {
        var food = item.getDefaultInstance().get(net.minecraft.core.component.DataComponents.FOOD);
        return food != null && food.nutrition() > 0 ? Math.max(MAKER_LOT, item.getDefaultMaxStackSize()) : MAKER_LOT;
    }

    /**
     * The price each, in emeralds, a supplier asks on these terms: the good's worth and its margin (times
     * {@code premium}), and under Delivered At Place the carriage and the tolls on the way.
     */
    public static double landed(Territory territory, Item item, Means means, double premium) {
        double forwarding = means.onward().map(l -> Freight.forwardingPrice(l.route())
                / item.getDefaultMaxStackSize()).orElse(0.0);
        return landed(territory, item, means.route(), means.incoterm(), premium) + forwarding;
    }

    public static double landed(Territory territory, Item item, dev.b28.enterprise.caravan.Route route,
            dev.b28.enterprise.contract.Incoterm incoterm, double premium) {
        double exWorks = Prices.emeraldsEach(item) * (1 + SUPPLIER_MARGIN) * premium;
        double carriage = Caravans.foodNeeded(route.distance()) * 2 * Caravans.PURSE_PER_FOOD_POINT
                / (Caravans.CARGO_SLOTS * (double) item.getDefaultMaxStackSize());
        double tolls = Caravans.tollShare(territory, route) * exWorks;
        return incoterm.price(exWorks, carriage, tolls);
    }

    /** What of an item the desk has under contract from the markets it knows and not yet delivered. */
    public static int coming(MinecraftServer server, GlobalPos desk, Item item) {
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        var plain = new dev.b28.enterprise.contract.Delivery(item, 1, 0);
        int coming = 0;
        for (GlobalPos m : Atlas.get(server).known(server, desk)) {
            for (var c : book.open(m, desk, plain)) {
                coming += c.delivery().map(d -> d.quantity() * c.installments() - c.delivered()).orElse(0);
            }
        }
        return coming;
    }

    /** What its town and its orders want of an item, beyond what it holds. */
    static int want(MinecraftServer server, GlobalPos desk, MerchantStock stock, Item item, Set<Item> wanted, long now) {
        int held = stock.count(new ItemCost(item, 1));
        int ordered = Exchange.get(server).wanted(desk).getOrDefault(item, 0);
        int par = wanted.contains(item) ? Warehouse.get(server).par(desk, item, true, now) : 0;
        return Math.max(0, ordered + par - held);
    }

    /** Buys from a player's shop what its town or its orders want, at up to the worth (more for posted wants). */
    private static void buy(ServerLevel level, GlobalPos desk, Territory territory, MerchantStock stock,
            VendorBlockEntity shop, List<Exchange.Signal> signals, double rate, long now) {
        MinecraftServer server = level.getServer();
        ItemStack lot = shop.getFilterItem();
        Item item = lot.getItem();
        if (Shops.enchanted(lot) || item instanceof CoinItem) {
            return;
        }
        Set<Item> wanted = Caravans.wantedAt(server, desk);
        Optional<Exchange.Signal> signal = signals.stream().filter(s -> s.item() == item).findFirst();
        int want = want(server, desk, stock, item, wanted, now) + signal.map(Exchange.Signal::count).orElse(0);
        double ceiling = signal.map(s -> (double) s.spursEach() * lot.getCount())
                .orElse(Prices.emeraldsEach(item) * lot.getCount() * rate * MAKER_CEILING);
        int price = shop.getTotalPrice();
        Mint mint = Mint.get(server);
        Exchange exchange = Exchange.get(server);
        int bought = 0;
        while (want > 0 && price > 0 && price <= ceiling && Shops.lots(shop) > 0 && stock.hasRoomFor(lot)
                && Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk) >= price / rate) {
            ItemStack goods = Shops.buyFrom(shop);
            if (goods.isEmpty()) {
                break;
            }
            Treasury.withdraw(level, desk.pos(), stock, price / rate, mint, now);
            ItemStack rest = stock.insert(goods);
            if (!rest.isEmpty()) {
                Shops.stock(shop, rest); // shouldn't happen: room was checked
            }
            want -= goods.getCount();
            bought++;
            Exchange.Tally tally = exchange.tally(desk);
            tally.lotsBought++;
            tally.spursOnPurchases += price;
        }
        if (bought > 0) {
            exchange.note(desk, now, "bought %d lots of %d %s from the shop at %s for %d spurs each".formatted(bought,
                    lot.getCount(), path(item), shop.getBlockPos().toShortString(), price));
        }
    }

    /**
     * What the town wants that no market it knows has, nor a player's shop here sells: its open orders, its lockers'
     * wants, and the base goods its commissions are waiting on.
     */
    static List<Exchange.Signal> signals(ServerLevel level, GlobalPos desk, Territory territory, MerchantStock stock,
            List<VendorBlockEntity> shops, double rate, long now) {
        MinecraftServer server = level.getServer();
        Set<Item> wanted = Caravans.wantedAt(server, desk);
        Map<Item, Integer> wants = new LinkedHashMap<>();
        Map<Item, String> why = new LinkedHashMap<>();
        Exchange.get(server).wanted(desk).forEach((item, count) -> {
            wants.merge(item, Math.max(0, count - stock.count(new ItemCost(item, 1))), Integer::sum);
            why.putIfAbsent(item, "a player's order");
        });
        for (Item item : wanted) {
            int short_ = Warehouse.get(server).par(desk, item, true, now) - stock.count(new ItemCost(item, 1));
            if (short_ > 0) {
                wants.merge(item, short_, Integer::sum);
                why.putIfAbsent(item, "its lockers' trade");
            }
        }
        Exchange.get(server).jobs(desk).forEach((shop, job) -> {
            if (job.step.startsWith("waiting for ")) {
                Item base = BuiltInRegistries.ITEM.get(ResourceLocation.parse(job.step.substring("waiting for ".length())));
                if (base != Items.AIR) {
                    wants.merge(base, 1, Integer::sum);
                    why.putIfAbsent(base, "a commission");
                }
            }
        });
        Sovereign.get(server).unmet(desk).forEach((item, count) -> {
            wants.merge(item, count, Integer::sum);
            why.putIfAbsent(item, "the war effort");
        });

        Warehouse warehouse = Warehouse.get(server);
        List<GlobalPos> markets = Atlas.get(server).known(server, desk);
        List<Exchange.Signal> signals = new ArrayList<>();
        wants.forEach((item, count) -> {
            if (count <= 0 || item == Items.AIR) {
                return;
            }
            boolean supplied = markets.stream().anyMatch(m -> warehouse.onHand(m, item) > 0)
                    || shops.stream().anyMatch(s -> s.getMode() == VendorBlockEntity.Mode.SELL
                            && s.getFilterItem().is(item) && Shops.lots(s) > 0);
            if (!supplied) {
                int each = (int) Math.ceil(Prices.emeraldsEach(item) * rate * SIGNAL_PREMIUM);
                signals.add(new Exchange.Signal(item, count, Math.max(1, each), why.get(item)));
            }
        });
        signals.sort(Comparator.comparingDouble((Exchange.Signal s) -> -(double) s.count() * s.spursEach()));
        return signals;
    }

    /** The town's own shops sell its biggest surpluses (beyond par, no food, no money), a little over their worth. */
    private static void sellSurplus(ServerLevel level, GlobalPos desk, MerchantStock stock, List<VendorBlockEntity> own,
            double rate, long now) {
        if (own.isEmpty()) {
            return;
        }
        MinecraftServer server = level.getServer();
        Exchange exchange = Exchange.get(server);
        for (VendorBlockEntity shop : own) {
            int takings = Shops.collect(shop);
            if (takings > 0) {
                Treasury.deposit(level, desk.pos(), stock, takings);
                exchange.tally(desk).spursFromOwnShops += takings;
                exchange.note(desk, now, "its shop at %s took %d spurs".formatted(shop.getBlockPos().toShortString(), takings));
            }
        }
        Set<Item> wanted = Caravans.wantedAt(server, desk);
        Map<Item, Integer> ordered = exchange.wanted(desk);
        List<Map.Entry<Item, Integer>> surplus = new ArrayList<>();
        stock.totals().forEach((id, count) -> {
            Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(id));
            if (item == Items.EMERALD || item instanceof CoinItem || id.endsWith("shulker_box")
                    || item == dev.b28.enterprise.cargo.Cargo.CRATE_ITEM.get() || item == dev.b28.enterprise.cargo.Cargo.DOCUMENTS.get()
                    || item.components().has(DataComponents.FOOD) || ordered.containsKey(item)) {
                return;
            }
            int spare = spare(server, desk, stock, item, wanted, now);
            if (spare >= Math.min(OWN_LOT, item.getDefaultMaxStackSize())) {
                surplus.add(Map.entry(item, spare));
            }
        });
        surplus.sort(Comparator.comparingDouble((Map.Entry<Item, Integer> e) -> -e.getValue() * Prices.emeraldsEach(e.getKey())));
        for (int i = 0; i < own.size(); i++) {
            VendorBlockEntity shop = own.get(i);
            if (i >= surplus.size()) {
                break;
            }
            Item item = surplus.get(i).getKey();
            int lotSize = Math.min(OWN_LOT, item.getDefaultMaxStackSize());
            int price = Math.max(1, (int) Math.ceil(Prices.emeraldsEach(item) * lotSize * rate * OWN_MARKUP));
            if (!shop.getFilterItem().is(item) || shop.getFilterItem().getCount() != lotSize) {
                Shops.offer(shop, new ItemStack(item, lotSize), price);
                Shops.clearOthers(shop).forEach(back -> {
                    ItemStack rest = stock.insert(back);
                    if (!rest.isEmpty()) {
                        Shops.stock(shop, rest);
                    }
                });
            }
            // Keep a stack's worth on its shelf.
            int onShelf = shop.items.stream().filter(s -> s.is(item)).mapToInt(ItemStack::getCount).sum();
            int top = Math.min(item.getDefaultMaxStackSize() - onShelf, surplus.get(i).getValue());
            if (top > 0) {
                ItemStack goods = merge(stock.take(new ItemCost(item, 1), top));
                ItemStack rest = Shops.stock(shop, goods);
                if (!rest.isEmpty()) {
                    stock.insert(rest);
                }
            }
        }
    }

    // Commissions: enchanted goods to order.

    /** Plain goods: no enchantment yet. */
    static ItemCost plain(Item item) {
        return new ItemCost(item.builtInRegistryHolder(), 1,
                DataComponentPredicate.builder().expect(DataComponents.ENCHANTMENTS, ItemEnchantments.EMPTY).build());
    }

    /** Books holding exactly these enchantments. */
    static ItemCost book(ItemEnchantments enchantments) {
        return new ItemCost(Items.ENCHANTED_BOOK.builtInRegistryHolder(), 1,
                DataComponentPredicate.builder().expect(DataComponents.STORED_ENCHANTMENTS, enchantments).build());
    }

    static ItemEnchantments stored(ItemStack book) {
        return book.getOrDefault(DataComponents.STORED_ENCHANTMENTS, ItemEnchantments.EMPTY);
    }

    /** The books in stock holding {@code enchantment}, with the level each holds it at. */
    static List<Map.Entry<ItemStack, Integer>> booksOf(MerchantStock stock, Holder<Enchantment> enchantment) {
        List<Map.Entry<ItemStack, Integer>> books = new ArrayList<>();
        for (ItemStack book : stock.copies(new ItemCost(Items.ENCHANTED_BOOK, 1))) {
            int at = stored(book).getLevel(enchantment);
            if (at > 0) {
                for (int i = 0; i < book.getCount(); i++) {
                    books.add(Map.entry(book.copyWithCount(1), at));
                }
            }
        }
        books.sort(Comparator.comparingInt((Map.Entry<ItemStack, Integer> e) -> -e.getValue()));
        return books;
    }

    /** What the order asks for: each enchantment and its level. */
    static ItemEnchantments wants(ItemStack filter) {
        ItemEnchantments on = filter.getOrDefault(DataComponents.ENCHANTMENTS, ItemEnchantments.EMPTY);
        return on.isEmpty() ? stored(filter) : on;
    }

    /**
     * Works a commission a few steps on, as the town's master Librarian and Cleric can: for each enchantment the order
     * wants, a book at its level, by copying a book (a book and a lapis), combining two at the same level into one a
     * level up (an emerald of bottles), or, with none, having the Cleric enchant a book at the table (lapis and
     * bottles) and seeing what comes; then the Cleric lays the books on the plain good (an emerald each) and the good
     * is handed over at the shop's price.
     */
    static void commission(ServerLevel level, GlobalPos desk, Territory territory, MerchantStock stock,
            VendorBlockEntity shop, long now) {
        MinecraftServer server = level.getServer();
        Exchange exchange = Exchange.get(server);
        Exchange.Job job = exchange.job(desk, shop.getBlockPos());
        ItemStack filter = shop.getFilterItem().copyWithCount(1);
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        var contract = contractFor(level, desk, shop, rate, now);
        if (contract.isEmpty()) {
            job.step = "no contract: the shop offers too little, can't pay, or trust has run out";
            return;
        }
        if (!job.ready.isEmpty()) {
            if (!shop.matchesFilterItem(job.ready)) {
                stock.insert(job.ready); // the order changed: the good goes to the warehouse
                job.ready = ItemStack.EMPTY;
            } else if (contract.get().owed(now) < 1) {
                job.step = "made, ready for the next installment";
                return;
            } else if (deliver(level, desk, stock, shop, contract.get(), job.ready, rate, now)) {
                exchange.tally(desk).commissions++;
                exchange.note(desk, now, "handed over a commissioned %s under contract #%d".formatted(
                        path(filter.getItem()), contract.get().id()));
                job.ready = ItemStack.EMPTY;
                job.step = "delivered";
                return;
            } else {
                job.step = "made, waiting for the shop's room or money";
                return;
            }
        }
        int remaining = contract.get().installments() * contract.get().delivery().get().quantity()
                - contract.get().delivered();
        if (remaining <= 0) {
            job.step = "contract fulfilled";
            return;
        }
        Optional<GlobalPos> bell = territory.bellOf(desk);
        Map<String, Integer> skills = bell.map(b -> Services.residents(level, b)).orElse(Map.of());
        int librarian = skills.getOrDefault("minecraft:librarian", 0);
        int cleric = skills.getOrDefault("minecraft:cleric", 0);
        if (librarian < MASTER || cleric < 3) {
            job.step = "needs a master Librarian and a Cleric in town";
            return;
        }
        ItemEnchantments wants = wants(filter);
        boolean book = filter.is(Items.ENCHANTED_BOOK);
        for (int step = 0; step < STEPS; step++) {
            Optional<Map.Entry<Holder<Enchantment>, Integer>> missing = wants.entrySet().stream()
                    .map(e -> Map.entry(e.getKey(), e.getIntValue()))
                    .filter(e -> booksOf(stock, e.getKey()).stream().noneMatch(b -> b.getValue() >= e.getValue()))
                    .findFirst();
            if (missing.isEmpty()) {
                // Every book is ready: lay them on.
                if (!book && stock.count(plain(filter.getItem())) == 0) {
                    job.step = "waiting for " + BuiltInRegistries.ITEM.getKey(filter.getItem());
                    source(level, desk, territory, filter.getItem(), remaining, now); // from a supplier town
                    return;
                }
                if (!pay(level, desk, territory, stock, XP_EMERALDS * wants.size(), now)) {
                    job.step = "saving for bottles o' enchanting";
                    return;
                }
                ItemStack good = book ? new ItemStack(Items.ENCHANTED_BOOK) : stock.take(plain(filter.getItem()), 1).get(0);
                for (var entry : wants.entrySet()) {
                    var holder = entry.getKey();
                    var source = booksOf(stock, holder).stream().filter(b -> b.getValue() >= entry.getIntValue()).findFirst();
                    source.ifPresent(b -> stock.take(book(stored(b.getKey())), 1));
                    int lvl = entry.getIntValue();
                    EnchantmentHelper.updateEnchantments(good, m -> m.set(holder, lvl));
                }
                job.ready = good;
                job.steps++;
                job.step = "made";
                exchange.note(desk, now, "made a %s to order".formatted(path(filter.getItem())));
                return;
            }
            Holder<Enchantment> enchantment = missing.get().getKey();
            int target = missing.get().getValue();
            if (target > enchantment.value().getMaxLevel()) {
                job.step = "can't be made: " + enchantment.getRegisteredName() + " goes no higher than "
                        + enchantment.value().getMaxLevel();
                return;
            }
            List<Map.Entry<ItemStack, Integer>> books = booksOf(stock, enchantment);
            if (!books.isEmpty()) {
                int best = books.get(0).getValue();
                long pair = books.stream().filter(b -> b.getValue() == best).count();
                if (pair >= 2) {
                    // The Librarian combines two at the same level, as an anvil would: one a level up.
                    if (!pay(level, desk, territory, stock, XP_EMERALDS, now)) {
                        job.step = "saving for bottles o' enchanting";
                        return;
                    }
                    var two = books.stream().filter(b -> b.getValue() == best).limit(2).toList();
                    two.forEach(b -> stock.take(book(stored(b.getKey())), 1));
                    ItemStack better = new ItemStack(Items.ENCHANTED_BOOK);
                    better.set(DataComponents.STORED_ENCHANTMENTS, single(enchantment, best + 1));
                    stock.insert(better);
                    job.step = "combined two books into %s %d".formatted(enchantment.getRegisteredName(), best + 1);
                } else {
                    // The Librarian copies the best one: a book and a lapis.
                    if (stock.count(new ItemCost(Items.BOOK, 1)) == 0 || stock.count(new ItemCost(Items.LAPIS_LAZULI, 1)) == 0) {
                        job.step = "waiting for a book and a lapis to copy with";
                        source(level, desk, territory, Items.BOOK, 4, now);
                        source(level, desk, territory, Items.LAPIS_LAZULI, 4, now);
                        return;
                    }
                    stock.take(new ItemCost(Items.BOOK, 1), 1);
                    stock.take(new ItemCost(Items.LAPIS_LAZULI, 1), 1);
                    ItemStack copy = new ItemStack(Items.ENCHANTED_BOOK);
                    copy.set(DataComponents.STORED_ENCHANTMENTS, single(enchantment, best));
                    stock.insert(copy);
                    job.step = "copied %s %d".formatted(enchantment.getRegisteredName(), best);
                }
            } else {
                // No book has it: the Cleric enchants a book at the table and they see what comes.
                int tier = cleric >= 5 ? 30 : cleric >= 4 ? 20 : 10;
                if (stock.count(new ItemCost(Items.BOOK, 1)) == 0
                        || stock.count(new ItemCost(Items.LAPIS_LAZULI, 1)) < tier / 10) {
                    job.step = "waiting for books and lapis to search for " + enchantment.getRegisteredName();
                    source(level, desk, territory, Items.BOOK, 4, now);
                    source(level, desk, territory, Items.LAPIS_LAZULI, 4 * (tier / 10), now);
                    return;
                }
                if (!pay(level, desk, territory, stock, tier / 10.0, now)) {
                    job.step = "saving for bottles o' enchanting";
                    return;
                }
                stock.take(new ItemCost(Items.BOOK, 1), 1);
                stock.take(new ItemCost(Items.LAPIS_LAZULI, 1), tier / 10);
                var table = level.registryAccess().registryOrThrow(Registries.ENCHANTMENT)
                        .getTag(EnchantmentTags.IN_ENCHANTING_TABLE);
                ItemStack found = EnchantmentHelper.enchantItem(level.random, new ItemStack(Items.BOOK), tier,
                        level.registryAccess(), table);
                stock.insert(found);
                job.step = "searched the table for " + enchantment.getRegisteredName();
            }
            job.steps++;
        }
    }

    private static ItemEnchantments single(Holder<Enchantment> enchantment, int level) {
        ItemEnchantments.Mutable book = new ItemEnchantments.Mutable(ItemEnchantments.EMPTY);
        book.set(enchantment, level);
        return book.toImmutable();
    }

    /** Pays for commission work from money beyond the town's reserve; false if it can't. */
    private static boolean pay(ServerLevel level, GlobalPos desk, Territory territory, MerchantStock stock,
            double emeralds, long now) {
        Mint mint = Mint.get(level.getServer());
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        if (Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk) < emeralds) {
            return false;
        }
        Treasury.withdraw(level, desk.pos(), stock, emeralds, mint, now);
        return true;
    }

    private static ItemStack merge(List<ItemStack> pieces) {
        ItemStack out = ItemStack.EMPTY;
        for (ItemStack piece : pieces) {
            if (out.isEmpty()) {
                out = piece.copy();
            } else {
                out.grow(piece.getCount());
            }
        }
        return out;
    }

    private static String path(Item item) {
        return BuiltInRegistries.ITEM.getKey(item).getPath();
    }
}
