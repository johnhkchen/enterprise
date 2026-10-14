package dev.b28.enterprise.contract;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.core.GlobalPos;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;

/**
 * An agreement between two parties (Merchant's Desks, or a player's shop): terms owed by the seller to the buyer on
 * a schedule, at a price locked when it was drafted, shipped on its {@link Incoterm}s. News travels with carriers: a party acts on a contract only once
 * it holds a copy, and it's in force once both do. Each installment is due on the schedule; a caravan settles what's
 * owed when it can. A party that falls a whole installment behind past the grace period has missed it (the seller, for
 * goods it didn't have or didn't send; the buyer, for goods set aside that it didn't collect, or couldn't pay for);
 * three misses by either break the contract.
 */
public final class Contract {
    public enum State { DRAFTED, ACTIVE, COMPLETED, BROKEN }

    /** Installments may be settled this long after they fall due before they count as missed. */
    public static final long GRACE_TICKS = 2 * 24000L;

    /** Misses that break a contract. */
    public static final int MISSES_TO_BREAK = 3;

    final int id;
    final GlobalPos seller;
    final GlobalPos buyer;
    final List<Term> terms;
    final long start;
    final long every;
    final int installments;
    final long drafted;
    Incoterm incoterm = Incoterm.DDP;
    State state = State.DRAFTED;
    final Set<GlobalPos> knownBy = new LinkedHashSet<>();
    int delivered; // items delivered so far (for a delivery term)
    double paid; // emeralds paid so far
    int missed; // the seller's misses: not delivered
    int buyerMissed; // the buyer's misses: not collected (Ex Works) or not paid for
    int shipped; // items on the road to the buyer, counted as delivered where the buyer bears the road
    int missesCounted; // installments already judged
    final List<String> history = new ArrayList<>(); // the latest settlements, for the record

    Contract(int id, GlobalPos seller, GlobalPos buyer, List<Term> terms, long start, long every, int installments,
            long drafted) {
        this.id = id;
        this.seller = seller;
        this.buyer = buyer;
        this.terms = List.copyOf(terms);
        this.start = start;
        this.every = every;
        this.installments = installments;
        this.drafted = drafted;
    }

    public int id() {
        return id;
    }

    public GlobalPos seller() {
        return seller;
    }

    public GlobalPos buyer() {
        return buyer;
    }

    public List<Term> terms() {
        return terms;
    }

    public State state() {
        return state;
    }

    public boolean knownBy(GlobalPos desk) {
        return knownBy.contains(desk);
    }

    public int delivered() {
        return delivered;
    }

    public double paid() {
        return paid;
    }

    public int missed() {
        return missed;
    }

    public int buyerMissed() {
        return buyerMissed;
    }

    public int shipped() {
        return shipped;
    }

    public Incoterm incoterm() {
        return incoterm;
    }

    public int installments() {
        return installments;
    }

    public long every() {
        return every;
    }

    public List<String> history() {
        return List.copyOf(history);
    }

    /** The contract's delivery term, if it has one. */
    public java.util.Optional<Delivery> delivery() {
        return terms.stream().filter(Delivery.class::isInstance).map(Delivery.class::cast).findFirst();
    }

    /** Installments due by {@code at}. */
    public int dueBy(long at) {
        if (at < start) {
            return 0;
        }
        return (int) Math.min(installments, (at - start) / every + 1);
    }

    /** Items owed under the delivery term by {@code at}, not yet delivered. */
    public int owed(long at) {
        return delivery().map(d -> Math.max(0, dueBy(at) * d.quantity() - delivered)).orElse(0);
    }

    public boolean inForce() {
        return state == State.ACTIVE;
    }

    void note(String line) {
        history.add(line);
        while (history.size() > 10) {
            history.remove(0);
        }
    }

    CompoundTag save() {
        CompoundTag tag = new CompoundTag();
        tag.putInt("id", id);
        GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, seller).ifSuccess(t -> tag.put("seller", t));
        GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, buyer).ifSuccess(t -> tag.put("buyer", t));
        ListTag termList = new ListTag();
        terms.forEach(term -> termList.add(term.save()));
        tag.put("terms", termList);
        tag.putLong("start", start);
        tag.putLong("every", every);
        tag.putInt("installments", installments);
        tag.putLong("drafted", drafted);
        tag.putString("state", state.name());
        tag.putString("incoterm", incoterm.name());
        tag.putInt("buyerMissed", buyerMissed);
        tag.putInt("shipped", shipped);
        ListTag known = new ListTag();
        knownBy.forEach(desk -> GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(known::add));
        tag.put("knownBy", known);
        tag.putInt("delivered", delivered);
        tag.putDouble("paid", paid);
        tag.putInt("missed", missed);
        tag.putInt("missesCounted", missesCounted);
        ListTag lines = new ListTag();
        history.forEach(line -> lines.add(net.minecraft.nbt.StringTag.valueOf(line)));
        tag.put("history", lines);
        return tag;
    }

    static Contract load(CompoundTag tag) {
        GlobalPos seller = GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("seller")).getOrThrow();
        GlobalPos buyer = GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("buyer")).getOrThrow();
        List<Term> terms = new ArrayList<>();
        tag.getList("terms", Tag.TAG_COMPOUND).forEach(t -> terms.add(Term.load((CompoundTag) t)));
        Contract contract = new Contract(tag.getInt("id"), seller, buyer, terms, tag.getLong("start"),
                tag.getLong("every"), tag.getInt("installments"), tag.getLong("drafted"));
        contract.state = State.valueOf(tag.getString("state"));
        contract.incoterm = tag.contains("incoterm") ? Incoterm.valueOf(tag.getString("incoterm")) : Incoterm.DDP;
        contract.buyerMissed = tag.getInt("buyerMissed");
        contract.shipped = tag.getInt("shipped");
        tag.getList("knownBy", Tag.TAG_COMPOUND).forEach(t ->
                GlobalPos.CODEC.parse(NbtOps.INSTANCE, t).ifSuccess(contract.knownBy::add));
        contract.delivered = tag.getInt("delivered");
        contract.paid = tag.getDouble("paid");
        contract.missed = tag.getInt("missed");
        contract.missesCounted = tag.getInt("missesCounted");
        tag.getList("history", Tag.TAG_STRING).forEach(t -> contract.history.add(t.getAsString()));
        return contract;
    }
}
