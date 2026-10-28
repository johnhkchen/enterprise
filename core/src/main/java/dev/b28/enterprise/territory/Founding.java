package dev.b28.enterprise.territory;

import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.levelgen.Heightmap;

/**
 * What a town founded on open ground stands with before its houses come (the studio's founding kit, batch 034, thread
 * 0012), laid once its land is shaped, in its kingdom's set ({@code structure/<kingdom>/founding/}, Lordaeron's for a
 * kingdom with none): the charter stone at the forum's edge facing the square, the notice board by the bell, the well at
 * the forum's edge, the settlers' camp at the main street's end, gate posts astride it at the inner wall's line, and a
 * stake every 8 along the rest of that line. The forum's centrepiece (the market cross) is the forum's own.
 */
public final class Founding {
    private Founding() {}

    /** How far apart the stakes stand along the inner wall's line. */
    static final int STAKES = 8;

    /** Lays the kit round the town's square. The parts laid. */
    public static int lay(ServerLevel level, TownPlans.Plan plan, String kingdom, Direction main) {
        String set = level.getServer().getResourceManager()
                .getResource(Kits.template("enterprise:" + kingdom + "/founding/well_3").withPrefix("structure/").withSuffix(".json"))
                .isPresent() ? kingdom : "lordaeron";
        BlockPos bell = plan.bell();
        int r = Forums.REACH + 1, laid = 0;
        Direction back = main.getOpposite(), side = main.getClockWise();
        // The charter stone at the forum's edge across from the main street, facing the square; the notice board at the
        // edge to one side of the bell, the well to the other side by the main street.
        laid += part(level, set, "founding_stone_3", bell.relative(back, r + 2).relative(side, 3), 3, 3, main);
        laid += part(level, set, "notice_board_3", bell.relative(side, r + 1), 3, 1, side.getOpposite());
        // The well at the forum's edge beside the charter stone, off the main street's plots (r78: by the main street it
        // took a plot the first builds wanted).
        laid += part(level, set, "well_3", bell.relative(back, r + 2).relative(side.getOpposite(), 3), 3, 3, main);
        // The gate posts astride the main street at the inner wall's line, the camp just inside them beside it.
        int out = switch (main) {
            case NORTH -> bell.getZ() - plan.inner().z0();
            case SOUTH -> plan.inner().z1() - bell.getZ();
            case WEST -> bell.getX() - plan.inner().x0();
            default -> plan.inner().x1() - bell.getX();
        };
        laid += part(level, set, "gate_posts_5", bell.relative(main, out), 5, 1, back);
        laid += part(level, set, "camp_9x7", bell.relative(main, out - 6).relative(side, TownPlans.GATE / 2 + 6), 9, 7,
                side.getOpposite());
        // A stake every 8 along the rest of the inner wall's line, clear of the avenues.
        TownPlans.Rect in = plan.inner();
        for (int x = in.x0(); x <= in.x1(); x += STAKES) {
            laid += stake(level, set, plan, x, in.z0()) + stake(level, set, plan, x, in.z1());
        }
        for (int z = in.z0() + STAKES; z < in.z1(); z += STAKES) {
            laid += stake(level, set, plan, in.x0(), z) + stake(level, set, plan, in.x1(), z);
        }
        return laid;
    }

    private static int stake(ServerLevel level, String set, TownPlans.Plan plan, int x, int z) {
        if (Math.abs(x - plan.bell().getX()) <= TownPlans.GATE / 2 + 2 || Math.abs(z - plan.bell().getZ()) <= TownPlans.GATE / 2 + 2) {
            return 0; // an avenue's gate
        }
        return part(level, set, "stake_1", new BlockPos(x, 0, z), 1, 1, Direction.SOUTH);
    }

    /**
     * One part, centred on {@code centre} (its width across {@code front}'s way, its depth along it), its front to
     * {@code front}, on the ground there. 1 if laid.
     */
    private static int part(ServerLevel level, String set, String name, BlockPos centre, int width, int depth, Direction front) {
        boolean across = front.getAxis() == Direction.Axis.Z;
        int sx = across ? width : depth, sz = across ? depth : width;
        int x0 = centre.getX() - sx / 2, z0 = centre.getZ() - sz / 2;
        int y = Integer.MIN_VALUE;
        for (int dx = 0; dx < sx; dx++) {
            for (int dz = 0; dz < sz; dz++) {
                if (!level.isLoaded(new BlockPos(x0 + dx, 0, z0 + dz))) {
                    return 0;
                }
                y = Math.max(y, Terraform.ground(level, x0 + dx, z0 + dz));
            }
        }
        return Kits.place(level, "enterprise:" + set + "/founding/" + name, new BlockPos(x0, y, z0), front).isPresent() ? 1 : 0;
    }
}
