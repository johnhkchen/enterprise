package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Query;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;

/**
 * What the commands show of a town's books, from the core (docs/binding-plan.md 2, concern 11): {@code /enterprise
 * cashbook}, {@code books}, {@code whys}, {@code trust} and {@code contracts} in core mode, and {@code /enterprise core
 * ...} of the same in either mode. Each is one question ({@code Query.CashBook}, {@code Books}, {@code Whys}, {@code
 * Trust}, {@code Contracts}) and its answer set out in lines; the studies that drive a server over RCON read the same
 * lines. The adapter keeps nothing and does the sums of no book: the core's are shown as they come.
 *
 * <p>Money here is in emeralds, as the core keeps it, where the Java books count spurs.
 */
public final class BooksAdapter {
    private final CoreHost host;

    public BooksAdapter(CoreHost host) {
        this.host = host;
    }

    private static String money(long millis) {
        return "%.1f".formatted(millis / 1000.0);
    }

    private Answer ask(Query query) {
        return host.ask(List.of(query)).get(0);
    }

    private static void whys(List<String> out, List<dev.b28.enterprise.wire.WhyView> whys, String prefix) {
        whys.forEach(w -> out.add("  %s%s: %d times, %.0f items, last day %d".formatted(prefix, w.line(), w.times(), w.items(), w.last())));
    }

    /** A town's cash book for the last days: totals by what for, then what the Accountant held back. */
    public List<String> cashBook(long town, int days) {
        if (!(ask(new Query.CashBook(town, days)) instanceof Answer.CashBook a)) {
            throw new IllegalStateException("the core answered CashBook with something else");
        }
        List<String> lines = new ArrayList<>();
        lines.add("Cash book of %s:".formatted(a.name()));
        lines.add("days %d to %d: %s in, %s out".formatted(Math.max(0, a.today() - days + 1), a.today(), money(a.received()),
                money(a.paid())));
        a.totals().forEach(t -> lines.add("  %s%s %s".formatted(t.amount() >= 0 ? "+" : "-", money(Math.abs(t.amount())), t.what())));
        a.heldTotals().forEach(t -> lines.add("  held back by the accountant: %s for %s".formatted(money(t.amount()), t.what())));
        return lines;
    }

    /** A market's books summed by kind, counterparty and good, then its reasons. */
    public List<String> books(long town) {
        if (!(ask(new Query.Books(town)) instanceof Answer.Books a)) {
            throw new IllegalStateException("the core answered Books with something else");
        }
        List<String> lines = new ArrayList<>();
        lines.add("Books of %s (kept %d days): %d lines".formatted(a.name(), a.keeps(), a.lines().size()));
        a.lines().forEach(l -> lines.add("  %s | %s | %s: %.0f for %s emeralds".formatted(l.kind(), l.with(), l.good(), l.count(),
                money(l.money()))));
        lines.add("Reasons (%d):".formatted(a.whys().size()));
        whys(lines, a.whys(), "why | ");
        return lines;
    }

    /** A market's reasons alone, most often first. */
    public List<String> whys(long town) {
        if (!(ask(new Query.Whys(town)) instanceof Answer.Whys a)) {
            throw new IllegalStateException("the core answered Whys with something else");
        }
        List<String> lines = new ArrayList<>();
        lines.add("Reasons of %s (%d):".formatted(a.name(), a.whys().size()));
        whys(lines, a.whys(), "");
        return lines;
    }

    /** Each pair of towns that has dealt, and the trust between them. */
    public List<String> trust() {
        if (!(ask(new Query.Trust()) instanceof Answer.Trust a)) {
            throw new IllegalStateException("the core answered Trust with something else");
        }
        List<String> lines = new ArrayList<>();
        lines.add("%d record(s)".formatted(a.records().size()));
        a.records().forEach(r -> lines.add("  %s: trust %.2f, %.1f kept, %.1f missed".formatted(r.parties(), r.trust(), r.kept(),
                r.missed())));
        return lines;
    }

    /** Every contract, or those a town is a party to. */
    public List<String> contracts(Long town) {
        if (!(ask(new Query.Contracts(Optional.ofNullable(town))) instanceof Answer.Contracts a)) {
            throw new IllegalStateException("the core answered Contracts with something else");
        }
        List<String> lines = new ArrayList<>();
        lines.add("%d contract(s)".formatted(a.contracts().size()));
        a.contracts().forEach(c -> {
            lines.add(("  #%d %s -> %s, %s, %s (copies at %d of 2): %s; %.0f delivered, %.0f on the road, "
                    + "%s emeralds paid (%s up front), %d missed by the seller, %d by the buyer; trust %.2f").formatted(c.id(),
                    c.sellerName(), c.buyerName(), c.state(), c.incoterm(), c.copies(), c.what(), c.delivered(), c.shipped(),
                    money(c.paid()), money(c.advanced()), c.missed(), c.buyerMissed(), c.trust()));
            c.history().forEach(h -> lines.add("    " + h));
        });
        lines.add("%d rent(s)".formatted(a.rents().size()));
        a.rents().forEach(r -> {
            lines.add(("  rent %d: %s owes %s %.0f %s a season, bought for %s emeralds; %s, %d coupons, %.0f delivered, "
                    + "%.0f in arrears").formatted(r.id(), r.farmName(), r.holderName(), r.quantity(),
                    r.crop().replaceFirst(".*:", ""), money(r.capital()), r.state(), r.coupons(), r.delivered(), r.arrears()));
            r.history().forEach(h -> lines.add("    " + h));
        });
        return lines;
    }
}
