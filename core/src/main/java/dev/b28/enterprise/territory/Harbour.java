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
    public record Laid(boolean sea, int quay, int jetty, int boats) {}

    /** A shore cell: the land's last column, its way out over the water. */
    record Shore(BlockPos at, Direction out) {}

    /** Lays the town's harbour at its tier ("hamlet", "village", "town", ...), if its plan meets water. */
    public static java.util.Optional<Laid> lay(ServerLevel level, TownPlans.Plan plan, String kingdom, String tier) {
        BlockPos bell = plan.bell();
        // The head: the first water along an avenue from the forum's edge, the nearest to the bell.
        Shore head = null;
        int nearest = Integer.MAX_VALUE;
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
                if (water(level, c) != Integer.MIN_VALUE) {
                    if (k < nearest) {
                        nearest = k;
                        head = new Shore(bell.relative(way, k - 1), way);
                    }
                    break;
                }
            }
        }
        if (head == null) {
            return java.util.Optional.empty();
        }
        int w = water(level, head.at().relative(head.out()));
        boolean sea = openWater(level, head.at().relative(head.out()), plan);
        String set = level.getServer().getResourceManager()
                .getResource(net.minecraft.resources.ResourceLocation.parse("enterprise:structure/" + kingdom + "/harbour/jetty_timber_3x4.json"))
                .isPresent() ? kingdom : "lordaeron";
        int rank = switch (tier) {
            case "hamlet" -> 0;
            case "village" -> 1;
            default -> 2;
        };
        int quay = 0, jetty = 0, boats = 0;
        // The quay along the shore from the head, both ways.
        if (rank >= 1) {
            int length = rank >= 2 ? 32 : sea ? 16 : 8;
            List<Shore> run = shore(level, head, length, w);
            for (int k = 0; k < run.size(); k++) {
                Shore s = run.get(k);
                String tile = (sea ? "quay_stone_" : "quay_timber_") + (k == 0 ? "1a"
                        : k % 8 == 4 ? "ladder_1" : k % 5 == 2 ? "lamp_1" : sea && k % 3 == 1 ? "bollard_1"
                        : "1" + (char) ('a' + Math.floorMod(s.at().getX() * 31 + s.at().getZ(), sea ? 3 : 2)));
                if (quayTile(level, set, tile, s, w)) {
                    quay++;
                }
            }
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
        return java.util.Optional.of(new Laid(sea, quay, jetty, boats));
    }

    /** The water's surface height at a column, or {@link Integer#MIN_VALUE} if its top isn't water. */
    static int water(ServerLevel level, BlockPos c) {
        int y = level.getHeight(net.minecraft.world.level.levelgen.Heightmap.Types.MOTION_BLOCKING, c.getX(), c.getZ()) - 1;
        return level.getFluidState(new BlockPos(c.getX(), y, c.getZ())).isSource() ? y : Integer.MIN_VALUE;
    }

    /** Is the water open sea: 2,000 columns or more round it, or reaching past the plan's outer edge? */
    static boolean openWater(ServerLevel level, BlockPos from, TownPlans.Plan plan) {
        Set<Long> seen = new HashSet<>();
        ArrayDeque<BlockPos> q = new ArrayDeque<>();
        q.add(from);
        seen.add(from.asLong());
        TownPlans.Rect o = plan.outer();
        while (!q.isEmpty() && seen.size() < 2000) {
            BlockPos c = q.pop();
            if (!o.contains(c.getX(), c.getZ())) {
                return true;
            }
            for (Direction d : Direction.Plane.HORIZONTAL) {
                BlockPos n = c.relative(d);
                if (level.isLoaded(n) && !seen.contains(n.asLong()) && water(level, n) != Integer.MIN_VALUE) {
                    seen.add(n.asLong());
                    q.add(n);
                }
            }
        }
        return seen.size() >= 2000;
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
