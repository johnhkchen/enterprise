package dev.b28.enterprise.territory;

import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.settlement.Settlements;
import java.util.List;
import java.util.Map;
import net.minecraft.core.Holder;
import net.minecraft.core.QuartPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BiomeTags;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.biome.Biome;
import net.neoforged.neoforge.common.Tags;

/**
 * Plain work for a town's hands without a trade: its Nitwits, and adults waiting for work. Each works one of the
 * town's hexes, gathering what the land there gives: logs and sticks in woods, sand in deserts, clay by water, stone
 * and flint in hills, seeds and flowers on open ground. These are the goods no trade makes, which other trades buy
 * (sticks and string for Fletchers, clay for Masons) and the town needs (timber, fuel). Nitwits are tireless at it.
 * When the town posts guards, its hands serve as militia first: they stand watch for their food, without pay, and
 * gather nothing while they do.
 */
public final class Labour {
    public static final String NITWIT = "minecraft:nitwit";
    public static final String NONE = "minecraft:none";

    /** A Nitwit gathers this much more than another villager would. */
    public static final double NITWIT_FACTOR = 1.5;

    /** A good is gathered only while the warehouse holds fewer than this. */
    public static final int KEEP = 128;

    /** Something gathered: so many a day per hand. */
    public record Haul(Item item, double perDay) {}

    private Labour() {}

    /** Is this a trade without work of its own? */
    public static boolean idle(String profession) {
        return profession.equals(NITWIT) || profession.equals(NONE);
    }

    /** What a hand gathers a day on land of this biome. */
    public static List<Haul> hauls(Holder<Biome> biome) {
        if (biome.is(BiomeTags.IS_OCEAN) || biome.is(BiomeTags.IS_DEEP_OCEAN) || biome.is(BiomeTags.IS_RIVER)) {
            return List.of(new Haul(Items.CLAY_BALL, 4), new Haul(Items.SUGAR_CANE, 2), new Haul(Items.INK_SAC, 1));
        }
        if (biome.is(BiomeTags.IS_BEACH)) {
            return List.of(new Haul(Items.SAND, 12), new Haul(Items.CLAY_BALL, 2));
        }
        if (biome.is(BiomeTags.IS_BADLANDS)) {
            return List.of(new Haul(Items.RED_SAND, 16), new Haul(Items.STICK, 2));
        }
        if (biome.is(Tags.Biomes.IS_DESERT)) {
            return List.of(new Haul(Items.SAND, 16), new Haul(Items.CACTUS, 2), new Haul(Items.STICK, 2));
        }
        if (biome.is(Tags.Biomes.IS_SWAMP)) {
            return List.of(new Haul(Items.CLAY_BALL, 4), new Haul(Items.OAK_LOG, 2), new Haul(Items.STRING, 1));
        }
        if (biome.is(BiomeTags.IS_JUNGLE)) {
            return List.of(new Haul(Items.JUNGLE_LOG, 5), new Haul(Items.COCOA_BEANS, 3), new Haul(Items.STICK, 2));
        }
        if (biome.is(BiomeTags.IS_SAVANNA)) {
            return List.of(new Haul(Items.ACACIA_LOG, 5), new Haul(Items.STICK, 2), new Haul(Items.STRING, 0.5));
        }
        if (biome.is(BiomeTags.IS_MOUNTAIN) || biome.is(BiomeTags.IS_HILL)) {
            return List.of(new Haul(Items.COBBLESTONE, 16), new Haul(Items.FLINT, 3));
        }
        if (biome.is(Tags.Biomes.IS_SNOWY)) {
            return List.of(new Haul(Items.SNOWBALL, 8), new Haul(Items.SPRUCE_LOG, 2));
        }
        if (biome.is(BiomeTags.IS_TAIGA)) {
            return List.of(new Haul(Items.SPRUCE_LOG, 6), new Haul(Items.SWEET_BERRIES, 3), new Haul(Items.STICK, 2));
        }
        if (biome.is(BiomeTags.IS_FOREST)) {
            return List.of(new Haul(Items.OAK_LOG, 6), new Haul(Items.STICK, 4), new Haul(Items.APPLE, 1),
                    new Haul(Items.STRING, 0.5));
        }
        // Open ground: plains, meadows and the like.
        return List.of(new Haul(Items.WHEAT_SEEDS, 4), new Haul(Items.DANDELION, 2), new Haul(Items.POPPY, 2),
                new Haul(Items.STICK, 2), new Haul(Items.FLINT, 1), new Haul(Items.OAK_LOG, 1), new Haul(Items.STRING, 0.5));
    }

    /** The biome at a hex's centre, from the world's generator: known whether or not it's loaded. */
    public static Holder<Biome> biome(ServerLevel level, Hex hex) {
        var source = level.getChunkSource();
        return source.getGenerator().getBiomeSource().getNoiseBiome(
                QuartPos.fromBlock((int) Math.floor(hex.centerX())), QuartPos.fromBlock(level.getSeaLevel()),
                QuartPos.fromBlock((int) Math.floor(hex.centerZ())), source.randomState().sampler());
    }

    /** The town's hands at home, by weight: a Nitwit counts {@link #NITWIT_FACTOR}, another idle adult 1. */
    public static List<Double> hands(Map<java.util.UUID, Settlements.Resident> roster) {
        return roster.values().stream()
                .filter(r -> !r.away() && !r.baby() && idle(r.profession()))
                .sorted(java.util.Comparator.comparing((Settlements.Resident r) -> !r.profession().equals(NITWIT))
                        .thenComparing(r -> r.id().toString()))
                .map(r -> r.profession().equals(NITWIT) ? NITWIT_FACTOR : 1.0)
                .toList();
    }

    /**
     * {@code days} of the hands' work, but for the last {@code militia} (who stand watch: the Nitwits gather longest):
     * each works the next of the town's hexes, home first, and what it gathers is owed to the warehouse in {@code pending}, while the warehouse
     * holds less than {@link #KEEP} of it.
     */
    public static void work(ServerLevel level, List<Hex> hexes, List<Double> hands, int militia, MerchantStock stock,
            double days, Map<String, Double> pending) {
        if (hexes.isEmpty() || days <= 0) {
            return;
        }
        for (int i = 0; i < hands.size() - militia; i++) {
            Hex hex = hexes.get(i % hexes.size());
            for (Haul haul : hauls(biome(level, hex))) {
                if (stock.count(new ItemCost(haul.item(), 1)) >= KEEP) {
                    continue;
                }
                pending.merge(BuiltInRegistries.ITEM.getKey(haul.item()).toString(),
                        haul.perDay() * hands.get(i) * days, Double::sum);
            }
        }
    }
}
