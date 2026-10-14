package dev.b28.enterprise.merchant;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;
import net.neoforged.neoforge.items.ItemStackHandler;

/**
 * Markets off the map: places the region trades with that no one builds or visits, such as the front a war is fought
 * at, or a market across the sea. Each sits at a real position (its roads are planned like any other), but its
 * warehouse lives in save data instead of chests, its money in a bank account, and its Merchant is a master who is
 * never anywhere to be seen. Caravans, contracts, bills and the treasury deal with it as with any desk; only what
 * happens there (an army fighting, a far country producing) is its own.
 */
public final class OffMap extends SavedData {
    private static final String NAME = "enterprise_off_map";

    /** Slots in an off-map warehouse: four double chests. */
    public static final int SLOTS = 216;

    /**
     * An off-map market: what it's called, its warehouse, the bank account its money is kept in, and whether it sells
     * what it holds to caravans (a market across the sea does; the front keeps everything for the army).
     */
    public static final class Market {
        final String name;
        final ItemStackHandler stock = new ItemStackHandler(SLOTS);
        UUID account;
        boolean sells;

        Market(String name) {
            this.name = name;
        }

        public String name() {
            return name;
        }

        public Optional<UUID> account() {
            return Optional.ofNullable(account);
        }

        public boolean sells() {
            return sells;
        }
    }

    private final Map<GlobalPos, Market> markets = new LinkedHashMap<>();

    public static OffMap get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(OffMap::new, OffMap::load, null), NAME);
    }

    /** The off-map market at {@code at}, opened if there's none, its money kept in {@code account}. */
    public Market open(GlobalPos at, String name, UUID account, boolean sells) {
        Market market = markets.computeIfAbsent(at, a -> new Market(name));
        market.account = account;
        market.sells = sells;
        setDirty();
        return market;
    }

    public Optional<Market> market(GlobalPos at) {
        return Optional.ofNullable(markets.get(at));
    }

    public boolean is(GlobalPos at) {
        return markets.containsKey(at);
    }

    /** Does the market at {@code at} sell to caravans? Every market on the map does. */
    public boolean sells(GlobalPos at) {
        return market(at).map(Market::sells).orElse(true);
    }

    public Map<GlobalPos, Market> all() {
        return Map.copyOf(markets);
    }

    /** Its warehouse, for {@link MerchantStock}: changes are saved with the world. */
    ItemStackHandler stock(GlobalPos at) {
        setDirty();
        return markets.get(at).stock;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        markets.forEach((at, market) -> {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, at).ifSuccess(t -> entry.put("at", t));
            entry.putString("name", market.name);
            entry.putBoolean("sells", market.sells);
            entry.put("stock", market.stock.serializeNBT(registries));
            if (market.account != null) {
                entry.putUUID("account", market.account);
            }
            list.add(entry);
        });
        tag.put("markets", list);
        return tag;
    }

    public static OffMap load(CompoundTag tag, HolderLookup.Provider registries) {
        OffMap offMap = new OffMap();
        for (Tag t : tag.getList("markets", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("at")).ifSuccess(at -> {
                Market market = new Market(entry.getString("name"));
                market.stock.deserializeNBT(registries, entry.getCompound("stock"));
                market.account = entry.hasUUID("account") ? entry.getUUID("account") : null;
                market.sells = entry.getBoolean("sells");
                offMap.markets.put(at, market);
            });
        }
        return offMap;
    }
}
