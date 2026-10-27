package dev.b28.enterprise.territory;

import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Each town's kingdom (`docs/assets/README.md`, C2): the architectural language its buildings are chosen in
 * ({@link Kits}). Lordaeron for every town until the sim assigns kingdoms (the first set: John's "hybrid of WoW human
 * kingdoms and real life cultures", piloted in Lordaeron); set by command for a world (`/enterprise kingdom`).
 *
 * <p>And a town's tier, from its people: what size of building it builds.
 */
public final class Kingdoms extends SavedData {
    private static final String NAME = "enterprise_kingdoms";
    /** The kingdom of a town none was set for. */
    public static final String DEFAULT = "lordaeron";

    private final Map<Long, String> kingdoms = new LinkedHashMap<>();

    public static Kingdoms get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Kingdoms::new, Kingdoms::load, null), NAME);
    }

    public String of(long town) {
        return kingdoms.getOrDefault(town, DEFAULT);
    }

    public void set(long town, String kingdom) {
        kingdoms.put(town, kingdom);
        setDirty();
    }

    /** A town's tier by its villagers: hamlet, village, town, city, capital (the request's tiers). */
    public static String tier(double villagers) {
        return villagers < 8 ? "hamlet" : villagers < 20 ? "village" : villagers < 50 ? "town" : villagers < 120 ? "city"
                : "capital";
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        CompoundTag towns = new CompoundTag();
        kingdoms.forEach((town, k) -> towns.putString(Long.toString(town), k));
        tag.put("towns", towns);
        return tag;
    }

    public static Kingdoms load(CompoundTag tag, HolderLookup.Provider registries) {
        Kingdoms out = new Kingdoms();
        CompoundTag towns = tag.getCompound("towns");
        for (String key : towns.getAllKeys()) {
            out.kingdoms.put(Long.parseLong(key), towns.getString(key));
        }
        return out;
    }
}
