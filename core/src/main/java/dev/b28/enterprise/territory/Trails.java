package dev.b28.enterprise.territory;

import dev.b28.enterprise.hex.Hex;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Desire paths: the trails the core's wagons wore ({@code Output.Trail}, {@code map.rs}), laid in the world as dirt
 * path. A trail runs through its hex from its edge with the hex the road came from, by the hex's centre, to its edge
 * with the hex it goes on to, as wide as the core says (two blocks, wider as it's worn more: laid again each time);
 * where the ground is soil (grass, dirt, podzol and the like) it's trodden to path, a tree in its way felled (its trunk
 * in the way cut down: its leaves, with no trunk, fall on their own), what grows on it and the leaves over it cleared,
 * and a lone block's bump or dip on the way levelled. Sand, stone, water and anything built are left as they are.
 *
 * <p>Saved with the world: the trails not yet laid (their ground not loaded), laid as it is.
 */
public final class Trails extends SavedData {
    private static final String NAME = "enterprise_trails";
    /** Blocks over a trail's ground cleared of leaves: a wagon's height and its load. */
    static final int HEADROOM = 4;

    /** A trail to lay: its hex, the hexes before and after it on the road (the same hex at a road's end), its width. */
    public record Trail(Hex hex, Hex from, Hex to, int width) {}

    private final List<Trail> waiting = new ArrayList<>();

    public static Trails get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Trails::new, Trails::load, null), NAME);
    }

    public void add(Trail trail) {
        waiting.add(trail);
        setDirty();
    }

    public List<Trail> waiting() {
        return List.copyOf(waiting);
    }

    public void laid(Trail trail) {
        if (waiting.remove(trail)) {
            setDirty();
        }
    }

    /** The line a trail treads, in blocks: from its edge with {@code from}, by its centre, to its edge with {@code to}. */
    public static double[][] line(Trail t) {
        double cx = t.hex().centerX(), cz = t.hex().centerZ();
        double ax = (cx + t.from().centerX()) / 2, az = (cz + t.from().centerZ()) / 2;
        double bx = (cx + t.to().centerX()) / 2, bz = (cz + t.to().centerZ()) / 2;
        return new double[][] {{ax, az}, {cx, cz}, {bx, bz}};
    }

    /** The chunks a trail's line crosses (to be loaded before it's laid). */
    public static List<ChunkPos> chunks(Trail t) {
        List<ChunkPos> out = new ArrayList<>();
        for (BlockPos p : columns(line(t), t.width())) {
            ChunkPos c = new ChunkPos(p);
            if (!out.contains(c)) {
                out.add(c);
            }
        }
        return out;
    }

    /** The columns along a polyline, in order, each once: a block's step at a time, {@code width} wide about it. */
    static List<BlockPos> columns(double[][] points, int width) {
        List<BlockPos> out = new ArrayList<>();
        java.util.Set<Long> seen = new java.util.HashSet<>();
        for (int i = 0; i + 1 < points.length; i++) {
            double x0 = points[i][0], z0 = points[i][1], x1 = points[i + 1][0], z1 = points[i + 1][1];
            double length = Math.hypot(x1 - x0, z1 - z0);
            int steps = Math.max(1, (int) Math.ceil(length * 2));
            // The side the second block lies to: a step across the line.
            double sx = length == 0 ? 0 : -(z1 - z0) / length, sz = length == 0 ? 0 : (x1 - x0) / length;
            for (int k = 0; k <= steps; k++) {
                double x = x0 + (x1 - x0) * k / steps, z = z0 + (z1 - z0) * k / steps;
                for (int w = -(width / 2); w < width - width / 2; w++) {
                    BlockPos p = BlockPos.containing(x + sx * w, 0, z + sz * w);
                    if (seen.add(p.asLong())) {
                        out.add(p);
                    }
                }
            }
        }
        return out;
    }

    /** Laid in the world: a trail's line trodden to path where it's soil. Its chunks must be loaded. */
    public static int lay(ServerLevel level, Trail t) {
        return layLine(level, line(t), t.width());
    }

    /**
     * The columns along {@code points} (block x, z), {@code width} wide, trodden to path where their top is soil: a
     * trunk standing there felled, what grows there and the leaves over it cleared, a lone block's bump cut and dip
     * filled to the ground either side. Returns the columns laid.
     */
    public static int layLine(ServerLevel level, double[][] points, int width) {
        List<BlockPos> cols = columns(points, width);
        int[] top = new int[cols.size()];
        for (int i = 0; i < cols.size(); i++) {
            BlockPos c = cols.get(i);
            int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, c.getX(), c.getZ()) - 1;
            while (level.getBlockState(c.atY(y)).is(net.minecraft.tags.BlockTags.LOGS)) {
                level.setBlock(c.atY(y), Blocks.AIR.defaultBlockState(), 3); // a trunk in the way, felled
                y--;
            }
            top[i] = y;
        }
        int laid = 0;
        for (int i = 0; i < cols.size(); i++) {
            int y = top[i];
            BlockPos c = cols.get(i);
            BlockState ground = level.getBlockState(c.atY(y));
            if (!soil(ground)) {
                continue;
            }
            // A lone block's bump or dip along the way (the columns a step before and after, WIDTH apart, both level).
            int before = i >= width ? top[i - width] : y, after = i + width < cols.size() ? top[i + width] : y;
            if (before == after && y == before + 1 && soil(level.getBlockState(c.atY(y - 1)))) {
                level.setBlock(c.atY(y), Blocks.AIR.defaultBlockState(), 3);
                y -= 1;
            } else if (before == after && y == before - 1 && level.getBlockState(c.atY(y + 1)).canBeReplaced()
                    && level.getFluidState(c.atY(y + 1)).isEmpty()) {
                level.setBlock(c.atY(y), Blocks.DIRT.defaultBlockState(), 3);
                y += 1;
            }
            BlockState above = level.getBlockState(c.atY(y + 1));
            if (!above.isAir() && above.canBeReplaced() && level.getFluidState(c.atY(y + 1)).isEmpty()) {
                level.setBlock(c.atY(y + 1), Blocks.AIR.defaultBlockState(), 3);
                BlockState higher = level.getBlockState(c.atY(y + 2)); // a tall plant's top half
                if (!higher.isAir() && higher.canBeReplaced() && level.getFluidState(c.atY(y + 2)).isEmpty()) {
                    level.setBlock(c.atY(y + 2), Blocks.AIR.defaultBlockState(), 3);
                }
            }
            for (int h = y + 1; h <= y + HEADROOM; h++) {
                if (level.getBlockState(c.atY(h)).is(net.minecraft.tags.BlockTags.LEAVES)) {
                    level.setBlock(c.atY(h), Blocks.AIR.defaultBlockState(), 3);
                }
            }
            level.setBlock(c.atY(y), Blocks.DIRT_PATH.defaultBlockState(), 3);
            laid++;
        }
        return laid;
    }

    /** Ground a wagon's wheels tread to path: the soil a world grows grass on. */
    static boolean soil(BlockState state) {
        return state.is(Blocks.GRASS_BLOCK) || state.is(Blocks.DIRT) || state.is(Blocks.COARSE_DIRT)
                || state.is(Blocks.PODZOL) || state.is(Blocks.MYCELIUM) || state.is(Blocks.ROOTED_DIRT)
                || state.is(Blocks.DIRT_PATH);
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        for (Trail t : waiting) {
            CompoundTag e = new CompoundTag();
            e.putIntArray("hexes", new int[] {t.hex().q(), t.hex().r(), t.from().q(), t.from().r(), t.to().q(), t.to().r()});
            e.putInt("width", t.width());
            list.add(e);
        }
        tag.put("waiting", list);
        return tag;
    }

    public static Trails load(CompoundTag tag, HolderLookup.Provider registries) {
        Trails trails = new Trails();
        for (Tag t : tag.getList("waiting", Tag.TAG_COMPOUND)) {
            int[] h = ((CompoundTag) t).getIntArray("hexes");
            int width = ((CompoundTag) t).contains("width") ? ((CompoundTag) t).getInt("width") : 2;
            if (h.length == 6) {
                trails.waiting.add(new Trail(new Hex(h[0], h[1]), new Hex(h[2], h[3]), new Hex(h[4], h[5]), width));
            }
        }
        return trails;
    }
}
