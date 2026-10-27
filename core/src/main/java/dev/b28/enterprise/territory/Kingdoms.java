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
    /** Tiers set by hand (a test town shown at a tier it hasn't grown to): `/enterprise tier`. */
    private final Map<Long, String> tiers = new LinkedHashMap<>();

    /** On the game-test server, towns have no kingdom unless a test gives them one: their arenas are 7 x 7. */
    private boolean gameTests;

    public static Kingdoms get(MinecraftServer server) {
        Kingdoms k = server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Kingdoms::new, Kingdoms::load, null), NAME);
        k.gameTests = server instanceof net.minecraft.gametest.framework.GameTestServer;
        return k;
    }

    public String of(long town) {
        return kingdoms.getOrDefault(town, gameTests ? "none" : DEFAULT);
    }

    public void set(long town, String kingdom) {
        kingdoms.put(town, kingdom);
        setDirty();
    }

    /** A town's tier: the one set for it by hand, else by its villagers ({@link #tier(double)}). */
    public String tierOf(long town, double villagers) {
        return tiers.getOrDefault(town, tier(villagers));
    }

    public void setTier(long town, String tier) {
        if (tier.equals("auto")) {
            tiers.remove(town);
        } else {
            tiers.put(town, tier);
        }
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
        CompoundTag t = new CompoundTag();
        tiers.forEach((town, k) -> t.putString(Long.toString(town), k));
        tag.put("tiers", t);
        return tag;
    }

    public static Kingdoms load(CompoundTag tag, HolderLookup.Provider registries) {
        Kingdoms out = new Kingdoms();
        CompoundTag towns = tag.getCompound("towns");
        for (String key : towns.getAllKeys()) {
            out.kingdoms.put(Long.parseLong(key), towns.getString(key));
        }
        CompoundTag t = tag.getCompound("tiers");
        for (String key : t.getAllKeys()) {
            out.tiers.put(Long.parseLong(key), t.getString(key));
        }
        return out;
    }
}
