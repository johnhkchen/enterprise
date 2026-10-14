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
            Map.entry("minecraft:raw_iron", 5.0),
            // What hands gather (no villager buys most of it; caravans carry it where it's wanted)
            Map.entry("minecraft:oak_log", 8.0),
            Map.entry("minecraft:spruce_log", 8.0),
            Map.entry("minecraft:jungle_log", 8.0),
            Map.entry("minecraft:acacia_log", 8.0),
            Map.entry("minecraft:sand", 32.0),
            Map.entry("minecraft:red_sand", 32.0),
            Map.entry("minecraft:cactus", 16.0),
            Map.entry("minecraft:cobblestone", 32.0),
            Map.entry("minecraft:snowball", 32.0),
            Map.entry("minecraft:dandelion", 16.0),
            Map.entry("minecraft:poppy", 16.0),
            Map.entry("minecraft:wheat_seeds", 32.0),
            Map.entry("minecraft:iron_ingot", 4.0),
            Map.entry("minecraft:diamond", 1.0), // an Expert smith buys one for an emerald
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
            Map.entry("minecraft:candle", 3.0),
            // Smiths' goods (what villagers sell them for, or near it)
            Map.entry("minecraft:iron_hoe", 1.0),
            Map.entry("minecraft:iron_pickaxe", 2 / 3.0),
            Map.entry("minecraft:iron_sword", 2 / 3.0),
            Map.entry("minecraft:iron_chestplate", 1 / 9.0),
            Map.entry("minecraft:bow", 0.5),
            Map.entry("minecraft:arrow", 16.0),
            Map.entry("minecraft:wooden_sword", 2.0),
            Map.entry("minecraft:stone_sword", 1.0),
            Map.entry("minecraft:diamond_sword", 1 / 12.0),
            Map.entry("minecraft:chainmail_chestplate", 0.25),
            Map.entry("minecraft:diamond_chestplate", 1 / 16.0),
            Map.entry("minecraft:shield", 0.2),
            Map.entry("minecraft:raw_gold", 3.0),
            Map.entry("minecraft:gold_ingot", 3.0),
            Map.entry("minecraft:saddle", 1 / 6.0),
            Map.entry("minecraft:white_banner", 1 / 3.0),
            Map.entry("minecraft:enchanted_book", 0.1),
            Map.entry("minecraft:lapis_lazuli", 1.0),
            // Game and meat
            Map.entry("minecraft:rabbit", 8.0),
            Map.entry("minecraft:rabbit_hide", 9.0),
            Map.entry("minecraft:beef", 8.0),
            Map.entry("minecraft:porkchop", 7.0),
            Map.entry("minecraft:chicken", 14.0),
            Map.entry("minecraft:mutton", 7.0),
            Map.entry("minecraft:cooked_beef", 5.0),
            Map.entry("minecraft:cooked_mutton", 5.0),
            Map.entry("minecraft:cooked_rabbit", 5.0)));

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
