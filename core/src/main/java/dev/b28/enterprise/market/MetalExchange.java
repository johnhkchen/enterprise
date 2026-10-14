package dev.b28.enterprise.market;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.territory.Territory;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.saveddata.SavedData;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * The capital's metal exchange: a market maker in iron, coal, flint, diamonds and gold at the sovereign's port. It
 * aims to hold a stock of each; ships bring what it lacks from far away ({@link #VOYAGE_TICKS} at sea), paid from the
 * royal treasury at {@link #IMPORT_PREMIUM} times the metal's worth. It posts two prices for each, a bid and an ask,
 * a {@link #SPREAD} either side of a middle that rises as its stock runs short: the capital's caravans sell its metal
 * to towns at the ask (coal and iron for their smiths to train on and forge with), and buy towns' metal at the bid,
 * so a town with a mine has a buyer, and a short exchange pays more. Mobilising it aims to hold twice as much, at war
 * three times: the war's demand reaches the mines through the price.
 */
public final class MetalExchange extends SavedData {
    private static final String NAME = "enterprise_metal_exchange";

    /** A metal it makes a market in: the stock it aims to hold in peace, and the most one day's ships bring (0: none). */
    public record Metal(Item item, int target, int lot) {}

    public static final List<Metal> METALS = List.of(
            new Metal(Items.IRON_INGOT, 192, 64),
            new Metal(Items.COAL, 192, 64),
            new Metal(Items.FLINT, 96, 32),
            new Metal(Items.DIAMOND, 12, 4),
            new Metal(Items.GOLD_INGOT, 32, 0)); // gold the region's mines bring: the war chest

    public static final double SPREAD = 0.1;
    public static final double IMPORT_PREMIUM = 1.5;
    public static final long VOYAGE_TICKS = 3 * 24000L;
    /** How far the middle price moves with the stock: half again when empty, down to three quarters when over. */
    public static final double PRESSURE = 0.5;

    /** Metal at sea: so many, landing at the port on a day. */
    record Shipment(Item item, int count, long lands) {}

    /** What it has done in each metal, in all. */
    public static final class Book {
        int imported;
        double importEmeralds;
        int bought;
        double boughtEmeralds;
        int sold;
        double soldEmeralds;

        public int imported() {
            return imported;
        }

        public int bought() {
            return bought;
        }

        public int sold() {
            return sold;
        }
    }

    private GlobalPos desk;
    private long lastDay = -1;
    private final List<Shipment> atSea = new ArrayList<>();
    private final Map<Item, Book> books = new LinkedHashMap<>();
    private final Map<Item, Integer> held = new LinkedHashMap<>(); // at its last count

    public static MetalExchange get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(MetalExchange::new, MetalExchange::load, null), NAME);
    }

    public Optional<GlobalPos> desk() {
        return Optional.ofNullable(desk);
    }

    /** Is this the exchange's desk? */
    public boolean at(GlobalPos where) {
        return desk != null && desk.equals(where);
    }

    /** Opens the exchange at the capital's desk. */
    public void open(GlobalPos at) {
        desk = at;
        lastDay = -1;
        setDirty();
    }

    public static Optional<Metal> metal(Item item) {
        return METALS.stream().filter(m -> m.item() == item).findFirst();
    }

    public Book book(Item item) {
        setDirty();
        return books.computeIfAbsent(item, i -> new Book());
    }

    /** The stock it aims to hold now: more while the sovereign mobilises, more again at war. */
    public static int target(Metal metal, Sovereign.Phase phase) {
        return metal.target() * switch (phase) {
            case PEACE -> 1;
            case MOBILISING -> 2;
            case WAR -> 3;
        };
    }

    int target(MinecraftServer server, Metal metal) {
        return target(metal, Sovereign.get(server).phase());
    }

    public int held(Item item) {
        return held.getOrDefault(item, 0);
    }

    public int atSea(Item item) {
        return atSea.stream().filter(s -> s.item() == item).mapToInt(Shipment::count).sum();
    }

    /** The middle of its prices for a metal, in emeralds: its worth, more the shorter the exchange is. */
    public double middle(MinecraftServer server, Metal metal) {
        int target = target(server, metal);
        double gap = target <= 0 ? 0 : (target - held(metal.item())) / (double) target;
        return Prices.emeraldsEach(metal.item()) * Math.max(0.75, Math.min(1 + PRESSURE, 1 + PRESSURE * gap));
    }

    /** What it pays for a metal, in emeralds each; none if this isn't its desk or its metal. */
    public Optional<Double> bid(MinecraftServer server, GlobalPos at, Item item) {
        return at(at) ? metal(item).map(m -> middle(server, m) * (1 - SPREAD)) : Optional.empty();
    }

    /** What it asks for a metal, in emeralds each. */
    public Optional<Double> ask(MinecraftServer server, GlobalPos at, Item item) {
        return at(at) ? metal(item).map(m -> middle(server, m) * (1 + SPREAD)) : Optional.empty();
    }

    /** What it would buy of each metal now: its target, less what it holds and what's at sea. */
    public Map<Item, Integer> wants(MinecraftServer server, GlobalPos at) {
        Map<Item, Integer> wants = new LinkedHashMap<>();
        if (!at(at)) {
            return wants;
        }
        for (Metal metal : METALS) {
            int want = target(server, metal) - held(metal.item()) - atSea(metal.item());
            if (want > 0) {
                wants.put(metal.item(), want);
            }
        }
        return wants;
    }

    /** What it aims to hold of each metal, less what's at sea: what the capital's caravans should buy toward. */
    public Map<Item, Integer> targets(MinecraftServer server, GlobalPos at) {
        Map<Item, Integer> targets = new LinkedHashMap<>();
        if (at(at)) {
            METALS.forEach(m -> targets.put(m.item(), Math.max(0, target(server, m) - atSea(m.item()))));
        }
        return targets;
    }

    /** A caravan home sold a town {@code count} of a metal at the ask, or bought them at the bid. */
    public void sold(Item item, int count, double emeralds) {
        Book book = book(item);
        book.sold += count;
        book.soldEmeralds += emeralds;
        held.merge(item, -count, Integer::sum);
    }

    public void bought(Item item, int count, double emeralds) {
        Book book = book(item);
        book.bought += count;
        book.boughtEmeralds += emeralds;
        held.merge(item, count, Integer::sum);
    }

    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        MetalExchange exchange = get(server);
        if (exchange.desk == null) {
            return;
        }
        ServerLevel level = server.getLevel(exchange.desk.dimension());
        long now = server.overworld().getGameTime();
        if (level == null || !level.isLoaded(exchange.desk.pos())
                || exchange.lastDay >= 0 && now - exchange.lastDay < 24000) {
            return;
        }
        exchange.lastDay = now;
        exchange.day(level, now);
    }

    /** A day at the port: the ships that have come in are unloaded, and what's short is sent for. */
    public void day(ServerLevel level, long now) {
        MinecraftServer server = level.getServer();
        MerchantStock stock = MerchantStock.at(level, desk.pos());
        List<Shipment> still = new ArrayList<>();
        for (Shipment ship : atSea) {
            int left = ship.lands() > now ? ship.count() : land(stock, ship.item(), ship.count());
            if (left > 0) {
                // At sea still, or landed with no room in the warehouse: the rest waits on the quay for room.
                still.add(ship.lands() > now ? ship : new Shipment(ship.item(), left, now));
            }
        }
        atSea.clear();
        atSea.addAll(still);
        count(stock);
        Warehouse.get(server).recordStocktake(desk, stock.totals(), now);
        double rate = Mint.get(server).rate(Mint.DEFAULT_FACTION, now);
        Territory territory = Territory.get(server);
        for (Metal metal : METALS) {
            int want = Math.min(metal.lot(), target(server, metal) - held(metal.item()) - atSea(metal.item()));
            if (want <= 0) {
                continue;
            }
            double each = Prices.emeraldsEach(metal.item()) * IMPORT_PREMIUM;
            double can = Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk);
            int afford = (int) Math.floor(Math.max(0, can) / each);
            int order = Math.min(want, afford);
            if (order <= 0) {
                continue;
            }
            Treasury.withdraw(level, desk.pos(), stock, order * each, Mint.get(server), now);
            atSea.add(new Shipment(metal.item(), order, now + VOYAGE_TICKS));
            Book book = book(metal.item());
            book.imported += order;
            book.importEmeralds += order * each;
            Enterprise.LOGGER.debug("Metal exchange: sent for {} {} at {} emeralds each", order,
                    BuiltInRegistries.ITEM.getKey(metal.item()).getPath(), each);
        }
        setDirty();
    }

    /** Puts {@code count} of a metal in the warehouse, as far as there's room; returns what didn't fit. */
    private static int land(MerchantStock stock, Item item, int count) {
        int left = count;
        while (left > 0) {
            int piece = Math.min(left, item.getDefaultMaxStackSize());
            ItemStack rest = stock.insert(new ItemStack(item, piece));
            left -= piece - rest.getCount();
            if (!rest.isEmpty()) {
                break;
            }
        }
        return left;
    }

    /** Counts its metal in the capital's warehouse, for its prices. */
    public void count(MerchantStock stock) {
        for (Metal metal : METALS) {
            held.put(metal.item(), stock.count(new ItemCost(metal.item(), 1)));
        }
        setDirty();
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        if (desk != null) {
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> tag.put("desk", t));
        }
        tag.putLong("lastDay", lastDay);
        ListTag sea = new ListTag();
        atSea.forEach(s -> {
            CompoundTag t = new CompoundTag();
            t.putString("item", BuiltInRegistries.ITEM.getKey(s.item()).toString());
            t.putInt("count", s.count());
            t.putLong("lands", s.lands());
            sea.add(t);
        });
        tag.put("atSea", sea);
        CompoundTag bookTag = new CompoundTag();
        books.forEach((item, b) -> {
            CompoundTag t = new CompoundTag();
            t.putInt("imported", b.imported);
            t.putDouble("importEmeralds", b.importEmeralds);
            t.putInt("bought", b.bought);
            t.putDouble("boughtEmeralds", b.boughtEmeralds);
            t.putInt("sold", b.sold);
            t.putDouble("soldEmeralds", b.soldEmeralds);
            t.putInt("held", held.getOrDefault(item, 0));
            bookTag.put(BuiltInRegistries.ITEM.getKey(item).toString(), t);
        });
        held.forEach((item, n) -> {
            String key = BuiltInRegistries.ITEM.getKey(item).toString();
            if (!bookTag.contains(key)) {
                CompoundTag t = new CompoundTag();
                t.putInt("held", n);
                bookTag.put(key, t);
            }
        });
        tag.put("books", bookTag);
        return tag;
    }

    public static MetalExchange load(CompoundTag tag, HolderLookup.Provider registries) {
        MetalExchange exchange = new MetalExchange();
        exchange.desk = tag.contains("desk")
                ? GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("desk")).result().orElse(null) : null;
        exchange.lastDay = tag.contains("lastDay") ? tag.getLong("lastDay") : -1;
        for (Tag t : tag.getList("atSea", Tag.TAG_COMPOUND)) {
            CompoundTag s = (CompoundTag) t;
            exchange.atSea.add(new Shipment(BuiltInRegistries.ITEM.get(ResourceLocation.parse(s.getString("item"))),
                    s.getInt("count"), s.getLong("lands")));
        }
        CompoundTag bookTag = tag.getCompound("books");
        for (String key : bookTag.getAllKeys()) {
            Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(key));
            CompoundTag t = bookTag.getCompound(key);
            Book b = new Book();
            b.imported = t.getInt("imported");
            b.importEmeralds = t.getDouble("importEmeralds");
            b.bought = t.getInt("bought");
            b.boughtEmeralds = t.getDouble("boughtEmeralds");
            b.sold = t.getInt("sold");
            b.soldEmeralds = t.getDouble("soldEmeralds");
            exchange.books.put(item, b);
            exchange.held.put(item, t.getInt("held"));
        }
        return exchange;
    }
}
