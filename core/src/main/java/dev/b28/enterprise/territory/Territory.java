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
    public static final double TOLL_STEP = 0.01;

    /** A town judges its toll over this many days: caravans pass every few days, so one day says little. */
    public static final long TOLL_REVIEW_TICKS = 3 * 24000L;

    /** A guard's daily food (points of nutrition) and pay (emeralds). */
    public static final double GUARD_FOOD_PER_DAY = 6;
    public static final double GUARD_PAY_PER_DAY = 0.5;

    /** At most this many days of produce accrue between visits. */
    private static final double MAX_DAYS = 60;

    /** A hex in a dimension. */
    public record Place(ResourceKey<Level> dimension, Hex hex) {}

    private static final class Holding {
        double culture;
        GlobalPos desk; // nearest Merchant's desk, if any
        Map<String, Double> perDay = new LinkedHashMap<>(); // farm plan: item id -> items per day
        Map<String, Double> pending = new HashMap<>(); // produce owed, fractions carried
        long lastHarvest = -1;
        long tollsOwed;
        double wageOwed; // emeralds owed to the farmers, fractions carried
        double wagesPaid; // emeralds' worth paid, in all
        double wagesUnpaid; // emeralds' worth that couldn't be paid: those days' produce was lost
        long harvested; // items delivered to the Merchant's chest, in all
        double laborShare = LABOR_SHARE;
        double salesSince; // emeralds from selling crops since the last accrual
        double salesPerDay; // averaged
        double valuePerDay; // the harvest's value, averaged
        double treasury; // the market's money at its last touch, in emeralds (chest emeralds, coins, Banker)
        double tollRate = DEFAULT_TOLL_RATE;
        int tollDirection = 1; // which way the last change went
        double tollIncomeToday;
        double tollIncomeYesterday = -1; // none yet
        long lastTollReview = -1;
        int guards;
        double guardUpkeepOwed; // emeralds owed to the guards, fractions carried
        double guardFoodOwed; // food points owed, fractions carried
        long lastGuardUpkeep = -1;
        double trafficSince; // coins carried across this land since the last review, in spurs
        double trafficPerDay; // averaged
        int deserted; // guards lost to hunger or no pay, in all
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

    public Map<String, Double> farmPlan(GlobalPos bell) {
        return holding(bell).map(h -> Map.copyOf(h.perDay)).orElse(Map.of());
    }

    /** The farmers' wages per day, in emeralds: {@link #LABOR_SHARE} of the value of what they grow. */
    public double wagePerDay(GlobalPos bell) {
        return holding(bell).map(Territory::wagePerDay).orElse(0.0);
    }

    private static double wagePerDay(Holding holding) {
        return valuePerDay(holding.perDay) * holding.laborShare;
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
        Hex home = Hex.at(bellPos);
        claim(bell, home);
        if (owner(level.dimension(), home).filter(bell::equals).isPresent()) {
            holding.culture += census.value() * days;
            grow(bell, home);
        }
        if (holding.desk != null && level.isLoaded(holding.desk.pos())) {
            // Settle produce and wages under the old plan before re-planning.
            touch(level, holding.desk.pos(), MerchantStock.at(level, holding.desk.pos()));
        } else {
            accrue(holding, level.getGameTime());
        }
        holding.perDay = plan(level, bellPos.getY(), hexes(bell), farmers);
        setDirty();
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

    /**
     * Farmers take the most-planted hexes first, each tending its plots; a hex's produce follows the crops growing in
     * it. Returns items per day by item id.
     */
    public Map<String, Double> plan(ServerLevel level, int centerY, List<Hex> hexes, List<Integer> farmers) {
        Map<String, Double> perDay = new LinkedHashMap<>();
        if (farmers.isEmpty()) {
            return perDay;
        }
        List<Map<Item, Integer>> fields = new ArrayList<>();
        for (Hex hex : hexes) {
            Map<Item, Integer> plots = cropsIn(level, hex, centerY);
            if (!plots.isEmpty()) {
                fields.add(new HashMap<>(plots));
            }
        }
        fields.sort(Comparator.comparingInt((Map<Item, Integer> f) -> total(f)).reversed());
        List<Integer> crew = farmers.stream().sorted(Comparator.reverseOrder()).toList();
        int field = 0;
        for (int level0 : crew) {
            int capacity = PLOTS_PER_FARMER_LEVEL * Math.max(1, level0);
            while (capacity > 0 && field < fields.size()) {
                Map<Item, Integer> plots = fields.get(field);
                int total = total(plots);
                int worked = Math.min(capacity, total);
                for (Map.Entry<Item, Integer> crop : plots.entrySet()) {
                    double share = (double) crop.getValue() / total;
                    String id = BuiltInRegistries.ITEM.getKey(crop.getKey()).toString();
                    perDay.merge(id, worked * share * YIELD_PER_PLOT_PER_DAY, Double::sum);
                }
                capacity -= worked;
                if (worked == total) {
                    field++;
                } else {
                    plots.replaceAll((item, count) -> (int) Math.round(count * (1 - (double) worked / total)));
                }
            }
        }
        return perDay;
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
            holding.perDay.forEach((item, rate) -> holding.pending.merge(item, rate * days, Double::sum));
            holding.wageOwed += wagePerDay(holding) * days;
            if (days > 0) {
                double weight = 1 - Math.pow(0.5, days / SALES_HALF_LIFE_DAYS);
                holding.salesPerDay += (holding.salesSince / days - holding.salesPerDay) * weight;
                holding.valuePerDay += (valuePerDay(holding.perDay) - holding.valuePerDay) * weight;
                holding.salesSince = 0;
                if (holding.valuePerDay > 0) {
                    double target = Math.max(MIN_LABOR_SHARE,
                            Math.min(MAX_LABOR_SHARE, holding.salesPerDay / holding.valuePerDay));
                    holding.laborShare += (target - holding.laborShare)
                            * (1 - Math.pow(0.5, days / SHARE_HALF_LIFE_DAYS));
                }
            }
        }
        holding.lastHarvest = now;
    }

    /**
     * Pays the farmers the whole emeralds owed: emeralds from the Merchant's chest first, then its coins at the
     * region's rate, then the settlement's Blaze Banker. Returns the share of the wage paid (1 when nothing whole is
     * owed yet). Unpaid wages are forgiven: the farmers didn't work, and that part of the harvest is lost.
     */
    private double payWages(ServerLevel level, BlockPos desk, MerchantStock stock, Holding holding, long now) {
        int whole = (int) Math.floor(holding.wageOwed);
        if (whole <= 0) {
            return 1.0;
        }
        Mint mint = Mint.get(level.getServer());
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        double paid = Math.min(whole, Treasury.withdraw(level, desk, stock, whole, mint, now).value(rate));
        holding.wageOwed -= whole;
        holding.wagesPaid += paid;
        holding.wagesUnpaid += whole - paid;
        return paid / whole;
    }

    /** The market at {@code desk} was touched: hand over the produce and tolls owed to its settlement. */
    public void touch(ServerLevel level, BlockPos desk, MerchantStock stock) {
        touch(level, desk, stock, level.getGameTime());
    }

    /** As {@link #touch(ServerLevel, BlockPos, MerchantStock)}, at game time {@code now}. */
    public void touch(ServerLevel level, BlockPos desk, MerchantStock stock, long now) {
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        Optional<GlobalPos> bell = bellOf(at);
        if (bell.isEmpty()) {
            return;
        }
        Holding holding = holdings.get(bell.get());
        upkeepGuards(level, desk, stock, holding, now); // pay the guards who served, then review the garrison
        reviewTolls(bell.get(), now, Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now));
        long since = holding.lastHarvest;
        accrue(holding, now);
        double owedBefore = holding.wageOwed;
        double paidShare = payWages(level, desk, stock, holding, now);
        long harvestedBefore = holding.harvested;
        if (paidShare < 1) {
            holding.pending.replaceAll((item, owed) -> owed * paidShare); // unpaid days' harvest is lost
        }
        for (Map.Entry<String, Double> owed : holding.pending.entrySet()) {
            int whole = (int) Math.floor(owed.getValue());
            if (whole <= 0) {
                continue;
            }
            Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(owed.getKey()));
            int left = whole;
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
        }
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

    /** The toll this town has set, as a share of the coins caravans carry. */
    public double tollRate(GlobalPos bell) {
        return holding(bell).map(h -> h.tollRate).orElse(DEFAULT_TOLL_RATE);
    }

    /**
     * The toll a caravan actually pays crossing this town's land: its rate times the share of its hexes guarded. With
     * no guards it collects nothing.
     */
    public double effectiveToll(GlobalPos bell) {
        return tollRate(bell) * coverage(bell);
    }

    /** Share of the town's hexes held by a guard. */
    public double coverage(GlobalPos bell) {
        int hexes = hexes(bell).size();
        return hexes == 0 ? 0 : Math.min(1, (double) guards(bell) / hexes);
    }

    public int guards(GlobalPos bell) {
        return holding(bell).map(h -> h.guards).orElse(0);
    }

    public void setGuards(GlobalPos bell, int guards) {
        holdings.computeIfAbsent(bell, b -> new Holding()).guards = Math.max(0, guards);
        setDirty();
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

    /** One guard's daily cost, in spurs: its pay plus its food at bread prices. */
    public static double guardCostPerDay(double rate) {
        return (GUARD_PAY_PER_DAY + GUARD_FOOD_PER_DAY / 30.0) * rate;
    }

    /**
     * Hire or let go one guard: hire if the toll the next guard would unlock (a hex's share of the toll on the traffic
     * crossing this land) beats a guard's daily cost; let one go if the last guard brings in less than it costs.
     */
    public void reviewGarrison(GlobalPos bell, double days, double rate) {
        Holding holding = holdings.computeIfAbsent(bell, b -> new Holding());
        if (days > 0) {
            holding.trafficPerDay = holding.trafficSince / days;
            holding.trafficSince = 0;
        }
        int hexes = Math.max(1, hexes(bell).size());
        double perGuard = holding.trafficPerDay * holding.tollRate / hexes; // toll one more guarded hex brings in
        double cost = guardCostPerDay(rate);
        if (holding.guards < hexes && perGuard > cost) {
            holding.guards++;
        } else if (holding.guards > 0 && perGuard < cost) {
            holding.guards--;
        }
        setDirty();
    }

    /**
     * The guards' food and pay since the last upkeep, from the town's warehouse: food first, then pay from its
     * money. Guards who go unfed or unpaid desert, in proportion to what was missing.
     */
    private void upkeepGuards(ServerLevel level, BlockPos desk, MerchantStock stock, Holding holding, long now) {
        if (holding.lastGuardUpkeep < 0 || holding.guards == 0) {
            holding.lastGuardUpkeep = now;
            return;
        }
        double days = Math.min(MAX_DAYS, (now - holding.lastGuardUpkeep) / 24000.0);
        holding.lastGuardUpkeep = now;
        holding.guardFoodOwed += holding.guards * GUARD_FOOD_PER_DAY * days;
        holding.guardUpkeepOwed += holding.guards * GUARD_PAY_PER_DAY * days;
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
        }
        double kept = Math.min(fed, paid);
        if (kept < 1) {
            int stay = (int) Math.floor(holding.guards * kept);
            holding.deserted += holding.guards - stay;
            holding.guards = stay;
        }
    }

    private static int nutritionOf(Item item) {
        var food = item.getDefaultInstance().get(net.minecraft.core.component.DataComponents.FOOD);
        return food == null ? 0 : food.nutrition();
    }

    public void setTollRate(GlobalPos bell, double rate) {
        holdings.computeIfAbsent(bell, b -> new Holding()).tollRate = Math.max(0, Math.min(MAX_TOLL_RATE, rate));
        setDirty();
    }

    /** Toll income over the last full period reviewed (-1 before the first). */
    public double tollIncomeYesterday(GlobalPos bell) {
        return holding(bell).map(h -> h.tollIncomeYesterday).orElse(-1.0);
    }

    /**
     * Every {@link #TOLL_REVIEW_TICKS}, a town moves its toll a step the way that raised its income: if the last
     * period's tolls fell short of the one before, it reverses direction. A town with no toll income in two periods
     * running lowers its toll to draw traffic.
     */
    public void reviewTolls(GlobalPos bell, long now) {
        reviewTolls(bell, now, 16);
    }

    /** As {@link #reviewTolls(GlobalPos, long)}, also reviewing the garrison at the mint's {@code rate}. */
    public void reviewTolls(GlobalPos bell, long now, double rate) {
        Holding holding = holdings.computeIfAbsent(bell, b -> new Holding());
        if (holding.lastTollReview < 0) {
            holding.lastTollReview = now;
            return;
        }
        if (now - holding.lastTollReview < TOLL_REVIEW_TICKS) {
            return;
        }
        reviewGarrison(bell, (now - holding.lastTollReview) / 24000.0, rate);
        double income = holding.tollIncomeToday;
        if (income == 0 && holding.tollIncomeYesterday <= 0) {
            holding.tollDirection = -1;
        } else if (holding.tollIncomeYesterday >= 0 && income < holding.tollIncomeYesterday) {
            holding.tollDirection = -holding.tollDirection;
        }
        holding.tollRate = Math.max(0, Math.min(MAX_TOLL_RATE, holding.tollRate + holding.tollDirection * TOLL_STEP));
        holding.tollIncomeYesterday = income;
        holding.tollIncomeToday = 0;
        holding.lastTollReview = now;
        setDirty();
    }

    private Optional<Holding> holding(GlobalPos bell) {
        return Optional.ofNullable(holdings.get(bell));
    }

    private static Optional<BlockPos> nearestDesk(ServerLevel level, BlockPos bell) {
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
            entry.putDouble("wagesPaid", holding.wagesPaid);
            entry.putDouble("wagesUnpaid", holding.wagesUnpaid);
            entry.putLong("harvested", holding.harvested);
            entry.putDouble("laborShare", holding.laborShare);
            entry.putDouble("salesSince", holding.salesSince);
            entry.putDouble("salesPerDay", holding.salesPerDay);
            entry.putDouble("valuePerDay", holding.valuePerDay);
            entry.putDouble("treasury", holding.treasury);
            entry.putDouble("tollRate", holding.tollRate);
            entry.putInt("tollDirection", holding.tollDirection);
            entry.putDouble("tollIncomeToday", holding.tollIncomeToday);
            entry.putDouble("tollIncomeYesterday", holding.tollIncomeYesterday);
            entry.putLong("lastTollReview", holding.lastTollReview);
            entry.putInt("guards", holding.guards);
            entry.putDouble("guardUpkeepOwed", holding.guardUpkeepOwed);
            entry.putDouble("guardFoodOwed", holding.guardFoodOwed);
            entry.putLong("lastGuardUpkeep", holding.lastGuardUpkeep);
            entry.putDouble("trafficSince", holding.trafficSince);
            entry.putDouble("trafficPerDay", holding.trafficPerDay);
            entry.putInt("deserted", holding.deserted);
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
                holding.wagesPaid = entry.getDouble("wagesPaid");
                holding.wagesUnpaid = entry.getDouble("wagesUnpaid");
                holding.harvested = entry.getLong("harvested");
                if (entry.contains("laborShare")) {
                    holding.laborShare = entry.getDouble("laborShare");
                }
                holding.salesSince = entry.getDouble("salesSince");
                holding.salesPerDay = entry.getDouble("salesPerDay");
                holding.valuePerDay = entry.getDouble("valuePerDay");
                holding.treasury = entry.getDouble("treasury");
                if (entry.contains("tollRate")) {
                    holding.tollRate = entry.getDouble("tollRate");
                    holding.tollDirection = entry.getInt("tollDirection");
                    holding.tollIncomeToday = entry.getDouble("tollIncomeToday");
                    holding.tollIncomeYesterday = entry.getDouble("tollIncomeYesterday");
                    holding.lastTollReview = entry.getLong("lastTollReview");
                }
                holding.guards = entry.getInt("guards");
                holding.guardUpkeepOwed = entry.getDouble("guardUpkeepOwed");
                holding.guardFoodOwed = entry.getDouble("guardFoodOwed");
                holding.lastGuardUpkeep = entry.contains("lastGuardUpkeep") ? entry.getLong("lastGuardUpkeep") : -1;
                holding.trafficSince = entry.getDouble("trafficSince");
                holding.trafficPerDay = entry.getDouble("trafficPerDay");
                holding.deserted = entry.getInt("deserted");
                holding.perDay = readDoubles(entry.getCompound("perDay"));
                holding.pending = readDoubles(entry.getCompound("pending"));
                territory.holdings.put(bell, holding);
            });
        }
        return territory;
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
