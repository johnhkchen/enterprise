package dev.b28.enterprise.market;

import dev.b28.enterprise.caravan.Caravan;
import dev.b28.enterprise.caravan.Route;
import dev.b28.enterprise.cargo.Cargo;
import dev.b28.enterprise.cargo.Crates;
import dev.b28.enterprise.cargo.Papers;
import dev.b28.enterprise.contract.Bills;
import dev.b28.enterprise.contract.Contract;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.territory.Territory;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Banks: a market whose Merchant lends and discounts, from its own treasury, through its caravans (its agents: money
 * goes only where a caravan carries it). Where one of a bank's caravans calls:
 * <ul>
 *   <li><b>Discounting:</b> bills of exchange the town holds, not yet due, are bought for coin now, less a fee and so
 *   much a day to their due day. A bill drawn on the bank's own market is redeemed early; any other is endorsed to
 *   the bank, carried home, and presented in its turn.</li>
 *   <li><b>Lending against contracts:</b> a town with a contract in force, short of the money its work needs, is lent
 *   up to half of what the goods still to come are worth (less what it was paid up front), secured on the contract.
 *   When the bank's caravan pays for those goods, it takes the loan back out of the payment first; a loan left on a
 *   contract that's done or broken is collected from what the town can spare.</li>
 * </ul>
 * So a bill in hand, or a contract with goods coming, is money now: a town can buy what its contracts need of its
 * villagers today, rather than when it's paid.
 */
public final class Bank extends SavedData {
    private static final String NAME = "enterprise_banks";

    public static final double DISCOUNT_FEE = 0.01;
    public static final double DISCOUNT_PER_DAY = 0.01;
    public static final double LOAN_SHARE = 0.5;
    public static final double LOAN_INTEREST = 0.03;
    /** A bank's caravan carries this much more for its dealings, as far as its market can spare. */
    public static final double FLOAT = 48;

    /** A loan: the bank, the borrower, the contract it's secured on, and what's still owed. */
    public static final class Loan {
        final int id;
        final GlobalPos bank;
        final GlobalPos borrower;
        final int contract;
        final double principal;
        double owed;
        final long made;

        Loan(int id, GlobalPos bank, GlobalPos borrower, int contract, double principal, double owed, long made) {
            this.id = id;
            this.bank = bank;
            this.borrower = borrower;
            this.contract = contract;
            this.principal = principal;
            this.owed = owed;
            this.made = made;
        }

        public GlobalPos borrower() {
            return borrower;
        }

        public int contract() {
            return contract;
        }

        public double principal() {
            return principal;
        }

        public double owed() {
            return owed;
        }
    }

    /** What a bank has done, in all. */
    public static final class Book {
        int billsBought;
        double billsPaid; // emeralds paid for them
        double billsFace; // what they were worth when due
        int loansMade;
        double lent;
        double repaid;

        public int billsBought() {
            return billsBought;
        }

        public double billsPaid() {
            return billsPaid;
        }

        public double billsFace() {
            return billsFace;
        }

        public int loansMade() {
            return loansMade;
        }

        public double lent() {
            return lent;
        }

        public double repaid() {
            return repaid;
        }
    }

    private final Set<GlobalPos> banks = new LinkedHashSet<>();
    private final List<Loan> loans = new ArrayList<>();
    private final Map<GlobalPos, Book> books = new LinkedHashMap<>();
    private int nextId = 1;

    public static Bank get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Bank::new, Bank::load, null), NAME);
    }

    public void open(GlobalPos desk) {
        if (banks.add(desk)) {
            setDirty();
        }
    }

    public void close(GlobalPos desk) {
        if (banks.remove(desk)) {
            setDirty();
        }
    }

    public boolean is(GlobalPos desk) {
        return banks.contains(desk);
    }

    public Set<GlobalPos> banks() {
        return Set.copyOf(banks);
    }

    public List<Loan> loans() {
        return List.copyOf(loans);
    }

    public Book book(GlobalPos bank) {
        setDirty();
        return books.computeIfAbsent(bank, b -> new Book());
    }

    /** Loans still owed on a contract. */
    public double owedOn(int contract) {
        return loans.stream().filter(l -> l.contract == contract).mapToDouble(l -> l.owed).sum();
    }

    /**
     * A bank's caravan pays a borrower {@code payment} emeralds for goods under {@code contract}: the loan on it comes
     * out first. Returns what the bank took back.
     */
    public double repayFrom(GlobalPos bank, Contract contract, double payment) {
        double taken = 0;
        for (Loan loan : loans) {
            if (loan.bank.equals(bank) && loan.contract == contract.id() && loan.owed > 0.001 && taken < payment) {
                double back = Math.min(loan.owed, payment - taken);
                loan.owed -= back;
                taken += back;
            }
        }
        if (taken > 0) {
            book(bank).repaid += taken;
            loans.removeIf(l -> l.owed <= 0.001);
            setDirty();
        }
        return taken;
    }

    /** The bank's caravan at a town: buys its bills, lends against its contracts, collects what's due. */
    public static void deal(MinecraftServer server, ServerLevel level, Route route, MerchantStock stock, Caravan caravan,
            Mint mint, double rate, long now) {
        Bank bank = get(server);
        if (!bank.is(route.from())) {
            return;
        }
        int keep = (int) Math.ceil(dev.b28.enterprise.caravan.Caravans.foodNeeded(route.distance())
                * caravan.mouths() * dev.b28.enterprise.caravan.Caravans.PURSE_PER_FOOD_POINT * rate);
        bank.collect(level, route, stock, caravan, mint, rate, now);
        bank.discount(server, level, route, stock, caravan, keep, rate, now);
        bank.lend(server, level, route, stock, caravan, keep, rate, now);
    }

    /** Bills the town holds, not yet due, bought for coin now, less the fee and the days to their due day. */
    void discount(MinecraftServer server, ServerLevel level, Route route, MerchantStock stock, Caravan caravan, int keep,
            double rate, long now) {
        Bills ledger = Bills.get(server);
        List<ItemStack> bundles = stock.removeLoose(s -> s.is(Cargo.DOCUMENTS.get()) && !Bills.ids(s).isEmpty());
        List<Papers.Document> kept = new ArrayList<>();
        List<Papers.Document> bought = new ArrayList<>();
        Book book = book(route.from());
        for (ItemStack bundle : bundles) {
            for (Papers.Document doc : bundle.getOrDefault(Cargo.PAPERS.get(), Papers.NONE).documents()) {
                var bill = ledger.bill(doc.data().getInt("bill")).orElse(null);
                if (bill == null || bill.state() != Bills.State.OUTSTANDING || !bill.payee().equals(route.to())
                        || bill.due() <= now) {
                    kept.add(doc); // not its to sell, settled, or due anyway: presented in the ordinary way
                    continue;
                }
                double days = (bill.due() - now) / 24000.0;
                double price = bill.emeralds() * (1 - DISCOUNT_FEE - DISCOUNT_PER_DAY * days);
                int spurs = (int) Math.floor(price * rate);
                if (spurs <= 0 || caravan.spurs() - keep < spurs) {
                    kept.add(doc);
                    continue;
                }
                caravan.pay(spurs);
                Treasury.deposit(level, route.to().pos(), stock, spurs);
                book.billsBought++;
                book.billsPaid += spurs / rate;
                book.billsFace += bill.emeralds();
                if (bill.drawee().equals(route.from())) {
                    ledger.redeem(bill); // its own market's paper: settled now
                } else {
                    ledger.endorse(bill, route.from());
                    bought.add(doc);
                }
            }
        }
        if (!kept.isEmpty()) {
            stock.insert(Crates.documents(new Papers(kept)));
        }
        if (!bought.isEmpty()) {
            caravan.carry(Crates.documents(new Papers(bought))); // home, to present when due
        }
        setDirty();
    }

    /**
     * Loans against the town's contracts in force, where its money beyond its reserve is short of what's to come:
     * up to half of what the goods still to come are worth, less what was paid up front and lent already.
     */
    void lend(MinecraftServer server, ServerLevel level, Route route, MerchantStock stock, Caravan caravan, int keep,
            double rate, long now) {
        ContractBook contracts = ContractBook.get(server);
        if (contracts.trust(route.to(), route.from(), now) < ContractBook.MIN_TRUST) {
            return;
        }
        double spare = Treasury.available(level, route.to().pos(), stock, rate) - Territory.get(server).reserve(route.to());
        for (Contract contract : contracts.of(route.to())) {
            if (!contract.inForce() || !contract.seller().equals(route.to())) {
                continue;
            }
            double toCome = contract.delivery().map(d -> (d.quantity() * contract.installments() - contract.delivered())
                    * d.unitPrice()).orElse(contract.training().isPresent() ? contract.worth() : 0.0);
            double cap = LOAN_SHARE * toCome - Math.max(0, contract.advanced() - contract.recouped()) - owedOn(contract.id());
            if (cap < 1 || spare >= toCome * LOAN_SHARE) {
                continue; // little to lend on, or it has the money
            }
            int spurs = (int) Math.floor(Math.min(cap, (caravan.spurs() - keep) / rate) * rate);
            if (spurs < rate) {
                continue;
            }
            caravan.pay(spurs);
            Treasury.deposit(level, route.to().pos(), stock, spurs);
            double principal = spurs / rate;
            loans.add(new Loan(nextId++, route.from(), route.to(), contract.id(), principal,
                    principal * (1 + LOAN_INTEREST), now));
            Book book = book(route.from());
            book.loansMade++;
            book.lent += principal;
            spare += principal;
            setDirty();
        }
    }

    /** Loans on contracts done or broken: collected from what the town can spare beyond its reserve. */
    void collect(ServerLevel level, Route route, MerchantStock stock, Caravan caravan, Mint mint, double rate, long now) {
        ContractBook contracts = ContractBook.get(level.getServer());
        for (Loan loan : loans) {
            if (!loan.bank.equals(route.from()) || !loan.borrower.equals(route.to()) || loan.owed <= 0.001) {
                continue;
            }
            boolean over = contracts.contract(loan.contract).map(c -> !c.inForce()).orElse(true);
            if (!over) {
                continue; // repaid when its goods are paid for
            }
            double can = Math.max(0, Treasury.available(level, route.to().pos(), stock, rate)
                    - Territory.get(level.getServer()).reserve(route.to()));
            double back = Math.min(can, loan.owed);
            if (back > 0.01) {
                Treasury.Withdrawal paid = Treasury.withdraw(level, route.to().pos(), stock, back, mint, now);
                caravan.receive(paid.spurs() + mint.sell(Mint.DEFAULT_FACTION, paid.emeralds(), now));
                loan.owed -= paid.value(rate);
                book(route.from()).repaid += paid.value(rate);
            }
        }
        loans.removeIf(l -> l.owed <= 0.001);
        setDirty();
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag desks = new ListTag();
        banks.forEach(b -> GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, b).ifSuccess(desks::add));
        tag.put("banks", desks);
        ListTag list = new ListTag();
        for (Loan l : loans) {
            CompoundTag e = new CompoundTag();
            e.putInt("id", l.id);
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, l.bank).ifSuccess(t -> e.put("bank", t));
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, l.borrower).ifSuccess(t -> e.put("borrower", t));
            e.putInt("contract", l.contract);
            e.putDouble("principal", l.principal);
            e.putDouble("owed", l.owed);
            e.putLong("made", l.made);
            list.add(e);
        }
        tag.put("loans", list);
        ListTag bookList = new ListTag();
        books.forEach((bank, b) -> {
            CompoundTag e = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, bank).ifSuccess(t -> e.put("bank", t));
            e.putInt("billsBought", b.billsBought);
            e.putDouble("billsPaid", b.billsPaid);
            e.putDouble("billsFace", b.billsFace);
            e.putInt("loansMade", b.loansMade);
            e.putDouble("lent", b.lent);
            e.putDouble("repaid", b.repaid);
            bookList.add(e);
        });
        tag.put("books", bookList);
        tag.putInt("nextId", nextId);
        return tag;
    }

    public static Bank load(CompoundTag tag, HolderLookup.Provider registries) {
        Bank bank = new Bank();
        for (Tag t : tag.getList("banks", Tag.TAG_COMPOUND)) {
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, t).ifSuccess(bank.banks::add);
        }
        for (Tag t : tag.getList("loans", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            var b = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("bank")).result();
            var who = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("borrower")).result();
            if (b.isPresent() && who.isPresent()) {
                bank.loans.add(new Loan(e.getInt("id"), b.get(), who.get(), e.getInt("contract"), e.getDouble("principal"),
                        e.getDouble("owed"), e.getLong("made")));
            }
        }
        for (Tag t : tag.getList("books", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("bank")).ifSuccess(b -> {
                Book book = new Book();
                book.billsBought = e.getInt("billsBought");
                book.billsPaid = e.getDouble("billsPaid");
                book.billsFace = e.getDouble("billsFace");
                book.loansMade = e.getInt("loansMade");
                book.lent = e.getDouble("lent");
                book.repaid = e.getDouble("repaid");
                bank.books.put(b, book);
            });
        }
        bank.nextId = Math.max(1, tag.getInt("nextId"));
        return bank;
    }
}
