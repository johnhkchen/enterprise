package dev.b28.enterprise.territory;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BiomeTags;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.grower.TreeGrower;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * A town's woodlots in the world (core mode; John: "Woodlots, felled visibly"; docs/lumberjack-plan.md): the core keeps
 * each lot's growth (`kit::Kits::woodlots`, told with a town's land), and the host shows it. A lot is a square of
 * {@link #SIZE} in a ring past the town's pastures, its trees at {@link #SPOTS}: grown, they stand (the land's own where
 * it had them); felled, their logs and leaves are gone and saplings stand in their place. Each lot is laid as the core
 * first counts it, nearest the bell first, and shown again when the core's word of it changes and its ground is loaded.
 * Its trees are the wood the town's Lumberjacks like ({@link #wood}; John: "they'll grow the type of lumber they like"),
 * the land's own (spruce on taiga, else oak) before they've said.
 *
 * <p>Saved with the world: each town's lots, where they lie and whether they're shown grown.
 */
public final class Woodlots extends SavedData {
    private static final String NAME = "enterprise_woodlots";

    /** A lot's side, and where its four trees stand in it. */
    public static final int SIZE = 7;
    static final int[][] SPOTS = {{1, 1}, {1, 5}, {5, 1}, {5, 5}};
    /** The woodlots' ring: so deep past the pastures' (a town with no plan: this far from its bell and on). */
    static final int DEPTH = 32;
    static final int NEAREST_UNPLANNED = 24;
    /** How far a lot's ground may rise and fall; how high a grown tree's crown is cleared when it's felled. */
    static final int ROLL = 2;
    static final int CROWN = 24;

    /** A lot: its corner (lowest x and z, on its ground) and whether it's shown grown. */
    public record Lot(BlockPos corner, boolean grown) {}

    private final Map<Long, List<Lot>> lots = new LinkedHashMap<>();

    public static Woodlots get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Woodlots::new, Woodlots::load, null), NAME);
    }

    /** A town's lots as shown. */
    public List<Lot> of(long town) {
        return List.copyOf(lots.getOrDefault(town, List.of()));
    }

    /**
     * Shows a town's woodlots as the core has them ({@code grown}, in the order laid): each new one laid on the nearest
     * free ground of its ring, and each whose growth changed felled and replanted or grown. A lot whose ground isn't
     * loaded waits for a later look; so does a new one with no ground found. How many lots changed.
     */
    public int show(ServerLevel level, long town, BlockPos bell, List<Boolean> grown, Optional<String> log) {
        List<Lot> have = lots.computeIfAbsent(town, t -> new ArrayList<>());
        int changed = 0;
        for (int i = 0; i < grown.size(); i++) {
            boolean want = grown.get(i);
            if (i >= have.size()) {
                var corner = site(level, bell, have);
                if (corner == null) {
                    break;
                }
                // The land's own trees count as grown; else it's planted, and grown at once if the core says so.
                boolean standing = hasTrees(level, corner);
                if (!standing || !want) {
                    fell(level, corner, wood(level, corner, log));
                    standing = false;
                }
                have.add(new Lot(corner, standing));
                setDirty();
                changed++;
            }
            Lot lot = have.get(i);
            if (lot.grown() == want || !loaded(level, lot.corner())) {
                continue;
            }
            Wood wood = wood(level, lot.corner(), log);
            if (want) {
                grow(level, lot.corner(), wood);
            } else {
                fell(level, lot.corner(), wood);
            }
            have.set(i, new Lot(lot.corner(), want));
            setDirty();
            changed++;
            dev.b28.enterprise.settlement.Grounds.unbury(level, lot.corner().offset(SIZE / 2, 0, SIZE / 2), SIZE + 2);
        }
        return changed;
    }

    /** A wood a lot grows: the tree its sapling grows into. */
    public record Wood(TreeGrower grower, BlockState sapling) {}

    static final Wood OAK = new Wood(TreeGrower.OAK, Blocks.OAK_SAPLING.defaultBlockState());
    static final Wood SPRUCE = new Wood(TreeGrower.SPRUCE, Blocks.SPRUCE_SAPLING.defaultBlockState());

    /**
     * The wood of a log by its item id ({@code minecraft:acacia_log}: the acacia's); oak for a log whose tree a single
     * sapling doesn't grow (dark oak's takes four, mangrove's a propagule); the land's own with none said.
     */
    public static Wood wood(ServerLevel level, BlockPos corner, Optional<String> log) {
        if (log.isEmpty()) {
            return taiga(level, corner) ? SPRUCE : OAK;
        }
        return switch (log.get()) {
            case "minecraft:spruce_log" -> SPRUCE;
            case "minecraft:birch_log" -> new Wood(TreeGrower.BIRCH, Blocks.BIRCH_SAPLING.defaultBlockState());
            case "minecraft:jungle_log" -> new Wood(TreeGrower.JUNGLE, Blocks.JUNGLE_SAPLING.defaultBlockState());
            case "minecraft:acacia_log" -> new Wood(TreeGrower.ACACIA, Blocks.ACACIA_SAPLING.defaultBlockState());
            case "minecraft:cherry_log" -> new Wood(TreeGrower.CHERRY, Blocks.CHERRY_SAPLING.defaultBlockState());
            default -> OAK;
        };
    }

    /** Its trees felled (every log and leaf over the lot, its crowns' overhang too) and saplings planted at its spots. */
    public static void fell(ServerLevel level, BlockPos corner, Wood wood) {
        for (int dx = -2; dx < SIZE + 2; dx++) {
            for (int dz = -2; dz < SIZE + 2; dz++) {
                boolean inside = dx >= 0 && dz >= 0 && dx < SIZE && dz < SIZE;
                for (int dy = 1; dy <= CROWN; dy++) {
                    BlockPos at = corner.offset(dx, dy, dz);
                    BlockState s = level.getBlockState(at);
                    if (s.is(BlockTags.LEAVES) || inside && (s.is(BlockTags.LOGS) || s.is(BlockTags.SAPLINGS))) {
                        level.setBlock(at, Blocks.AIR.defaultBlockState(), 2);
                    }
                }
            }
        }
        for (int[] spot : SPOTS) {
            BlockPos ground = surface(level, corner.getX() + spot[0], corner.getZ() + spot[1]);
            if (ground != null && level.getBlockState(ground).is(BlockTags.DIRT)) {
                level.setBlock(ground.above(), wood.sapling(), 3);
            }
        }
    }

    /** Its saplings grown into trees, as bone meal would (a spot with none planted first). */
    public static void grow(ServerLevel level, BlockPos corner, Wood wood) {
        for (int[] spot : SPOTS) {
            BlockPos ground = surface(level, corner.getX() + spot[0], corner.getZ() + spot[1]);
            if (ground == null || !level.getBlockState(ground).is(BlockTags.DIRT)) {
                continue;
            }
            BlockPos at = ground.above();
            if (!level.getBlockState(at).is(BlockTags.SAPLINGS)) {
                level.setBlock(at, wood.sapling(), 3);
            }
            wood.grower().growTree(level, level.getChunkSource().getGenerator(), at, level.getBlockState(at), level.random);
        }
    }

    /** Whether any log stands on the lot. */
    public static boolean hasTrees(ServerLevel level, BlockPos corner) {
        for (int dx = 0; dx < SIZE; dx++) {
            for (int dz = 0; dz < SIZE; dz++) {
                for (int dy = 1; dy <= 6; dy++) {
                    if (level.getBlockState(corner.offset(dx, dy, dz)).is(BlockTags.LOGS)) {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    private static boolean taiga(ServerLevel level, BlockPos corner) {
        return level.getBiome(corner.offset(SIZE / 2, 1, SIZE / 2)).is(BiomeTags.IS_TAIGA);
    }

    private static boolean loaded(ServerLevel level, BlockPos corner) {
        return level.isLoaded(corner) && level.isLoaded(corner.offset(SIZE - 1, 0, SIZE - 1));
    }

    /**
     * The ground's top block in a column, under any tree on it (its logs, leaves and saplings, and plants), or null
     * where it isn't loaded or is water.
     */
    static BlockPos surface(ServerLevel level, int x, int z) {
        if (!level.isLoaded(new BlockPos(x, 0, z))) {
            return null;
        }
        BlockPos at = new BlockPos(x, level.getHeight(Heightmap.Types.MOTION_BLOCKING, x, z) - 1, z);
        while (at.getY() > level.getMinBuildHeight()) {
            BlockState s = level.getBlockState(at);
            if (s.is(BlockTags.LOGS) || s.is(BlockTags.LEAVES) || s.is(BlockTags.SAPLINGS) || s.canBeReplaced() && s.getFluidState().isEmpty()) {
                at = at.below();
                continue;
            }
            return s.getFluidState().isEmpty() ? at : null;
        }
        return null;
    }

    /**
     * The nearest free square for a lot in the town's woodlots' ring: its ground the land's own (no field, path or
     * building), within {@link #ROLL} of level, apart from the town's other lots. Null if none is loaded and free.
     */
    static BlockPos site(ServerLevel level, BlockPos bell, List<Lot> others) {
        var plan = TownPlans.at(level, bell);
        int inner, outer;
        if (plan.isPresent()) {
            var o = plan.get().outer();
            int far = Math.max(Math.max(bell.getX() - o.x0(), o.x1() - bell.getX()), Math.max(bell.getZ() - o.z0(), o.z1() - bell.getZ()));
            inner = far + TownPlans.PASTURES_BEYOND + TownPlans.PASTURE_DEPTH;
        } else {
            inner = NEAREST_UNPLANNED;
        }
        outer = inner + DEPTH;
        List<BlockPos> tries = new ArrayList<>();
        for (int dx = -outer; dx <= outer - SIZE; dx += SIZE + 1) {
            for (int dz = -outer; dz <= outer - SIZE; dz += SIZE + 1) {
                int ring = Math.max(Math.abs(dx + SIZE / 2), Math.abs(dz + SIZE / 2));
                if (ring >= inner && ring <= outer) {
                    tries.add(new BlockPos(bell.getX() + dx, 0, bell.getZ() + dz));
                }
            }
        }
        tries.sort(java.util.Comparator.comparingDouble(p -> p.distSqr(new BlockPos(bell.getX(), 0, bell.getZ()))));
        for (BlockPos t : tries) {
            if (others.stream().anyMatch(l -> Math.abs(l.corner().getX() - t.getX()) <= SIZE
                    && Math.abs(l.corner().getZ() - t.getZ()) <= SIZE)) {
                continue;
            }
            Integer low = null, high = null;
            boolean ok = true;
            for (int dx = 0; dx < SIZE && ok; dx++) {
                for (int dz = 0; dz < SIZE && ok; dz++) {
                    BlockPos g = surface(level, t.getX() + dx, t.getZ() + dz);
                    if (g == null || !Pastures.grassland(level.getBlockState(g))) {
                        ok = false;
                        break;
                    }
                    low = low == null ? g.getY() : Math.min(low, g.getY());
                    high = high == null ? g.getY() : Math.max(high, g.getY());
                }
            }
            if (ok && high - low <= ROLL) {
                return new BlockPos(t.getX(), low, t.getZ());
            }
        }
        return null;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag towns = new ListTag();
        lots.forEach((town, list) -> {
            CompoundTag t = new CompoundTag();
            t.putLong("town", town);
            ListTag ls = new ListTag();
            for (Lot l : list) {
                CompoundTag c = new CompoundTag();
                c.putLong("corner", l.corner().asLong());
                c.putBoolean("grown", l.grown());
                ls.add(c);
            }
            t.put("lots", ls);
            towns.add(t);
        });
        tag.put("towns", towns);
        return tag;
    }

    public static Woodlots load(CompoundTag tag, HolderLookup.Provider registries) {
        Woodlots w = new Woodlots();
        for (Tag t : tag.getList("towns", Tag.TAG_COMPOUND)) {
            CompoundTag town = (CompoundTag) t;
            List<Lot> list = new ArrayList<>();
            for (Tag l : town.getList("lots", Tag.TAG_COMPOUND)) {
                CompoundTag c = (CompoundTag) l;
                list.add(new Lot(BlockPos.of(c.getLong("corner")), c.getBoolean("grown")));
            }
            w.lots.put(town.getLong("town"), list);
        }
        return w;
    }
}
