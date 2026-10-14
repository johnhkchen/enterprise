package dev.b28.enterprise.money;

import java.util.HashMap;
import java.util.Map;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Each faction's floating emerald rate. The rate starts at a base number of spurs per emerald, falls as emeralds are
 * sold to the faction, and recovers as that selling pressure decays. Computed on touch: one pressure number and the
 * tick it was last brought up to date, per faction.
 */
public final class Mint extends SavedData {
    private static final String NAME = "enterprise_mints";

    /** Until factions exist, every Merchant changes money with this one. */
    public static final String DEFAULT_FACTION = "enterprise:commons";

    /** One sprocket per emerald when nobody has been selling. */
    public static final int BASE_SPURS_PER_EMERALD = 16;

    /** Emeralds of recent selling that halve the rate: one full shulker box. */
    public static final double PRESSURE_SCALE = 1728;

    /** Selling pressure halves every three in-game days. */
    public static final long HALF_LIFE_TICKS = 3 * 24000L;

    /**
     * Coin a sovereign mints without emeralds behind it (paying for a war) debases the spur: this many spurs minted
     * doubles the spurs an emerald fetches. It works its way out over {@link #DEBASEMENT_HALF_LIFE_TICKS}.
     */
    public static final double DEBASEMENT_SCALE = BASE_SPURS_PER_EMERALD * PRESSURE_SCALE;
    public static final long DEBASEMENT_HALF_LIFE_TICKS = 30 * 24000L;

    private final Map<String, double[]> factions = new HashMap<>(); // faction -> {pressure, lastTick}
    private final Map<String, double[]> minted = new HashMap<>(); // faction -> {spurs minted, lastTick}
    private final Map<String, Long> mintedInAll = new HashMap<>();

    public static Mint get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Mint::new, Mint::load, null), NAME);
    }

    /** Selling pressure now: recent emeralds sold, decayed by elapsed time. */
    public double pressure(String faction, long now) {
        double[] state = factions.get(faction);
        if (state == null) {
            return 0;
        }
        long elapsed = Math.max(0, now - (long) state[1]);
        return state[0] * Math.pow(0.5, (double) elapsed / HALF_LIFE_TICKS);
    }

    /** Spurs minted without backing, as they stand now (decayed). */
    public double minted(String faction, long now) {
        double[] state = minted.get(faction);
        if (state == null) {
            return 0;
        }
        long elapsed = Math.max(0, now - (long) state[1]);
        return state[0] * Math.pow(0.5, (double) elapsed / DEBASEMENT_HALF_LIFE_TICKS);
    }

    /** Spurs minted in all, ever. */
    public long mintedInAll(String faction) {
        return mintedInAll.getOrDefault(faction, 0L);
    }

    /** How far the spur is debased: 1 when nothing's been minted, 2 when it fetches half as much. */
    public double debasement(String faction, long now) {
        return 1 + minted(faction, now) / DEBASEMENT_SCALE;
    }

    /** A sovereign mints {@code spurs} to pay with: coin with nothing behind it. */
    public void mint(String faction, int spurs, long now) {
        minted.put(faction, new double[] {minted(faction, now) + spurs, now});
        mintedInAll.merge(faction, (long) spurs, Long::sum);
        setDirty();
    }

    /** Spurs per emerald for the next emerald sold. */
    public double rate(String faction, long now) {
        return BASE_SPURS_PER_EMERALD * debasement(faction, now) / (1 + pressure(faction, now) / PRESSURE_SCALE);
    }

    /** Spurs paid for {@code emeralds} sold now, priced along the falling rate. Changes nothing. */
    public int quote(String faction, int emeralds, long now) {
        return (int) Math.floor(spursFor(pressure(faction, now), emeralds) * debasement(faction, now));
    }

    /** Sells {@code emeralds} to the faction: returns the spurs paid and adds the selling pressure. */
    public int sell(String faction, int emeralds, long now) {
        double pressure = pressure(faction, now);
        int spurs = (int) Math.floor(spursFor(pressure, emeralds) * debasement(faction, now));
        factions.put(faction, new double[] {pressure + emeralds, now});
        setDirty();
        return spurs;
    }

    /** Buying emeralds back costs this much over the rate. */
    public static final double BUY_SPREAD = 0.05;

    /** Spurs it would take to buy {@code emeralds} from the faction now. Changes nothing. */
    public int buyCost(String faction, int emeralds, long now) {
        return (int) Math.ceil(emeralds * rate(faction, now) * (1 + BUY_SPREAD));
    }

    /**
     * Buys {@code emeralds} from the faction (to pay villagers in): returns the spurs it cost. Buying takes back
     * selling pressure, so the spur fetches less; pressure goes no lower than minus half its scale.
     */
    public int buy(String faction, int emeralds, long now) {
        int spurs = buyCost(faction, emeralds, now);
        factions.put(faction, new double[] {Math.max(-PRESSURE_SCALE / 2, pressure(faction, now) - emeralds), now});
        setDirty();
        return spurs;
    }

    // The integral of base / (1 + p / scale) from p to p + n, rounded down.
    private static int spursFor(double pressure, int emeralds) {
        if (emeralds <= 0) {
            return 0;
        }
        double paid = BASE_SPURS_PER_EMERALD * PRESSURE_SCALE
                * Math.log((PRESSURE_SCALE + pressure + emeralds) / (PRESSURE_SCALE + pressure));
        return (int) Math.floor(paid);
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        CompoundTag all = new CompoundTag();
        factions.forEach((faction, state) -> {
            CompoundTag entry = new CompoundTag();
            entry.putDouble("pressure", state[0]);
            entry.putLong("tick", (long) state[1]);
            all.put(faction, entry);
        });
        tag.put("factions", all);
        CompoundTag mints = new CompoundTag();
        minted.forEach((faction, state) -> {
            CompoundTag entry = new CompoundTag();
            entry.putDouble("spurs", state[0]);
            entry.putLong("tick", (long) state[1]);
            entry.putLong("inAll", mintedInAll.getOrDefault(faction, 0L));
            mints.put(faction, entry);
        });
        tag.put("minted", mints);
        return tag;
    }

    public static Mint load(CompoundTag tag, HolderLookup.Provider registries) {
        Mint mint = new Mint();
        CompoundTag all = tag.getCompound("factions");
        for (String faction : all.getAllKeys()) {
            CompoundTag entry = all.getCompound(faction);
            mint.factions.put(faction, new double[] {entry.getDouble("pressure"), entry.getLong("tick")});
        }
        CompoundTag mints = tag.getCompound("minted");
        for (String faction : mints.getAllKeys()) {
            CompoundTag entry = mints.getCompound(faction);
            mint.minted.put(faction, new double[] {entry.getDouble("spurs"), entry.getLong("tick")});
            mint.mintedInAll.put(faction, entry.getLong("inAll"));
        }
        return mint;
    }
}
