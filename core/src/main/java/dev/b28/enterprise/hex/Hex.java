package dev.b28.enterprise.hex;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.BlockPos;

/**
 * A hex of the world grid, in axial coordinates (pointy-top). The hex is the ledger's unit of place: ownership,
 * security, tolls and route lengths are per hex. A hex is {@link #ACROSS} blocks across its flat sides, about the
 * core of a naturally generated village.
 */
public record Hex(int q, int r) {
    /** Blocks across a hex, flat side to flat side. */
    public static final double ACROSS = 80;

    /** Centre to corner. */
    public static final double SIZE = ACROSS / Math.sqrt(3);

    private static final int[][] DIRECTIONS = {{1, 0}, {1, -1}, {0, -1}, {-1, 0}, {-1, 1}, {0, 1}};

    public static Hex at(BlockPos pos) {
        return at(pos.getX() + 0.5, pos.getZ() + 0.5);
    }

    public static Hex at(double x, double z) {
        double q = (Math.sqrt(3) / 3 * x - z / 3) / SIZE;
        double r = (2.0 / 3 * z) / SIZE;
        return round(q, r);
    }

    public double centerX() {
        return SIZE * Math.sqrt(3) * (q + r / 2.0);
    }

    public double centerZ() {
        return SIZE * 1.5 * r;
    }

    public int s() {
        return -q - r;
    }

    public int distance(Hex other) {
        return (Math.abs(q - other.q) + Math.abs(r - other.r) + Math.abs(s() - other.s())) / 2;
    }

    public List<Hex> neighbors() {
        List<Hex> around = new ArrayList<>(6);
        for (int[] d : DIRECTIONS) {
            around.add(new Hex(q + d[0], r + d[1]));
        }
        return around;
    }

    /** Every hex on the straight line from {@code a} to {@code b}, both included. */
    public static List<Hex> line(Hex a, Hex b) {
        int n = a.distance(b);
        List<Hex> hexes = new ArrayList<>(n + 1);
        // Nudge off the exact edges so ties round the same way every time.
        double aq = a.q + 1e-6;
        double ar = a.r + 1e-6;
        double bq = b.q + 1e-6;
        double br = b.r + 1e-6;
        for (int i = 0; i <= n; i++) {
            double t = n == 0 ? 0 : (double) i / n;
            hexes.add(round(aq + (bq - aq) * t, ar + (br - ar) * t));
        }
        return hexes;
    }

    private static Hex round(double q, double r) {
        double s = -q - r;
        long rq = Math.round(q);
        long rr = Math.round(r);
        long rs = Math.round(s);
        double dq = Math.abs(rq - q);
        double dr = Math.abs(rr - r);
        double ds = Math.abs(rs - s);
        if (dq > dr && dq > ds) {
            rq = -rr - rs;
        } else if (dr > ds) {
            rr = -rq - rs;
        }
        return new Hex((int) rq, (int) rr);
    }

    @Override
    public String toString() {
        return "(" + q + ", " + r + ")";
    }
}
