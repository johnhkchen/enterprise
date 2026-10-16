package dev.b28.enterprise.merchant;

import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.StringTag;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Every Merchant's settings, keyed by its desk: which trades each locker may run. Lives in world save data, so it
 * holds while the village is unloaded.
 */
public final class MerchantBook extends SavedData {
    private static final String NAME = "enterprise_merchants";

    // desk -> workstation -> enabled trade keys
    private final Map<GlobalPos, Map<BlockPos, Set<String>>> enabled = new HashMap<>();
    // desk -> game time of its last round (see Rounds)
    private final Map<GlobalPos, Long> lastRound = new HashMap<>();
    // desk -> "workstation|trade key" the Merchant switched on to train a villager (see Apprentices)
    private final Map<GlobalPos, Set<String>> training = new HashMap<>();
    // desk -> the lockers it has taken on as clients, in the order taken (see Lockers#review)
    private final Map<GlobalPos, java.util.List<BlockPos>> clients = new HashMap<>();
    // desk -> workstation -> what its villager's trades are worth to the town a restock, in emeralds, when last seen
    private final Map<GlobalPos, Map<BlockPos, Double>> worth = new HashMap<>();

    /** The lockers the Merchant has taken on as clients, in the order taken. */
    public java.util.List<BlockPos> clients(GlobalPos desk) {
        return java.util.List.copyOf(clients.getOrDefault(desk, java.util.List.of()));
    }

    public void setClients(GlobalPos desk, java.util.List<BlockPos> lockers) {
        if (!lockers.equals(clients.getOrDefault(desk, java.util.List.of()))) {
            clients.put(desk, new java.util.ArrayList<>(lockers));
            setDirty();
        }
    }

    // desk -> workstation -> what its trades did for the town since the last review, in emeralds (not saved)
    private final Map<GlobalPos, Map<BlockPos, Double>> earned = new HashMap<>();

    // desk -> workstation -> game time it was taken on as a client by a swap (not saved: an old client after a restart)
    private final Map<GlobalPos, Map<BlockPos, Long>> takenOn = new HashMap<>();

    public long takenOn(GlobalPos desk, BlockPos workstation) {
        return takenOn.getOrDefault(desk, Map.of()).getOrDefault(workstation, Long.MIN_VALUE / 2);
    }

    public void setTakenOn(GlobalPos desk, BlockPos workstation, long gameTime) {
        takenOn.computeIfAbsent(desk, d -> new HashMap<>()).put(workstation, gameTime);
    }

    public void addEarned(GlobalPos desk, BlockPos workstation, double emeralds) {
        if (emeralds > 0) {
            earned.computeIfAbsent(desk, d -> new HashMap<>()).merge(workstation, emeralds, Double::sum);
        }
    }

    /** What the desk's lockers did for the town since the last review, which this clears. */
    public Map<BlockPos, Double> takeEarned(GlobalPos desk) {
        Map<BlockPos, Double> taken = earned.remove(desk);
        return taken == null ? Map.of() : taken;
    }

    /** What a locker's villager's trades were worth to the town a restock, in emeralds, when last seen (0 unseen). */
    public double worth(GlobalPos desk, BlockPos workstation) {
        return worth.getOrDefault(desk, Map.of()).getOrDefault(workstation, 0.0);
    }

    public void setWorth(GlobalPos desk, BlockPos workstation, double emeralds) {
        worth.computeIfAbsent(desk, d -> new HashMap<>()).put(workstation, emeralds);
        setDirty();
    }

    public static MerchantBook get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(MerchantBook::new, MerchantBook::load, null), NAME);
    }

    public boolean isEnabled(GlobalPos desk, BlockPos workstation, String tradeKey) {
        return enabledTrades(desk, workstation).contains(tradeKey);
    }

    public Set<String> enabledTrades(GlobalPos desk, BlockPos workstation) {
        return enabled.getOrDefault(desk, Map.of()).getOrDefault(workstation, Set.of());
    }

    /** Every desk with at least one trade switched on: the markets the ledger keeps running. */
    public Set<GlobalPos> desks() {
        return Set.copyOf(enabled.keySet());
    }

    /** Workstations with trades switched on for this desk. */
    public Set<BlockPos> workstations(GlobalPos desk) {
        return Set.copyOf(enabled.getOrDefault(desk, Map.of()).keySet());
    }

    public long lastRound(GlobalPos desk) {
        return lastRound.getOrDefault(desk, 0L);
    }

    public void setLastRound(GlobalPos desk, long gameTime) {
        lastRound.put(desk, gameTime);
        setDirty();
    }

    /** A trade switched on (or off) to pay a smith in training for its work. */
    public void training(GlobalPos desk, BlockPos workstation, String key, boolean on) {
        Set<String> keys = training.computeIfAbsent(desk, d -> new HashSet<>());
        if (on ? keys.add(workstation.asLong() + "|" + key) : keys.remove(workstation.asLong() + "|" + key)) {
            setDirty();
        }
    }

    public boolean isTraining(GlobalPos desk, BlockPos workstation, String key) {
        return training.getOrDefault(desk, Set.of()).contains(workstation.asLong() + "|" + key);
    }

    public void setEnabled(GlobalPos desk, BlockPos workstation, String tradeKey, boolean on) {
        Map<BlockPos, Set<String>> lockers = enabled.computeIfAbsent(desk, d -> new HashMap<>());
        Set<String> trades = lockers.computeIfAbsent(workstation, w -> new HashSet<>());
        boolean changed = on ? trades.add(tradeKey) : trades.remove(tradeKey);
        if (trades.isEmpty()) {
            lockers.remove(workstation);
        }
        if (lockers.isEmpty()) {
            enabled.remove(desk);
        }
        if (changed) {
            setDirty();
        }
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag desks = new ListTag();
        enabled.forEach((desk, lockers) -> {
            CompoundTag deskTag = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(pos -> deskTag.put("desk", pos));
            ListTag lockerList = new ListTag();
            lockers.forEach((workstation, trades) -> {
                CompoundTag lockerTag = new CompoundTag();
                lockerTag.putLong("workstation", workstation.asLong());
                ListTag tradeList = new ListTag();
                trades.stream().sorted().forEach(key -> tradeList.add(StringTag.valueOf(key)));
                lockerTag.put("trades", tradeList);
                lockerList.add(lockerTag);
            });
            deskTag.put("lockers", lockerList);
            deskTag.putLong("lastRound", lastRound.getOrDefault(desk, 0L));
            desks.add(deskTag);
        });
        tag.put("desks", desks);
        ListTag trained = new ListTag();
        training.forEach((desk, keys) -> {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(pos -> entry.put("desk", pos));
            ListTag list = new ListTag();
            keys.forEach(k -> list.add(StringTag.valueOf(k)));
            entry.put("training", list);
            trained.add(entry);
        });
        tag.put("trainingTrades", trained);
        ListTag clientList = new ListTag();
        java.util.Set<GlobalPos> known = new HashSet<>(clients.keySet());
        known.addAll(worth.keySet());
        known.forEach(desk -> {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(pos -> entry.put("desk", pos));
            entry.putLongArray("clients", clients.getOrDefault(desk, java.util.List.of()).stream()
                    .mapToLong(BlockPos::asLong).toArray());
            CompoundTag worths = new CompoundTag();
            worth.getOrDefault(desk, Map.of()).forEach((at, w) -> worths.putDouble(Long.toString(at.asLong()), w));
            entry.put("worth", worths);
            clientList.add(entry);
        });
        tag.put("clients", clientList);
        return tag;
    }

    public static MerchantBook load(CompoundTag tag, HolderLookup.Provider registries) {
        MerchantBook book = new MerchantBook();
        for (Tag deskEntry : tag.getList("desks", Tag.TAG_COMPOUND)) {
            CompoundTag deskTag = (CompoundTag) deskEntry;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, deskTag.get("desk")).ifSuccess(desk -> {
                if (deskTag.getLong("lastRound") > 0) {
                    book.lastRound.put(desk, deskTag.getLong("lastRound"));
                }
                for (Tag lockerEntry : deskTag.getList("lockers", Tag.TAG_COMPOUND)) {
                    CompoundTag lockerTag = (CompoundTag) lockerEntry;
                    BlockPos workstation = BlockPos.of(lockerTag.getLong("workstation"));
                    for (Tag trade : lockerTag.getList("trades", Tag.TAG_STRING)) {
                        book.enabled
                                .computeIfAbsent(desk, d -> new HashMap<>())
                                .computeIfAbsent(workstation, w -> new HashSet<>())
                                .add(trade.getAsString());
                    }
                }
            });
        }
        for (Tag t : tag.getList("trainingTrades", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("desk")).ifSuccess(desk ->
                    entry.getList("training", Tag.TAG_STRING).forEach(k ->
                            book.training.computeIfAbsent(desk, d -> new HashSet<>()).add(k.getAsString())));
        }
        for (Tag t : tag.getList("clients", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("desk")).ifSuccess(desk -> {
                java.util.List<BlockPos> list = new java.util.ArrayList<>();
                for (long at : entry.getLongArray("clients")) {
                    list.add(BlockPos.of(at));
                }
                if (!list.isEmpty()) {
                    book.clients.put(desk, list);
                }
                CompoundTag worths = entry.getCompound("worth");
                worths.getAllKeys().forEach(k -> book.worth.computeIfAbsent(desk, d -> new HashMap<>())
                        .put(BlockPos.of(Long.parseLong(k)), worths.getDouble(k)));
            });
        }
        return book;
    }
}
