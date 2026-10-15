package dev.b28.enterprise.territory;

import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Treasury;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

/**
 * A town's households: what its farmers are paid doesn't leave the world, it's theirs. Wages go into their purse, and
 * they spend it on what they want, paying the Merchant: the comforts they use, homes for their families and children,
 * land of their own. So wages are money in new hands and demand at the market, and a Merchant who stocks what the town
 * wants earns it back.
 *
 * A Merchant short of coin pays in credit (the town owes its farmers), up to {@link #CREDIT_DAYS} days of a customary
 * wage (half the crop's worth): the
 * harvest still comes in. Beyond that, nobody works on credit; the days unpaid are unworked, as before. Credit is
 * spent first on land: a household with {@link #LAND_EMERALDS} in purse and credit buys a field of its own (a
 * freehold), where there are farmers with hands to work it. On freehold plots the farmers are owners, not hands: they
 * take at least {@link #FREEHOLD_SHARE} of what the crop is worth.
 *
 * The wage itself follows the labour market: the farmers' share moves toward the share of the harvest that sells,
 * times how scarce hands are (see {@link #tightness}).
 */
public final class Households {
    /** Days of pay farmers will wait for, as credit, before they stop working unpaid days. */
    public static final double CREDIT_DAYS = 10;
    /** What a field of one's own costs a household: the field's seed and labour, and the land. */
    public static final double LAND_EMERALDS = 8;
    /** On freehold plots the farmers take at least this share of the crop's worth. */
    public static final double FREEHOLD_SHARE = 0.75;
    /** Hands scarce (plots going untended) raise the farmers' share up to this much; idle hands lower it this far. */
    public static final double TIGHTEST = 1.5;
    public static final double SLACKEST = 0.5;

    private Households() {}

    /**
     * How scarce farm hands are, as a factor on the wage: above 1 with plots going untended (as far as
     * {@link #TIGHTEST} when as many go untended as are worked), below 1 with hands idle (as low as {@link #SLACKEST}
     * when half are), 1 with none of either.
     */
    public static double tightness(double capacity, double tended, double untended) {
        if (capacity <= 0) {
            return 1;
        }
        if (untended > 0) {
            return Math.min(TIGHTEST, 1 + untended / capacity);
        }
        return Math.max(SLACKEST, Math.min(1, tended / capacity));
    }

    /** The farmers' share on their own plots and on the estate's, by the share of plots that are theirs. */
    public static double share(double laborShare, double freeholdShare) {
        double f = Math.max(0, Math.min(1, freeholdShare));
        return (1 - f) * laborShare + f * Math.max(laborShare, FREEHOLD_SHARE);
    }

    /**
     * The households pay the Merchant: whole emeralds into the chest, the rest as coins (or into the Banker). Returns
     * what went in, in emeralds; what found no room stays in the purse.
     */
    public static double payIn(ServerLevel level, BlockPos desk, MerchantStock stock, double emeralds, double rate) {
        if (emeralds <= 0) {
            return 0;
        }
        int whole = (int) Math.floor(emeralds);
        int put = 0;
        if (whole > 0) {
            ItemStack rest = stock.insert(new ItemStack(Items.EMERALD, whole));
            put = whole - rest.getCount();
        }
        int spurs = (int) Math.floor((emeralds - put) * rate);
        if (spurs > 0) {
            Treasury.deposit(level, desk, stock, spurs);
        }
        return put + spurs / rate;
    }
}
