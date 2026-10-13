package dev.b28.enterprise.merchant;

import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.StringTag;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Every Merchant's settings, keyed by its desk: which trades each locker may run. Lives in world save data, so it
 * holds while the village is unloaded.
 */
public final class MerchantBook extends SavedData {
    private static final String NAME = "enterprise_merchants";

    // desk -> workstation -> enabled trade keys
    private final Map<GlobalPos, Map<BlockPos, Set<String>>> enabled = new HashMap<>();

    public static MerchantBook get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(MerchantBook::new, MerchantBook::load, null), NAME);
    }

    public boolean isEnabled(GlobalPos desk, BlockPos workstation, String tradeKey) {
        return enabledTrades(desk, workstation).contains(tradeKey);
    }

    public Set<String> enabledTrades(GlobalPos desk, BlockPos workstation) {
        return enabled.getOrDefault(desk, Map.of()).getOrDefault(workstation, Set.of());
    }

    public void setEnabled(GlobalPos desk, BlockPos workstation, String tradeKey, boolean on) {
        Map<BlockPos, Set<String>> lockers = enabled.computeIfAbsent(desk, d -> new HashMap<>());
        Set<String> trades = lockers.computeIfAbsent(workstation, w -> new HashSet<>());
        boolean changed = on ? trades.add(tradeKey) : trades.remove(tradeKey);
        if (trades.isEmpty()) {
            lockers.remove(workstation);
        }
        if (lockers.isEmpty()) {
            enabled.remove(desk);
        }
        if (changed) {
            setDirty();
        }
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag desks = new ListTag();
        enabled.forEach((desk, lockers) -> {
            CompoundTag deskTag = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(pos -> deskTag.put("desk", pos));
            ListTag lockerList = new ListTag();
            lockers.forEach((workstation, trades) -> {
                CompoundTag lockerTag = new CompoundTag();
                lockerTag.putLong("workstation", workstation.asLong());
                ListTag tradeList = new ListTag();
                trades.stream().sorted().forEach(key -> tradeList.add(StringTag.valueOf(key)));
                lockerTag.put("trades", tradeList);
                lockerList.add(lockerTag);
            });
            deskTag.put("lockers", lockerList);
            desks.add(deskTag);
        });
        tag.put("desks", desks);
        return tag;
    }

    public static MerchantBook load(CompoundTag tag, HolderLookup.Provider registries) {
        MerchantBook book = new MerchantBook();
        for (Tag deskEntry : tag.getList("desks", Tag.TAG_COMPOUND)) {
            CompoundTag deskTag = (CompoundTag) deskEntry;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, deskTag.get("desk")).ifSuccess(desk -> {
                for (Tag lockerEntry : deskTag.getList("lockers", Tag.TAG_COMPOUND)) {
                    CompoundTag lockerTag = (CompoundTag) lockerEntry;
                    BlockPos workstation = BlockPos.of(lockerTag.getLong("workstation"));
                    for (Tag trade : lockerTag.getList("trades", Tag.TAG_STRING)) {
                        book.enabled
                                .computeIfAbsent(desk, d -> new HashMap<>())
                                .computeIfAbsent(workstation, w -> new HashSet<>())
                                .add(trade.getAsString());
                    }
                }
            });
        }
        return book;
    }
}
