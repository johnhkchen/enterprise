package dev.b28.enterprise.cargo;

import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Treasury;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Collectors;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.NonNullList;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.component.ItemContainerContents;

/**
 * Packing goods in crates with their papers, and the Merchant's handling of crates that reach its warehouse. A
 * shipment under a contract goes in crates, each with a bundle of documents: a bill of lading naming the shipper, the
 * consignee, the goods and the terms, and a copy of the contract. The consignee's Merchant opens crates consigned to
 * it, whoever brought them (a caravan, or a player), delivers the goods under the contract the bill names, takes the
 * contract's copy, and keeps the empty crate for its own shipping. A Merchant uses its own empty crates first, and pays
 * to have the rest made (for now, conjured; later, a carpenter's).
 */
public final class Crates {
    public static final int GOODS = 9;
    public static final int PAPERS_SLOT = 9;
    public static final int SLOTS = 10;

    /**
     * Empty crates stack, this many to a stack (a packed crate doesn't); a Merchant keeps a stack of them for its own
     * shipping and hands the rest back to the caravan that brought them, as returnable crates.
     */
    public static final int EMPTIES_KEPT = 16;

    /** What a Merchant pays for a new crate, in emeralds: a log's worth of wood and the work. */
    public static final double CRATE_EMERALDS = 0.125;

    private Crates() {}

    public static boolean isCrate(ItemStack stack) {
        return stack.is(Cargo.CRATE_ITEM.get());
    }

    /** Goods go in the nine slots (not crates or anything that won't go in a box), papers in the tenth. */
    public static boolean fits(int slot, ItemStack stack) {
        if (slot == PAPERS_SLOT) {
            return stack.is(Cargo.DOCUMENTS.get());
        }
        return !isCrate(stack) && !stack.is(Cargo.DOCUMENTS.get()) && stack.getItem().canFitInsideContainerItems();
    }

    private static NonNullList<ItemStack> slots(ItemStack crate) {
        NonNullList<ItemStack> slots = NonNullList.withSize(SLOTS, ItemStack.EMPTY);
        crate.getOrDefault(DataComponents.CONTAINER, ItemContainerContents.EMPTY).copyInto(slots);
        return slots;
    }

    /** The goods in a crate. */
    public static List<ItemStack> goods(ItemStack crate) {
        return slots(crate).subList(0, GOODS).stream().filter(s -> !s.isEmpty()).map(ItemStack::copy).toList();
    }

    /** The papers in a crate's documents slot. */
    public static Papers papers(ItemStack crate) {
        return slots(crate).get(PAPERS_SLOT).getOrDefault(Cargo.PAPERS.get(), Papers.NONE);
    }

    public static boolean empty(ItemStack crate) {
        return isCrate(crate) && goods(crate).isEmpty() && slots(crate).get(PAPERS_SLOT).isEmpty();
    }

    /** A bundle of documents holding {@code papers}. */
    public static ItemStack documents(Papers papers) {
        ItemStack bundle = new ItemStack(Cargo.DOCUMENTS.get());
        bundle.set(Cargo.PAPERS.get(), papers);
        return bundle;
    }

    /** A crate holding {@code goods} (up to nine stacks) and, if any, {@code papers}. */
    public static ItemStack of(List<ItemStack> goods, Papers papers) {
        List<ItemStack> slots = new ArrayList<>();
        for (int i = 0; i < GOODS; i++) {
            slots.add(i < goods.size() ? goods.get(i).copy() : ItemStack.EMPTY);
        }
        slots.add(papers.documents().isEmpty() ? ItemStack.EMPTY : documents(papers));
        ItemStack crate = new ItemStack(Cargo.CRATE_ITEM.get());
        crate.set(DataComponents.CONTAINER, ItemContainerContents.fromItems(slots));
        if (!goods.isEmpty() || !papers.documents().isEmpty()) {
            crate.set(DataComponents.MAX_STACK_SIZE, 1); // a packed crate is one of a kind
        }
        return crate;
    }

    /** Stacks of cargo space a load takes: a crate takes what's in it (at least one), anything else one. */
    public static int stacks(List<ItemStack> load) {
        return load.stream().mapToInt(s -> isCrate(s) ? Math.max(1, goods(s).size()) : 1).sum();
    }

    /** A line of what's in a shipment, for its bill of lading. */
    public static String describe(List<ItemStack> goods) {
        return goods.stream().collect(Collectors.groupingBy(s -> BuiltInRegistries.ITEM.getKey(s.getItem()).getPath(),
                        java.util.LinkedHashMap::new, Collectors.summingInt(ItemStack::getCount)))
                .entrySet().stream().map(e -> e.getValue() + " " + e.getKey()).collect(Collectors.joining(", "));
    }

    /**
     * Packs {@code goods} in crates, nine stacks to a crate, each with a copy of {@code papers}: the shipper's own empty
     * crates first, then new ones it pays for. Returns the crates and whatever it couldn't crate (no crates and no money),
     * which travels loose.
     */
    public static List<ItemStack> pack(ServerLevel level, GlobalPos shipper, MerchantStock stock, List<ItemStack> goods,
            Papers papers, double spare, long now) {
        List<ItemStack> packed = new ArrayList<>();
        Mint mint = Mint.get(level.getServer());
        for (int from = 0; from < goods.size(); from += GOODS) {
            List<ItemStack> lot = goods.subList(from, Math.min(goods.size(), from + GOODS));
            boolean crated = takeEmpty(stock);
            if (!crated && spare >= CRATE_EMERALDS) {
                Treasury.withdraw(level, shipper.pos(), stock, CRATE_EMERALDS, mint, now);
                spare -= CRATE_EMERALDS;
                crated = true;
            }
            if (crated) {
                packed.add(of(lot, papers));
            } else {
                packed.addAll(lot); // no crate to be had: it goes loose, without papers
            }
        }
        return packed;
    }

    /** Takes one empty crate from the stock, if it has one. */
    static boolean takeEmpty(MerchantStock stock) {
        boolean[] taken = {false};
        List<ItemStack> removed = stock.removeLoose(s -> {
            if (!taken[0] && empty(s)) {
                taken[0] = true;
                return true;
            }
            return false;
        });
        if (removed.isEmpty()) {
            return false;
        }
        ItemStack stack = removed.get(0);
        if (stack.getCount() > 1) {
            stock.insert(stack.copyWithCount(stack.getCount() - 1)); // one taken from a stack of empties
        }
        return true;
    }

    /** As {@link #pack}, with what the shipper can spare beyond its reserve. */
    public static List<ItemStack> pack(ServerLevel level, GlobalPos shipper, MerchantStock stock, List<ItemStack> goods,
            Papers papers, long now) {
        var territory = dev.b28.enterprise.territory.Territory.get(level.getServer());
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now);
        double spare = Treasury.available(level, shipper.pos(), stock, rate) - territory.reserve(shipper);
        return pack(level, shipper, stock, goods, papers, spare, now);
    }

    /** The papers for a shipment under a contract: its bill of lading and a copy of the contract. */
    public static Papers shipping(dev.b28.enterprise.contract.Contract contract, GlobalPos shipper, GlobalPos consignee,
            List<ItemStack> goods, int route, long now, boolean settled) {
        Papers.Document bill = Papers.billOfLading(contract.id(), shipper, consignee, describe(goods),
                contract.incoterm().name(), route, now);
        bill.data().putBoolean("settled", settled);
        String summary = contract.delivery().map(d -> "%d %s every %.1f days, %d times, at %.3f emeralds each, %s"
                .formatted(d.quantity(), d.describe(), contract.every() / 24000.0, contract.installments(), d.unitPrice(),
                        contract.incoterm().name())).orElse("");
        return Papers.NONE.with(bill).with(Papers.contractCopy(contract.id(), summary));
    }

    /**
     * The Merchant opens a crate consigned to it: takes its papers (a copy of the contract is now its copy), and
     * returns the goods; the crate, emptied, goes into its stock to ship with later.
     */
    public static List<ItemStack> open(MinecraftServer server, GlobalPos desk, MerchantStock stock, ItemStack crate) {
        Papers papers = papers(crate);
        ContractBook book = ContractBook.get(server);
        papers.documents().stream().filter(d -> d.kind().equals(Papers.CONTRACT))
                .forEach(d -> book.deliverCopy(d.data().getInt("contract"), desk));
        ItemStack rest = stock.insert(new ItemStack(Cargo.CRATE_ITEM.get()));
        if (!rest.isEmpty()) {
            dev.b28.enterprise.Enterprise.LOGGER.debug("No room for an empty crate at {}", desk.pos().toShortString());
        }
        return goods(crate);
    }

    /** Empty crates beyond the stack a Merchant keeps, taken from its stock (to go back with a caravan). */
    public static ItemStack spareEmpties(MerchantStock stock) {
        int held = stock.count(new net.minecraft.world.item.trading.ItemCost(Cargo.CRATE_ITEM.get(), 1));
        int spare = 0;
        for (int i = held; i > EMPTIES_KEPT && takeEmpty(stock); i--) {
            spare++;
        }
        return spare == 0 ? ItemStack.EMPTY : new ItemStack(Cargo.CRATE_ITEM.get(), spare);
    }

    /** Is this a crate whose bill of lading names {@code desk} as consignee? */
    public static boolean consignedTo(ItemStack stack, GlobalPos desk) {
        return isCrate(stack) && papers(stack).consignee().filter(desk::equals).isPresent();
    }

    /**
     * Crates in the desk's warehouse consigned to it (a player brought them, or they came home on a caravan): the
     * Merchant opens each, puts the goods in stock, and, if the shipment wasn't already settled, settles it under the
     * contract its bill of lading names: delivered, with the payment owed the seller until a caravan carries it there.
     * Returns the crates opened.
     */
    public static int receive(MinecraftServer server, ServerLevel level, GlobalPos desk, MerchantStock stock, long now) {
        List<ItemStack> crates = stock.removeLoose(s -> consignedTo(s, desk));
        ContractBook book = ContractBook.get(server);
        for (ItemStack crate : crates) {
            Papers papers = papers(crate);
            List<ItemStack> goods = open(server, desk, stock, crate);
            int items = goods.stream().mapToInt(ItemStack::getCount).sum();
            goods.forEach(g -> {
                ItemStack rest = stock.insert(g);
                if (!rest.isEmpty()) {
                    stock.insert(of(List.of(rest), Papers.NONE)); // no room loose: it stays crated
                }
            });
            boolean settled = papers.first(Papers.BILL_OF_LADING).map(d -> d.data().getBoolean("settled")).orElse(true);
            papers.contract().flatMap(book::contract).filter(c -> !settled && c.buyer().equals(desk) && c.inForce())
                    .ifPresent(contract -> {
                        int counted = Math.min(items, contract.owed(now + contract.every()));
                        double owing = counted * contract.delivery().map(d -> d.unitPrice()).orElse(0.0);
                        book.settle(contract, counted, 0, now);
                        book.owe(contract, owing);
                    });
        }
        return crates.size();
    }

    /** Copies of the stacks in a load, opening crates: what's really aboard. */
    public static List<ItemStack> contents(List<ItemStack> load) {
        List<ItemStack> all = new ArrayList<>();
        for (ItemStack stack : load) {
            if (isCrate(stack)) {
                all.addAll(goods(stack));
            } else {
                all.add(stack.copy());
            }
        }
        return all;
    }
}
