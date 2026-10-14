package dev.b28.enterprise.contract;

import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.component.DataComponentPredicate;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.enchantment.EnchantmentHelper;
import net.minecraft.world.item.enchantment.ItemEnchantments;
import net.minecraft.world.item.trading.ItemCost;

/**
 * The seller delivers {@code quantity} of {@code item} to the buyer at each installment, and the buyer pays
 * {@code unitPrice} emeralds for each, the price locked when the contract was drafted. Goods made to order carry
 * their {@code enchantments} (by id, with levels; on a book, stored): only goods enchanted just so count.
 */
public record Delivery(Item item, int quantity, double unitPrice, Map<String, Integer> enchantments) implements Term {
    public static final String TYPE = "delivery";

    public Delivery(Item item, int quantity, double unitPrice) {
        this(item, quantity, unitPrice, Map.of());
    }

    /** A delivery of goods just like {@code spec} (its item and enchantments). */
    public static Delivery of(ItemStack spec, int quantity, double unitPrice) {
        Map<String, Integer> enchantments = new LinkedHashMap<>();
        ItemEnchantments on = spec.getOrDefault(DataComponents.ENCHANTMENTS, ItemEnchantments.EMPTY);
        if (on.isEmpty()) {
            on = spec.getOrDefault(DataComponents.STORED_ENCHANTMENTS, ItemEnchantments.EMPTY);
        }
        on.entrySet().forEach(e -> e.getKey().unwrapKey()
                .ifPresent(key -> enchantments.put(key.location().toString(), e.getIntValue())));
        return new Delivery(spec.getItem(), quantity, unitPrice, Map.copyOf(enchantments));
    }

    @Override
    public String type() {
        return TYPE;
    }

    /** Is it goods made to order (enchanted)? */
    public boolean toOrder() {
        return !enchantments.isEmpty();
    }

    /** One of the goods, as it must be. */
    public ItemStack prototype(RegistryAccess registries) {
        ItemStack stack = new ItemStack(item);
        var registry = registries.registryOrThrow(Registries.ENCHANTMENT);
        enchantments.forEach((id, level) -> registry.getHolder(ResourceKey.create(Registries.ENCHANTMENT,
                ResourceLocation.parse(id))).ifPresent(holder ->
                EnchantmentHelper.updateEnchantments(stack, m -> m.set(holder, level))));
        return stack;
    }

    /** What counts as the goods, in a warehouse: the item, enchanted just so (or not at all, if plain). */
    public ItemCost cost(RegistryAccess registries) {
        ItemStack proto = prototype(registries);
        var component = item == Items.ENCHANTED_BOOK ? DataComponents.STORED_ENCHANTMENTS : DataComponents.ENCHANTMENTS;
        if (!toOrder()) {
            return new ItemCost(item, 1);
        }
        return new ItemCost(item.builtInRegistryHolder(), 1, DataComponentPredicate.builder()
                .expect(component, proto.getOrDefault(component, ItemEnchantments.EMPTY)).build());
    }

    public boolean matches(ItemStack stack, RegistryAccess registries) {
        return cost(registries).test(stack);
    }

    public String describe() {
        String name = BuiltInRegistries.ITEM.getKey(item).getPath();
        return enchantments.isEmpty() ? name : name + " " + enchantments.entrySet().stream()
                .map(e -> e.getKey().replaceFirst(".*:", "") + " " + e.getValue())
                .collect(java.util.stream.Collectors.joining(", ", "(", ")"));
    }

    @Override
    public CompoundTag save() {
        CompoundTag tag = new CompoundTag();
        tag.putString("type", TYPE);
        tag.putString("item", BuiltInRegistries.ITEM.getKey(item).toString());
        tag.putInt("quantity", quantity);
        tag.putDouble("unitPrice", unitPrice);
        CompoundTag ench = new CompoundTag();
        enchantments.forEach(ench::putInt);
        tag.put("enchantments", ench);
        return tag;
    }

    static Delivery load(CompoundTag tag) {
        Map<String, Integer> enchantments = new LinkedHashMap<>();
        CompoundTag ench = tag.getCompound("enchantments");
        ench.getAllKeys().forEach(key -> enchantments.put(key, ench.getInt(key)));
        return new Delivery(BuiltInRegistries.ITEM.get(ResourceLocation.parse(tag.getString("item"))),
                tag.getInt("quantity"), tag.getDouble("unitPrice"), Map.copyOf(enchantments));
    }
}
