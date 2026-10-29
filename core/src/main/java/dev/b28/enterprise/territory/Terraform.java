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
            int natural, int crossings, int built, int landings) {}

    /**
     * Shapes the town's ground round its plan, if all of it is loaded (empty if not: ask again).
     */
    public static Optional<Shaped> shape(ServerLevel level, TownPlans.Plan plan, String kingdom) {
        return shape(level, plan, kingdom, List.of());
    }

    /**
     * As the other; {@code others}: the other towns' bells, so an avenue heading toward one (its bearing within 30°)
     * serves the road there, and keeps its crossing.
     */
    public static Optional<Shaped> shape(ServerLevel level, TownPlans.Plan plan, String kingdom, List<BlockPos> others) {
        return shape(level, plan, kingdom, others, Optional.empty());
    }

    /**
     * As the other; {@code main}: a founded town's main street (the studio's, thread 0012): it and the forum paved and
     * its steps walled; every other street a dirt path on its graded line, every other step a cut face, till its plots
     * are built.
     */
    public static Optional<Shaped> shape(ServerLevel level, TownPlans.Plan plan, String kingdom, List<BlockPos> others,
            Optional<net.minecraft.core.Direction> main) {
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
        List<int[]> mainPlots = new ArrayList<>();
        for (TownPlans.Frontage p : TownPlans.plots(plan, Plots.width(10))) {
            if (main.isPresent() && TownPlans.onMain(plan, p, main.get())) {
                mainPlots.add(new int[] {p.x() - x0, p.z() - z0, p.sx(), p.sz()});
            }
            int door = switch (p.door()) {
                case NORTH -> 0;
                case SOUTH -> 1;
                case EAST -> 2;
                default -> 3;
            };
            plots.add(new Grading.Plot(p.x() - x0, p.z() - z0, p.sx(), p.sz(), door));
        }
        // The roads out: an avenue's end heading toward another town (the studio's `serves`, thread 0014).
        List<int[]> serves = new ArrayList<>();
        for (Grading.Street st : streets) {
            if (!st.kind().equals("avenue")) {
                continue;
            }
            int[] end = st.line()[1];
            double heading = Math.atan2(end[1] - bz, end[0] - bx);
            for (BlockPos other : others) {
                double toward = Math.atan2(other.getZ() - plan.bell().getZ(), other.getX() - plan.bell().getX());
                double off = Math.abs(Math.atan2(Math.sin(heading - toward), Math.cos(heading - toward)));
                if (off <= Math.toRadians(30)) {
                    serves.add(end);
                    break;
                }
            }
        }
        int forumLevel = plan.bell().getY() - 1;
        java.util.function.IntUnaryOperator deckOver = span -> Bridges.over(kingdom, span);
        var first = Grading.grade(X, Z, top, water, fixed, streets, plots, bx, bz, Forums.REACH, forumLevel, o, BLEND, serves,
                deckOver);
        // No bridge to nowhere (John): a crossing is built only where the street serves something past it, and no wider
        // than 40 (a ferry's, not yet); the street is cut back to its near bank, and graded again.
        java.util.function.Predicate<Grading.Crossing> build = c -> c.keep() && !c.kind().equals("ferry");
        List<Grading.Crossing> dropped = first.crossings().stream().filter(c -> !build.test(c) && c.nearBank() != null).toList();
        List<Grading.Street> graded = dropped.isEmpty() ? streets : Grading.trim(streets, first.crossings(), build);
        var g = dropped.isEmpty() ? first
                : Grading.grade(X, Z, top, water, fixed, graded, plots, bx, bz, Forums.REACH, forumLevel, o, BLEND, serves,
                        deckOver);
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

        // A founded town's grade applied only where it's built (the studio's r77 render of town 5: every plot levelled
        // at founding read as a survey drawn in stone): its forum, its main street and the plots on it. Its other plots
        // keep their land as it lies, its other streets are dirt paths on the land's own lie, till they open.
        boolean[] built = null;
        if (main.isPresent()) {
            built = new boolean[N];
            for (int z = 0; z < Z; z++) {
                for (int x = 0; x < X; x++) {
                    int i = z * X + x;
                    built[i] = role[i] == Grading.FORUM || role[i] == Grading.STREET && nearMainStreet(main.get(), bx, bz, x, z);
                }
            }
            // The main street's plots: the whole band a plot's depth either side of it, whichever way its plots are cut
            // (r79: the grade's 14-wide pads and a workshop's 12-wide plot straddled two levels, too steep to build).
            int reach = TownPlans.GATE / 2 + 2 + plan.deepest();
            for (int z = 0; z < Z; z++) {
                for (int x = 0; x < X; x++) {
                    boolean band = switch (main.get()) {
                        case NORTH -> Math.abs(x - bx) <= reach && z < bz;
                        case SOUTH -> Math.abs(x - bx) <= reach && z > bz;
                        case WEST -> Math.abs(z - bz) <= reach && x < bx;
                        default -> Math.abs(z - bz) <= reach && x > bx;
                    };
                    int i = z * X + x;
                    if (band && (role[i] == Grading.PLOT || role[i] == Grading.FREE)) {
                        built[i] = true;
                    }
                }
            }
            for (int i = 0; i < N; i++) {
                if (built[i] || role[i] == Grading.WATER) {
                    continue;
                }
                if (role[i] == Grading.STREET && g.bridge()[i] == 0) {
                    // The street's line as a worn track on the land as it lies.
                    BlockPos at = new BlockPos(x0 + i % X, top[i], z0 + i / X);
                    BlockState was = level.getBlockState(at);
                    if (was.is(Blocks.GRASS_BLOCK) || was.is(Blocks.DIRT) || was.is(Blocks.COARSE_DIRT) || was.is(Blocks.PODZOL)) {
                        level.setBlock(at, Blocks.DIRT_PATH.defaultBlockState(), 2 | 16);
                        BlockState over = level.getBlockState(at.above());
                        if (!over.isAir() && over.canBeReplaced() && over.getFluidState().isEmpty()) {
                            level.setBlock(at.above(), Blocks.AIR.defaultBlockState(), 2 | 16);
                        }
                    }
                }
                role[i] = Grading.WILD;
                g.bridge()[i] = 0;
                target[i] = top[i];
            }
        }
        final boolean[] applied = built;
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
        // A founded town's main street, by column: on it ({@code near} 0) or within 4 of it.
        java.util.function.BiPredicate<Integer, Integer> nearMain = (x, z) -> main.isEmpty() || switch (main.get()) {
            case NORTH -> Math.abs(x - bx) <= TownPlans.GATE / 2 + 4 && z < bz;
            case SOUTH -> Math.abs(x - bx) <= TownPlans.GATE / 2 + 4 && z > bz;
            case WEST -> Math.abs(z - bz) <= TownPlans.GATE / 2 + 4 && x < bx;
            default -> Math.abs(z - bz) <= TownPlans.GATE / 2 + 4 && x > bx;
        };
        BlockState track = Blocks.DIRT_PATH.defaultBlockState();
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
                level.setBlock(new BlockPos(wx, t, wz), r == Grading.FORUM ? surface(kingdom, wx, wz, false, true)
                        : r == Grading.STREET && nearMain.test(x, z) ? surface(kingdom, wx, wz, kerb(role, X, Z, x, z), true)
                        : r == Grading.STREET ? track : grass, 2 | 16);
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
        // Each crossing built, assembled from the kingdom's bridge parts (`Bridges`, the studio's bridges.mjs): abutments,
        // arches on piers, the channel arch high enough for a boat; a causeway with a culvert for 3 or less.
        for (Grading.Crossing c : g.crossings()) {
            if (build.test(c) && c.nearBank() != null && c.facing() >= 0
                    && (applied == null || applied[c.nearBank()[1] * X + c.nearBank()[0]])) {
                Bridges.lay(level, kingdom, c, x0 + c.nearBank()[0], z0 + c.nearBank()[1], graded.get(c.street()).width(), pave);
            }
        }
        // Where a street now ends at the shore, a landing: steps down to the water, a jetty, lanterns (the studio's
        // landingBlocks).
        int landings = 0;
        for (Grading.Crossing c : dropped) {
            // A dropped crossing of 3 or less gets no jetty: the street just ends at the water (the studio's rule).
            if (c.facing() >= 0 && c.span() > 3 && (applied == null || applied[c.nearBank()[1] * X + c.nearBank()[0]])) {
                int i = c.nearBank()[1] * X + c.nearBank()[0];
                int width = streets.get(c.street()).width();
                landing(level, kingdom, x0 + c.nearBank()[0], z0 + c.nearBank()[1], c.facing(), target[i], c.water(), width, pave);
                // A crossing the street would have served but too long to bridge yet: a ferry's stage beyond the steps.
                if (c.keep() && c.kind().equals("ferry")) {
                    ferryStage(level, x0 + c.nearBank()[0], z0 + c.nearBank()[1], c.facing(), target[i], c.water(), width);
                }
                landings++;
            }
        }
        // The kingdom's earthworks tile at every step.
        int steps = 0;
        for (Grading.Edge e : g.edges()) {
            if (e.kind().startsWith("street_") || e.kind().equals("slope")) {
                continue;
            }
            if (applied != null) {
                int[] dd = e.dir() < 4 ? Grading.DIRS[e.dir()] : new int[] {0, 0};
                int hi = e.z() * X + e.x(), xx = e.x() + dd[0], zz = e.z() + dd[1];
                if (!applied[hi] || xx < 0 || zz < 0 || xx >= X || zz >= Z || !applied[zz * X + xx]) {
                    continue; // a step to land not yet built on: its tile when its plot opens
                }
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
            // Walled where the town is built (in a founded town, its main street), a cut face elsewhere till it is.
            boolean walled = !e.kind().equals("cut_face") && nearMain.test(e.x(), e.z());
            // On a founded town's main street, only a step facing across the street is walled; one along it (the graded
            // band's ends, fields beyond) is a cut face (the studio's r80 render: a fortress wall along the band's ends).
            if (walled && main.isPresent() && e.dir() < 4) {
                boolean streetAlongZ = main.get().getAxis() == net.minecraft.core.Direction.Axis.Z;
                boolean faceAcross = streetAlongZ ? e.dir() >= 2 : e.dir() < 2;
                walled = faceAcross;
            }
            if (Earthworks.step(level, kingdom, x0 + e.x(), z0 + e.z(), d[0], d[1], low, e.rise(), walled ? "wall" : "cut_face")) {
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
                level.setBlock(new BlockPos(x0 + x, target[i], z0 + z), nearMain.test(x, z)
                        ? surface(kingdom, x0 + x, z0 + z, kerb(role, X, Z, x, z), true) : track, 2 | 16);
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
                        level.setBlock(new BlockPos(x0 + x, target[i] + 1, z0 + z), halfStep(kingdom, x0 + x, z0 + z), 2 | 16);
                        break;
                    }
                }
            }
        }
        return Optional.of(new Shaped(columns, g.cut(), g.fill(), felled, steps, bridges, g.conflicts(), g.plotsOver(),
                g.natural(), first.crossings().size(), (int) g.crossings().stream().filter(build).count(), landings));
    }

    /** Is the column (in the box's frame, the bell at bx, bz) on the main street {@code main}, or within 4 of it? */
    static boolean nearMainStreet(net.minecraft.core.Direction main, int bx, int bz, int x, int z) {
        return switch (main) {
            case NORTH -> Math.abs(x - bx) <= TownPlans.GATE / 2 + 4 && z < bz;
            case SOUTH -> Math.abs(x - bx) <= TownPlans.GATE / 2 + 4 && z > bz;
            case WEST -> Math.abs(z - bz) <= TownPlans.GATE / 2 + 4 && x < bx;
            default -> Math.abs(z - bz) <= TownPlans.GATE / 2 + 4 && x > bx;
        };
    }

    /**
     * A founded town's street opened (its first plot built: the studio's, thread 0012): its worn track inside the inner
     * wall paved in the kingdom's way, where it's still a track. The paved blocks.
     */
    public static int openStreet(ServerLevel level, TownPlans.Plan plan, String kingdom, String street) {
        boolean northSouth = street.startsWith("x=");
        int line = Integer.parseInt(street.substring(2));
        boolean avenue = line == (northSouth ? plan.bell().getX() : plan.bell().getZ());
        int width = avenue ? TownPlans.GATE : TownPlans.LANE, from = line - width / 2;
        TownPlans.Rect in = plan.inner();
        int paved = 0;
        for (int along = northSouth ? in.z0() : in.x0(); along <= (northSouth ? in.z1() : in.x1()); along++) {
            for (int o = 0; o < width; o++) {
                int x = northSouth ? from + o : along, z = northSouth ? along : from + o;
                if (!level.isLoaded(new BlockPos(x, 0, z))) {
                    continue;
                }
                BlockPos at = new BlockPos(x, ground(level, x, z), z);
                BlockState surface = surface(kingdom, x, z, o == 0 || o == width - 1,
                        !level.getBlockState(at.below()).isAir() && level.getBlockState(at.below()).getFluidState().isEmpty());
                if (level.getBlockState(at).is(Blocks.DIRT_PATH) && !surface.is(Blocks.DIRT_PATH)) {
                    level.setBlock(at, surface, 2 | 16);
                    paved++;
                }
            }
        }
        return paved;
    }

    /** How far a landing's jetty runs out over the water. */
    static final int JETTY = 4;

    /**
     * A street's end at the shore (the studio's landingBlocks): from its last cell, the way out over the water, half
     * steps down from the street's level to a block over the water on coursed stone, the street's width, then a timber
     * jetty on posts; a lantern on a post either side at the head of the steps.
     */
    static void landing(ServerLevel level, String kingdom, int x0, int z0, int facing, int streetLevel, int water, int width,
            BlockState pave) {
        int[] f = Grading.DIRS[facing];
        int fx = f[0], fz = f[1], sx = fz, sz = fx, half = (width - 1) / 2;
        int deck = water + 1, k = 1;
        BlockState slab = halfStep(kingdom), course = Blocks.STONE_BRICKS.defaultBlockState();
        for (double walk = streetLevel + 0.5; walk > deck + 1; walk -= 0.5, k++) {
            boolean whole = walk == Math.floor(walk);
            int y = whole ? (int) walk - 1 : (int) Math.floor(walk);
            for (int o = -half; o <= width - 1 - half; o++) {
                int x = x0 + fx * k + sx * o, z = z0 + fz * k + sz * o;
                for (int yy = y + 1; yy <= streetLevel + 3; yy++) {
                    level.setBlock(new BlockPos(x, yy, z), Blocks.AIR.defaultBlockState(), 2 | 16);
                }
                for (int yy = y - 1; yy >= water - 2; yy--) {
                    level.setBlock(new BlockPos(x, yy, z), course, 2 | 16);
                }
                level.setBlock(new BlockPos(x, y, z), whole ? pave : slab, 2 | 16);
            }
        }
        for (int j = 0; j < JETTY; j++) {
            for (int o = -half; o <= width - 1 - half; o++) {
                int x = x0 + fx * (k + j) + sx * o, z = z0 + fz * (k + j) + sz * o;
                level.setBlock(new BlockPos(x, deck, z), Blocks.SPRUCE_PLANKS.defaultBlockState(), 2 | 16);
                if ((j == JETTY - 1 || j % 3 == 0) && (o == -half || o == width - 1 - half)) {
                    for (int y = deck - 1; y >= water - 3; y--) {
                        level.setBlock(new BlockPos(x, y, z), Blocks.SPRUCE_LOG.defaultBlockState(), 2 | 16);
                    }
                }
            }
        }
        for (int o : new int[] {-half - 1, width - half}) {
            int x = x0 + sx * o, z = z0 + sz * o;
            level.setBlock(new BlockPos(x, streetLevel + 1, z), parapet(kingdom, true), 2 | 16);
            level.setBlock(new BlockPos(x, streetLevel + 2, z), parapet(kingdom, true), 2 | 16);
            level.setBlock(new BlockPos(x, streetLevel + 3, z), Blocks.LANTERN.defaultBlockState(), 2 | 16);
        }
    }

    /**
     * A ferry's landing stage beyond a landing's steps (the studio's ferryStageBlocks): a broad timber stage on posts out
     * from the jetty's end, a post and lantern at its corners, a moored skiff alongside.
     */
    static void ferryStage(ServerLevel level, int x0, int z0, int facing, int streetLevel, int water, int width) {
        int[] f = Grading.DIRS[facing];
        int fx = f[0], fz = f[1], sx = fz, sz = fx, half = (width - 1) / 2, deck = water + 1;
        // Past the steps (2 a block of fall) and the jetty.
        int start = 2 * Math.max(0, streetLevel - deck - 1) + 1 + JETTY;
        for (int i = 0; i < 5; i++) {
            for (int o = -half - 1; o <= width - half; o++) {
                int x = x0 + fx * (start + i) + sx * o, z = z0 + fz * (start + i) + sz * o;
                level.setBlock(new BlockPos(x, deck, z), Blocks.SPRUCE_PLANKS.defaultBlockState(), 2 | 16);
                boolean corner = (i == 0 || i == 4) && (o == -half - 1 || o == width - half);
                if (corner) {
                    for (int y = deck - 1; y >= water - 3; y--) {
                        level.setBlock(new BlockPos(x, y, z), Blocks.SPRUCE_LOG.defaultBlockState(), 2 | 16);
                    }
                    if (i == 4) {
                        level.setBlock(new BlockPos(x, deck + 1, z), Blocks.SPRUCE_FENCE.defaultBlockState(), 2 | 16);
                        level.setBlock(new BlockPos(x, deck + 2, z), Blocks.LANTERN.defaultBlockState(), 2 | 16);
                    }
                }
            }
        }
        // The skiff, moored along the stage's side: a hull of slabs and open trapdoors.
        for (int i = 1; i <= 3; i++) {
            int o = width - half + 1;
            int x = x0 + fx * (start + i) + sx * o, z = z0 + fz * (start + i) + sz * o;
            level.setBlock(new BlockPos(x, water, z), Blocks.SPRUCE_SLAB.defaultBlockState()
                    .setValue(net.minecraft.world.level.block.SlabBlock.TYPE, SlabType.TOP)
                    .setValue(net.minecraft.world.level.block.SlabBlock.WATERLOGGED, true), 2 | 16);
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

    /** The chunks shaping would touch: the outer wall and its blend band. */
    public static List<net.minecraft.world.level.ChunkPos> chunks(TownPlans.Plan plan) {
        List<net.minecraft.world.level.ChunkPos> out = new ArrayList<>();
        int x0 = plan.outer().x0() - BLEND - 1, z0 = plan.outer().z0() - BLEND - 1;
        int x1 = plan.outer().x1() + BLEND + 1, z1 = plan.outer().z1() + BLEND + 1;
        for (int cx = x0 >> 4; cx <= x1 >> 4; cx++) {
            for (int cz = z0 >> 4; cz <= z1 >> 4; cz++) {
                out.add(new net.minecraft.world.level.ChunkPos(cx, cz));
            }
        }
        return out;
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

    /**
     * A street's surface at a cell (the studio's, thread 0012): Lordaeron's opened street cobbles with a dressed kerb (its
     * two outer cells stone bricks, a cracked one in 6; between them cobblestone 55%, mossy cobble 15%, gravel 20% (only on
     * solid ground: it falls), andesite 10%, by the cell); the cities' their dressed paving.
     */
    static BlockState surface(String kingdom, int x, int z, boolean kerb, boolean solidBelow) {
        if (!kingdom.equals("lordaeron") && !kingdom.isEmpty() && !kingdom.equals("none")) {
            return paving(kingdom);
        }
        long h = (x * 73856093L) ^ (z * 19349663L);
        int r = (int) Math.floorMod(h * 0x9E3779B97F4A7C15L >>> 40, 100L);
        if (kerb) {
            return r < 17 ? Blocks.CRACKED_STONE_BRICKS.defaultBlockState() : Blocks.STONE_BRICKS.defaultBlockState();
        }
        return r < 55 ? Blocks.COBBLESTONE.defaultBlockState() : r < 70 ? Blocks.MOSSY_COBBLESTONE.defaultBlockState()
                : r < 90 ? (solidBelow ? Blocks.GRAVEL.defaultBlockState() : Blocks.COBBLESTONE.defaultBlockState())
                : Blocks.ANDESITE.defaultBlockState();
    }

    /** Is the street cell a kerb: an edge of its street (a neighbour neither street nor forum)? */
    static boolean kerb(byte[] role, int X, int Z, int x, int z) {
        for (int[] d : Grading.DIRS) {
            int xx = x + d[0], zz = z + d[1];
            if (xx < 0 || zz < 0 || xx >= X || zz >= Z) {
                continue;
            }
            byte r = role[zz * X + xx];
            if (r != Grading.STREET && r != Grading.FORUM) {
                return true;
            }
        }
        return false;
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
    /** The half step a kingdom's street climbs by, at a cell: Lordaeron's cobble slab, a mossy one in 4. */
    static BlockState halfStep(String kingdom, int x, int z) {
        if (kingdom.equals("lordaeron") && Math.floorMod((x * 31L + z * 17L) * 0x9E3779B97F4A7C15L >>> 40, 4L) == 0) {
            return Blocks.MOSSY_COBBLESTONE_SLAB.defaultBlockState().setValue(SlabBlock.TYPE, SlabType.BOTTOM);
        }
        return halfStep(kingdom);
    }

    static BlockState halfStep(String kingdom) {
        BlockState s = switch (kingdom) {
            case "dalaran" -> Blocks.POLISHED_ANDESITE_SLAB.defaultBlockState();
            case "stormwind" -> Blocks.STONE_BRICK_SLAB.defaultBlockState();
            default -> Blocks.COBBLESTONE_SLAB.defaultBlockState();
        };
        return s.setValue(SlabBlock.TYPE, SlabType.BOTTOM);
    }
}
