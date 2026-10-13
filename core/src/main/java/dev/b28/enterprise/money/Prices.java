package dev.b28.enterprise.money;

import java.util.Map;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.item.Item;

/**
 * Local prices, in items per emerald: what villagers pay for goods and crops, and what they charge for food. One
 * table for farm wages, caravan wholesale and road food. Unknown items are valued at 32 to the emerald.
 */
public final class Prices {
    private static final double DEFAULT = 32;

    private static final Map<String, Double> ITEMS_PER_EMERALD = new java.util.HashMap<>(Map.ofEntries(
            // Crops villagers buy (farmers; clerics buy nether wart)
            Map.entry("minecraft:wheat", 20.0),
            Map.entry("minecraft:carrot", 22.0),
            Map.entry("minecraft:potato", 26.0),
            Map.entry("minecraft:beetroot", 15.0),
            Map.entry("minecraft:pumpkin", 6.0),
            Map.entry("minecraft:melon", 4.0),
            Map.entry("minecraft:nether_wart", 22.0),
            Map.entry("minecraft:sugar_cane", 24.0), // as paper
            Map.entry("minecraft:cocoa_beans", 20.0),
            Map.entry("minecraft:sweet_berries", 10.0),
            // Goods villagers buy
            Map.entry("minecraft:paper", 24.0),
            Map.entry("minecraft:book", 4.0),
            Map.entry("minecraft:ink_sac", 5.0),
            Map.entry("minecraft:writable_book", 2.0),
            Map.entry("minecraft:stick", 32.0),
            Map.entry("minecraft:flint", 26.0),
            Map.entry("minecraft:string", 14.0),
            Map.entry("minecraft:feather", 24.0),
            Map.entry("minecraft:tripwire_hook", 8.0),
            Map.entry("minecraft:glass_pane", 11.0),
            Map.entry("minecraft:compass", 1.0),
            Map.entry("minecraft:coal", 15.0),
            Map.entry("minecraft:clay_ball", 10.0),
            Map.entry("minecraft:leather", 6.0),
            // Food villagers sell
            Map.entry("minecraft:bread", 6.0),
            Map.entry("minecraft:apple", 4.0),
            Map.entry("minecraft:cookie", 18.0),
            Map.entry("minecraft:cooked_porkchop", 5.0),
            Map.entry("minecraft:cooked_chicken", 8.0),
            Map.entry("minecraft:baked_potato", 8.0),
            // Comforts (what villagers sell them for, or near it)
            Map.entry("minecraft:cake", 1.0),
            Map.entry("minecraft:pumpkin_pie", 4.0),
            Map.entry("minecraft:honey_bottle", 3.0),
            Map.entry("minecraft:leather_helmet", 0.2),
            Map.entry("minecraft:leather_chestplate", 1 / 7.0),
            Map.entry("minecraft:leather_leggings", 1 / 3.0),
            Map.entry("minecraft:leather_boots", 0.25),
            Map.entry("minecraft:painting", 0.5),
            Map.entry("minecraft:flower_pot", 4.0),
            Map.entry("minecraft:lantern", 1.0),
            Map.entry("minecraft:candle", 3.0)));

    static {
        // Every colour of carpet and candle: carpets 4 to the emerald (shepherds), dyed candles as plain ones.
        for (net.minecraft.world.item.DyeColor color : net.minecraft.world.item.DyeColor.values()) {
            ITEMS_PER_EMERALD.put("minecraft:" + color.getName() + "_carpet", 4.0);
            ITEMS_PER_EMERALD.put("minecraft:" + color.getName() + "_candle", 3.0);
        }
    }

    private Prices() {}

    public static double itemsPerEmerald(String itemId) {
        return ITEMS_PER_EMERALD.getOrDefault(itemId, DEFAULT);
    }

    public static double itemsPerEmerald(Item item) {
        return itemsPerEmerald(BuiltInRegistries.ITEM.getKey(item).toString());
    }

    /** One item's local value in emeralds. */
    public static double emeraldsEach(Item item) {
        return 1 / itemsPerEmerald(item);
    }

    public static boolean isListed(String itemId) {
        return ITEMS_PER_EMERALD.containsKey(itemId);
    }
}
