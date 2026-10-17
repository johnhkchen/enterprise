package dev.b28.enterprise.territory;

import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.MoneyChanger;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import dev.ithundxr.createnumismatics.content.coins.CoinItem;
import dev.b28.enterprise.settlement.Census;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Which settlement holds which hex, and what each settlement's hexes earn it. Civilization-style: a settlement claims
 * its bell's hex, gathers culture from its value at each census, and grows into the nearest unclaimed neighbouring
 * hex when culture covers the cost, out to two rings. Its farmers work its hexes, producing the crops growing there;
 * the produce and any tolls owed to it land in its Merchant's chest whenever the market is touched.
 */
public final class Territory extends SavedData {
    private static final String NAME = "enterprise_territory";

    /** Culture for the next hex: this much times the hexes already held. */
    public static final double CULTURE_PER_HEX = 5;

    /** Rings around the home hex a settlement can grow into: its hex plus two rings, 19 hexes. */
    public static final int MAX_RINGS = 2;

    /** Plots a farmer tends per profession level. */
    public static final int PLOTS_PER_FARMER_LEVEL = 32;

    /** Items a worked plot yields per in-game day. */
    public static final double YIELD_PER_PLOT_PER_DAY = 0.5;

    /** The farmers' starting share of the value of what they grow. */
    public static final double LABOR_SHARE = 0.5;

    /**
     * The share floats between these: toward the fraction of the harvest that actually sells for emeralds. A farm
     * whose produce all sells ends up paying its whole sale value in wages (passive farming pays for itself, not for
     * you); one whose produce doesn't sell pays the floor, so its harvest is cheap material, never free.
     */
    public static final double MIN_LABOR_SHARE = 0.25;
    public static final double MAX_LABOR_SHARE = 1.0;

    /** Days for the share to close half the gap to its target, and for crop sales to be averaged over. */
    private static final double SHARE_HALF_LIFE_DAYS = 2;
    private static final double SALES_HALF_LIFE_DAYS = 3;

    /** Days of wages the Merchant keeps in emeralds rather than selling them to the mint. */
    public static final double WAGE_FLOAT_DAYS = 3;

    /** Every town starts tolling caravans that cross its land at this rate, then sets its own. */
    public static final double DEFAULT_TOLL_RATE = 0.05;
    public static final double MAX_TOLL_RATE = 0.25;
    /** A town asks at most this many emeralds a caravan as a handling fee at its posts, where there's no way round. */
    public static final double PASSAGE_FEE_CAP = 3;
    public static final double TOLL_STEP = 0.01;

    /** A town judges its toll over this many days: caravans pass every few days, so one day says little. */
    public static final long TOLL_REVIEW_TICKS = 3 * 24000L;

    /** Guards on posts that bring in tolls wait this many days' pay before they go unpaid. */
    public static final double GUARD_CREDIT_DAYS = 3;

    /** A guard's daily food (points of nutrition) and pay (emeralds). */
    public static final double GUARD_FOOD_PER_DAY = 6;
    public static final double GUARD_PAY_PER_DAY = 0.5;

    /** At most this many days of produce accrue between visits. */
    private static final double MAX_DAYS = 60;

    /**
     * Mood: each kind of comfort fully supplied adds 1. A town wants more as it grows: every this many villagers takes
     * 1 away. Each villager without a bed takes {@link #CROWDING_MOOD} away, and wages wholly unpaid
     * {@link #UNPAID_MOOD}. Mood closes half its gap to that every {@link #MOOD_HALF_LIFE_DAYS} days.
     */
    public static final double VILLAGERS_PER_WANT = 8;
    public static final double CROWDING_MOOD = 0.5;
    public static final double UNPAID_MOOD = 2;

    /** Each of its soldiers fallen in the last {@link #GRIEF_TICKS} takes this much from its mood, at most {@link #GRIEF_MOST}. */
    public static final double GRIEF_MOOD = 0.5;
    public static final double GRIEF_MOST = 2;
    public static final long GRIEF_TICKS = 10 * 24000L;

    /** A Cleric's blessing adds this much to where the mood is heading, while it lasts. */
    public static final double BLESSING_MOOD = 0.5;
    private static final double MOOD_HALF_LIFE_DAYS = 3;

    /** A hex in a dimension. */
    public record Place(ResourceKey<Level> dimension, Hex hex) {}

    /** A villager's trade buying {@code item} for emeralds, at its job site, by trade key. */
    public record VillageTrade(BlockPos site, String key, String item) {}

    private static final class Holding {
        double culture;
        GlobalPos desk; // nearest Merchant's desk, if any
        Map<String, Double> perDay = new LinkedHashMap<>(); // farm plan: item id -> items per day
        Map<String, Double> pending = new HashMap<>(); // produce owed, fractions carried
        long lastHarvest = -1;
        long tollsOwed;
        double wageOwed; // emeralds owed to the farmers, fractions carried
        double purse; // the households' money: their wages, to spend at the market (see Households)
        double deposits; // what the Merchant has borrowed of the households' savings, with interest: owed them
        long lastDeposits; // when interest on them was last reckoned
        double credit; // pay the town owes its farmers, in emeralds
        int freeholds; // plots the farmers own
        double householdSpent; // emeralds the households have paid the market, in all
        double labourTightness = 1; // how scarce farm hands are, from the last census
        double handTools; // tools given to the hands working its land: they gather more until worn out
        double wagesPaid; // emeralds' worth paid, in all
        double wagesUnpaid; // emeralds' worth that couldn't be paid: the farmers took it in crops
        long harvested; // items delivered to the Merchant's chest, in all
        double farmXp; // experience the Farmers earned with their harvest, not yet given them
        Map<String, Double> banked = new HashMap<>(); // crops banked in its stores (granary, root cellar), by item
        Map<String, Integer> turned = new HashMap<>(); // plots turned over on its books, not yet in the ground, by crop
        double laborShare = LABOR_SHARE;
        double salesSince; // emeralds from selling crops since the last accrual
        double salesPerDay; // averaged
        double valuePerDay; // the harvest's value, averaged
        double treasury; // the market's money at its last touch, in emeralds (chest emeralds, coins, Banker)
        double tollRate = DEFAULT_TOLL_RATE;
        double passageFee; // spurs a caravan through its posts, whatever it carries
        double plannedTollIncome; // spurs a day its toll plan expects, before the guards' pay
        double tollIncomeToday; // since the last review
        double tollIncomeYesterday = -1; // over the last period reviewed; none yet
        long lastTollReview = -1;
        java.util.Set<Hex> posts = new java.util.LinkedHashSet<>(); // hexes with a guard: the town's toll posts
        Map<Integer, Chokepoints.Crossing> crossings = new LinkedHashMap<>(); // by caravan route id
        double guardUpkeepOwed; // emeralds owed to the guards, fractions carried
        double guardFoodOwed; // food points owed, fractions carried
        long lastGuardUpkeep = -1;
        double trafficSince; // coins carried across this land since the last review, in spurs
        double trafficPerDay; // averaged
        int deserted; // guards lost to hunger or no pay, in all
        int villagers; // at the last census
        int beds;
        double mood;
        Map<String, Double> comfortOwed = new HashMap<>(); // by kind, emeralds' worth; below 0: a big item's credit
        Map<String, Double> comfortMet = new LinkedHashMap<>(); // by kind, share of the ration met at the last touch
        long lastComforts = -1;
        double paidShare = 1; // of the wages due at the last touch
        double homeProgress; // toward the next home, 1 when due
        double birthProgress; // toward the next child
        int homesBuilt;
        Focus focus = Focus.GROW; // what it puts first, from its last census while loaded
        int born;
        double walled; // share of its wall standing, from the works ledger at the last touch
        double tradingCapital; // emeralds to stock its shelves, from the warehouse at the last touch
        Map<String, Double> demand = new HashMap<>(); // what its villagers would buy for emeralds, items a day
        int fieldsPlanted;
        Map<String, Double> herd = new LinkedHashMap<>(); // animals kept, by entity id, fractions carried
        BlockPos pasture; // the pasture's corner, once fenced
        List<BlockPos> morePastures = new ArrayList<>(); // pastures fenced since, as the herd outgrew the first
        Map<String, Double> fodder = new LinkedHashMap<>(); // feed each kind is owed, less than a whole item
        BlockPos dock; // the pier's first block over water, once built
        long lastTend = -1;
        long blessedUntil = -1; // a Cleric's blessing lasts until then
        Map<String, Double> idlePerDay = new LinkedHashMap<>(); // what its untended plots would yield, items a day
        int spareFarmPlots; // plots its farmers could tend beyond those planted
        BlockPos mine; // the mine head's shaft, once dug
        Map<String, Double> kitWear = new LinkedHashMap<>(); // by kit item id, share of one worn since last taken
        long mendedUntil = -1; // a Toolsmith mends its tools until then
        Map<String, Double> kitShare = new LinkedHashMap<>(); // by kit item id, share of its wearers covered
        int militia; // hands without a trade standing watch at its posts, at the last touch
        double hunting; // share of its Fletchers with bows, at the last touch
        double armed; // share of its guards with swords or bows (or archers on a watchtower), at the last touch
        java.util.Set<String> buildings = new java.util.LinkedHashSet<>(); // kinds finished, from the works ledger
        boolean flowerBeds; // a Farmer has planted beds on its square
        boolean deepMine; // a master Toolsmith has sunk its mine deep: lapis too
        boolean blastFurnace; // a master Armorer has built one: smelting takes half the coal
        long armouryUntil = -1; // a master Weaponsmith keeps its guards' kit until then
        java.util.List<Long> fallen = new java.util.ArrayList<>(); // when its soldiers fell, for its grief
    }

    private final Map<Place, GlobalPos> owners = new HashMap<>();
    // Fields: per hex, per chunk with crops, the plots last counted there. Chunks are recounted whenever loaded
    // (rounds load a settlement's known field chunks), and keep their last count otherwise.
    private final Map<Place, Map<Long, Map<String, Integer>>> fields = new HashMap<>();
    private final Map<GlobalPos, Holding> holdings = new HashMap<>();

    public static Territory get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Territory::new, Territory::load, null), NAME);
    }

    public Optional<GlobalPos> owner(ResourceKey<Level> dimension, Hex hex) {
        return Optional.ofNullable(owners.get(new Place(dimension, hex)));
    }

    /** The hexes a settlement holds. */
    public List<Hex> hexes(GlobalPos bell) {
        return owners.entrySet().stream()
                .filter(entry -> entry.getValue().equals(bell))
                .map(entry -> entry.getKey().hex())
                .sorted(Comparator.comparingInt(Hex::q).thenComparingInt(Hex::r))
                .toList();
    }

    public double culture(GlobalPos bell) {
        return holding(bell).map(h -> h.culture).orElse(0.0);
    }

    public double nextHexCost(GlobalPos bell) {
        return CULTURE_PER_HEX * Math.max(1, hexes(bell).size());
    }

    /**
     * What more of a farmland crop the town could grow a day: its Farmers' spare hands (on new fields), half what it
     * grows of it now, and a quarter of its other field crops (turned over). Not its untended plots: no one to work
     * them. What a farm can promise forward, before what it has promised.
     */
    public double couldGrow(GlobalPos bell, Item crop) {
        return holding(bell).map(h -> {
            String id = BuiltInRegistries.ITEM.getKey(crop).toString();
            double others = ROTATION.stream().filter(c -> c != crop).mapToDouble(c -> h.perDay.getOrDefault(
                    BuiltInRegistries.ITEM.getKey(c).toString(), 0.0)).sum();
            return h.spareFarmPlots * YIELD_PER_PLOT_PER_DAY + dev.b28.enterprise.Dials.OWN_SHARE.get() * h.perDay.getOrDefault(id, 0.0)
                    + dev.b28.enterprise.Dials.OTHER_SHARE.get() * others;
        }).orElse(0.0);
    }

    /** What the plots no one tends would yield a day, by crop id. */
    public Map<String, Double> idlePlots(GlobalPos bell) {
        return holding(bell).map(h -> Map.copyOf(h.idlePerDay)).orElse(Map.of());
    }

    public Map<String, Double> farmPlan(GlobalPos bell) {
        return holding(bell).map(h -> Map.copyOf(h.perDay)).orElse(Map.of());
    }

    /** The farmers' wages per day, in emeralds: {@link #LABOR_SHARE} of the value of what they grow. */
    public double wagePerDay(GlobalPos bell) {
        return holding(bell).map(Territory::wagePerDay).orElse(0.0);
    }

    private static double wagePerDay(Holding holding) {
        double plots = holding.perDay.values().stream().mapToDouble(Double::doubleValue).sum() / YIELD_PER_PLOT_PER_DAY;
        return valuePerDay(holding.perDay) * Households.share(holding.laborShare, plots <= 0 ? 0 : holding.freeholds / plots);
    }

    /** What a day's harvest sells for at local buying prices, in emeralds. */
    static double valuePerDay(Map<String, Double> perDay) {
        double value = 0;
        for (Map.Entry<String, Double> crop : perDay.entrySet()) {
            value += crop.getValue() / Prices.itemsPerEmerald(crop.getKey());
        }
        return value;
    }

    public double laborShare(GlobalPos bell) {
        return holding(bell).map(h -> h.laborShare).orElse(LABOR_SHARE);
    }

    /** The settlement market's money at its last touch, in emeralds at the rate then. */
    public double treasury(GlobalPos bell) {
        return holding(bell).map(h -> h.treasury).orElse(0.0);
    }

    public double cropSalesPerDay(GlobalPos bell) {
        return holding(bell).map(h -> h.salesPerDay).orElse(0.0);
    }

    /** Is this item a crop farmers grow? (Locker trades selling one count as farm sales.) */
    public static boolean isCrop(String itemId) {
        return Crops.YIELDS.values().stream()
                .anyMatch(item -> BuiltInRegistries.ITEM.getKey(item).toString().equals(itemId));
    }

    /** A locker at the settlement's market sold crops for {@code emeralds}. */
    public void recordCropSale(GlobalPos desk, double emeralds) {
        bellOf(desk).ifPresent(bell -> {
            holdings.get(bell).salesSince += emeralds;
            setDirty();
        });
    }

    public double wagesPaid(GlobalPos bell) {
        return holding(bell).map(h -> h.wagesPaid).orElse(0.0);
    }

    public long harvested(GlobalPos bell) {
        return holding(bell).map(h -> h.harvested).orElse(0L);
    }

    /** {purse, credit owed them, freehold plots, spent at the market in all, labour tightness} of the town's households. */
    public double[] households(GlobalPos bell) {
        return holding(bell).map(h -> new double[] {h.purse, h.credit, h.freeholds, h.householdSpent, h.labourTightness})
                .orElse(new double[] {0, 0, 0, 0, 1});
    }

    public double wagesUnpaid(GlobalPos bell) {
        return holding(bell).map(h -> h.wagesUnpaid).orElse(0.0);
    }

    /** Emeralds the settlement's Merchant keeps back for wages. */
    public int wageFloat(GlobalPos desk) {
        return bellOf(desk).map(bell -> (int) Math.ceil(wagePerDay(bell) * WAGE_FLOAT_DAYS)).orElse(0);
    }

    public long tollsOwed(GlobalPos bell) {
        return holding(bell).map(h -> h.tollsOwed).orElse(0L);
    }

    /** Desks of settlements with farms: markets the ledger keeps running even with no trades switched on. */
    public java.util.Set<GlobalPos> farmDesks() {
        java.util.Set<GlobalPos> desks = new java.util.LinkedHashSet<>();
        holdings.values().stream().filter(h -> h.desk != null && !h.perDay.isEmpty()).forEach(h -> desks.add(h.desk));
        return desks;
    }

    /** Sets the households' savings (for tests). */
    public void setPurseForTest(GlobalPos bell, double emeralds) {
        holdings.computeIfAbsent(bell, b -> new Holding()).purse = emeralds;
        setDirty();
    }

    /** Links a settlement to its Merchant's desk without a census (for tests). */
    public void setDeskForTest(GlobalPos bell, GlobalPos desk) {
        holdings.computeIfAbsent(bell, b -> new Holding()).desk = desk;
        setDirty();
    }

    /** What the census found the village's people would buy for emeralds, item id -> items a day. */
    public void recordDemand(GlobalPos bell, Map<String, Double> demand) {
        Holding holding = holdings.computeIfAbsent(bell, b -> new Holding());
        holding.demand = new HashMap<>(demand);
        Husbandry.feedDemand(holding.herd).forEach((feed, perDay) -> holding.demand.merge(feed, perDay, Double::sum));
        setDirty();
    }

    /** The trades of the town's residents, by profession id (from its roster). */
    static java.util.Set<String> trades(MinecraftServer server, GlobalPos bell) {
        return dev.b28.enterprise.settlement.Settlements.get(server).roster(bell).values().stream()
                .map(dev.b28.enterprise.settlement.Settlements.Resident::profession)
                .collect(java.util.stream.Collectors.toSet());
    }

    public Map<String, Double> herd(GlobalPos bell) {
        return holding(bell).map(h -> Map.copyOf(h.herd)).orElse(Map.of());
    }

    public void setHerd(GlobalPos bell, String kind, double count) {
        holdings.computeIfAbsent(bell, b -> new Holding()).herd.put(kind, count);
        setDirty();
    }

    public Optional<BlockPos> pasture(GlobalPos bell) {
        return holding(bell).map(h -> h.pasture);
    }

    /**
     * When counted while loaded: a town with a Fisherman and no dock builds one where its land meets water (4
     * emeralds beyond its reserve).
     */
    private void buildDock(ServerLevel level, BlockPos bellPos, Holding holding, MerchantStock stock) {
        GlobalPos bell = GlobalPos.of(level.dimension(), bellPos);
        if (holding.dock != null || !trades(level.getServer(), bell).contains("minecraft:fisherman")) {
            return;
        }
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        if (Treasury.available(level, holding.desk.pos(), stock, rate) - reserve(holding.desk) < Fields.FIELD_EMERALDS) {
            return;
        }
        Docks.findShore(level, bellPos)
                .filter(pier -> owner(level.dimension(), Hex.at(pier.start())).filter(bell::equals).isPresent())
                .ifPresent(pier -> {
                    Docks.build(level, pier);
                    Treasury.withdraw(level, holding.desk.pos(), stock, Fields.FIELD_EMERALDS, Mint.get(level.getServer()),
                            level.getGameTime());
                    holding.dock = pier.start();
                    dev.b28.enterprise.Enterprise.LOGGER.debug("Settlement at {} builds a dock at {}",
                            bellPos.toShortString(), pier.start().toShortString());
                });
    }

    public static final String TOOLSMITH = "minecraft:toolsmith";
    public static final String FLETCHER = "minecraft:fletcher";

    /** The levels of a trade's grown residents at home, summed. */
    static double levels(MinecraftServer server, GlobalPos bell, String trade) {
        return dev.b28.enterprise.settlement.Settlements.get(server).roster(bell).values().stream()
                .filter(r -> r.profession().equals(trade) && !r.away() && !r.baby())
                .mapToInt(r -> Math.max(1, r.level())).sum();
    }

    /** The best level of a trade's grown residents at home, 0 if none. */
    public static int bestLevel(MinecraftServer server, GlobalPos bell, String trade) {
        return dev.b28.enterprise.settlement.Settlements.get(server).roster(bell).values().stream()
                .filter(r -> r.profession().equals(trade) && !r.away() && !r.baby())
                .mapToInt(r -> Math.max(1, r.level())).max().orElse(0);
    }

    /** One of the town's soldiers fell: it grieves. */
    public void mourn(GlobalPos bell, long now) {
        Holding holding = holdings.computeIfAbsent(bell, b -> new Holding());
        holding.fallen.removeIf(t -> now - t > GRIEF_TICKS);
        holding.fallen.add(now);
        setDirty();
    }

    public boolean deepMine(GlobalPos bell) {
        return holding(bell).map(h -> h.deepMine).orElse(false);
    }

    public void sinkDeepMine(GlobalPos bell) {
        holdings.computeIfAbsent(bell, b -> new Holding()).deepMine = true;
        setDirty();
    }

    public boolean blastFurnace(GlobalPos bell) {
        return holding(bell).map(h -> h.blastFurnace).orElse(false);
    }

    public void buildBlastFurnace(GlobalPos bell) {
        holdings.computeIfAbsent(bell, b -> new Holding()).blastFurnace = true;
        setDirty();
    }

    public long armouryUntil(GlobalPos bell) {
        return holding(bell).map(h -> h.armouryUntil).orElse(-1L);
    }

    public void keepArmoury(GlobalPos bell, long until) {
        Holding holding = holdings.computeIfAbsent(bell, b -> new Holding());
        holding.armouryUntil = Math.max(holding.armouryUntil, until);
        setDirty();
    }

    /** Kinds of building it has finished, at the last touch. */
    public java.util.Set<String> buildings(GlobalPos bell) {
        return holding(bell).map(h -> java.util.Set.copyOf(h.buildings)).orElse(java.util.Set.of());
    }

    public double armed(GlobalPos bell) {
        return holding(bell).map(h -> h.armed).orElse(0.0);
    }

    public double hunting(GlobalPos bell) {
        return holding(bell).map(h -> h.hunting).orElse(0.0);
    }

    public boolean flowerBeds(GlobalPos bell) {
        return holding(bell).map(h -> h.flowerBeds).orElse(false);
    }

    public void plantFlowerBeds(GlobalPos bell) {
        holdings.computeIfAbsent(bell, b -> new Holding()).flowerBeds = true;
        setDirty();
    }

    public double moodTargetForTest(GlobalPos bell) {
        return holding(bell).map(Territory::moodTarget).orElse(0.0);
    }

    public void setArmedForTest(GlobalPos bell, double armed) {
        holdings.computeIfAbsent(bell, b -> new Holding()).armed = armed;
    }

    /** Grown residents at home, by trade. */
    public static Map<String, Integer> workers(MinecraftServer server, GlobalPos bell) {
        Map<String, Integer> count = new LinkedHashMap<>();
        dev.b28.enterprise.settlement.Settlements.get(server).roster(bell).values().stream()
                .filter(r -> !r.away() && !r.baby())
                .forEach(r -> count.merge(r.profession(), 1, Integer::sum));
        return count;
    }

    /** Its miners: its Toolsmiths at home, once it has a mine. */
    private static int miners(Holding holding, Map<String, Integer> workers) {
        return holding.mine == null ? 0 : workers.getOrDefault(TOOLSMITH, 0);
    }

    /**
     * When counted while loaded: a town with a Toolsmith and no mine digs one, a 3 x 3 head on open ground in its land
     * near the bell, for {@link Mines#MINE_EMERALDS} beyond its reserve.
     */
    private void digMine(ServerLevel level, BlockPos bellPos, Holding holding, MerchantStock stock) {
        GlobalPos bell = GlobalPos.of(level.dimension(), bellPos);
        if (holding.mine != null || !trades(level.getServer(), bell).contains(TOOLSMITH)) {
            return;
        }
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        if (Treasury.available(level, holding.desk.pos(), stock, rate) - reserve(holding.desk) < Mines.MINE_EMERALDS) {
            return;
        }
        Mines.site(level, bellPos)
                .filter(corner -> owner(level.dimension(), Hex.at(corner)).filter(bell::equals).isPresent())
                .ifPresent(corner -> {
                    holding.mine = Mines.build(level, corner);
                    Treasury.withdraw(level, holding.desk.pos(), stock, Mines.MINE_EMERALDS, Mint.get(level.getServer()),
                            level.getGameTime());
                    dev.b28.enterprise.Enterprise.LOGGER.debug("Settlement at {} digs a mine at {}",
                            bellPos.toShortString(), corner.toShortString());
                });
    }

    /** Its hands without a trade standing watch at its posts, at the last touch. */
    public int militia(GlobalPos bell) {
        return holding(bell).map(h -> h.militia).orElse(0);
    }

    public Optional<BlockPos> mine(GlobalPos bell) {
        return holding(bell).map(h -> h.mine);
    }

    public void setMineForTest(GlobalPos bell, BlockPos shaft) {
        holdings.computeIfAbsent(bell, b -> new Holding()).mine = shaft;
    }

    /** Share of each kit's wearers it covers, by kit item id, at the last touch. */
    public Map<String, Double> kitShare(GlobalPos bell) {
        return holding(bell).map(h -> Map.copyOf(h.kitShare)).orElse(Map.of());
    }

    public void setKitShareForTest(GlobalPos bell, Smithing.Kit kit, double share) {
        holdings.computeIfAbsent(bell, b -> new Holding()).kitShare.put(kit.id(), share);
    }

    /** A Toolsmith mends the town's tools until {@code until}: they wear half as fast. */
    public void mend(GlobalPos bell, long until) {
        Holding holding = holdings.computeIfAbsent(bell, b -> new Holding());
        holding.mendedUntil = Math.max(holding.mendedUntil, until);
        setDirty();
    }

    public long mendedUntil(GlobalPos bell) {
        return holding(bell).map(h -> h.mendedUntil).orElse(-1L);
    }

    public Optional<BlockPos> dock(GlobalPos bell) {
        return holding(bell).map(h -> h.dock);
    }

    /**
     * When counted while loaded: a new Shepherd's or Butcher's animals join the herd; a town with animals and no
     * pasture fences one (9 x 9, in its home hex, 4 emeralds beyond its reserve), and another when the herd fills the
     * ones it has (up to the {@code pastures_max} dial); and the pastures' animals are set right by the ledger.
     */
    private void keepHerds(ServerLevel level, BlockPos bellPos, Holding holding, MerchantStock stock) {
        GlobalPos bell = GlobalPos.of(level.dimension(), bellPos);
        Husbandry.seed(holding.herd, trades(level.getServer(), bell));
        if (!Husbandry.any(holding.herd)) {
            return;
        }
        double animals = holding.herd.values().stream().mapToDouble(Double::doubleValue).sum();
        boolean full = holding.pasture != null && pastures(holding).size() < dev.b28.enterprise.Dials.PASTURES_MAX.getInt()
                && animals >= 0.9 * Husbandry.capacity(pastures(holding).size());
        if (holding.pasture == null || full) {
            fencePasture(level, bellPos, bell, holding, stock);
        }
        Husbandry.sync(level, pastures(holding), holding.herd);
    }

    private void fencePasture(ServerLevel level, BlockPos bellPos, GlobalPos bell, Holding holding, MerchantStock stock) {
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        double cash = Treasury.available(level, holding.desk.pos(), stock, rate) - reserve(holding.desk);
        if (cash + depositsOnCall(holding.desk) < Fields.FIELD_EMERALDS) {
            return; // not even with its households' savings
        }
        Optional<BlockPos> site = Growth.farmSite(level, bellPos, Husbandry.PASTURE_SIZE, 8)
                .filter(corner -> owner(level.dimension(), Hex.at(corner.offset(4, 0, 4))).filter(bell::equals).isPresent());
        if (site.isEmpty() || cash >= Fields.FIELD_EMERALDS && !dev.b28.enterprise.money.Accountant.approve(level,
                holding.desk, stock, "a pasture", Fields.FIELD_EMERALDS, level.getGameTime())) {
            return;
        }
        Husbandry.fence(level, site.get());
        if (cash < Fields.FIELD_EMERALDS) {
            borrowDeposits(level, holding.desk, stock, Fields.FIELD_EMERALDS - cash);
        }
        Treasury.withdraw(level, holding.desk.pos(), stock, Fields.FIELD_EMERALDS, Mint.get(level.getServer()),
                level.getGameTime());
        if (holding.pasture == null) {
            holding.pasture = site.get();
        } else {
            holding.morePastures.add(site.get());
        }
        dev.b28.enterprise.Enterprise.LOGGER.debug("Settlement at {} fences a pasture at {}", bellPos.toShortString(),
                site.get().toShortString());
    }

    private static List<BlockPos> pastures(Holding holding) {
        List<BlockPos> all = new ArrayList<>();
        if (holding.pasture != null) {
            all.add(holding.pasture);
        }
        all.addAll(holding.morePastures);
        return all;
    }

    /** The pastures this town has fenced, the first first. */
    public List<BlockPos> pastures(GlobalPos bell) {
        return holding(bell).map(Territory::pastures).orElse(List.of());
    }

    /**
     * What of {@code item} the town at {@code desk} keeps back from selling: the feed for the {@code feed_days} dial's
     * days of every kind in its herds that eats it.
     */
    public int feedKept(GlobalPos desk, Item item) {
        return bellOf(desk).flatMap(this::holding).map(h -> {
            double perDay = 0;
            for (Husbandry.Kind kind : Husbandry.KINDS) {
                if (kind.feed().contains(item)) {
                    perDay += h.herd.getOrDefault(kind.id(), 0.0) * kind.feedPerDay();
                }
            }
            return (int) Math.ceil(perDay * dev.b28.enterprise.Dials.FEED_DAYS.get());
        }).orElse(0);
    }

    public int fieldsPlanted(GlobalPos bell) {
        return holding(bell).map(h -> h.fieldsPlanted).orElse(0);
    }

    void notePlanted(GlobalPos bell) {
        holdings.computeIfAbsent(bell, b -> new Holding()).fieldsPlanted++;
        setDirty();
    }

    public Map<String, Double> demand(GlobalPos bell) {
        return holding(bell).map(h -> Map.copyOf(h.demand)).orElse(Map.of());
    }

    /**
     * The Merchant tells its farmers what sells: every locker trade in the village that buys a crop the town grows
     * (or paper, when it grows cane) is switched on, so a town's harvest finds its buyers without anyone's help.
     */
    public void mediate(ServerLevel level, GlobalPos bell, List<VillageTrade> trades) {
        Holding holding = holdings.get(bell);
        if (holding == null || holding.desk == null) {
            return;
        }
        java.util.Set<String> grown = new java.util.HashSet<>(holding.perDay.keySet());
        Workshop.outputsOf(grown).forEach(grown::add);
        var book = dev.b28.enterprise.merchant.MerchantBook.get(level.getServer());
        for (VillageTrade trade : trades) {
            if (grown.contains(trade.item()) && !dev.b28.enterprise.locker.Lockers.touchingContainers(level, trade.site()).isEmpty()
                    && !book.isEnabled(holding.desk, trade.site(), trade.key())) {
                book.setEnabled(holding.desk, trade.site(), trade.key(), true);
            }
        }
    }

    /** A tool given to a hand lasts this many days of work. */
    public static final double HAND_TOOL_DAYS = 16;

    /** Tools the Merchant gives the hands working the town's land: they gather half again as much while they last. */
    public void giveTools(GlobalPos bell, int tools) {
        holdings.computeIfAbsent(bell, b -> new Holding()).handTools += tools;
        setDirty();
    }

    public double handTools(GlobalPos bell) {
        return holding(bell).map(h -> h.handTools).orElse(0.0);
    }

    /** The desk of the settlement's Merchant, if it has one. */
    public Optional<GlobalPos> desk(GlobalPos bell) {
        return holding(bell).map(h -> h.desk);
    }

    /** Every settlement's Merchant's Desk. */
    public java.util.Set<GlobalPos> desks() {
        java.util.Set<GlobalPos> desks = new java.util.LinkedHashSet<>();
        holdings.values().stream().filter(h -> h.desk != null).forEach(h -> desks.add(h.desk));
        return desks;
    }

    /** The settlement whose Merchant works at this desk. */
    public Optional<GlobalPos> bellOf(GlobalPos desk) {
        return holdings.entrySet().stream()
                .filter(entry -> desk.equals(entry.getValue().desk))
                .map(Map.Entry::getKey)
                .findFirst();
    }

    /** Claims a hex for a settlement if nobody holds it. Returns true if claimed. */
    public boolean claim(GlobalPos bell, Hex hex) {
        Place place = new Place(bell.dimension(), hex);
        if (owners.containsKey(place)) {
            return false;
        }
        owners.put(place, bell);
        holdings.computeIfAbsent(bell, b -> new Holding());
        setDirty();
        return true;
    }

    public void addCulture(GlobalPos bell, double culture) {
        holdings.computeIfAbsent(bell, b -> new Holding()).culture += culture;
        setDirty();
    }

    /**
     * After a census: link the settlement to its Merchant's desk, claim its home hex, add culture for the days since
     * the last census, grow while culture allows, and re-plan its farms from the farmers counted.
     */
    public void afterCensus(ServerLevel level, BlockPos bellPos, Census census, double days, List<Integer> farmers) {
        GlobalPos bell = GlobalPos.of(level.dimension(), bellPos);
        Holding holding = holdings.computeIfAbsent(bell, b -> new Holding());
        holding.desk = nearestDesk(level, bellPos).map(desk -> GlobalPos.of(level.dimension(), desk)).orElse(null);
        holding.villagers = census.villagers();
        holding.beds = census.beds();
        Hex home = Hex.at(bellPos);
        claim(bell, home);
        if (owner(level.dimension(), home).filter(bell::equals).isPresent()) {
            holding.culture += (census.value() + Works.WALL_CULTURE_PER_DAY * Works.get(level.getServer()).walled(bell)
                    + (holding.buildings.contains(Buildings.LIBRARY.id()) ? Buildings.LIBRARY_CULTURE_PER_DAY : 0)
                    + (holding.buildings.contains(Buildings.CARAVANSERAI.id()) ? Buildings.CARAVANSERAI_CULTURE_PER_DAY : 0))
                    * days * cultureFactor(holding.mood);
            grow(bell, home);
        }
        if (holding.desk != null && level.isLoaded(holding.desk.pos())) {
            // Settle produce and wages under the old plan before re-planning.
            MerchantStock stock = MerchantStock.at(level, holding.desk.pos());
            touch(level, holding.desk.pos(), stock);
            grow(level, bellPos, holding, census, days, stock);
            keepHerds(level, bellPos, holding, stock);
            buildDock(level, bellPos, holding, stock);
            digMine(level, bellPos, holding, stock);
        } else {
            accrue(holding, level.getGameTime());
        }
        Map<String, Double> lastIdle = holding.idlePerDay; // what lay untended under the last plan
        holding.idlePerDay = new LinkedHashMap<>();
        double hands = 1 + Smithing.TOOL_BONUS * holding.kitShare.getOrDefault(Smithing.HOE.id(), 0.0); // hoes help
        // What it would buy, and what its own workshop would draw to make its people's comforts: its potential.
        Map<String, Double> wants = new HashMap<>(holding.demand);
        Map<String, double[]> workshop = holding.desk == null ? Map.of()
                : Workshop.inputDemand(level, holding.desk, bell, census.villagers());
        workshop.forEach((item, d) -> wants.merge(item, d[0], Double::sum));
        // And what the markets its caravans reach would take: the Merchant tells its Farmers what sells there.
        Map<String, List<double[]>> markets = holding.desk == null ? new HashMap<>()
                : marketWants(level.getServer(), holding.desk);
        // What it has sold forward comes first, after the herds' feed: a contract to keep, at its locked price.
        Map<String, Double> needs = new HashMap<>(Husbandry.feedDemand(holding.herd));
        if (holding.desk != null) {
            dev.b28.enterprise.caravan.ForwardSales.sold(level.getServer(), holding.desk).forEach((item, sold) -> {
                needs.merge(item, sold[0], Double::sum);
                markets.computeIfAbsent(item, k -> new ArrayList<>()).add(sold);
            });
        }
        markets.forEach((item, each) -> each.forEach(m -> wants.merge(item, m[0], Double::sum)));
        boolean fed = holding.desk != null && level.isLoaded(holding.desk.pos()) && MerchantStock.at(level,
                holding.desk.pos()).foodNutrition() >= 2 * dev.b28.enterprise.merchant.Warehouse.FOOD_PAR_POINTS;
        rotate(level, bellPos.getY(), hexes(bell), holding.demand, workshop, markets, needs, fed, holding.turned, lastIdle);
        holding.perDay = plan(level, bellPos.getY(), hexes(bell), farmers, holding.demand, workshop, markets,
                needs, holding.idlePerDay, hands, fed, holding.turned);
        double tended = holding.perDay.values().stream().mapToDouble(Double::doubleValue).sum() / YIELD_PER_PLOT_PER_DAY;
        double capacity = hands * farmers.stream().mapToInt(l -> PLOTS_PER_FARMER_LEVEL * Math.max(1, l)).sum();
        holding.spareFarmPlots = (int) Math.max(0, capacity - Math.round(tended));
        double untended = holding.idlePerDay.values().stream().mapToDouble(Double::doubleValue).sum() / YIELD_PER_PLOT_PER_DAY;
        holding.labourTightness = Households.tightness(capacity, tended, untended);
        holding.freeholds = (int) Math.min(holding.freeholds, Math.round(tended + untended)); // fields gone, titles with them
        if (holding.desk != null && level.isLoaded(holding.desk.pos())) {
            double plotsWorked = holding.perDay.values().stream().mapToDouble(Double::doubleValue).sum() / YIELD_PER_PLOT_PER_DAY;
            MerchantStock fieldStock = MerchantStock.at(level, holding.desk.pos());
            holding.focus = Focus.of(census.villagers(), census.beds(), fieldStock.foodNutrition());
            Fields.consider(level, bellPos, holding.desk, fieldStock, this, farmers, plotsWorked,
                    wants, holding.perDay, holding.focus);
            buyLand(level, bellPos, holding, fieldStock);
        }
        setDirty();
    }

    /**
     * Experience a Farmer earns for each emerald's worth of harvest it brings in: as a Novice's trade of crops for an
     * emerald gives (vanilla's 2), and a little over, since it grew them too.
     */
    public static final double HARVEST_XP_PER_EMERALD = 3;

    /**
     * The Farmers of the town (loaded, on its roster) take the experience their harvest earned, shared out, and rise
     * a level when it's enough, as for a trade: a Farmer gets better by farming, not only by selling at its locker.
     */
    private void farmersLearn(ServerLevel level, BlockPos bell, Holding holding) {
        int xp = (int) Math.floor(holding.farmXp);
        if (xp <= 0) {
            return;
        }
        var residents = dev.b28.enterprise.settlement.Settlements.get(level.getServer())
                .roster(GlobalPos.of(level.dimension(), bell)).keySet();
        List<net.minecraft.world.entity.npc.Villager> farmers = level.getEntitiesOfClass(
                net.minecraft.world.entity.npc.Villager.class, new net.minecraft.world.phys.AABB(bell).inflate(64),
                v -> !v.isBaby() && residents.contains(v.getUUID())
                        && v.getVillagerData().getProfession() == net.minecraft.world.entity.npc.VillagerProfession.FARMER);
        if (farmers.isEmpty()) {
            return; // not loaded: it waits for them
        }
        int each = xp / farmers.size();
        if (each <= 0) {
            return;
        }
        for (var farmer : farmers) {
            farmer.setVillagerXp(farmer.getVillagerXp() + each);
            while (farmer.shouldIncreaseLevel()) {
                farmer.increaseMerchantCareer();
            }
        }
        holding.farmXp -= each * farmers.size();
    }

    /** Spends culture on the nearest unclaimed neighbouring hexes, one at a time. */
    private void grow(GlobalPos bell, Hex home) {
        Holding holding = holdings.get(bell);
        while (holding.culture >= nextHexCost(bell)) {
            Optional<Hex> next = hexes(bell).stream()
                    .flatMap(hex -> hex.neighbors().stream())
                    .distinct()
                    .filter(hex -> hex.distance(home) <= MAX_RINGS)
                    .filter(hex -> !owners.containsKey(new Place(bell.dimension(), hex)))
                    .min(Comparator.comparingDouble((Hex hex) -> Math.hypot(
                                    hex.centerX() - bell.pos().getX(), hex.centerZ() - bell.pos().getZ()))
                            .thenComparingInt(Hex::q)
                            .thenComparingInt(Hex::r));
            if (next.isEmpty()) {
                return;
            }
            double cost = nextHexCost(bell);
            claim(bell, next.get());
            holding.culture -= cost;
        }
    }

    /** As {@link #plan(ServerLevel, int, List, List, Map, Map)} with no demand known: the most-planted crops first. */
    public Map<String, Double> plan(ServerLevel level, int centerY, List<Hex> hexes, List<Integer> farmers) {
        return plan(level, centerY, hexes, farmers, Map.of(), Map.of());
    }

    /**
     * Farmers tend the plots the herds' feed needs first ({@code needs}, items a day), then those the village's demand
     * wants (the most valuable demand first), then the rest, the most-planted first; each farmer tends
     * {@link #PLOTS_PER_FARMER_LEVEL} plots a level. The crops growing set the produce. Returns items per day by id.
     */
    public Map<String, Double> plan(ServerLevel level, int centerY, List<Hex> hexes, List<Integer> farmers,
            Map<String, Double> demand, Map<String, Double> needs) {
        return plan(level, centerY, hexes, farmers, demand, needs, new LinkedHashMap<>(), 1);
    }

    /**
     * As {@link #plan(ServerLevel, int, List, List, Map, Map)}; what the plots no one tends would yield goes in
     * {@code idle}, and each farmer tends {@code hands} times its plots (more with tools).
     */
    public Map<String, Double> plan(ServerLevel level, int centerY, List<Hex> hexes, List<Integer> farmers,
            Map<String, Double> demand, Map<String, Double> needs, Map<String, Double> idle, double hands) {
        return plan(level, centerY, hexes, farmers, demand, Map.of(), needs, idle, hands);
    }

    /**
     * What the markets the town's caravans go to would take of its crops, item id -> each market's {items a day,
     * emeralds each}: a town's people's demand (its lockers' trades) for a crop beyond what it grows itself, or a
     * realm's (what its people use), at the crop's worth less the road's cost of carrying it there and back. Only
     * goods the far end's Merchant buys (see {@link dev.b28.enterprise.caravan.Caravans#wantsAt}).
     */
    public Map<String, List<double[]>> marketWants(MinecraftServer server, GlobalPos desk) {
        Map<String, List<double[]>> wants = new HashMap<>();
        for (var route : dev.b28.enterprise.caravan.CaravanLedger.get(server).routes()) {
            if (!route.from().equals(desk) || route.stopping()) {
                continue;
            }
            GlobalPos far = route.to();
            var buys = dev.b28.enterprise.caravan.Caravans.wantsAt(server, far).keySet();
            var realm = dev.b28.enterprise.trade.Realms.get(server).of(far);
            Optional<Holding> there = bellOf(far).flatMap(this::holding);
            for (Item crop : Crops.YIELDS.values().stream().distinct().toList()) {
                if (!buys.contains(crop)) {
                    continue;
                }
                String id = BuiltInRegistries.ITEM.getKey(crop).toString();
                double perDay = realm.isPresent() ? dev.b28.enterprise.trade.Realms.USES_PER_DAY
                        : there.map(h -> h.demand.getOrDefault(id, 0.0) - h.perDay.getOrDefault(id, 0.0)).orElse(0.0);
                double each = Prices.emeraldsEach(crop) - dev.b28.enterprise.caravan.Desires.haulEach(route, crop);
                if (perDay > 0 && each > 0) {
                    wants.computeIfAbsent(id, k -> new ArrayList<>()).add(new double[] {perDay, each});
                }
            }
        }
        return wants;
    }

    /** Field crops that share farmland, so a field can be turned over from one to another. */
    public static final List<Item> ROTATION = List.of(Items.WHEAT, Items.CARROT, Items.POTATO, Items.BEETROOT);

    /**
     * Crop rotation: plots of a crop grown beyond all its uses are turned over, a field at most a census, to the crop
     * owed under contract that's shortest of plots ({@code needs}: what it has sold forward, the herds' feed), whether
     * or not the town is {@code fed}; else, once it is, to the crop whose unmet use earns most (its own lockers', its
     * workshop's, the markets' its caravans reach).
     */
    private void rotate(ServerLevel level, int centerY, List<Hex> hexes, Map<String, Double> demand,
            Map<String, double[]> workshop, Map<String, List<double[]>> markets, Map<String, Double> needs,
            boolean fed, Map<String, Integer> turned, Map<String, Double> idle) {
        Map<Item, Integer> plots = plots(level, centerY, hexes, turned);
        Item from = null;
        int spare = 0;
        Item to = null;
        double toEach = 0;
        int short_ = 0;
        Item owedTo = null;
        int owedShort = 0;
        for (Item crop : ROTATION) {
            String id = BuiltInRegistries.ITEM.getKey(crop).toString();
            List<Use> uses = new ArrayList<>(usesOf(crop, demand, workshop));
            markets.getOrDefault(id, List.of()).forEach(m -> uses.add(new Use(crop, m[0], m[1])));
            double wanted = uses.stream().mapToDouble(Use::perDay).sum();
            double each = uses.stream().mapToDouble(Use::each).max().orElse(0);
            int needed = (int) Math.ceil(wanted / YIELD_PER_PLOT_PER_DAY);
            int have = plots.getOrDefault(crop, 0);
            if (have - needed > spare) {
                from = crop;
                spare = have - needed;
            }
            if (needed > have && each > toEach) {
                to = crop;
                toEach = each;
                short_ = needed - have;
            }
            int owed = (int) Math.ceil(needs.getOrDefault(id, 0.0) / YIELD_PER_PLOT_PER_DAY) - have;
            if (owed > owedShort) {
                owedTo = crop;
                owedShort = owed;
            }
        }
        if (owedTo != null) {
            // A promise first: from the crop with most plots lying untended, else the most planted.
            to = owedTo;
            short_ = owedShort;
            Item target = owedTo;
            Item idlest = ROTATION.stream().filter(c -> c != target).max(java.util.Comparator
                    .comparingDouble((Item c) -> idle.getOrDefault(BuiltInRegistries.ITEM.getKey(c).toString(), 0.0))
                    .thenComparingInt(c -> plots.getOrDefault(c, 0))).orElse(null);
            if (idlest != null && plots.getOrDefault(idlest, 0) > 0) {
                from = idlest;
                spare = plots.get(idlest);
            }
        } else if (!fed) {
            return; // nothing owed, and food first
        }
        if (from == null || to == null || from == to) {
            return;
        }
        int plotsPer = dev.b28.enterprise.Dials.ROTATE_PLOTS.getInt();
        int most = owedTo != null ? dev.b28.enterprise.Dials.OWED_FIELDS.getInt() * plotsPer : plotsPer; // more for a promise
        int plots_ = Math.min(most, Math.min(spare, short_));
        // In the ground where the fields are loaded; on the books for the rest, sown when next it's worked.
        int sown = Crops.resow(level, hexes, centerY, from, to, plots_);
        int booked = plots_ - sown;
        if (booked > 0) {
            turned.merge(BuiltInRegistries.ITEM.getKey(from).toString(), -booked, Integer::sum);
            turned.merge(BuiltInRegistries.ITEM.getKey(to).toString(), booked, Integer::sum);
        }
        if (plots_ > 0) {
            dev.b28.enterprise.Enterprise.LOGGER.debug("Fields turned over at {}: {} plots of {} to {}{} ({} on the books)",
                    hexes.isEmpty() ? "?" : hexes.get(0), plots_, BuiltInRegistries.ITEM.getKey(from),
                    BuiltInRegistries.ITEM.getKey(to), owedTo != null ? " (owed)" : "", booked);
        }
    }

    /** The hexes' plots by crop: those in the ground, and those turned over on the books (never fewer than none). */
    private Map<Item, Integer> plots(ServerLevel level, int centerY, List<Hex> hexes, Map<String, Integer> turned) {
        Map<Item, Integer> plots = new HashMap<>();
        for (Hex hex : hexes) {
            cropsIn(level, hex, centerY).forEach((item, count) -> plots.merge(item, count, Integer::sum));
        }
        turned.forEach((id, n) -> plots.merge(BuiltInRegistries.ITEM.get(ResourceLocation.parse(id)), n, Integer::sum));
        plots.replaceAll((item, n) -> Math.max(0, n));
        return plots;
    }

    /** One use of a crop: so many items a day, at what an item earns there. */
    private record Use(Item crop, double perDay, double each) {}

    /**
     * Farmers tend the plots the herds' feed needs first, then go plot by plot where the next item earns most: each use
     * of a crop (as itself, as what a trade makes of it, as its workshop's input, {@code workshop}: id -> {items a day,
     * emeralds each}) at what an item earns there, the dearest first, as many plots as that use takes; then the rest,
     * by what the crop earns, the most-planted first. A huge cheap market (paper for the Librarians' lockers) no longer
     * takes every hand from a small dear one (the cane and wheat for the town's biscuits).
     */
    public Map<String, Double> plan(ServerLevel level, int centerY, List<Hex> hexes, List<Integer> farmers,
            Map<String, Double> demand, Map<String, double[]> workshop, Map<String, Double> needs,
            Map<String, Double> idle, double hands) {
        return plan(level, centerY, hexes, farmers, demand, workshop, Map.of(), needs, idle, hands, false, Map.of());
    }

    /**
     * As above, with the markets its caravans reach as uses too ({@code markets}: id -> each market's {items a day,
     * emeralds each, less the road}, see {@link #marketWants}); and, once the town is {@code fed}, plots of a crop no
     * one wants lie fallow: not worked, not paid for, and no reason to take on another Farmer.
     */
    public Map<String, Double> plan(ServerLevel level, int centerY, List<Hex> hexes, List<Integer> farmers,
            Map<String, Double> demand, Map<String, double[]> workshop, Map<String, List<double[]>> markets,
            Map<String, Double> needs, Map<String, Double> idle, double hands, boolean fed, Map<String, Integer> turned) {
        Map<String, Double> perDay = new LinkedHashMap<>();
        Map<Item, Integer> plots = plots(level, centerY, hexes, turned);
        int capacity = (int) Math.round(hands * farmers.stream().mapToInt(l -> PLOTS_PER_FARMER_LEVEL * Math.max(1, l)).sum());
        Map<Item, Integer> left = new HashMap<>(plots);
        java.util.function.BiConsumer<Item, Integer> work = (crop, n) -> {
            perDay.merge(BuiltInRegistries.ITEM.getKey(crop).toString(), n * YIELD_PER_PLOT_PER_DAY, Double::sum);
            left.merge(crop, -n, Integer::sum);
        };
        // Feed first: as many plots of each feed crop as the herds eat.
        List<Item> crops = new ArrayList<>(plots.keySet());
        crops.sort(Comparator.comparing(c -> BuiltInRegistries.ITEM.getKey(c).toString()));
        for (Item crop : crops) {
            double need = needs.getOrDefault(BuiltInRegistries.ITEM.getKey(crop).toString(), 0.0);
            int worked = Math.min(capacity, Math.min(left.get(crop), (int) Math.ceil(need / YIELD_PER_PLOT_PER_DAY)));
            if (worked > 0) {
                work.accept(crop, worked);
                capacity -= worked;
            }
        }
        // Then each use, where an item earns most.
        List<Use> uses = new ArrayList<>();
        for (Item crop : crops) {
            uses.addAll(usesOf(crop, demand, workshop));
            for (double[] market : markets.getOrDefault(BuiltInRegistries.ITEM.getKey(crop).toString(), List.of())) {
                uses.add(new Use(crop, market[0], market[1]));
            }
        }
        uses.sort(Comparator.comparingDouble((Use u) -> -u.each()));
        Map<Item, Double> best = new HashMap<>();
        for (Use use : uses) {
            best.merge(use.crop(), use.each(), Math::max);
            int worked = Math.min(capacity, Math.min(left.get(use.crop()),
                    (int) Math.ceil(use.perDay() / YIELD_PER_PLOT_PER_DAY)));
            if (worked > 0) {
                work.accept(use.crop(), worked);
                capacity -= worked;
            }
        }
        // The rest: what's left of each crop's plots, the best-earning crop first, then the most planted.
        crops.sort(Comparator.comparingDouble((Item c) -> -best.getOrDefault(c, 0.0))
                .thenComparing(Comparator.comparingInt((Item c) -> plots.get(c)).reversed())
                .thenComparing(c -> BuiltInRegistries.ITEM.getKey(c).toString()));
        for (Item crop : crops) {
            if (fed && best.getOrDefault(crop, 0.0) <= 0 && needs.getOrDefault(
                    BuiltInRegistries.ITEM.getKey(crop).toString(), 0.0) <= 0) {
                continue; // no one wants it, and the town has its food: fallow
            }
            int worked = Math.min(capacity, left.get(crop));
            if (worked > 0) {
                work.accept(crop, worked);
                capacity -= worked;
            }
            if (left.get(crop) > 0) {
                idle.merge(BuiltInRegistries.ITEM.getKey(crop).toString(), left.get(crop) * YIELD_PER_PLOT_PER_DAY,
                        Double::sum);
            }
        }
        return perDay;
    }

    /**
     * A crop's uses: as itself at its price; as what a trade makes of it alone (paper of cane, bread of wheat) at that
     * good's price per item of the crop; and as its workshop's input at the share of the output's worth it earns.
     */
    private static List<Use> usesOf(Item crop, Map<String, Double> demand, Map<String, double[]> workshop) {
        String id = BuiltInRegistries.ITEM.getKey(crop).toString();
        List<Use> uses = new ArrayList<>();
        if (demand.getOrDefault(id, 0.0) > 0) {
            uses.add(new Use(crop, demand.get(id), Prices.emeraldsEach(crop)));
        }
        for (Workshop.Conversion c : Workshop.CONVERSIONS) {
            String made = BuiltInRegistries.ITEM.getKey(c.output()).toString();
            if (c.input() == crop && c.input2() == null && demand.getOrDefault(made, 0.0) > 0) {
                uses.add(new Use(crop, demand.get(made) * c.in() / c.out(),
                        Prices.emeraldsEach(c.output()) * c.out() / c.in()));
            }
        }
        double[] drawn = workshop.get(id);
        if (drawn != null && drawn[0] > 0) {
            uses.add(new Use(crop, drawn[0], drawn[1]));
        }
        return uses;
    }

    /** Recounts the hex's loaded chunks, keeps the last count of the rest, and returns the hex's plots. */
    private Map<Item, Integer> cropsIn(ServerLevel level, Hex hex, int centerY) {
        Map<Long, Map<String, Integer>> chunks = fields.computeIfAbsent(new Place(level.dimension(), hex), p -> new HashMap<>());
        for (long chunk : Crops.chunksOf(hex)) {
            Map<Item, Integer> fresh = Crops.countChunk(level, chunk, hex, centerY);
            if (fresh == null) {
                continue; // not loaded: keep what was counted last time
            }
            if (fresh.isEmpty()) {
                chunks.remove(chunk);
            } else {
                Map<String, Integer> byId = new LinkedHashMap<>();
                fresh.forEach((item, count) -> byId.put(BuiltInRegistries.ITEM.getKey(item).toString(), count));
                chunks.put(chunk, byId);
            }
        }
        setDirty();
        Map<Item, Integer> plots = new HashMap<>();
        chunks.values().forEach(counts -> counts.forEach((id, count) ->
                plots.merge(BuiltInRegistries.ITEM.get(ResourceLocation.parse(id)), count, Integer::sum)));
        return plots;
    }

    /** Chunks holding a settlement's known fields: what a round loads so the census reads them fresh. */
    public java.util.Set<Long> fieldChunks(GlobalPos bell) {
        java.util.Set<Long> chunks = new java.util.LinkedHashSet<>();
        for (Hex hex : hexes(bell)) {
            chunks.addAll(fields.getOrDefault(new Place(bell.dimension(), hex), Map.of()).keySet());
        }
        return chunks;
    }

    private static int total(Map<Item, Integer> plots) {
        return plots.values().stream().mapToInt(Integer::intValue).sum();
    }

    /**
     * Adds produce, and the farmers' wages for it, for the time since the last harvest; then moves the farmers'
     * share toward the fraction of the harvest that sold.
     */
    private static void accrue(Holding holding, long now) {
        if (holding.lastHarvest >= 0) {
            double days = Math.min(MAX_DAYS, (now - holding.lastHarvest) / 24000.0);
            double worked = days * yieldFactor(holding.mood); // content farmers work harder
            holding.perDay.forEach((item, rate) -> holding.pending.merge(item, rate * worked, Double::sum));
            holding.wageOwed += wagePerDay(holding) * days;
            if (days > 0) {
                double weight = 1 - Math.pow(0.5, days / SALES_HALF_LIFE_DAYS);
                holding.salesPerDay += (holding.salesSince / days - holding.salesPerDay) * weight;
                holding.valuePerDay += (valuePerDay(holding.perDay) - holding.valuePerDay) * weight;
                holding.salesSince = 0;
                if (holding.valuePerDay > 0) {
                    double target = Math.max(MIN_LABOR_SHARE, Math.min(MAX_LABOR_SHARE,
                            holding.salesPerDay / holding.valuePerDay * holding.labourTightness));
                    holding.laborShare += (target - holding.laborShare)
                            * (1 - Math.pow(0.5, days / SHARE_HALF_LIFE_DAYS));
                }
            }
        }
        holding.lastHarvest = now;
    }

    /** The store the town has built that banks {@code item}, if any. */
    private static Optional<Buildings.Store> storeFor(Holding holding, Item item) {
        return Buildings.STORES.stream()
                .filter(s -> s.items().contains(item) && holding.buildings.contains(s.kind().id())).findFirst();
    }

    /** What's banked in a store, of all it keeps. */
    private static double inStore(Holding holding, Buildings.Store store) {
        return store.items().stream()
                .mapToDouble(i -> holding.banked.getOrDefault(BuiltInRegistries.ITEM.getKey(i).toString(), 0.0)).sum();
    }

    /** What the warehouse keeps loose of a crop beside its store: its par, and at least two stacks for its workshop. */
    private static int loose(ServerLevel level, GlobalPos desk, Item item, long now) {
        return Math.max(2 * 64, dev.b28.enterprise.merchant.Warehouse.get(level.getServer())
                .par(desk, item, true, now));
    }

    /**
     * Does the town grow more of a store's crops than it keeps: twice what it keeps loose in the warehouse, or a
     * stack of harvest waiting for room there? Only then is a granary, or a root cellar, worth building.
     */
    public boolean wantsStore(ServerLevel level, GlobalPos desk, MerchantStock stock, Buildings.Store store, long now) {
        Optional<Holding> holding = bellOf(desk).flatMap(this::holding);
        if (holding.isEmpty()) {
            return false;
        }
        for (Item item : store.items()) {
            String id = BuiltInRegistries.ITEM.getKey(item).toString();
            if (stock.count(new ItemCost(item, 1)) >= 2 * loose(level, desk, item, now)
                    || holding.get().pending.getOrDefault(id, 0.0) >= item.getDefaultMaxStackSize()) {
                return true;
            }
        }
        return false;
    }

    /**
     * With a store: crops beyond what the warehouse keeps loose go into it (wheat as bales in the granary, roots in
     * the cellar's barrels), out of the way; and it tops the warehouse back up as its lockers, its workshop and its
     * caravans draw them down. A crop waits there until someone wants it.
     */
    private static void bank(ServerLevel level, GlobalPos desk, MerchantStock stock, Holding holding, long now) {
        for (Buildings.Store store : Buildings.STORES) {
            if (!holding.buildings.contains(store.kind().id())) {
                continue;
            }
            for (Item item : store.items()) {
                String id = BuiltInRegistries.ITEM.getKey(item).toString();
                int keep = loose(level, desk, item, now);
                int held = stock.count(new ItemCost(item, 1));
                if (held > keep) {
                    int in = (int) Math.min(held - keep, store.holds() - inStore(holding, store));
                    if (in > 0) {
                        int taken = stock.take(new ItemCost(item, 1), in).stream().mapToInt(ItemStack::getCount).sum();
                        holding.banked.merge(id, (double) taken, Double::sum);
                    }
                    continue;
                }
                int out = (int) Math.min(keep - held, Math.floor(holding.banked.getOrDefault(id, 0.0)));
                while (out > 0) {
                    ItemStack stack = new ItemStack(item, Math.min(out, item.getDefaultMaxStackSize()));
                    if (!stock.hasRoomFor(stack)) {
                        break;
                    }
                    int put = stack.getCount() - stock.insert(stack).getCount();
                    holding.banked.merge(id, (double) -put, Double::sum);
                    out -= put;
                    if (put == 0) {
                        break;
                    }
                }
            }
        }
    }

    /** Wheat banked in the town's granary. */
    public double grain(GlobalPos bell) {
        return banked(bell, Items.WHEAT);
    }

    /** What the town has banked of a crop in its stores. */
    public double banked(GlobalPos bell, Item item) {
        return holding(bell).map(h -> h.banked.getOrDefault(BuiltInRegistries.ITEM.getKey(item).toString(), 0.0))
                .orElse(0.0);
    }

    /**
     * Pays the farmers the whole emeralds owed: emeralds from the Merchant's chest first, then its coins at the
     * region's rate, then the settlement's Blaze Banker. Returns the share of the wage paid (1 when nothing whole is
     * owed yet). What's paid is the households' (see {@link Households}); what the Merchant can't pay it owes them as
     * credit, up to {@link Households#CREDIT_DAYS} days' pay, and the farmers keep working. Beyond that, the farmers
     * take their wage in kind, as sharecroppers do: their share of the crop is theirs, and the town gets the rest.
     */
    private double payWages(ServerLevel level, BlockPos desk, MerchantStock stock, Holding holding, long now) {
        int whole = (int) Math.floor(holding.wageOwed);
        if (whole <= 0) {
            return 1.0;
        }
        Mint mint = Mint.get(level.getServer());
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        double paid = Math.min(whole, Treasury.withdraw(level, desk, stock, whole, mint, now).value(rate));
        holding.purse += paid; // the households' now: to spend at the market
        // Short of coin, the Merchant pays in credit, as far as the farmers will wait.
        double room = Math.max(0, Households.CREDIT_DAYS * valuePerDay(holding.perDay) * LABOR_SHARE - holding.credit);
        double credited = Math.min(whole - paid, room);
        holding.credit += credited;
        holding.wageOwed -= whole;
        holding.wagesPaid += paid;
        holding.wagesUnpaid += whole - paid - credited;
        return (paid + credited) / whole;
    }

    /** The market at {@code desk} was touched: hand over the produce and tolls owed to its settlement. */
    public void touch(ServerLevel level, BlockPos desk, MerchantStock stock) {
        touch(level, desk, stock, level.getGameTime());
    }

    /** As {@link #touch(ServerLevel, BlockPos, MerchantStock)}, at game time {@code now}. */
    public void touch(ServerLevel level, BlockPos desk, MerchantStock stock, long now) {
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        dev.b28.enterprise.market.ColdStore.tidy(level, at, stock, now); // room first: packed, stored, given, thrown out
        dev.b28.enterprise.cargo.Paperwork.get(level.getServer()).settle(level, at, stock, now); // its papers' stationery
        dev.b28.enterprise.market.Post.atDesk(level, at, stock, now); // its mail read, and its letters written
        Optional<GlobalPos> bell = bellOf(at);
        if (bell.isEmpty()) {
            return;
        }
        Holding holding = holdings.get(bell.get());
        dev.b28.enterprise.money.CashBook.get(level.getServer()).level(at, now, Treasury.available(level, desk, stock,
                Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now))); // the day's opening balance
        upkeepGuards(level, desk, stock, bell.get(), holding, now); // pay the guards who served, then review the garrison
        long lastReview = holding.lastTollReview;
        reviewTolls(bell.get(), now, Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now));
        holding.tradingCapital = dev.b28.enterprise.merchant.Warehouse.get(level.getServer()).tradingCapital(at, now);
        Works works = Works.get(level.getServer());
        works.work(level, desk, stock, bell.get(), hexes(bell.get()).size(), now); // masons' wages and progress
        holding.walled = works.walled(bell.get());
        holding.buildings = new java.util.LinkedHashSet<>(works.builtOf(bell.get()));
        if (holding.lastTollReview != lastReview && lastReview >= 0) {
            works.consider(level, desk, stock, bell.get(), this, now); // with the tolls: is a bridge worth building?
            dev.b28.enterprise.caravan.Commissions.consider(level, at, stock, bell.get(), this, now); // a caravan of its own?
            dev.b28.enterprise.service.BuildServices.hire(level, at, stock, bell.get(), this, now); // a hall, by hired builders
        }
        long since = holding.lastHarvest;
        accrue(holding, now);
        if (holding.lastTend >= 0) {
            double days = Math.min(MAX_DAYS, (now - holding.lastTend) / 24000.0);
            Husbandry.tend(stock, holding.herd, holding.fodder, trades(level.getServer(), bell.get()), days,
                    Husbandry.capacity(pastures(holding).size()), holding.pending);
            if (holding.dock != null) {
                // The Fishermen's catch: so many cod a day per level each.
                double levels = dev.b28.enterprise.settlement.Settlements.get(level.getServer()).roster(bell.get()).values()
                        .stream().filter(r -> r.profession().equals("minecraft:fisherman") && !r.away() && !r.baby())
                        .mapToInt(r -> Math.max(1, r.level())).sum();
                holding.pending.merge("minecraft:cod", levels * Docks.COD_PER_LEVEL_DAY * days
                        * (holding.buildings.contains(Buildings.STONE_DOCKS.id()) ? 2 : 1), Double::sum);
            }
            Map<String, Integer> workers = workers(level.getServer(), bell.get());
            if (holding.mine != null) {
                // The Toolsmiths' ore: so much a day per level each, half again with pickaxes.
                double levels = dev.b28.enterprise.settlement.Settlements.get(level.getServer()).roster(bell.get()).values()
                        .stream().filter(r -> r.profession().equals(TOOLSMITH) && !r.away() && !r.baby())
                        .mapToInt(r -> Math.max(1, r.level())).sum()
                        * (1 + Smithing.TOOL_BONUS * holding.kitShare.getOrDefault(Smithing.PICKAXE.id(), 0.0))
                        * (holding.buildings.contains(Buildings.SMITHY.id()) ? 1.5 : 1);
                holding.pending.merge("minecraft:raw_iron", levels * Mines.RAW_IRON_PER_LEVEL_DAY * days, Double::sum);
                holding.pending.merge("minecraft:coal", levels * Mines.COAL_PER_LEVEL_DAY * days, Double::sum);
                holding.pending.merge("minecraft:raw_gold", levels * Mines.RAW_GOLD_PER_LEVEL_DAY * days, Double::sum);
                if (holding.deepMine) {
                    holding.pending.merge("minecraft:lapis_lazuli", levels * Mines.LAPIS_PER_LEVEL_DAY * days, Double::sum);
                }
            }
            // Its hands gather, more with tools to hand (given them from the warehouse), which wear out.
            List<Double> hands = Labour.hands(dev.b28.enterprise.settlement.Settlements.get(level.getServer()).roster(bell.get()));
            double tooled = Math.min(hands.size(), holding.handTools);
            double withTools = hands.isEmpty() ? 0 : tooled / hands.size();
            Labour.work(level, at, hexes(bell.get()),
                    hands.stream().map(h -> h * (1 + Smithing.TOOL_BONUS * withTools)).toList(),
                    Math.min(holding.militia, holding.posts.size()), stock, days, holding.pending);
            holding.handTools = Math.max(0, holding.handTools - tooled * days / HAND_TOOL_DAYS);
            // Its Fletchers hunt: so much game a day per level each, a quarter of it without bows.
            double hunterLevels = levels(level.getServer(), bell.get(), FLETCHER)
                    * (Smithing.SNARES + (1 - Smithing.SNARES) * holding.hunting)
                    * (bestLevel(level.getServer(), bell.get(), FLETCHER) >= 5 ? 2 : 1); // a master leads a hunting party
            if (hunterLevels > 0) {
                for (Labour.Haul game : Hunting.game(Labour.biome(level, Hex.at(bell.get().pos())))) {
                    if (!Labour.room(level, at, stock, game.item(), holding.pending)) {
                        continue;
                    }
                    holding.pending.merge(BuiltInRegistries.ITEM.getKey(game.item()).toString(),
                            game.perDay() * hunterLevels * days, Double::sum);
                }
            }
            Smithing.wear(stock, holding.kitWear, days, workers.getOrDefault("minecraft:farmer", 0), miners(holding, workers),
                    holding.posts.size(), workers.getOrDefault(FLETCHER, 0), holding.mendedUntil >= now ? 0.5 : 1,
                    holding.buildings.contains(Buildings.SMITHY.id()) ? 0.8 : 1, holding.armouryUntil >= now ? 0.5 : 1);
        }
        holding.lastTend = now;
        double owedBefore = holding.wageOwed;
        double paidShare = payWages(level, desk, stock, holding, now);
        long harvestedBefore = holding.harvested;
        if (paidShare < 1) {
            // Unpaid, the farmers take their wage in kind: their share of those days' crop. The town gets the rest.
            double value = valuePerDay(holding.perDay);
            double share = value <= 0 ? 1 : Math.min(1, wagePerDay(holding) / value);
            double kept = 1 - (1 - paidShare) * share;
            holding.pending.replaceAll((item, owed) -> isCrop(item) ? owed * kept : owed);
        }
        for (Map.Entry<String, Double> owed : holding.pending.entrySet()) {
            int whole = (int) Math.floor(owed.getValue());
            if (whole <= 0) {
                continue;
            }
            Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(owed.getKey()));
            int left = whole;
            Optional<Buildings.Store> store = storeFor(holding, item);
            if (store.isPresent()) {
                // What the warehouse needn't keep loose goes straight to the granary, or the root cellar.
                int short_ = Math.max(0, loose(level, at, item, now) - stock.count(new ItemCost(item, 1)));
                int banked = (int) Math.max(0, Math.min(left - short_, store.get().holds() - inStore(holding, store.get())));
                holding.banked.merge(owed.getKey(), (double) banked, Double::sum);
                left -= banked;
            }
            while (left > 0) {
                ItemStack stack = new ItemStack(item, Math.min(left, item.getDefaultMaxStackSize()));
                if (!stock.hasRoomFor(stack)) {
                    break; // the warehouse keeps room for locker results and coins: the rest stays owed
                }
                int put = stack.getCount();
                ItemStack rest = stock.insert(stack);
                left -= put - rest.getCount();
                if (!rest.isEmpty()) {
                    break; // the chest is full: the rest stays owed
                }
            }
            owed.setValue(owed.getValue() - (whole - left));
            holding.harvested += whole - left;
            if (isCrop(owed.getKey())) {
                // The Farmers learn their trade by working it: the harvest earns them what selling it would.
                holding.farmXp += (whole - left) * Prices.emeraldsEach(item) * HARVEST_XP_PER_EMERALD;
            }
        }
        holding.paidShare = paidShare;
        bank(level, at, stock, holding, now); // before the workshop: the wheat it bakes with
        settleDeposits(level, at, stock, holding, now);
        dev.b28.enterprise.caravan.ForwardSales.earmark(level, at, stock, now); // what it owes, crated for its buyer
        Workshop.convert(level, dev.b28.enterprise.merchant.Warehouse.get(level.getServer()), at, bell.get(), stock, now);
        dev.b28.enterprise.service.Services.home(level, this, at, bell.get(), stock, now); // what its own trades do for it
        Map<String, Integer> workers = workers(level.getServer(), bell.get());
        int hunters = workers.getOrDefault(FLETCHER, 0);
        for (Smithing.Kit kit : Smithing.KITS) {
            holding.kitShare.put(kit.id(), Smithing.share(stock, kit, Smithing.wearers(kit,
                    workers.getOrDefault("minecraft:farmer", 0), miners(holding, workers), holding.posts.size(), hunters)));
        }
        holding.hunting = Smithing.hunting(stock, hunters);
        holding.armed = Smithing.armed(stock, holding.posts.size(), hunters,
                holding.buildings.contains(Buildings.WATCHTOWER.id()) ? 0.5 : 0);
        // The comforts the town used: the households pay for them, as far as their purse goes.
        householdsPay(level, desk, stock, holding, enjoyComforts(stock, holding, now));
        // Its shop: what it's after, listed at what it would pay.
        dev.b28.enterprise.market.ShopBoard.get(level.getServer()).keep(level, at, stock, now, this);
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now);
        int bankSpurs = MoneyChanger.bankAt(level, desk).map(b -> b.getBalance()).orElse(0);
        holding.treasury = stock.count(new ItemCost(Items.EMERALD, 1))
                + (MoneyChanger.value(stock.looseStacks()) + bankSpurs) / rate;
        dev.b28.enterprise.Enterprise.LOGGER.debug(
                "Farm touch at {}: {} days since last, wages owed {}, paid share {}, delivered {}",
                desk.toShortString(), since < 0 ? "first" : String.format("%.2f", (now - since) / 24000.0),
                String.format("%.2f", owedBefore), String.format("%.2f", paidShare), holding.harvested - harvestedBefore);
        if (holding.tollsOwed > 0) {
            var bank = MoneyChanger.bankAt(level, desk);
            if (bank.isPresent()) {
                bank.get().deposit((int) holding.tollsOwed);
                holding.tollsOwed = 0;
            } else if (stock.fits(MoneyChanger.coins((int) holding.tollsOwed))) {
                MoneyChanger.coins((int) holding.tollsOwed).forEach(stock::insert);
                holding.tollsOwed = 0;
            }
        }
        setDirty();
    }

    /** A caravan crossing this settlement's land paid it a toll; it arrives at the next touch. */
    public void oweToll(GlobalPos bell, int spurs) {
        Holding holding = holdings.computeIfAbsent(bell, b -> new Holding());
        holding.tollsOwed += spurs;
        holding.tollIncomeToday += spurs;
        setDirty();
    }

    /**
     * The town's villagers use their daily ration of each kind of comfort from the warehouse, for the days since the
     * last touch; rations that can't be met are gone without, not owed. Then the mood moves toward what that,
     * the town's size, its beds and its wages make it.
     */
    /**
     * A household with {@link Households#LAND_EMERALDS} in purse and credit, where the farmers have hands for a third
     * of a field more, buys a field of its own: what the town owes it first, then its money, paid to the market.
     */
    private void buyLand(ServerLevel level, BlockPos bellPos, Holding holding, MerchantStock stock) {
        if (holding.purse + holding.credit < Households.LAND_EMERALDS
                || holding.spareFarmPlots < Fields.SIZE * Fields.SIZE / 3) {
            return;
        }
        Optional<Integer> plots = Fields.freehold(level, bellPos, holding.focus, holding.demand, holding.perDay, stock);
        if (plots.isEmpty()) {
            return;
        }
        double fromCredit = Math.min(holding.credit, Households.LAND_EMERALDS);
        holding.credit -= fromCredit;
        double rest = Households.LAND_EMERALDS - fromCredit;
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        double paid = Households.payIn(level, holding.desk.pos(), stock, Math.min(rest, holding.purse), rate);
        holding.purse -= paid;
        holding.householdSpent += paid;
        holding.freeholds += plots.get();
        holding.fieldsPlanted++;
        dev.b28.enterprise.Enterprise.LOGGER.debug("Settlement at {}: a household buys a field of its own ({} plots; {} freehold)",
                bellPos.toShortString(), plots.get(), holding.freeholds);
    }

    /**
     * What the Merchant could still borrow of its households' savings now, in emeralds: its share of all they have
     * (in purse and lent it), less what it has borrowed already, as far as their purse goes.
     */
    public double depositsOnCall(GlobalPos desk) {
        return bellOf(desk).flatMap(this::holding).map(h -> Math.max(0, Math.min(h.purse,
                dev.b28.enterprise.Dials.DEPOSIT_SHARE.get() * (h.purse + h.deposits) - h.deposits))).orElse(0.0);
    }

    /** What the Merchant owes its households on their savings, in emeralds. */
    public double deposits(GlobalPos bell) {
        return holding(bell).map(h -> h.deposits).orElse(0.0);
    }

    /**
     * The Merchant borrows up to {@code emeralds} of its households' savings (as far as {@link #depositsOnCall}), into
     * its chest: to honour a bill put to it, or buy paper a town can't collect. Owed them back with interest. Returns
     * what came in.
     */
    public double borrowDeposits(ServerLevel level, GlobalPos desk, MerchantStock stock, double emeralds) {
        Optional<Holding> holding = bellOf(desk).flatMap(this::holding);
        double take = Math.min(emeralds, depositsOnCall(desk));
        if (holding.isEmpty() || take <= 0.01) {
            return 0;
        }
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        double paid = Households.payIn(level, desk.pos(), stock, take, rate);
        if (holding.get().deposits <= 0.01) {
            holding.get().lastDeposits = level.getGameTime(); // interest runs from now
        }
        holding.get().purse -= paid;
        holding.get().deposits += paid;
        setDirty();
        dev.b28.enterprise.Enterprise.LOGGER.info("{} borrows {} emeralds of its households' savings",
                dev.b28.enterprise.settlement.Names.label(desk), String.format("%.1f", paid));
        return paid;
    }

    /** Interest on what the Merchant owes its households; and it repays them from what it can spare beyond its keep. */
    private void settleDeposits(ServerLevel level, GlobalPos desk, MerchantStock stock, Holding holding, long now) {
        if (holding.deposits <= 0.01) {
            return;
        }
        double days = Math.max(0, (now - holding.lastDeposits) / 24000.0);
        holding.lastDeposits = now;
        holding.deposits *= Math.pow(1 + dev.b28.enterprise.Dials.DEPOSIT_INTEREST.get(), Math.min(MAX_DAYS, days));
        Mint mint = Mint.get(level.getServer());
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        double spare = Treasury.available(level, desk.pos(), stock, rate) - reserve(desk)
                - dev.b28.enterprise.Dials.DEPOSIT_KEEP.get();
        double repay = Math.min(holding.deposits, spare);
        if (repay > 0.01) {
            Treasury.Withdrawal paid = Treasury.withdraw(level, desk.pos(), stock, repay, mint, now);
            double value = paid.value(rate);
            holding.deposits -= value;
            holding.purse += value;
        }
        setDirty();
    }

    /** The households pay the market for what they use, as far as their purse goes. */
    private static void householdsPay(ServerLevel level, BlockPos desk, MerchantStock stock, Holding holding, double emeralds) {
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        double paid = Households.payIn(level, desk, stock, Math.min(holding.purse, emeralds), rate);
        holding.purse -= paid;
        holding.householdSpent += paid;
    }

    private double enjoyComforts(MerchantStock stock, Holding holding, long now) {
        if (holding.lastComforts < 0) {
            holding.lastComforts = now;
            return 0;
        }
        double days = Math.min(MAX_DAYS, (now - holding.lastComforts) / 24000.0);
        if (days <= 0) {
            return 0;
        }
        holding.lastComforts = now;
        double used = 0;
        for (var kind : Comforts.kinds()) {
            String name = Comforts.name(kind);
            double due = holding.villagers * Comforts.RATION_EMERALDS * days;
            double owed = holding.comfortOwed.getOrDefault(name, 0.0) + due;
            if (owed > 0) {
                double took = Comforts.use(stock, kind, owed);
                owed -= took;
                used += took;
            }
            holding.comfortMet.put(name, due <= 0 ? 0 : Math.max(0, Math.min(1, 1 - Math.max(0, owed) / due)));
            holding.comfortOwed.put(name, Math.min(0, owed));
        }
        holding.mood += (moodTarget(holding) - holding.mood) * (1 - Math.pow(0.5, days / MOOD_HALF_LIFE_DAYS));
        return used;
    }

    /** Where the town's mood is heading: comforts met, its wall and any blessing, less its size, villagers without beds, unpaid wages. */
    private static double moodTarget(Holding holding) {
        double comforts = holding.comfortMet.values().stream().mapToDouble(Double::doubleValue).sum();
        return comforts
                + Works.WALL_MOOD * holding.walled
                + (holding.blessedUntil >= holding.lastComforts
                        ? BLESSING_MOOD * (holding.buildings.contains(Buildings.TEMPLE.id()) ? 2 : 1) : 0)
                - Math.min(GRIEF_MOST, GRIEF_MOOD * holding.fallen.stream()
                        .filter(t -> holding.lastComforts - t < GRIEF_TICKS).count())
                + (holding.buildings.contains(Buildings.SQUARE.id())
                        ? Buildings.SQUARE_MOOD + (holding.flowerBeds ? Buildings.FLOWER_MOOD : 0) : 0)
                - holding.villagers / VILLAGERS_PER_WANT
                - CROWDING_MOOD * Math.max(0, holding.villagers - holding.beds)
                - UNPAID_MOOD * (1 - holding.paidShare);
    }

    /**
     * A content town counted while loaded: with every bed taken it works toward a home, with a bed free toward a child
     * (see {@link Growth}), for the days since its last census. Each is paid for when due, if the warehouse can: a home
     * from money beyond the town's reserve, a child from that and food beyond the caravans' par.
     */
    private void grow(ServerLevel level, BlockPos bell, Holding holding, Census census, double days, MerchantStock stock) {
        days = Math.min(MAX_DAYS, Math.max(0, days));
        // Squads before anything; then work for whoever is still idle, unless the town is raising a party of settlers
        // (its idle hands wait for that).
        var expeditions = dev.b28.enterprise.caravan.Expeditions.get(level.getServer());
        boolean raising = expeditions.raising(holding.desk, holding.focus, census.villagers(), level.getGameTime());
        farmersLearn(level, bell, holding);
        if (dev.b28.enterprise.caravan.Commissions.train(level, holding.desk, bell, stock, this).isEmpty()
                && !dev.b28.enterprise.caravan.Commissions.squadShort(level, holding.desk, bell) && !raising) {
            Jobs.employ(level, bell, holding.desk, stock, this);
        }
        BlockPos desk = holding.desk.pos();
        Mint mint = Mint.get(level.getServer());
        long now = level.getGameTime();
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        // Grown, with hands and money to spare: settlers, to a village nobody trades with or to found a town.
        expeditions.consider(level, holding.desk,
                GlobalPos.of(level.dimension(), bell), stock, this, holding.focus, census.villagers(), now);
        if (holding.mood >= dev.b28.enterprise.Dials.HOME_MOOD.get() && census.villagers() >= census.beds()) {
            holding.homeProgress = Math.min(1, holding.homeProgress + days / Growth.DAYS_PER_HOME);
        }
        if (holding.homeProgress >= 1 && holding.purse
                + Treasury.available(level, desk, stock, rate) - reserve(holding.desk) >= Growth.HOME_EMERALDS) {
            Optional<BlockPos> site = Growth.homeSite(level, bell);
            if (site.isPresent()) {
                Growth.buildHome(level, site.get());
                // A family's home, paid for by the households as far as they can; the town pays the rest.
                double theirs = Math.min(holding.purse, Growth.HOME_EMERALDS);
                holding.purse -= theirs;
                holding.householdSpent += theirs;
                Treasury.withdraw(level, desk, stock, Growth.HOME_EMERALDS - theirs, mint, now);
                holding.homeProgress = 0;
                holding.homesBuilt++;
                dev.b28.enterprise.Enterprise.LOGGER.debug("Settlement at {} built a home at {}",
                        bell.toShortString(), site.get().toShortString());
            }
        }
        if (holding.mood >= dev.b28.enterprise.Dials.BIRTH_MOOD.get() && census.beds() > census.villagers()) {
            holding.birthProgress = Math.min(1, holding.birthProgress + days / Growth.DAYS_PER_BIRTH
                    * (holding.buildings.contains(Buildings.TEMPLE.id()) ? 2 : 1));
        }
        if (holding.birthProgress >= 1
                && stock.foodNutrition() >= dev.b28.enterprise.merchant.Warehouse.FOOD_PAR_POINTS + Growth.BIRTH_FOOD_POINTS
                && holding.purse + Treasury.available(level, desk, stock, rate) - reserve(holding.desk) >= Growth.BIRTH_EMERALDS) {
            stock.eat(Growth.BIRTH_FOOD_POINTS);
            // The family pays the market for the food, and what it can of the rest; the town the remainder.
            householdsPay(level, desk, stock, holding, Growth.BIRTH_FOOD_POINTS / 5.0
                    * dev.b28.enterprise.money.Prices.emeraldsEach(Items.BREAD));
            double theirs = Math.min(holding.purse, Growth.BIRTH_EMERALDS);
            holding.purse -= theirs;
            holding.householdSpent += theirs;
            Treasury.withdraw(level, desk, stock, Growth.BIRTH_EMERALDS - theirs, mint, now);
            Growth.bear(level, bell);
            holding.birthProgress = 0;
            holding.born++;
            dev.b28.enterprise.Enterprise.LOGGER.debug("Settlement at {} had a child", bell.toShortString());
        }
    }

    /** What the town puts first (see {@link Focus}). */
    public Focus focus(GlobalPos bell) {
        return holding(bell).map(h -> h.focus).orElse(Focus.GROW);
    }

    public int homesBuilt(GlobalPos bell) {
        return holding(bell).map(h -> h.homesBuilt).orElse(0);
    }

    public int born(GlobalPos bell) {
        return holding(bell).map(h -> h.born).orElse(0);
    }

    /** How the town feels: above 0 content, below 0 unhappy. */
    public double mood(GlobalPos bell) {
        return holding(bell).map(h -> h.mood).orElse(0.0);
    }

    /** A Cleric blesses the town until {@code until} (or longer, if already blessed longer). */
    public void bless(GlobalPos bell, long until) {
        Holding holding = holdings.computeIfAbsent(bell, b -> new Holding());
        holding.blessedUntil = Math.max(holding.blessedUntil, until);
        setDirty();
    }

    public long blessedUntil(GlobalPos bell) {
        return holding(bell).map(h -> h.blessedUntil).orElse(-1L);
    }

    /** What the town's untended plots would yield a day, by item id, at its last census. */
    public Map<String, Double> idlePerDay(GlobalPos bell) {
        return holding(bell).map(h -> Map.copyOf(h.idlePerDay)).orElse(Map.of());
    }

    public void setIdleForTest(GlobalPos bell, Map<String, Double> idle, int spare) {
        Holding holding = holdings.computeIfAbsent(bell, b -> new Holding());
        holding.idlePerDay = new LinkedHashMap<>(idle);
        holding.spareFarmPlots = spare;
    }

    /** Plots the town's farmers could tend beyond those planted, at its last census. */
    public int spareFarmPlots(GlobalPos bell) {
        return holding(bell).map(h -> h.spareFarmPlots).orElse(0);
    }

    /** Produce owed to the town's warehouse (help with the harvest): delivered at its next touch. */
    public void owe(GlobalPos bell, Map<String, Double> produce) {
        Holding holding = holdings.computeIfAbsent(bell, b -> new Holding());
        produce.forEach((item, count) -> holding.pending.merge(item, count, Double::sum));
        setDirty();
    }

    public void setMood(GlobalPos bell, double mood) {
        holdings.computeIfAbsent(bell, b -> new Holding()).mood = mood;
        setDirty();
    }

    /** Share of each kind's ration met at the last touch, by kind. */
    public Map<String, Double> comfortsMet(GlobalPos bell) {
        return holding(bell).map(h -> Map.copyOf(h.comfortMet)).orElse(Map.of());
    }

    /** Farm output for a mood: a tenth more per point, between half and one and a half. */
    public static double yieldFactor(double mood) {
        return Math.max(0.5, Math.min(1.5, 1 + 0.1 * mood));
    }

    /** Culture for a mood: a quarter more per point, between a quarter and double. */
    public static double cultureFactor(double mood) {
        return Math.max(0.25, Math.min(2, 1 + 0.25 * mood));
    }

    public int villagers(GlobalPos bell) {
        return holding(bell).map(h -> h.villagers).orElse(0);
    }

    public int beds(GlobalPos bell) {
        return holding(bell).map(h -> h.beds).orElse(0);
    }

    /** What the warehouse at {@code desk} aims to hold of each kind of comfort, in emeralds' worth. */
    public double comfortPar(GlobalPos desk) {
        return bellOf(desk).map(bell -> holdings.get(bell).villagers * Comforts.RATION_EMERALDS
                * dev.b28.enterprise.Dials.COMFORT_DAYS.get())
                .orElse(0.0);
    }

    /**
     * Money the town keeps back before spending on comforts, homes, children or works, in emeralds: its wage float,
     * three days of guards' pay, and its trading capital (two days of what its lockers sell, on average). A town that spent its
     * trading capital couldn't buy the goods that earn its money, and would stay poor.
     */
    public double reserve(GlobalPos desk) {
        return payroll(desk) + bellOf(desk).map(bell -> holdings.get(bell).tradingCapital).orElse(0.0);
    }

    /**
     * Money the town keeps back even from the goods its trades use, in emeralds: its wage float and three days of
     * guards' pay. A Merchant that spent its farmers' wages on coal would lose their harvest.
     */
    public double payroll(GlobalPos desk) {
        return wageFloat(desk) + bellOf(desk).map(bell -> holdings.get(bell).posts.size() * GUARD_PAY_PER_DAY * 3)
                .orElse(0.0);
    }

    /** The toll this town has set, as a share of the coins caravans carry. It's collected only at its posts. */
    /** The handling fee a caravan through the town's posts pays, whatever it carries, in spurs. */
    public double passageFee(GlobalPos bell) {
        return holding(bell).map(h -> h.posts.isEmpty() ? 0 : h.passageFee).orElse(0.0);
    }

    public double tollRate(GlobalPos bell) {
        return holding(bell).map(h -> h.tollRate).orElse(DEFAULT_TOLL_RATE);
    }

    public void setTollRate(GlobalPos bell, double rate) {
        holdings.computeIfAbsent(bell, b -> new Holding()).tollRate = Math.max(0, Math.min(MAX_TOLL_RATE, rate));
        setDirty();
    }

    /** The town with a toll post (a guarded hex) here, if any. */
    public Optional<GlobalPos> postOwner(ResourceKey<Level> dimension, Hex hex) {
        return owner(dimension, hex).filter(bell -> holdings.containsKey(bell) && holdings.get(bell).posts.contains(hex));
    }

    /** The town's toll posts: one guard each. */
    public java.util.Set<Hex> posts(GlobalPos bell) {
        return holding(bell).map(h -> java.util.Set.copyOf(h.posts)).orElse(java.util.Set.of());
    }

    public void setPosts(GlobalPos bell, java.util.Collection<Hex> posts) {
        holdings.computeIfAbsent(bell, b -> new Holding()).posts = new java.util.LinkedHashSet<>(posts);
        setDirty();
    }

    public int guards(GlobalPos bell) {
        return holding(bell).map(h -> h.posts.size()).orElse(0);
    }

    /** Posts guards on the first {@code guards} of the town's hexes. */
    public void setGuards(GlobalPos bell, int guards) {
        setPosts(bell, hexes(bell).subList(0, Math.max(0, Math.min(guards, hexes(bell).size()))));
    }

    public int deserted(GlobalPos bell) {
        return holding(bell).map(h -> h.deserted).orElse(0);
    }

    public double trafficPerDay(GlobalPos bell) {
        return holding(bell).map(h -> h.trafficPerDay).orElse(0.0);
    }

    /** A caravan carrying {@code spurs} crossed this town's land (whatever toll it paid). */
    public void recordTraffic(GlobalPos bell, int spurs) {
        holdings.computeIfAbsent(bell, b -> new Holding()).trafficSince += spurs;
        setDirty();
    }

    /**
     * A caravan on route {@code route} carrying {@code spurs} crossed this town's land; {@code options} is what going
     * around ever larger sets of posts on its land would have cost it.
     */
    public void observeCrossing(GlobalPos bell, int route, int spurs, List<Chokepoints.Option> options) {
        Chokepoints.Crossing crossing = holdings.computeIfAbsent(bell, b -> new Holding())
                .crossings.computeIfAbsent(route, r -> new Chokepoints.Crossing());
        crossing.value = crossing.value == 0 ? spurs : (crossing.value + spurs) / 2;
        crossing.tripsSince++;
        crossing.options.clear();
        crossing.options.addAll(options);
        setDirty();
    }

    /** What the town has seen of the routes crossing its land, by route id. */
    public Map<Integer, Chokepoints.Crossing> crossings(GlobalPos bell) {
        return holding(bell).map(h -> Map.copyOf(h.crossings)).orElse(Map.of());
    }

    /** One guard's daily cost, in spurs: its pay plus its food at bread prices. */
    public static double guardCostPerDay(double rate) {
        return (GUARD_PAY_PER_DAY + GUARD_FOOD_PER_DAY / 30.0) * rate;
    }

    /** Routes not seen for this many reviews are forgotten. */
    private static final int FORGET_AFTER_REVIEWS = 4;

    /**
     * The guards' food and pay since the last upkeep, from the town's warehouse: food first, then pay from its
     * money. Guards who go unfed or unpaid desert, in proportion to what was missing.
     */
    private void upkeepGuards(ServerLevel level, BlockPos desk, MerchantStock stock, GlobalPos bell, Holding holding,
            long now) {
        if (holding.lastGuardUpkeep < 0 || holding.posts.isEmpty()) {
            holding.lastGuardUpkeep = now;
            return;
        }
        double days = Math.min(MAX_DAYS, (now - holding.lastGuardUpkeep) / 24000.0);
        holding.lastGuardUpkeep = now;
        // Its hands without a trade stand watch first, as militia: for their food, without pay.
        holding.militia = Math.min(holding.posts.size(),
                Labour.hands(dev.b28.enterprise.settlement.Settlements.get(level.getServer())
                        .roster(bell)).size());
        holding.guardFoodOwed += holding.posts.size() * GUARD_FOOD_PER_DAY * days;
        holding.guardUpkeepOwed += (holding.posts.size() - holding.militia) * GUARD_PAY_PER_DAY * days;
        int food = (int) Math.floor(holding.guardFoodOwed);
        int pay = (int) Math.floor(holding.guardUpkeepOwed);
        if (food <= 0 && pay <= 0) {
            return;
        }
        double fed = 1;
        if (food > 0) {
            int eaten = stock.eat(food).entrySet().stream().mapToInt(e -> e.getValue() * nutritionOf(e.getKey())).sum();
            fed = Math.min(1, (double) eaten / food);
            holding.guardFoodOwed -= food;
        }
        double paid = 1;
        if (pay > 0) {
            Mint mint = Mint.get(level.getServer());
            double rate = mint.rate(Mint.DEFAULT_FACTION, now);
            paid = Math.min(1, Treasury.withdraw(level, desk, stock, pay, mint, now).value(rate) / pay);
            holding.guardUpkeepOwed -= pay;
            // Guards on posts that took tolls last review will wait for their pay, up to a few days' worth: it comes
            // out of the tolls to come. Past that, unpaid, they go.
            double unpaid = pay * (1 - paid);
            double credit = GUARD_CREDIT_DAYS * holding.posts.size() * GUARD_PAY_PER_DAY;
            // (tolls taken last review, or a plan that expects them: new guards wait for the first)
            boolean earning = holding.tollIncomeYesterday > 0
                    || holding.plannedTollIncome > holding.posts.size() * guardCostPerDay(
                            Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now));
            if (unpaid > 0 && earning && holding.guardUpkeepOwed + unpaid <= credit) {
                holding.guardUpkeepOwed += unpaid;
                paid = 1;
            }
        }
        double kept = Math.min(fed, paid);
        if (kept < 1) {
            int stay = (int) Math.floor(holding.posts.size() * kept);
            holding.deserted += holding.posts.size() - stay;
            // The guards posted first stay longest.
            holding.posts = new java.util.LinkedHashSet<>(new ArrayList<>(holding.posts).subList(0, stay));
        }
    }

    private static int nutritionOf(Item item) {
        var food = item.getDefaultInstance().get(net.minecraft.core.component.DataComponents.FOOD);
        return food == null ? 0 : food.nutrition();
    }

    /** Toll income over the last full period reviewed (-1 before the first). */
    public double tollIncomeYesterday(GlobalPos bell) {
        return holding(bell).map(h -> h.tollIncomeYesterday).orElse(-1.0);
    }

    /** As {@link #reviewTolls(GlobalPos, long, double)} at the base rate of 16 spurs to the emerald. */
    public void reviewTolls(GlobalPos bell, long now) {
        reviewTolls(bell, now, 16);
    }

    /**
     * Every {@link #TOLL_REVIEW_TICKS} a town looks at the caravans that crossed its land and what going around each of
     * its hexes would have cost them ({@link Chokepoints}), then settles its toll and the hexes to guard: it lets go
     * guards on posts it no longer wants at once and hires one guard a review for a new post. With no post worth a
     * guard it keeps its toll for later.
     */
    public void reviewTolls(GlobalPos bell, long now, double rate) {
        Holding holding = holdings.computeIfAbsent(bell, b -> new Holding());
        if (holding.lastTollReview < 0) {
            holding.lastTollReview = now;
            return;
        }
        if (now - holding.lastTollReview < TOLL_REVIEW_TICKS) {
            return;
        }
        double days = (now - holding.lastTollReview) / 24000.0;
        holding.trafficPerDay = holding.trafficSince / days;
        holding.trafficSince = 0;
        for (var it = holding.crossings.values().iterator(); it.hasNext(); ) {
            Chokepoints.Crossing crossing = it.next();
            crossing.tripsPerDay = crossing.tripsSince / days;
            crossing.quietReviews = crossing.tripsSince > 0 ? 0 : crossing.quietReviews + 1;
            crossing.tripsSince = 0;
            if (crossing.quietReviews >= FORGET_AFTER_REVIEWS) {
                it.remove();
            }
        }
        // Armed guards ask more of the detour, armoured ones may ask a higher toll.
        double armed = holding.armed;
        double armoured = holding.kitShare.getOrDefault(Smithing.ARMOUR.id(), 0.0);
        Chokepoints.Plan plan = Chokepoints.bestPlan(holding.crossings.values(), guardCostPerDay(rate),
                Chokepoints.DETOUR_SHARE + (Smithing.ARMED_DETOUR_SHARE - Chokepoints.DETOUR_SHARE) * armed,
                MAX_TOLL_RATE + (Smithing.ARMOURED_TOLL_RATE - MAX_TOLL_RATE) * armoured, PASSAGE_FEE_CAP * rate);
        holding.posts.retainAll(plan.posts());
        plan.posts().stream()
                .filter(hex -> !holding.posts.contains(hex))
                .findFirst()
                .ifPresent(holding.posts::add);
        if (!plan.posts().isEmpty()) {
            holding.tollRate = plan.rate();
            holding.passageFee = plan.fee();
        }
        holding.plannedTollIncome = plan.income();
        holding.tollIncomeYesterday = holding.tollIncomeToday;
        holding.tollIncomeToday = 0;
        holding.lastTollReview = now;
        setDirty();
    }

    private Optional<Holding> holding(GlobalPos bell) {
        return Optional.ofNullable(holdings.get(bell));
    }

    /** The Merchant's desk nearest the bell, in the census's reach. */
    public static Optional<BlockPos> nearestDesk(ServerLevel level, BlockPos bell) {
        return level.getPoiManager()
                .findClosest(type -> type.is(Merchant.DESK_POI.getKey()), bell,
                        dev.b28.enterprise.settlement.CensusTaker.RADIUS, PoiManager.Occupancy.ANY);
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag hexes = new ListTag();
        owners.forEach((place, bell) -> {
            CompoundTag entry = new CompoundTag();
            entry.putString("dimension", place.dimension().location().toString());
            entry.putInt("q", place.hex().q());
            entry.putInt("r", place.hex().r());
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, bell).ifSuccess(t -> entry.put("bell", t));
            hexes.add(entry);
        });
        ListTag settlements = new ListTag();
        holdings.forEach((bell, holding) -> {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, bell).ifSuccess(t -> entry.put("bell", t));
            if (holding.desk != null) {
                GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, holding.desk).ifSuccess(t -> entry.put("desk", t));
            }
            entry.putDouble("culture", holding.culture);
            entry.putLong("lastHarvest", holding.lastHarvest);
            entry.putLong("tollsOwed", holding.tollsOwed);
            entry.putDouble("wageOwed", holding.wageOwed);
            entry.putDouble("purse", holding.purse);
            entry.putDouble("deposits", holding.deposits);
            entry.putLong("lastDeposits", holding.lastDeposits);
            entry.putDouble("credit", holding.credit);
            entry.putInt("freeholds", holding.freeholds);
            entry.putDouble("householdSpent", holding.householdSpent);
            entry.putDouble("labourTightness", holding.labourTightness);
            entry.putDouble("handTools", holding.handTools);
            entry.putDouble("wagesPaid", holding.wagesPaid);
            entry.putDouble("wagesUnpaid", holding.wagesUnpaid);
            entry.putLong("harvested", holding.harvested);
            entry.putDouble("farmXp", holding.farmXp);
            CompoundTag banked = new CompoundTag();
            holding.banked.forEach((id, n) -> {
                if (n > 0) {
                    banked.putDouble(id, n);
                }
            });
            entry.put("banked", banked);
            CompoundTag turnedTag = new CompoundTag();
            holding.turned.forEach((id, n) -> {
                if (n != 0) {
                    turnedTag.putInt(id, n);
                }
            });
            entry.put("turned", turnedTag);
            entry.putDouble("laborShare", holding.laborShare);
            entry.putDouble("salesSince", holding.salesSince);
            entry.putDouble("salesPerDay", holding.salesPerDay);
            entry.putDouble("valuePerDay", holding.valuePerDay);
            entry.putDouble("treasury", holding.treasury);
            entry.putDouble("tollRate", holding.tollRate);
            entry.putDouble("passageFee", holding.passageFee);
            entry.putDouble("plannedTollIncome", holding.plannedTollIncome);
            entry.putDouble("tollIncomeToday", holding.tollIncomeToday);
            entry.putDouble("tollIncomeYesterday", holding.tollIncomeYesterday);
            entry.putLong("lastTollReview", holding.lastTollReview);
            entry.putIntArray("posts", hexArray(holding.posts));
            ListTag crossings = new ListTag();
            holding.crossings.forEach((route, crossing) -> {
                CompoundTag c = new CompoundTag();
                c.putInt("route", route);
                c.putDouble("value", crossing.value);
                c.putDouble("tripsSince", crossing.tripsSince);
                c.putDouble("tripsPerDay", crossing.tripsPerDay);
                c.putInt("quietReviews", crossing.quietReviews);
                ListTag options = new ListTag();
                for (Chokepoints.Option option : crossing.options) {
                    CompoundTag o = new CompoundTag();
                    o.putIntArray("posts", hexArray(option.posts()));
                    o.putDouble("detour", Double.isInfinite(option.detour()) ? -1 : option.detour()); // -1: no way
                    options.add(o);
                }
                c.put("options", options);
                crossings.add(c);
            });
            entry.put("crossings", crossings);
            entry.putDouble("guardUpkeepOwed", holding.guardUpkeepOwed);
            entry.putDouble("guardFoodOwed", holding.guardFoodOwed);
            entry.putLong("lastGuardUpkeep", holding.lastGuardUpkeep);
            entry.putDouble("trafficSince", holding.trafficSince);
            entry.putDouble("trafficPerDay", holding.trafficPerDay);
            entry.putInt("deserted", holding.deserted);
            entry.putInt("villagers", holding.villagers);
            entry.putInt("beds", holding.beds);
            entry.putDouble("mood", holding.mood);
            entry.put("comfortOwed", doubles(holding.comfortOwed));
            entry.put("comfortMet", doubles(holding.comfortMet));
            entry.putLong("lastComforts", holding.lastComforts);
            entry.putDouble("paidShare", holding.paidShare);
            entry.put("demand", doubles(holding.demand));
            entry.putInt("fieldsPlanted", holding.fieldsPlanted);
            entry.put("herd", doubles(holding.herd));
            if (!holding.morePastures.isEmpty()) {
                entry.putLongArray("morePastures", holding.morePastures.stream().mapToLong(BlockPos::asLong).toArray());
            }
            entry.put("fodder", doubles(holding.fodder));
            if (holding.pasture != null) {
                entry.putLong("pasture", holding.pasture.asLong());
            }
            entry.putLong("lastTend", holding.lastTend);
            entry.putLong("blessedUntil", holding.blessedUntil);
            entry.put("idlePerDay", doubles(holding.idlePerDay));
            entry.putInt("spareFarmPlots", holding.spareFarmPlots);
            if (holding.mine != null) {
                entry.putLong("mine", holding.mine.asLong());
            }
            entry.put("kitWear", doubles(holding.kitWear));
            entry.putLong("mendedUntil", holding.mendedUntil);
            entry.put("kitShare", doubles(holding.kitShare));
            entry.putInt("militia", holding.militia);
            entry.putDouble("hunting", holding.hunting);
            entry.putDouble("armed", holding.armed);
            entry.putBoolean("flowerBeds", holding.flowerBeds);
            entry.putBoolean("deepMine", holding.deepMine);
            entry.putBoolean("blastFurnace", holding.blastFurnace);
            entry.putLong("armouryUntil", holding.armouryUntil);
            entry.putLongArray("fallen", holding.fallen.stream().mapToLong(Long::longValue).toArray());
            if (holding.dock != null) {
                entry.putLong("dock", holding.dock.asLong());
            }
            entry.putDouble("homeProgress", holding.homeProgress);
            entry.putDouble("birthProgress", holding.birthProgress);
            entry.putInt("homesBuilt", holding.homesBuilt);
            entry.putString("focus", holding.focus.name());
            entry.putInt("born", holding.born);
            entry.put("perDay", doubles(holding.perDay));
            entry.put("pending", doubles(holding.pending));
            settlements.add(entry);
        });
        ListTag cropCounts = new ListTag();
        fields.forEach((place, chunks) -> chunks.forEach((chunk, counts) -> {
            CompoundTag entry = new CompoundTag();
            entry.putString("dimension", place.dimension().location().toString());
            entry.putInt("q", place.hex().q());
            entry.putInt("r", place.hex().r());
            entry.putLong("chunk", chunk);
            CompoundTag byItem = new CompoundTag();
            counts.forEach(byItem::putInt);
            entry.put("crops", byItem);
            cropCounts.add(entry);
        }));
        tag.put("hexes", hexes);
        tag.put("settlements", settlements);
        tag.put("crops", cropCounts);
        return tag;
    }

    public static Territory load(CompoundTag tag, HolderLookup.Provider registries) {
        Territory territory = new Territory();
        for (Tag t : tag.getList("hexes", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            ResourceKey<Level> dimension = ResourceKey.create(
                    net.minecraft.core.registries.Registries.DIMENSION, ResourceLocation.parse(entry.getString("dimension")));
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("bell")).ifSuccess(bell -> territory.owners.put(
                    new Place(dimension, new Hex(entry.getInt("q"), entry.getInt("r"))), bell));
        }
        for (Tag t : tag.getList("crops", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            ResourceKey<Level> dimension = ResourceKey.create(
                    net.minecraft.core.registries.Registries.DIMENSION, ResourceLocation.parse(entry.getString("dimension")));
            Map<String, Integer> counts = new LinkedHashMap<>();
            CompoundTag byItem = entry.getCompound("crops");
            byItem.getAllKeys().forEach(id -> counts.put(id, byItem.getInt(id)));
            territory.fields.computeIfAbsent(new Place(dimension, new Hex(entry.getInt("q"), entry.getInt("r"))),
                    p -> new HashMap<>()).put(entry.getLong("chunk"), counts);
        }
        for (Tag t : tag.getList("settlements", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("bell")).ifSuccess(bell -> {
                Holding holding = new Holding();
                if (entry.contains("desk")) {
                    GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("desk")).ifSuccess(desk -> holding.desk = desk);
                }
                holding.culture = entry.getDouble("culture");
                holding.lastHarvest = entry.getLong("lastHarvest");
                holding.tollsOwed = entry.getLong("tollsOwed");
                holding.wageOwed = entry.getDouble("wageOwed");
                holding.purse = entry.getDouble("purse");
                holding.deposits = entry.getDouble("deposits");
                holding.lastDeposits = entry.getLong("lastDeposits");
                holding.credit = entry.getDouble("credit");
                holding.freeholds = entry.getInt("freeholds");
                holding.householdSpent = entry.getDouble("householdSpent");
                holding.labourTightness = entry.contains("labourTightness") ? entry.getDouble("labourTightness") : 1;
                holding.handTools = entry.getDouble("handTools");
                holding.wagesPaid = entry.getDouble("wagesPaid");
                holding.wagesUnpaid = entry.getDouble("wagesUnpaid");
                holding.harvested = entry.getLong("harvested");
                holding.farmXp = entry.getDouble("farmXp");
                if (entry.contains("grain")) { // saved before root cellars: the granary's wheat
                    holding.banked.put("minecraft:wheat", entry.getDouble("grain"));
                }
                CompoundTag banked = entry.getCompound("banked");
                banked.getAllKeys().forEach(id -> holding.banked.put(id, banked.getDouble(id)));
                CompoundTag turnedTag = entry.getCompound("turned");
                turnedTag.getAllKeys().forEach(id -> holding.turned.put(id, turnedTag.getInt(id)));
                if (entry.contains("laborShare")) {
                    holding.laborShare = entry.getDouble("laborShare");
                }
                holding.salesSince = entry.getDouble("salesSince");
                holding.salesPerDay = entry.getDouble("salesPerDay");
                holding.valuePerDay = entry.getDouble("valuePerDay");
                holding.treasury = entry.getDouble("treasury");
                holding.passageFee = entry.getDouble("passageFee");
                holding.plannedTollIncome = entry.getDouble("plannedTollIncome");
                if (entry.contains("tollRate")) {
                    holding.tollRate = entry.getDouble("tollRate");
                    holding.tollIncomeToday = entry.getDouble("tollIncomeToday");
                    holding.tollIncomeYesterday = entry.getDouble("tollIncomeYesterday");
                    holding.lastTollReview = entry.getLong("lastTollReview");
                }
                holding.posts = new java.util.LinkedHashSet<>(readHexes(entry.getIntArray("posts")));
                for (Tag ct : entry.getList("crossings", Tag.TAG_COMPOUND)) {
                    CompoundTag c = (CompoundTag) ct;
                    Chokepoints.Crossing crossing = new Chokepoints.Crossing();
                    crossing.value = c.getDouble("value");
                    crossing.tripsSince = c.getDouble("tripsSince");
                    crossing.tripsPerDay = c.getDouble("tripsPerDay");
                    crossing.quietReviews = c.getInt("quietReviews");
                    for (Tag ot : c.getList("options", Tag.TAG_COMPOUND)) {
                        CompoundTag o = (CompoundTag) ot;
                        double detour = o.getDouble("detour");
                        crossing.options.add(new Chokepoints.Option(readHexes(o.getIntArray("posts")),
                                detour < 0 ? Double.POSITIVE_INFINITY : detour));
                    }
                    holding.crossings.put(c.getInt("route"), crossing);
                }
                holding.guardUpkeepOwed = entry.getDouble("guardUpkeepOwed");
                holding.guardFoodOwed = entry.getDouble("guardFoodOwed");
                holding.lastGuardUpkeep = entry.contains("lastGuardUpkeep") ? entry.getLong("lastGuardUpkeep") : -1;
                holding.trafficSince = entry.getDouble("trafficSince");
                holding.trafficPerDay = entry.getDouble("trafficPerDay");
                holding.deserted = entry.getInt("deserted");
                holding.villagers = entry.getInt("villagers");
                holding.beds = entry.getInt("beds");
                holding.mood = entry.getDouble("mood");
                holding.comfortOwed = new HashMap<>(readDoubles(entry.getCompound("comfortOwed")));
                holding.comfortMet = readDoubles(entry.getCompound("comfortMet"));
                holding.lastComforts = entry.contains("lastComforts") ? entry.getLong("lastComforts") : -1;
                holding.paidShare = entry.contains("paidShare") ? entry.getDouble("paidShare") : 1;
                holding.demand = new HashMap<>(readDoubles(entry.getCompound("demand")));
                holding.fieldsPlanted = entry.getInt("fieldsPlanted");
                holding.herd = readDoubles(entry.getCompound("herd"));
                holding.pasture = entry.contains("pasture") ? BlockPos.of(entry.getLong("pasture")) : null;
                for (long corner : entry.getLongArray("morePastures")) {
                    holding.morePastures.add(BlockPos.of(corner));
                }
                holding.fodder = readDoubles(entry.getCompound("fodder"));
                holding.lastTend = entry.contains("lastTend") ? entry.getLong("lastTend") : -1;
                holding.blessedUntil = entry.contains("blessedUntil") ? entry.getLong("blessedUntil") : -1;
                holding.idlePerDay = readDoubles(entry.getCompound("idlePerDay"));
                holding.spareFarmPlots = entry.getInt("spareFarmPlots");
                holding.mine = entry.contains("mine") ? BlockPos.of(entry.getLong("mine")) : null;
                holding.kitWear = new LinkedHashMap<>(readDoubles(entry.getCompound("kitWear")));
                holding.mendedUntil = entry.contains("mendedUntil") ? entry.getLong("mendedUntil") : -1;
                holding.kitShare = new LinkedHashMap<>(readDoubles(entry.getCompound("kitShare")));
                holding.militia = entry.getInt("militia");
                holding.hunting = entry.getDouble("hunting");
                holding.armed = entry.getDouble("armed");
                holding.flowerBeds = entry.getBoolean("flowerBeds");
                holding.deepMine = entry.getBoolean("deepMine");
                holding.blastFurnace = entry.getBoolean("blastFurnace");
                holding.armouryUntil = entry.contains("armouryUntil") ? entry.getLong("armouryUntil") : -1;
                for (long when : entry.getLongArray("fallen")) {
                    holding.fallen.add(when);
                }
                holding.dock = entry.contains("dock") ? BlockPos.of(entry.getLong("dock")) : null;
                holding.homeProgress = entry.getDouble("homeProgress");
                holding.birthProgress = entry.getDouble("birthProgress");
                holding.homesBuilt = entry.getInt("homesBuilt");
                holding.focus = entry.contains("focus") ? Focus.valueOf(entry.getString("focus")) : Focus.GROW;
                holding.born = entry.getInt("born");
                holding.perDay = readDoubles(entry.getCompound("perDay"));
                holding.pending = readDoubles(entry.getCompound("pending"));
                territory.holdings.put(bell, holding);
            });
        }
        return territory;
    }

    private static int[] hexArray(java.util.Collection<Hex> hexes) {
        int[] values = new int[hexes.size() * 2];
        int i = 0;
        for (Hex hex : hexes) {
            values[i++] = hex.q();
            values[i++] = hex.r();
        }
        return values;
    }

    private static List<Hex> readHexes(int[] values) {
        List<Hex> hexes = new ArrayList<>();
        for (int i = 0; i + 1 < values.length; i += 2) {
            hexes.add(new Hex(values[i], values[i + 1]));
        }
        return hexes;
    }

    private static CompoundTag doubles(Map<String, Double> values) {
        CompoundTag tag = new CompoundTag();
        values.forEach(tag::putDouble);
        return tag;
    }

    private static Map<String, Double> readDoubles(CompoundTag tag) {
        Map<String, Double> values = new LinkedHashMap<>();
        tag.getAllKeys().forEach(key -> values.put(key, tag.getDouble(key)));
        return values;
    }
}
