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
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;

/**
 * Fields a town lays out for its farmers when they have hands to spare, growing what its villagers would buy: the
 * crop with the most demand (from every resident's crop-for-emerald trades, the farmers' own among them) it doesn't
 * already grow, sugar cane counting as the paper Librarians and Cartographers buy (see {@link Workshop}), sweet
 * berries for Master Butchers; or carrots when it's short of food for caravans. A field is 9 x 9 on open ground near
 * the bell.
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
            Items.BEETROOT, Items.BEETROOT, Items.SUGAR_CANE, Items.PAPER, Items.SWEET_BERRIES, Items.SWEET_BERRIES);
    private static final List<Item> CROPS = List.of(
            Items.WHEAT, Items.CARROT, Items.POTATO, Items.BEETROOT, Items.SUGAR_CANE, Items.SWEET_BERRIES);

    private Fields() {}

    /**
     * What a new field should grow: the crop whose sale (as itself, or as paper for cane) the village's demand most
     * exceeds what the town already grows, by value; carrots if nothing's wanted but the warehouse is short of food;
     * else nothing.
     */
    public static Optional<Item> choose(Map<String, Double> demand, Map<String, Double> grown, MerchantStock stock) {
        return choose(demand, grown, stock, Map.of());
    }

    /**
     * As {@link #choose(Map, Map, MerchantStock)}, but feed the herds need ({@code needs}, items a day) and the town
     * doesn't grow enough of comes first: starving animals before anything sells.
     */
    public static Optional<Item> choose(Map<String, Double> demand, Map<String, Double> grown, MerchantStock stock,
            Map<String, Double> needs) {
        for (Item crop : CROPS) {
            String id = BuiltInRegistries.ITEM.getKey(crop).toString();
            if (needs.getOrDefault(id, 0.0) > grown.getOrDefault(id, 0.0)) {
                return Optional.of(crop);
            }
        }
        Item best = null;
        double bestValue = 0;
        for (Item crop : CROPS) {
            String sold = BuiltInRegistries.ITEM.getKey(SOLD_AS.get(crop)).toString();
            double already = CROPS.stream()
                    .filter(c -> SOLD_AS.get(c) == SOLD_AS.get(crop))
                    .mapToDouble(c -> grown.getOrDefault(BuiltInRegistries.ITEM.getKey(c).toString(), 0.0))
                    .sum();
            double value = (demand.getOrDefault(sold, 0.0) - already) * Prices.emeraldsEach(SOLD_AS.get(crop));
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
            Territory territory, List<Integer> farmers, double plotsWorked, Map<String, Double> demand,
            Map<String, Double> grown) {
        int capacity = farmers.stream().mapToInt(l -> Territory.PLOTS_PER_FARMER_LEVEL * Math.max(1, l)).sum();
        // A crop the village would buy and the town doesn't grow at all gets a field even if the farmers are busy
        // (they split their time): a farmer always plants for its own trades. Up to two fields a farmer level.
        int levels = farmers.stream().mapToInt(l -> Math.max(1, l)).sum();
        boolean unmetCrop = CROPS.stream().anyMatch(crop -> {
            String sold = BuiltInRegistries.ITEM.getKey(SOLD_AS.get(crop)).toString();
            return demand.getOrDefault(sold, 0.0) > 0 && CROPS.stream()
                    .filter(c -> SOLD_AS.get(c) == SOLD_AS.get(crop))
                    .noneMatch(c -> grown.getOrDefault(BuiltInRegistries.ITEM.getKey(c).toString(), 0.0) > 0);
        });
        boolean spareHands = capacity - plotsWorked >= SPARE_PLOTS;
        Map<String, Double> feed = Husbandry.feedDemand(territory.herd(bellPos(level, bell)));
        unmetCrop |= feed.entrySet().stream().anyMatch(e -> e.getValue() > grown.getOrDefault(e.getKey(), 0.0));
        if (farmers.isEmpty() || !(spareHands || unmetCrop && territory.fieldsPlanted(bellPos(level, bell)) < 2 * levels)) {
            return Optional.empty();
        }
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        if (Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk) < FIELD_EMERALDS) {
            return Optional.empty();
        }
        Map<String, Double> needs = Husbandry.feedDemand(territory.herd(bellPos(level, bell)));
        Optional<Item> crop = choose(demand, grown, stock, needs);
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
        territory.notePlanted(bellPos(level, bell));
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
                if (crop == Items.SWEET_BERRIES) {
                    level.setBlock(ground, Blocks.GRASS_BLOCK.defaultBlockState(), 3);
                    level.setBlock(ground.above(), Blocks.SWEET_BERRY_BUSH.defaultBlockState(), 3);
                } else if (crop == Items.SUGAR_CANE) {
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

    private static GlobalPos bellPos(ServerLevel level, BlockPos bell) {
        return GlobalPos.of(level.dimension(), bell);
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
}
