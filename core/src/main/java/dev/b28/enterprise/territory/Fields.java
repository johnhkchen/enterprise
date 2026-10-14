package dev.b28.enterprise.territory;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;

/**
 * Fields a town lays out for its farmers when they have hands to spare, growing what its market wants: crops its
 * lockers buy, sugar cane when its Librarians and Cartographers want paper (the warehouse presses cane into paper),
 * or carrots when it's short of food for caravans. A field is 9 x 9 on open ground near the bell.
 */
public final class Fields {
    public static final int SIZE = 9;

    /** A field's seed and labour, in emeralds. */
    public static final double FIELD_EMERALDS = 4;

    /** Farmers with room for this many more plots get a field. */
    public static final int SPARE_PLOTS = 54;

    /** What can be planted, and what a locker would take for it (cane is pressed into paper first). */
    private static final Map<Item, Item> SOLD_AS = Map.of(
            Items.WHEAT, Items.WHEAT, Items.CARROT, Items.CARROT, Items.POTATO, Items.POTATO,
            Items.BEETROOT, Items.BEETROOT, Items.SUGAR_CANE, Items.PAPER);
    private static final List<Item> CROPS = List.of(Items.WHEAT, Items.CARROT, Items.POTATO, Items.BEETROOT, Items.SUGAR_CANE);

    private Fields() {}

    /**
     * What a new field should grow: the crop whose sale (as itself, or as paper for cane) the town's lockers can take
     * the most of a day, by value; carrots if there's no market for any but the warehouse is short of food; else
     * nothing.
     */
    public static Optional<Item> choose(Warehouse warehouse, GlobalPos desk, MerchantStock stock) {
        Item best = null;
        double bestValue = 0;
        for (Item crop : CROPS) {
            Item sold = SOLD_AS.get(crop);
            double value = warehouse.capacity(desk, sold) * Prices.emeraldsEach(sold);
            if (value > bestValue) {
                best = crop;
                bestValue = value;
            }
        }
        if (best == null && stock.foodNutrition() < Warehouse.FOOD_PAR_POINTS * 2) {
            best = Items.CARROT;
        }
        return Optional.ofNullable(best);
    }

    /**
     * After a census while loaded: farmers with room for {@link #SPARE_PLOTS} more plots than they work get a field of
     * what the market wants, on open ground in the town's land, for {@link #FIELD_EMERALDS} beyond the reserve.
     */
    public static Optional<BlockPos> consider(ServerLevel level, BlockPos bell, GlobalPos desk, MerchantStock stock,
            Territory territory, List<Integer> farmers, double plotsWorked) {
        int capacity = farmers.stream().mapToInt(l -> Territory.PLOTS_PER_FARMER_LEVEL * Math.max(1, l)).sum();
        if (capacity - plotsWorked < SPARE_PLOTS) {
            return Optional.empty();
        }
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        if (Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk) < FIELD_EMERALDS) {
            return Optional.empty();
        }
        Optional<Item> crop = choose(Warehouse.get(level.getServer()), desk, stock);
        if (crop.isEmpty()) {
            return Optional.empty();
        }
        GlobalPos home = GlobalPos.of(level.dimension(), bell);
        Optional<BlockPos> site = Growth.site(level, bell, SIZE, 8)
                .filter(corner -> territory.owner(level.dimension(), dev.b28.enterprise.hex.Hex.at(corner.offset(4, 0, 4)))
                        .filter(home::equals).isPresent());
        if (site.isEmpty()) {
            return Optional.empty();
        }
        plant(level, site.get(), crop.get());
        Treasury.withdraw(level, desk.pos(), stock, FIELD_EMERALDS, Mint.get(level.getServer()), level.getGameTime());
        Enterprise.LOGGER.debug("Settlement at {} plants a field of {} at {}", bell.toShortString(),
                net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(crop.get()), site.get().toShortString());
        return site;
    }

    /**
     * Plants a 9 x 9 field with its corner at ground level. Crops: farmland round a water block in the middle (it
     * waters 4 blocks out), seedlings on it. Sugar cane: three water channels with cane along both banks.
     */
    public static void plant(ServerLevel level, BlockPos corner, Item crop) {
        for (int dx = 0; dx < SIZE; dx++) {
            for (int dz = 0; dz < SIZE; dz++) {
                BlockPos ground = corner.offset(dx, 0, dz);
                if (crop == Items.SUGAR_CANE) {
                    if (dz % 3 == 1) {
                        level.setBlock(ground, Blocks.WATER.defaultBlockState(), 3);
                    } else {
                        level.setBlock(ground, Blocks.GRASS_BLOCK.defaultBlockState(), 3);
                        level.setBlock(ground.above(), Blocks.SUGAR_CANE.defaultBlockState(), 3);
                    }
                } else if (dx == SIZE / 2 && dz == SIZE / 2) {
                    level.setBlock(ground, Blocks.WATER.defaultBlockState(), 3);
                } else {
                    level.setBlock(ground, Blocks.FARMLAND.defaultBlockState(), 3);
                    level.setBlock(ground.above(), seedling(crop), 3);
                }
            }
        }
    }

    private static BlockState seedling(Item crop) {
        if (crop == Items.CARROT) {
            return Blocks.CARROTS.defaultBlockState();
        } else if (crop == Items.POTATO) {
            return Blocks.POTATOES.defaultBlockState();
        } else if (crop == Items.BEETROOT) {
            return Blocks.BEETROOTS.defaultBlockState();
        }
        return Blocks.WHEAT.defaultBlockState();
    }

    /**
     * The warehouse presses sugar cane into paper, one for one as at a crafting table, while paper is below its par.
     * Returns the paper made.
     */
    public static int press(Warehouse warehouse, GlobalPos desk, MerchantStock stock, long now) {
        int paper = stock.count(new ItemCost(Items.PAPER, 1));
        int want = warehouse.par(desk, Items.PAPER, warehouse.capacity(desk, Items.PAPER) > 0, now) - paper;
        int cane = stock.count(new ItemCost(Items.SUGAR_CANE, 1));
        int pressed = Math.min(Math.max(0, want), cane);
        if (pressed <= 0 || warehouse.capacity(desk, Items.PAPER) == 0) {
            return 0;
        }
        int taken = stock.take(new ItemCost(Items.SUGAR_CANE, 1), pressed).stream().mapToInt(ItemStack::getCount).sum();
        int left = taken;
        while (left > 0) {
            ItemStack rest = stock.insert(new ItemStack(Items.PAPER, Math.min(64, left)));
            left -= Math.min(64, left) - rest.getCount();
            if (!rest.isEmpty()) {
                stock.insert(new ItemStack(Items.SUGAR_CANE, rest.getCount())); // no room: the cane stays cane
                taken -= rest.getCount();
                break;
            }
        }
        return taken;
    }
}
