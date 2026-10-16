package dev.b28.enterprise.contract;

import dev.b28.enterprise.cargo.Cargo;
import dev.b28.enterprise.cargo.Crates;
import dev.b28.enterprise.cargo.Papers;
import dev.b28.enterprise.merchant.MerchantStock;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Bills of exchange: a buyer short of coin pays with paper instead, an order on its own treasury to pay the bearer so
 * much on a day to come. The bill is a document: it lies in the payee's warehouse, filed with other bills on the same
 * drawee, and is paid only when presented there, by a caravan between the two (the drawee's, arriving where the
 * bills lie, pays them; the holder's carries them to the drawee and brings the coin home). Unpaid well past its day,
 * a bill is dishonoured, and trust between the two falls. The ledger is the record; the paper is the title.
 */
public final class Bills extends SavedData {
    private static final String NAME = "enterprise_bills";

    /** Bills fall due this long after they're drawn, and carry this much over the debt for the wait. */
    public static final long TERM_TICKS = 4 * 24000L;
    public static final double INTEREST = 0.02;
    /** Unpaid this long past its day, a bill is dishonoured. */
    public static final long DISHONOUR_TICKS = 4 * 24000L;
    /**
     * Never presented this long past its day, a bill lapses (dishonoured: no one could carry it to its drawee, or no
     * one would pay): written off, its paper thrown out, and trust between the two falls.
     */
    public static final long LAPSE_TICKS = 3 * DISHONOUR_TICKS;

    /** A bill paid when presented counts this much of a kept installment toward trust; a dishonoured one, a whole miss. */
    public static final double HONOURED_WEIGHT = 0.5;

    public enum State { OUTSTANDING, PAID, DISHONOURED }

    /** A bill: the drawee (who pays), the payee, the sum in emeralds, when drawn and when due. */
    public static final class Bill {
        final int id;
        final GlobalPos drawee;
        GlobalPos payee; // whoever holds it now: a bank that bought it, say
        final double emeralds;
        final long drawn;
        final long due;
        State state = State.OUTSTANDING;
        int contract = -1; // the contract it pays for, if any (goods crated for the drawee: their title)
        long presented = -1; // when it was first presented to its drawee

        Bill(int id, GlobalPos drawee, GlobalPos payee, double emeralds, long drawn, long due) {
            this.id = id;
            this.drawee = drawee;
            this.payee = payee;
            this.emeralds = emeralds;
            this.drawn = drawn;
            this.due = due;
        }

        public int id() {
            return id;
        }

        public GlobalPos drawee() {
            return drawee;
        }

        public GlobalPos payee() {
            return payee;
        }

        public double emeralds() {
            return emeralds;
        }

        public long due() {
            return due;
        }

        public State state() {
            return state;
        }

        public int contract() {
            return contract;
        }
    }

    private final Map<Integer, Bill> bills = new LinkedHashMap<>();
    private int nextId = 1;

    public static Bills get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Bills::new, Bills::load, null), NAME);
    }

    /** {@code drawee} draws a bill on itself to pay {@code payee} {@code debt} emeralds: the bill's paper. */
    public Papers.Document draw(GlobalPos drawee, GlobalPos payee, double debt, long now) {
        return draw(drawee, payee, debt, now, -1);
    }

    /** As {@link #draw(GlobalPos, GlobalPos, double, long)}, paying for goods under {@code contract}. */
    public Papers.Document draw(GlobalPos drawee, GlobalPos payee, double debt, long now, int contract) {
        return draw(drawee, payee, debt, now, contract, TERM_TICKS);
    }

    /** As above, due {@code term} ticks after it's drawn (a forwarder's thirty days, say), at the same interest. */
    public Papers.Document draw(GlobalPos drawee, GlobalPos payee, double debt, long now, int contract, long term) {
        Bill bill = new Bill(nextId++, drawee, payee, debt * (1 + INTEREST * Math.max(1, term / (double) TERM_TICKS)),
                now, now + term);
        bill.contract = contract;
        dev.b28.enterprise.cargo.Paperwork.drawn(payee, 1); // drawn by its payee
        bills.put(bill.id, bill);
        setDirty();
        CompoundTag tag = new CompoundTag();
        tag.putInt("bill", bill.id);
        Papers.put(tag, "drawee", drawee);
        Papers.put(tag, "payee", payee);
        tag.putDouble("emeralds", bill.emeralds);
        tag.putLong("due", bill.due);
        return new Papers.Document(Papers.BILL_OF_EXCHANGE, tag);
    }

    public Optional<Bill> bill(int id) {
        return Optional.ofNullable(bills.get(id));
    }

    public List<Bill> all() {
        return List.copyOf(bills.values());
    }

    /** Emeralds outstanding on bills drawn on {@code drawee}. */
    public double owedBy(GlobalPos drawee) {
        return bills.values().stream().filter(b -> b.state != State.PAID && b.drawee.equals(drawee))
                .mapToDouble(b -> b.emeralds).sum();
    }

    /** Emeralds a caravan from {@code drawee} should carry to pay the bills on it due by {@code at} held at {@code holder}. */
    public double dueAt(GlobalPos drawee, GlobalPos holder, long at) {
        return bills.values().stream().filter(b -> b.state != State.PAID && b.drawee.equals(drawee)
                && b.payee.equals(holder) && b.due <= at).mapToDouble(b -> b.emeralds).sum();
    }

    void paid(Bill bill) {
        bill.state = State.PAID;
        setDirty();
    }

    /** The bill is sold on (to a bank): it's paid to {@code holder} now. */
    public void endorse(Bill bill, GlobalPos holder) {
        bill.payee = holder;
        setDirty();
    }

    /** The drawee buys back its own bill early (a bank at the drawee's market discounted it): it's settled. */
    public void redeem(Bill bill) {
        bill.state = State.PAID;
        setDirty();
    }

    /**
     * A drawee's grace to pay a bill put to it: so many days (a dial), and a day more for every so many emeralds it
     * owes the same holder on bills outstanding (a capital owing a town a fortune needs longer to find it).
     */
    public long grace(Bill bill) {
        double per = dev.b28.enterprise.Dials.BILL_GRACE_PER.get();
        double owed = bills.values().stream().filter(b -> b.state == State.OUTSTANDING && b.drawee.equals(bill.drawee)
                && b.payee.equals(bill.payee)).mapToDouble(b -> b.emeralds).sum();
        return dev.b28.enterprise.Dials.BILL_GRACE.ticks() + (per > 0 ? Math.round(owed / per * 24000) : 0);
    }

    /**
     * Bills never presented {@link #LAPSE_TICKS} past their day, that no one can carry to their drawee (no road between
     * the two, and no freight hub the holder reaches that runs there to buy them), lapse: dishonoured, and trust
     * between the two falls. A bill someone could still carry stands.
     */
    public void lapse(MinecraftServer server, ContractBook book, long now) {
        var routes = dev.b28.enterprise.caravan.CaravanLedger.get(server);
        for (Bill bill : bills.values()) {
            if (bill.state == State.OUTSTANDING && now > bill.due + LAPSE_TICKS
                    && routes.route(bill.payee, bill.drawee).isEmpty()
                    && !dev.b28.enterprise.market.Apprentices.collectable(server, bill.payee, bill.drawee)) {
                bill.state = State.DISHONOURED;
                book.missed(bill.drawee, bill.payee, 1, now);
                setDirty();
            }
        }
    }

    /** Is there a bill still to be paid for goods under this contract? */
    public boolean outstandingFor(int contract) {
        return contract >= 0 && bills.values().stream()
                .anyMatch(b -> b.contract == contract && b.state == State.OUTSTANDING);
    }

    /**
     * Bills in a warehouse that are no longer this desk's to collect (paid, dishonoured, or sold on) are thrown out;
     * a bundle keeps only its live ones. Dead paper shouldn't take the room a harvest needs.
     */
    public void tidy(MerchantStock stock, GlobalPos desk) {
        List<ItemStack> bundles = stock.removeLoose(s -> s.is(Cargo.DOCUMENTS.get()) && !ids(s).isEmpty());
        for (ItemStack bundle : bundles) {
            Papers papers = bundle.getOrDefault(Cargo.PAPERS.get(), Papers.NONE);
            List<Papers.Document> live = papers.documents().stream().filter(d -> !d.kind().equals(Papers.BILL_OF_EXCHANGE)
                    || Optional.ofNullable(bills.get(d.data().getInt("bill")))
                            .filter(b -> b.state == State.OUTSTANDING && b.payee.equals(desk)).isPresent()).toList();
            if (!live.isEmpty()) {
                stock.insert(Crates.documents(new Papers(live)));
            }
        }
    }

    /** The bills a bundle of documents holds, as ledger entries. */
    public List<Bill> in(ItemStack bundle) {
        return ids(bundle).stream().map(bills::get).filter(java.util.Objects::nonNull).toList();
    }

    /** The bills a document holds. */
    public static List<Integer> ids(ItemStack documents) {
        return documents.getOrDefault(Cargo.PAPERS.get(), Papers.NONE).documents().stream()
                .filter(d -> d.kind().equals(Papers.BILL_OF_EXCHANGE)).map(d -> d.data().getInt("bill")).toList();
    }

    private static Optional<GlobalPos> drawee(ItemStack documents) {
        return documents.getOrDefault(Cargo.PAPERS.get(), Papers.NONE).first(Papers.BILL_OF_EXCHANGE)
                .flatMap(d -> Papers.pos(d.data(), "drawee"));
    }

    /** Files a bill with the others on the same drawee in the warehouse (one bundle per drawee). */
    public static void file(MerchantStock stock, Papers.Document bill) {
        Optional<GlobalPos> on = Papers.pos(bill.data(), "drawee");
        List<ItemStack> bundles = stock.removeLoose(s -> s.is(Cargo.DOCUMENTS.get()) && !ids(s).isEmpty()
                && drawee(s).equals(on));
        Papers papers = bundles.isEmpty() ? Papers.NONE : bundles.get(0).getOrDefault(Cargo.PAPERS.get(), Papers.NONE);
        bundles.stream().skip(1).forEach(stock::insert);
        stock.insert(Crates.documents(papers.with(bill)));
    }

    /** The bundles of bills in the warehouse drawn on {@code drawee}, taken out (to present). */
    public static List<ItemStack> takeBillsOn(MerchantStock stock, GlobalPos drawee) {
        return stock.removeLoose(s -> s.is(Cargo.DOCUMENTS.get()) && !ids(s).isEmpty()
                && drawee(s).filter(drawee::equals).isPresent());
    }

    /**
     * Bills presented to their drawee by {@code now}: pays each due one it can from {@code pay} (emeralds it can
     * spend), returns what it paid; bills it can't pay yet go back in the bundle returned with the rest, and those
     * long past due are dishonoured (trust falls).
     */
    public Presented present(List<ItemStack> bundles, double pay, long now, ContractBook book) {
        double paid = 0;
        List<Papers.Document> unpaid = new ArrayList<>();
        for (ItemStack bundle : bundles) {
            for (Papers.Document doc : bundle.getOrDefault(Cargo.PAPERS.get(), Papers.NONE).documents()) {
                Bill bill = bills.get(doc.data().getInt("bill"));
                if (bill == null || bill.state == State.PAID) {
                    continue;
                }
                if (bill.due <= now && paid + bill.emeralds <= pay) {
                    paid += bill.emeralds;
                    if (bill.state == State.OUTSTANDING) {
                        book.kept(bill.drawee, bill.payee, HONOURED_WEIGHT, now); // a promise kept
                    }
                    paid(bill);
                    continue;
                }
                // The drawee has its grace from the day the bill is first put to it, not before: a bill no one could
                // carry to it for weeks isn't its fault.
                if (bill.presented < 0) {
                    bill.presented = now;
                    setDirty();
                }
                if (bill.state == State.OUTSTANDING && now > Math.max(bill.due, bill.presented) + grace(bill)) {
                    bill.state = State.DISHONOURED;
                    book.missed(bill.drawee, bill.payee, 1, now);
                    setDirty();
                }
                unpaid.add(doc);
            }
        }
        return new Presented(paid, unpaid.isEmpty() ? List.of() : List.of(Crates.documents(new Papers(unpaid))));
    }

    /** What presenting paid, and the bills to take back. */
    public record Presented(double paid, List<ItemStack> returned) {}

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        bills.values().forEach(b -> {
            CompoundTag entry = new CompoundTag();
            entry.putInt("id", b.id);
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, b.drawee).ifSuccess(t -> entry.put("drawee", t));
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, b.payee).ifSuccess(t -> entry.put("payee", t));
            entry.putDouble("emeralds", b.emeralds);
            entry.putLong("drawn", b.drawn);
            entry.putLong("due", b.due);
            entry.putString("state", b.state.name());
            entry.putInt("contract", b.contract);
            entry.putLong("presented", b.presented);
            list.add(entry);
        });
        tag.put("bills", list);
        tag.putInt("nextId", nextId);
        return tag;
    }

    public static Bills load(CompoundTag tag, HolderLookup.Provider registries) {
        Bills ledger = new Bills();
        for (Tag t : tag.getList("bills", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            var drawee = GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("drawee")).result();
            var payee = GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("payee")).result();
            if (drawee.isEmpty() || payee.isEmpty()) {
                continue;
            }
            Bill bill = new Bill(entry.getInt("id"), drawee.get(), payee.get(), entry.getDouble("emeralds"),
                    entry.getLong("drawn"), entry.getLong("due"));
            bill.state = State.valueOf(entry.getString("state"));
            bill.contract = entry.contains("contract") ? entry.getInt("contract") : -1;
            bill.presented = entry.contains("presented") ? entry.getLong("presented") : -1;
            ledger.bills.put(bill.id, bill);
        }
        ledger.nextId = Math.max(1, tag.getInt("nextId"));
        return ledger;
    }
}
