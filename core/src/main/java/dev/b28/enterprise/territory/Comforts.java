package dev.b28.enterprise.territory;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Prices;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.tags.TagKey;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.trading.ItemCost;

/**
 * What villagers enjoy, beyond bread and a bed: treats, clothing, furnishings, reading. Each kind is an item tag under
 * {@code enterprise:comforts/}, so a pack adds a kind (cigarettes, tea, finished clothes from another mod) with a tag.
 * A town's villagers use up a ration of each kind every day from its warehouse; the kinds they get make the town's
 * mood. Comforts are the town's way to spend money it doesn't need: they come by caravan and are used up.
 */
public final class Comforts {
    private static final String PREFIX = "comforts/";

    /** Each villager uses this much of each kind a day, in emeralds' worth at local prices. */
    public static final double RATION_EMERALDS = 0.1;

    // A town keeps comfort_days of rations of each kind in its warehouse (see Dials).

    private Comforts() {}

    /** Every kind of comfort: the item tags under {@code enterprise:comforts/}, by name. */
    public static List<TagKey<Item>> kinds() {
        return BuiltInRegistries.ITEM.getTagNames()
                .filter(tag -> tag.location().getNamespace().equals(Enterprise.MOD_ID)
                        && tag.location().getPath().startsWith(PREFIX))
                .sorted(Comparator.comparing(tag -> tag.location().toString()))
                .toList();
    }

    /** The kind's short name: "treats" for {@code enterprise:comforts/treats}. */
    public static String name(TagKey<Item> kind) {
        return kind.location().getPath().substring(PREFIX.length());
    }

    public static Optional<TagKey<Item>> kind(String name) {
        return Optional.of(TagKey.create(Registries.ITEM, Enterprise.id(PREFIX + name)));
    }

    /** Is this a comfort? Reads the item's own tags, so it's cheap enough for every slot of a warehouse. */
    public static boolean isComfort(ItemStack stack) {
        return stack.getTags().anyMatch(tag -> tag.location().getNamespace().equals(Enterprise.MOD_ID)
                && tag.location().getPath().startsWith(PREFIX));
    }

    /** The kind of comfort this item is, if any. */
    public static Optional<TagKey<Item>> kindOf(Item item) {
        return kinds().stream().filter(kind -> item.builtInRegistryHolder().is(kind)).findFirst();
    }

    /** What the warehouse holds of each kind, in emeralds' worth. */
    public static Map<String, Double> onHand(MerchantStock stock) {
        Map<String, Double> worth = new LinkedHashMap<>();
        kinds().forEach(kind -> worth.put(name(kind), 0.0));
        stock.totals().forEach((id, count) -> {
            Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(id));
            kindOf(item).ifPresent(kind -> worth.merge(name(kind), count * Prices.emeraldsEach(item), Double::sum));
        });
        return worth;
    }

    /**
     * Uses up to {@code emeralds}' worth of a kind from the warehouse, cheapest goods first, whole items only (a cake
     * may cover more than one day's ration). Returns the worth used.
     */
    public static double use(MerchantStock stock, TagKey<Item> kind, double emeralds) {
        List<Item> goods = stock.totals().keySet().stream()
                .map(id -> BuiltInRegistries.ITEM.get(ResourceLocation.parse(id)))
                .filter(item -> item.builtInRegistryHolder().is(kind))
                .sorted(Comparator.comparingDouble(Prices::emeraldsEach))
                .toList();
        double used = 0;
        for (Item item : goods) {
            double each = Prices.emeraldsEach(item);
            while (used < emeralds - 1e-9) {
                List<ItemStack> taken = stock.take(new ItemCost(item, 1), 1);
                if (taken.isEmpty() || taken.get(0).isEmpty()) {
                    break;
                }
                used += each;
            }
        }
        return used;
    }
}
