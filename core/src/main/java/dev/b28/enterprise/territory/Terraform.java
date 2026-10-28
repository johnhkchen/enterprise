package dev.b28.enterprise.territory;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BlockTags;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.SlabBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.SlabType;
import net.minecraft.world.level.levelgen.Heightmap;

/**
 * A town's land shaped once at its founding, before any plot is laid (John: "the houses are sitting on weird hills";
 * the studio's grader and painter, thread 0013): its streets graded, its plots at their streets' levels, the land
 * between terraced, its edge blended into the wild land (`Grading`); then the ground cut and filled to plan, the trees
 * felled where it moves, the streets paved with half steps where they climb, and the kingdom's earthworks tile laid at
 * every step (walls near streets and buildings, cut faces in the open, quays at the water). Its fields, the forum and
 * anything built stay as they are. A street over water is a bridge: a paved deck on piers, the kingdom's parapet.
 */
public final class Terraform {
    private Terraform() {}

    /** How far past the outer wall the town's ground blends into the wild land. */
    static final int BLEND = 16;

    /** What shaping did. */
    public record Shaped(int columns, int cut, int fill, int felled, int steps, int bridges, int conflicts, int plotsOver,
            int natural) {}

    /**
     * Shapes the town's ground round its plan, if all of it is loaded (empty if not: ask again).
     */
    public static Optional<Shaped> shape(ServerLevel level, TownPlans.Plan plan, String kingdom) {
        if (!loaded(level, plan)) {
            return Optional.empty();
        }
        int x0 = plan.outer().x0() - BLEND - 1, z0 = plan.outer().z0() - BLEND - 1;
        int X = plan.outer().x1() - plan.outer().x0() + 2 * BLEND + 3, Z = plan.outer().z1() - plan.outer().z0() + 2 * BLEND + 3;
        int N = X * Z;
        short[] top = new short[N], fixed = new short[N];
        boolean[] water = new boolean[N];
        for (int z = 0; z < Z; z++) {
            for (int x = 0; x < X; x++) {
                int i = z * X + x, wx = x0 + x, wz = z0 + z;
                fixed[i] = Grading.NONE;
                int y = ground(level, wx, wz);
                BlockState s = level.getBlockState(new BlockPos(wx, y, wz));
                // A fence or wall on the land (the founding's safety fences at its drops) isn't a building: the ground
                // under it is the column's, and shaping takes it down (the studio's r70: the west avenue left unlevelled
                // round fences at the forum's edge).
                while (loose(s) && y > level.getMinBuildHeight()) {
                    s = level.getBlockState(new BlockPos(wx, --y, wz));
                }
                if (!s.getFluidState().isEmpty()) {
                    water[i] = true;
                    top[i] = (short) y;
                } else if (s.is(Blocks.FARMLAND)) {
                    top[i] = (short) y; // a field: kept as it lies
                    fixed[i] = (short) y;
                } else if (dev.b28.enterprise.settlement.Grounds.natural(s)) {
                    top[i] = (short) y;
                } else {
                    // Something built: its ground the first natural block under it, plus one; over water a dock or a bridge.
                    int g = y, wet = Integer.MIN_VALUE;
                    for (; g > level.getMinBuildHeight(); g--) {
                        BlockState b = level.getBlockState(new BlockPos(wx, g, wz));
                        if (wet == Integer.MIN_VALUE && !b.getFluidState().isEmpty()) {
                            wet = g;
                        }
                        if (dev.b28.enterprise.settlement.Grounds.natural(b)) {
                            break;
                        }
                    }
                    if (wet != Integer.MIN_VALUE) {
                        water[i] = true;
                        top[i] = (short) wet;
                    } else {
                        top[i] = (short) (g + 1);
                        fixed[i] = (short) (g + 1);
                    }
                }
            }
        }
        // The plan in the box's frame: the avenues from the forum's edge to the outer wall, the lanes, the plots.
        int bx = plan.bell().getX() - x0, bz = plan.bell().getZ() - z0, f = Forums.REACH + 1;
        int[] o = {plan.outer().x0() - x0, plan.outer().z0() - z0, plan.outer().x1() - x0, plan.outer().z1() - z0};
        List<Grading.Street> streets = new ArrayList<>();
        for (int[][] line : new int[][][] {{{bx, bz - f}, {bx, o[1]}}, {{bx, bz + f}, {bx, o[3]}}, {{bx - f, bz}, {o[0], bz}},
                {{bx + f, bz}, {o[2], bz}}}) {
            streets.add(new Grading.Street("avenue", TownPlans.GATE, line));
        }
        for (double[][] lane : TownPlans.lanes(plan)) {
            streets.add(new Grading.Street("lane", TownPlans.LANE, new int[][] {
                {(int) Math.floor(lane[0][0]) - x0, (int) Math.floor(lane[0][1]) - z0},
                {(int) Math.floor(lane[1][0]) - x0, (int) Math.floor(lane[1][1]) - z0}}));
        }
        List<Grading.Plot> plots = new ArrayList<>();
        for (TownPlans.Frontage p : TownPlans.plots(plan, Plots.width(10))) {
            int door = switch (p.door()) {
                case NORTH -> 0;
                case SOUTH -> 1;
                case EAST -> 2;
                default -> 3;
            };
            plots.add(new Grading.Plot(p.x() - x0, p.z() - z0, p.sx(), p.sz(), door));
        }
        var g = Grading.grade(X, Z, top, water, fixed, streets, plots, bx, bz, Forums.REACH, plan.bell().getY() - 1, o, BLEND);
        short[] target = g.target();
        byte[] role = g.role();
        if (System.getenv("ENTERPRISE_GRADE_PROBE") != null) {
            String[] at = System.getenv("ENTERPRISE_GRADE_PROBE").split(",");
            int px = Integer.parseInt(at[0]) - x0, pz0 = Integer.parseInt(at[1]) - z0;
            for (int pz = pz0; pz < pz0 + 7; pz++) {
                StringBuilder b = new StringBuilder();
                for (int dx = -2; dx <= 2; dx++) {
                    if (pz < 0 || pz >= Z || px + dx < 0 || px + dx >= X) {
                        continue;
                    }
                    int i = pz * X + px + dx;
                    b.append(String.format(" [r%d t%d n%d f%d]", role[i], target[i], top[i], fixed[i]));
                }
                dev.b28.enterprise.Enterprise.LOGGER.info("Grade probe z {}:{}", pz + z0, b);
            }
            for (Grading.Edge e : g.edges()) {
                if (Math.abs(e.x() - px) <= 2 && e.z() >= pz0 && e.z() < pz0 + 7) {
                    dev.b28.enterprise.Enterprise.LOGGER.info("Grade probe edge {}", e.withWorld(x0, z0));
                }
            }
        }

        // The trees where the ground moves: on a street, a plot or the forum, or land cut or filled by 2 or more.
        int felled = 0;
        for (int z = 0; z < Z; z++) {
            for (int x = 0; x < X; x++) {
                int i = z * X + x;
                byte r = role[i];
                if (r == Grading.WILD || r == Grading.WATER || r == Grading.FIXED) {
                    continue;
                }
                if (r == Grading.STREET || r == Grading.PLOT || r == Grading.FORUM || Math.abs(target[i] - top[i]) >= 2) {
                    felled += fell(level, x0 + x, top[i], z0 + z);
                }
            }
        }
        // Cut and fill to plan; streets paved, everything else grassed.
        BlockState pave = paving(kingdom), dirt = Blocks.DIRT.defaultBlockState(), grass = Blocks.GRASS_BLOCK.defaultBlockState();
        int columns = 0, bridges = 0;
        for (int z = 0; z < Z; z++) {
            for (int x = 0; x < X; x++) {
                int i = z * X + x;
                byte r = role[i];
                if (g.bridge()[i] != 0) {
                    bridges++; // laid below, as a bridge
                    continue;
                }
                if (r == Grading.WILD || r == Grading.WATER || r == Grading.FIXED) {
                    continue;
                }
                int t = target[i], n = top[i], wx = x0 + x, wz = z0 + z;
                if (t == n && r != Grading.STREET) {
                    continue;
                }
                columns++;
                for (int y = Math.min(t, n) + 1; y <= Math.max(t, n) + 3; y++) {
                    BlockPos at = new BlockPos(wx, y, wz);
                    BlockState b = level.getBlockState(at);
                    if (y > t && (y <= n || b.isAir() || b.canBeReplaced() || loose(b) || dev.b28.enterprise.settlement.Grounds.natural(b))) {
                        level.setBlock(at, Blocks.AIR.defaultBlockState(), 2 | 16);
                    }
                }
                for (int y = n + 1; y < t; y++) {
                    level.setBlock(new BlockPos(wx, y, wz), dirt, 2 | 16);
                }
                level.setBlock(new BlockPos(wx, t, wz), r == Grading.STREET || r == Grading.FORUM ? pave : grass, 2 | 16);
                if (t < n) {
                    BlockState under = level.getBlockState(new BlockPos(wx, t - 1, wz));
                    if (under.isAir() || !under.getFluidState().isEmpty()) {
                        level.setBlock(new BlockPos(wx, t - 1, wz), dirt, 2 | 16);
                    }
                }
                // No hollow under the new ground: an air gap below it (an old floor over the shallows cleared to air,
                // the land laid over it: the studio's r74, a 2-thick crust over water) filled to what's under it.
                for (int y = t - 1; y > t - 12; y--) {
                    BlockPos at = new BlockPos(wx, y, wz);
                    BlockState b = level.getBlockState(at);
                    if (b.isAir() || b.canBeReplaced() && b.getFluidState().isEmpty()) {
                        level.setBlock(at, dirt, 2 | 16);
                    }
                }
            }
        }
        // Bridges where a street crosses water (the studio's bridge.mjs): a paved deck on a stone course at the street's
        // level, piers every 6 down to the bed, the kingdom's parapet along both sides, a lantern over each pier.
        bridges(level, kingdom, x0, z0, X, Z, target, role, g.bridge(), pave);
        // The kingdom's earthworks tile at every step.
        int steps = 0;
        for (Grading.Edge e : g.edges()) {
            if (e.kind().startsWith("street_") || e.kind().equals("slope")) {
                continue;
            }
            if (e.dir() >= 4) {
                // An inner corner: the high column where two walls meet dressed in coursed stone, no dirt gap between them.
                for (int y = target[e.z() * X + e.x()] - e.rise() + 1; y <= target[e.z() * X + e.x()]; y++) {
                    level.setBlock(new BlockPos(x0 + e.x(), y, z0 + e.z()), Blocks.STONE_BRICKS.defaultBlockState(), 2 | 16);
                }
                steps++;
                continue;
            }
            int[] d = Grading.DIRS[e.dir()];
            int j = (e.z() + d[1]) * X + e.x() + d[0];
            int low = target[e.z() * X + e.x()] - e.rise();
            if (Earthworks.step(level, kingdom, x0 + e.x(), z0 + e.z(), d[0], d[1], low, e.rise(),
                    e.kind().equals("cut_face") ? "cut_face" : "wall")) {
                steps++;
            }
            if (role[j] == Grading.STREET && e.kind().equals("wall")) {
                // A wall's foot on a street keeps the street's surface (no stair across the way).
                level.setBlock(new BlockPos(x0 + e.x() + d[0], target[j], z0 + e.z() + d[1]), pave, 2 | 16);
                level.setBlock(new BlockPos(x0 + e.x() + d[0], target[j] + 1, z0 + e.z() + d[1]), Blocks.AIR.defaultBlockState(), 2 | 16);
            }
        }
        // Every street cell paved again and clear to head height, after the walls: a step up a steep street is a wall,
        // and its tile's top would leave grass on the street (the studio's r68 render: 107 grass cells up the avenue).
        for (int z = 0; z < Z; z++) {
            for (int x = 0; x < X; x++) {
                int i = z * X + x;
                if (role[i] != Grading.STREET || g.bridge()[i] != 0) {
                    continue;
                }
                level.setBlock(new BlockPos(x0 + x, target[i], z0 + z), pave, 2 | 16);
                // A pit a tile dug under it (the studio's r69: dirt 3 below the paving either side) filled.
                for (int y = target[i] - 1; y > target[i] - 8; y--) {
                    BlockPos under = new BlockPos(x0 + x, y, z0 + z);
                    BlockState b = level.getBlockState(under);
                    if (!b.isAir() && !b.canBeReplaced()) {
                        break;
                    }
                    level.setBlock(under, Blocks.DIRT.defaultBlockState(), 2 | 16);
                }
                for (int y = target[i] + 1; y <= target[i] + 3; y++) {
                    level.setBlock(new BlockPos(x0 + x, y, z0 + z), Blocks.AIR.defaultBlockState(), 2 | 16);
                }
            }
        }
        // A street climbs by half steps: a slab on the lower cell wherever its neighbour along the street is a block higher.
        BlockState slab = halfStep(kingdom);
        for (int z = 0; z < Z; z++) {
            for (int x = 0; x < X; x++) {
                int i = z * X + x;
                if (role[i] != Grading.STREET || g.bridge()[i] != 0) {
                    continue;
                }
                for (int[] d : Grading.DIRS) {
                    int xx = x + d[0], zz = z + d[1];
                    if (xx < 0 || zz < 0 || xx >= X || zz >= Z) {
                        continue;
                    }
                    int j = zz * X + xx;
                    if ((role[j] == Grading.STREET || role[j] == Grading.FORUM) && target[j] == target[i] + 1) {
                        level.setBlock(new BlockPos(x0 + x, target[i] + 1, z0 + z), slab, 2 | 16);
                        break;
                    }
                }
            }
        }
        return Optional.of(new Shaped(columns, g.cut(), g.fill(), felled, steps, bridges, g.conflicts(), g.plotsOver(),
                g.natural()));
    }

    /** How often a bridge stands on a pier. */
    static final int PIER_EVERY = 6;

    private static void bridges(ServerLevel level, String kingdom, int x0, int z0, int X, int Z, short[] target, byte[] role,
            byte[] bridge, BlockState pave) {
        List<int[]> rails = new ArrayList<>();
        Set<Long> railAt = new HashSet<>();
        for (int z = 0; z < Z; z++) {
            for (int x = 0; x < X; x++) {
                int i = z * X + x;
                if (bridge[i] == 0) {
                    continue;
                }
                int t = target[i], wx = x0 + x, wz = z0 + z;
                boolean alongZ = bridge[i] == 1, pier = Math.floorMod(alongZ ? wz : wx, PIER_EVERY) == 0;
                for (int y = t + 1; y <= t + 3; y++) {
                    level.setBlock(new BlockPos(wx, y, wz), Blocks.AIR.defaultBlockState(), 2 | 16);
                }
                level.setBlock(new BlockPos(wx, t, wz), pave, 2 | 16);
                level.setBlock(new BlockPos(wx, t - 1, wz), Blocks.STONE_BRICKS.defaultBlockState(), 2 | 16);
                // Under the deck, what was built over the water (an old dock's planks: the studio's r69 render) goes,
                // water again where water lies under it.
                for (int y = t - 2; y > level.getMinBuildHeight(); y--) {
                    BlockPos at = new BlockPos(wx, y, wz);
                    BlockState b = level.getBlockState(at);
                    if (b.isAir()) {
                        continue;
                    }
                    if (!b.getFluidState().isEmpty() || dev.b28.enterprise.settlement.Grounds.natural(b)) {
                        break;
                    }
                    BlockState under = level.getBlockState(at.below());
                    level.setBlock(at, under.getFluidState().isEmpty() ? Blocks.AIR.defaultBlockState()
                            : under.getFluidState().createLegacyBlock(), 2 | 16);
                }
                if (pier) {
                    for (int y = t - 2; y > level.getMinBuildHeight(); y--) {
                        BlockPos at = new BlockPos(wx, y, wz);
                        BlockState b = level.getBlockState(at);
                        if (b.getFluidState().isEmpty() && !b.isAir() && !b.canBeReplaced()) {
                            break; // the bed
                        }
                        level.setBlock(at, Blocks.STONE_BRICKS.defaultBlockState(), 2 | 16);
                    }
                }
                int[][] across = alongZ ? new int[][] {{1, 0}, {-1, 0}} : new int[][] {{0, 1}, {0, -1}};
                for (int[] a : across) {
                    int xx = x + a[0], zz = z + a[1];
                    int j = xx < 0 || zz < 0 || xx >= X || zz >= Z ? -1 : zz * X + xx;
                    if (j < 0 || bridge[j] == 0 && role[j] != Grading.STREET && role[j] != Grading.FORUM) {
                        rails.add(new int[] {wx, t + 1, wz, pier ? 1 : 0});
                        railAt.add(BlockPos.asLong(wx, t + 1, wz));
                        break;
                    }
                }
            }
        }
        // The parapet: joined by the game's own shapes once all of it stands; a post over each pier carries a lantern.
        BlockState rail = parapet(kingdom, false), post = parapet(kingdom, true);
        for (int[] r : rails) {
            level.setBlock(new BlockPos(r[0], r[1], r[2]), r[3] == 1 ? post : rail, 2 | 16);
        }
        for (int[] r : rails) {
            BlockPos at = new BlockPos(r[0], r[1], r[2]);
            level.setBlock(at, net.minecraft.world.level.block.Block.updateFromNeighbourShapes(level.getBlockState(at), level, at), 2 | 16);
            if (r[3] == 1) {
                level.setBlock(at.above(), Blocks.LANTERN.defaultBlockState(), 2 | 16);
            }
        }
    }

    /** A kingdom's bridge parapet: Lordaeron's cobble wall, Dalaran's dark oak rail on mangrove posts, Stormwind's stone. */
    static BlockState parapet(String kingdom, boolean post) {
        return switch (kingdom) {
            case "dalaran" -> post ? Blocks.MANGROVE_FENCE.defaultBlockState() : Blocks.DARK_OAK_FENCE.defaultBlockState();
            case "stormwind" -> Blocks.STONE_BRICK_WALL.defaultBlockState();
            default -> Blocks.COBBLESTONE_WALL.defaultBlockState();
        };
    }

    /** Is all the ground shaping would touch (the outer wall and its blend band) loaded? */
    public static boolean loaded(ServerLevel level, TownPlans.Plan plan) {
        int x0 = plan.outer().x0() - BLEND - 1, z0 = plan.outer().z0() - BLEND - 1;
        int x1 = plan.outer().x1() + BLEND + 1, z1 = plan.outer().z1() + BLEND + 1;
        for (int x = x0; x <= x1 + 15; x += 16) {
            for (int z = z0; z <= z1 + 15; z += 16) {
                if (!level.isLoaded(new BlockPos(Math.min(x, x1), 0, Math.min(z, z1)))) {
                    return false;
                }
            }
        }
        return true;
    }

    /** The land's own ground at a column: under any plant, leaves or tree. */
    static int ground(ServerLevel level, int x, int z) {
        int y = level.getHeight(Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z) - 1;
        while (y > level.getMinBuildHeight()) {
            BlockState s = level.getBlockState(new BlockPos(x, y, z));
            if (isTree(s) || s.isAir() || s.canBeReplaced() && s.getFluidState().isEmpty()) {
                y--;
                continue;
            }
            break;
        }
        return y;
    }

    /** A fence or a wall standing on the land: taken down where the ground is shaped. */
    private static boolean loose(BlockState s) {
        return s.is(BlockTags.FENCES) || s.is(BlockTags.WALLS) || s.is(BlockTags.FENCE_GATES);
    }

    private static boolean isTree(BlockState s) {
        String id = net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(s.getBlock()).getPath();
        return s.is(BlockTags.LEAVES) || (id.endsWith("_log") || id.endsWith("_wood")) && !id.startsWith("stripped_");
    }

    /** Fells the tree over a column: its trunk above the ground and its crown within 3 of the trunk. Blocks taken. */
    private static int fell(ServerLevel level, int x, int ground, int z) {
        int taken = 0;
        ArrayDeque<BlockPos> queue = new ArrayDeque<>();
        Set<BlockPos> seen = new HashSet<>();
        for (int y = ground + 1; y < ground + 32; y++) {
            BlockPos at = new BlockPos(x, y, z);
            BlockState s = level.getBlockState(at);
            if (isTree(s)) {
                queue.add(at);
            }
        }
        while (!queue.isEmpty()) {
            BlockPos at = queue.pop();
            if (!seen.add(at) || Math.max(Math.abs(at.getX() - x), Math.abs(at.getZ() - z)) > 3) {
                continue;
            }
            BlockState s = level.getBlockState(at);
            if (!isTree(s) && !s.is(Blocks.VINE)) {
                continue;
            }
            level.setBlock(at, Blocks.AIR.defaultBlockState(), 2 | 16);
            taken++;
            for (int dx = -1; dx <= 1; dx++) {
                for (int dy = -1; dy <= 1; dy++) {
                    for (int dz = -1; dz <= 1; dz++) {
                        if (dx != 0 || dy != 0 || dz != 0) {
                            queue.add(at.offset(dx, dy, dz));
                        }
                    }
                }
            }
        }
        return taken;
    }

    /** A kingdom's street paving: Lordaeron's a beaten path, the cities' dressed stone. */
    static BlockState paving(String kingdom) {
        return switch (kingdom) {
            case "dalaran" -> Blocks.POLISHED_ANDESITE.defaultBlockState();
            case "stormwind" -> Blocks.STONE_BRICKS.defaultBlockState();
            default -> Blocks.DIRT_PATH.defaultBlockState();
        };
    }

    /** The half step a kingdom's street climbs by. */
    static BlockState halfStep(String kingdom) {
        BlockState s = switch (kingdom) {
            case "dalaran" -> Blocks.POLISHED_ANDESITE_SLAB.defaultBlockState();
            case "stormwind" -> Blocks.STONE_BRICK_SLAB.defaultBlockState();
            default -> Blocks.COBBLESTONE_SLAB.defaultBlockState();
        };
        return s.setValue(SlabBlock.TYPE, SlabType.BOTTOM);
    }
}
