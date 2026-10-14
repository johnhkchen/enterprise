package dev.b28.enterprise.service;

import java.util.ArrayDeque;
import java.util.Deque;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Services done, on the ledger: when each was last done for each town (so a town isn't charted every day), how many
 * times each worker's town has done each for each town and what it was paid, and the latest few, for the readout.
 */
public final class ServiceLedger extends SavedData {
    private static final String NAME = "enterprise_services";
    private static final int RECENT = 40;

    /** Who did what for whom. */
    public record Key(String service, GlobalPos from, GlobalPos at) {}

    /** How often, and for how much in all. */
    public record Tally(int times, double emeralds) {}

    public record Entry(long time, Key key, double emeralds) {}

    private final Map<Key, Tally> tallies = new LinkedHashMap<>();
    private final Map<Key, Long> last = new HashMap<>(); // by service and town served (from = at)
    private final Deque<Entry> recent = new ArrayDeque<>();

    public static ServiceLedger get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(ServiceLedger::new, ServiceLedger::load, null), NAME);
    }

    public void record(String service, GlobalPos from, GlobalPos at, double emeralds, long now) {
        Key key = new Key(service, from, at);
        tallies.merge(key, new Tally(1, emeralds), (a, b) -> new Tally(a.times() + b.times(), a.emeralds() + b.emeralds()));
        last.put(new Key(service, at, at), now);
        recent.addFirst(new Entry(now, key, emeralds));
        while (recent.size() > RECENT) {
            recent.removeLast();
        }
        setDirty();
    }

    /** When {@code service} was last done for the town at {@code at}, by anyone; -1 if never. */
    public long last(String service, GlobalPos at) {
        return last.getOrDefault(new Key(service, at, at), -1L);
    }

    /** Tallies of services this desk's town did or had done. */
    public Map<Key, Tally> tallies(GlobalPos desk) {
        Map<Key, Tally> mine = new LinkedHashMap<>();
        tallies.forEach((key, tally) -> {
            if (key.from().equals(desk) || key.at().equals(desk)) {
                mine.put(key, tally);
            }
        });
        return mine;
    }

    public List<Entry> recent(GlobalPos desk) {
        return recent.stream().filter(e -> e.key().from().equals(desk) || e.key().at().equals(desk)).toList();
    }

    public int times(String service, GlobalPos from, GlobalPos at) {
        return tallies.getOrDefault(new Key(service, from, at), new Tally(0, 0)).times();
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        tallies.forEach((key, tally) -> {
            CompoundTag entry = key(key);
            entry.putInt("times", tally.times());
            entry.putDouble("emeralds", tally.emeralds());
            list.add(entry);
        });
        tag.put("tallies", list);
        ListTag lasts = new ListTag();
        last.forEach((key, time) -> {
            CompoundTag entry = key(key);
            entry.putLong("time", time);
            lasts.add(entry);
        });
        tag.put("last", lasts);
        ListTag recents = new ListTag();
        recent.forEach(e -> {
            CompoundTag entry = key(e.key());
            entry.putLong("time", e.time());
            entry.putDouble("emeralds", e.emeralds());
            recents.add(entry);
        });
        tag.put("recent", recents);
        return tag;
    }

    private static CompoundTag key(Key key) {
        CompoundTag tag = new CompoundTag();
        tag.putString("service", key.service());
        GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, key.from()).ifSuccess(t -> tag.put("from", t));
        GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, key.at()).ifSuccess(t -> tag.put("at", t));
        return tag;
    }

    private static Key key(CompoundTag tag) {
        return new Key(tag.getString("service"),
                GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("from")).getOrThrow(),
                GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("at")).getOrThrow());
    }

    public static ServiceLedger load(CompoundTag tag, HolderLookup.Provider registries) {
        ServiceLedger ledger = new ServiceLedger();
        try {
            for (Tag t : tag.getList("tallies", Tag.TAG_COMPOUND)) {
                CompoundTag entry = (CompoundTag) t;
                ledger.tallies.put(key(entry), new Tally(entry.getInt("times"), entry.getDouble("emeralds")));
            }
            for (Tag t : tag.getList("last", Tag.TAG_COMPOUND)) {
                ledger.last.put(key((CompoundTag) t), ((CompoundTag) t).getLong("time"));
            }
            for (Tag t : tag.getList("recent", Tag.TAG_COMPOUND)) {
                CompoundTag entry = (CompoundTag) t;
                ledger.recent.addLast(new Entry(entry.getLong("time"), key(entry), entry.getDouble("emeralds")));
            }
        } catch (IllegalStateException e) {
            dev.b28.enterprise.Enterprise.LOGGER.warn("Skipping service records that can't be read: {}", e.getMessage());
        }
        return ledger;
    }
}
