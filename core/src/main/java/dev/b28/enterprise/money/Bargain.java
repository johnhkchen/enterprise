package dev.b28.enterprise.money;

/**
 * Two Merchants settling a price for one item worth {@code value} emeralds locally. The seller won't go below half
 * the value; the buyer will pay up to the full value when its shelf is empty, scaling down to half at par (no deal
 * at or above par). The price lands in that gap by level: the seller takes level / (its level + the buyer's) of it.
 */
public final class Bargain {
    /** No Merchant sells below this share of an item's local value. */
    public static final double SELLER_FLOOR = 0.5;

    private Bargain() {}

    /** What the buyer will pay at most, given how short of par it is (0 at par, 1 with nothing on hand). */
    public static double buyerCeiling(double value, double shortage) {
        return value * (SELLER_FLOOR + (1 - SELLER_FLOOR) * Math.max(0, Math.min(1, shortage)));
    }

    public static double sellerFloor(double value) {
        return value * SELLER_FLOOR;
    }

    /** The settled price, or -1 if the buyer's ceiling doesn't clear the seller's floor. */
    public static double price(double value, double shortage, int sellerLevel, int buyerLevel) {
        return between(sellerFloor(value), buyerCeiling(value, shortage), sellerLevel, buyerLevel);
    }

    /** The price between a seller's floor and a buyer's ceiling, split by level; -1 if there's no gap. */
    public static double between(double floor, double ceiling, int sellerLevel, int buyerLevel) {
        if (ceiling <= floor) {
            return -1;
        }
        double sellerShare = (double) Math.max(1, sellerLevel) / (Math.max(1, sellerLevel) + Math.max(1, buyerLevel));
        return floor + (ceiling - floor) * sellerShare;
    }

    /** Shortage below par: 1 with nothing on hand, 0 at or above par. */
    public static double shortage(int onHand, int par) {
        return par <= 0 ? 0 : Math.max(0, 1 - (double) onHand / par);
    }
}
