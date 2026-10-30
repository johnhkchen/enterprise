package dev.b28.enterprise.territory;

import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Each town's kingdom (`docs/assets/README.md`, C2): the architectural language its buildings are chosen in
 * ({@link Kits}), and its clan where its race has clans: drawn from the land at its first survey ({@link Settle}),
 * set by command for a world (`/enterprise kingdom`); Lordaeron for a town none was drawn for.
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

    private final Map<Long, String> clans = new LinkedHashMap<>();

    /** Has the town a kingdom of its own (drawn or set), not the default? */
    public boolean has(long town) {
        return kingdoms.containsKey(town);
    }

    /** The town's clan, empty where its race has none. */
    public String clanOf(long town) {
        return clans.getOrDefault(town, "");
    }

    /**
     * The town's people drawn from the land at its centre if none were yet (thread 0015): its kingdom and clan. Not on
     * the game-test server, whose towns have none unless a test gives them one. True if drawn now.
     */
    public boolean settle(net.minecraft.server.level.ServerLevel level, long town, net.minecraft.core.BlockPos centre) {
        if (gameTests || has(town)) {
            return false;
        }
        Settle.Who who = Settle.settle(level, centre, Zoning.mix(level.getSeed(), town));
        kingdoms.put(town, who.style());
        if (!who.clan().isEmpty()) {
            clans.put(town, who.clan());
        }
        setDirty();
        dev.b28.enterprise.Enterprise.LOGGER.info("Town {}'s people drawn from the land ({}): {}{}", town, who.biome(),
                who.style(), who.clan().isEmpty() ? "" : " of the " + who.clan());
        return true;
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
        CompoundTag c = new CompoundTag();
        clans.forEach((town, k) -> c.putString(Long.toString(town), k));
        tag.put("clans", c);
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
        CompoundTag c = tag.getCompound("clans");
        for (String key : c.getAllKeys()) {
            out.clans.put(Long.parseLong(key), c.getString(key));
        }
        return out;
    }
}
