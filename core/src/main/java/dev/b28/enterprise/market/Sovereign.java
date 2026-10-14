package dev.b28.enterprise.market;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.army.Army;
import dev.b28.enterprise.contract.Bonus;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.contract.Incoterm;
import dev.b28.enterprise.contract.Levy;
import dev.b28.enterprise.contract.Term;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.MoneyChanger;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.territory.Labour;
import dev.b28.enterprise.territory.Territory;
import dev.ithundxr.createnumismatics.Numismatics;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import dev.ithundxr.createnumismatics.content.bank.CardItem;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.saveddata.SavedData;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * A sovereign shifting the region onto a war footing, through the region's own institutions. Its capital is a desk
 * whose Blaze Banker is the royal treasury, which it mints coin into each day (debasing the spur). Mobilising, it
 * stockpiles kit: its capital's Merchant contracts towns for it at a premium, with bonus clauses for good enchantments,
 * and posts as wanted what no town can supply; towns answer by making it, buying it from their own villagers, and
 * putting idle hands to the trades it calls for, their smiths trading their way up to master (who sell diamond gear).
 * It buys gold through a Numismatics shop by the capital. At war it levies soldiers from each town by contract (paid a
 * day each, owed home), outfits them from the capital's warehouse, and replaces what the campaign wears out. At peace
 * the soldiers walk home and the minting falls back to the crown's peacetime revenue.
 *
 * <p>A capital can be named in peacetime: its port's {@link MetalExchange} imports iron, coal, flint and diamonds and
 * makes a market in them, paid from the crown's revenue (minted, until taxes stand in for it), so smiths across the
 * region train and mines open before any war; its own smiths sell to the depot when it stockpiles.
 */
public final class Sovereign extends SavedData {
    private static final String NAME = "enterprise_sovereign";

    public enum Phase { PEACE, MOBILISING, WAR }

    /** Mobilising, it pays half again over worth; at war, two and a half times. */
    public static final double MOBILISING_PREMIUM = 1.5;
    public static final double WAR_PREMIUM = 2.5;
    /** It buys gold at three times its worth. */
    public static final double GOLD_PREMIUM = 3;
    /** Villagers of a trade the crown wants in its realm, so many at the level. */
    /**
     * Villagers of a trade the crown wants in its realm, so many at the level; if {@code sells} is given, counted by
     * the towns whose villagers offer one of those goods (a trade's offers are dealt at random).
     */
    public record Trained(String trade, int count, int level, List<Item> sells) {}

    /**
     * The villagers the crown wants in its realm: master smiths (diamond swords and armour are theirs to sell), and
     * Butchers selling meat for emeralds (hearty food for the army). Short of them, it tenders for
     * training on its market: this much a master, less for a lower level (a third up front), due in so many days.
     */
    public static final List<Trained> TRAINED = List.of(new Trained("minecraft:weaponsmith", 2, 5, List.of()),
            new Trained("minecraft:armorer", 2, 5, List.of()), new Trained("minecraft:toolsmith", 1, 5, List.of()),
            new Trained("minecraft:butcher", 2, 2, Army.MEATS));
    public static final double TRAINING_BONUS = 24;
    /** And it buys what the apprentices make on the way, at its worth. */
    public static final double OFFTAKE = 1.0;
    public static final long TRAINING_TICKS = 15 * 24000L;

    /** The front lies this far north of the capital: an off-map market, reached by its own road. */
    public static final int FRONT_DISTANCE = 1500;

    /** A soldier is paid this much a day, owed to its town. */
    public static final double SOLDIER_PAY = 0.25;
    /**
     * Short of its planned army after the levies, the crown hires mercenaries at the market across the sea, by contract
     * like a levy: four times a levied soldier's pay, owed to that market. Its caravan brings them, so many a contract.
     */
    public static final double MERCENARY_PAY = 1.0;
    public static final int MERCENARIES_MOST = 8;
    /** A town sends its idle hands and this share of the rest, at least one and at most four. */
    public static final double LEVY_SHARE = 0.2;
    public static final int LEVY_MOST = 4;

    /**
     * The stockpile it builds while mobilising, and keeps up at war: kit for an army of {@link #PLANNED_ARMY}, the best
     * a third of it (diamond, from master smiths), and food put by.
     */
    public static final int PLANNED_ARMY = 24;
    /** Days of food for the planned army the crown puts by while mobilising, against the campaign's first weeks. */
    public static final int RATION_STORE_DAYS = 20;
    public static final Map<Item, Integer> STOCKPILE = stockpile();
    /**
     * Hearty rations put by too (good food is good morale): half the campaign's store, in food points, of any meat the
     * towns sell (a Butcher's porkchops, chicken or stew).
     */
    public static final int MEAT_POINTS = PLANNED_ARMY * Army.RATION * Sovereign.RATION_STORE_DAYS / 2;

    private static Map<Item, Integer> stockpile() {
        Map<Item, Integer> m = new LinkedHashMap<>();
        int best = PLANNED_ARMY * 2 / 3;
        m.put(Items.DIAMOND_SWORD, best);
        m.put(Items.IRON_SWORD, PLANNED_ARMY - best);
        // Armour in whole sets: a helmet, chestplate, leggings and boots.
        for (Item piece : List.of(Items.DIAMOND_HELMET, Items.DIAMOND_CHESTPLATE, Items.DIAMOND_LEGGINGS, Items.DIAMOND_BOOTS)) {
            m.put(piece, PLANNED_ARMY / 3);
        }
        for (Item piece : List.of(Items.IRON_HELMET, Items.IRON_CHESTPLATE, Items.IRON_LEGGINGS, Items.IRON_BOOTS)) {
            m.put(piece, PLANNED_ARMY * 2 / 3);
        }
        m.put(Items.CHAINMAIL_CHESTPLATE, 6);
        m.put(Items.LEATHER_CHESTPLATE, 6);
        m.put(Items.SHIELD, PLANNED_ARMY / 2);
        m.put(Items.BOW, 8);
        m.put(Items.ARROW, 256);
        // And rations put by in the run-up: half the campaign's store as bread (meat the other half: MEAT_POINTS).
        m.put(Items.BREAD, PLANNED_ARMY * Army.RATION * RATION_STORE_DAYS / 2 / 5);
        return m;
    }

    /** Bonus clauses on its contracts: quality is paid for, never required. */
    public static List<Term> bonuses(Item item, Army.Theatre theatre) {
        String path = BuiltInRegistries.ITEM.getKey(item).getPath();
        List<Term> terms = new ArrayList<>();
        if (path.endsWith("_sword")) {
            terms.add(new Bonus(Map.of("minecraft:unbreaking", 1, "minecraft:sharpness", 2), 25));
            if (theatre == Army.Theatre.UNDEAD) {
                terms.add(new Bonus(Map.of("minecraft:smite", 3), 30));
            }
        } else if (path.endsWith("_helmet") || path.endsWith("_chestplate") || path.endsWith("_leggings")
                || path.endsWith("_boots")) {
            terms.add(new Bonus(Map.of(theatre.armourEnchant, 2), 25));
            terms.add(new Bonus(Map.of("minecraft:unbreaking", 2), 15));
        } else if (item == Items.BOW) {
            terms.add(new Bonus(Map.of("minecraft:power", 2), 25));
            terms.add(new Bonus(Map.of("minecraft:unbreaking", 1), 10));
        } else if (item == Items.SHIELD) {
            terms.add(new Bonus(Map.of("minecraft:unbreaking", 1), 10));
        }
        return terms;
    }

    private Phase phase = Phase.PEACE;
    private GlobalPos capital;
    private UUID treasury;
    private int mintPerDay;
    private int peaceRevenue; // spurs a day minted in peacetime
    private long lastDay = -1;
    private long phaseSince = -1;
    private final List<BlockPos> shops = new ArrayList<>();
    private final Map<String, Integer> received = new java.util.TreeMap<>(); // gold bought at the shop
    private final Map<Item, Integer> unmet = new LinkedHashMap<>(); // kit no town could be contracted for
    private final Map<Item, Integer> shortfall = new LinkedHashMap<>(); // all it lacks at the capital, as of today
    private final Map<String, Integer> raised = new LinkedHashMap<>(); // tenders gone untaken, by trade: its price rises

    public static Sovereign get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Sovereign::new, Sovereign::load, null), NAME);
    }

    public Phase phase() {
        return phase;
    }

    public Optional<GlobalPos> desk() {
        return Optional.ofNullable(capital);
    }

    public Optional<BankAccount> treasury() {
        return treasury == null ? Optional.empty() : Optional.ofNullable(Numismatics.BANK.getAccount(treasury));
    }

    public List<BlockPos> shops() {
        return List.copyOf(shops);
    }

    public int mintPerDay() {
        return mintPerDay;
    }

    public Map<String, Integer> received() {
        return Map.copyOf(received);
    }

    /** What it lacks at this desk of its stockpile: its own villagers who sell it are bought from. */
    public Map<Item, Integer> shortfall(GlobalPos desk) {
        return desk.equals(capital) ? Map.copyOf(shortfall) : Map.of();
    }

    public int peaceRevenue() {
        return peaceRevenue;
    }

    /**
     * Names the capital in peacetime: its Blaze Banker becomes the royal treasury (a new account if it has none),
     * {@code perDay} spurs a day are minted into it (the crown's revenue), and its port's metal exchange opens.
     */
    public void establish(ServerLevel level, GlobalPos at, int perDay) {
        capital = at;
        peaceRevenue = perDay;
        treasury = MoneyChanger.bankAt(level, at.pos()).map(account -> account.id).orElseGet(() -> {
            UUID id = UUID.randomUUID();
            Numismatics.BANK.getOrCreateAccount(id, BankAccount.Type.PLAYER);
            return id;
        });
        if (phase == Phase.PEACE) {
            mintPerDay = perDay;
            phaseSince = level.getGameTime();
        }
        lastDay = level.getGameTime() - 24000; // today's revenue is due
        if (!MetalExchange.get(level.getServer()).at(at)) {
            MetalExchange.get(level.getServer()).open(level, at, treasury);
        }
        Bank.get(level.getServer()).open(at); // the crown's bank: discounting and lending through its caravans
        setDirty();
        Enterprise.LOGGER.info("The sovereign's capital at {}: {} spurs a day, its metal exchange open",
                at.pos().toShortString(), perDay);
    }

    /** What the war wants at this desk that no town could be contracted for: posted as wanted. */
    public Map<Item, Integer> unmet(GlobalPos desk) {
        return desk.equals(capital) ? Map.copyOf(unmet) : Map.of();
    }

    /**
     * Mobilisation: the capital's Blaze Banker becomes the royal treasury (a new account if it has none), minting
     * {@code perDay} spurs a day into it, and a shop goes up by the capital buying gold, paid from it.
     */
    public int mobilise(ServerLevel level, GlobalPos at, int perDay) {
        peace(level);
        capital = at;
        mintPerDay = perDay;
        treasury = MoneyChanger.bankAt(level, at.pos()).map(account -> account.id).orElseGet(() -> {
            UUID id = UUID.randomUUID();
            Numismatics.BANK.getOrCreateAccount(id, BankAccount.Type.PLAYER);
            return id;
        });
        phase = Phase.MOBILISING;
        phaseSince = level.getGameTime();
        lastDay = level.getGameTime(); // today's coin is this
        mint(level, level.getGameTime());
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        Block vendor = BuiltInRegistries.BLOCK.get(ResourceLocation.parse("numismatics:vendor"));
        Item card = BuiltInRegistries.ITEM.get(ResourceLocation.parse("numismatics:red_card"));
        BlockPos pos = at.pos().offset(0, 0, -10);
        level.setBlock(pos, vendor.defaultBlockState(), 3);
        if (level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
            shop.setMode(VendorBlockEntity.Mode.BUY);
            shop.filterContainer.setItem(0, new ItemStack(Items.GOLD_INGOT, 8));
            Shops.setPrice(shop, (int) Math.ceil(Prices.emeraldsEach(Items.GOLD_INGOT) * 8 * GOLD_PREMIUM * rate));
            ItemStack bound = new ItemStack(card);
            CardItem.set(bound, treasury);
            shop.cardContainer.setItem(0, bound);
            shop.setChanged();
            shops.add(pos);
        }
        setDirty();
        Enterprise.LOGGER.info("Mobilisation: the sovereign at {} mints {} spurs a day", at.pos().toShortString(), perDay);
        return shops.size();
    }

    /** War: each town it knows by a road is levied by contract, the copies riding with its caravans. Returns levies drafted. */
    public int declare(ServerLevel level, Army.Theatre theatre, long now) {
        if (capital == null) {
            return 0;
        }
        phase = Phase.WAR;
        phaseSince = now;
        MinecraftServer server = level.getServer();
        Army.get(server).setTheatre(theatre);
        openFront(level, now);
        ContractBook book = ContractBook.get(server);
        Territory territory = Territory.get(server);
        var settlements = dev.b28.enterprise.settlement.Settlements.get(server);
        int levies = 0;
        for (var route : dev.b28.enterprise.caravan.CaravanLedger.get(server).routes()) {
            if (!route.from().equals(capital)) {
                continue;
            }
            Optional<GlobalPos> bell = territory.bellOf(route.to());
            if (bell.isEmpty()) {
                continue;
            }
            var roster = settlements.roster(bell.get()).values();
            long idle = roster.stream().filter(r -> !r.baby() && Labour.idle(r.profession())).count();
            long rest = roster.stream().filter(r -> !r.baby() && !Labour.idle(r.profession())).count();
            // A town with a barracks can put more men under arms: as many again as it has beds there.
            int most = LEVY_MOST + dev.b28.enterprise.merchant.Depots.get(server).barracksBeds(route.to());
            int soldiers = (int) Math.max(1, Math.min(most, idle + Math.round(rest * LEVY_SHARE)));
            book.draft(route.to(), capital, List.of(new Levy(soldiers, SOLDIER_PAY)), Incoterm.EXW, now, 24000, 1, now,
                    capital, false);
            levies++;
        }
        setDirty();
        Enterprise.LOGGER.info("War against {}: {} levies", theatre, levies);
        return levies;
    }

    /**
     * Meat for the stockpile, {@link #MEAT_POINTS} of it, counted in food points: any kind will do. What's held and
     * already contracted counts; the rest is sourced kind by kind, from whoever sells it. Unmet, it's posted as wanted.
     */
    void sourceMeat(ServerLevel level, MerchantStock depot, Territory territory, long now, double premium) {
        MinecraftServer server = level.getServer();
        int points = MEAT_POINTS;
        for (Item meat : Army.MEATS) {
            points -= (depot.count(new ItemCost(meat, 1)) + Broker.coming(server, capital, meat)) * Army.nutrition(meat);
        }
        if (points <= 0) {
            return;
        }
        shortfall.put(Army.MEATS.get(0), (points + 7) / 8);
        for (Item meat : Army.MEATS) {
            if (points <= 0) {
                return;
            }
            int coming = Broker.coming(server, capital, meat);
            int n = (points + Army.nutrition(meat) - 1) / Army.nutrition(meat);
            Broker.source(level, capital, territory, meat, coming + n, now, premium, List.of());
            points -= (Broker.coming(server, capital, meat) - coming) * Army.nutrition(meat);
        }
        if (points > 0) {
            unmet.put(Army.MEATS.get(0), (points + 7) / 8); // posted as wanted: a pork chop's worth of points each
        }
    }

    /**
     * At war, short of its planned army (counting soldiers serving, recruits waiting or on the road, and those levied
     * and not yet raised), the crown hires mercenaries at the market across the sea: one contract open at a time, its
     * caravan bringing them to the barracks like any levy. Returns the mercenaries contracted.
     */
    int hireMercenaries(ServerLevel level, Army army, long now) {
        MinecraftServer server = level.getServer();
        Optional<GlobalPos> sea = MetalExchange.get(server).overseas()
                .filter(far -> dev.b28.enterprise.merchant.OffMap.get(server).is(far));
        if (sea.isEmpty() || capital == null) {
            return 0;
        }
        ContractBook book = ContractBook.get(server);
        int coming = 0;
        boolean hiring = false;
        for (var contract : book.of(capital)) {
            if (contract.buyer().equals(capital) && contract.levy().isPresent()
                    && (contract.inForce() || contract.state() == dev.b28.enterprise.contract.Contract.State.DRAFTED)) {
                int left = Math.max(0, contract.levy().get().soldiers() - contract.delivered());
                coming += left;
                hiring |= left > 0 && contract.seller().equals(sea.get());
            }
        }
        int strength = army.size() + army.waiting() + coming
                + dev.b28.enterprise.caravan.CaravanLedger.get(server).recruitsOnTheRoad();
        int short_ = PLANNED_ARMY - strength;
        if (short_ <= 0 || hiring) {
            return 0;
        }
        int hired = Math.min(short_, MERCENARIES_MOST);
        book.draft(sea.get(), capital, List.of(new Levy(hired, MERCENARY_PAY)), Incoterm.EXW, now, 24000, 1, now,
                capital, false);
        Exchange.get(server).note(capital, now, "hired %d mercenaries across the sea, at %.2f emeralds a day"
                .formatted(hired, MERCENARY_PAY));
        setDirty();
        return hired;
    }

    /**
     * The front: an off-map market with a quartermaster, its money the treasury's, on a road from the capital. What the
     * army needs reaches it as everything else does: by contract, carried by the capital's caravans.
     */
    void openFront(ServerLevel level, long now) {
        MinecraftServer server = level.getServer();
        GlobalPos front = front().orElseThrow();
        dev.b28.enterprise.merchant.OffMap.get(server).open(front, "the front", treasury, false); // keeps all for the army
        dev.b28.enterprise.merchant.Warehouse.get(server).recordLevel(front, 5); // a quartermaster knows his trade
        var ledger = dev.b28.enterprise.caravan.CaravanLedger.get(server);
        if (ledger.route(capital, front).isEmpty()) {
            ledger.addRoute(capital, front);
        }
        dev.b28.enterprise.caravan.Atlas.get(server).learnBoth(capital, front, now);
        Army.get(server).setFront(front);
    }

    /** Is this the crown's own desk: its capital, or its army's front? Its bills are drawn on the treasury. */
    public boolean isCrown(GlobalPos desk) {
        return desk.equals(capital) || front().filter(desk::equals).isPresent();
    }

    /** Where the front is, given the capital. */
    public Optional<GlobalPos> front() {
        return desk().map(c -> GlobalPos.of(c.dimension(), c.pos().offset(0, 0, -FRONT_DISTANCE)));
    }

    /** Peace: the army walks home, the gold shop comes down, the minting stops. Returns the soldiers sent home. */
    public int peace(ServerLevel level) {
        int home = capital == null ? 0 : Army.get(level.getServer()).demobilise(level.getServer(), capital, level.getGameTime());
        if (capital != null) { // the road to the front closes once its caravan is home
            var ledger = dev.b28.enterprise.caravan.CaravanLedger.get(level.getServer());
            front().flatMap(f -> ledger.route(capital, f)).ifPresent(r -> ledger.removeRoute(r.id()));
            Army.get(level.getServer()).setFront(null);
        }
        shops.forEach(pos -> level.setBlock(pos, Blocks.AIR.defaultBlockState(), 3));
        shops.clear();
        mintPerDay = peaceRevenue;
        phase = Phase.PEACE;
        phaseSince = level.getGameTime();
        unmet.clear();
        shortfall.clear();
        setDirty();
        return home;
    }

    /** A day's coin into the treasury, and the gold the shop bought sent to the mint. */
    void mint(ServerLevel level, long now) {
        for (BlockPos pos : shops) {
            if (level.isLoaded(pos) && level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                for (int i = 0; i < shop.items.size(); i++) {
                    ItemStack stack = shop.items.get(i);
                    if (!stack.isEmpty()) {
                        received.merge(BuiltInRegistries.ITEM.getKey(stack.getItem()).getPath(), stack.getCount(),
                                Integer::sum);
                        shop.items.set(i, ItemStack.EMPTY);
                    }
                }
                shop.setChanged();
            }
        }
        treasury().ifPresent(account -> {
            account.deposit(mintPerDay);
            Mint.get(level.getServer()).mint(Mint.DEFAULT_FACTION, mintPerDay, now);
        });
        setDirty();
    }

    /** What it wants on hand at the capital now: the stockpile, and at war a set of kit for each soldier besides. */
    public Map<Item, Integer> targets(Army army) {
        Map<Item, Integer> targets = new LinkedHashMap<>(STOCKPILE);
        if (phase == Phase.WAR) {
            army.wants(rationDays(army)).forEach((item, n) -> targets.merge(item, n, Integer::sum));
        }
        return targets;
    }

    /** Days of rations the army keeps at the front: more the further it is from the capital. */
    int rationDays(Army army) {
        return army.front().filter(front -> capital != null)
                .map(front -> Army.rationDays((int) Math.sqrt(front.pos().distSqr(capital.pos()))))
                .orElse(Army.rationDays(0));
    }

    /**
     * Each day of mobilisation or war: mints, has its capital's Merchant contract towns for what the stockpile lacks
     * (at the premium, with bonus clauses), posts what no town can supply, and at war campaigns.
     */
    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        Sovereign sovereign = get(server);
        long now = server.overworld().getGameTime();
        if (sovereign.capital == null || sovereign.phase == Phase.PEACE && sovereign.mintPerDay <= 0) {
            return;
        }
        ServerLevel level = server.getLevel(sovereign.capital.dimension());
        if (level == null || !level.isLoaded(sovereign.capital.pos())) {
            return;
        }
        if (sovereign.lastDay >= 0 && now - sovereign.lastDay < 24000) {
            return;
        }
        sovereign.lastDay = now;
        sovereign.mint(level, now);
        sovereign.offerTraining(level, now);
        if (sovereign.phase == Phase.PEACE) {
            sovereign.setDirty();
            return; // the crown's revenue only: nothing to stockpile
        }
        Army army = Army.get(server);
        MerchantStock depot = MerchantStock.at(level, sovereign.capital.pos());
        double premium = sovereign.phase == Phase.WAR ? WAR_PREMIUM : MOBILISING_PREMIUM;
        Territory territory = Territory.get(server);
        sovereign.unmet.clear();
        sovereign.shortfall.clear();
        for (Map.Entry<Item, Integer> target : sovereign.targets(army).entrySet()) {
            int need = target.getValue() - depot.count(new ItemCost(target.getKey(), 1));
            if (need <= 0) {
                continue;
            }
            sovereign.shortfall.put(target.getKey(), need);
            boolean covered = Broker.source(level, sovereign.capital, territory, target.getKey(), need, now, premium,
                    bonuses(target.getKey(), army.theatre()));
            if (!covered) {
                sovereign.unmet.put(target.getKey(), need);
            }
        }
        sovereign.sourceMeat(level, depot, territory, now, premium);
        if (sovereign.phase == Phase.WAR) {
            sovereign.hireMercenaries(level, army, now);
        }
        if (sovereign.phase == Phase.WAR && army.front().isPresent()) {
            // The front's quartermaster: what the army will want there, beyond what it holds, by contract from the
            // capital (its caravans carry it); and the army campaigns on what it has.
            GlobalPos front = army.front().get();
            MerchantStock lines = MerchantStock.at(level, front.pos());
            var warehouse = dev.b28.enterprise.merchant.Warehouse.get(server);
            for (Map.Entry<Item, Integer> want : army.wants(sovereign.rationDays(army)).entrySet()) {
                if (want.getKey() == Items.BREAD) {
                    // Rations, in food points: any food the lines hold counts. Food heartier than bread that the
                    // capital holds goes first (good food is good morale), counted as it's contracted; bread makes up
                    // the rest.
                    int points = want.getValue() * Army.nutrition(Items.BREAD) - Army.foodPoints(lines);
                    for (Item food : Army.heartiestFirst(depot)) {
                        if (points <= 0 || Army.heartiness(food) < 0.05) {
                            break;
                        }
                        int coming = Broker.coming(server, front, food);
                        int n = Math.min((points + Army.nutrition(food) - 1) / Army.nutrition(food),
                                coming + warehouse.onHand(sovereign.capital, food));
                        if (n > 0) {
                            Broker.source(level, front, territory, food, n, now);
                            points -= Math.min(n, Broker.coming(server, front, food)) * Army.nutrition(food);
                        }
                    }
                    if (points > 0) {
                        Broker.source(level, front, territory, Items.BREAD,
                                (points + Army.nutrition(Items.BREAD) - 1) / Army.nutrition(Items.BREAD), now);
                    }
                    continue;
                }
                // A weapon, and a piece of armour for each place, of any kind: the best the capital holds first
                // (diamond from its master smiths, iron, the apprentices' axes), then what's wanted.
                List<Army.Gear> kinds = want.getKey() == Items.IRON_SWORD
                        ? Army.WEAPONS.stream().filter(g -> g.item() != Items.BOW).toList()
                        : Army.ARMOUR.stream().filter(place -> place.stream().anyMatch(g -> g.item() == want.getKey()))
                                .findFirst().orElse(List.of());
                if (!kinds.isEmpty()) {
                    int need = want.getValue() - kinds.stream().mapToInt(g -> lines.count(new ItemCost(g.item(), 1))).sum();
                    for (Army.Gear kind : kinds) {
                        int held = warehouse.onHand(sovereign.capital, kind.item());
                        if (need > 0 && held > 0 && kind.item() != want.getKey()) {
                            Broker.source(level, front, territory, kind.item(), Math.min(need, held), now);
                            need -= Math.min(need, held);
                        }
                    }
                    if (need > 0) {
                        Broker.source(level, front, territory, want.getKey(), need, now);
                    }
                    continue;
                }
                int need = want.getValue() - lines.count(new ItemCost(want.getKey(), 1));
                if (need > 0) {
                    Broker.source(level, front, territory, want.getKey(), need, now);
                }
            }
            army.campaign(level, lines, sovereign.capital, now);
        }
        sovereign.setDirty();
    }

    /**
     * Masters short in the realm (the capital and the towns on its roads), less those being trained under contract or
     * already tendered: posted as tenders on the capital's board, for the region's Merchants to take or carry on. One
     * that lapses untaken is posted again for a quarter more: the market finds the price.
     */
    void offerTraining(ServerLevel level, long now) {
        MinecraftServer server = level.getServer();
        ContractBook book = ContractBook.get(server);
        Territory territory = Territory.get(server);
        Tenders tenders = Tenders.get(server);
        var settlements = dev.b28.enterprise.settlement.Settlements.get(server);
        for (Tenders.Tender lapsed : tenders.lapse(now)) {
            if (lapsed.board().equals(capital)) {
                raised.merge(lapsed.work().profession(), 1, Integer::sum);
            }
        }
        List<GlobalPos> realm = new ArrayList<>();
        realm.add(capital);
        for (var route : dev.b28.enterprise.caravan.CaravanLedger.get(server).routes()) {
            if (route.from().equals(capital) && territory.bellOf(route.to()).isPresent() && !realm.contains(route.to())) {
                realm.add(route.to());
            }
        }
        double premium = switch (phase) {
            case PEACE -> 1;
            case MOBILISING -> MOBILISING_PREMIUM;
            case WAR -> WAR_PREMIUM;
        };
        for (Trained wanted : TRAINED) {
            String trade = wanted.trade();
            var warehouse = dev.b28.enterprise.merchant.Warehouse.get(server);
            long masters = !wanted.sells().isEmpty()
                    ? realm.stream().filter(t -> wanted.sells().stream()
                            .anyMatch(item -> warehouse.offers(t, item).orElse(false))).count()
                    : realm.stream().flatMap(t -> territory.bellOf(t).stream())
                            .flatMap(bell -> settlements.roster(bell).values().stream())
                            .filter(r -> r.profession().equals(trade) && r.level() >= wanted.level()).count();
            long training = book.of(capital).stream()
                    .filter(c -> c.buyer().equals(capital) && (c.state() == dev.b28.enterprise.contract.Contract.State.DRAFTED
                            || c.inForce()))
                    .filter(c -> c.training().filter(t -> t.profession().equals(trade)).isPresent()).count();
            int tendered = tenders.openFor(capital, trade); // on its board, or carried on by a Merchant
            long short_ = wanted.count() - masters - training - tendered;
            if (short_ > 0) {
                double bonus = TRAINING_BONUS * wanted.level() / 5 * premium * Math.pow(1.25, raised.getOrDefault(trade, 0));
                // It takes the apprentices' work too, at its worth: a town needn't fear being left with it.
                var work = new dev.b28.enterprise.contract.Training(trade, wanted.level(), bonus, OFFTAKE);
                tenders.post(capital, capital, work, (int) short_, now + TRAINING_TICKS);
                Exchange.get(server).note(capital, now, "tendered for %d: %s".formatted(short_, work.describe()));
            }
        }
    }

    /** Days in the current phase. */
    public double daysInPhase(long now) {
        return phaseSince < 0 ? 0 : (now - phaseSince) / 24000.0;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        tag.putString("phase", phase.name());
        if (capital != null) {
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, capital).ifSuccess(t -> tag.put("capital", t));
        }
        if (treasury != null) {
            tag.putUUID("treasury", treasury);
        }
        tag.putInt("mintPerDay", mintPerDay);
        tag.putInt("peaceRevenue", peaceRevenue);
        tag.putLong("lastDay", lastDay);
        tag.putLong("phaseSince", phaseSince);
        tag.putLongArray("shops", shops.stream().mapToLong(BlockPos::asLong).toArray());
        CompoundTag got = new CompoundTag();
        received.forEach(got::putInt);
        tag.put("received", got);
        return tag;
    }

    public static Sovereign load(CompoundTag tag, HolderLookup.Provider registries) {
        Sovereign s = new Sovereign();
        s.phase = tag.contains("phase") ? Phase.valueOf(tag.getString("phase")) : Phase.PEACE;
        s.capital = tag.contains("capital")
                ? GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("capital")).result().orElse(null) : null;
        s.treasury = tag.hasUUID("treasury") ? tag.getUUID("treasury") : null;
        s.mintPerDay = tag.getInt("mintPerDay");
        s.peaceRevenue = tag.getInt("peaceRevenue");
        s.lastDay = tag.contains("lastDay") ? tag.getLong("lastDay") : -1;
        s.phaseSince = tag.contains("phaseSince") ? tag.getLong("phaseSince") : -1;
        for (long pos : tag.getLongArray("shops")) {
            s.shops.add(BlockPos.of(pos));
        }
        CompoundTag got = tag.getCompound("received");
        got.getAllKeys().forEach(k -> s.received.put(k, got.getInt(k)));
        return s;
    }
}
