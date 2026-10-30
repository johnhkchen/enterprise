package dev.b28.enterprise.territory;

import dev.b28.enterprise.hex.Hex;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * The hexes the core asked explored ({@code Output.Explore}: a road's, once), in the order asked: each has its chunks
 * loaded (generated if they never were) a few hexes a look, so {@link Terrain} surveys them as they load and the core
 * plans over the land itself rather than its guess. Saved with the world: what's left to explore.
 */
public final class Exploration extends SavedData {
    private static final String NAME = "enterprise_exploration";
    private final List<Hex> waiting = new ArrayList<>();

    public static Exploration get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Exploration::new, Exploration::load, null), NAME);
    }

    public void add(Hex hex) {
        if (!waiting.contains(hex)) {
            waiting.add(hex);
            setDirty();
        }
    }

    public List<Hex> waiting() {
        return List.copyOf(waiting);
    }

    public void explored(Hex hex) {
        if (waiting.remove(hex)) {
            setDirty();
        }
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        int[] hexes = new int[2 * waiting.size()];
        for (int i = 0; i < waiting.size(); i++) {
            hexes[2 * i] = waiting.get(i).q();
            hexes[2 * i + 1] = waiting.get(i).r();
        }
        tag.putIntArray("waiting", hexes);
        return tag;
    }

    public static Exploration load(CompoundTag tag, HolderLookup.Provider registries) {
        Exploration e = new Exploration();
        int[] hexes = tag.getIntArray("waiting");
        for (int i = 0; i + 1 < hexes.length; i += 2) {
            e.waiting.add(new Hex(hexes[i], hexes[i + 1]));
        }
        return e;
    }
}
