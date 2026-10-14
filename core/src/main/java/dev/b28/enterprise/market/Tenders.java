package dev.b28.enterprise.market;

import dev.b28.enterprise.contract.Advance;
import dev.b28.enterprise.contract.Contract;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.contract.Incoterm;
import dev.b28.enterprise.contract.Term;
import dev.b28.enterprise.contract.Training;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Jobs;
import dev.b28.enterprise.territory.Labour;
import dev.b28.enterprise.territory.Territory;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Tenders: work a buyer posts on its market's board for any seller to take, at a price, by a day (so far, training:
 * "a master Weaponsmith, 32 emeralds, by day 40"). The board's news travels with caravans: a caravan between two
 * markets lets each one's Merchant read the other's board. A Merchant takes a tender when it's worth it to its own town
 * (the price, against what the training would cost it and whether it can be done in time); one that can't do the work
 * itself, but has roads to towns of its own, carries the tender on into its region, posting it on its own board for a
 * little less, and contracts with the buyer once a town of its takes it up. A tender nobody takes by its day lapses;
 * the buyer may post it again, for more.
 */
public final class Tenders extends SavedData {
    private static final String NAME = "enterprise_tenders";

    /** A Merchant carrying a tender on keeps this share of its price. */
    public static final double RELAY_MARGIN = 0.15;
    /** What a Merchant reckons training costs its town, a level: coal, iron and the rest bought in and carried. */
    public static final double LEVEL_COST = 2;
    /** And the days a level takes, with the coal and iron to hand, and the days to get them there. */
    public static final double DAYS_PER_LEVEL = 2;
    public static final double SUPPLY_DAYS = 2;

    /** A posted tender: its board, its buyer, the work, how many are wanted still, and when they're due. */
    public static final class Tender {
        final int id;
        final GlobalPos board;
        final GlobalPos buyer;
        final Training work;
        int open;
        final long due;
        final int relayFor; // minus the id of the tender this carries on into the board's region, or 0
        final GlobalPos origin; // who first wanted it: the buyer of the tender this one carries on, at the root

        Tender(int id, GlobalPos board, GlobalPos buyer, Training work, int open, long due, int relayFor, GlobalPos origin) {
            this.id = id;
            this.board = board;
            this.buyer = buyer;
            this.work = work;
            this.open = open;
            this.due = due;
            this.relayFor = relayFor;
            this.origin = origin;
        }

        public GlobalPos origin() {
            return origin;
        }

        public GlobalPos board() {
            return board;
        }

        public Training work() {
            return work;
        }

        public int open() {
            return open;
        }

        public long due() {
            return due;
        }
    }

    private final List<Tender> tenders = new ArrayList<>();
    private int nextId = 1;

    public static Tenders get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Tenders::new, Tenders::load, null), NAME);
    }

    /** Posts {@code count} of {@code work} on {@code board}'s board, for {@code buyer}, due by {@code due}. */
    public Tender post(GlobalPos board, GlobalPos buyer, Training work, int count, long due) {
        return post(board, buyer, work, count, due, 0);
    }

    Tender post(GlobalPos board, GlobalPos buyer, Training work, int count, long due, int relayFor) {
        GlobalPos origin = relayFor < 0 ? tenders.stream().filter(t -> t.id == -relayFor).map(t -> t.origin).findFirst()
                .orElse(buyer) : buyer;
        Tender tender = new Tender(nextId++, board, buyer, work, count, due, relayFor, origin);
        tenders.add(tender);
        setDirty();
        return tender;
    }

    /** Open tenders first wanted by {@code origin}, wherever they're posted now (carried on or not). */
    public int openFor(GlobalPos origin, String profession) {
        return tenders.stream().filter(t -> t.origin.equals(origin) && t.work.profession().equals(profession))
                .mapToInt(t -> t.open).sum();
    }

    /** Tenders open on a market's board. */
    public List<Tender> on(GlobalPos board) {
        return tenders.stream().filter(t -> t.board.equals(board) && t.open > 0).toList();
    }

    public List<Tender> all() {
        return List.copyOf(tenders);
    }

    /** Tenders past their day lapse; returns those that lapsed untaken (or partly), for their buyers to think again. */
    public List<Tender> lapse(long now) {
        List<Tender> lapsed = tenders.stream().filter(t -> t.open > 0 && now >= t.due - SUPPLY_DAYS * 24000).toList();
        // A tender carried on that no town took: the Merchant who carried it gives it back.
        lapsed.stream().filter(t -> t.relayFor < 0).forEach(t -> tenders.stream().filter(f -> f.id == -t.relayFor)
                .findFirst().ifPresent(first -> first.open += t.open));
        tenders.removeIf(t -> t.open <= 0 || now >= t.due - SUPPLY_DAYS * 24000);
        setDirty();
        return lapsed;
    }

    /**
     * A caravan between two markets: {@code reader}'s Merchant reads {@code board}'s board and takes what's worth it.
     * Returns contracts drafted.
     */
    public int read(ServerLevel level, GlobalPos reader, GlobalPos board, long now) {
        int taken = 0;
        for (Tender tender : on(board)) {
            if (tender.buyer.equals(reader)) {
                continue;
            }
            while (tender.open > 0 && consider(level, reader, tender, now)) {
                taken++;
            }
        }
        return taken;
    }

    /**
     * What this tender is worth to {@code desk}'s own town: its price, less what training would cost (a stall for a
     * trade it hasn't, coal and iron for each level to go); empty if its town can't do it: no one of the trade and no
     * idle hands, not by the day, no money for it (beyond its reserve, with what's paid up front), or no room in its
     * warehouse for what the pupil makes and learns on (and no cold store for it).
     */
    public static Optional<Double> worth(ServerLevel level, GlobalPos desk, Tender tender, long now) {
        MinecraftServer server = level.getServer();
        var bell = Territory.get(server).bellOf(desk);
        if (bell.isEmpty()) {
            return Optional.empty();
        }
        var roster = Settlements.get(server).roster(bell.get()).values();
        // A new one: the best of the trade not yet at the level (a master it has already doesn't count).
        int best = roster.stream().filter(r -> !r.baby() && r.profession().equals(tender.work.profession())
                        && r.level() < tender.work.level())
                .mapToInt(r -> Math.max(1, r.level())).max().orElse(0);
        boolean idle = roster.stream().anyMatch(r -> !r.baby() && r.profession().equals(Labour.NONE));
        if (best == 0 && !idle) {
            return Optional.empty();
        }
        int levels = Math.max(0, tender.work.level() - Math.max(1, best));
        double days = levels == 0 ? 0 : levels * DAYS_PER_LEVEL + SUPPLY_DAYS + (best == 0 ? 1 : 0);
        if (now + days * 24000 > tender.due) {
            return Optional.empty();
        }
        double cost = (best == 0 ? Jobs.STALL_EMERALDS : 0) + levels * LEVEL_COST;
        var stock = dev.b28.enterprise.merchant.MerchantStock.at(level, desk.pos());
        double rate = dev.b28.enterprise.money.Mint.get(server).rate(dev.b28.enterprise.money.Mint.DEFAULT_FACTION, now);
        double money = dev.b28.enterprise.money.Treasury.available(level, desk.pos(), stock, rate)
                - Territory.get(server).reserve(desk) + tender.work.bonus() * Broker.MAKER_ADVANCE;
        boolean room = stock.freeSlots() >= dev.b28.enterprise.merchant.MerchantStock.FREE_SLOTS
                || dev.b28.enterprise.merchant.Depots.get(server).has(desk, dev.b28.enterprise.merchant.Depots.Kind.COLD_STORE);
        if (money < cost || !room) {
            return Optional.empty();
        }
        return Optional.of(tender.work.bonus() - cost);
    }

    /** {@code desk}'s Merchant considers one of a tender: takes it for its own town, or carries it on. */
    boolean consider(ServerLevel level, GlobalPos desk, Tender tender, long now) {
        MinecraftServer server = level.getServer();
        ContractBook book = ContractBook.get(server);
        Optional<Double> own = worth(level, desk, tender, now);
        boolean already = book.trainingFor(desk).stream().anyMatch(c -> c.training().get().profession()
                .equals(tender.work.profession())) || book.of(desk).stream().anyMatch(c -> c.seller().equals(desk)
                && c.state() == Contract.State.DRAFTED && c.training().isPresent());
        if (own.isPresent() && own.get() > 0 && !already) {
            Contract contract = take(server, desk, tender, now);
            Exchange.get(server).note(desk, now, "took a tender from %s: %s, worth %.1f to this town (#%d)".formatted(
                    tender.board.pos().toShortString(), tender.work.describe(), own.get(), contract.id()));
            return true;
        }
        // Can't (or it isn't worth it) here: carried on, to the towns this market's caravans go to that don't know
        // the board it's on.
        var atlas = dev.b28.enterprise.caravan.Atlas.get(server);
        boolean roads = dev.b28.enterprise.caravan.CaravanLedger.get(server).routes().stream()
                .anyMatch(r -> r.from().equals(desk) && Territory.get(server).bellOf(r.to()).isPresent()
                        && !r.to().equals(tender.buyer) && !atlas.known(server, r.to()).contains(tender.board));
        boolean relaying = tenders.stream().anyMatch(t -> t.board.equals(desk) && t.relayFor == -tender.id);
        if (roads && !relaying && tender.relayFor == 0) {
            tender.open--;
            double price = tender.work.bonus() * (1 - RELAY_MARGIN);
            post(desk, desk, new Training(tender.work.profession(), tender.work.level(), price, tender.work.offtake()), 1,
                    tender.due - (long) (SUPPLY_DAYS * 24000), -tender.id);
            Exchange.get(server).note(desk, now, "carried a tender from %s on to its own towns: %s".formatted(
                    tender.board.pos().toShortString(), tender.work.describe()));
            setDirty();
            return true;
        }
        return false;
    }

    /** Takes one of a tender: a training contract with its buyer (and, carried on, the Merchant's own contract too). */
    Contract take(MinecraftServer server, GlobalPos seller, Tender tender, long now) {
        ContractBook book = ContractBook.get(server);
        tender.open--;
        setDirty();
        Training work = tender.work.forSeller(ContractBook.at(server, seller, tender.work.profession(), tender.work.level()));
        Contract contract = book.draft(seller, tender.buyer, terms(book, seller, tender.buyer, work, now),
                Incoterm.EXW, tender.due, 24000, 1, now, seller, false);
        if (tender.relayFor < 0) {
            // The tender this one carried on: the carrying Merchant now contracts with the first buyer, done when this is.
            tenders.stream().filter(t -> t.id == -tender.relayFor).findFirst().ifPresent(first -> {
                Contract upstream = book.draft(tender.buyer, first.buyer,
                        terms(book, tender.buyer, first.buyer, first.work, now), Incoterm.EXW, first.due, 24000, 1, now,
                        tender.buyer, false);
                book.via(upstream, contract);
            });
        }
        return contract;
    }

    private static List<Term> terms(ContractBook book, GlobalPos seller, GlobalPos buyer, Training work, long now) {
        List<Term> terms = new ArrayList<>();
        terms.add(work);
        if (book.trust(seller, buyer, now) >= ContractBook.MIN_TRUST) {
            terms.add(new Advance(Broker.MAKER_ADVANCE));
        }
        return terms;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        for (Tender t : tenders) {
            CompoundTag entry = new CompoundTag();
            entry.putInt("id", t.id);
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, t.board).ifSuccess(x -> entry.put("board", x));
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, t.buyer).ifSuccess(x -> entry.put("buyer", x));
            entry.put("work", t.work.save());
            entry.putInt("open", t.open);
            entry.putLong("due", t.due);
            entry.putInt("relayFor", t.relayFor);
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, t.origin).ifSuccess(x -> entry.put("origin", x));
            list.add(entry);
        }
        tag.put("tenders", list);
        tag.putInt("nextId", nextId);
        return tag;
    }

    public static Tenders load(CompoundTag tag, HolderLookup.Provider registries) {
        Tenders tenders = new Tenders();
        for (Tag t : tag.getList("tenders", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            var board = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("board")).result();
            var buyer = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("buyer")).result();
            if (board.isPresent() && buyer.isPresent()) {
                tenders.tenders.add(new Tender(e.getInt("id"), board.get(), buyer.get(),
                        (Training) Term.load(e.getCompound("work")), e.getInt("open"), e.getLong("due"),
                        e.getInt("relayFor"), GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("origin")).result()
                                .orElse(buyer.get())));
            }
        }
        tenders.nextId = Math.max(1, tag.getInt("nextId"));
        return tenders;
    }
}
