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
 * when the destination's are stale, a Shepherd drives animals from the market pen to sell, a Leatherworker carries
 * packs (more cargo), and a Fisherman captains a boat across water, fishing it (a fleet of Fishermen sells fish).
 * A Cleric, a Farmer or a smith goes along when the town there wants their services (a blessing, help with the
 * harvest, smelting, tools, arms), but never leads. Each trade's work at the far end is a service (see {@link dev.b28.enterprise.service.Services}).
 * Villagers without a trade go as porters, each carrying as much as a Leatherworker's packs, when there's more
 * cargo than that; they walk home once it's delivered. Babies stay home, and so does the Merchant who works the
 * town's desk.
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

    /**
     * Which trades this trip has a use for, beyond its lead: those whose services the destination wants (see
     * {@link dev.b28.enterprise.service.Services#wantedAbroad}), packs for more cargo than a wagon holds, and whether
     * there's water a boat would sail better than the land way (then Fishermen go, and fish).
     */
    public record Needs(java.util.Set<String> trades, boolean water, int porters) {
        public Needs(java.util.Set<String> trades, boolean water) {
            this(trades, water, 0);
        }

        /** The old three: stale books (a Librarian), an unmapped road (a Cartographer), animals to drive (a Shepherd). */
        public Needs(boolean books, boolean mapping, boolean animals) {
            this(of(books, mapping, animals), false);
        }

        private static java.util.Set<String> of(boolean books, boolean mapping, boolean animals) {
            java.util.Set<String> trades = new java.util.LinkedHashSet<>();
            if (books) {
                trades.add("minecraft:librarian");
            }
            if (mapping) {
                trades.add("minecraft:cartographer");
            }
            if (animals) {
                trades.add("minecraft:shepherd");
            }
            return trades;
        }

        public static final Needs ALL = new Needs(java.util.Set.copyOf(TRADES), true, MAX);

        boolean wants(String trade) {
            return trades.contains(trade) || trade.equals(FISHERMAN) && water;
        }
    }

    /** A Shepherd drives at most this many animals, from within this many blocks of the desk (the market pen). */
    public static final int HERD = 8;
    public static final int PEN_REACH = 8;

    public static final String FISHERMAN = "minecraft:fisherman";
    public static final String LEATHERWORKER = "minecraft:leatherworker";

    /** The trades that can lead a squad, and that towns train for their caravans. */
    public static final List<String> LEADS = List.of(
            "enterprise:merchant", "minecraft:cartographer", "minecraft:librarian", "minecraft:shepherd",
            LEATHERWORKER, FISHERMAN);

    /** Every trade that can go with a caravan: the leads, and those who go only when their service is wanted there. */
    public static final List<String> TRADES = List.of(
            "enterprise:merchant", "minecraft:cartographer", "minecraft:librarian", "minecraft:shepherd",
            LEATHERWORKER, FISHERMAN, "minecraft:cleric", "minecraft:farmer", "minecraft:toolsmith", "minecraft:armorer",
            "minecraft:weaponsmith", "minecraft:mason", "minecraft:fletcher", "minecraft:butcher");

    /** A Leatherworker's packs carry this many more stacks. */
    public static final int PACK_STACKS = 9;
    /** Llamas from the town's pen go along as pack animals (not for sale): this many stacks each, at most so many. */
    public static final int LLAMA_STACKS = 9;
    public static final int LLAMAS = 4;

    /** Cod each Fisherman aboard catches on each water hex the caravan sails. */
    public static final int FISH_PER_WATER_HEX = 8;

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
        if (villager.isBaby() || !TRADES.contains(trade(villager)) && !porter(trade(villager))) {
            return false;
        }
        var site = villager.getBrain().getMemory(MemoryModuleType.JOB_SITE);
        if (site.filter(desk::equals).isPresent()) {
            return false; // the Merchant who works the desk
        }
        // And whoever works a locker: the market's trade goes on through them.
        return site.filter(at -> at.dimension() == villager.level().dimension()
                && villager.level() instanceof net.minecraft.server.level.ServerLevel level
                && !dev.b28.enterprise.locker.Lockers.touchingContainers(level, at.pos()).isEmpty()).isEmpty();
    }

    /**
     * Who would go: a lead (the most experienced Merchant, else the first of the other trades in {@link #LEADS}
     * order), then one of each trade this trip {@code needs}, at most {@code extras} more.
     */
    public static List<Villager> muster(ServerLevel level, GlobalPos desk, int reach, Needs needs, int extras) {
        List<Villager> candidates = available(level, desk, reach);
        List<Villager> squad = new ArrayList<>();
        // By water, a Fisherman leads: the boat needs its captain more than the wagon needs anyone else.
        if (needs.water()) {
            candidates.stream().filter(v -> trade(v).equals(FISHERMAN)).findFirst().ifPresent(squad::add);
        }
        if (squad.isEmpty()) {
            candidates.stream().filter(v -> LEADS.contains(trade(v))).findFirst().ifPresent(squad::add);
        }
        if (squad.isEmpty()) {
            return squad; // a Cleric or a Farmer goes along, but doesn't drive the wagon
        }
        for (String trade : TRADES) {
            if (squad.size() > extras || squad.size() >= MAX || !needs.wants(trade)
                    || squad.stream().anyMatch(v -> trade(v).equals(trade))) {
                continue;
            }
            candidates.stream().filter(v -> trade(v).equals(trade)).findFirst().ifPresent(squad::add);
        }
        // Porters for cargo beyond the wagon and the packs, Nitwits first.
        candidates.stream().filter(v -> porter(trade(v)) && !squad.contains(v))
                .limit(Math.max(0, Math.min(needs.porters(), Math.min(MAX, extras + 1) - squad.size())))
                .forEach(squad::add);
        if (needs.water()) {
            // A fishing fleet: on water, every Fisherman to spare goes and fishes, up to a full squad.
            candidates.stream().filter(v -> trade(v).equals(FISHERMAN) && !squad.contains(v))
                    .limit(Math.max(0, Math.min(MAX, extras + 1) - squad.size()))
                    .forEach(squad::add);
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
                .sorted(Comparator.comparingInt((Villager v) -> order(trade(v)))
                        .thenComparing(Comparator.comparingInt((Villager v) -> v.getVillagerData().getLevel()).reversed())
                        .thenComparing(Villager::getStringUUID))
                .toList();
    }

    /** A villager without a trade (a Nitwit, or an adult waiting for work) can carry: a porter. */
    public static boolean porter(String trade) {
        return dev.b28.enterprise.territory.Labour.idle(trade);
    }

    /** Does this squad member carry cargo beyond the wagon: a Leatherworker's packs, or a porter's back? */
    public static boolean carries(String trade) {
        return trade.equals(LEATHERWORKER) || porter(trade);
    }

    /** Where a trade comes in the squad's order: {@link #TRADES}, then porters (Nitwits first). */
    static int order(String trade) {
        int at = TRADES.indexOf(trade);
        return at >= 0 ? at : trade.equals(dev.b28.enterprise.territory.Labour.NITWIT) ? TRADES.size() : TRADES.size() + 1;
    }

    /** Of these, those who could lead a squad. */
    public static List<Villager> leads(List<Villager> villagers) {
        return villagers.stream().filter(v -> LEADS.contains(trade(v))).toList();
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

    /** Grown llamas in the pen by the desk, free to carry packs. */
    public static List<Animal> llamas(ServerLevel level, BlockPos desk) {
        return level.getEntitiesOfClass(Animal.class, new AABB(desk).inflate(PEN_REACH),
                        animal -> !animal.isBaby() && !animal.isLeashed()
                                && animal instanceof net.minecraft.world.entity.animal.horse.Llama)
                .stream()
                .limit(LLAMAS)
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
