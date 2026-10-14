package dev.b28.enterprise.service;

import dev.b28.enterprise.caravan.Atlas;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.territory.Smithing;
import dev.b28.enterprise.territory.Territory;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.GlobalPos;

/**
 * Services the trades do for towns, their own or others: a Cleric's blessing, a Farmer's help with the harvest, a
 * Cartographer's map swap and charting, a Toolsmith's mending; and what trades make ({@link Crafts}).
 */
public final class TownServices {
    public static final String CLERIC = "minecraft:cleric";
    public static final String FARMER = "minecraft:farmer";
    public static final String CARTOGRAPHER = "minecraft:cartographer";
    public static final String LIBRARIAN = "minecraft:librarian";
    public static final String TOOLSMITH = "minecraft:toolsmith";
    public static final String FLETCHER = "minecraft:fletcher";
    public static final String MASON = "minecraft:mason";

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

    /** A visiting Toolsmith's mending costs this much and lasts four days and a day a level. */
    public static final double MENDING_EMERALDS = 1;
    public static final double MENDING_DAYS = 4;

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
        Services.register(new Service("enterprise:home_blessing", CLERIC, Service.Where.HOME,
                "blesses its own town: happier while it stays",
                visit -> visit.bell().filter(bell -> visit.territory().blessedUntil(bell) < visit.now() + 12000).isPresent(),
                TownServices::bless));
        Services.register(new Service("enterprise:charting", CARTOGRAPHER, Service.Where.HOME,
                "charts the country around: its town learns of markets within " + CHART_RANGE + " blocks",
                TownServices::chartingWanted, TownServices::chart));
        Services.register(new Service("enterprise:mending", TOOLSMITH, Service.Where.HOME,
                "mends its town's tools: they last twice as long",
                visit -> visit.bell().filter(bell -> visit.territory().mendedUntil(bell) < visit.now() + 12000).isPresent(),
                TownServices::mend));
        Services.register(new Service("enterprise:mending_abroad", TOOLSMITH, Service.Where.ABROAD,
                "mends the tools of a town without a Toolsmith: they last twice as long for a few days, "
                        + MENDING_EMERALDS + " emerald",
                TownServices::mendingWanted, TownServices::mend));
        Crafts.register();
        BuildServices.register();
        // Worked by the ledger as days pass.
        Services.register(Service.passive("enterprise:mine", TOOLSMITH, Service.Where.HOME,
                "digs a mine: 2 raw iron and half a coal a day a level, half again with pickaxes"));
        Services.register(Service.passive("enterprise:hunting", FLETCHER, Service.Where.HOME,
                "hunts the town's land: game, hides and feathers, a quarter as much without a bow"));
        Services.register(Service.passive("enterprise:docks", "minecraft:fisherman", Service.Where.HOME,
                "builds a pier and lands 4 cod a day a level"));
        Services.register(Service.passive("enterprise:gathering", dev.b28.enterprise.territory.Labour.NITWIT,
                Service.Where.HOME, "works a hex: gathers what the land gives (logs, sand, clay, flint, flowers), half "
                        + "again as much as another villager would"));
        Services.register(Service.passive("enterprise:militia", dev.b28.enterprise.territory.Labour.NITWIT,
                Service.Where.HOME, "stands watch at the town's posts for its food, without pay"));
        Services.register(Service.road("enterprise:porter", dev.b28.enterprise.territory.Labour.NITWIT,
                "carries 9 stacks out with a caravan, then walks home"));
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

    // A Toolsmith's mending.

    private static boolean mendingWanted(Visit visit) {
        Optional<GlobalPos> bell = visit.bell();
        if (bell.isEmpty() || hasOwn(visit, TOOLSMITH)
                || visit.territory().mendedUntil(bell.get()) > visit.now() + 24000) {
            return false;
        }
        boolean inUse = visit.territory().kitShare(bell.get()).entrySet().stream()
                .anyMatch(e -> e.getValue() > 0 && (e.getKey().equals(Smithing.HOE.id()) || e.getKey().equals(Smithing.PICKAXE.id())));
        return inUse && (visit.planning() || visit.budget() >= MENDING_EMERALDS);
    }

    private static double mend(Visit visit) {
        double days = visit.abroad() ? MENDING_DAYS + visit.skill() : 1;
        visit.bell().ifPresent(bell -> visit.territory().mend(bell, visit.now() + (long) (days * 24000)));
        return visit.abroad() ? MENDING_EMERALDS : 0;
    }
}
