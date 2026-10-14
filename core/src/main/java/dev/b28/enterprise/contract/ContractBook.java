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
 * Every contract, and the trust between each pair of parties. Trust starts at a half; each installment settled in
 * full raises it, each missed one lowers it, and parties below {@link #MIN_TRUST} draft nothing new together. The book
 * is the ledger of record; a party knows of a contract only once a carrier has brought it a copy.
 */
public final class ContractBook extends SavedData {
    private static final String NAME = "enterprise_contracts";

    public static final double START_TRUST = 0.5;
    public static final double MIN_TRUST = 0.3;
    public static final double TRUST_PER_SETTLEMENT = 0.05;
    public static final double TRUST_PER_MISS = 0.2;

    private final Map<Integer, Contract> contracts = new LinkedHashMap<>();
    private final Map<String, Double> trust = new HashMap<>(); // by pair of parties, either order
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
        Contract contract = new Contract(nextId++, seller, buyer, List.of(new Delivery(item, quantity, unitPrice)),
                start, every, installments, now);
        contract.knownBy.add(where);
        contract.note("drafted on day %d: %d %s every %.1f days, %d times, at %.3f emeralds each".formatted(
                now / 24000, quantity, BuiltInRegistries.ITEM.getKey(item).getPath(), every / 24000.0, installments,
                unitPrice));
        contracts.put(contract.id, contract);
        setDirty();
        Enterprise.LOGGER.debug("Contract {} drafted: {}", contract.id, contract.history.get(0));
        return contract;
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
        for (Contract contract : contracts.values()) {
            if (!contract.inForce()) {
                continue;
            }
            int judged = contract.dueBy(now - Contract.GRACE_TICKS);
            Optional<Delivery> delivery = contract.delivery();
            while (contract.missesCounted < judged && delivery.isPresent()) {
                int needed = (contract.missesCounted + 1) * delivery.get().quantity();
                if (contract.delivered >= needed) {
                    adjustTrust(contract.seller, contract.buyer, TRUST_PER_SETTLEMENT);
                } else {
                    contract.missed++;
                    adjustTrust(contract.seller, contract.buyer, -TRUST_PER_MISS);
                    contract.note("day %d: installment %d missed".formatted(now / 24000, contract.missesCounted + 1));
                }
                contract.missesCounted++;
            }
            if (contract.missed >= Contract.MISSES_TO_BREAK) {
                contract.state = Contract.State.BROKEN;
                contract.note("day %d: broken after %d misses".formatted(now / 24000, contract.missed));
            }
            setDirty();
        }
    }

    public double trust(GlobalPos a, GlobalPos b) {
        return trust.getOrDefault(pair(a, b), START_TRUST);
    }

    public void adjustTrust(GlobalPos a, GlobalPos b, double delta) {
        trust.put(pair(a, b), Math.max(0, Math.min(1, trust(a, b) + delta)));
        setDirty();
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
        CompoundTag trustTag = new CompoundTag();
        trust.forEach(trustTag::putDouble);
        tag.put("trust", trustTag);
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
        CompoundTag trustTag = tag.getCompound("trust");
        trustTag.getAllKeys().forEach(key -> book.trust.put(key, trustTag.getDouble(key)));
        book.nextId = Math.max(1, tag.getInt("nextId"));
        return book;
    }
}
