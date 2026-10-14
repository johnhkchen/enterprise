package dev.b28.enterprise.contract;

import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.item.Item;

/**
 * The seller delivers {@code quantity} of {@code item} to the buyer at each installment, and the buyer pays
 * {@code unitPrice} emeralds for each, the price locked when the contract was drafted.
 */
public record Delivery(Item item, int quantity, double unitPrice) implements Term {
    public static final String TYPE = "delivery";

    @Override
    public String type() {
        return TYPE;
    }

    @Override
    public CompoundTag save() {
        CompoundTag tag = new CompoundTag();
        tag.putString("type", TYPE);
        tag.putString("item", BuiltInRegistries.ITEM.getKey(item).toString());
        tag.putInt("quantity", quantity);
        tag.putDouble("unitPrice", unitPrice);
        return tag;
    }

    static Delivery load(CompoundTag tag) {
        return new Delivery(BuiltInRegistries.ITEM.get(ResourceLocation.parse(tag.getString("item"))),
                tag.getInt("quantity"), tag.getDouble("unitPrice"));
    }
}
