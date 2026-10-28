package dev.b28.enterprise.territory;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.CropBlock;
import net.minecraft.world.level.block.state.BlockState;

/**
 * A town's farmland laid to its land as bocage (the studio's `js/farmland/layout.mjs`, thread 0011, ported; John: "I had
 * the designer work on an approach"): over the rings round the town's walls, seeds jittered on a {@link #SPACING} grid, and
 * each block joins the seed nearest by a distance that counts height three times over (d² = dx² + dz² + 9·dh², over the
 * land smoothed), so fields stretch along the contours as bocage fields do: no two alike, no ground left over. Each field's
 * use by its rank of steepness, so any land gets the mix: the steepest woodland, then pasture, then orchard where it faces
 * the sun (terraces or pasture where it doesn't), the valley floor by water meadow, the rest arable, its crops in bands of
 * {@link #BAND} blocks of height so the strips bend with the land. Hedgerows (an oak or birch now and then) round each
 * field, dry-stone walls on the high ground and round terraces, a gate where a field meets a track.
 *
 * <p>Every crop is sown and counted (John: "sow everything, count it"), its water a source every 9 blocks each way where
 * nothing lower stands beside it (so it can't run off down the hill; farmland a step below still drinks from it).
 */
public final class Bocage {
    private Bocage() {}

    /** The seeds' grid; a crop band's height; the fields laid a look. */
    static final int SPACING = 26, BAND = 3, EACH = 12;
    /** How far past the inner wall the farmland is looked for; how dear a reach it may take (a flat field's walk). */
    static final int SPREAD = 160;
    static final float BUDGET = 140;
    /** The share of the reach past which the land goes wild: pasture, meadow and wood, not crops. */
    static final float WILD = 0.8f;

    /** A town's bocage: the ground it covers, each block's field (-1 none), the smoothed land, the fields. */
    static final class Land {
        final int x0, z0, sx, sz;
        final int[] field;
        final float[] h;

        Land(int x0, int z0, int sx, int sz, int[] field, float[] h) {
            this.x0 = x0;
            this.z0 = z0;
            this.sx = sx;
            this.sz = sz;
            this.field = field;
            this.h = h;
        }

        int at(int x, int z) {
            int i = x - x0, j = z - z0;
            return i < 0 || j < 0 || i >= sx || j >= sz ? -1 : field[j * sx + i];
        }

        float height(int x, int z) {
            int i = Math.clamp(x - x0, 0, sx - 1), j = Math.clamp(z - z0, 0, sz - 1);
            return h[j * sx + i];
        }
    }

    /**
     * Lays the town's land out (not yet sown): its fields, their uses and crops. Null while its ground isn't loaded. The
     * fields go into {@code m.fields} (a field's {@code holding} its index there).
     */
    static Land layOut(ServerLevel level, TownPlans.Plan plan, FieldMaps.TownMap m, Map<Integer, int[]> lots, long town) {
        // The ground the farmland may reach: round the inner wall, as far as it could spread along a road.
        TownPlans.Rect in = plan.inner(), out = plan.outer();
        int x0 = in.x0() - SPREAD, z0 = in.z0() - SPREAD, sx = in.x1() - in.x0() + 2 * SPREAD + 1, sz = in.z1() - in.z0() + 2 * SPREAD + 1;
        for (int[] c : new int[][] {{in.x0(), in.z0()}, {in.x1(), in.z1()}}) {
            if (!level.isLoaded(new BlockPos(c[0], 0, c[1]))) {
                return null;
            }
        }
        int n = sx * sz;
        float[] top = new float[n];
        boolean[] wet = new boolean[n], ok = new boolean[n], road = new boolean[n], wood = new boolean[n], gone = new boolean[n];
        // The tracks and roads drawn on the grid once (a block's distance to every line, block by block, held a tick for
        // a minute): the farm tracks and the trade routes out of the gates.
        List<double[][]>[] paths = Furlongs.paths(plan);
        for (FieldMaps.Track t : m.tracks) {
            stamp(road, sx, sz, x0, z0, t.line(), FieldMaps.TRACK_WIDTH / 2.0 + 1);
        }
        for (List<double[][]> ring : paths) {
            for (double[][] line : ring) {
                stamp(road, sx, sz, x0, z0, line, TownPlans.GATE / 2.0 + 2);
            }
        }
        int band = TownPlans.GATE / 2 + 2 + plan.deepest() + 1;
        for (int j = 0; j < sz; j++) {
            for (int i = 0; i < sx; i++) {
                int x = x0 + i, z = z0 + j, k = j * sx + i;
                if (!level.isLoaded(new BlockPos(x, 0, z))) {
                    gone[k] = true; // not loaded: the farmland doesn't reach it (this time)
                    continue;
                }
                int y = Earthworks.height(level, x, z);
                if (y == Integer.MIN_VALUE) {
                    gone[k] = true;
                    continue;
                }
                top[k] = y;
                BlockPos at = new BlockPos(x, y, z);
                wet[k] = !level.getBlockState(at).getFluidState().isEmpty();
                wood[k] = level.getHeight(net.minecraft.world.level.levelgen.Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, x, z) - 1 > y;
                boolean inside = in.contains(x, z) || in.fromEdge(x, z) <= TownPlans.STRIP + 1;
                boolean wall = !out.contains(x, z) && out.fromEdge(x, z) <= TownPlans.STRIP + 1
                        || out.contains(x, z) && out.fromEdge(x, z) <= TownPlans.STRIP + 1;
                // The suburbs along the avenues between the walls are the town's plots, not fields.
                boolean suburb = out.contains(x, z) && (Math.abs(x - plan.bell().getX()) <= band || Math.abs(z - plan.bell().getZ()) <= band);
                boolean lot = false;
                for (int[] l : lots.values()) {
                    lot |= x >= l[0] - 1 && x <= l[2] + 1 && z >= l[1] - 1 && z <= l[3] + 1;
                }
                ok[k] = !inside && !wall && !wet[k] && !road[k] && !suburb && !lot;
            }
        }
        // How dear each block is to reach from the town's gates: a step's climb dear (1 + 40·climb², the studio's lanes),
        // woods dearer, water out of reach, the roads and tracks cheap. The farmland is what's within reach: it runs out
        // along the trade routes and stops where the land does, not at a line.
        float[] hs = blur(blur(top, sx, sz, 2), sx, sz, 2);
        float[] cost = reach(plan, in, hs, wet, gone, road, wood, x0, z0, sx, sz);
        for (int k = 0; k < n; k++) {
            ok[k] &= cost[k] <= BUDGET;
        }
        // Seeds jittered on the grid, at their land's height.
        List<float[]> seeds = new ArrayList<>();
        int gx = (sx + SPACING - 1) / SPACING, gz = (sz + SPACING - 1) / SPACING;
        for (int b = 0; b < gz; b++) {
            for (int a = 0; a < gx; a++) {
                long hh = Zoning.mix(town, a * 7919L + b);
                int i = Math.min(sx - 1, a * SPACING + (int) Math.floorMod(hh, (long) SPACING));
                int j = Math.min(sz - 1, b * SPACING + (int) Math.floorMod(hh >>> 20, (long) SPACING));
                // Past 60% of the reach, every other seed dropped: bigger, quieter fields toward the farmland's edge (the
                // studio, r60: "lots of small parcels at the ends read busy").
                boolean drop = cost[j * sx + i] > 0.6f * BUDGET && (a + b) % 2 == 1;
                seeds.add(new float[] {i, j, hs[j * sx + i], drop ? 1 : 0});
            }
        }
        // Each block to the seed nearest by d² = dx² + dz² + 9·dh² (the seeds of the grid cells round it).
        int[] field = new int[n];
        java.util.Arrays.fill(field, -1);
        for (int j = 0; j < sz; j++) {
            for (int i = 0; i < sx; i++) {
                if (!ok[j * sx + i]) {
                    continue;
                }
                float h = hs[j * sx + i];
                double best = Double.MAX_VALUE;
                int k = -1, ca = i / SPACING, cb = j / SPACING;
                for (int b = Math.max(0, cb - 2); b <= Math.min(gz - 1, cb + 2); b++) {
                    for (int a = Math.max(0, ca - 2); a <= Math.min(gx - 1, ca + 2); a++) {
                        float[] s = seeds.get(b * gx + a);
                        if (s[3] == 1) {
                            continue;
                        }
                        double d = (i - s[0]) * (i - s[0]) + (j - s[1]) * (j - s[1]) + 9 * (h - s[2]) * (h - s[2]);
                        if (d < best) {
                            best = d;
                            k = b * gx + a;
                        }
                    }
                }
                field[j * sx + i] = k;
            }
        }
        // Each field's land: its blocks, slope, how it faces the sun, height, distance to water, bounds.
        int[] dWater = waterDistance(wet, sx, sz);
        Map<Integer, double[]> stats = new HashMap<>(); // n, slope, south, h, dw, x0, z0, x1, z1, lo, hi
        for (int j = 0; j < sz; j++) {
            for (int i = 0; i < sx; i++) {
                int f = field[j * sx + i];
                if (f < 0) {
                    continue;
                }
                double[] s = stats.computeIfAbsent(f, q -> new double[] {0, 0, 0, 0, 0, 1e9, 1e9, -1e9, -1e9, 1e9, -1e9, 0});
                float east = hs[j * sx + Math.min(sx - 1, i + 1)] - hs[j * sx + Math.max(0, i - 1)];
                float south = hs[Math.min(sz - 1, j + 1) * sx + i] - hs[Math.max(0, j - 1) * sx + i];
                s[0]++;
                s[1] += Math.hypot(east, south) / 2;
                s[2] += -south / 2;
                s[3] += hs[j * sx + i];
                s[4] += Math.min(dWater[j * sx + i], 60);
                s[5] = Math.min(s[5], x0 + i);
                s[6] = Math.min(s[6], z0 + j);
                s[7] = Math.max(s[7], x0 + i);
                s[8] = Math.max(s[8], z0 + j);
                s[9] = Math.min(s[9], top[j * sx + i]);
                s[10] = Math.max(s[10], top[j * sx + i]);
                s[11] += cost[j * sx + i];
            }
        }
        double hMin = Double.MAX_VALUE, hMax = -Double.MAX_VALUE;
        for (float v : hs) {
            hMin = Math.min(hMin, v);
            hMax = Math.max(hMax, v);
        }
        // Uses by rank of steepness, so any land gets its woods and pastures.
        List<Integer> order = new ArrayList<>(stats.keySet());
        order.removeIf(f -> stats.get(f)[0] < Furlongs.LEAST);
        order.sort(Comparator.comparingDouble((Integer f) -> -stats.get(f)[1] / stats.get(f)[0]));
        Map<Integer, Integer> index = new HashMap<>();
        m.fields.clear();
        for (int r = 0; r < order.size(); r++) {
            int f = order.get(r);
            double[] s = stats.get(f);
            double q = r / (double) order.size(), so = s[2] / s[0], dw = s[4] / s[0];
            double rel = (s[3] / s[0] - hMin) / Math.max(1, hMax - hMin);
            long hh = Zoning.mix(town, f * 31L + 9);
            String use;
            double far = s[11] / s[0] / BUDGET;
            if (far <= WILD && (use0(q) == 1 && q >= 0.04 || use0(q) == 2 && q >= 0.10)) {
                // Within the worked reach the land's ploughed: what rank would leave to wood or grazing is arable, but the
                // steepest (the studio, r60: planted share 0.24 against their 0.53).
                use = "arable:" + crops(hh, s[10] - s[9]);
            } else if (far > WILD) {
                // The edge of the farmland gone wild: rough pasture, meadow and wood, the bocage fading into the land.
                use = Math.floorMod(hh, 10L) < 4 ? "woodland" : Math.floorMod(hh, 10L) < 7 ? "pasture" : "meadow";
            } else if (dw < 10 && q > 0.5) {
                use = "meadow";
            } else if (q < 0.08) {
                use = "woodland";
            } else if (q < 0.22) {
                use = "pasture";
            } else if (q < 0.45) {
                use = so > 0 ? "orchard" : Math.floorMod(hh, 2L) == 0 ? "terraces" : "pasture";
            } else {
                use = rel > 0.75 && Math.floorMod(hh, 10L) < 3 ? "pasture" : "arable:" + crops(hh, s[10] - s[9]);
            }
            Furlongs.Field fl = new Furlongs.Field(m.fields.size(), 0, 0, (int) s[5], (int) s[6], (int) s[7], (int) s[8], use,
                    (int) (s[10] - s[9]));
            fl.area = (int) s[0];
            // Walls on the high ground only (the top quarter of the land's height); bands at least 7 blocks across on
            // the ground, whatever the slope (the studio, r58: 3 blocks of height was 4 or 5 across on a steep bank).
            fl.high = rel > 0.75;
            fl.band = Math.max(BAND, (int) Math.ceil(7 * s[1] / s[0]));
            index.put(f, m.fields.size());
            m.fields.add(fl);
        }
        // Each block's field by its place in m.fields; the small pieces nobody's (the hedge's).
        for (int i = 0; i < n; i++) {
            field[i] = field[i] < 0 ? -1 : index.getOrDefault(field[i], -1);
        }
        return new Land(x0, z0, sx, sz, field, hs);
    }

    /** The use the rank alone gives: 1 woodland, 2 pasture, 0 else. */
    private static int use0(double q) {
        return q < 0.08 ? 1 : q < 0.22 ? 2 : 0;
    }

    /**
     * An arable field's crops, two to four, alternating band by band so the contours show: wheat and a root crop; on a
     * hill (relief over {@link Furlongs#FLAT}) only what grows on hills, wheat and potatoes.
     */
    static String crops(long h, double relief) {
        String[] roots = relief > Furlongs.FLAT ? new String[] {"potato"} : new String[] {"carrot", "potato", "beetroot"};
        String second = roots[(int) Math.floorMod(h >>> 4, (long) roots.length)];
        if (Math.floorMod(h >>> 9, 100L) < 35) {
            String third = roots[(int) Math.floorMod(h >>> 13, (long) roots.length)];
            return "wheat," + second + ",wheat," + third;
        }
        return "wheat," + second;
    }

    /**
     * Sows and dresses up to {@link #EACH} of the town's fields not yet laid, then their edges: how many it laid. Fields
     * {@code from} on.
     */
    static int lay(ServerLevel level, Land land, FieldMaps.TownMap m, long town, int from) {
        return lay(level, land, m, town, from, EACH);
    }

    /** As {@link #lay(ServerLevel, Land, FieldMaps.TownMap, long, int)}, up to {@code most} fields. */
    static int lay(ServerLevel level, Land land, FieldMaps.TownMap m, long town, int from, int most) {
        int laid = 0;
        for (int f = from; f >= 0 && f < m.fields.size() && laid < most; f++, laid++) {
            Furlongs.Field fl = m.fields.get(f);
            for (int x = fl.x0; x <= fl.x1; x++) {
                for (int z = fl.z0; z <= fl.z1; z++) {
                    if (land.at(x, z) == f) {
                        cell(level, land, fl, f, x, z);
                    }
                }
            }
        }
        return laid;
    }

    /** One block of a field: its edge, or its use. */
    static void cell(ServerLevel level, Land land, Furlongs.Field fl, int f, int x, int z) {
        int y = Earthworks.height(level, x, z);
        if (y == Integer.MIN_VALUE || !level.getBlockState(new BlockPos(x, y, z)).getFluidState().isEmpty()) {
            return;
        }
        // Its edge: where it meets another field (each line once, on the lower field's side), the end of the land, or a
        // track (a gate there, now and then: the field's way in).
        int edge = 0; // 0 none, 1 hedge, 2 gate
        for (int[] d : new int[][] {{1, 0}, {-1, 0}, {0, 1}, {0, -1}}) {
            int o = land.at(x + d[0], z + d[1]);
            if (o == -1 || o > f) {
                edge = 1;
                if (o == -1 && Math.floorMod(Zoning.mix(x * 31L, z), 23L) == 0) {
                    edge = 2;
                }
                break;
            }
        }
        long h = Zoning.mix(x * 31L, z);
        Furlongs.fell(level, x, y, z);
        BlockPos ground = new BlockPos(x, y, z), over = ground.above();
        if (dev.b28.enterprise.settlement.Grounds.natural(level.getBlockState(ground))) {
            level.setBlock(ground, Blocks.GRASS_BLOCK.defaultBlockState(), 2);
        }
        String use = fl.state;
        if (edge == 2) {
            level.setBlock(over, Blocks.OAK_FENCE_GATE.defaultBlockState(), 2);
            return;
        }
        if (edge == 1) {
            // Dry-stone walls on the high ground and round terraces; elsewhere a hedgerow, a tree in it about every 70
            // blocks, a second layer of leaves on a quarter of it (the studio's layout).
            if (fl.high || use.equals("terraces")) {
                level.setBlock(over, Blocks.MOSSY_COBBLESTONE_WALL.defaultBlockState(), 3);
            } else if (Math.floorMod(h, 70L) == 0) {
                Furlongs.tree(level, over, h);
            } else {
                BlockState leaf = Furlongs.hedge(h);
                level.setBlock(over, leaf, 2);
                if (Math.floorMod(h >>> 7, 4L) == 0) {
                    level.setBlock(over.above(), leaf, 2);
                }
            }
            return;
        }
        switch (use.contains(":") ? use.substring(0, use.indexOf(':')) : use) {
            case "woodland" -> {
                if (Math.floorMod(h, 9L) == 0) {
                    Furlongs.tree(level, over, h);
                } else if (Math.floorMod(h, 3L) == 0) {
                    level.setBlock(over, Blocks.FERN.defaultBlockState(), 2);
                }
            }
            case "pasture" -> {
                // Grazing: grass, a hay bale now and then, and a shelter with hay and a trough in its middle.
                if (x == (fl.x0 + fl.x1) / 2 && z == (fl.z0 + fl.z1) / 2) {
                    shelter(level, over);
                } else if (Math.floorMod(h, 97L) == 0) {
                    level.setBlock(over, Blocks.HAY_BLOCK.defaultBlockState(), 2);
                } else if (Math.floorMod(h, 3L) == 0) {
                    level.setBlock(over, Blocks.SHORT_GRASS.defaultBlockState(), 2);
                }
            }
            case "meadow" -> {
                long r = Math.floorMod(h, 20L);
                BlockState flower = r == 0 ? Blocks.CORNFLOWER.defaultBlockState() : r == 1 ? Blocks.OXEYE_DAISY.defaultBlockState()
                        : r == 2 ? Blocks.POPPY.defaultBlockState() : r < 12 ? Blocks.SHORT_GRASS.defaultBlockState()
                        : Blocks.AIR.defaultBlockState();
                level.setBlock(over, flower, 2);
            }
            case "orchard" -> {
                // Trees in rows along the contours: on a row's height, every fifth block along it.
                int band = (int) Math.floor(land.height(x, z));
                if (band % 4 == 0 && Math.floorMod(x + z, 5) == 0) {
                    Furlongs.tree(level, over, h);
                } else if (Math.floorMod(h, 4L) == 0) {
                    level.setBlock(over, Blocks.SHORT_GRASS.defaultBlockState(), 2);
                }
            }
            case "terraces" -> {
                // Benches two blocks high, a mossy stone riser where the next bench down begins; wheat on the benches.
                int bench = Math.floorDiv(y, 2) * 2;
                Plots.level(level, x, z, x, z, bench);
                boolean riser = false;
                for (int[] d : new int[][] {{1, 0}, {-1, 0}, {0, 1}, {0, -1}}) {
                    int ny = Earthworks.height(level, x + d[0], z + d[1]);
                    riser |= ny != Integer.MIN_VALUE && Math.floorDiv(ny, 2) * 2 < bench;
                }
                BlockPos b = new BlockPos(x, bench, z);
                if (riser) {
                    level.setBlock(b, Blocks.MOSSY_STONE_BRICKS.defaultBlockState(), 2);
                } else {
                    sow(level, b, x, z, "wheat", h, fl);
                }
            }
            default -> {
                // Arable: its crops by band of height, a grass balk where one band meets the next.
                String[] crops = use.substring(use.indexOf(':') + 1).split(",");
                int band = (int) Math.floor(land.height(x, z) / fl.band);
                boolean balk = false;
                for (int[] d : new int[][] {{1, 0}, {0, 1}}) {
                    if (land.at(x + d[0], z + d[1]) == f && (int) Math.floor(land.height(x + d[0], z + d[1]) / fl.band) > band) {
                        balk = true;
                    }
                }
                if (balk) {
                    level.setBlock(over, Blocks.SHORT_GRASS.defaultBlockState(), 2);
                } else {
                    sow(level, ground, x, z, crops[Math.floorMod(band, crops.length)], h, fl);
                }
            }
        }
    }

    /**
     * A block sown with a crop on farmland at its own height, or a water source where the 9-block grid puts one and nothing
     * lower stands beside it (it can't run off; each crop is within 4 of one, a step below drinking from it too).
     */
    static void sow(ServerLevel level, BlockPos ground, int x, int z, String crop, long h, Furlongs.Field fl) {
        if (Math.floorMod(x, Furlongs.CHANNEL) == Furlongs.CHANNEL / 2 && Math.floorMod(z, Furlongs.CHANNEL) == Furlongs.CHANNEL / 2) {
            boolean spills = false;
            for (int[] d : new int[][] {{1, 0}, {-1, 0}, {0, 1}, {0, -1}}) {
                int ny = Earthworks.height(level, x + d[0], z + d[1]);
                spills |= ny != Integer.MIN_VALUE && ny < ground.getY();
            }
            if (!spills) {
                level.setBlock(ground, Blocks.WATER.defaultBlockState(), 2);
                level.setBlock(ground.above(), Blocks.AIR.defaultBlockState(), 2);
                return;
            }
        }
        CropBlock block = (CropBlock) switch (crop) {
            case "carrot" -> Blocks.CARROTS;
            case "potato" -> Blocks.POTATOES;
            case "beetroot" -> Blocks.BEETROOTS;
            default -> Blocks.WHEAT;
        };
        level.setBlock(ground, Blocks.FARMLAND.defaultBlockState().setValue(net.minecraft.world.level.block.FarmBlock.MOISTURE, 7), 2);
        int ripe = block.getMaxAge();
        int behind = (int) Math.floorMod(Zoning.mix(fl.x0, fl.z0), 3L);
        level.setBlock(ground.above(), block.getStateForAge(Math.max(0, ripe - behind - (Math.floorMod(h, 7L) == 0 ? 2 : 0))), 2);
    }

    /**
     * Each block's cost to reach from the inner wall's gates (where the avenues leave it), stepping eight ways: a step's
     * length times 1 + 40·climb² (climbs dear, so the reach runs round hills), half again in woods, a road or track a
     * quarter; water and ground not loaded out of reach.
     */
    static float[] reach(TownPlans.Plan plan, TownPlans.Rect in, float[] top, boolean[] wet, boolean[] gone, boolean[] road,
            boolean[] wood, int x0, int z0, int sx, int sz) {
        float[] cost = new float[sx * sz];
        java.util.Arrays.fill(cost, Float.MAX_VALUE);
        java.util.PriorityQueue<long[]> heap = new java.util.PriorityQueue<>(Comparator.comparingLong(e -> e[0]));
        int bx = plan.bell().getX(), bz = plan.bell().getZ(), out = TownPlans.STRIP + 2;
        for (int[] g : new int[][] {{bx, in.z0() - out}, {bx, in.z1() + out}, {in.x0() - out, bz}, {in.x1() + out, bz}}) {
            int i = g[0] - x0, j = g[1] - z0;
            if (i >= 0 && j >= 0 && i < sx && j < sz && !gone[j * sx + i]) {
                cost[j * sx + i] = 0;
                heap.add(new long[] {0, j * sx + i});
            }
        }
        int[][] steps = {{1, 0}, {-1, 0}, {0, 1}, {0, -1}, {1, 1}, {1, -1}, {-1, 1}, {-1, -1}};
        while (!heap.isEmpty()) {
            long[] e = heap.poll();
            int k = (int) e[1];
            float c = cost[k];
            if (e[0] != Float.floatToIntBits(c)) {
                continue; // a dearer way found before the cheaper one
            }
            if (c > BUDGET * 1.5f) {
                break;
            }
            int i = k % sx, j = k / sx;
            for (int[] d : steps) {
                int ii = i + d[0], jj = j + d[1];
                if (ii < 0 || jj < 0 || ii >= sx || jj >= sz) {
                    continue;
                }
                int q = jj * sx + ii;
                if (gone[q] || wet[q] || in.contains(x0 + ii, z0 + jj)) {
                    continue;
                }
                float climb = Math.abs(top[q] - top[k]); // over the land smoothed: a gentle slope climbs a fraction a step
                float step = (d[0] != 0 && d[1] != 0 ? 1.41f : 1f) * (1 + 40 * climb * climb)
                        * (road[q] ? 0.25f : wood[q] ? 1.5f : 1f);
                if (c + step < cost[q]) {
                    cost[q] = c + step;
                    heap.add(new long[] {Float.floatToIntBits(cost[q]), q});
                }
            }
        }
        return cost;
    }

    /** A line drawn on the grid {@code r} wide either side. */
    static void stamp(boolean[] grid, int sx, int sz, int x0, int z0, double[][] line, double r) {
        int ri = (int) Math.ceil(r);
        for (double[] p : FieldMaps.resample(line, 0.5)) {
            int ci = (int) Math.floor(p[0]) - x0, cj = (int) Math.floor(p[1]) - z0;
            for (int dj = -ri; dj <= ri; dj++) {
                for (int di = -ri; di <= ri; di++) {
                    int i = ci + di, j = cj + dj;
                    if (i >= 0 && j >= 0 && i < sx && j < sz && Math.hypot(i + 0.5 - (p[0] - x0), j + 0.5 - (p[1] - z0)) < r) {
                        grid[j * sx + i] = true;
                    }
                }
            }
        }
    }

    /** A field shelter standing at {@code at}: four posts, a slab roof, hay and a trough under it. */
    static void shelter(ServerLevel level, BlockPos at) {
        for (int dx = -1; dx <= 1; dx++) {
            for (int dz = -1; dz <= 1; dz++) {
                for (int up = 0; up <= 2; up++) {
                    level.setBlock(at.offset(dx, up, dz), Blocks.AIR.defaultBlockState(), 2);
                }
                level.setBlock(at.offset(dx, 2, dz), Blocks.SPRUCE_SLAB.defaultBlockState(), 2);
            }
        }
        for (int[] c : new int[][] {{-1, -1}, {-1, 1}, {1, -1}, {1, 1}}) {
            level.setBlock(at.offset(c[0], 0, c[1]), Blocks.SPRUCE_FENCE.defaultBlockState(), 3);
            level.setBlock(at.offset(c[0], 1, c[1]), Blocks.SPRUCE_FENCE.defaultBlockState(), 3);
        }
        level.setBlock(at, Blocks.HAY_BLOCK.defaultBlockState(), 2);
        level.setBlock(at.offset(1, 0, 0), dev.b28.enterprise.settlement.Props.CAULDRON.get().defaultBlockState(), 2);
    }

    /** A grid blurred over a square of radius {@code r}. */
    static float[] blur(float[] a, int sx, int sz, int r) {
        float[] b = new float[a.length];
        for (int j = 0; j < sz; j++) {
            for (int i = 0; i < sx; i++) {
                float s = 0;
                int n = 0;
                for (int dj = -r; dj <= r; dj++) {
                    for (int di = -r; di <= r; di++) {
                        int ii = i + di, jj = j + dj;
                        if (ii >= 0 && jj >= 0 && ii < sx && jj < sz) {
                            s += a[jj * sx + ii];
                            n++;
                        }
                    }
                }
                b[j * sx + i] = s / n;
            }
        }
        return b;
    }

    /** Each block's distance to water, by steps. */
    static int[] waterDistance(boolean[] wet, int sx, int sz) {
        int[] d = new int[wet.length];
        java.util.Arrays.fill(d, Integer.MAX_VALUE);
        ArrayDeque<Integer> q = new ArrayDeque<>();
        for (int i = 0; i < wet.length; i++) {
            if (wet[i]) {
                d[i] = 0;
                q.add(i);
            }
        }
        while (!q.isEmpty()) {
            int i = q.poll(), x = i % sx, z = i / sx;
            for (int[] s : new int[][] {{1, 0}, {-1, 0}, {0, 1}, {0, -1}}) {
                int xx = x + s[0], zz = z + s[1];
                if (xx >= 0 && zz >= 0 && xx < sx && zz < sz && d[zz * sx + xx] > d[i] + 1) {
                    d[zz * sx + xx] = d[i] + 1;
                    q.add(zz * sx + xx);
                }
            }
        }
        return d;
    }
}
