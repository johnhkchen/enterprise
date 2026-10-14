package dev.b28.enterprise.merchant;

import dev.b28.enterprise.locker.Lockers;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.ArmorItem;
import net.minecraft.world.item.ArrowItem;
import net.minecraft.world.item.BowItem;
import net.minecraft.world.item.CrossbowItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.ShieldItem;
import net.minecraft.world.item.SwordItem;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Warehouses beyond the desk: buildings a market keeps its goods in. Every container inside a building's floorplan
 * is one store, part of its market's stock (no chest-per-workstation: the building is the store). What a building is
 * for decides what it holds: an armoury keeps war goods (weapons, armour, shields, bows and arrows), a barracks keeps
 * war goods and rations for the men it houses, a storehouse keeps anything. Goods go first to a building meant for
 * them, then to the desk's own chests and the storehouses; a building never takes goods it isn't for.
 */
public final class Depots extends SavedData {
    private static final String NAME = "enterprise_depots";

    public enum Kind {
        ARMOURY, BARRACKS, STOREHOUSE, COLD_STORE;

        /** Is this building meant for these goods? A cold store keeps packed crates: goods out of use, nine to a slot. */
        public boolean holds(ItemStack stack) {
            return switch (this) {
                case STOREHOUSE -> true;
                case ARMOURY -> warGoods(stack);
                case BARRACKS -> warGoods(stack) || stack.has(net.minecraft.core.component.DataComponents.FOOD);
                case COLD_STORE -> dev.b28.enterprise.cargo.Crates.isCrate(stack);
            };
        }

        /** Does it take these goods before the desk's chests do? Only a building meant for them in particular. */
        public boolean prefers(ItemStack stack) {
            return this != STOREHOUSE && holds(stack);
        }

        public String label() {
            return name().toLowerCase(Locale.ROOT);
        }
    }

    public static boolean warGoods(ItemStack stack) {
        var item = stack.getItem();
        return item instanceof SwordItem || item instanceof net.minecraft.world.item.AxeItem || item instanceof ArmorItem
                || item instanceof ShieldItem
                || item instanceof BowItem || item instanceof CrossbowItem || item instanceof ArrowItem;
    }

    /**
     * A building: the market it belongs to, what it's for, its floorplan (corners, inclusive), and the beds in it when
     * it was registered (a barracks houses men under arms).
     */
    public record Depot(int id, GlobalPos desk, Kind kind, BlockPos min, BlockPos max, int bedsBuilt) {
        /** The containers inside its floorplan, a double chest once. */
        public List<BlockPos> containers(ServerLevel level) {
            List<BlockPos> found = new ArrayList<>();
            for (BlockPos pos : BlockPos.betweenClosed(min, max)) {
                if (level.getBlockState(pos).is(Lockers.CONTAINERS)) {
                    BlockPos key = Lockers.canonicalContainer(pos.immutable(), level.getBlockState(pos));
                    if (!found.contains(key)) {
                        found.add(key);
                    }
                }
            }
            return found;
        }

        public List<ChunkPos> chunks() {
            List<ChunkPos> chunks = new ArrayList<>();
            for (int x = min.getX() >> 4; x <= max.getX() >> 4; x++) {
                for (int z = min.getZ() >> 4; z <= max.getZ() >> 4; z++) {
                    chunks.add(new ChunkPos(x, z));
                }
            }
            return chunks;
        }

        public boolean loaded(ServerLevel level) {
            return chunks().stream().allMatch(c -> level.hasChunk(c.x, c.z));
        }

        /** Beds in it: for a barracks, the men it can house. */
        public int beds(ServerLevel level) {
            int beds = 0;
            for (BlockPos pos : BlockPos.betweenClosed(min, max)) {
                var state = level.getBlockState(pos);
                if (state.getBlock() instanceof net.minecraft.world.level.block.BedBlock
                        && state.getValue(net.minecraft.world.level.block.BedBlock.PART)
                                == net.minecraft.world.level.block.state.properties.BedPart.HEAD) {
                    beds++;
                }
            }
            return beds;
        }
    }

    private final List<Depot> depots = new ArrayList<>();
    private int nextId = 1;

    public static Depots get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Depots::new, Depots::load, null), NAME);
    }

    /** Registers a building of {@code kind} between two corners as one of {@code desk}'s stores. */
    public Depot add(ServerLevel level, GlobalPos desk, Kind kind, BlockPos a, BlockPos b) {
        BlockPos min = new BlockPos(Math.min(a.getX(), b.getX()), Math.min(a.getY(), b.getY()), Math.min(a.getZ(), b.getZ()));
        BlockPos max = new BlockPos(Math.max(a.getX(), b.getX()), Math.max(a.getY(), b.getY()), Math.max(a.getZ(), b.getZ()));
        Depot depot = new Depot(nextId++, desk, kind, min, max, 0);
        depot = new Depot(depot.id(), desk, kind, min, max, depot.beds(level));
        depots.add(depot);
        setDirty();
        return depot;
    }

    public boolean remove(int id) {
        boolean removed = depots.removeIf(d -> d.id() == id);
        setDirty();
        return removed;
    }

    /** The buildings a market keeps its goods in. */
    public List<Depot> of(GlobalPos desk) {
        return depots.stream().filter(d -> d.desk().equals(desk)).toList();
    }

    public List<Depot> all() {
        return List.copyOf(depots);
    }

    /** Beds in a market's barracks: men it can house under arms. */
    public int barracksBeds(GlobalPos desk) {
        return depots.stream().filter(d -> d.desk().equals(desk) && d.kind() == Kind.BARRACKS).mapToInt(Depot::bedsBuilt).sum();
    }

    /** Does this market have a building of this kind? */
    public boolean has(GlobalPos desk, Kind kind) {
        return depots.stream().anyMatch(d -> d.desk().equals(desk) && d.kind() == kind);
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        for (Depot d : depots) {
            CompoundTag entry = new CompoundTag();
            entry.putInt("id", d.id());
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, d.desk()).ifSuccess(t -> entry.put("desk", t));
            entry.putString("kind", d.kind().name());
            entry.putLong("min", d.min().asLong());
            entry.putLong("max", d.max().asLong());
            entry.putInt("beds", d.bedsBuilt());
            list.add(entry);
        }
        tag.put("depots", list);
        tag.putInt("nextId", nextId);
        return tag;
    }

    public static Depots load(CompoundTag tag, HolderLookup.Provider registries) {
        Depots depots = new Depots();
        for (Tag t : tag.getList("depots", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("desk")).ifSuccess(desk -> depots.depots.add(new Depot(
                    entry.getInt("id"), desk, Kind.valueOf(entry.getString("kind")), BlockPos.of(entry.getLong("min")),
                    BlockPos.of(entry.getLong("max")), entry.getInt("beds"))));
        }
        depots.nextId = Math.max(1, tag.getInt("nextId"));
        return depots;
    }
}
