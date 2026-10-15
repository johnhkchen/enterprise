package dev.b28.enterprise.market;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.cargo.Cargo;
import dev.b28.enterprise.cargo.Crates;
import dev.b28.enterprise.cargo.Papers;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.merchant.Depots;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.OffMap;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.territory.Growth;
import dev.b28.enterprise.territory.Territory;
import dev.ithundxr.createnumismatics.content.coins.CoinItem;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.AxeItem;
import net.minecraft.world.item.HoeItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.PickaxeItem;
import net.minecraft.world.item.ShovelItem;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.ChestBlock;
import net.minecraft.world.level.block.state.properties.ChestType;

/**
 * Storekeeping: what a Merchant does when its warehouse runs short of room. A warehouse that can't take in anything
 * more can't take in the emeralds that pay its villagers, nor the coal they learn on; goods nobody uses (a Novice
 * smith's axes, bought to train it) pile up. Short of room, the Merchant, as far as it must:
 * <ol>
 *   <li>packs goods it isn't using into crates, nine stacks to one slot, those that don't stack first;</li>
 *   <li>keeps the crates in cold storage, a double chest it sets up by the desk if it has none (the cheapest warehouse
 *   there is), where its stock still reaches into them;</li>
 *   <li>gives tools it has no use for to the hands working its land (they gather more with them);</li>
 *   <li>and last of all throws out what's worth least: waste, counted.</li>
 * </ol>
 */
public final class ColdStore {
    /** It starts once fewer than this many slots beyond those it keeps free are left. */
    public static final int MARGIN = 2;
    /** Two chests, set up by the desk: what cold storage costs. */
    public static final double COLD_STORE_EMERALDS = 1;

    private ColdStore() {}

    /** The Merchant at {@code desk} makes room, if it's short. Returns slots freed. */
    public static int tidy(ServerLevel level, GlobalPos desk, MerchantStock stock, long now) {
        if (dev.b28.enterprise.merchant.OffMap.get(level.getServer()).is(desk)) {
            return 0; // a market off the map keeps its own house
        }
        MinecraftServer server = level.getServer();
        if (OffMap.get(server).is(desk)) {
            return 0;
        }
        equipHands(level, desk, stock);
        if (stock.freeSlots() >= MerchantStock.FREE_SLOTS + MARGIN) {
            return 0;
        }
        int before = stock.freeSlots();
        Set<Item> using = using(server, desk);
        pack(level, desk, stock, using, now);
        if (stock.freeSlots() < MerchantStock.FREE_SLOTS + MARGIN && !Depots.get(server).has(desk, Depots.Kind.COLD_STORE)) {
            setUp(level, desk, stock, now);
            pack(level, desk, stock, using, now);
        }
        if (stock.freeSlots() < MerchantStock.FREE_SLOTS) {
            giveTools(level, desk, stock, using);
        }
        if (stock.freeSlots() < MerchantStock.FREE_SLOTS) {
            throwOut(server, desk, stock, using);
        }
        return stock.freeSlots() - before;
    }

    /** What the market is using: what its trades take, what it has contracted to deliver, and what its caravans eat. */
    static Set<Item> using(MinecraftServer server, GlobalPos desk) {
        Set<Item> using = new java.util.HashSet<>(dev.b28.enterprise.caravan.Caravans.wantedAt(server, desk));
        ContractBook book = ContractBook.get(server);
        book.of(desk).stream().filter(c -> c.inForce() && c.seller().equals(desk))
                .forEach(c -> c.delivery().ifPresent(d -> using.add(d.item())));
        using.addAll(Warehouse.get(server).inputs(desk).keySet());
        if (MetalExchange.get(server).at(desk)) {
            MetalExchange.METALS.forEach(m -> using.add(m.item())); // the exchange's metal is there to be traded
        }
        return using;
    }

    /** Loose goods it isn't using: not money, papers, crates, boxes or food, nor anything its work takes. */
    static boolean idle(ItemStack stack, Set<Item> using) {
        return !stack.isEmpty() && !using.contains(stack.getItem()) && !stack.is(Items.EMERALD)
                && !(stack.getItem() instanceof CoinItem) && !Crates.isCrate(stack) && !stack.is(Cargo.DOCUMENTS.get())
                && !stack.has(net.minecraft.core.component.DataComponents.FOOD)
                && !stack.has(net.minecraft.core.component.DataComponents.CONTAINER)
                && stack.getItem().canFitInsideContainerItems();
    }

    /**
     * Packs into crates idle loose goods, and full stacks of what it uses beyond one stack kept loose to hand (a crate
     * is still stock: its villagers' lockers and its caravans reach into it), those that don't stack first; a crate
     * only where it saves room.
     */
    static void pack(ServerLevel level, GlobalPos desk, MerchantStock stock, Set<Item> using, long now) {
        List<ItemStack> idle = new ArrayList<>(stock.removeLoose(s -> idle(s, using)));
        for (Item item : using) {
            if (!new ItemStack(item).getItem().canFitInsideContainerItems() || item.getDefaultMaxStackSize() < 16) {
                continue;
            }
            int full = (int) stock.looseStacks().stream().filter(s -> s.is(item) && s.getCount() == s.getMaxStackSize())
                    .count();
            if (full > 1) {
                idle.addAll(stock.take(new net.minecraft.world.item.trading.ItemCost(item, 1),
                        (full - 1) * item.getDefaultMaxStackSize()));
            }
        }
        if (idle.size() < 2) {
            idle.forEach(stock::insert);
            return;
        }
        idle.sort(Comparator.comparingInt(ItemStack::getMaxStackSize).thenComparingInt(ItemStack::getCount));
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now);
        double spare = Treasury.available(level, desk.pos(), stock, rate) - Territory.get(level.getServer()).reserve(desk);
        List<ItemStack> left = new ArrayList<>();
        int packed = 0;
        for (int from = 0; from < idle.size(); from += Crates.GOODS) {
            List<ItemStack> lot = idle.subList(from, Math.min(idle.size(), from + Crates.GOODS));
            boolean crate = lot.size() >= 2 && (Crates.takeEmpty(stock) || spare >= Crates.CRATE_EMERALDS && buyCrate(level, desk, stock, now));
            if (crate) {
                spare -= Crates.CRATE_EMERALDS;
                ItemStack rest = stock.insert(Crates.of(lot, Papers.NONE));
                if (!rest.isEmpty()) {
                    left.addAll(lot); // nowhere even for the crate: unpack it again
                } else {
                    packed++;
                }
            } else {
                left.addAll(lot);
            }
        }
        left.forEach(stack -> {
            ItemStack rest = stock.insert(stack);
            if (!rest.isEmpty()) {
                Enterprise.LOGGER.debug("Cold store at {}: no room to put back {}", desk.pos().toShortString(), rest);
            }
        });
        if (packed > 0) {
            Enterprise.LOGGER.debug("Cold store at {}: packed {} crate(s)", desk.pos().toShortString(), packed);
        }
    }

    private static boolean buyCrate(ServerLevel level, GlobalPos desk, MerchantStock stock, long now) {
        Treasury.withdraw(level, desk.pos(), stock, Crates.CRATE_EMERALDS, Mint.get(level.getServer()), now);
        return true;
    }

    /** Sets up cold storage: a double chest on open ground by the desk, one of the market's stores for crates. */
    static void setUp(ServerLevel level, GlobalPos desk, MerchantStock stock, long now) {
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now);
        double spare = Treasury.available(level, desk.pos(), stock, rate) - Territory.get(level.getServer()).reserve(desk);
        if (spare < COLD_STORE_EMERALDS) {
            return;
        }
        Growth.site(level, desk.pos(), 2, 3).ifPresent(site -> {
            BlockPos a = site.above();
            BlockPos b = a.east();
            level.setBlock(a, Blocks.CHEST.defaultBlockState().setValue(ChestBlock.FACING, Direction.NORTH)
                    .setValue(ChestBlock.TYPE, ChestType.RIGHT), 3);
            level.setBlock(b, Blocks.CHEST.defaultBlockState().setValue(ChestBlock.FACING, Direction.NORTH)
                    .setValue(ChestBlock.TYPE, ChestType.LEFT), 3);
            Depots.get(level.getServer()).add(level, desk, Depots.Kind.COLD_STORE, a, b);
            Treasury.withdraw(level, desk.pos(), stock, COLD_STORE_EMERALDS, Mint.get(level.getServer()), now);
            Enterprise.LOGGER.debug("Market at {} sets up cold storage at {}", desk.pos().toShortString(), a.toShortString());
        });
    }

    /** Hands working its land without a tool get one, an axe first, from the warehouse. */
    static void equipHands(ServerLevel level, GlobalPos desk, MerchantStock stock) {
        Territory territory = Territory.get(level.getServer());
        var bell = territory.bellOf(desk);
        if (bell.isEmpty()) {
            return;
        }
        int hands = dev.b28.enterprise.territory.Labour.hands(
                dev.b28.enterprise.settlement.Settlements.get(level.getServer()).roster(bell.get())).size();
        int lacking = (int) Math.floor(hands - territory.handTools(bell.get()));
        if (lacking <= 0) {
            return;
        }
        int given = 0;
        for (Item tool : List.of(Items.IRON_AXE, Items.STONE_AXE, Items.IRON_SHOVEL, Items.STONE_SHOVEL)) {
            given += stock.take(new net.minecraft.world.item.trading.ItemCost(tool, 1), lacking - given).stream()
                    .mapToInt(ItemStack::getCount).sum();
        }
        if (given > 0) {
            territory.giveTools(bell.get(), given);
        }
    }

    /** Tools it has no use for go to the hands working its land. */
    static void giveTools(ServerLevel level, GlobalPos desk, MerchantStock stock, Set<Item> using) {
        var bell = Territory.get(level.getServer()).bellOf(desk);
        if (bell.isEmpty()) {
            return;
        }
        List<ItemStack> tools = stock.removeLoose(s -> idle(s, using) && (s.getItem() instanceof AxeItem
                || s.getItem() instanceof PickaxeItem || s.getItem() instanceof ShovelItem || s.getItem() instanceof HoeItem));
        int given = tools.stream().mapToInt(ItemStack::getCount).sum();
        if (given > 0) {
            Territory.get(level.getServer()).giveTools(bell.get(), given);
            Warehouse.get(level.getServer()).recordGiven(desk, given);
        }
    }

    /** Only goods worth less than this each are ever thrown out: the greys. Anything better is kept, room or no. */
    public static final double GREY = 0.1;

    /** Last of all: idle greys worth least are thrown out, until there's room again. */
    static void throwOut(MinecraftServer server, GlobalPos desk, MerchantStock stock, Set<Item> using) {
        int short_ = MerchantStock.FREE_SLOTS - stock.freeSlots();
        List<ItemStack> idle = new ArrayList<>(stock.removeLoose(s -> idle(s, using)
                && Prices.emeraldsEach(s.getItem()) < GREY));
        idle.sort(Comparator.comparingDouble(s -> Prices.emeraldsEach(s.getItem()) * s.getCount()));
        Warehouse warehouse = Warehouse.get(server);
        int thrown = Math.min(short_, idle.size());
        for (ItemStack waste : idle.subList(0, thrown)) {
            warehouse.recordWaste(desk, waste.getCount(), Prices.emeraldsEach(waste.getItem()) * waste.getCount());
            Enterprise.LOGGER.debug("Market at {} throws out {} {}", desk.pos().toShortString(), waste.getCount(),
                    net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(waste.getItem()).getPath());
        }
        idle.subList(thrown, idle.size()).forEach(stock::insert);
    }
}
