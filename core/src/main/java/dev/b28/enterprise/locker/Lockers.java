package dev.b28.enterprise.locker;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
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
     * a Weaponsmith whose swords it has contracted to sell), then those with trades switched on, then the nearest;
     * otherwise a Merchant with few slots could spend them on lockers it has nothing to do with, and never get to the
     * work it's paid for. Merchant's Desks are never lockers.
     */
    public static List<Locker> near(ServerLevel level, BlockPos desk, int range, int limit) {
        PoiManager pois = level.getPoiManager();
        pois.ensureLoadedAndValid(level, desk, range);
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        Set<BlockPos> switchedOn = MerchantBook.get(level.getServer()).workstations(at);
        Set<String> needed = dev.b28.enterprise.market.Apprentices.neededTrades(level.getServer(), at);
        return pois.getInRange(
                        type -> type.is(PoiTypeTags.ACQUIRABLE_JOB_SITE) && !type.is(Merchant.DESK_POI.getKey()),
                        desk,
                        range,
                        PoiManager.Occupancy.ANY)
                .sorted(Comparator.comparing((PoiRecord poi) -> !needed.contains(trade(poi.getPoiType())))
                        .thenComparing(poi -> !switchedOn.contains(poi.getPos()))
                        .thenComparingDouble(poi -> poi.getPos().distSqr(desk))
                        .thenComparing(PoiRecord::getPos))
                .map(poi -> new Locker(poi.getPos(), poi.getPoiType(), touchingContainers(level, poi.getPos())))
                .filter(locker -> !locker.containers().isEmpty())
                .limit(limit)
                .toList();
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
