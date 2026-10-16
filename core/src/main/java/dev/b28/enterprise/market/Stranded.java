package dev.b28.enterprise.market;

import dev.b28.enterprise.Dials;
import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.cargo.Cargo;
import dev.b28.enterprise.cargo.Crates;
import dev.b28.enterprise.cargo.Papers;
import dev.b28.enterprise.contract.Bills;
import dev.b28.enterprise.contract.ContractBook;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.item.ItemStack;

/**
 * Stranded paper: bills a town holds on a market whose caravans no longer call (its run wound down, say), and the
 * goods it crated for that market, paid for by those bills, that no one comes to collect. Sound on paper, worth
 * nothing where it lies: a town whose wealth is all such paper has no money. A freight hub that runs to the drawee buys
 * it, as a merchant bank would: each bill at its face less a fee (more, past its day), less the forwarding of its
 * goods; paid in coin as far as the hub trusts the drawee and can spare, and the rest in the hub's own bill, which the
 * town can present where its caravans go. The hub delivers the goods and presents the bills itself.
 */
public final class Stranded {
    /** The hub pays what it can't in coin in at most this many bills, a bill term apart. */
    public static final int INSTALLMENTS = 6;

    private Stranded() {}

    /** Can the hub reach this market: a run of its own there, or it's the hub? */
    public static boolean reaches(MinecraftServer server, GlobalPos hub, GlobalPos market) {
        return market.equals(hub) || !Freight.legs(server, hub, market).isEmpty();
    }

    /** Does a caravan of {@code from}'s still call at {@code to}? */
    public static boolean calls(MinecraftServer server, GlobalPos from, GlobalPos to) {
        return CaravanLedger.get(server).route(from, to).filter(r -> !r.stopping()).isPresent();
    }

    /** A bill the town holds that the hub would buy: one it can carry to its drawee, whose own caravans don't call. */
    public static boolean bill(MinecraftServer server, GlobalPos hub, GlobalPos town, Bills.Bill bill, long now) {
        // Not a bill on the hub itself: that one the town presents there, for its face.
        return bill.state() == Bills.State.OUTSTANDING && bill.payee().equals(town) && !bill.drawee().equals(town)
                && !bill.drawee().equals(hub) && reaches(server, hub, bill.drawee())
                && (!calls(server, bill.drawee(), town) || bill.due() < now);
    }

    /** A crate the town shipped, paid for by a bill, that its consignee doesn't come for. */
    public static boolean crate(MinecraftServer server, GlobalPos hub, GlobalPos town, ItemStack stack) {
        if (!Crates.isCrate(stack)) {
            return false;
        }
        Papers papers = Crates.papers(stack);
        Optional<GlobalPos> to = papers.consignee();
        return papers.settled() && papers.shipper().filter(town::equals).isPresent() && to.isPresent()
                && !to.get().equals(town) && reaches(server, hub, to.get()) && !calls(server, to.get(), town);
    }

    /** What the hub pays: coin now (spurs), its own bills for the rest (in installments), and the paper it bought. */
    public record Deal(int spurs, List<Papers.Document> hubBills, List<ItemStack> crates, List<ItemStack> bills,
            double face) {
        public Optional<Papers.Document> hubBill() {
            return hubBills.stream().findFirst();
        }
    }

    /**
     * The hub buys the stranded paper among {@code crates} and {@code bundles} (bills; the rest is left in them): the
     * bills whose goods it can forward, and the crates that go with them (consigned to their drawees). Pays coin up to
     * {@code coin} emeralds; the rest in its bill on itself, due in a forwarder's term. Returns nothing if nothing was
     * worth buying. It promises no more in its own bills than its treasury ({@code means}, emeralds) could pay:
     * installments every bill term, each at most half of it; beyond that it doesn't buy.
     */
    public static Optional<Deal> buy(MinecraftServer server, GlobalPos hub, GlobalPos town, List<ItemStack> crates,
            List<ItemStack> bundles, double coin, double rate, long now) {
        return buy(server, hub, town, crates, bundles, coin, Double.MAX_VALUE, rate, now);
    }

    /** As above, its own bills within {@code means}. */
    public static Optional<Deal> buy(MinecraftServer server, GlobalPos hub, GlobalPos town, List<ItemStack> crates,
            List<ItemStack> bundles, double coin, double means, double rate, long now) {
        Bills ledger = Bills.get(server);
        ContractBook book = ContractBook.get(server);
        Map<GlobalPos, List<Papers.Document>> bought = new LinkedHashMap<>();
        double face = 0;
        double price = 0;
        double least = 1; // the trust the hub has in the least trusted drawee bought
        for (ItemStack bundle : bundles) {
            List<Papers.Document> kept = new ArrayList<>();
            for (Papers.Document doc : bundle.getOrDefault(Cargo.PAPERS.get(), Papers.NONE).documents()) {
                Optional<Bills.Bill> bill = doc.kind().equals(Papers.BILL_OF_EXCHANGE)
                        ? ledger.bill(doc.data().getInt("bill")) : Optional.empty();
                if (bill.isEmpty() || !bill(server, hub, town, bill.get(), now)) {
                    kept.add(doc);
                    continue;
                }
                double days = Math.max(0, (bill.get().due() - now) / 24000.0);
                double off = Freight.DISCOUNT_FEE + (bill.get().due() < now ? Dials.STALE_DISCOUNT.get()
                        : Freight.DISCOUNT_PER_DAY * days);
                double paying = bill.get().emeralds() * Math.max(0, 1 - off);
                if (price + paying - Math.max(0, coin) > INSTALLMENTS * means / 2) {
                    kept.add(doc); // more than it could pay in its own bills: it leaves the rest
                    continue;
                }
                face += bill.get().emeralds();
                price += bill.get().emeralds() * Math.max(0, 1 - off);
                least = Math.min(least, book.trust(hub, bill.get().drawee(), now));
                bought.computeIfAbsent(bill.get().drawee(), d -> new ArrayList<>()).add(doc);
            }
            bundle.set(Cargo.PAPERS.get(), new Papers(kept));
        }
        if (bought.isEmpty()) {
            return Optional.empty();
        }
        // The crates paid for by those bills go with them; their forwarding comes off the price.
        Set<GlobalPos> drawees = bought.keySet();
        List<ItemStack> goods = new ArrayList<>();
        for (var it = crates.iterator(); it.hasNext(); ) {
            ItemStack crate = it.next();
            Optional<GlobalPos> to = Crates.papers(crate).consignee();
            if (crate(server, hub, town, crate) && to.filter(drawees::contains).isPresent()) {
                double forwarding = to.get().equals(hub) ? 0 : Freight.legs(server, hub, to.get()).stream()
                        .mapToDouble(Freight::forwardingPrice).min().orElse(0);
                price -= forwarding * Crates.stacks(List.of(crate));
                goods.add(crate);
                it.remove();
            }
        }
        price = Math.max(0, price);
        int spurs = (int) Math.floor(Math.min(Math.max(0, coin), price * least) * rate);
        double rest = price - spurs / rate;
        // The rest in installments, a bill term apart, each no more than half what it could pay.
        List<Papers.Document> hubBills = new ArrayList<>();
        int pieces = rest <= 0.01 ? 0 : (int) Math.min(INSTALLMENTS, Math.max(1,
                Math.ceil(rest / Math.max(1, Math.min(rest, means / 2)))));
        for (int i = 0; i < pieces; i++) {
            hubBills.add(ledger.draw(hub, town, rest / pieces / (1 + Bills.INTEREST), now, -1,
                    Bills.TERM_TICKS * (i + 1)));
        }
        List<ItemStack> billBundles = new ArrayList<>();
        bought.forEach((drawee, docs) -> {
            for (Papers.Document doc : docs) {
                ledger.bill(doc.data().getInt("bill")).ifPresent(b -> ledger.endorse(b, hub));
            }
            billBundles.add(Crates.documents(new Papers(docs)));
        });
        Freight freight = Freight.get(server);
        List<ItemStack> forwarded = goods.stream().map(freight::forwarding).toList();
        return Optional.of(new Deal(spurs, hubBills, forwarded, billBundles, face));
    }
}
