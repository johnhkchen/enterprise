package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.HexAt;
import dev.b28.enterprise.wire.NameCount;
import dev.b28.enterprise.wire.ProjectLine;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.RouteBook;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Collectors;

/**
 * What the commands show of the mint, a town's territory, the war and the routes' books, from the core (docs/binding-plan.md
 * 2, concern 11): {@code /enterprise mint}, {@code territory}, {@code war}, {@code war tally} and {@code routes} in core
 * mode. Each is one question ({@code Query.Mint}, {@code Territory}, {@code War}, {@code RouteBooks}) and its answer set out
 * in lines; the studies that drive a server over RCON read the same lines, so the lines keep the words the scripts' regexes
 * look for (the Java rules' own words, with the core's numbers). The adapter keeps nothing and sums nothing: the core's
 * figures are shown as they come.
 *
 * <p>Money is in emeralds, as the core keeps it; where the mod counted spurs (tolls, traffic, the war's treasury) the core
 * has worked them out at the mint's rate and the line says spurs.
 */
public final class CommandsAdapter {
    private final CoreHost host;

    public CommandsAdapter(CoreHost host) {
        this.host = host;
    }

    private Answer ask(Query query) {
        return host.ask(List.of(query)).get(0);
    }

    private static double em(long millis) {
        return millis / 1000.0;
    }

    /**
     * The refusal a command that is still the Java rules' gives in core mode (docs/alpha-plan.md 2): not in the alpha yet,
     * and what to do instead.
     */
    public static String notYet(String command, String instead) {
        return "/enterprise %s: not in the alpha yet. %s".formatted(command, instead);
    }

    /** The refusal of a command core mode has dropped for good (docs/decisions.md, the lost list), and where its figures went. */
    public static String gone(String command, String instead) {
        return "/enterprise %s: gone. %s".formatted(command, instead);
    }

    /** The mint: what an emerald fetches, and what moves it. The first words are the Java command's, for the scripts. */
    public List<String> mint() {
        if (!(ask(new Query.Mint()) instanceof Answer.Mint m)) {
            throw new IllegalStateException("the core answered Mint with something else");
        }
        return List.of(("The core's mint: %.2f spurs per emerald, selling pressure %.0f emeralds; %.0f spurs minted without "
                + "backing (the spur debased %.3fx)").formatted(m.rate(), m.pressure(), m.minted(), m.debased()));
    }

    private static String hexes(List<HexAt> hexes) {
        return hexes.stream().map(h -> "(%d, %d)".formatted(h.q(), h.r())).collect(Collectors.joining(", ", "[", "]"));
    }

    private static String percent(ProjectLine p) {
        return p.done() ? "built" : "%.0f%% built".formatted(p.progress() * 100);
    }

    /**
     * A town's territory: the lines the Java command prints and the studies parse (the settlement's hexes and culture, its
     * farm plan, treasury, mood, people, buildings, tolls and posts), from the core.
     */
    public List<String> territory(long town) {
        if (!(ask(new Query.Territory(town)) instanceof Answer.Territory t)) {
            throw new IllegalStateException("the core answered Territory with something else");
        }
        List<String> out = new ArrayList<>();
        out.add("Settlement at %s holds %d hex(es), culture %.1f of %.1f for the next".formatted(t.name(), t.hexes().size(),
                t.culture(), t.nextHex()));
        out.add("  hexes " + hexes(t.hexes()));
        t.farms().forEach(f -> out.add("  farms %s: %.1f a day".formatted(f.name(), f.count())));
        t.idle().forEach(f -> out.add("  idle %s: %.1f a day untended".formatted(f.name(), f.count())));
        t.soldForward().forEach(f -> out.add("  sold forward %s: %.1f a day at %.3f".formatted(f.good(), f.perDay(), f.price())));
        if (t.wagePerDay() > 0) {
            out.add(("  farm wages %.2f emeralds a day (%.0f%% of the harvest's value; crops sell %.2f a day); %.1f paid, "
                    + "%.1f unpaid (harvest lost); %d items harvested").formatted(t.wagePerDay(), t.laborShare() * 100,
                    t.cropSalesPerDay(), em(t.wagesPaid()), em(t.wagesUnpaid()), (long) t.harvested()));
        }
        if (t.deposits() > 0.01) {
            out.add("  deposits: %.1f emeralds borrowed of its households' savings".formatted(t.deposits()));
        }
        out.add("  focus: " + t.focus());
        out.add("  households: purse %.1f emeralds, owed %.1f in credit, %d freehold plots; hands %s (wage x%.2f)".formatted(
                em(t.purse()), em(t.credit()), t.freeholds(),
                t.tightness() > 1.01 ? "scarce" : t.tightness() < 0.99 ? "idle" : "matched", t.tightness()));
        out.add("  treasury %.1f emeralds at the market's last touch, %.1f kept back".formatted(em(t.treasury()), (double) t.reserve()));
        String met = t.comforts().stream().filter(c -> c.count() > 0).map(c -> "%s %.0f%%".formatted(c.name(), c.count() * 100))
                .collect(Collectors.joining(", "));
        out.add(("  mood %+.2f (%s): %d villagers, %d beds; comforts met: %s; farms x%.2f, culture x%.2f").formatted(t.mood(),
                t.mood() >= 0.5 ? "content" : t.mood() > -0.5 ? "so-so" : "unhappy", (long) t.villagers(), (long) t.beds(),
                met.isEmpty() ? "none yet" : met, t.yieldFactor(), t.cultureFactor()));
        t.bridges().forEach(b -> out.add("  bridge on %s: %s, %.0f emeralds of masons' wages paid".formatted(
                b.what().replaceFirst("^bridge on ", ""), b.done() ? "open" : percent(b), em(b.wages()))));
        t.wall().ifPresent(w -> out.add("  wall %.0f%% built, %.0f emeralds of masons' wages paid".formatted(w.progress() * 100,
                em(w.wages()))));
        if (t.militia() > 0) {
            out.add("  hands without a trade: %d standing watch".formatted(t.militia()));
        }
        if (!t.buildings().isEmpty() || t.hunting() > 0 || t.armed() > 0) {
            out.add("  buildings: %s; hunters with bows %.0f%%, guards armed %.0f%%".formatted(t.buildings().isEmpty() ? "none"
                    : t.buildings().stream().map(b -> b.what() + (b.done() ? "" : " (" + percent(b) + ")"))
                            .collect(Collectors.joining(", ")), t.hunting() * 100, t.armed() * 100));
        }
        out.add("  %d home(s) built, %d child(ren) born".formatted(t.homesBuilt(), t.born()));
        out.add(("  toll %.1f%% at its posts and a handling fee of %.0f spurs a caravan (%.0f spurs collected last review), "
                + "%d guards").formatted(t.tollRate() * 100, t.passageFee(), t.tollIncome(), t.guards()));
        out.add("  %d guard(s) posted at %s; traffic %.0f spurs a day; %d deserted".formatted(t.guards(), hexes(t.posts()),
                t.traffic(), t.deserted()));
        out.add("  locker trades: %d".formatted(t.trades()));
        if (t.tollsOwed() > 0) {
            out.add("  tolls owed: %.0f spurs".formatted(em(t.tollsOwed()) * t.rate()));
        }
        return out;
    }

    private static String names(List<NameCount> counts) {
        return counts.stream().map(c -> "%s=%.0f".formatted(c.name().replaceFirst(".*:", ""), c.count()))
                .collect(Collectors.joining(" "));
    }

    private Answer.War war0() {
        if (!(ask(new Query.War()) instanceof Answer.War w)) {
            throw new IllegalStateException("the core answered War with something else");
        }
        return w;
    }

    /** The war effort in a line (the Java command's words, for the studies): the last line the command prints. */
    public String war() {
        Answer.War w = war0();
        return ("war: %s %.0f days | treasury %.0f spurs, minted %.0f | contracts %d, delivered %.0f | depot %s | "
                + "army %d serving, %d raised, %d fallen | unmet %s | bills %.1f | rate %.2f, debasement %.3f").formatted(
                w.phase(), w.days(), w.treasurySpurs(), w.minted(), w.contracts(), w.delivered(),
                w.depot().isEmpty() ? "?" : names(w.depot()), w.serving(), w.raised(), w.fallen(),
                w.unmet().stream().map(g -> g.replaceFirst(".*:", "")).toList(), w.bills(), w.rate(), w.debasement());
    }

    /** What went into the war: by town, and in all. */
    public List<String> warTally() {
        Answer.War w = war0();
        List<String> out = new ArrayList<>();
        out.add("War tally, against %s".formatted(w.theatre()));
        w.towns().forEach(t -> out.add(("  %s: %d raised, %d fallen, %d deserted, %d soldier-days (%d hungry), strength %.0f; "
                + "%.0f pieces of kit delivered; %.1f emeralds earned; trust %.2f").formatted(t.name(), t.raised(), t.fallen(),
                t.deserted(), t.days(), t.hungry(), t.strength(), t.kit(), em(t.earned()), t.trust())));
        out.add("  kit issued: " + names(w.issued()));
        out.add("  food eaten: %.0f points, its meals %.0f%% hearty".formatted(w.eaten(), w.heartiness() * 100));
        out.add("  levies drafted %d, mercenaries hired %d, requisitions' contracts %d".formatted(w.levies(), w.hired(), w.awarded()));
        out.add("  minted %.0f spurs; rate now %.2f spurs an emerald".formatted(w.minted(), w.rate()));
        return out;
    }

    private static String desk(java.util.Optional<dev.b28.enterprise.wire.Point> p, long town) {
        return p.map(d -> "%d, %d, %d".formatted(d.x(), d.y(), d.z())).orElse("town " + town);
    }

    /**
     * Every route with its totals and where its wagon is: the Java command's words for the totals (the studies parse them,
     * with each end named by its desk's place), then the core's own account of the wagon.
     */
    public List<String> routes() {
        if (!(ask(new Query.RouteBooks()) instanceof Answer.RouteBooks books)) {
            throw new IllegalStateException("the core answered RouteBooks with something else");
        }
        var lines = host.wagons().routes();
        List<String> out = new ArrayList<>();
        out.add("%d route(s), the core's".formatted(books.routes().size()));
        for (RouteBook r : books.routes()) {
            String wagon = lines.stream().filter(l -> l.route() == r.route()).findFirst()
                    .map(l -> " | " + WagonAdapter.describe(l)).orElse("");
            out.add(("  #%d %s -> %s: %d blocks, %d trips, %.0f items sold for %.0f spurs, %.0f for services, %.0f bought for "
                    + "%.0f, %.0f spurs on road food, %.0f tolls paid, %.0f spurs brought home, %.0f food eaten%s").formatted(
                    r.route(), desk(r.fromDesk(), r.from()), desk(r.toDesk(), r.to()), r.blocks(), r.trips(), r.soldItems(),
                    r.salesSpurs(), r.serviceSpurs(), r.boughtItems(), r.purchaseSpurs(), r.foodSpurs(), r.tollsSpurs(),
                    r.homeSpurs(), r.foodEaten(), wagon));
        }
        return out;
    }

    /**
     * Switches on, for every locker of a town the core keeps (its hands' offers) that has nothing switched on yet, the
     * offers that earn emeralds (the town buys the good), or every offer: the Java command's work, through the core's
     * switches. Returns how many it switched on.
     */
    public int autoswitch(long town, boolean all) {
        int switched = 0;
        for (var locker : host.lockers().town(town).lockers()) {
            if (locker.offers().stream().anyMatch(dev.b28.enterprise.wire.OfferView::on)) {
                continue;
            }
            for (var offer : locker.offers()) {
                if (!offer.on() && (all || !offer.sells()) && host.lockers().flip(LockerAdapter.key(locker.hand(), offer), true)) {
                    switched++;
                }
            }
        }
        return switched;
    }

    private String name(long town) {
        return ask(new Query.Territory(town)) instanceof Answer.Territory t ? t.name() : "town " + town;
    }

    private List<dev.b28.enterprise.wire.TownPoint> points() {
        if (!(ask(new Query.Points()) instanceof Answer.Points p)) {
            throw new IllegalStateException("the core answered Points with something else");
        }
        return p.points();
    }

    /**
     * The core's town whose desk is nearest a place (a bell, or the desk itself) within {@code radius} blocks across, in
     * the same dimension, or null: how a command that names a bell finds its town.
     */
    public Long townNear(net.minecraft.core.GlobalPos at, int radius) {
        Long best = null;
        double nearest = Double.MAX_VALUE;
        for (var e : host.points().of(PointsAdapter.Kind.DESK).entrySet()) {
            if (!e.getKey().dimension().equals(at.dimension())) {
                continue;
            }
            double d = Math.sqrt(e.getKey().pos().distSqr(at.pos()));
            if (d <= radius && d < nearest) {
                best = e.getValue();
                nearest = d;
            }
        }
        return best;
    }

    /** The markets a town knows, nearest first, from what its Merchant's screen shows of them. */
    public List<String> atlas(long town) {
        if (!(ask(new Query.Routes(town)) instanceof Answer.Routes r)) {
            throw new IllegalStateException("the core answered Routes with something else");
        }
        List<String> out = new ArrayList<>();
        out.add("The desk of %s knows %d market(s)".formatted(name(town), r.markets().size()));
        r.markets().forEach(m -> out.add("  %s, %s".formatted(name(m.town()),
                m.blocks().map(b -> b + " blocks").orElse("how far unknown"))));
        return out;
    }

    /** The core's towns by the name it has for them, at their desks. */
    public List<String> names() {
        List<String> out = new ArrayList<>();
        out.add("Markets:");
        points().stream().filter(p -> p.point().kind() == dev.b28.enterprise.wire.PointKind.Desk)
                .forEach(p -> out.add("  %s at %d, %d, %d".formatted(name(p.town()), p.point().x(), p.point().y(), p.point().z())));
        return out;
    }

    /** The depots the core keeps, each a trade point of a town (the buildings themselves are the world's). */
    public List<String> depots() {
        var depots = points().stream().filter(p -> p.point().kind() == dev.b28.enterprise.wire.PointKind.Depot).toList();
        List<String> out = new ArrayList<>();
        out.add("%d building(s)".formatted(depots.size()));
        depots.forEach(p -> out.add("  depot of %s at %d, %d, %d".formatted(name(p.town()), p.point().x(), p.point().y(),
                p.point().z())));
        return out;
    }
}
