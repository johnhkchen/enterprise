package dev.b28.enterprise.merchant;

import java.util.HashMap;
import java.util.Map;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Each Merchant's chest as the town's warehouse, run on pars. Each locker session records what its switched-on
 * trades could take in a day (price times uses per restock; vanilla trades about once a day, the restock after a
 * busy one being inflated). Par is {@link #PAR_DAYS} days of that capacity across the town's lockers, at least
 * {@link #MIN_PAR} for a good a switched-on trade takes. Capacity, not past use: a town short of stock would
 * otherwise learn a small par and stay short. Below par the Merchant buys; above par is surplus it will sell. It also
 * keeps {@link #FOOD_PAR_POINTS} of food for caravans' road food, and its level for bargaining.
 */
public final class Warehouse extends SavedData {
    private static final String NAME = "enterprise_warehouses";

    public static final double PAR_DAYS = 2;
    public static final int MIN_PAR = 64;
    public static final int FOOD_PAR_POINTS = 320;
    private static final double USE_HALF_LIFE_DAYS = 3;

    private static final class Use {
        double perDay;
        double since;
        long lastTick = -1;
    }

    private final Map<GlobalPos, Map<String, Use>> uses = new HashMap<>();
    // desk -> workstation -> item -> what that locker's switched-on trades can take in a day
    private final Map<GlobalPos, Map<Long, Map<String, Integer>>> capacity = new HashMap<>();
    private final Map<GlobalPos, Integer> levels = new HashMap<>();
    // desk -> item -> count, as of the market's last stocktake (a hub takes one when a caravan leaves)
    private final Map<GlobalPos, Map<String, Integer>> stocktake = new HashMap<>();
    private final Map<GlobalPos, Long> stocktakeTime = new HashMap<>();

    public static Warehouse get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Warehouse::new, Warehouse::load, null), NAME);
    }

    /** A locker managed from this desk used {@code count} of {@code item} in a trade. */
    public void recordUse(GlobalPos desk, Item item, int count, long now) {
        Use use = uses.computeIfAbsent(desk, d -> new HashMap<>())
                .computeIfAbsent(BuiltInRegistries.ITEM.getKey(item).toString(), i -> new Use());
        settle(use, now);
        use.since += count;
        setDirty();
    }

    /** The good's daily use at this market, averaged. */
    public double perDay(GlobalPos desk, Item item, long now) {
        Use use = uses.getOrDefault(desk, Map.of()).get(BuiltInRegistries.ITEM.getKey(item).toString());
        if (use == null) {
            return 0;
        }
        settle(use, now);
        return use.perDay;
    }

    /** A locker session: what this locker's switched-on trades can take in a day, by item (replaces the last). */
    public void recordCapacity(GlobalPos desk, long workstation, Map<Item, Integer> perDay) {
        Map<String, Integer> byId = new HashMap<>();
        perDay.forEach((item, count) -> byId.put(BuiltInRegistries.ITEM.getKey(item).toString(), count));
        capacity.computeIfAbsent(desk, d -> new HashMap<>()).put(workstation, byId);
        setDirty();
    }

    /** What the town's lockers can take of {@code item} in a day. */
    public int capacity(GlobalPos desk, Item item) {
        String id = BuiltInRegistries.ITEM.getKey(item).toString();
        return capacity.getOrDefault(desk, Map.of()).values().stream()
                .mapToInt(byItem -> byItem.getOrDefault(id, 0))
                .sum();
    }

    /**
     * What it costs to stock the town's shelves: {@link #PAR_DAYS} days of what its lockers actually sell (averaged), at
     * local value, in emeralds. The town's trading capital: spend it on anything else and it can't buy the goods that
     * earn its money. What its lockers could sell but don't (nothing comes to sell) ties up nothing.
     */
    public double tradingCapital(GlobalPos desk, long now) {
        double emeralds = 0;
        for (Map.Entry<String, Use> item : uses.getOrDefault(desk, Map.of()).entrySet()) {
            settle(item.getValue(), now);
            emeralds += item.getValue().perDay * PAR_DAYS / dev.b28.enterprise.money.Prices.itemsPerEmerald(item.getKey());
        }
        return emeralds;
    }

    /** How many of {@code item} the warehouse aims to hold. {@code wanted}: a switched-on trade takes it. */
    public int par(GlobalPos desk, Item item, boolean wanted, long now) {
        int days = (int) Math.ceil(capacity(desk, item) * PAR_DAYS);
        return wanted ? Math.max(MIN_PAR, days) : days;
    }

    /** Records what the market holds now, for decisions made while it's unloaded. */
    public void recordStocktake(GlobalPos desk, Map<String, Integer> totals) {
        recordStocktake(desk, totals, -1);
    }

    /** As {@link #recordStocktake(GlobalPos, Map)}, taken at game time {@code now}. */
    public void recordStocktake(GlobalPos desk, Map<String, Integer> totals, long now) {
        stocktake.put(desk, new HashMap<>(totals));
        stocktakeTime.put(desk, now);
        setDirty();
    }

    /** When the market's last stocktake was taken, or -1 if never (or not known). */
    public long stocktakeTime(GlobalPos desk) {
        return stocktakeTime.getOrDefault(desk, -1L);
    }

    /** How many of {@code item} the market held at its last stocktake. */
    public int onHand(GlobalPos desk, Item item) {
        return stocktake.getOrDefault(desk, Map.of()).getOrDefault(BuiltInRegistries.ITEM.getKey(item).toString(), 0);
    }

    public void recordLevel(GlobalPos desk, int level) {
        if (levels.getOrDefault(desk, 0) != level) {
            levels.put(desk, level);
            setDirty();
        }
    }

    /** The Merchant's level when last seen at this desk (Novice if never). */
    public int level(GlobalPos desk) {
        return levels.getOrDefault(desk, 1);
    }

    // Folds the uses since the last update into the daily average, by whole elapsed time.
    private static void settle(Use use, long now) {
        if (use.lastTick < 0) {
            use.lastTick = now;
            return;
        }
        double days = (now - use.lastTick) / 24000.0;
        if (days < 0.25) {
            return;
        }
        double weight = 1 - Math.pow(0.5, days / USE_HALF_LIFE_DAYS);
        use.perDay += (use.since / days - use.perDay) * weight;
        use.since = 0;
        use.lastTick = now;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        uses.forEach((desk, items) -> {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> entry.put("desk", t));
            entry.putInt("level", levels.getOrDefault(desk, 1));
            CompoundTag goods = new CompoundTag();
            items.forEach((id, use) -> {
                CompoundTag u = new CompoundTag();
                u.putDouble("perDay", use.perDay);
                u.putDouble("since", use.since);
                u.putLong("lastTick", use.lastTick);
                goods.put(id, u);
            });
            entry.put("uses", goods);
            list.add(entry);
        });
        ListTag capacities = new ListTag();
        capacity.forEach((desk, lockers) -> lockers.forEach((workstation, byItem) -> {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> entry.put("desk", t));
            entry.putLong("workstation", workstation);
            CompoundTag items = new CompoundTag();
            byItem.forEach(items::putInt);
            entry.put("perDay", items);
            capacities.add(entry);
        }));
        tag.put("capacity", capacities);
        ListTag takes = new ListTag();
        stocktake.forEach((desk, totals) -> {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> entry.put("desk", t));
            CompoundTag items = new CompoundTag();
            totals.forEach(items::putInt);
            entry.put("items", items);
            entry.putLong("time", stocktakeTime.getOrDefault(desk, -1L));
            takes.add(entry);
        });
        tag.put("stocktake", takes);
        levels.forEach((desk, level) -> {
            if (!uses.containsKey(desk)) {
                CompoundTag entry = new CompoundTag();
                GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> entry.put("desk", t));
                entry.putInt("level", level);
                list.add(entry);
            }
        });
        tag.put("warehouses", list);
        return tag;
    }

    public static Warehouse load(CompoundTag tag, HolderLookup.Provider registries) {
        Warehouse warehouse = new Warehouse();
        for (Tag t : tag.getList("warehouses", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("desk")).ifSuccess(desk -> {
                warehouse.levels.put(desk, entry.getInt("level"));
                CompoundTag goods = entry.getCompound("uses");
                for (String id : goods.getAllKeys()) {
                    CompoundTag u = goods.getCompound(id);
                    Use use = new Use();
                    use.perDay = u.getDouble("perDay");
                    use.since = u.getDouble("since");
                    use.lastTick = u.getLong("lastTick");
                    warehouse.uses.computeIfAbsent(desk, d -> new HashMap<>()).put(id, use);
                }
            });
        }
        for (Tag t : tag.getList("capacity", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("desk")).ifSuccess(desk -> {
                Map<String, Integer> byItem = new HashMap<>();
                CompoundTag items = entry.getCompound("perDay");
                items.getAllKeys().forEach(id -> byItem.put(id, items.getInt(id)));
                warehouse.capacity.computeIfAbsent(desk, d -> new HashMap<>()).put(entry.getLong("workstation"), byItem);
            });
        }
        for (Tag t : tag.getList("stocktake", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("desk")).ifSuccess(desk -> {
                Map<String, Integer> totals = new HashMap<>();
                CompoundTag items = entry.getCompound("items");
                items.getAllKeys().forEach(id -> totals.put(id, items.getInt(id)));
                warehouse.stocktake.put(desk, totals);
                warehouse.stocktakeTime.put(desk, entry.contains("time") ? entry.getLong("time") : -1L);
            });
        }
        return warehouse;
    }
}
