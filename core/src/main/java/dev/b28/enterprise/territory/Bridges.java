package dev.b28.enterprise.territory;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.StairBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.Half;

/**
 * A crossing the grade keeps, assembled from parts (the studio's `js/terrain/bridges.mjs`, thread 0014; John approved
 * Dalaran's look; change one, change both): abutments into both banks, the channel arch in the middle (the largest of 9,
 * 7 or 5 the span and the deck allow), arches of 5 on piers with cutwaters stepping out from it, a beam where there's no
 * room for an arch; a crossing of 3 or less a causeway with a culvert. In the kingdom's dress: its coursed stone, coping,
 * stairs and parapet, a post and lantern over each pier and at each end.
 * <p>Laid in the crossing's frame: {@code i} along the street (0 the near bank's last dry cell, 1..span over the water,
 * span + 1 the far bank), {@code o} across it (0..width - 1 the street, -1 and width the parapets, -2 and width + 1 the
 * cutwaters), turned onto the crossing's way out.
 */
public final class Bridges {
    private Bridges() {}

    /** Each arch's ring, its rise over the springing cell by cell. */
    static final int[][] ARCH = {{}, {}, {}, {}, {}, {1, 2, 3, 2, 1}, {}, {1, 2, 3, 4, 3, 2, 1}, {}, {1, 3, 4, 5, 5, 5, 4, 3, 1}};

    /** How far over the water an arch of {@code n} needs the deck: its crown, a ring and a course. */
    static int need(int n) {
        int top = 0;
        for (int h : ARCH[n]) {
            top = Math.max(top, h);
        }
        return top + 3;
    }

    /** A segment of the crossing: abutment, arch (of {@code n}), pier or beam, from {@code i0}, {@code len} long. */
    record Segment(String part, int i0, int len, int n) {}

    /** The crossing's segments, near bank to far: the channel arch in the middle, arches of 5 on piers either side. */
    static List<Segment> plan(int span, int deck, int water) {
        int room = deck - water, n = 0;
        for (int m : new int[] {9, 7, 5}) {
            if (m <= span && need(m) <= room) {
                n = m;
                break;
            }
        }
        List<Segment> segs = new ArrayList<>();
        if (n == 0) {
            segs.add(new Segment("abutment", 0, 1, 0));
            segs.add(new Segment("beam", 1, span, 0));
            segs.add(new Segment("abutment", span + 1, 1, 0));
            return segs;
        }
        int rest = span - n, left = rest / 2, right = rest - left;
        int lq = left / 6, la = left - 6 * lq, rq = right / 6;
        segs.add(new Segment("abutment", 0, 1 + la, 0));
        int i = 1 + la;
        for (int k = 0; k < lq; k++) {
            segs.add(new Segment("arch", i, 5, 5));
            segs.add(new Segment("pier", i + 5, 1, 0));
            i += 6;
        }
        segs.add(new Segment("arch", i, n, n));
        i += n;
        for (int k = 0; k < rq; k++) {
            segs.add(new Segment("pier", i, 1, 0));
            segs.add(new Segment("arch", i + 1, 5, 5));
            i += 6;
        }
        segs.add(new Segment("abutment", i, span + 2 - i, 0));
        return segs;
    }

    /**
     * A kingdom's bridges (the studio's BRIDGE, rev 2): arches or timber beams, their coursed stone, coping, the arch
     * ring's stone, stairs and parapet, a timber bridge's bay between piers.
     */
    record Style(boolean arch, BlockState[] courses, BlockState coping, BlockState ring, Block stair, BlockState rail,
            BlockState post, int bay) {
        BlockState course(long seed) {
            return courses[(int) Math.floorMod(seed * 0x9E3779B97F4A7C15L >>> 33, (long) courses.length)];
        }
    }

    /** How far over the water a kingdom's deck stands for a crossing of {@code span}: the grade's floor (deckOver). */
    public static int over(String kingdom, int span) {
        if (span <= 3) {
            return 1;
        }
        if (!style(kingdom).arch()) {
            return 5;
        }
        return span <= 8 ? 6 : span <= 20 ? 7 : 8;
    }

    static Style style(String kingdom) {
        BlockState sb = Blocks.STONE_BRICKS.defaultBlockState();
        return switch (kingdom) {
            // Dalaran's, as approved: pale dressed stone a little cracked and mossy, polished andesite coping and ring, the
            // dark oak rail on red posts.
            case "dalaran" -> new Style(true, new BlockState[] {sb, sb, sb, Blocks.CRACKED_STONE_BRICKS.defaultBlockState(),
                    Blocks.MOSSY_STONE_BRICKS.defaultBlockState(), Blocks.CHISELED_STONE_BRICKS.defaultBlockState()},
                    Blocks.POLISHED_ANDESITE.defaultBlockState(), Blocks.POLISHED_ANDESITE.defaultBlockState(),
                    Blocks.POLISHED_ANDESITE_STAIRS, Blocks.DARK_OAK_FENCE.defaultBlockState(), Blocks.MANGROVE_FENCE.defaultBlockState(), 0);
            // Stormwind's: white stone, the arch rings in smooth quartz (its pilasters' stone), a stone-brick parapet.
            case "stormwind" -> new Style(true, new BlockState[] {sb, sb, sb, sb, Blocks.CRACKED_STONE_BRICKS.defaultBlockState(),
                    Blocks.CHISELED_STONE_BRICKS.defaultBlockState()}, Blocks.POLISHED_ANDESITE.defaultBlockState(),
                    Blocks.SMOOTH_QUARTZ.defaultBlockState(), Blocks.QUARTZ_STAIRS, Blocks.STONE_BRICK_WALL.defaultBlockState(),
                    Blocks.STONE_BRICK_WALL.defaultBlockState(), 0);
            // Lordaeron's: timber on fieldstone piers every 4, a spruce rail.
            default -> new Style(false, new BlockState[] {Blocks.COBBLESTONE.defaultBlockState(), Blocks.COBBLESTONE.defaultBlockState(),
                    Blocks.MOSSY_COBBLESTONE.defaultBlockState(), Blocks.MOSSY_COBBLESTONE.defaultBlockState(),
                    Blocks.STONE.defaultBlockState(), Blocks.ANDESITE.defaultBlockState()}, sb, sb, Blocks.COBBLESTONE_STAIRS,
                    Blocks.SPRUCE_FENCE.defaultBlockState(), Blocks.SPRUCE_FENCE.defaultBlockState(), 4);
        };
    }

    /** The frame onto the world: the near bank's centre cell, the way out (0 north, 1 south, 2 east, 3 west), the width. */
    record Frame(int x, int z, int facing, int width) {
        int[] f() {
            return Grading.DIRS[facing];
        }

        /** The way across the street, as the grade's cells run: +x for a street along z, +z for one along x. */
        int[] across() {
            return f()[0] == 0 ? new int[] {1, 0} : new int[] {0, 1};
        }

        /** The world column of frame cell (i, o): o 0 is the street's first cell across, as the grade's cells run. */
        int[] at(int i, int o) {
            int[] f = f(), a = across();
            int first = -((width - 1) / 2);
            return new int[] {x + f[0] * i + a[0] * (o + first), z + f[1] * i + a[1] * (o + first)};
        }

        /** The way out over the water ({@code out}), or back to the near bank, as a direction. */
        Direction along(boolean out) {
            int[] f = f();
            return Direction.fromDelta(out ? f[0] : -f[0], 0, out ? f[1] : -f[1]);
        }

        /** Across the street toward its first cell's far side ({@code plus}) or back. */
        Direction across(boolean plus) {
            int[] a = across();
            return Direction.fromDelta(plus ? a[0] : -a[0], 0, plus ? a[1] : -a[1]);
        }
    }

    /** Lays a built crossing. Returns the blocks placed. */
    public static int lay(ServerLevel level, String kingdom, Grading.Crossing c, int worldX, int worldZ, int width,
            BlockState pave) {
        Frame fr = new Frame(worldX, worldZ, c.facing(), width);
        Style st = style(kingdom);
        return c.span() <= 3 ? causeway(level, fr, st, c, pave) : st.arch() ? crossing(level, fr, st, c, pave)
                : beams(level, fr, st, c);
    }

    private static int crossing(ServerLevel level, Frame fr, Style st, Grading.Crossing c, BlockState pave) {
        int span = c.span(), deck = c.deck(), water = c.water(), spring = water + 1, width = fr.width(), placed = 0;
        Set<Integer> posts = new HashSet<>();
        List<BlockPos> rails = new ArrayList<>();
        for (Segment sg : plan(span, deck, water)) {
            for (int i = sg.i0(); i < sg.i0() + sg.len(); i++) {
                for (int o = -1; o <= width; o++) {
                    boolean edge = o == -1 || o == width;
                    int[] w = fr.at(i, o);
                    put(level, w, deck, edge ? st.coping() : pave);
                    for (int y = deck + 1; y <= deck + 3; y++) {
                        put(level, w, y, Blocks.AIR.defaultBlockState());
                    }
                    placed++;
                    switch (sg.part()) {
                        case "abutment", "pier" -> {
                            for (int y = bed(level, w, water) + 1; y < deck; y++) {
                                put(level, w, y, st.course(i * 31L + o + y * 7L));
                            }
                        }
                        case "arch" -> {
                            int k = i - sg.i0(), h = ARCH[sg.n()][k], ring = spring + h;
                            for (int y = ring; y < deck; y++) {
                                put(level, w, y, y == ring ? st.ring() : st.course(i * 31L + o + y * 7L));
                            }
                            // The soffit's step: an upside-down stair under the ring where the next cell toward the crown rises.
                            int[] nb = ARCH[sg.n()];
                            double mid = (nb.length - 1) / 2.0;
                            int inward = k < mid ? 1 : k > mid ? -1 : 0;
                            if (inward != 0 && k + inward >= 0 && k + inward < nb.length && nb[k + inward] > h) {
                                // Its high side toward the springing: back toward the near bank on the near half.
                                put(level, w, ring, st.stair().defaultBlockState().setValue(StairBlock.FACING, fr.along(inward < 0))
                                        .setValue(StairBlock.HALF, Half.TOP));
                            }
                        }
                        case "beam" -> put(level, w, deck - 1, Blocks.STRIPPED_DARK_OAK_LOG.defaultBlockState()
                                .setValue(net.minecraft.world.level.block.RotatedPillarBlock.AXIS,
                                        fr.f()[0] != 0 ? Direction.Axis.X : Direction.Axis.Z));
                        default -> {}
                    }
                }
                if (sg.part().equals("pier")) {
                    // Cutwaters: pointed stone to a block over the water on both faces.
                    for (int o : new int[] {-2, width + 1}) {
                        int[] w = fr.at(i, o);
                        for (int y = bed(level, w, water) + 1; y <= water; y++) {
                            put(level, w, y, st.course(i * 17L + o + y));
                        }
                        put(level, w, water + 1, st.stair().defaultBlockState().setValue(StairBlock.FACING, fr.across(o < 0)));
                    }
                    posts.add(i);
                }
            }
            if (sg.part().equals("abutment")) {
                posts.add(sg.i0() == 0 ? 0 : sg.i0() + sg.len() - 1);
            }
        }
        // The parapet along both sides, a post and a lantern over each pier and at each end; joined by the game's shapes.
        for (int i = 0; i <= span + 1; i++) {
            for (int o : new int[] {-1, width}) {
                int[] w = fr.at(i, o);
                boolean post = posts.contains(i) || i == 0 || i == span + 1;
                put(level, w, deck + 1, post ? st.post() : st.rail());
                rails.add(new BlockPos(w[0], deck + 1, w[1]));
                if (post) {
                    put(level, w, deck + 2, Blocks.LANTERN.defaultBlockState());
                }
            }
        }
        for (BlockPos at : rails) {
            level.setBlock(at, Block.updateFromNeighbourShapes(level.getBlockState(at), level, at), 2 | 16);
        }
        return placed;
    }

    /**
     * A timber crossing (Lordaeron's): fieldstone piers every bay (and abutments at the banks) up to the beams, capped by
     * a crosswise log; stripped spruce beams under both edges and the middle, braced off each pier with upturned stairs;
     * a plank deck; the rail, a post over each pier, lanterns at the ends and every other pier.
     */
    private static int beams(ServerLevel level, Frame fr, Style st, Grading.Crossing c) {
        int span = c.span(), deck = c.deck(), water = c.water(), width = fr.width(), bay = st.bay(), beam = deck - 1, placed = 0;
        java.util.function.IntPredicate pier = i -> i == 0 || i == span + 1 || i % bay == 0 && i < span;
        int[] under = {-1, (width - 1) / 2, width};
        Direction.Axis along = fr.f()[0] != 0 ? Direction.Axis.X : Direction.Axis.Z, crosswise = along == Direction.Axis.X ? Direction.Axis.Z : Direction.Axis.X;
        List<BlockPos> rails = new ArrayList<>();
        for (int i = 0; i <= span + 1; i++) {
            for (int o = -1; o <= width; o++) {
                int[] w = fr.at(i, o);
                int bedY = bed(level, w, water);
                for (int y = deck + 1; y <= deck + 3; y++) {
                    put(level, w, y, Blocks.AIR.defaultBlockState());
                }
                put(level, w, deck, o == -1 || o == width ? Blocks.SPRUCE_SLAB.defaultBlockState()
                        .setValue(net.minecraft.world.level.block.SlabBlock.TYPE, net.minecraft.world.level.block.state.properties.SlabType.TOP)
                        : Blocks.SPRUCE_PLANKS.defaultBlockState());
                placed++;
                if (pier.test(i)) {
                    for (int y = bedY + 1; y < beam; y++) {
                        put(level, w, y, y == beam - 1 ? Blocks.STRIPPED_SPRUCE_LOG.defaultBlockState()
                                .setValue(net.minecraft.world.level.block.RotatedPillarBlock.AXIS, crosswise) : st.course(i * 31L + o + y * 9L));
                    }
                }
                for (int u : under) {
                    if (u == o) {
                        put(level, w, beam, Blocks.STRIPPED_SPRUCE_LOG.defaultBlockState()
                                .setValue(net.minecraft.world.level.block.RotatedPillarBlock.AXIS, along));
                    }
                }
            }
            if (pier.test(i) && i > 0 && i < span + 1) {
                for (int o : new int[] {-2, width + 1}) {
                    int[] w = fr.at(i, o);
                    for (int y = bed(level, w, water) + 1; y <= water; y++) {
                        put(level, w, y, st.course(i * 7L + o + y));
                    }
                    put(level, w, water + 1, st.stair().defaultBlockState().setValue(StairBlock.FACING, fr.across(o < 0)));
                }
                // The braces: off each pier's face, an upturned stair under the beam either side, toward the span.
                for (int u : under) {
                    put(level, fr.at(i - 1, u), beam - 1, Blocks.SPRUCE_STAIRS.defaultBlockState()
                            .setValue(StairBlock.FACING, fr.along(true)).setValue(StairBlock.HALF, Half.TOP));
                    put(level, fr.at(i + 1, u), beam - 1, Blocks.SPRUCE_STAIRS.defaultBlockState()
                            .setValue(StairBlock.FACING, fr.along(false)).setValue(StairBlock.HALF, Half.TOP));
                }
            }
        }
        for (int i = 0; i <= span + 1; i++) {
            for (int o : new int[] {-1, width}) {
                int[] w = fr.at(i, o);
                put(level, w, deck + 1, pier.test(i) ? st.post() : st.rail());
                rails.add(new BlockPos(w[0], deck + 1, w[1]));
                if (i == 0 || i == span + 1 || pier.test(i) && (i / bay) % 2 == 0) {
                    put(level, w, deck + 2, Blocks.LANTERN.defaultBlockState());
                }
            }
        }
        for (BlockPos at : rails) {
            level.setBlock(at, Block.updateFromNeighbourShapes(level.getBlockState(at), level, at), 2 | 16);
        }
        return placed;
    }

    /** A crossing of 3 or less: coursed fill from the bed, paved at the street's level, coped, a culvert in its middle. */
    private static int causeway(ServerLevel level, Frame fr, Style st, Grading.Crossing c, BlockState pave) {
        int span = c.span(), deck = c.deck(), water = c.water(), mid = (span + 1) / 2, placed = 0;
        for (int i = 1; i <= span; i++) {
            for (int o = -1; o <= fr.width(); o++) {
                boolean edge = o == -1 || o == fr.width(), culvert = i == mid;
                int[] w = fr.at(i, o);
                for (int y = bed(level, w, water) + 1; y < deck; y++) {
                    put(level, w, y, culvert && y <= water + 1 ? (y <= water ? Blocks.WATER.defaultBlockState() : Blocks.AIR.defaultBlockState())
                            : y == deck - 1 && edge ? st.coping() : st.course(i * 13L + o + y));
                }
                put(level, w, deck, edge ? st.coping() : pave);
                placed++;
            }
        }
        return placed;
    }

    /** The first solid height under the water's surface at a column (the bank's own ground where it's dry). */
    static int bed(ServerLevel level, int[] w, int water) {
        int y = water;
        while (y > level.getMinBuildHeight()) {
            BlockState b = level.getBlockState(new BlockPos(w[0], y, w[1]));
            if (!b.isAir() && b.getFluidState().isEmpty() && !b.canBeReplaced()) {
                break;
            }
            y--;
        }
        return y;
    }

    private static void put(ServerLevel level, int[] w, int y, BlockState s) {
        level.setBlock(new BlockPos(w[0], y, w[1]), s, 2 | 16);
    }
}
