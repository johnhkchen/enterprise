package dev.b28.enterprise.merchant;

import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.settlement.Names;
import java.util.HashSet;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.network.chat.Component;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Town charters (docs/decisions.md, "Merchant money"): a town a player founds starts with the core's founding purse, so
 * it costs a charter, bought from any Merchant for a block of diamond and used on a Merchant's Desk. A desk a player
 * places without one is only a workstation (a villager takes it and becomes a Merchant, who sells charters) until a
 * charter is used on it. A desk put back where a town's desk stood is that town's again, no charter asked (the core
 * gives it no second purse). A desk placed unseen (a command, a datapack, a structure: an operator's) is founded as it
 * always was.
 *
 * <p>Saved with the world: the desks waiting for a charter, so a chunk loading again doesn't found them unseen, and
 * the places towns' desks stood.
 */
public final class Charters extends SavedData {
    private static final String NAME = "enterprise_charters";

    private final Set<GlobalPos> waiting = new HashSet<>();
    private final Set<GlobalPos> former = new HashSet<>();

    public static Charters get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Charters::new, Charters::load, null), NAME);
    }

    /** Whether the desk at {@code at} was placed by a player and waits for a charter. */
    public boolean waits(GlobalPos at) {
        return waiting.contains(at);
    }

    /**
     * A desk placed by a player: true if it's to be founded now (a town's desk stood there), else it waits for a
     * charter.
     */
    public boolean placed(GlobalPos at) {
        if (former.remove(at)) {
            setDirty();
            return true;
        }
        waiting.add(at);
        setDirty();
        return false;
    }

    /** The desk at {@code at} founded with a charter: it waits no longer. */
    public void chartered(GlobalPos at) {
        if (waiting.remove(at)) {
            setDirty();
        }
    }

    /** A desk broken: if it was a town's, its place is remembered; one waiting for a charter waits no longer. */
    public void broken(GlobalPos at, boolean town) {
        boolean changed = waiting.remove(at);
        if (town) {
            changed |= former.add(at);
        }
        if (changed) {
            setDirty();
        }
    }

    /**
     * A charter bought: a block of diamond taken from {@code inventory} (none in creative) and a charter put in it, or
     * dropped at the player's feet if it's full. False, and nothing taken, when it holds no block of diamond.
     */
    public static boolean buy(Inventory inventory) {
        if (!inventory.player.getAbilities().instabuild) {
            int slot = -1;
            for (int i = 0; i < inventory.getContainerSize() && slot < 0; i++) {
                if (inventory.getItem(i).is(Items.DIAMOND_BLOCK)) {
                    slot = i;
                }
            }
            if (slot < 0) {
                return false;
            }
            inventory.removeItem(slot, 1);
        }
        ItemStack charter = new ItemStack(Merchant.CHARTER.get());
        if (!inventory.add(charter)) {
            inventory.player.drop(charter, false);
        }
        return true;
    }

    /**
     * {@code charter} used on the desk at {@code desk}: the desk founded a town in {@code host}'s core (its founding
     * purse with it) and the charter used up (kept in creative). Returns what to tell the player.
     */
    public static Component use(CoreHost host, ServerLevel level, BlockPos desk, ItemStack charter, boolean creative) {
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        if (host == null || host.pausedWhy() != null) {
            return Component.translatable("message.enterprise.charter.no_core");
        }
        if (host.stock().townAt(at) != null) {
            return Component.translatable("message.enterprise.charter.already");
        }
        String name = Names.of(level.getServer(), at);
        host.stock().found(level, desk, name);
        get(level.getServer()).chartered(at);
        if (!creative) {
            charter.shrink(1);
        }
        return Component.translatable("message.enterprise.charter.founded", name);
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        tag.put("waiting", list(waiting));
        tag.put("former", list(former));
        return tag;
    }

    private static ListTag list(Set<GlobalPos> spots) {
        ListTag list = new ListTag();
        spots.forEach(at -> GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, at).ifSuccess(list::add));
        return list;
    }

    public static Charters load(CompoundTag tag, HolderLookup.Provider registries) {
        Charters charters = new Charters();
        for (Tag t : tag.getList("waiting", Tag.TAG_COMPOUND)) {
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, t).ifSuccess(charters.waiting::add);
        }
        for (Tag t : tag.getList("former", Tag.TAG_COMPOUND)) {
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, t).ifSuccess(charters.former::add);
        }
        return charters;
    }
}
