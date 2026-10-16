package dev.b28.enterprise.locker;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.PoiTypeTags;
import net.minecraft.tags.TagKey;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiRecord;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.ChestBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.ChestType;
import net.neoforged.neoforge.capabilities.Capabilities;
import net.neoforged.neoforge.items.IItemHandler;
import net.neoforged.neoforge.items.IItemHandlerModifiable;
import net.neoforged.neoforge.items.wrapper.CombinedInvWrapper;

/** Finds lockers: containers touching a workstation. The same rule finds a Merchant's stock around its desk. */
public final class Lockers {
    /** Blocks that can serve as a locker or as a Merchant's stock. */
    public static final TagKey<Block> CONTAINERS = TagKey.create(Registries.BLOCK, Enterprise.id("locker_containers"));

    /** How far from its desk a Merchant looks for lockers, until settlements exist. */
    public static final int MERCHANT_RANGE = 48;

    private Lockers() {}

    /** Containers touching {@code pos} on any face. A double chest appears once, at its lower-sorting half. */
    public static List<BlockPos> touchingContainers(Level level, BlockPos pos) {
        List<BlockPos> found = new ArrayList<>();
        for (Direction dir : Direction.values()) {
            BlockPos at = pos.relative(dir);
            BlockState state = level.getBlockState(at);
            if (!state.is(CONTAINERS)) {
                continue;
            }
            BlockPos key = canonical(at, state);
            if (!found.contains(key)) {
                found.add(key);
            }
        }
        return found;
    }

    /**
     * Lockers a Merchant at {@code desk} can manage: job sites within {@code range} with a container touching them,
     * at most {@code limit}. Lockers of the trades its contracts need come first (a smith it's contracted to train,
     * a Weaponsmith whose swords it has contracted to sell), then its clients (see {@link #review}), then those worth
     * most to the town, then those with trades switched on, then the nearest; otherwise a Merchant with few slots
     * could spend them on lockers it has nothing to do with, and never get to the work it's paid for. Merchant's Desks
     * are never lockers.
     */
    public static List<Locker> near(ServerLevel level, BlockPos desk, int range, int limit) {
        PoiManager pois = level.getPoiManager();
        pois.ensureLoadedAndValid(level, desk, range);
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        Set<BlockPos> switchedOn = MerchantBook.get(level.getServer()).workstations(at);
        Set<String> needed = dev.b28.enterprise.market.Apprentices.neededTrades(level.getServer(), at);
        MerchantBook book = MerchantBook.get(level.getServer());
        List<BlockPos> clients = book.clients(at);
        return pois.getInRange(
                        type -> type.is(PoiTypeTags.ACQUIRABLE_JOB_SITE) && !type.is(Merchant.DESK_POI.getKey()),
                        desk,
                        range,
                        PoiManager.Occupancy.ANY)
                .sorted(Comparator.comparing((PoiRecord poi) -> !needed.contains(trade(poi.getPoiType())))
                        .thenComparingInt(poi -> clients.contains(poi.getPos()) ? clients.indexOf(poi.getPos())
                                : Integer.MAX_VALUE)
                        .thenComparingDouble(poi -> -book.worth(at, poi.getPos()))
                        .thenComparing(poi -> !switchedOn.contains(poi.getPos()))
                        .thenComparingDouble(poi -> poi.getPos().distSqr(desk))
                        .thenComparing(PoiRecord::getPos))
                .map(poi -> new Locker(poi.getPos(), poi.getPoiType(), touchingContainers(level, poi.getPos())))
                .filter(locker -> !locker.containers().isEmpty())
                .limit(limit)
                .toList();
    }

    /** A waiting locker takes a client's place when worth this many times as much to the town... */
    public static final double SWAP_MARGIN = 1.5;
    /** ...and at least this many emeralds a restock more. */
    public static final double SWAP_GAIN = 0.5;
    /** A new client keeps its place this many days at least: time to trade, before it's judged on what it did. */
    public static final int CLIENT_DAYS = 2;

    /**
     * Once a day (at the census): the Merchant values each locker in reach by what its villager's trades would do
     * for the town a restock, takes on clients up to its level's limit, and lets the weakest go for a waiting locker
     * worth {@link #SWAP_MARGIN} times as much: it switches the old client's trades off, and the new one is served
     * from then on. One change a day, so its clients don't churn; a contract's lockers are never let go. Returns the
     * locker let go, if any.
     */
    public static java.util.Optional<BlockPos> review(ServerLevel level, BlockPos desk) {
        var merchant = Merchant.workingAt(level, desk, MERCHANT_RANGE);
        if (merchant.isEmpty()) {
            return java.util.Optional.empty();
        }
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        MerchantBook book = MerchantBook.get(level.getServer());
        var stock = dev.b28.enterprise.merchant.MerchantStock.at(level, desk);
        var territory = dev.b28.enterprise.territory.Territory.get(level.getServer());
        // What the town has to sell now, in emeralds: its warehouse, not the harvest still in the ground.
        java.util.Map<String, Double> supply = dev.b28.enterprise.territory.Jobs.supply(java.util.Map.of(), stock);
        Set<net.minecraft.world.item.Item> wanted = dev.b28.enterprise.caravan.Caravans.wantedAt(level.getServer(), at);
        List<Locker> all = near(level, desk, MERCHANT_RANGE, Integer.MAX_VALUE);
        // A client is worth what it did for the town since the last review, a restock's share (two a day), if more
        // than what it could do with the goods left: having sold the town's carrots, a Farmer has none left to buy.
        Map<BlockPos, Double> earned = book.takeEarned(at);
        for (Locker locker : all) {
            Merchant.workingAt(level, locker.workstation(), MERCHANT_RANGE).ifPresent(v -> {
                double w = Math.max(worth(v.getOffers(), supply, wanted), earned.getOrDefault(locker.workstation(), 0.0) / 2);
                book.setWorth(at, locker.workstation(), w);
                book.noteSlack(at, locker.workstation(), w <= 0.01); // a trade doing nothing for the town today
            });
        }
        int limit = Merchant.lockerLimit(merchant.get().getVillagerData().getLevel());
        Set<String> needed = dev.b28.enterprise.market.Apprentices.neededTrades(level.getServer(), at);
        List<BlockPos> taken = new ArrayList<>(all.stream().limit(limit).map(Locker::workstation).toList());
        List<Locker> waiting = all.stream().skip(limit).toList();
        java.util.Optional<BlockPos> letGo = java.util.Optional.empty();
        var best = waiting.stream().max(Comparator.comparingDouble(l -> book.worth(at, l.workstation())));
        long now = level.getGameTime();
        var weakest = all.stream().limit(limit).filter(l -> !needed.contains(trade(l.type()))
                        && now - book.takenOn(at, l.workstation()) >= CLIENT_DAYS * 24000L)
                .min(Comparator.comparingDouble(l -> book.worth(at, l.workstation())));
        if (best.isPresent() && weakest.isPresent()) {
            double gain = book.worth(at, best.get().workstation());
            double loss = book.worth(at, weakest.get().workstation());
            if (gain >= SWAP_MARGIN * loss && gain - loss >= SWAP_GAIN) {
                BlockPos old = weakest.get().workstation();
                for (String key : Set.copyOf(book.enabledTrades(at, old))) {
                    if (!book.isTraining(at, old, key)) {
                        book.setEnabled(at, old, key, false);
                    }
                }
                taken.set(taken.indexOf(old), best.get().workstation());
                book.setTakenOn(at, best.get().workstation(), now);
                letGo = java.util.Optional.of(old);
                Enterprise.LOGGER.debug("Merchant at {} lets its locker at {} go ({} emeralds a restock) for {} ({})",
                        desk.toShortString(), old.toShortString(), String.format("%.1f", loss),
                        best.get().workstation().toShortString(), String.format("%.1f", gain));
            }
        }
        book.setClients(at, taken);
        // A client's trades that would buy goods the town has are switched on: taking it on is serving it.
        for (Locker locker : all) {
            if (!taken.contains(locker.workstation())) {
                continue;
            }
            Merchant.workingAt(level, locker.workstation(), MERCHANT_RANGE).ifPresent(v -> {
                for (var offer : v.getOffers()) {
                    var cost = offer.getBaseCostA();
                    if (offer.getResult().is(net.minecraft.world.item.Items.EMERALD) && offer.getCostB().isEmpty()
                            && !cost.isEmpty() && supply.getOrDefault(net.minecraft.core.registries.BuiltInRegistries
                                    .ITEM.getKey(cost.getItem()).toString(), 0.0) >= 1) {
                        book.setEnabled(at, locker.workstation(), dev.b28.enterprise.merchant.TradeKeys.of(offer), true);
                    }
                }
            });
        }
        return letGo;
    }

    /**
     * What a villager's trades would do for the town a restock, in emeralds: what its buying trades would pay for
     * the town's goods (as far as it has them, see {@link dev.b28.enterprise.territory.Jobs#supply}), and half what
     * its selling trades would charge for goods the town wants.
     */
    public static double worth(net.minecraft.world.item.trading.MerchantOffers offers, java.util.Map<String, Double> supply,
            Set<net.minecraft.world.item.Item> wanted) {
        double worth = 0;
        for (var offer : offers) {
            var cost = offer.getBaseCostA();
            var result = offer.getResult();
            if (cost.isEmpty() || result.isEmpty()) {
                continue;
            }
            if (result.is(net.minecraft.world.item.Items.EMERALD) && offer.getCostB().isEmpty()) {
                String id = net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(cost.getItem()).toString();
                worth += Math.min((double) offer.getMaxUses() * result.getCount(), supply.getOrDefault(id, 0.0));
            } else if (cost.is(net.minecraft.world.item.Items.EMERALD) && wanted.contains(result.getItem())) {
                worth += 0.5 * offer.getMaxUses() * cost.getCount();
            }
        }
        return worth;
    }

    /** The trade whose job site this is, by its registered name (empty if none). */
    static String trade(net.minecraft.core.Holder<net.minecraft.world.entity.ai.village.poi.PoiType> type) {
        for (var profession : net.minecraft.core.registries.BuiltInRegistries.VILLAGER_PROFESSION) {
            if (profession.heldJobSite().test(type)) {
                return net.minecraft.core.registries.BuiltInRegistries.VILLAGER_PROFESSION.getKey(profession).toString();
            }
        }
        return "";
    }

    /** One inventory over all the given containers, in order. Double chests expose both halves. */
    public static IItemHandler inventory(Level level, List<BlockPos> containers) {
        List<IItemHandlerModifiable> handlers = new ArrayList<>();
        for (BlockPos pos : containers) {
            IItemHandler handler = level.getCapability(Capabilities.ItemHandler.BLOCK, pos, null);
            if (handler instanceof IItemHandlerModifiable modifiable) {
                handlers.add(modifiable);
            }
        }
        return new CombinedInvWrapper(handlers.toArray(IItemHandlerModifiable[]::new));
    }

    /** The position a container is known by: a double chest's lower-sorting half. */
    public static BlockPos canonicalContainer(BlockPos pos, BlockState state) {
        return canonical(pos, state);
    }

    private static BlockPos canonical(BlockPos pos, BlockState state) {
        if (state.getBlock() instanceof ChestBlock && state.getValue(ChestBlock.TYPE) != ChestType.SINGLE) {
            BlockPos other = pos.relative(ChestBlock.getConnectedDirection(state));
            return pos.compareTo(other) <= 0 ? pos : other;
        }
        return pos;
    }
}
