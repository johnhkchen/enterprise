package dev.b28.enterprise.territory;

import dev.b28.enterprise.merchant.MerchantStock;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.MobSpawnType;
import net.minecraft.world.entity.animal.Animal;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.FenceGateBlock;
import net.minecraft.world.phys.AABB;

/**
 * A town's herds, on the ledger: sheep, cows, pigs and chickens, counted per town and tended whenever the town is
 * touched. A Shepherd comes with a few sheep and a Butcher with a few pigs and chickens; the animals eat feed from the
 * warehouse (which the village's demand asks its farmers for), breed while fed up to what the pasture holds, and
 * dwindle while hungry. A Shepherd shears the flock; a Butcher slaughters stock beyond the breeding animals for meat
 * (and leather from cows), so a butcher's chain needs farmers for feed and a shepherd's flock for mutton. The pasture
 * is a fenced plot near the bell, its animals a view of the ledger, set right whenever the town is counted.
 */
public final class Husbandry {
    /** An animal kept: its entity, what it eats and how much a day, what slaughtering one gives. */
    public record Kind(String id, EntityType<? extends Animal> type, List<Item> feed, double feedPerDay, Item meat,
            double meatEach) {}

    public static final List<Kind> KINDS = List.of(
            new Kind("minecraft:sheep", EntityType.SHEEP, List.of(Items.WHEAT), 0.25, Items.MUTTON, 1.5),
            new Kind("minecraft:cow", EntityType.COW, List.of(Items.WHEAT), 0.25, Items.BEEF, 2),
            new Kind("minecraft:pig", EntityType.PIG, List.of(Items.CARROT, Items.POTATO, Items.BEETROOT), 0.25,
                    Items.PORKCHOP, 2),
            new Kind("minecraft:chicken", EntityType.CHICKEN, List.of(Items.WHEAT), 0.1, Items.CHICKEN, 1));

    /** A pasture holds this many animals in all. */
    public static final int PASTURE_CAPACITY = 16;

    /** A Butcher keeps this many of each kind to breed from, and slaughters half the rest a day. */
    public static final int BREEDING_STOCK = 4;
    public static final double SLAUGHTER_PER_DAY = 0.5;

    /** Fed animals breed by this share a day; hungry ones dwindle by this share. */
    public static final double BREED_PER_DAY = 0.15;
    public static final double STARVE_PER_DAY = 0.1;

    /** A shorn sheep gives this much wool a day; a slaughtered cow this much leather. */
    public static final double WOOL_PER_SHEEP_DAY = 0.5;
    public static final double LEATHER_PER_COW = 1;

    /** Pastures are 9 x 9, fenced, with a gate. */
    public static final int PASTURE_SIZE = 9;

    private Husbandry() {}

    public static Kind kind(String id) {
        return KINDS.stream().filter(k -> k.id().equals(id)).findFirst().orElseThrow();
    }

    /**
     * The town's first Shepherd brings 3 sheep, its first Butcher 2 pigs and 2 chickens. Once only: a flock that
     * starves away isn't given back.
     */
    public static void seed(Map<String, Double> herd, Set<String> trades) {
        if (trades.contains("minecraft:shepherd") && !herd.containsKey("minecraft:sheep")) {
            herd.put("minecraft:sheep", 3.0);
        }
        if (trades.contains("minecraft:butcher") && !herd.containsKey("minecraft:pig")
                && !herd.containsKey("minecraft:chicken")) {
            herd.put("minecraft:pig", 2.0);
            herd.put("minecraft:chicken", 2.0);
        }
    }

    /** What the herds eat a day, by feed item id: the village asks its farmers (and caravans) for it. */
    public static Map<String, Double> feedDemand(Map<String, Double> herd) {
        Map<String, Double> feed = new LinkedHashMap<>();
        herd.forEach((id, count) -> {
            Kind kind = kind(id);
            feed.merge(BuiltInRegistries.ITEM.getKey(kind.feed().get(0)).toString(), count * kind.feedPerDay(), Double::sum);
        });
        return feed;
    }

    /**
     * Tends the herds for {@code days}: they eat from the warehouse, breed or dwindle, are shorn and slaughtered.
     * Products (wool, meat, leather) are added to {@code produce}, owed to the warehouse with the farms' harvest.
     */
    public static void tend(MerchantStock stock, Map<String, Double> herd, Set<String> trades, double days,
            Map<String, Double> produce) {
        if (days <= 0) {
            return;
        }
        double total = herd.values().stream().mapToDouble(Double::doubleValue).sum();
        for (Kind kind : KINDS) {
            double count = herd.getOrDefault(kind.id(), 0.0);
            if (count <= 0) {
                continue;
            }
            int wanted = (int) Math.ceil(count * kind.feedPerDay() * days);
            int eaten = 0;
            for (Item feed : kind.feed()) {
                if (eaten >= wanted) {
                    break;
                }
                eaten += stock.take(new ItemCost(feed, 1), wanted - eaten).stream().mapToInt(ItemStack::getCount).sum();
            }
            double fed = wanted == 0 ? 1 : (double) eaten / wanted;
            if (fed >= 0.9) {
                double room = Math.max(0, PASTURE_CAPACITY - total);
                double born = Math.min(room, count * BREED_PER_DAY * days);
                count += born;
                total += born;
            } else {
                double lost = count * STARVE_PER_DAY * (1 - fed) * days;
                count -= lost;
                total -= lost;
            }
            if (kind.id().equals("minecraft:sheep") && trades.contains("minecraft:shepherd")) {
                produce.merge("minecraft:white_wool", count * WOOL_PER_SHEEP_DAY * days, Double::sum);
            }
            if (trades.contains("minecraft:butcher") && count > BREEDING_STOCK) {
                double slaughtered = Math.min(count - BREEDING_STOCK, (count - BREEDING_STOCK) * SLAUGHTER_PER_DAY * days);
                count -= slaughtered;
                total -= slaughtered;
                produce.merge(BuiltInRegistries.ITEM.getKey(kind.meat()).toString(), slaughtered * kind.meatEach(), Double::sum);
                if (kind.id().equals("minecraft:cow")) {
                    produce.merge("minecraft:leather", slaughtered * LEATHER_PER_COW, Double::sum);
                }
            }
            herd.put(kind.id(), Math.max(0, count));
        }
    }

    /** A fenced 9 x 9 pasture with its corner at ground level, a gate in the middle of its south side. */
    public static void fence(ServerLevel level, BlockPos corner) {
        for (int dx = 0; dx < PASTURE_SIZE; dx++) {
            for (int dz = 0; dz < PASTURE_SIZE; dz++) {
                boolean edge = dx == 0 || dz == 0 || dx == PASTURE_SIZE - 1 || dz == PASTURE_SIZE - 1;
                BlockPos ground = corner.offset(dx, 0, dz);
                level.setBlock(ground, Blocks.GRASS_BLOCK.defaultBlockState(), 3);
                if (edge) {
                    boolean gate = dz == PASTURE_SIZE - 1 && dx == PASTURE_SIZE / 2;
                    level.setBlock(ground.above(), gate
                            ? Blocks.OAK_FENCE_GATE.defaultBlockState().setValue(FenceGateBlock.FACING, Direction.SOUTH)
                            : Blocks.OAK_FENCE.defaultBlockState(), 3);
                }
            }
        }
    }

    /**
     * Sets the pasture's animals right by the ledger: spawns the ones it's missing of each kind, removes ones it no
     * longer has (slaughtered, starved). Only whole animals show.
     */
    public static void sync(ServerLevel level, BlockPos corner, Map<String, Double> herd) {
        AABB inside = new AABB(corner.above()).expandTowards(PASTURE_SIZE - 1, 2, PASTURE_SIZE - 1);
        for (Kind kind : KINDS) {
            int wanted = (int) Math.floor(herd.getOrDefault(kind.id(), 0.0));
            List<Animal> there = level.getEntitiesOfClass(Animal.class, inside, animal -> animal.getType() == kind.type());
            for (int i = there.size(); i < wanted; i++) {
                Animal animal = kind.type().create(level);
                if (animal == null) {
                    break;
                }
                BlockPos spot = corner.offset(1 + level.random.nextInt(PASTURE_SIZE - 2), 1,
                        1 + level.random.nextInt(PASTURE_SIZE - 2));
                animal.moveTo(spot.getX() + 0.5, spot.getY(), spot.getZ() + 0.5, level.random.nextFloat() * 360, 0);
                animal.finalizeSpawn(level, level.getCurrentDifficultyAt(spot), MobSpawnType.BREEDING, null);
                animal.setPersistenceRequired();
                level.addFreshEntity(animal);
            }
            for (int i = wanted; i < there.size(); i++) {
                there.get(i).discard();
            }
        }
    }

    static boolean any(Map<String, Double> herd) {
        return herd.values().stream().anyMatch(count -> count >= 1);
    }
}
