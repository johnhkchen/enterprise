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
        Exchange exchange = Exchange.get(server);
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        List<VendorBlockEntity> shops = Shops.near(level, desk.pos());
        Map<Item, Exchange.Order> open = new LinkedHashMap<>();
        Set<BlockPos> commissioned = new LinkedHashSet<>();
        for (VendorBlockEntity shop : shops) {
            if (Shops.own(shop, desk.pos()) || shop.getFilterItem().isEmpty()
                    || shop.getMode() != VendorBlockEntity.Mode.BUY) {
                continue;
            }
            if (Shops.enchanted(shop.getFilterItem())) {
                commissioned.add(shop.getBlockPos());
                commission(level, desk, territory, stock, shop, now);
            } else {
                fill(level, desk, stock, shop, rate, now, open);
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
                    && !shop.getFilterItem().isEmpty()) {
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

    /** Fills a player's order from the warehouse, at its price, lot by lot; what's left stays open. */
    private static void fill(ServerLevel level, GlobalPos desk, MerchantStock stock, VendorBlockEntity shop, double rate,
            long now, Map<Item, Exchange.Order> open) {
        MinecraftServer server = level.getServer();
        ItemStack lot = shop.getFilterItem();
        Item item = lot.getItem();
        int price = shop.getTotalPrice();
        double worth = Prices.emeraldsEach(item) * lot.getCount() * rate;
        Exchange exchange = Exchange.get(server);
        if (price < AGENT_FLOOR * worth) {
            return; // priced under its worth: the Merchant won't fill it, nor ask the region for it
        }
        Set<Item> wanted = Caravans.wantedAt(server, desk);
        int sold = 0;
        while (Shops.roomFor(shop) > 0 && Shops.purse(shop).map(p -> p.getBalance() >= price).orElse(false)
                && spare(server, desk, stock, item, wanted, now) >= lot.getCount()) {
            ItemStack goods = merge(stock.take(new ItemCost(item, 1), lot.getCount()));
            int paid = Shops.sellTo(shop, goods);
            if (paid == 0) {
                stock.insert(goods);
                break;
            }
            Treasury.deposit(level, desk.pos(), stock, paid);
            Exchange.Tally tally = exchange.tally(desk);
            tally.lotsSold++;
            tally.spursFromSales += paid;
            sold++;
        }
        if (sold > 0) {
            exchange.note(desk, now, "sold %d lots of %d %s to the shop at %s for %d spurs each".formatted(sold,
                    lot.getCount(), path(item), shop.getBlockPos().toShortString(), price));
        }
        int lots = Math.min(Shops.roomFor(shop),
                Shops.purse(shop).map(p -> price <= 0 ? 0 : p.getBalance() / price).orElse(0));
        if (lots > 0) {
            int outstanding = lots * lot.getCount();
            open.merge(item, new Exchange.Order(item, outstanding, price / (double) lot.getCount(), shop.getBlockPos()),
                    (a, b) -> new Exchange.Order(item, a.outstanding() + b.outstanding(),
                            Math.max(a.spursEach(), b.spursEach()), a.shop()));
        }
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
        if (!job.ready.isEmpty()) {
            if (!shop.matchesFilterItem(job.ready)) {
                stock.insert(job.ready); // the order changed: the good goes to the warehouse
                job.ready = ItemStack.EMPTY;
            } else {
                int paid = Shops.sellTo(shop, job.ready);
                if (paid > 0) {
                    Treasury.deposit(level, desk.pos(), stock, paid);
                    Exchange.Tally tally = exchange.tally(desk);
                    tally.commissions++;
                    tally.spursFromSales += paid;
                    exchange.note(desk, now, "handed over a commissioned %s for %d spurs".formatted(path(filter.getItem()), paid));
                    job.ready = ItemStack.EMPTY;
                    job.step = "delivered";
                } else {
                    job.step = "made, waiting for the shop's room or money";
                }
                return;
            }
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
