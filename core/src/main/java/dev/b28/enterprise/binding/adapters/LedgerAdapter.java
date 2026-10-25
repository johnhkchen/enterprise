package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.merchant.LedgerView;
import dev.b28.enterprise.merchant.LedgerView.Line;
import dev.b28.enterprise.merchant.LedgerView.Section;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Wire;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;

/**
 * A town's books in short lines (docs/alpha-plan.md 3): the Ledger tab of the Merchant's screen and {@code /enterprise ledger
 * <desk>}. One question of existing variants ({@code Cash}, {@code CashBook}, {@code Mint}, {@code Territory}, {@code Stock},
 * {@code Chests}, {@code Hands}, {@code Lockers}, {@code Routes}, {@code Whys}, {@code Contracts}, {@code Pallets}, {@code
 * War}), then a second for what the first showed (a {@code Manifest} for each wagon out, the {@code Territory} of the other
 * ends for their names). The adapter keeps nothing and sums nothing but the day's lines by day: the core's figures are set out
 * as they come, money in emeralds as the core keeps it, a negative smaller than 1e-9 shown as 0. Asked once, when the screen
 * opens or the command runs, never per tick. No wire query is new.
 */
public final class LedgerAdapter {
    /** The days of the cash book shown. */
    public static final int DAYS = 7;
    /** A section's lines: the Merchant screen's packet is small (about 32 KB), so a long list ends in "and N more". */
    static final int MAX_LINES = 30;
    private static final int MAX_TEXT = 110;

    private final CoreHost host;

    public LedgerAdapter(CoreHost host) {
        this.host = host;
    }

    /** A number as the ledger writes it: a whole number plain, else one decimal; a figure within 1e-9 of zero is 0. */
    public static String fmt(double v) {
        if (Math.abs(v) < 1e-9) {
            return "0";
        }
        if (Math.abs(v - Math.rint(v)) < 1e-9) {
            return Long.toString(Math.round(v));
        }
        return String.format(Locale.ROOT, "%.1f", v);
    }

    /** Money the core keeps in thousandths of an emerald, as emeralds. */
    public static String em(long millis) {
        return fmt(millis / 1000.0);
    }

    private static String signed(long millis) {
        return (millis < 0 ? "-" : "+") + em(Math.abs(millis));
    }

    private static String bare(String name) {
        return name.replaceFirst("^.*:", "");
    }

    private String good(long number) {
        var item = host.stock().item(number);
        return item == net.minecraft.world.item.Items.AIR ? "good " + number : BuiltInRegistries.ITEM.getKey(item).getPath();
    }

    private static <T extends Answer> T as(Answer a, Class<T> type) {
        if (!type.isInstance(a)) {
            throw new IllegalStateException("the core answered " + type.getSimpleName() + " with something else");
        }
        return type.cast(a);
    }

    /** The ledger of the town whose desk stands at {@code desk}: only a word if the core keeps no town there. */
    public LedgerView view(GlobalPos desk) {
        Long town = host.stock().townAt(desk);
        if (town == null) {
            return LedgerView.note("No town here: the core keeps none at " + desk.pos().toShortString());
        }
        List<Answer> a = host.ask(List.of(
                new Query.Cash(StockAdapter.point(desk)), new Query.CashBook(town, DAYS), new Query.Mint(),
                new Query.Territory(town), new Query.Stock(town), new Query.Chests(StockAdapter.point(desk)),
                new Query.Hands(town), new Query.Lockers(town), new Query.Routes(town), new Query.Whys(town),
                new Query.Contracts(Optional.of(town)), new Query.Pallets(town), new Query.War()));
        var cash = as(a.get(0), Answer.Cash.class);
        var book = as(a.get(1), Answer.CashBook.class);
        var mint = as(a.get(2), Answer.Mint.class);
        var land = as(a.get(3), Answer.Territory.class);
        var stock = as(a.get(4), Answer.Stock.class);
        var chests = as(a.get(5), Answer.Chests.class);
        var hands = as(a.get(6), Answer.Hands.class);
        var lockers = as(a.get(7), Answer.Lockers.class);
        var routes = as(a.get(8), Answer.Routes.class);
        var whys = as(a.get(9), Answer.Whys.class);
        var contracts = as(a.get(10), Answer.Contracts.class);
        var pallets = as(a.get(11), Answer.Pallets.class);
        var war = as(a.get(12), Answer.War.class);

        // What the first answers showed: the other ends' names, and the manifest of each wagon out.
        Set<Long> others = new LinkedHashSet<>();
        List<Long> out = new ArrayList<>();
        for (var r : routes.routes()) {
            others.add(r.from());
            others.add(r.to());
            if (r.wagon().isPresent() && !r.wagon().get().porter()) {
                out.add(r.route());
            }
        }
        others.remove(town);
        List<Query> more = new ArrayList<>();
        others.forEach(t -> more.add(new Query.Territory(t)));
        out.forEach(r -> more.add(new Query.Manifest(r)));
        Map<Long, String> names = new HashMap<>();
        names.put(town, land.name());
        Map<Long, Answer.Manifest> manifests = new HashMap<>();
        if (!more.isEmpty()) {
            List<Answer> b = host.ask(more);
            int i = 0;
            for (Long t : others) {
                names.put(t, ((Answer.Territory) b.get(i++)).name());
            }
            for (Long r : out) {
                manifests.put(r, (Answer.Manifest) b.get(i++));
            }
        }

        String heading = "%s · day %d · mood %s · %s people".formatted(land.name(), book.today(),
                String.format(Locale.ROOT, "%.2f", land.mood()).replace("-0.00", "0.00"), fmt(land.villagers()));
        String till = till(cash);
        List<Section> sections = List.of(
                new Section("Money", clip(money(cash, book))),
                new Section("Goods", clip(goods(stock, chests))),
                new Section("People", clip(people(hands, lockers))),
                new Section("Wagons", clip(wagons(town, routes, whys, names, manifests))),
                new Section("Papers", clip(papers(contracts, pallets, names))),
                new Section("Mint", clip(mint(mint, war))));
        return new LedgerView(heading, till, sections, footer());
    }

    private static String till(Answer.Cash c) {
        return "Treasury %s emeralds · reserve %d · chests hold %d emeralds, %d spurs · %s".formatted(em(c.treasury()), c.reserve(),
                c.emeralds(), c.coins(), c.bank().isPresent() ? "Banker " + c.bank().get() + " spurs" : "no Blaze Banker");
    }

    private static String footer() {
        String version = net.neoforged.fml.ModList.get().getModContainerById(dev.b28.enterprise.Enterprise.MOD_ID)
                .map(c -> c.getModInfo().getVersion().toString()).orElse("?");
        return "asked at open: reopen to refresh · enterprise %s · schema %016x".formatted(version, Wire.SCHEMA);
    }

    private static List<Line> clip(List<Line> lines) {
        List<Line> out = new ArrayList<>();
        for (Line l : lines) {
            if (out.size() == MAX_LINES - 1 && lines.size() > MAX_LINES) {
                out.add(new Line("… and %d more".formatted(lines.size() - out.size()), l.source()));
                break;
            }
            out.add(l.text().length() > MAX_TEXT ? new Line(l.text().substring(0, MAX_TEXT - 1) + "…", l.source()) : l);
        }
        return out;
    }

    private static List<Line> money(Answer.Cash c, Answer.CashBook b) {
        List<Line> out = new ArrayList<>();
        out.add(new Line("Treasury %s emeralds".formatted(em(c.treasury())), "Cash"));
        out.add(new Line("Reserve %d emeralds kept back".formatted(c.reserve()), "Cash"));
        out.add(new Line("Till: chests hold %d emeralds and %d spurs of coin (worth %s emeralds)".formatted(c.emeralds(), c.coins(),
                em(c.coinWorth())), "Cash"));
        out.add(new Line(c.bank().isPresent() ? "Banker: %d spurs (worth %s emeralds)".formatted(c.bank().get(), em(c.bankWorth()))
                : "No Blaze Banker", "Cash"));
        out.add(new Line("Last %d days (day %d to %d): %s in, %s out".formatted(DAYS, Math.max(0, b.today() - DAYS + 1), b.today(),
                em(b.received()), em(b.paid())), "CashBook"));
        b.totals().forEach(t -> out.add(new Line("  %s %s".formatted(signed(t.amount()), t.what()), "CashBook")));
        b.heldTotals().forEach(t -> out.add(new Line("  held back by the accountant: %s for %s".formatted(em(t.amount()), t.what()),
                "CashBook")));
        // By day, newest first: the lines come oldest first, a day's together.
        List<Long> days = b.lines().stream().map(dev.b28.enterprise.wire.BookLine::day).distinct().sorted(Comparator.reverseOrder())
                .toList();
        for (long day : days) {
            StringBuilder s = new StringBuilder("day " + day + ": ");
            boolean first = true;
            for (var line : b.lines()) {
                if (line.day() == day) {
                    s.append(first ? "" : ", ").append(signed(line.amount())).append(' ').append(line.what());
                    first = false;
                }
            }
            out.add(new Line(s.toString(), "CashBook"));
        }
        return out;
    }

    private List<Line> goods(Answer.Stock s, Answer.Chests c) {
        List<Line> out = new ArrayList<>();
        List<Integer> held = new ArrayList<>();
        for (int g = 0; g < s.stock().size(); g++) {
            if (Math.abs(s.stock().get(g)) >= 1e-9) {
                held.add(g);
            }
        }
        held.sort(Comparator.comparingDouble((Integer g) -> s.stock().get(g)).reversed());
        out.add(new Line(held.isEmpty() ? "The town holds nothing" : "The town holds %d kinds of goods, largest first".formatted(held.size()),
                "Stock"));
        for (int g : held) {
            out.add(new Line("  %s %s".formatted(good(g), fmt(s.stock().get(g))), "Stock"));
        }
        List<String> short_ = new ArrayList<>();
        for (int g = 0; g < c.short_().size(); g++) {
            if (Math.abs(c.short_().get(g)) >= 1e-9) {
                short_.add("%s %s".formatted(good(g), fmt(c.short_().get(g))));
            }
        }
        if (!short_.isEmpty()) {
            out.add(new Line("Chests couldn't take (left unmoved): " + String.join(", ", short_), "Chests"));
        }
        out.add(new Line("Free slots in the chests: " + c.free(), "Chests"));
        return out;
    }

    private List<Line> people(Answer.Hands h, Answer.Lockers l) {
        List<Line> out = new ArrayList<>();
        out.add(new Line("%d hands; the Merchant may manage %d lockers".formatted(h.hands().size(), h.lockers()), "Hands"));
        Map<Long, String> trades = new HashMap<>();
        for (var hand : h.hands()) {
            trades.put(hand.hand(), bare(hand.trade()));
            out.add(new Line("  %s, level %d%s%s%s".formatted(bare(hand.trade()), hand.level(), hand.merchant() ? ", the desk's" : "",
                    hand.baby() ? ", a child" : "", hand.away() ? ", away" : ""), "Hands"));
        }
        long managed = l.lockers().stream().filter(x -> x.managed()).count();
        out.add(new Line("%d lockers managed".formatted(managed), "Lockers"));
        // What keeps the rest from being lockers: a chest touching a workstation makes one.
        List<String> none = host.lockers().noLockers(net.neoforged.neoforge.server.ServerLifecycleHooks.getCurrentServer(), h.town());
        if (!none.isEmpty()) {
            out.add(new Line("  Not lockers yet (a chest touching a workstation makes one):", "Lockers"));
            none.forEach(n -> out.add(new Line("    " + n, "Lockers")));
        }
        for (var locker : l.lockers()) {
            if (!locker.managed()) {
                continue;
            }
            List<String> on = new ArrayList<>();
            for (var o : locker.offers()) {
                if (o.on()) {
                    on.add("%s %s".formatted(o.sells() ? "sells" : "buys", good(o.good())));
                }
            }
            out.add(new Line("  %s: %s".formatted(trades.getOrDefault(locker.hand(), "hand " + locker.hand()),
                    on.isEmpty() ? "no offer switched on" : String.join(", ", on)), "Lockers"));
        }
        return out;
    }

    private static String name(Map<Long, String> names, long town) {
        return names.getOrDefault(town, "town " + town);
    }

    private List<Line> wagons(long town, Answer.Routes r, Answer.Whys whys, Map<Long, String> names,
            Map<Long, Answer.Manifest> manifests) {
        List<Line> out = new ArrayList<>();
        out.add(new Line("%d routes; %d markets known%s".formatted(r.routes().size(), r.markets().size(),
                r.squad().isEmpty() ? "" : "; at home to send: " + String.join(", ", r.squad().stream().map(LedgerAdapter::bare).toList())),
                "Routes"));
        for (var route : r.routes()) {
            out.add(new Line("  #%d %s → %s: %s%s, %d trips, %s sold".formatted(route.route(), name(names, route.from()),
                    name(names, route.to()), route.status(), route.open() ? "" : " (wound down)", route.trips(), fmt(route.sold())),
                    "Routes"));
            route.wagon().ifPresent(w -> {
                out.add(new Line("    wagon %s, there at tick %d, crew %d, %d stacks%s".formatted(w.out() ? "out" : "home", w.arrives(),
                        w.crew().size(), w.stacks(), w.porter() ? ", a porter" : ""), "Routes"));
                var m = manifests.get(route.route());
                if (m != null) {
                    m.lines().forEach(line -> out.add(new Line("    aboard: pallet #%d, %d items to %s".formatted(line.pallet().id(),
                            line.pallet().items(), line.drop().map(d -> name(names, d)).orElse("home")), "Manifest")));
                }
            });
        }
        whys.whys().stream().limit(5).forEach(w -> out.add(new Line("why: %s (%d times, last day %d)".formatted(w.line(), w.times(),
                w.last()), "Whys")));
        return out;
    }

    private static List<Line> papers(Answer.Contracts c, Answer.Pallets p, Map<Long, String> names) {
        List<Line> out = new ArrayList<>();
        out.add(new Line("%d contracts, %d rents".formatted(c.contracts().size(), c.rents().size()), "Contracts"));
        for (var k : c.contracts()) {
            out.add(new Line("  #%d %s → %s: %s, %s, %s; %s delivered, %s on the road, %s emeralds paid".formatted(k.id(), k.sellerName(),
                    k.buyerName(), k.state(), k.incoterm(), k.what(), fmt(k.delivered()), fmt(k.shipped()), em(k.paid())), "Contracts"));
        }
        for (var r : c.rents()) {
            out.add(new Line("  rent %d: %s owes %s %s %s a season; %s, %s in arrears".formatted(r.id(), r.farmName(), r.holderName(),
                    fmt(r.quantity()), bare(r.crop()), r.state(), fmt(r.arrears())), "Contracts"));
        }
        out.add(new Line("Pallets: %d held, %d inbound, %d received, %d lost".formatted(p.held().size(), p.inbound().size(),
                p.received(), p.lost()), "Pallets"));
        p.held().forEach(v -> out.add(new Line("  held #%d: %d items from %s to %s%s".formatted(v.id(), v.items(), v.from(), v.to(),
                v.sealed() ? ", sealed" : ""), "Pallets")));
        p.inbound().forEach(v -> out.add(new Line("  inbound #%d: %d items from %s".formatted(v.id(), v.items(), v.from()), "Pallets")));
        return out;
    }

    private static List<Line> mint(Answer.Mint m, Answer.War w) {
        return List.of(
                new Line("An emerald fetches %s spurs".formatted(String.format(Locale.ROOT, "%.2f", m.rate())), "Mint"),
                new Line("Selling pressure %s emeralds".formatted(fmt(m.pressure())), "Mint"),
                new Line("%s spurs minted without backing".formatted(fmt(m.minted())), "Mint"),
                new Line("Spur debased to %sx (1 is not at all)".formatted(String.format(Locale.ROOT, "%.3f", m.debased())), "Mint"),
                new Line("War: %s, %s days".formatted(w.phase(), fmt(w.days())), "War"));
    }
}
