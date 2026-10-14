package dev.b28.enterprise.caravan;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.animal.Animal;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.phys.AABB;

/**
 * A caravan's squad: one to four trained villagers from its home town, who leave the world while on the road and come
 * back with the caravan. One is enough to drive the wagon; each more goes only when its trade helps this trip, and
 * each eats its own road food, so the town must be able to spare it. A Merchant haggles at its own level (without one
 * the squad haggles as a Novice), a Cartographer maps a route not mapped lately, a Librarian brings back the books
 * when the destination's are stale, and a Shepherd drives animals from the market pen to sell. Babies and untrained
 * villagers stay home, and so does the Merchant who works the town's desk.
 */
public final class Squad {
    public static final int MAX = 4;

    /** How far from its desk a town's squad is looked for: the town. Only its community is mustered. */
    public static final int SEARCH = 48;

    /** Villagers standing this close to the desk are in its market, and count as its community. */
    public static final int MARKET = 8;

    public static int reach() {
        return SEARCH;
    }

    /** Which trades this trip has a use for, beyond its lead: stale books, an unmapped road, animals to drive. */
    public record Needs(boolean books, boolean mapping, boolean animals) {
        public static final Needs ALL = new Needs(true, true, true);

        boolean wants(String trade) {
            return switch (trade) {
                case "minecraft:librarian" -> books;
                case "minecraft:cartographer" -> mapping;
                case "minecraft:shepherd" -> animals;
                default -> false;
            };
        }
    }

    /** A Shepherd drives at most this many animals, from within this many blocks of the desk (the market pen). */
    public static final int HERD = 8;
    public static final int PEN_REACH = 8;

    /** The trades a squad is made of, in the order a squad is filled. */
    public static final List<String> TRADES = List.of(
            "enterprise:merchant", "minecraft:cartographer", "minecraft:librarian", "minecraft:shepherd");

    /** What an animal fetches, in emeralds. */
    public static final Map<String, Double> ANIMAL_VALUE = Map.of(
            "minecraft:sheep", 1.0, "minecraft:cow", 1.0, "minecraft:pig", 0.5, "minecraft:chicken", 0.25);

    private Squad() {}

    public static String trade(Villager villager) {
        return BuiltInRegistries.VILLAGER_PROFESSION.getKey(villager.getVillagerData().getProfession()).toString();
    }

    /** A squad member's trade, read from its saved data. */
    public static String trade(CompoundTag member) {
        return member.getCompound("VillagerData").getString("profession");
    }

    public static int level(CompoundTag member) {
        return Math.max(1, member.getCompound("VillagerData").getInt("level"));
    }

    /** Can this villager go with a caravan from {@code desk}? A trained adult of a squad trade, not the desk's Merchant. */
    public static boolean eligible(Villager villager, GlobalPos desk) {
        if (villager.isBaby() || !TRADES.contains(trade(villager))) {
            return false;
        }
        return villager.getBrain().getMemory(MemoryModuleType.JOB_SITE).filter(desk::equals).isEmpty();
    }

    /**
     * Who would go: a lead (the most experienced Merchant, else the first of the other trades in {@link #TRADES}
     * order), then one of each trade this trip {@code needs}, at most {@code extras} more.
     */
    public static List<Villager> muster(ServerLevel level, GlobalPos desk, int reach, Needs needs, int extras) {
        List<Villager> candidates = available(level, desk, reach);
        List<Villager> squad = new ArrayList<>();
        if (candidates.isEmpty()) {
            return squad;
        }
        squad.add(candidates.get(0));
        for (String trade : TRADES) {
            if (squad.size() > extras || !needs.wants(trade) || squad.stream().anyMatch(v -> trade(v).equals(trade))) {
                continue;
            }
            candidates.stream().filter(v -> trade(v).equals(trade)).findFirst().ifPresent(squad::add);
        }
        return squad;
    }

    /**
     * The desk's community, by villager: the residents of its town, and those who have gone out on its caravans
     * before. With the workers at the lockers its Merchant manages and anyone in its market, these are the villagers
     * its Merchant knows, and only they are mustered.
     */
    public static java.util.Set<java.util.UUID> community(ServerLevel level, GlobalPos desk) {
        var server = level.getServer();
        java.util.Set<java.util.UUID> ids = new java.util.HashSet<>(CaravanLedger.get(server).served(desk));
        dev.b28.enterprise.territory.Territory.get(server).bellOf(desk).ifPresent(bell ->
                ids.addAll(dev.b28.enterprise.settlement.Settlements.get(server).roster(bell).keySet()));
        return ids;
    }

    /** Is this villager one the desk's Merchant knows: its community, a worker at one of its lockers, or in its market? */
    public static boolean connected(Villager villager, GlobalPos desk, java.util.Set<java.util.UUID> community,
            java.util.Set<BlockPos> lockerSites) {
        return community.contains(villager.getUUID())
                || villager.getBrain().getMemory(MemoryModuleType.JOB_SITE)
                        .filter(site -> site.dimension().equals(desk.dimension()) && lockerSites.contains(site.pos()))
                        .isPresent()
                || villager.blockPosition().closerThan(desk.pos(), MARKET);
    }

    /**
     * Villagers of the squad trades the desk's Merchant knows (see {@link #connected}) within {@code reach}, who could
     * go, in {@link #TRADES} order, most experienced first.
     */
    public static List<Villager> available(ServerLevel level, GlobalPos desk, int reach) {
        var community = community(level, desk);
        var lockerSites = dev.b28.enterprise.merchant.MerchantBook.get(level.getServer()).workstations(desk);
        return level.getEntitiesOfClass(Villager.class, new AABB(desk.pos()).inflate(reach),
                        villager -> eligible(villager, desk) && connected(villager, desk, community, lockerSites))
                .stream()
                .sorted(Comparator.comparingInt((Villager v) -> TRADES.indexOf(trade(v)))
                        .thenComparing(Comparator.comparingInt((Villager v) -> v.getVillagerData().getLevel()).reversed())
                        .thenComparing(Villager::getStringUUID))
                .toList();
    }

    /** Takes an entity off the world into a ledger record, to be put back by {@link #release}. */
    public static CompoundTag stow(Entity entity) {
        CompoundTag tag = new CompoundTag();
        entity.save(tag);
        entity.discard();
        return tag;
    }

    /** Puts a stowed entity back in the world by {@code near} (on open ground beside it if there is some). */
    public static Optional<Entity> release(ServerLevel level, BlockPos near, CompoundTag tag) {
        BlockPos at = openGroundBeside(level, near);
        Entity entity = EntityType.loadEntityRecursive(tag, level, e -> {
            e.moveTo(at.getX() + 0.5, at.getY(), at.getZ() + 0.5, e.getYRot(), e.getXRot());
            return e;
        });
        if (entity == null || level.getEntity(entity.getUUID()) != null) {
            return Optional.empty(); // already back somehow: don't duplicate it
        }
        level.addFreshEntity(entity);
        return Optional.of(entity);
    }

    private static BlockPos openGroundBeside(ServerLevel level, BlockPos near) {
        for (int ring = 1; ring <= 3; ring++) {
            for (Direction side : Direction.Plane.HORIZONTAL) {
                BlockPos spot = near.relative(side, ring);
                if (level.getBlockState(spot).isAir() && level.getBlockState(spot.above()).isAir()
                        && level.getBlockState(spot.below()).isFaceSturdy(level, spot.below(), Direction.UP)) {
                    return spot;
                }
            }
        }
        return near.above();
    }

    public static boolean has(List<CompoundTag> crew, String trade) {
        return crew.stream().anyMatch(member -> trade(member).equals(trade));
    }

    /** The level the squad haggles at: its best Merchant's, or a Novice's. */
    public static int haggleLevel(List<CompoundTag> crew) {
        return crew.stream().filter(m -> trade(m).equals("enterprise:merchant")).mapToInt(Squad::level).max().orElse(1);
    }

    /** Grown farm animals in the market pen, up to {@link #HERD}, for a Shepherd to drive. */
    public static List<Animal> herd(ServerLevel level, BlockPos desk) {
        return level.getEntitiesOfClass(Animal.class, new AABB(desk).inflate(PEN_REACH),
                        animal -> !animal.isBaby() && !animal.isLeashed() && !animal.hasCustomName()
                                && ANIMAL_VALUE.containsKey(EntityType.getKey(animal.getType()).toString()))
                .stream()
                .limit(HERD)
                .toList();
    }

    public static double value(CompoundTag animal) {
        return ANIMAL_VALUE.getOrDefault(animal.getString("id"), 0.0);
    }

    /** Grown animals of the kinds a Shepherd drives near a town's bell: its pasture. */
    public static int pasture(ServerLevel level, BlockPos bell) {
        return level.getEntitiesOfClass(Animal.class, new AABB(bell).inflate(32),
                animal -> ANIMAL_VALUE.containsKey(EntityType.getKey(animal.getType()).toString())).size();
    }
}
