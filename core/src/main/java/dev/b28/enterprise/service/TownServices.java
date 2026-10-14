package dev.b28.enterprise.service;

import dev.b28.enterprise.caravan.Atlas;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.territory.Territory;
import dev.b28.enterprise.territory.Workshop;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.GlobalPos;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;

/**
 * Services the trades do for towns, their own or others: a Cleric's blessing, a Farmer's help with the harvest, a
 * Cartographer's map swap and charting, a Librarian's bookbinding.
 */
public final class TownServices {
    public static final String CLERIC = "minecraft:cleric";
    public static final String FARMER = "minecraft:farmer";
    public static final String CARTOGRAPHER = "minecraft:cartographer";
    public static final String LIBRARIAN = "minecraft:librarian";

    /** A visiting Cleric's blessing costs this much and lasts two days and a day a level; a resident's lasts a day. */
    public static final double BLESSING_EMERALDS = 2;
    public static final double BLESSING_DAYS = 2;
    public static final double HOME_BLESSING_DAYS = 1;

    /** A visiting Farmer brings in a day's harvest from fields no one tends, as many plots as a Farmer of its level. */
    public static final double HELP_DAYS = 1;

    /** A town pays this much for each market it learns of from a map swap beyond those it teaches. */
    public static final double MAP_EMERALDS = 0.25;

    /** A Cartographer at home charts the country this far around, every so many days. */
    public static final int CHART_RANGE = 800;
    public static final long CHART_TICKS = 10 * 24000L;

    /** Bookbinding: 3 paper and a leather make a book; at most 16 a level each time; a visitor charges for each. */
    public static final int PAPER_PER_BOOK = 3;
    public static final int BOOKS_PER_LEVEL = 16;
    public static final double BINDING_EMERALDS = 0.05;

    private TownServices() {}

    static void register() {
        Services.register(new Service("enterprise:blessing", CLERIC, Service.Where.ABROAD,
                "blesses a town without a Cleric of its own: happier for a few days",
                TownServices::blessingWanted, TownServices::bless));
        Services.register(new Service("enterprise:harvest_help", FARMER, Service.Where.ABROAD,
                "helps bring in the harvest from fields no one tends, for a farmhand's share",
                TownServices::harvestWanted, TownServices::helpHarvest));
        Services.register(new Service("enterprise:map_swap", CARTOGRAPHER, Service.Where.ABROAD,
                "swaps maps: each town learns of the markets the other knows",
                TownServices::swapWanted, TownServices::swapMaps));
        Services.register(new Service("enterprise:bookbinding_abroad", LIBRARIAN, Service.Where.ABROAD,
                "binds books for a town short of reading, from its paper and leather",
                TownServices::bindingWanted, TownServices::bind));
        Services.register(new Service("enterprise:home_blessing", CLERIC, Service.Where.HOME,
                "blesses its own town: happier while it stays",
                visit -> visit.bell().filter(bell -> visit.territory().blessedUntil(bell) < visit.now() + 12000).isPresent(),
                TownServices::bless));
        Services.register(new Service("enterprise:charting", CARTOGRAPHER, Service.Where.HOME,
                "charts the country around: its town learns of markets within " + CHART_RANGE + " blocks",
                TownServices::chartingWanted, TownServices::chart));
        Services.register(new Service("enterprise:bookbinding", LIBRARIAN, Service.Where.HOME,
                "binds books from paper and leather while its town is short of reading",
                TownServices::bindingWanted, TownServices::bind));
    }

    /** Does the town served have a resident of this trade (at home now)? Then it does this for itself. */
    private static boolean hasOwn(Visit visit, String trade) {
        return visit.bell().map(bell -> Services.residents(visit.level(), bell).containsKey(trade)).orElse(false);
    }

    // A Cleric's blessing.

    private static boolean blessingWanted(Visit visit) {
        Optional<GlobalPos> bell = visit.bell();
        if (bell.isEmpty() || hasOwn(visit, CLERIC)) {
            return false;
        }
        boolean lasting = visit.territory().blessedUntil(bell.get()) > visit.now() + 24000;
        return !lasting && (visit.planning() || visit.budget() >= BLESSING_EMERALDS);
    }

    private static double bless(Visit visit) {
        Territory territory = visit.territory();
        double days = visit.abroad() ? BLESSING_DAYS + visit.skill() : HOME_BLESSING_DAYS;
        visit.bell().ifPresent(bell -> territory.bless(bell, visit.now() + (long) (days * 24000)));
        return visit.abroad() ? BLESSING_EMERALDS : 0;
    }

    // A Farmer's help with the harvest.

    private static boolean harvestWanted(Visit visit) {
        Optional<GlobalPos> bell = visit.bell();
        if (bell.isEmpty() || visit.territory().idlePerDay(bell.get()).isEmpty()) {
            return false;
        }
        if (visit.planning()) {
            // Only a Farmer its own fields can spare: its town tends all it has with a Farmer's plots left over.
            return visit.territory().bellOf(visit.from())
                    .map(home -> visit.territory().spareFarmPlots(home) >= Territory.PLOTS_PER_FARMER_LEVEL)
                    .orElse(true);
        }
        return visit.budget() > 0;
    }

    private static double helpHarvest(Visit visit) {
        Territory territory = visit.territory();
        GlobalPos bell = visit.bell().orElseThrow();
        Map<String, Double> idle = territory.idlePerDay(bell);
        double idlePlots = idle.values().stream().mapToDouble(Double::doubleValue).sum() / Territory.YIELD_PER_PLOT_PER_DAY;
        double share = Math.min(1, Territory.PLOTS_PER_FARMER_LEVEL * visit.skill() / idlePlots);
        Map<String, Double> produce = new LinkedHashMap<>();
        idle.forEach((item, perDay) -> produce.put(item, perDay * share * HELP_DAYS));
        double value = produce.entrySet().stream()
                .mapToDouble(e -> e.getValue() / Prices.itemsPerEmerald(e.getKey())).sum();
        double fee = value * territory.laborShare(bell);
        if (fee > visit.budget()) {
            double scale = visit.budget() / fee; // it pays for what it can: the rest stays in the fields
            produce.replaceAll((item, count) -> count * scale);
            fee = visit.budget();
        }
        territory.owe(bell, produce);
        return fee;
    }

    // A Cartographer's map swap and charting.

    private static Set<GlobalPos> marketsKnown(Visit visit, GlobalPos desk) {
        Set<GlobalPos> known = new LinkedHashSet<>(Atlas.get(visit.server()).known(visit.server(), desk));
        known.add(desk);
        return known;
    }

    private static boolean swapWanted(Visit visit) {
        Set<GlobalPos> ours = marketsKnown(visit, visit.from());
        Set<GlobalPos> theirs = marketsKnown(visit, visit.at());
        return !ours.containsAll(theirs) || !theirs.containsAll(ours);
    }

    private static double swapMaps(Visit visit) {
        Atlas atlas = Atlas.get(visit.server());
        Set<GlobalPos> ours = marketsKnown(visit, visit.from());
        Set<GlobalPos> theirs = marketsKnown(visit, visit.at());
        List<GlobalPos> taught = ours.stream().filter(m -> !theirs.contains(m)).toList();
        List<GlobalPos> learned = theirs.stream().filter(m -> !ours.contains(m)).toList();
        taught.forEach(market -> atlas.learn(visit.at(), market, visit.now()));
        learned.forEach(visit::hear); // our copy of their map rides home
        return MAP_EMERALDS * Math.max(0, taught.size() - learned.size());
    }

    private static boolean chartingWanted(Visit visit) {
        long last = ServiceLedger.get(visit.server()).last("enterprise:charting", visit.at());
        return last < 0 || visit.now() - last >= CHART_TICKS;
    }

    private static double chart(Visit visit) {
        GlobalPos desk = visit.at();
        Atlas.markets(visit.server()).stream()
                .filter(m -> m.dimension().equals(desk.dimension())
                        && m.pos().distSqr(desk.pos()) <= (double) CHART_RANGE * CHART_RANGE)
                .forEach(visit::hear);
        return 0;
    }

    // A Librarian's bookbinding.

    private static boolean bindingWanted(Visit visit) {
        Optional<GlobalPos> bell = visit.bell();
        if (bell.isEmpty() || visit.abroad() && hasOwn(visit, LIBRARIAN)) {
            return false;
        }
        if (visit.planning()) {
            Warehouse warehouse = Warehouse.get(visit.server());
            return visit.territory().comfortsMet(bell.get()).getOrDefault("reading", 0.0) < 1
                    && warehouse.onHand(visit.at(), Items.PAPER) >= PAPER_PER_BOOK
                    && warehouse.onHand(visit.at(), Items.LEATHER) >= 1;
        }
        return booksWanted(visit) > 0 && (!visit.abroad() || visit.budget() >= BINDING_EMERALDS);
    }

    /** Books the town could use and has the makings of: paper beyond what its lockers want, and leather. */
    private static int booksWanted(Visit visit) {
        var stock = visit.stock();
        int wanted = Workshop.comfortWanted(visit.territory(), visit.at(), stock, "reading", Items.BOOK);
        Warehouse warehouse = Warehouse.get(visit.server());
        int paperKept = warehouse.capacity(visit.at(), Items.PAPER) > 0
                ? warehouse.par(visit.at(), Items.PAPER, true, visit.now()) : 0;
        int paper = Math.max(0, stock.count(new ItemCost(Items.PAPER, 1)) - paperKept);
        int leather = stock.count(new ItemCost(Items.LEATHER, 1));
        return Math.min(Math.min(Math.min(wanted, BOOKS_PER_LEVEL * visit.skill()), Items.BOOK.getDefaultMaxStackSize()),
                Math.min(paper / PAPER_PER_BOOK, leather));
    }

    private static double bind(Visit visit) {
        int books = booksWanted(visit);
        if (visit.abroad()) {
            books = Math.min(books, (int) Math.floor(visit.budget() / BINDING_EMERALDS));
        }
        if (books <= 0) {
            return 0;
        }
        var stock = visit.stock();
        int paper = stock.take(new ItemCost(Items.PAPER, 1), books * PAPER_PER_BOOK).stream()
                .mapToInt(ItemStack::getCount).sum();
        int leather = stock.take(new ItemCost(Items.LEATHER, 1), books).stream().mapToInt(ItemStack::getCount).sum();
        int made = Math.min(paper / PAPER_PER_BOOK, leather);
        if (paper > made * PAPER_PER_BOOK) {
            stock.insert(new ItemStack(Items.PAPER, paper - made * PAPER_PER_BOOK));
        }
        if (leather > made) {
            stock.insert(new ItemStack(Items.LEATHER, leather - made));
        }
        ItemStack rest = stock.insert(new ItemStack(Items.BOOK, made)); // inputs taken: there's room
        made -= rest.getCount();
        return visit.abroad() ? made * BINDING_EMERALDS : 0;
    }
}
