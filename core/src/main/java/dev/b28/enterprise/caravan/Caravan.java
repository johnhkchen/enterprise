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
    final java.util.Map<Integer, Integer> contracted = new java.util.LinkedHashMap<>(); // contract id -> items carried for it
    final List<Integer> drafts = new ArrayList<>(); // contracts drafted on this trip: their copies ride home
    final List<net.minecraft.core.GlobalPos> news = new ArrayList<>(); // markets the squad learned of, told at home
    double pace = 1; // saddled: faster

    Caravan(int route, Leg leg, long arrives, List<ItemStack> cargo, int spurs, int merchantLevel) {
        this.route = route;
        this.leg = leg;
        this.arrives = arrives;
        this.cargo = cargo;
        this.spurs = spurs;
        this.merchantLevel = merchantLevel;
    }

    /** Stacks the caravan carries: a wagon's, and more for each Leatherworker's packs and each porter. */
    public int capacity() {
        return Caravans.CARGO_SLOTS + Squad.PACK_STACKS
                * (int) crew.stream().filter(m -> Squad.carries(Squad.trade(m))).count();
    }

    public List<CompoundTag> crew() {
        return List.copyOf(crew);
    }

    public List<CompoundTag> herd() {
        return List.copyOf(herd);
    }

    public double pace() {
        return pace;
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

    /** Items aboard, crates opened. */
    public int items() {
        return dev.b28.enterprise.cargo.Crates.contents(cargo).stream().mapToInt(ItemStack::getCount).sum();
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
        CompoundTag carried = new CompoundTag();
        contracted.forEach((id, count) -> carried.putInt(Integer.toString(id), count));
        tag.put("contracted", carried);
        tag.putIntArray("drafts", drafts.stream().mapToInt(Integer::intValue).toArray());
        ListTag heard = new ListTag();
        news.forEach(market -> net.minecraft.core.GlobalPos.CODEC.encodeStart(net.minecraft.nbt.NbtOps.INSTANCE, market)
                .ifSuccess(heard::add));
        tag.put("news", heard);
        tag.putDouble("pace", pace);
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
        CompoundTag carried = tag.getCompound("contracted");
        carried.getAllKeys().forEach(id -> caravan.contracted.put(Integer.parseInt(id), carried.getInt(id)));
        for (int id : tag.getIntArray("drafts")) {
            caravan.drafts.add(id);
        }
        caravan.pace = tag.contains("pace") ? tag.getDouble("pace") : 1;
        for (Tag market : tag.getList("news", Tag.TAG_COMPOUND)) {
            net.minecraft.core.GlobalPos.CODEC.parse(net.minecraft.nbt.NbtOps.INSTANCE, market).ifSuccess(caravan.news::add);
        }
        return caravan;
    }
}
