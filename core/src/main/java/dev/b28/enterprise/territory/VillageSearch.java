package dev.b28.enterprise.territory;

import java.util.Set;
import java.util.TreeSet;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.LongArrayTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * The towns whose country has been looked over for villages, and the villages told to the core (by chunk), saved with
 * the world: a server started again doesn't look again and tell the core of villages it was told of before, or of
 * others round towns the run had looked over (seed 1717: a day-30 point restarted gained three villages and a town the
 * run it was kept from never had). The world's villages don't move.
 */
public final class VillageSearch extends SavedData {
    private static final String NAME = "enterprise_village_search";

    private final Set<Long> looked = new TreeSet<>();
    private final Set<Long> said = new TreeSet<>();

    public static VillageSearch get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(VillageSearch::new, VillageSearch::load, null), NAME);
    }

    /** Whether the town's country is to be looked over now: true the first time, ever. */
    public boolean look(long town) {
        boolean first = looked.add(town);
        if (first) {
            setDirty();
        }
        return first;
    }

    /** Whether the village in this chunk is to be told to the core now: true the first time, ever. */
    public boolean say(long chunk) {
        boolean first = said.add(chunk);
        if (first) {
            setDirty();
        }
        return first;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        tag.put("looked", new LongArrayTag(looked.stream().mapToLong(Long::longValue).toArray()));
        tag.put("said", new LongArrayTag(said.stream().mapToLong(Long::longValue).toArray()));
        return tag;
    }

    public static VillageSearch load(CompoundTag tag, HolderLookup.Provider registries) {
        VillageSearch s = new VillageSearch();
        for (long t : tag.getLongArray("looked")) {
            s.looked.add(t);
        }
        for (long c : tag.getLongArray("said")) {
            s.said.add(c);
        }
        return s;
    }
}
