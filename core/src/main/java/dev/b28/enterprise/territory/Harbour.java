package dev.b28.enterprise.territory;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.state.BlockState;

/**
 * A town's harbour district where its plan meets water (the studio's harbour set, batch 037, thread 0012; John: "check
 * out the port/harbor work the designer has done"), laid once its land is shaped, in its kingdom's set
 * ({@code structure/<kingdom>/harbour/}): the sea where the water is open (2,000 columns or more, or reaching past the
 * plan's edge), else a river. Its head is where the avenue nearest the water meets it; from there, by the town's tier:
 * a hamlet a jetty and a fishing boat; a village the first run of quay along the shore (stone at sea, timber on a river;
 * a bollard every 3rd, a lamp every 5th, a ladder every 8th, steps by the jetty), 16 at sea and 8 on a river, and a
 * barge on a river; a town 32 of quay and a trader alongside. The buildings (customs house, warehouses, fish market),
 * the mole and its lighthouse come with the quay street.
 */
public final class Harbour {
    private Harbour() {}

    /** What was laid. */
    public record Laid(boolean sea, int quay, int jetty, int boats, int buildings) {}

    /** A shore cell: the land's last column, its way out over the water. */
    record Shore(BlockPos at, Direction out) {}

    /** Lays the town's harbour at its tier ("hamlet", "village", "town", ...), if its plan meets water. */
    public static java.util.Optional<Laid> lay(ServerLevel level, TownPlans.Plan plan, String kingdom, String tier) {
        BlockPos bell = plan.bell();
        // The head: of the places an avenue meets the water, the best for a harbour (the studio's, thread 0012): the most
        // dry land at the deck's height behind 24 of its shore, water 3 deep alongside it, at sea the wider water; the
        // nearer the forum on a tie.
        Shore head = null;
        int w = Integer.MIN_VALUE;
        boolean sea = false;
        double best = Double.NEGATIVE_INFINITY;
        for (Direction way : Direction.Plane.HORIZONTAL) {
            int reach = switch (way) {
                case NORTH -> bell.getZ() - plan.outer().z0();
                case SOUTH -> plan.outer().z1() - bell.getZ();
                case WEST -> bell.getX() - plan.outer().x0();
                default -> plan.outer().x1() - bell.getX();
            };
            for (int k = Forums.REACH + 2; k <= reach; k++) {
                BlockPos c = bell.relative(way, k);
                if (!level.isLoaded(c)) {
                    break;
                }
                int wk = water(level, c);
                if (wk == Integer.MIN_VALUE) {
                    continue;
                }
                Shore at = new Shore(bell.relative(way, k - 1), way);
                // Sea or river by the water's width here (the studio's): 24 or more along the avenue's line, or reaching
                // past the plan's edge within it.
                int span = 0;
                boolean past = false;
                for (int a = 0; a < 64; a++) {
                    BlockPos o = c.relative(way, a);
                    if (!level.isLoaded(o) || water(level, o) == Integer.MIN_VALUE) {
                        break;
                    }
                    span++;
                    past |= !plan.outer().contains(o.getX(), o.getZ());
                }
                boolean isSea = span >= 24 || past;
                double score = room(level, at, wk + 2) + 8 * deep(level, at, wk) + (isSea ? span : 0) - 0.1 * k;
                if (score > best) {
                    best = score;
                    head = at;
                    w = wk;
                    sea = isSea;
                }
                break;
            }
        }
        if (head == null) {
            return java.util.Optional.empty();
        }
        String set = level.getServer().getResourceManager()
                .getResource(net.minecraft.resources.ResourceLocation.parse("enterprise:structure/" + kingdom + "/harbour/jetty_timber_3x4.json"))
                .isPresent() ? kingdom : "lordaeron";
        int rank = switch (tier) {
            case "hamlet" -> 0;
            case "village" -> 1;
            default -> 2;
        };
        int quay = 0, jetty = 0, boats = 0;
        List<Shore> line = List.of();
        // The quay along the shore from the head, both ways.
        if (rank >= 1) {
            // One quay line along the shore through the head, under and between the buildings' fronts to the jetty's
            // root (the studio's r90 render: each hall had its own skirt of quay, earth between): 24 either way at sea,
            // 8 on a river, and the whole shore of a town.
            int length = rank >= 2 ? 64 : sea ? 48 : 16;
            List<Shore> run = shore(level, head, length, w);
            for (int k = 0; k < run.size(); k++) {
                Shore s = run.get(k);
                String tile = (sea ? "quay_stone_" : "quay_timber_") + (k == 0 ? "1a"
                        : k % 8 == 4 ? "ladder_1" : k % 5 == 2 ? "lamp_1" : sea && k % 3 == 1 ? "bollard_1"
                        : "1" + (char) ('a' + Math.floorMod(s.at().getX() * 31 + s.at().getZ(), sea ? 3 : 2)));
                if (quayTile(level, set, tile, s, w)) {
                    quay++;
                } else {
                    // A cell of the line that takes no tile (a corner, its water on the slant): paved at the deck, not a
                    // stub of the old bank in the quay wall.
                    clearOver(level, s.at().getX(), s.at().getZ(), w + 2);
                    Terraform.settle(level, s.at().getX(), s.at().getZ(), w + 2);
                    level.setBlock(new BlockPos(s.at().getX(), w + 2, s.at().getZ()),
                            net.minecraft.world.level.block.Blocks.STONE_BRICKS.defaultBlockState(), 2 | 16);
                    for (int y = w + 1; y > level.getMinBuildHeight(); y--) {
                        BlockPos b = new BlockPos(s.at().getX(), y, s.at().getZ());
                        BlockState under = level.getBlockState(b);
                        if (under.getFluidState().isEmpty() && !under.isAir() && !under.canBeReplaced()) {
                            break;
                        }
                        level.setBlock(b, net.minecraft.world.level.block.Blocks.STONE_BRICKS.defaultBlockState(), 2 | 16);
                    }
                }
            }
            // Behind the quay line, the quay walk and the district's ground levelled to the deck (6 in), walled where the
            // land rises behind it (the studio's: the hillside came down to the water in raw earth beside the halls).
            for (Shore s : run) {
                terrace(level, kingdom, s, w + 2);
            }
            line = run;
        }
        // The jetty out from the head: segments while the water's deep enough ahead, then its head.
        BlockPos start = head.at().relative(head.out(), 2);
        int along = 0;
        for (int seg = 0; seg < 3 && open(level, start, head.out(), along, 4, 1, w); seg++) {
            if (place(level, set, "jetty_timber_3x4", start, head.out(), -1, 1, along, along + 3, w)) {
                piles(level, start, head.out(), new int[][] {{-1, along}, {1, along}, {-1, along + 3}, {1, along + 3}}, w);
                jetty++;
            }
            along += 4;
        }
        if (jetty > 0 && open(level, start, head.out(), along, 5, 2, w)
                && place(level, set, "jetty_head_timber_5x5", start, head.out(), -2, 2, along, along + 4, w)) {
            piles(level, start, head.out(), new int[][] {{-2, along}, {2, along}, {-2, along + 4}, {2, along + 4}, {0, along + 4}}, w);
            jetty++;
        }
        // Boats: a fishing boat beside the jetty; a barge on a river from village; a trader at sea from town.
        if (jetty > 0 && boat(level, set, "boat_fishing_3x8", start, head.out(), 3, 5, 1, 8, w)) {
            boats++;
        }
        if (rank >= 1 && !sea && boat(level, set, "boat_barge_4x11", start, head.out(), -6, -3, 1, 11, w)) {
            boats++;
        }
        if (rank >= 2 && sea && boat(level, set, "boat_trader_9x25", start, head.out(), -12, -4, 2, 26, w)) {
            boats++;
        }
        // The district's first buildings behind the quay, fronts to the water (from village): at sea the customs house
        // at the head, a warehouse beside it and the fish market the other side; on a river the river warehouse behind
        // the wharf and the customs booth beside it.
        int buildings = 0;
        if (rank >= 1) {
            List<String> row = sea ? List.of("harbour:customs_house", "harbour:warehouse", "harbour:fish_market")
                    : List.of("harbour:warehouse", "harbour:river_customs");
            // Along the shore from the head, the first spot each fits on dry land clear of the others (a river bank or a
            // slant shore leaves room only here and there: r88), nearest the head first.
            List<int[]> taken = new ArrayList<>();
            for (String slot : row) {
                var kit = building(level, kingdom, slot, !sea && slot.equals("harbour:warehouse"));
                if (kit.isEmpty()) {
                    continue;
                }
                for (int step = 0; step <= 12; step++) {
                    int centre = (step % 2 == 0 ? 1 : -1) * ((step + 1) / 2) * 4;
                    int[] rect = hall(level, kit.get(), head, centre, w + 2, taken, set, sea, w);
                    if (rect != null) {
                        taken.add(rect);
                        buildings++;
                        break;
                    }
                }
            }
        }
        // No bare earth at the water (the studio's r95 render: 11 earth cells on the waterline where the shore turns):
        // within the quay's stretch, a land cell beside the water that's neither quay nor built gets a plain stone edge at
        // the deck.
        if (!line.isEmpty()) {
            edges(level, line, w);
        }
        return java.util.Optional.of(new Laid(sea, quay, jetty, boats, buildings));
    }

    /** How far round the quay line's bounds its stretch reaches, for the edges. */
    static final int EDGE = 4;

    /** Within the quay line's stretch (its bounds, {@link #EDGE} round), each land cell 4-touching the water whose top is earth: paved stone at the deck. */
    static void edges(ServerLevel level, List<Shore> run, int w) {
        int x0 = Integer.MAX_VALUE, z0 = Integer.MAX_VALUE, x1 = Integer.MIN_VALUE, z1 = Integer.MIN_VALUE;
        for (Shore s : run) {
            x0 = Math.min(x0, s.at().getX());
            z0 = Math.min(z0, s.at().getZ());
            x1 = Math.max(x1, s.at().getX());
            z1 = Math.max(z1, s.at().getZ());
        }
        int paved = 0;
        for (int x = x0 - EDGE; x <= x1 + EDGE; x++) {
            for (int z = z0 - EDGE; z <= z1 + EDGE; z++) {
                {
                    BlockPos c = new BlockPos(x, 0, z);
                    if (!level.isLoaded(c) || water(level, c) != Integer.MIN_VALUE) {
                        continue;
                    }
                    boolean wet = false;
                    for (Direction d : Direction.Plane.HORIZONTAL) {
                        wet |= water(level, c.relative(d)) == w;
                    }
                    int g = Terraform.ground(level, c.getX(), c.getZ());
                    BlockState top = level.getBlockState(new BlockPos(c.getX(), g, c.getZ()));
                    if (!wet || !dev.b28.enterprise.settlement.Grounds.natural(top) || top.is(net.minecraft.world.level.block.Blocks.DIRT_PATH)) {
                        continue;
                    }
                    paved++;
                    clearOver(level, c.getX(), c.getZ(), w + 2);
                    for (int y = w + 2; y > level.getMinBuildHeight(); y--) {
                        BlockPos b = new BlockPos(c.getX(), y, c.getZ());
                        BlockState under = level.getBlockState(b);
                        if (y < w + 2 && under.getFluidState().isEmpty() && !under.isAir() && !under.canBeReplaced()
                                && !dev.b28.enterprise.settlement.Grounds.natural(under)) {
                            break;
                        }
                        if (y < g - 1 && dev.b28.enterprise.settlement.Grounds.natural(under)) {
                            break; // the bank's own ground under it
                        }
                        level.setBlock(b, net.minecraft.world.level.block.Blocks.STONE_BRICKS.defaultBlockState(), 2 | 16);
                    }
                }
            }
        }
        dev.b28.enterprise.Enterprise.LOGGER.debug("Harbour edges: {} paved at the water", paved);
    }

    /** The kingdom's harbour building for a slot (its own set's, else Lordaeron's); the smaller warehouse on a river. */
    static java.util.Optional<Kits.Meta> building(ServerLevel level, String kingdom, String slot, boolean small) {
        var resources = level.getServer().getResourceManager();
        for (String k : List.of(kingdom, "lordaeron")) {
            List<Kits.Meta> fit = Kits.kits(resources, k, slot, 99, 99).stream().flatMap(id -> Kits.meta(resources, id).stream())
                    .sorted(java.util.Comparator.comparingInt(Kits.Meta::width)).toList();
            if (!fit.isEmpty()) {
                return java.util.Optional.of(small ? fit.get(0) : fit.get(fit.size() - 1));
            }
        }
        return java.util.Optional.empty();
    }

    /**
     * A harbour building on the shore: its front 2 in from the head's shore (the quay walk between), further in where
     * the shore runs slant (to 10), or, where no dry plot is there, out over the shallows (to 6 out, water 3 deep or
     * less: reclaimed, filled to the deck, as real harbours are built out; the studio's, thread 0012); centred
     * {@code centre} along the shore from the head, clear of the others, its plot levelled to the quay's deck, its front
     * to the water, and quay tiles along its water front (no bare bank under its arcade). Its rect, or null.
     */
    static int[] hall(ServerLevel level, Kits.Meta kit, Shore head, int centre, int deck, List<int[]> taken, String set,
            boolean sea, int w) {
        Direction out = head.out(), in = out.getOpposite(), across = out.getClockWise();
        int wd = kit.width(), dp = kit.depth(), o0 = centre - wd / 2;
        int back = Integer.MIN_VALUE, x0 = 0, z0 = 0, x1 = 0, z1 = 0;
        int[] tries = {2, 3, 4, 5, 6, 7, 8, 9, 10, 1, 0, -1, -2, -3, -4, -5, -6};
        search:
        for (int a0 : tries) {
            x0 = Integer.MAX_VALUE; z0 = Integer.MAX_VALUE; x1 = Integer.MIN_VALUE; z1 = Integer.MIN_VALUE;
            for (int a : new int[] {a0, a0 + dp - 1}) {
                for (int o : new int[] {o0, o0 + wd - 1}) {
                    BlockPos c = head.at().relative(in, a).relative(across, o);
                    x0 = Math.min(x0, c.getX());
                    z0 = Math.min(z0, c.getZ());
                    x1 = Math.max(x1, c.getX());
                    z1 = Math.max(z1, c.getZ());
                }
            }
            for (int[] t : taken) {
                if (x0 <= t[2] + 1 && x1 >= t[0] - 1 && z0 <= t[3] + 1 && z1 >= t[1] - 1) {
                    continue search;
                }
            }
            for (int x = x0; x <= x1; x++) {
                for (int z = z0; z <= z1; z++) {
                    BlockPos c = new BlockPos(x, 0, z);
                    if (!level.isLoaded(c)) {
                        continue search;
                    }
                    if (water(level, c) != Integer.MIN_VALUE && (a0 > 1 || water(level, c) != w || depth(level, c, w) > 3)) {
                        continue search; // water behind the shore, or too deep to fill
                    }
                }
            }
            back = a0;
            break;
        }
        if (back == Integer.MIN_VALUE) {
            return null;
        }
        // Its plot and the quay walk before it filled where they're water (reclaimed), then levelled to the deck.
        for (int a = back - 2; a < back + dp; a++) {
            for (int o = o0; o < o0 + wd; o++) {
                BlockPos c = head.at().relative(in, a).relative(across, o);
                if (water(level, c) == w && depth(level, c, w) <= 3 && (a >= back || back <= 1)) {
                    fill(level, c, w, deck);
                } else if (water(level, c) == Integer.MIN_VALUE && a < back) {
                    Terraform.settle(level, c.getX(), c.getZ(), deck);
                }
            }
        }
        // Its plot cleared over the deck of the land and the grade's earthworks in it (r92: they hung in the air over a
        // levelled plot), then levelled.
        for (int x = x0 - 1; x <= x1 + 1; x++) {
            for (int z = z0 - 1; z <= z1 + 1; z++) {
                clearOver(level, x, z, deck);
            }
        }
        for (int a = back - 2; a < back; a++) {
            for (int o = o0; o < o0 + wd; o++) {
                BlockPos c = head.at().relative(in, a).relative(across, o);
                clearOver(level, c.getX(), c.getZ(), deck);
            }
        }
        Plots.level(level, x0 - 1, z0 - 1, x1 + 1, z1 + 1, deck);
        boolean laid = Kits.place(level, kit.id(), new BlockPos(x0, deck, z0), out).isPresent();
        if (!laid) {
            dev.b28.enterprise.Enterprise.LOGGER.info("{} couldn't be laid behind the quay at {}", kit.id(), head.at().toShortString());
            return null;
        }
        // Quay tiles along its water front: from before its front, out to the water's edge.
        for (int o = o0; o < o0 + wd; o++) {
            BlockPos last = null;
            for (int a = back - 1; a >= back - 8; a--) {
                BlockPos c = head.at().relative(in, a).relative(across, o);
                if (water(level, c) == w) {
                    break;
                }
                last = c;
            }
            if (last != null && water(level, last.relative(out)) == w) {
                quayTile(level, set, (sea ? "quay_stone_" : "quay_timber_") + (Math.floorMod(o, 5) == 2 ? "lamp_1" : "1a"),
                        new Shore(last, out), w);
            }
        }
        return new int[] {x0, z0, x1, z1};
    }

    /** How deep behind the quay line its ground is levelled to the deck. */
    static final int BAND = 6;

    /**
     * Behind a quay cell, its ground levelled to the deck {@link #BAND} in (natural ground only: a street or anything built
     * is left), and where the land beyond stands higher, the kingdom's retaining wall at the band's back.
     */
    static void terrace(ServerLevel level, String kingdom, Shore s, int deck) {
        Direction in = s.out().getOpposite();
        for (int a = 1; a <= BAND; a++) {
            BlockPos c = s.at().relative(in, a);
            if (!level.isLoaded(c) || water(level, c) != Integer.MIN_VALUE) {
                return;
            }
            int g = Terraform.ground(level, c.getX(), c.getZ());
            BlockState top = level.getBlockState(new BlockPos(c.getX(), g, c.getZ()));
            boolean land = dev.b28.enterprise.settlement.Grounds.natural(top) && !top.is(net.minecraft.world.level.block.Blocks.DIRT_PATH)
                    || earthwork(top);
            if (!land) {
                return; // a street or something built: the band stops at it
            }
            // The land rising 4 or more over the deck within the band: the band stops there, walled (no dam of a face:
            // the studio's r91 render, a 14-high earth face at the band's back).
            if (g - deck >= 4) {
                Earthworks.step(level, kingdom, c.getX(), c.getZ(), s.out().getStepX(), s.out().getStepZ(), deck, g - deck, "wall");
                return;
            }
            // Everything over the deck cleared, the grade's own earthworks in it too (they belong to the land the band
            // reshapes: r91 left 326 terrace stones hanging in the air), the ground filled under it, grass on top.
            for (int y = deck + 1; y <= Math.max(g, deck) + 6; y++) {
                level.setBlock(new BlockPos(c.getX(), y, c.getZ()), net.minecraft.world.level.block.Blocks.AIR.defaultBlockState(), 2 | 16);
            }
            for (int y = Math.min(g, deck) + 1; y < deck; y++) {
                level.setBlock(new BlockPos(c.getX(), y, c.getZ()), net.minecraft.world.level.block.Blocks.DIRT.defaultBlockState(), 2 | 16);
            }
            level.setBlock(new BlockPos(c.getX(), deck, c.getZ()), net.minecraft.world.level.block.Blocks.GRASS_BLOCK.defaultBlockState(), 2 | 16);
        }
        BlockPos back = s.at().relative(in, BAND + 1);
        if (!level.isLoaded(back) || water(level, back) != Integer.MIN_VALUE) {
            return;
        }
        int g = Terraform.ground(level, back.getX(), back.getZ());
        if (g - deck >= 1) {
            Earthworks.step(level, kingdom, back.getX(), back.getZ(), s.out().getStepX(), s.out().getStepZ(), deck, g - deck, "wall");
        }
    }

    /** A column cleared over {@code deck} of land, growth and the grade's earthworks (up to 24 over); anything else stays. */
    static void clearOver(ServerLevel level, int x, int z, int deck) {
        for (int y = deck + 1; y <= deck + 24; y++) {
            BlockPos at = new BlockPos(x, y, z);
            BlockState b = level.getBlockState(at);
            if (!b.isAir() && (b.canBeReplaced() || earthwork(b) || dev.b28.enterprise.settlement.Grounds.natural(b)
                    || b.is(net.minecraft.tags.BlockTags.LOGS) || b.is(net.minecraft.tags.BlockTags.LEAVES))
                    && b.getFluidState().isEmpty()) {
                level.setBlock(at, net.minecraft.world.level.block.Blocks.AIR.defaultBlockState(), 2 | 16);
            }
        }
    }

    /** A block of the grade's earthworks (its walls' courses, copings, rails, lamps): land, to a band that reshapes it. */
    static boolean earthwork(BlockState b) {
        String id = net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(b.getBlock()).getPath();
        return id.contains("stone_brick") || id.contains("andesite") || id.contains("cobblestone") || id.equals("moss_block")
                || id.equals("moss_carpet") || id.equals("vine") || id.equals("lantern") || id.endsWith("_fence")
                || id.equals("tuff") || id.equals("rooted_dirt");
    }

    /** A shallow water column filled to the deck: coursed stone from the bed, earth under its top, grass on it. */
    static void fill(ServerLevel level, BlockPos c, int w, int deck) {
        for (int y = w - depth(level, c, w) + 1; y <= deck; y++) {
            BlockState b = y == deck ? net.minecraft.world.level.block.Blocks.GRASS_BLOCK.defaultBlockState()
                    : y >= w ? net.minecraft.world.level.block.Blocks.DIRT.defaultBlockState()
                    : net.minecraft.world.level.block.Blocks.STONE_BRICKS.defaultBlockState();
            level.setBlock(new BlockPos(c.getX(), y, c.getZ()), b, 2 | 16);
        }
    }

    /** The dry columns at the deck's height (within 1) in 14 behind 24 of the shore round the head: room for the district. */
    static int room(ServerLevel level, Shore head, int deck) {
        Direction in = head.out().getOpposite(), across = head.out().getClockWise();
        int n = 0;
        for (int a = 1; a <= 14; a++) {
            for (int o = -12; o <= 12; o++) {
                BlockPos c = head.at().relative(in, a).relative(across, o);
                if (level.isLoaded(c) && water(level, c) == Integer.MIN_VALUE && Math.abs(Terraform.ground(level, c.getX(), c.getZ()) - deck) <= 1) {
                    n++;
                }
            }
        }
        return n;
    }

    /** The columns of open water 3 deep within 4 of the shore along 24 of it round the head: room for boats alongside. */
    static int deep(ServerLevel level, Shore head, int w) {
        Direction across = head.out().getClockWise();
        int n = 0;
        for (int a = 1; a <= 4; a++) {
            for (int o = -12; o <= 12; o++) {
                BlockPos c = head.at().relative(head.out(), a).relative(across, o);
                if (level.isLoaded(c) && water(level, c) == w && depth(level, c, w) >= 3) {
                    n++;
                }
            }
        }
        return n;
    }

    /** How deep the water stands at a column: its surface down to the bed. */
    static int depth(ServerLevel level, BlockPos c, int w) {
        int y = w;
        while (y > level.getMinBuildHeight() && !level.getFluidState(new BlockPos(c.getX(), y, c.getZ())).isEmpty()) {
            y--;
        }
        return w - y;
    }

    /** The water's surface height at a column, or {@link Integer#MIN_VALUE} if its top isn't water. */
    static int water(ServerLevel level, BlockPos c) {
        int y = level.getHeight(net.minecraft.world.level.levelgen.Heightmap.Types.MOTION_BLOCKING, c.getX(), c.getZ()) - 1;
        return level.getFluidState(new BlockPos(c.getX(), y, c.getZ())).isSource() ? y : Integer.MIN_VALUE;
    }


    /** The shore from the head, both ways along it: land columns beside the water, nearest the head first. */
    static List<Shore> shore(ServerLevel level, Shore head, int length, int w) {
        List<Shore> out = new ArrayList<>();
        Set<Long> seen = new HashSet<>();
        ArrayDeque<BlockPos> q = new ArrayDeque<>();
        q.add(head.at());
        seen.add(head.at().asLong());
        while (!q.isEmpty() && out.size() < length) {
            BlockPos c = q.pop();
            Direction out1 = null;
            for (Direction d : Direction.Plane.HORIZONTAL) {
                if (water(level, c.relative(d)) == w) {
                    out1 = d;
                    break;
                }
            }
            if (out1 == null) {
                continue;
            }
            out.add(new Shore(c, out1));
            for (int dx = -1; dx <= 1; dx++) {
                for (int dz = -1; dz <= 1; dz++) {
                    BlockPos n = c.offset(dx, 0, dz);
                    if (level.isLoaded(n) && seen.add(n.asLong()) && water(level, n) == Integer.MIN_VALUE) {
                        q.add(n);
                    }
                }
            }
        }
        return out;
    }

    /** A quay tile on a shore cell: its paving on the shore, its edge on the first water (filled to the bed), its face over the water. */
    static boolean quayTile(ServerLevel level, String set, String tile, Shore s, int w) {
        if (water(level, s.at().relative(s.out())) != w) {
            return false;
        }
        // The shore's ground at the deck's height, clear over it.
        int deck = w + 2;
        clearOver(level, s.at().getX(), s.at().getZ(), deck); // the grade's walls on the bank too (r94: left hanging)
        Terraform.settle(level, s.at().getX(), s.at().getZ(), deck);
        for (int up = deck + 1; up <= deck + 3; up++) {
            BlockPos a = new BlockPos(s.at().getX(), up, s.at().getZ());
            BlockState b = level.getBlockState(a);
            if (b.canBeReplaced() || dev.b28.enterprise.settlement.Grounds.natural(b)) {
                level.setBlock(a, net.minecraft.world.level.block.Blocks.AIR.defaultBlockState(), 2 | 16);
            }
        }
        boolean laid = place(level, set, tile, s.at(), s.out(), 0, 0, 0, 2, w);
        if (laid) {
            piles(level, s.at(), s.out(), new int[][] {{0, 0}, {0, 1}}, w, net.minecraft.world.level.block.Blocks.STONE_BRICKS.defaultBlockState());
        }
        return laid;
    }

    /** Is the water open ahead: {@code len} along from {@code along}, {@code half} either side, all the water's surface? */
    static boolean open(ServerLevel level, BlockPos start, Direction out, int along, int len, int half, int w) {
        Direction across = out.getClockWise();
        for (int a = along; a < along + len; a++) {
            for (int o = -half; o <= half; o++) {
                if (water(level, start.relative(out, a).relative(across, o)) != w) {
                    return false;
                }
            }
        }
        return true;
    }

    /** A part laid across {@code o0..o1} and along {@code a0..a1} from {@code origin}, its front out, its water line at the water. */
    static boolean place(ServerLevel level, String set, String part, BlockPos origin, Direction out, int o0, int o1, int a0,
            int a1, int w) {
        Direction across = out.getClockWise();
        int x0 = Integer.MAX_VALUE, z0 = Integer.MAX_VALUE;
        for (int a : new int[] {a0, a1}) {
            for (int o : new int[] {o0, o1}) {
                BlockPos c = origin.relative(out, a).relative(across, o);
                x0 = Math.min(x0, c.getX());
                z0 = Math.min(z0, c.getZ());
            }
        }
        return Kits.place(level, "enterprise:" + set + "/harbour/" + part, new BlockPos(x0, w, z0), out).isPresent();
    }

    /** A part's pilings: each {across, along} from the origin run down from the water line to the bed. */
    static void piles(ServerLevel level, BlockPos origin, Direction out, int[][] at, int w) {
        piles(level, origin, out, at, w, net.minecraft.world.level.block.Blocks.SPRUCE_LOG.defaultBlockState());
    }

    static void piles(ServerLevel level, BlockPos origin, Direction out, int[][] at, int w, BlockState block) {
        Direction across = out.getClockWise();
        for (int[] p : at) {
            BlockPos c = origin.relative(out, p[1]).relative(across, p[0]);
            for (int y = w - 1; y > level.getMinBuildHeight(); y--) {
                BlockPos b = new BlockPos(c.getX(), y, c.getZ());
                BlockState s = level.getBlockState(b);
                if (s.getFluidState().isEmpty() && !s.isAir() && !s.canBeReplaced()) {
                    break;
                }
                level.setBlock(b, block, 2 | 16);
            }
        }
    }

    /** A boat moored where the water's open and 2 deep over its hull's span, its bow out. */
    static boolean boat(ServerLevel level, String set, String part, BlockPos start, Direction out, int o0, int o1, int a0,
            int a1, int w) {
        Direction across = out.getClockWise();
        for (int a = a0; a <= a1; a++) {
            for (int o = o0; o <= o1; o++) {
                BlockPos c = start.relative(out, a).relative(across, o);
                if (water(level, c) != w || !level.getFluidState(new BlockPos(c.getX(), w - 1, c.getZ())).isSource()) {
                    return false;
                }
            }
        }
        return place(level, set, part, start, out, o0, o1, a0, a1, w);
    }
}
