package dev.b28.enterprise.territory;

import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.Warehouse;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;

/**
 * A town's trades turning raw goods into the goods they deal in, in the warehouse: Librarians and Cartographers press
 * sugar cane into paper while paper is short, Farmers bake wheat into bread while food is short, and Shepherds weave
 * wool into carpets while the town is short of furnishings. A conversion
 * runs only if a villager of its trade lives in the town (by its roster).
 */
public final class Workshop {
    /** {@code in} of {@code input} make {@code out} of {@code output}, done by any of {@code trades}. */
    public record Conversion(Set<String> trades, Item input, int in, Item output, int out) {}

    public static final List<Conversion> CONVERSIONS = List.of(
            new Conversion(Set.of("minecraft:librarian", "minecraft:cartographer"), Items.SUGAR_CANE, 1, Items.PAPER, 1),
            new Conversion(Set.of("minecraft:farmer"), Items.WHEAT, 3, Items.BREAD, 1),
            new Conversion(Set.of("minecraft:shepherd"), Items.WHITE_WOOL, 2, Items.WHITE_CARPET, 3),
            new Conversion(Set.of("minecraft:leatherworker"), Items.LEATHER, 4, Items.LEATHER_BOOTS, 1),
            new Conversion(Set.of("minecraft:fisherman"), Items.COD, 1, Items.COOKED_COD, 1));

    private Workshop() {}

    /** What the town's conversions could make from these inputs, by item id. */
    public static List<String> outputsOf(Set<String> inputs) {
        return CONVERSIONS.stream()
                .filter(c -> inputs.contains(BuiltInRegistries.ITEM.getKey(c.input()).toString()))
                .map(c -> BuiltInRegistries.ITEM.getKey(c.output()).toString())
                .toList();
    }

    /** Runs each conversion the town's trades can do, as far as its output is wanted. Returns items made. */
    public static int convert(ServerLevel level, Warehouse warehouse, GlobalPos desk, GlobalPos bell, MerchantStock stock,
            long now) {
        Set<String> trades = dev.b28.enterprise.settlement.Settlements.get(level.getServer()).roster(bell).values()
                .stream().map(dev.b28.enterprise.settlement.Settlements.Resident::profession).collect(Collectors.toSet());
        int made = 0;
        for (Conversion conversion : CONVERSIONS) {
            if (conversion.trades().stream().noneMatch(trades::contains)) {
                continue;
            }
            int wanted = conversion.output() == Items.WHITE_CARPET
                    ? comfortWanted(Territory.get(level.getServer()), desk, stock, "furnishings", conversion.output())
                    : conversion.output() == Items.LEATHER_BOOTS
                            ? comfortWanted(Territory.get(level.getServer()), desk, stock, "clothing", conversion.output())
                            : wanted(conversion.output(), warehouse, desk, stock, now);
            made += run(conversion, wanted, stock);
        }
        return made;
    }

    /**
     * Comforts a trade makes while the town is short of that kind, up to its par: carpets for furnishings, leather
     * boots for clothing.
     */
    static int comfortWanted(Territory territory, GlobalPos desk, MerchantStock stock, String kind, Item output) {
        double short_ = territory.comfortPar(desk) - Comforts.onHand(stock).getOrDefault(kind, 0.0);
        return short_ <= 0 ? 0 : (int) Math.ceil(short_ * dev.b28.enterprise.money.Prices.itemsPerEmerald(output));
    }

    /** How many more of {@code output} the town wants: paper up to its par when a locker buys it; bread for food. */
    static int wanted(Item output, Warehouse warehouse, GlobalPos desk, MerchantStock stock, long now) {
        if (output == Items.BREAD || output == Items.COOKED_COD) {
            return Math.max(0, (2 * Warehouse.FOOD_PAR_POINTS - stock.foodNutrition()) / 5);
        }
        if (warehouse.capacity(desk, output) == 0) {
            return 0;
        }
        return Math.max(0, warehouse.par(desk, output, true, now) - stock.count(new ItemCost(output, 1)));
    }

    public static int runForTest(Conversion conversion, int wanted, MerchantStock stock) {
        return run(conversion, wanted, stock);
    }

    /** Converts as many whole batches as cover {@code wanted}, as far as inputs and room allow. */
    static int run(Conversion conversion, int wanted, MerchantStock stock) {
        int batches = Math.min((wanted + conversion.out() - 1) / conversion.out(),
                stock.count(new ItemCost(conversion.input(), 1)) / conversion.in());
        if (batches <= 0) {
            return 0;
        }
        int taken = stock.take(new ItemCost(conversion.input(), 1), batches * conversion.in()).stream()
                .mapToInt(ItemStack::getCount).sum();
        int made = taken / conversion.in() * conversion.out();
        int left = made;
        while (left > 0) {
            int batch = Math.min(conversion.output().getDefaultMaxStackSize(), left);
            ItemStack rest = stock.insert(new ItemStack(conversion.output(), batch));
            left -= batch - rest.getCount();
            if (!rest.isEmpty()) {
                // No room: put back the input for what couldn't be stored.
                stock.insert(new ItemStack(conversion.input(), rest.getCount() / conversion.out() * conversion.in()));
                made -= rest.getCount();
                break;
            }
        }
        return made;
    }
}
