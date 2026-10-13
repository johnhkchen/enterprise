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

/** The latest census of every bell that has been counted, kept in world save data for the ledger. */
public final class Settlements extends SavedData {
    private static final String NAME = "enterprise_settlements";

    private final Map<GlobalPos, Census> census = new HashMap<>();

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
            list.add(entry);
        });
        tag.put("settlements", list);
        return tag;
    }

    public static Settlements load(CompoundTag tag, HolderLookup.Provider registries) {
        Settlements settlements = new Settlements();
        for (Tag entry : tag.getList("settlements", Tag.TAG_COMPOUND)) {
            CompoundTag compound = (CompoundTag) entry;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, compound.get("bell"))
                    .ifSuccess(bell -> settlements.census.put(bell, Census.load(compound)));
        }
        return settlements;
    }
}
