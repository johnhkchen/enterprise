package dev.b28.enterprise.caravan;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.Tag;
import net.minecraft.world.item.ItemStack;

/**
 * A caravan on a route: a ledger record, not entities. It owns its cargo, the coins it carries, its squad and any
 * animals it drives, the last two stowed off the world until they're home (or sold, for animals).
 */
public final class Caravan {
    public enum Leg { OUTBOUND, WAITING_FOR_FOOD, RETURNING }

    final int route;
    Leg leg;
    long arrives;
    final List<ItemStack> cargo;
    int spurs;
    final int merchantLevel; // the travelling Merchant's level, for bargaining
    final List<CompoundTag> crew = new ArrayList<>(); // the squad, stowed off the world while on the road
    final List<CompoundTag> herd = new ArrayList<>(); // animals a Shepherd drives
    final java.util.Map<String, Double> shopping = new java.util.LinkedHashMap<>(); // comforts to bring home, by kind

    Caravan(int route, Leg leg, long arrives, List<ItemStack> cargo, int spurs, int merchantLevel) {
        this.route = route;
        this.leg = leg;
        this.arrives = arrives;
        this.cargo = cargo;
        this.spurs = spurs;
        this.merchantLevel = merchantLevel;
    }

    /** Stacks the caravan carries, whatever the size of its squad. */
    public int capacity() {
        return Caravans.CARGO_SLOTS;
    }

    public List<CompoundTag> crew() {
        return List.copyOf(crew);
    }

    public List<CompoundTag> herd() {
        return List.copyOf(herd);
    }

    public int merchantLevel() {
        return merchantLevel;
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
        tag.putInt("merchantLevel", merchantLevel);
        ListTag items = new ListTag();
        cargo.stream().filter(stack -> !stack.isEmpty()).forEach(stack -> items.add(stack.save(registries)));
        tag.put("cargo", items);
        ListTag members = new ListTag();
        members.addAll(crew);
        tag.put("crew", members);
        ListTag animals = new ListTag();
        animals.addAll(herd);
        tag.put("herd", animals);
        CompoundTag list = new CompoundTag();
        shopping.forEach(list::putDouble);
        tag.put("shopping", list);
        return tag;
    }

    static Caravan load(CompoundTag tag, HolderLookup.Provider registries) {
        List<ItemStack> cargo = new ArrayList<>();
        for (Tag item : tag.getList("cargo", Tag.TAG_COMPOUND)) {
            ItemStack.parse(registries, item).ifPresent(cargo::add);
        }
        Caravan caravan = new Caravan(tag.getInt("route"), Leg.valueOf(tag.getString("leg")), tag.getLong("arrives"),
                cargo, tag.getInt("spurs"), Math.max(1, tag.getInt("merchantLevel")));
        tag.getList("crew", Tag.TAG_COMPOUND).forEach(t -> caravan.crew.add((CompoundTag) t));
        tag.getList("herd", Tag.TAG_COMPOUND).forEach(t -> caravan.herd.add((CompoundTag) t));
        CompoundTag list = tag.getCompound("shopping");
        list.getAllKeys().forEach(kind -> caravan.shopping.put(kind, list.getDouble(kind)));
        return caravan;
    }
}
