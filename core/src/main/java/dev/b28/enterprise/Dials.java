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
    // Stranded paper and freight on credit (see market.Freight).
    public static final Dial FREIGHT_CREDIT_DAYS = dial("freight_credit_days", 30,
            "days a carrier gives a town to pay a fare it couldn't pay up front (its bill on itself to the hub)");
    public static final Dial FREIGHT_LIEN = dial("freight_lien", 0.5,
            "a carrier takes goods on credit while the fare is at most this share of their worth");
    public static final Dial STALE_DISCOUNT = dial("stale_bill_discount", 0.1,
            "a dealer takes this much more off a bill past its day");
    // Bills (see contract.Bills).
    public static final Dial BILL_GRACE = dial("bill_grace_days", 8,
            "days a drawee has to pay a bill from when it's first put to it");
    public static final Dial BILL_GRACE_PER = dial("bill_grace_emeralds_per_day", 100,
            "a day more of grace for every so many emeralds the drawee owes the same holder (0: none)");
    // Deposits (see territory.Territory#borrowDeposits).
    public static final Dial DEPOSIT_SHARE = dial("deposit_share", 0.5,
            "share of its households' savings a Merchant may borrow, to honour its bills or buy paper");
    public static final Dial DEPOSIT_INTEREST = dial("deposit_interest_per_day", 0.002,
            "what a Merchant pays its households a day on what it has borrowed of their savings");
    public static final Dial DEPOSIT_KEEP = dial("deposit_keep", 10,
            "emeralds beyond its reserve a Merchant keeps before it repays its households");
    // Families and trades (see territory.Territory#grow, territory.Jobs).
    public static final Dial HOME_MOOD = dial("home_mood", 0.5, "mood a full town needs to build a family a home");
    public static final Dial BIRTH_MOOD = dial("birth_mood", 0.5, "mood a town with a bed free needs for a child");
    public static final Dial COMFORT_MARBLES = dial("comfort_marbles", 0.2,
            "share of its households' savings a comfort the town lacks puts in the job draw, for the trade that makes it");
    public static final Dial SLACK_DAYS = dial("slack_days", 5,
            "days a villager's trades do nothing for the town before it takes up a trade the town needs");
    // Fields (see territory.Territory).
    public static final Dial ROTATE_PLOTS = dial("rotate_plots", 81, "plots turned over to another crop a census");
    public static final Dial OWED_FIELDS = dial("rotate_owed_fields", 2,
            "times as many plots a census for a crop owed under contract");
    // The counting house and a hub's discount desk (see territory.Buildings, market.Discounts).
    public static final Dial COUNTING_DOCUMENTS = dial("counting_house_documents", 3,
            "documents a day a market draws up before it wants a counting house");
    public static final Dial DISCOUNT_DAYS = dial("discount_days", 3,
            "a hub without a counting house buys paper every so many days (its discount days)");
    public static final Dial DISCOUNT_BOOK_SHARE = dial("discount_book_share", 1.0,
            "the most paper a hub holds, as a share of its spare money (twice with a counting house)");
    public static final Dial DISCOUNT_DRAWEE_SHARE = dial("discount_drawee_share", 0.35,
            "the most of a hub's book on one market's paper");
    public static final Dial DISCOUNT_FOREIGN_SHARE = dial("discount_foreign_share", 0.5,
            "the most of a hub's book on foreign markets' paper");
    public static final Dial DISCOUNT_UNBACKED = dial("discount_unbacked", 0.04,
            "more off a bill whose goods the hub can't see yet (not shipped)");
    public static final Dial DISCOUNT_LONG_DAYS = dial("discount_long_days", 12,
            "a bill due further out than this is past its season");
    public static final Dial DISCOUNT_LONG = dial("discount_long", 0.05, "more off a bill due past its season");
    public static final Dial DISCOUNT_THROTTLE = dial("discount_throttle", 0.03,
            "more off at a full warehouse, and again at a full book: the hub's throttle");
    public static final Dial CORRESPONDENT_FLOOR = dial("correspondent_floor", 24,
            "emeralds beyond its reserve below which a hub with a counting house rediscounts bills at the capital");
    // A town's own wagon: full or due (see caravan.Caravans#dispatch).
    public static final Dial WAGON_FULL = dial("wagon_full", 0.75, "share of its space a town's wagon fills before it goes");
    public static final Dial WAGON_WAIT_DAYS = dial("wagon_wait_days", 4,
            "a town's wagon runs on a timetable too, every so many days (or when full, or with something that can't wait)");
    public static final Dial WAGON_PAYS = dial("wagon_pays", 3,
            "times its road food a load must be worth to go however full");
    public static final Dial SHELF_DAYS = dial("shelf_days", 5,
            "days of what its lockers can sell a town's warehouse aims to hold of each good (its par)");
    public static final Dial COMFORT_DAYS = dial("comfort_days", 6, "days of its people's comforts a town aims to hold");
    public static final Dial COMFORT_KEEP_DAYS = dial("comfort_keep_days", 4,
            "days of its people's comforts a town keeps when others would buy them (it spares the rest)");
    public static final Dial WAGON_TEAM = dial("wagon_team", 1,
            "a wagon's team eats as so many hands, in fodder (wheat or hay, carried there and back; a porter has none)");
    public static final Dial LINER_DAYS = dial("liner_days", 4,
            "a hub's wagon runs on a timetable, every so many days (or when full, or with something due)");
    // The crown's post (see market.Post).
    public static final Dial POST_DAYS = dial("post_days", 3, "the crown's riders leave every so many days");
    public static final Dial POST_SPEED = dial("post_speed", 2000, "blocks a rider covers a day");
    public static final Dial POSTAGE = dial("postage", 0.1, "emeralds a letter costs its sender");
    public static final Dial CALL_MARGIN = dial("call_margin", 0.25,
            "share of the worth of a town's goods a hub reckons to make on them, buying and selling on");
    public static final Dial CALL_START = dial("call_start", 1.5,
            "times its trip's road food a town's goods must make the hub before it writes to come call");
    public static final Dial CALL_STOP = dial("call_stop", 1.0,
            "times its trip's road food (less the crown's aid) the goods offered must make the hub for it to call");
    public static final Dial CALLS_PER_DAY = dial("calls_per_day", 1, "calls a hub books a day at most");
    public static final Dial CROWN_AID = dial("crown_aid", 0.5,
            "share of the road food of a hub's first call at a town the crown pays");
    // The forwarder (see market.Forwarder).
    public static final Dial COLLECT_STACKS = dial("collect_stacks", 8,
            "stacks a town's bookings fill before the forwarder sends to collect");
    public static final Dial COLLECT_DAYS = dial("collect_days", 6, "days a booking waits before the forwarder collects anyway");
    public static final Dial CONSIGN_COMMISSION = dial("consign_commission", 0.1,
            "share of what a town's goods on consignment sell for the forwarder keeps");
    public static final Dial CONSIGN_DAYS = dial("consign_days", 20,
            "days the forwarder holds goods on consignment before it buys what's left itself");
    public static final Dial BLOCK_RUNS = dial("block_runs", 5, "runs a block of space the forwarder buys covers");
    public static final Dial BLOCK_PRICE = dial("block_price", 0.8,
            "what a stack of space bought in a block costs, as a share of the shop's price");
    // The accountant (see money.Accountant).
    public static final Dial BUDGET_SHARE = dial("budget_share", 0.5,
            "share of its free money a town spends a day at its own choosing (half again with a counting house)");
    public static final Dial BUDGET_DAYS = dial("budget_days", 3,
            "days ahead of bills falling due a town keeps the money for (twice with a counting house)");
    // Settlers (see caravan.Expeditions).
    public static final Dial FOUNDING_CAPITAL = dial("founding_capital", 42,
            "emeralds settlers carry to start a new town, if their town can spare twice that");
    public static final Dial SETTLING_CAPITAL = dial("settling_capital", 30,
            "emeralds a travelling Merchant carries to start a market in a village nobody trades with");
    // Herds (see territory.Husbandry).
    public static final Dial PASTURE_ANIMALS = dial("pasture_animals", 16, "animals a pasture holds");
    public static final Dial PASTURES_MAX = dial("pastures_max", 3, "pastures a town fences at most, a new one once full");
    public static final Dial BREED_PER_DAY = dial("breed_per_day", 0.15, "share a fed herd grows by a day, while there's room");
    public static final Dial CULL_SHARE = dial("cull_share", 0.75,
            "share of its pastures' room a herd fills before its Butcher or Leatherworker culls it");
    public static final Dial FEED_DAYS = dial("feed_days", 3,
            "days of its herds' feed a town keeps back from selling its crop");

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

    /** What the world has set, by name: the dials not at their defaults (the core follows them at start). */
    public Map<String, Double> settings() {
        return Map.copyOf(set);
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
