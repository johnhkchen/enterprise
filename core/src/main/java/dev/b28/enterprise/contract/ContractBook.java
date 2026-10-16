package dev.b28.enterprise.contract;

import dev.b28.enterprise.Enterprise;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Every contract, and the trust between each pair of parties. Trust is a track record: the installments kept and
 * missed between the two, weighed as {@code (kept + 2) / (kept + missed + 4)}, so it starts at a half, and each kept
 * installment raises it less as the record grows, while a long record isn't undone by one bad week. Misses are
 * forgiven over time (their weight halves every {@link #MISS_HALF_LIFE}); kept installments are remembered longer.
 * A new supplier still ramping up (fewer than {@link #TEETHING} kept) is judged gently: its misses count a quarter.
 * An installment delivered in part counts as kept in part. Below {@link #MIN_TRUST} the two still deal, on trial terms
 * (small lots, nothing paid up front), so a record can always be rebuilt. The book is the ledger of record; a party
 * knows of a contract only once a carrier has brought it a copy.
 */
public final class ContractBook extends SavedData {
    private static final String NAME = "enterprise_contracts";

    public static final double START_TRUST = 0.5;
    public static final double MIN_TRUST = 0.3;
    /** The record every pair starts with: as if two installments kept and two missed. */
    public static final double PRIOR = 2;
    /** A miss's weight halves this often; a kept installment's, this often. */
    public static final long MISS_HALF_LIFE = 20 * 24000L;
    public static final long KEPT_HALF_LIFE = 120 * 24000L;
    /** Until a pair has kept this many installments, a miss counts {@link #TEETHING_WEIGHT}. */
    public static final double TEETHING = 3;
    public static final double TEETHING_WEIGHT = 0.25;
    /** An installment short by at least this share counts toward breaking the contract. */
    public static final double BREAKING_SHORTFALL = 0.5;

    /** Two parties' record: installments kept and missed (weighted, fading), as of game time {@code touched}. */
    public static final class Record {
        GlobalPos a;
        GlobalPos b;
        double kept;
        double missed;
        long touched;

        /** The two parties, as first recorded (either may be the seller). */
        public String parties() {
            return a == null ? "?" : a.pos().toShortString() + " and " + b.pos().toShortString();
        }

        public double kept() {
            return kept;
        }

        public double missed() {
            return missed;
        }

        public double trust() {
            return (kept + PRIOR) / (kept + missed + 2 * PRIOR);
        }

        /** The record as it stands at {@code now}: misses forgiven, and kept installments a little forgotten. */
        Record at(long now) {
            Record r = new Record();
            r.a = a;
            r.b = b;
            double ticks = Math.max(0, now - touched);
            r.kept = kept * Math.pow(0.5, ticks / KEPT_HALF_LIFE);
            r.missed = missed * Math.pow(0.5, ticks / MISS_HALF_LIFE);
            r.touched = Math.max(now, touched);
            return r;
        }
    }

    private final Map<Integer, Contract> contracts = new LinkedHashMap<>();
    private final Map<String, Record> records = new HashMap<>(); // by pair of parties, either order
    private long clock; // the latest game time the book has seen
    private int nextId = 1;

    public static ContractBook get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(ContractBook::new, ContractBook::load, null), NAME);
    }

    /**
     * Drafts a standing order: {@code seller} delivers {@code quantity} of {@code item} to {@code buyer} every
     * {@code every} ticks from {@code start}, {@code installments} times, at {@code unitPrice} emeralds each. Drafted
     * at {@code where} (that party holds the first copy).
     */
    public Contract draftDelivery(GlobalPos seller, GlobalPos buyer, Item item, int quantity, double unitPrice,
            long start, long every, int installments, long now, GlobalPos where) {
        return draft(seller, buyer, new Delivery(item, quantity, unitPrice), Incoterm.DDP, start, every, installments,
                now, where, false);
    }

    /**
     * Drafts a contract for {@code delivery} on {@code incoterm}, from {@code start}, every {@code every} ticks,
     * {@code installments} times, drafted at {@code where} (that party holds the first copy). Drafted in person
     * ({@code bothPresent}: a player's shop at the seller's own market), both hold copies and it's in force at once.
     */
    public Contract draft(GlobalPos seller, GlobalPos buyer, Delivery delivery, Incoterm incoterm, long start, long every,
            int installments, long now, GlobalPos where, boolean bothPresent) {
        Contract contract = new Contract(nextId++, seller, buyer, List.of(delivery), start, every, installments, now);
        contract.incoterm = incoterm;
        contract.knownBy.add(where);
        if (bothPresent) {
            contract.knownBy.add(seller);
            contract.knownBy.add(buyer);
            contract.state = Contract.State.ACTIVE;
        }
        contract.note("drafted on day %d: %d %s every %.1f days, %d times, at %.3f emeralds each, %s".formatted(
                now / 24000, delivery.quantity(), delivery.describe(), every / 24000.0, installments,
                delivery.unitPrice(), incoterm.name()));
        contracts.put(contract.id, contract);
        setDirty();
        Enterprise.LOGGER.debug("Contract {} drafted: {}", contract.id, contract.history.get(0));
        return contract;
    }

    /** As {@link #draft}, with more terms (bonus clauses, a levy) after the first. */
    public Contract draft(GlobalPos seller, GlobalPos buyer, List<Term> terms, Incoterm incoterm, long start, long every,
            int installments, long now, GlobalPos where, boolean bothPresent) {
        Contract contract = new Contract(nextId++, seller, buyer, terms, start, every, installments, now);
        contract.incoterm = incoterm;
        contract.knownBy.add(where);
        dev.b28.enterprise.cargo.Paperwork.drawn(where, 2); // the contract and its copy
        if (bothPresent) {
            contract.knownBy.add(seller);
            contract.knownBy.add(buyer);
            contract.state = Contract.State.ACTIVE;
        }
        String what = contract.delivery().map(d -> d.quantity() + " " + d.describe() + " at %.3f emeralds each"
                        .formatted(d.unitPrice()) + (installments > 1 ? ", %d times every %.1f days".formatted(
                                installments, every / 24000.0) : ""))
                .or(() -> contract.training().map(t -> t.describe() + ", by day " + start / 24000))
                .orElse(contract.levy().map(l -> l.soldiers() + " soldiers at %.2f emeralds a day".formatted(l.payPerDay()))
                        .orElse("?"));
        contract.note("drafted on day %d: %s, %s%s%s".formatted(now / 24000, what, incoterm.name(),
                contract.advance().map(a -> "; " + a.describe()).orElse(""),
                contract.bonuses().isEmpty() ? "" : "; bonuses " + contract.bonuses().stream().map(Bonus::describe)
                        .collect(java.util.stream.Collectors.joining("; "))));
        contracts.put(contract.id, contract);
        setDirty();
        return contract;
    }

    /** Goods for a contract leave on the seller's caravan: where the buyer bears the road, they count as delivered. */
    public void ship(Contract contract, int items, long now) {
        contract.shipped += items;
        contract.note("day %d: %d on the road".formatted(now / 24000, items));
        setDirty();
    }

    /** The goods it shipped have arrived (and are settled): they're no longer on the road. */
    public void arrived(Contract contract, int items) {
        contract.shipped = Math.max(0, contract.shipped - items);
        setDirty();
    }

    /** Contracts in force or drafted between these two for goods like {@code delivery}'s. */
    public List<Contract> open(GlobalPos seller, GlobalPos buyer, Delivery delivery) {
        return contracts.values().stream()
                .filter(c -> c.state == Contract.State.DRAFTED || c.state == Contract.State.ACTIVE)
                .filter(c -> c.seller.equals(seller) && c.buyer.equals(buyer))
                .filter(c -> c.delivery().filter(d -> d.item() == delivery.item()
                        && d.enchantments().equals(delivery.enchantments())).isPresent())
                .toList();
    }

    /** Contracts either party of a route knows and the other doesn't: copies for a carrier between them. */
    public List<Contract> copiesFor(GlobalPos from, GlobalPos to) {
        // (from may be a party, or a hub the copy was left at to pass on)
        return contracts.values().stream()
                .filter(c -> c.state == Contract.State.DRAFTED || c.state == Contract.State.ACTIVE)
                .filter(c -> c.seller.equals(to) || c.buyer.equals(to))
                .filter(c -> c.knownBy(from) && !c.knownBy(to))
                .toList();
    }

    /**
     * Copies {@code from} holds, for a party {@code hub} can pass them on to (by its roads, as {@code onward} says),
     * neither party being the hub: left at the hub, they ride on with its caravans or those calling there.
     */
    public List<Contract> copiesVia(GlobalPos from, GlobalPos hub, java.util.function.Predicate<GlobalPos> onward) {
        return contracts.values().stream()
                .filter(c -> c.state == Contract.State.DRAFTED || c.state == Contract.State.ACTIVE)
                .filter(c -> !c.seller.equals(hub) && !c.buyer.equals(hub) && c.knownBy(from) && !c.knownBy(hub))
                .filter(c -> !c.knownBy(c.seller) && onward.test(c.seller) || !c.knownBy(c.buyer) && onward.test(c.buyer))
                .toList();
    }

    /** A carrier brings {@code desk} its copy; once both parties hold one, the contract is in force. */
    public void deliverCopy(int id, GlobalPos desk) {
        Contract contract = contracts.get(id);
        if (contract == null || !contract.knownBy.add(desk)) {
            return;
        }
        if (contract.state == Contract.State.DRAFTED && contract.knownBy.contains(contract.seller)
                && contract.knownBy.contains(contract.buyer)) {
            contract.state = Contract.State.ACTIVE;
        }
        setDirty();
    }

    public Optional<Contract> contract(int id) {
        return Optional.ofNullable(contracts.get(id));
    }

    public List<Contract> all() {
        return List.copyOf(contracts.values());
    }

    /**
     * What {@code seller} still has to deliver of plain {@code item} under contracts in force it holds: what it should
     * make, if it's a trade's work.
     */
    public int toDeliver(GlobalPos seller, Item item) {
        return contracts.values().stream()
                .filter(c -> c.inForce() && c.seller.equals(seller) && c.knownBy(seller))
                .mapToInt(c -> c.delivery().filter(d -> d.item() == item && !d.toOrder())
                        .map(d -> Math.max(0, d.quantity() * c.installments - c.delivered)).orElse(0))
                .sum();
    }

    /** Contracts this party is in, any state. */
    public List<Contract> of(GlobalPos desk) {
        return contracts.values().stream().filter(c -> c.seller.equals(desk) || c.buyer.equals(desk)).toList();
    }

    /** Contracts in force from this seller to this buyer, as the seller knows them. */
    public List<Contract> inForce(GlobalPos seller, GlobalPos buyer) {
        return contracts.values().stream()
                .filter(c -> c.inForce() && c.seller.equals(seller) && c.buyer.equals(buyer) && c.knownBy(seller))
                .toList();
    }

    /** Is there a contract (drafted or in force) for this good between these two? Then there's no need of another. */
    public boolean covers(GlobalPos seller, GlobalPos buyer, Item item) {
        return contracts.values().stream()
                .filter(c -> c.state == Contract.State.DRAFTED || c.state == Contract.State.ACTIVE)
                .filter(c -> c.seller.equals(seller) && c.buyer.equals(buyer))
                .anyMatch(c -> c.delivery().filter(d -> d.item() == item).isPresent());
    }

    /**
     * Endorsed over: the buyer's side is {@code to}'s from now (a rent's coupon, sold with the rent), on
     * {@code incoterm}. The new buyer holds a copy (the endorsed deed); the seller's stands.
     */
    public void endorse(Contract contract, GlobalPos to, Incoterm incoterm, long now) {
        GlobalPos was = contract.buyer;
        contract.buyer = to;
        contract.incoterm = incoterm;
        contract.knownBy.add(to);
        contract.note("day %d: endorsed over from %s".formatted(now / 24000,
                dev.b28.enterprise.settlement.Names.label(was)));
        setDirty();
    }

    /** Closes a contract whose business is carried on elsewhere (a rent's coupon, its arrears on the next). */
    public void close(Contract contract, String why, long now) {
        if (contract.state == Contract.State.ACTIVE || contract.state == Contract.State.DRAFTED) {
            contract.state = Contract.State.COMPLETED;
            contract.note("day %d: closed, %s".formatted(now / 24000, why));
            setDirty();
        }
    }

    /** Records a delivery under a contract: items delivered and emeralds paid for them. */
    public void settle(Contract contract, int items, double emeralds, long now) {
        contract.delivered += items;
        contract.paid += emeralds;
        contract.note("day %d: %d delivered, %.2f emeralds paid".formatted(now / 24000, items, emeralds));
        if (contract.delivery().map(d -> contract.delivered >= d.quantity() * contract.installments).orElse(false)) {
            contract.state = Contract.State.COMPLETED;
        }
        setDirty();
    }

    /**
     * Judges installments whose grace has run out: one not delivered in full is a miss (the seller's, and trust
     * falls), one delivered in full raises trust; three misses break the contract.
     */
    public void review(long now) {
        review(null, now);
    }

    /**
     * As {@link #review(long)}. Goods count as delivered once there, or (where the buyer bears the road) once on the
     * road. An installment not delivered Ex Works is the buyer's miss if the seller's warehouse held the goods at its
     * last stocktake (they were set aside, and nobody came for them), else the seller's.
     */
    public void review(MinecraftServer server, long now) {
        clock = Math.max(clock, now);
        if (server != null) {
            // What each seller needs to make what it has contracted to sell: its warehouse aims to hold it.
            var warehouse = dev.b28.enterprise.merchant.Warehouse.get(server);
            java.util.Set<GlobalPos> sellers = new java.util.LinkedHashSet<>();
            contracts.values().stream().filter(Contract::inForce).forEach(c -> sellers.add(c.seller));
            warehouse.retainInputs(sellers);
            sellers.forEach(seller -> warehouse.setInputs(seller,
                    dev.b28.enterprise.service.Crafts.contractInputs(server, seller)));
        }
        for (Contract contract : contracts.values()) {
            if (!contract.inForce()) {
                continue;
            }
            if (contract.training().isPresent()) {
                reviewTraining(server, contract, contract.training().get(), now);
                continue;
            }
            Optional<Harvest> harvest = contract.harvest();
            int judged = contract.dueBy(now - harvest.map(Harvest::windowTicks).orElse(Contract.GRACE_TICKS));
            Optional<Delivery> delivery = contract.delivery();
            int counted = contract.delivered + (contract.incoterm.sellerBearsRoad ? 0 : contract.shipped);
            while (contract.missesCounted < judged && delivery.isPresent()) {
                int quantity = delivery.get().quantity();
                int needed = (contract.missesCounted + 1) * quantity;
                double short_ = Math.min(1, Math.max(0, (needed - counted) / (double) quantity));
                boolean last = contract.missesCounted + 1 >= contract.installments;
                if (short_ <= 0) {
                    kept(contract.seller, contract.buyer, 1, now);
                } else if (harvest.isPresent() && (harvest.get().carried()
                        || !last && needed - counted <= Harvest.ARREARS * quantity)) {
                    // A thin or late harvest: the shortfall rolls over to the next installment.
                    kept(contract.seller, contract.buyer, 1 - short_, now);
                    missed(contract.seller, contract.buyer, short_ * Harvest.ROLLED_WEIGHT, now);
                    contract.note("day %d: installment %d %d short, rolled over".formatted(now / 24000,
                            contract.missesCounted + 1, needed - counted));
                } else if (contract.incoterm == Incoterm.EXW && server != null && dev.b28.enterprise.merchant.Warehouse
                        .get(server).onHand(contract.seller, delivery.get().item()) >= quantity) {
                    contract.buyerMissed++;
                    missed(contract.seller, contract.buyer, 1, now);
                    contract.note("day %d: installment %d set aside, not collected".formatted(now / 24000,
                            contract.missesCounted + 1));
                } else {
                    // Delivered in part: kept in part. Mostly undelivered, it counts toward breaking the contract.
                    kept(contract.seller, contract.buyer, 1 - short_, now);
                    missed(contract.seller, contract.buyer, short_, now);
                    if (short_ >= BREAKING_SHORTFALL) {
                        contract.missed++;
                    }
                    contract.note("day %d: installment %d %s".formatted(now / 24000, contract.missesCounted + 1,
                            short_ >= 1 ? "missed" : "%d%% short".formatted(Math.round(short_ * 100))));
                }
                contract.missesCounted++;
            }
            if (contract.missed >= Contract.MISSES_TO_BREAK || contract.buyerMissed >= Contract.MISSES_TO_BREAK) {
                contract.state = Contract.State.BROKEN;
                contract.note("day %d: broken after %d misses by the %s".formatted(now / 24000,
                        Math.max(contract.missed, contract.buyerMissed),
                        contract.missed >= Contract.MISSES_TO_BREAK ? "seller" : "buyer"));
                broken(contract);
            }
            setDirty();
        }
    }

    /** The buyer paid {@code emeralds} up front (coin, or a bill on itself). */
    public void advanced(Contract contract, double emeralds, long now) {
        contract.advanced += emeralds;
        contract.note("day %d: %.2f emeralds paid up front".formatted(now / 24000, emeralds));
        setDirty();
    }

    /** Of a delivery's price, {@code emeralds} were taken off to work off the advance. */
    public void recouped(Contract contract, double emeralds) {
        contract.recouped += emeralds;
        setDirty();
    }

    /** The seller paid back {@code emeralds} of an advance it hadn't worked off. */
    public void refunded(Contract contract, double emeralds) {
        contract.refund = Math.max(0, contract.refund - emeralds);
        contract.note("%.2f emeralds paid back".formatted(emeralds));
        setDirty();
    }

    /** What this seller holds paid up front and not yet worked off: money its contracts' work may spend. */
    public double advancesHeld(GlobalPos seller) {
        return contracts.values().stream().filter(c -> c.inForce() && c.seller.equals(seller))
                .mapToDouble(c -> Math.max(0, c.advanced - c.recouped)).sum();
    }

    /** Contracts from this seller to this buyer with an advance still to pay. */
    public List<Contract> advancesDue(GlobalPos seller, GlobalPos buyer) {
        return contracts.values().stream().filter(c -> c.seller.equals(seller) && c.buyer.equals(buyer)
                && c.advanceDue() > 0.01).toList();
    }

    /** Contracts where this seller owes this buyer back an advance. */
    public List<Contract> refunds(GlobalPos seller, GlobalPos buyer) {
        return contracts.values().stream().filter(c -> c.seller.equals(seller) && c.buyer.equals(buyer)
                && c.refund > 0.01).toList();
    }

    /** The buyer owes the seller this much more for goods delivered: it's paid when a caravan between them can carry it. */
    public void owe(Contract contract, double emeralds) {
        contract.owing += emeralds;
        if (emeralds > 0) {
            contract.note("%.2f emeralds owed to the seller".formatted(emeralds));
        }
        setDirty();
    }

    /** What's owed was paid (carried to the seller). */
    public void paidOwing(Contract contract, double emeralds) {
        contract.owing = Math.max(0, contract.owing - emeralds);
        contract.paid += emeralds;
        contract.note("%.2f emeralds of what was owed paid".formatted(emeralds));
        setDirty();
    }

    /** Contracts where {@code buyer} owes {@code seller} for goods already delivered. */
    public List<Contract> owing(GlobalPos seller, GlobalPos buyer) {
        return contracts.values().stream()
                .filter(c -> c.owing > 0.01 && c.seller.equals(seller) && c.buyer.equals(buyer)).toList();
    }

    /** The buyer couldn't pay for goods it was handed (or that were brought): its miss. */
    public void unpaid(Contract contract, long now) {
        contract.buyerMissed++;
        missed(contract.seller, contract.buyer, 1, now);
        contract.note("day %d: the buyer couldn't pay".formatted(now / 24000));
        if (contract.buyerMissed >= Contract.MISSES_TO_BREAK) {
            contract.state = Contract.State.BROKEN;
            contract.note("day %d: broken after %d misses by the buyer".formatted(now / 24000, contract.buyerMissed));
            broken(contract);
        }
        setDirty();
    }

    /** A town has as long again as it was given before a training contract it's late on breaks. */
    public static final double TRAINING_LATE = 1.0;

    /**
     * A training contract: done once the seller's census shows a villager of the trade at the level (the bonus, less
     * what came up front, is owed it, for the buyer's next caravan to carry); not done by its day plus grace, a miss
     * (lightly, while the two are new to each other), and as long again after that, broken.
     */
    private void reviewTraining(MinecraftServer server, Contract contract, Training training, long now) {
        boolean done = contract.via > 0
                ? Optional.ofNullable(contracts.get(contract.via)).filter(c -> c.state == Contract.State.COMPLETED).isPresent()
                : server != null && trained(server, contract.seller, training);
        if (done) {
            contract.delivered = 1;
            double recoup = contract.recoupFrom(training.bonus(), 1);
            contract.recouped += recoup;
            contract.paid += recoup;
            owe(contract, training.bonus() - recoup);
            kept(contract.seller, contract.buyer, 1, now);
            contract.state = Contract.State.COMPLETED;
            contract.note("day %d: trained".formatted(now / 24000));
            setDirty();
            return;
        }
        long due = contract.start + Contract.GRACE_TICKS;
        if (now > due && contract.missesCounted == 0) {
            contract.missesCounted = 1;
            contract.missed++;
            missed(contract.seller, contract.buyer, 1, now);
            contract.note("day %d: not yet trained".formatted(now / 24000));
            setDirty();
        }
        if (now > due + (long) ((contract.start - contract.drafted) * TRAINING_LATE)) {
            contract.state = Contract.State.BROKEN;
            contract.note("day %d: broken, still untrained".formatted(now / 24000));
            broken(contract);
            setDirty();
        }
    }

    /** The seller crated its pupil's work for the buyer, and drew a bill on it for so much. */
    public void noteOfftake(Contract contract, int items, double emeralds, long now) {
        contract.note("day %d: %d of the pupil's work crated for the buyer, %.2f emeralds drawn".formatted(now / 24000,
                items, emeralds));
        setDirty();
    }

    /** {@code contract} is carried out by {@code further}: done when it is. */
    public void via(Contract contract, Contract further) {
        contract.via = further.id;
        contract.note("carried out by #%d".formatted(further.id));
        setDirty();
    }

    /** Has the seller one more villager of the trade at the level than it had when it took this on, by its census? */
    public static boolean trained(MinecraftServer server, GlobalPos seller, Training training) {
        return at(server, seller, training.profession(), training.level()) > training.already();
    }

    /** The seller's villagers of a trade at a level or above, by its last census. */
    public static int at(MinecraftServer server, GlobalPos seller, String profession, int level) {
        return dev.b28.enterprise.territory.Territory.get(server).bellOf(seller)
                .map(bell -> (int) dev.b28.enterprise.settlement.Settlements.get(server).roster(bell).values().stream()
                        .filter(r -> !r.baby() && r.profession().equals(profession) && r.level() >= level).count())
                .orElse(0);
    }

    /** Training contracts this seller holds in force and knows of: what its Merchant is to train. */
    public List<Contract> trainingFor(GlobalPos seller) {
        return contracts.values().stream()
                .filter(c -> c.inForce() && c.seller.equals(seller) && c.knownBy(seller) && c.training().isPresent())
                .toList();
    }

    /** A broken contract: what was paid up front and not yet worked off is owed back to the buyer. */
    private void broken(Contract contract) {
        double unearned = contract.advanced - contract.recouped;
        if (unearned > 0.01) {
            contract.refund += unearned;
            contract.recouped = contract.advanced;
            contract.note("%.2f emeralds paid up front owed back to the buyer".formatted(unearned));
        }
    }

    /** Trust between two parties now: their record, misses forgiven as far as time allows. */
    public double trust(GlobalPos a, GlobalPos b) {
        return trust(a, b, clock);
    }

    public double trust(GlobalPos a, GlobalPos b, long now) {
        Record record = records.get(pair(a, b));
        return record == null ? START_TRUST : record.at(now).trust();
    }

    /** The two parties' record as it stands now. */
    public Record record(GlobalPos a, GlobalPos b) {
        Record record = records.get(pair(a, b));
        return record == null ? new Record() : record.at(clock);
    }

    /** Every pair's record as it stands now, by the pair's key. */
    public Map<String, Record> records() {
        Map<String, Record> all = new java.util.TreeMap<>();
        records.forEach((key, record) -> all.put(key, record.at(clock)));
        return all;
    }

    /** {@code weight} of an installment (or a service, or a bill) kept between the two. */
    public void kept(GlobalPos a, GlobalPos b, double weight, long now) {
        clock = Math.max(clock, now);
        Record record = records.computeIfAbsent(pair(a, b), k -> fresh(a, b)).at(now);
        record.kept += weight;
        records.put(pair(a, b), record);
        setDirty();
    }

    /** {@code weight} of an installment (or a bill) missed between the two: a quarter while they're new to each other. */
    public void missed(GlobalPos a, GlobalPos b, double weight, long now) {
        clock = Math.max(clock, now);
        Record record = records.computeIfAbsent(pair(a, b), k -> fresh(a, b)).at(now);
        record.missed += weight * (record.kept < TEETHING ? TEETHING_WEIGHT : 1);
        records.put(pair(a, b), record);
        setDirty();
    }

    private static Record fresh(GlobalPos a, GlobalPos b) {
        Record record = new Record();
        record.a = a;
        record.b = b;
        return record;
    }

    /** Is {@code a} still new to {@code b}, ramping up? */
    public boolean teething(GlobalPos a, GlobalPos b) {
        return record(a, b).kept < TEETHING;
    }

    private static String pair(GlobalPos a, GlobalPos b) {
        String x = a.toString();
        String y = b.toString();
        return x.compareTo(y) < 0 ? x + "|" + y : y + "|" + x;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        contracts.values().forEach(contract -> list.add(contract.save()));
        tag.put("contracts", list);
        CompoundTag recordTag = new CompoundTag();
        records.forEach((key, record) -> {
            CompoundTag r = new CompoundTag();
            r.putDouble("kept", record.kept);
            r.putDouble("missed", record.missed);
            r.putLong("touched", record.touched);
            if (record.a != null) {
                GlobalPos.CODEC.encodeStart(net.minecraft.nbt.NbtOps.INSTANCE, record.a).ifSuccess(t -> r.put("a", t));
                GlobalPos.CODEC.encodeStart(net.minecraft.nbt.NbtOps.INSTANCE, record.b).ifSuccess(t -> r.put("b", t));
            }
            recordTag.put(key, r);
        });
        tag.put("records", recordTag);
        tag.putLong("clock", clock);
        tag.putInt("nextId", nextId);
        return tag;
    }

    public static ContractBook load(CompoundTag tag, HolderLookup.Provider registries) {
        ContractBook book = new ContractBook();
        for (Tag t : tag.getList("contracts", Tag.TAG_COMPOUND)) {
            try {
                Contract contract = Contract.load((CompoundTag) t);
                book.contracts.put(contract.id, contract);
            } catch (IllegalArgumentException | IllegalStateException e) {
                Enterprise.LOGGER.warn("Skipping a contract that can't be read: {}", e.getMessage());
            }
        }
        CompoundTag recordTag = tag.getCompound("records");
        recordTag.getAllKeys().forEach(key -> {
            CompoundTag r = recordTag.getCompound(key);
            Record record = new Record();
            record.kept = r.getDouble("kept");
            record.missed = r.getDouble("missed");
            record.touched = r.getLong("touched");
            record.a = GlobalPos.CODEC.parse(net.minecraft.nbt.NbtOps.INSTANCE, r.get("a")).result().orElse(null);
            record.b = GlobalPos.CODEC.parse(net.minecraft.nbt.NbtOps.INSTANCE, r.get("b")).result().orElse(null);
            book.records.put(key, record);
        });
        book.clock = tag.getLong("clock");
        book.nextId = Math.max(1, tag.getInt("nextId"));
        return book;
    }
}
