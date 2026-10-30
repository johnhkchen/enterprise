package dev.b28.enterprise.territory;

import java.util.HashMap;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Each town's waterfront as its harbour laid it and it has grown since ({@link Harbour#grow}): its head (the shore cell
 * its first jetty leaves from) and its way out over the water, the water's level, sea or river, the quay cells along
 * its shore laid so far (nearest the head first), the stretches grown, the tier it was last grown for, and the
 * stretches owed (a berth paid for or a tier risen to, laid when its ground is loaded). Saved with the world.
 */
public final class Waterfronts extends SavedData {
    private static final String NAME = "enterprise_waterfronts";

    public record Front(BlockPos head, Direction out, int w, boolean sea, int shore, int stretches, int rank, int owed) {
        /** Grown a stretch of {@code cells} quay cells. */
        public Front grown(int cells) {
            return new Front(head, out, w, sea, shore + cells, stretches + 1, rank, Math.max(0, owed - 1));
        }

        /** Owed one more stretch, for a tier {@code rank} (its own if it's a berth). */
        public Front owe(int rank) {
            return new Front(head, out, w, sea, shore, stretches, Math.max(this.rank, rank), owed + 1);
        }
    }

    /** The towns owed a stretch. */
    public java.util.List<Long> owed() {
        return fronts.entrySet().stream().filter(e -> e.getValue().owed() > 0).map(Map.Entry::getKey).sorted().toList();
    }

    private final Map<Long, Front> fronts = new HashMap<>();

    public static Waterfronts get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Waterfronts::new, Waterfronts::load, null), NAME);
    }

    public Front of(long town) {
        return fronts.get(town);
    }

    public void put(long town, Front front) {
        fronts.put(town, front);
        setDirty();
    }

    /** A tier's rank, as the harbour builds by it: hamlet 0, village 1, a town and up 2. */
    public static int rank(String tier) {
        return switch (tier) {
            case "hamlet" -> 0;
            case "village" -> 1;
            default -> 2;
        };
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        CompoundTag all = new CompoundTag();
        fronts.forEach((town, f) -> {
            CompoundTag e = new CompoundTag();
            e.putLong("head", f.head().asLong());
            e.putInt("out", f.out().get3DDataValue());
            e.putInt("w", f.w());
            e.putBoolean("sea", f.sea());
            e.putInt("shore", f.shore());
            e.putInt("stretches", f.stretches());
            e.putInt("rank", f.rank());
            e.putInt("owed", f.owed());
            all.put(Long.toString(town), e);
        });
        tag.put("fronts", all);
        return tag;
    }

    public static Waterfronts load(CompoundTag tag, HolderLookup.Provider registries) {
        Waterfronts w = new Waterfronts();
        CompoundTag all = tag.getCompound("fronts");
        for (String key : all.getAllKeys()) {
            CompoundTag e = all.getCompound(key);
            w.fronts.put(Long.parseLong(key), new Front(BlockPos.of(e.getLong("head")), Direction.from3DDataValue(e.getInt("out")),
                    e.getInt("w"), e.getBoolean("sea"), e.getInt("shore"), e.getInt("stretches"), e.getInt("rank"),
                    e.getInt("owed")));
        }
        return w;
    }
}
