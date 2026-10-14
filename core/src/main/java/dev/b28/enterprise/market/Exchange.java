package dev.b28.enterprise.market;

import java.util.ArrayDeque;
import java.util.Deque;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * What each master Merchant knows of the shops around its desk: the players' standing orders it hasn't filled yet
 * (which its region's caravans then bring it), the commissions it's working (enchanted goods to order), the goods
 * its town wants that no market it knows has (signals, posted at a premium), and what it has traded with the shops.
 */
public final class Exchange extends SavedData {
    private static final String NAME = "enterprise_exchange";
    private static final int RECENT = 20;

    /** Lots still wanted by a player's buying shop: so many items at so many spurs each. */
    public record Order(Item item, int outstanding, double spursEach, BlockPos shop) {}

    /** A good the town wants and no market it knows has: so many, and what it will pay a player for each. */
    public record Signal(Item item, int count, int spursEach, String why) {}

    /** An enchanted good made to a player's order: its last step, and (once made) the good, waiting to be handed over. */
    public static final class Job {
        String step = "started";
        int steps;
        ItemStack ready = ItemStack.EMPTY;

        public String step() {
            return step;
        }

        public int steps() {
            return steps;
        }

        public ItemStack ready() {
            return ready;
        }
    }

    /** What the Merchant has traded with the shops, in all. */
    public static final class Tally {
        int lotsSold;
        long spursFromSales;
        int lotsBought;
        long spursOnPurchases;
        int commissions;
        long spursFromOwnShops;

        public int lotsSold() {
            return lotsSold;
        }

        public long spursFromSales() {
            return spursFromSales;
        }

        public int lotsBought() {
            return lotsBought;
        }

        public long spursOnPurchases() {
            return spursOnPurchases;
        }

        public int commissions() {
            return commissions;
        }

        public long spursFromOwnShops() {
            return spursFromOwnShops;
        }
    }

    private final Map<GlobalPos, Map<Item, Order>> orders = new LinkedHashMap<>();
    private final Map<GlobalPos, List<Signal>> signals = new LinkedHashMap<>();
    private final Map<GlobalPos, Map<BlockPos, Job>> jobs = new LinkedHashMap<>();
    private final Map<GlobalPos, Tally> tallies = new LinkedHashMap<>();
    private final Map<GlobalPos, Deque<String>> recent = new LinkedHashMap<>();
    private final Map<GlobalPos, java.util.Set<String>> procured = new LinkedHashMap<>(); // "workstation|trade key"

    /** A locker trade the Merchant switched on (or off) to buy what's wanted. */
    void procured(GlobalPos desk, BlockPos workstation, String key, boolean on) {
        var keys = procured.computeIfAbsent(desk, d -> new java.util.LinkedHashSet<>());
        if (on) {
            keys.add(workstation.asLong() + "|" + key);
        } else {
            keys.remove(workstation.asLong() + "|" + key);
        }
        setDirty();
    }

    public boolean isProcured(GlobalPos desk, BlockPos workstation, String key) {
        return procured.getOrDefault(desk, java.util.Set.of()).contains(workstation.asLong() + "|" + key);
    }

    public static Exchange get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Exchange::new, Exchange::load, null), NAME);
    }

    /** The desk's open orders, by item, as of its Merchant's last look. */
    public Map<Item, Order> orders(GlobalPos desk) {
        return Map.copyOf(orders.getOrDefault(desk, Map.of()));
    }

    /** Items still wanted by the shops round this desk: what caravans should bring it. */
    public Map<Item, Integer> wanted(GlobalPos desk) {
        Map<Item, Integer> wanted = new LinkedHashMap<>();
        orders.getOrDefault(desk, Map.of()).forEach((item, order) -> wanted.put(item, order.outstanding()));
        return wanted;
    }

    public void setOrdersForTest(GlobalPos desk, Map<Item, Order> open) {
        setOrders(desk, open);
    }

    void setOrders(GlobalPos desk, Map<Item, Order> open) {
        orders.put(desk, new LinkedHashMap<>(open));
        setDirty();
    }

    public List<Signal> signals(GlobalPos desk) {
        return List.copyOf(signals.getOrDefault(desk, List.of()));
    }

    void setSignals(GlobalPos desk, List<Signal> posted) {
        signals.put(desk, List.copyOf(posted));
        setDirty();
    }

    public Map<BlockPos, Job> jobs(GlobalPos desk) {
        return Map.copyOf(jobs.getOrDefault(desk, Map.of()));
    }

    Job job(GlobalPos desk, BlockPos shop) {
        setDirty();
        return jobs.computeIfAbsent(desk, d -> new LinkedHashMap<>()).computeIfAbsent(shop, s -> new Job());
    }

    void endJob(GlobalPos desk, BlockPos shop) {
        jobs.getOrDefault(desk, new LinkedHashMap<>()).remove(shop);
        setDirty();
    }

    public Tally tally(GlobalPos desk) {
        setDirty();
        return tallies.computeIfAbsent(desk, d -> new Tally());
    }

    public List<String> recent(GlobalPos desk) {
        return List.copyOf(recent.getOrDefault(desk, new ArrayDeque<>()));
    }

    void note(GlobalPos desk, long now, String line) {
        Deque<String> lines = recent.computeIfAbsent(desk, d -> new ArrayDeque<>());
        lines.addFirst("day %d: %s".formatted(now / 24000, line));
        while (lines.size() > RECENT) {
            lines.removeLast();
        }
        setDirty();
    }

    private static String id(Item item) {
        return BuiltInRegistries.ITEM.getKey(item).toString();
    }

    private static Item item(String id) {
        return BuiltInRegistries.ITEM.get(ResourceLocation.parse(id));
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag desks = new ListTag();
        java.util.Set<GlobalPos> all = new java.util.LinkedHashSet<>(orders.keySet());
        all.addAll(signals.keySet());
        all.addAll(jobs.keySet());
        all.addAll(tallies.keySet());
        all.addAll(recent.keySet());
        all.addAll(procured.keySet());
        for (GlobalPos desk : all) {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> entry.put("desk", t));
            ListTag orderList = new ListTag();
            orders.getOrDefault(desk, Map.of()).values().forEach(o -> {
                CompoundTag t = new CompoundTag();
                t.putString("item", id(o.item()));
                t.putInt("outstanding", o.outstanding());
                t.putDouble("spursEach", o.spursEach());
                t.putLong("shop", o.shop().asLong());
                orderList.add(t);
            });
            entry.put("orders", orderList);
            ListTag signalList = new ListTag();
            signals.getOrDefault(desk, List.of()).forEach(s -> {
                CompoundTag t = new CompoundTag();
                t.putString("item", id(s.item()));
                t.putInt("count", s.count());
                t.putInt("spursEach", s.spursEach());
                t.putString("why", s.why());
                signalList.add(t);
            });
            entry.put("signals", signalList);
            ListTag jobList = new ListTag();
            jobs.getOrDefault(desk, Map.of()).forEach((shop, job) -> {
                CompoundTag t = new CompoundTag();
                t.putLong("shop", shop.asLong());
                t.putString("step", job.step);
                t.putInt("steps", job.steps);
                if (!job.ready.isEmpty()) {
                    t.put("ready", job.ready.save(registries));
                }
                jobList.add(t);
            });
            entry.put("jobs", jobList);
            Tally tally = tallies.get(desk);
            if (tally != null) {
                CompoundTag t = new CompoundTag();
                t.putInt("lotsSold", tally.lotsSold);
                t.putLong("spursFromSales", tally.spursFromSales);
                t.putInt("lotsBought", tally.lotsBought);
                t.putLong("spursOnPurchases", tally.spursOnPurchases);
                t.putInt("commissions", tally.commissions);
                t.putLong("spursFromOwnShops", tally.spursFromOwnShops);
                entry.put("tally", t);
            }
            ListTag lines = new ListTag();
            recent.getOrDefault(desk, new ArrayDeque<>()).forEach(line -> lines.add(net.minecraft.nbt.StringTag.valueOf(line)));
            entry.put("recent", lines);
            ListTag keys = new ListTag();
            procured.getOrDefault(desk, java.util.Set.of()).forEach(k -> keys.add(net.minecraft.nbt.StringTag.valueOf(k)));
            entry.put("procured", keys);
            desks.add(entry);
        }
        tag.put("desks", desks);
        return tag;
    }

    public static Exchange load(CompoundTag tag, HolderLookup.Provider registries) {
        Exchange exchange = new Exchange();
        for (Tag t : tag.getList("desks", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("desk")).ifSuccess(desk -> {
                Map<Item, Order> open = new LinkedHashMap<>();
                for (Tag o : entry.getList("orders", Tag.TAG_COMPOUND)) {
                    CompoundTag order = (CompoundTag) o;
                    Item item = item(order.getString("item"));
                    open.put(item, new Order(item, order.getInt("outstanding"), order.getDouble("spursEach"),
                            BlockPos.of(order.getLong("shop"))));
                }
                exchange.orders.put(desk, open);
                List<Signal> posted = new java.util.ArrayList<>();
                for (Tag s : entry.getList("signals", Tag.TAG_COMPOUND)) {
                    CompoundTag signal = (CompoundTag) s;
                    posted.add(new Signal(item(signal.getString("item")), signal.getInt("count"),
                            signal.getInt("spursEach"), signal.getString("why")));
                }
                exchange.signals.put(desk, posted);
                Map<BlockPos, Job> work = new LinkedHashMap<>();
                for (Tag j : entry.getList("jobs", Tag.TAG_COMPOUND)) {
                    CompoundTag saved = (CompoundTag) j;
                    Job job = new Job();
                    job.step = saved.getString("step");
                    job.steps = saved.getInt("steps");
                    job.ready = saved.contains("ready")
                            ? ItemStack.parse(registries, saved.get("ready")).orElse(ItemStack.EMPTY) : ItemStack.EMPTY;
                    work.put(BlockPos.of(saved.getLong("shop")), job);
                }
                exchange.jobs.put(desk, work);
                if (entry.contains("tally")) {
                    CompoundTag saved = entry.getCompound("tally");
                    Tally tally = new Tally();
                    tally.lotsSold = saved.getInt("lotsSold");
                    tally.spursFromSales = saved.getLong("spursFromSales");
                    tally.lotsBought = saved.getInt("lotsBought");
                    tally.spursOnPurchases = saved.getLong("spursOnPurchases");
                    tally.commissions = saved.getInt("commissions");
                    tally.spursFromOwnShops = saved.getLong("spursFromOwnShops");
                    exchange.tallies.put(desk, tally);
                }
                Deque<String> lines = new ArrayDeque<>();
                entry.getList("recent", Tag.TAG_STRING).forEach(l -> lines.addLast(l.getAsString()));
                exchange.recent.put(desk, lines);
                java.util.Set<String> keys = new java.util.LinkedHashSet<>();
                entry.getList("procured", Tag.TAG_STRING).forEach(k -> keys.add(k.getAsString()));
                exchange.procured.put(desk, keys);
            });
        }
        return exchange;
    }
}
