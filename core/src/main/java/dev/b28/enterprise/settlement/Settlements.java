package dev.b28.enterprise.settlement;

import java.util.HashMap;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * The latest census of every bell that has been counted, and its roster of residents, kept in world save data for the
 * ledger. The roster lets a census count villagers it can't see: one whose chunk isn't loaded is assumed at home.
 */
public final class Settlements extends SavedData {
    private static final String NAME = "enterprise_settlements";

    /**
     * A villager of the settlement as last seen: where (packed chunk), its bed, its trade and level, whether it could
     * walk freely, whether a baby, and when. {@code away}: out with a caravan, so not to be looked for at home.
     */
    public record Resident(java.util.UUID id, long chunk, java.util.Optional<net.minecraft.core.BlockPos> home,
            String profession, int level, boolean free, boolean baby, long lastSeen, boolean away) {
        CompoundTag save() {
            CompoundTag tag = new CompoundTag();
            tag.putUUID("id", id);
            tag.putLong("chunk", chunk);
            home.ifPresent(pos -> tag.putLong("home", pos.asLong()));
            tag.putString("profession", profession);
            tag.putInt("level", level);
            tag.putBoolean("free", free);
            tag.putBoolean("baby", baby);
            tag.putLong("lastSeen", lastSeen);
            tag.putBoolean("away", away);
            return tag;
        }

        static Resident load(CompoundTag tag) {
            return new Resident(tag.getUUID("id"), tag.getLong("chunk"),
                    tag.contains("home") ? java.util.Optional.of(net.minecraft.core.BlockPos.of(tag.getLong("home")))
                            : java.util.Optional.empty(),
                    tag.getString("profession"), tag.getInt("level"), tag.getBoolean("free"), tag.getBoolean("baby"),
                    tag.getLong("lastSeen"), tag.getBoolean("away"));
        }

        /** As last seen, but taken to be there still at {@code now}: its chunk wasn't loaded to look. */
        public Resident assumedAt(long now) {
            return new Resident(id, chunk, home, profession, level, free, baby, now, away);
        }

        public Resident away(boolean out) {
            return new Resident(id, chunk, home, profession, level, free, baby, lastSeen, out);
        }
    }

    private final Map<GlobalPos, Census> census = new HashMap<>();
    private final Map<GlobalPos, Map<java.util.UUID, Resident>> rosters = new HashMap<>();

    public Map<java.util.UUID, Resident> roster(GlobalPos bell) {
        return Map.copyOf(rosters.getOrDefault(bell, Map.of()));
    }

    public void setRoster(GlobalPos bell, Map<java.util.UUID, Resident> roster) {
        rosters.put(bell, new java.util.LinkedHashMap<>(roster));
        setDirty();
    }

    /** A resident is gone for good (fallen in war). */
    public void remove(GlobalPos bell, java.util.UUID id) {
        var roster = rosters.get(bell);
        if (roster != null && roster.remove(id) != null) {
            setDirty();
        }
    }

    /** Marks a resident as out with a caravan, or back. */
    public void setAway(GlobalPos bell, java.util.UUID id, boolean away) {
        var roster = rosters.get(bell);
        if (roster != null && roster.containsKey(id)) {
            roster.put(id, roster.get(id).away(away));
            setDirty();
        }
    }

    /** Where the settlement's residents were last seen and sleep, as packed chunks: what a round should load. */
    public java.util.Set<Long> residentChunks(GlobalPos bell) {
        java.util.Set<Long> chunks = new java.util.LinkedHashSet<>();
        for (Resident resident : rosters.getOrDefault(bell, Map.of()).values()) {
            if (resident.away()) {
                continue;
            }
            resident.home().ifPresent(pos -> chunks.add(net.minecraft.world.level.ChunkPos.asLong(pos)));
            chunks.add(resident.chunk());
        }
        return chunks;
    }

    public static Settlements get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Settlements::new, Settlements::load, null), NAME);
    }

    public Optional<Census> census(GlobalPos bell) {
        return Optional.ofNullable(census.get(bell));
    }

    public Map<GlobalPos, Census> all() {
        return Map.copyOf(census);
    }

    public void record(GlobalPos bell, Census result) {
        census.put(bell, result);
        setDirty();
    }

    /** A villager has died: off every roster (the census otherwise keeps those it can't see). */
    public void died(java.util.UUID villager) {
        boolean changed = false;
        for (var entry : rosters.entrySet()) {
            if (entry.getValue().containsKey(villager)) {
                var roster = new java.util.HashMap<>(entry.getValue());
                roster.remove(villager);
                entry.setValue(roster);
                changed = true;
            }
        }
        if (changed) {
            setDirty();
        }
    }

    /** A villager's death leaves the Java roster; in core mode the census owns the roster, so not here (docs/alpha-plan.md 2). */
    public static void onDeath(net.neoforged.neoforge.event.entity.living.LivingDeathEvent event) {
        if (dev.b28.enterprise.binding.Society.isCore()) {
            return;
        }
        if (event.getEntity() instanceof net.minecraft.world.entity.npc.Villager villager
                && villager.level() instanceof net.minecraft.server.level.ServerLevel level) {
            get(level.getServer()).died(villager.getUUID());
        }
    }

    public void forget(GlobalPos bell) {
        if (census.remove(bell) != null) {
            setDirty();
        }
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        census.forEach((bell, result) -> {
            CompoundTag entry = result.save();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, bell).ifSuccess(pos -> entry.put("bell", pos));
            ListTag residents = new ListTag();
            rosters.getOrDefault(bell, Map.of()).values().forEach(r -> residents.add(r.save()));
            entry.put("residents", residents);
            list.add(entry);
        });
        tag.put("settlements", list);
        return tag;
    }

    public static Settlements load(CompoundTag tag, HolderLookup.Provider registries) {
        Settlements settlements = new Settlements();
        for (Tag entry : tag.getList("settlements", Tag.TAG_COMPOUND)) {
            CompoundTag compound = (CompoundTag) entry;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, compound.get("bell")).ifSuccess(bell -> {
                settlements.census.put(bell, Census.load(compound));
                Map<java.util.UUID, Resident> roster = new java.util.LinkedHashMap<>();
                for (Tag r : compound.getList("residents", Tag.TAG_COMPOUND)) {
                    Resident resident = Resident.load((CompoundTag) r);
                    roster.put(resident.id(), resident);
                }
                settlements.rosters.put(bell, roster);
            });
        }
        return settlements;
    }
}
