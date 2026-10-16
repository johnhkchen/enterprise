package dev.b28.enterprise;

import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;
import net.neoforged.neoforge.event.server.ServerStartedEvent;

/**
 * The settings a study tunes: each a named value with its default, read where it matters, and set by
 * {@code /enterprise dial <name> <value>} (saved with the world, so a fork keeps its settings and lanes from one build
 * can each try another). One catalogue, so a study can list them all.
 */
public final class Dials extends SavedData {
    private static final String NAME = "enterprise_dials";

    /** One setting: its name, default, what it does, and its value now. */
    public static final class Dial {
        public final String name;
        public final double initial;
        public final String does;
        private volatile double value;

        Dial(String name, double initial, String does) {
            this.name = name;
            this.initial = initial;
            this.does = does;
            this.value = initial;
        }

        public double get() {
            return value;
        }

        public int getInt() {
            return (int) Math.round(value);
        }

        public long ticks() {
            return Math.round(value * 24000);
        }
    }

    private static final Map<String, Dial> ALL = new LinkedHashMap<>();

    private static Dial dial(String name, double initial, String does) {
        Dial dial = new Dial(name, initial, does);
        ALL.put(name, dial);
        return dial;
    }

    // Forward crop sales (see caravan.ForwardSales, contract.Harvest).
    public static final Dial HARVEST_WINDOW = dial("harvest_window_days", 4,
            "days after an installment's day a farm may still deliver it");
    public static final Dial HARVEST_ARREARS = dial("harvest_arrears", 2,
            "installments a farm may fall behind before a shortfall is a miss");
    public static final Dial ROLLED_WEIGHT = dial("harvest_rolled_weight", 0.25,
            "a shortfall rolled over costs this much of a miss in trust");
    public static final Dial ADVANCE_SHARE = dial("forward_advance_share", 1.0 / 3,
            "share of a forward sale's worth the buyer pays up front");
    public static final Dial SOW_DAYS = dial("forward_sow_days", 4,
            "days a farm has to sow before its first installment falls due");
    public static final Dial FORWARD_MAX = dial("forward_max_quantity", 256, "at most this many of a crop an installment");
    public static final Dial FORWARD_INSTALLMENTS = dial("forward_installments", 10, "installments in a forward sale");
    public static final Dial OWN_SHARE = dial("could_grow_own_share", 0.5,
            "share of what a farm grows of a crop it may promise more of");
    public static final Dial OTHER_SHARE = dial("could_grow_other_share", 0.25,
            "share of its other field crops a farm may turn over to a crop it promises");
    // Rents (see contract.Rents, market.RentDealer).
    public static final Dial SEASON_DAYS = dial("rent_season_days", 10, "a rent's season: a coupon falls due each");
    public static final Dial PURCHASE_SEASONS = dial("rent_purchase_seasons", 8,
            "a rent sells for this many seasons' crop at today's price");
    public static final Dial RENT_ARREARS = dial("rent_arrears_seasons", 2,
            "arrears of more than this many seasons' crop are a default");
    public static final Dial DEALER_SPREAD = dial("dealer_spread", 0.1,
            "a dealer buys this much under its price and sells this much over");
    public static final Dial TAKE_PROFIT = dial("dealer_take_profit", 1.25,
            "a holder sells to the dealer when it bids this many times what the holder paid");
    // Fields (see territory.Territory).
    public static final Dial ROTATE_PLOTS = dial("rotate_plots", 81, "plots turned over to another crop a census");
    public static final Dial OWED_FIELDS = dial("rotate_owed_fields", 2,
            "times as many plots a census for a crop owed under contract");

    public static Collection<Dial> all() {
        return ALL.values();
    }

    public static Optional<Dial> named(String name) {
        return Optional.ofNullable(ALL.get(name));
    }

    // What the world has set, by name.
    private final Map<String, Double> set = new LinkedHashMap<>();

    public static Dials get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Dials::new, Dials::load, null), NAME);
    }

    /** Sets a dial for this world (or back to its default, with {@code null}). */
    public void set(Dial dial, Double value) {
        if (value == null) {
            set.remove(dial.name);
            dial.value = dial.initial;
        } else {
            set.put(dial.name, value);
            dial.value = value;
        }
        setDirty();
    }

    /** The world's settings, put in force (every dial not set back to its default). */
    void apply() {
        ALL.values().forEach(d -> d.value = set.getOrDefault(d.name, d.initial));
    }

    public static void onServerStarted(ServerStartedEvent event) {
        get(event.getServer()).apply();
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        set.forEach(tag::putDouble);
        return tag;
    }

    public static Dials load(CompoundTag tag, HolderLookup.Provider registries) {
        Dials dials = new Dials();
        tag.getAllKeys().forEach(k -> dials.set.put(k, tag.getDouble(k)));
        return dials;
    }
}
