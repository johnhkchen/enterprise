package dev.b28.enterprise.caravan;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.Tag;
import net.minecraft.world.item.ItemStack;

/**
 * A caravan on a route: a ledger record, not entities. It owns its cargo and the coins it carries; when it shows up
 * in the world later, the entities will be a view of this record.
 */
public final class Caravan {
    public enum Leg { OUTBOUND, WAITING_FOR_FOOD, RETURNING }

    final int route;
    Leg leg;
    long arrives;
    final List<ItemStack> cargo;
    int spurs;

    Caravan(int route, Leg leg, long arrives, List<ItemStack> cargo, int spurs) {
        this.route = route;
        this.leg = leg;
        this.arrives = arrives;
        this.cargo = cargo;
        this.spurs = spurs;
    }

    public Leg leg() {
        return leg;
    }

    public long arrives() {
        return arrives;
    }

    public List<ItemStack> cargo() {
        return cargo;
    }

    public int spurs() {
        return spurs;
    }

    public int items() {
        return cargo.stream().mapToInt(ItemStack::getCount).sum();
    }

    CompoundTag save(HolderLookup.Provider registries) {
        CompoundTag tag = new CompoundTag();
        tag.putInt("route", route);
        tag.putString("leg", leg.name());
        tag.putLong("arrives", arrives);
        tag.putInt("spurs", spurs);
        ListTag items = new ListTag();
        cargo.stream().filter(stack -> !stack.isEmpty()).forEach(stack -> items.add(stack.save(registries)));
        tag.put("cargo", items);
        return tag;
    }

    static Caravan load(CompoundTag tag, HolderLookup.Provider registries) {
        List<ItemStack> cargo = new ArrayList<>();
        for (Tag item : tag.getList("cargo", Tag.TAG_COMPOUND)) {
            ItemStack.parse(registries, item).ifPresent(cargo::add);
        }
        return new Caravan(tag.getInt("route"), Leg.valueOf(tag.getString("leg")), tag.getLong("arrives"), cargo,
                tag.getInt("spurs"));
    }
}
