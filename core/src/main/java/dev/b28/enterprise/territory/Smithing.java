package dev.b28.enterprise.territory;

import dev.b28.enterprise.merchant.MerchantStock;
import java.util.List;
import java.util.Map;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;

/**
 * What a town's iron becomes, and what it's for. Farmers work with hoes and miners (its Toolsmiths) with pickaxes;
 * its guards carry swords (or bows) and wear armour, and its Fletchers hunt with bows. Each worker uses up one of its kit in so many days (tools last twice as
 * long while a Toolsmith mends them). With tools a farmer tends half again as many plots and a miner brings up half
 * again as much ore; armed guards let a town ask more of the detour as its toll, armoured ones a higher toll.
 * Kit is held in the warehouse; what share of its wearers it covers is measured whenever the town is touched.
 */
public final class Smithing {
    /** Who wears a kind of kit: bows are for its hunters (Fletchers) and its guards. */
    public enum Wearer { FARMERS, MINERS, GUARDS, HUNTERS_AND_GUARDS }

    /** A kind of kit: the item, who uses it, how many days one lasts, and what it's made of. */
    public record Kit(Item item, Wearer wearer, double days, Map<Item, Integer> inputs, boolean tool) {
        public String id() {
            return BuiltInRegistries.ITEM.getKey(item).toString();
        }
    }

    public static final Kit HOE = new Kit(Items.IRON_HOE, Wearer.FARMERS, 16, Map.of(Items.IRON_INGOT, 2), true);
    public static final Kit PICKAXE = new Kit(Items.IRON_PICKAXE, Wearer.MINERS, 16, Map.of(Items.IRON_INGOT, 3), true);
    public static final Kit SWORD = new Kit(Items.IRON_SWORD, Wearer.GUARDS, 20, Map.of(Items.IRON_INGOT, 2), false);
    public static final Kit ARMOUR = new Kit(Items.IRON_CHESTPLATE, Wearer.GUARDS, 40, Map.of(Items.IRON_INGOT, 8), false);
    public static final Kit BOW = new Kit(Items.BOW, Wearer.HUNTERS_AND_GUARDS, 20,
            Map.of(Items.STICK, 3, Items.STRING, 3), false);
    public static final List<Kit> KITS = List.of(HOE, PICKAXE, SWORD, ARMOUR, BOW);

    /** A Fletcher without a bow still hunts with snares, at this share. */
    public static final double SNARES = 0.25;

    /** Smelting: this much raw iron and a coal make as many iron ingots. */
    public static final int RAW_PER_COAL = 8;

    /** With tools for all, farmers and miners do this much more. */
    public static final double TOOL_BONUS = 0.5;

    /** Armed guards: a town asks up to this share of the detour (from {@link Chokepoints#DETOUR_SHARE}). */
    public static final double ARMED_DETOUR_SHARE = 0.95;

    /** Armoured guards: a town's toll may go up to this (from {@link Territory#MAX_TOLL_RATE}). */
    public static final double ARMOURED_TOLL_RATE = 0.35;

    private Smithing() {}

    public static int wearers(Kit kit, int farmers, int miners, int guards, int hunters) {
        return switch (kit.wearer()) {
            case FARMERS -> farmers;
            case MINERS -> miners;
            case GUARDS -> guards;
            case HUNTERS_AND_GUARDS -> hunters + guards;
        };
    }

    /** Share of the hunters with a bow: bows go to the hunters first. */
    public static double hunting(MerchantStock stock, int hunters) {
        return hunters <= 0 ? 0 : Math.min(1, stock.count(new ItemCost(Items.BOW, 1)) / (double) hunters);
    }

    /**
     * Share of the guards armed: those with swords, and with the bows the hunters don't need; at least
     * {@code floor} (archers on a watchtower).
     */
    public static double armed(MerchantStock stock, int guards, int hunters, double floor) {
        if (guards <= 0) {
            return 0;
        }
        int bows = Math.max(0, stock.count(new ItemCost(Items.BOW, 1)) - hunters);
        return Math.max(floor, Math.min(1, (stock.count(new ItemCost(Items.IRON_SWORD, 1)) + bows) / (double) guards));
    }

    /** Share of the wearers this kit covers, from the warehouse: 0 to 1 (0 with no wearers). */
    public static double share(MerchantStock stock, Kit kit, int wearers) {
        return wearers <= 0 ? 0 : Math.min(1, stock.count(new ItemCost(kit.item(), 1)) / (double) wearers);
    }

    /**
     * Kit wears out over {@code days}: each wearer with kit uses up a share of one, by kind, carried in {@code wear};
     * whole ones are taken from the warehouse. Tools wear half as fast while {@code mended}. Returns items used up.
     */
    public static int wear(MerchantStock stock, Map<String, Double> wear, double days, int farmers, int miners,
            int guards, boolean mended) {
        return wear(stock, wear, days, farmers, miners, guards, 0, mended ? 0.5 : 1, 1);
    }

    /**
     * As above, with {@code hunters}; tools wear at {@code toolWear} of their rate (mended), all kit at
     * {@code kitWear} (a smithy keeps it).
     */
    public static int wear(MerchantStock stock, Map<String, Double> wear, double days, int farmers, int miners,
            int guards, int hunters, double toolWear, double kitWear) {
        int used = 0;
        for (Kit kit : KITS) {
            int held = stock.count(new ItemCost(kit.item(), 1));
            double equipped = Math.min(held, wearers(kit, farmers, miners, guards, hunters));
            if (equipped <= 0) {
                wear.remove(kit.id()); // nothing in use: nothing wearing out
                continue;
            }
            double owed = wear.getOrDefault(kit.id(), 0.0)
                    + equipped * days / kit.days() * (kit.tool() ? toolWear : 1) * kitWear;
            int whole = (int) Math.min(held, Math.floor(owed));
            if (whole > 0) {
                used += stock.take(new ItemCost(kit.item(), 1), whole).stream().mapToInt(ItemStack::getCount).sum();
            }
            wear.put(kit.id(), owed - whole);
        }
        return used;
    }
}
