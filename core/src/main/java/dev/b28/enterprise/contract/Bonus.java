package dev.b28.enterprise.contract;

import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.enchantment.ItemEnchantments;

/**
 * A bonus clause: goods delivered enchanted at least so well (each named enchantment at its level or better) are paid
 * so much over the price. The goods count whatever their enchantments; the bonus only adds to the pay. Several clauses
 * add up. (A player's shop wants exact goods; a sovereign's quartermaster takes what comes and pays for quality.)
 */
public record Bonus(Map<String, Integer> atLeast, double percent) implements Term {
    public static final String TYPE = "bonus";

    @Override
    public String type() {
        return TYPE;
    }

    /** Does this piece meet the clause? */
    public boolean met(ItemStack stack) {
        ItemEnchantments on = stack.getEnchantments();
        for (Map.Entry<String, Integer> want : atLeast.entrySet()) {
            int level = 0;
            for (var entry : on.entrySet()) {
                if (entry.getKey().unwrapKey().map(k -> k.location().toString().equals(want.getKey())).orElse(false)) {
                    level = entry.getIntValue();
                }
            }
            if (level < want.getValue()) {
                return false;
            }
        }
        return true;
    }

    public String describe() {
        return atLeast.entrySet().stream().map(e -> e.getKey().replaceFirst(".*:", "") + " " + e.getValue() + "+")
                .collect(java.util.stream.Collectors.joining(" and ")) + ": +" + Math.round(percent) + "%";
    }

    @Override
    public CompoundTag save() {
        CompoundTag tag = new CompoundTag();
        tag.putString("type", TYPE);
        CompoundTag want = new CompoundTag();
        atLeast.forEach(want::putInt);
        tag.put("atLeast", want);
        tag.putDouble("percent", percent);
        return tag;
    }

    static Bonus load(CompoundTag tag) {
        Map<String, Integer> want = new LinkedHashMap<>();
        CompoundTag saved = tag.getCompound("atLeast");
        saved.getAllKeys().forEach(k -> want.put(k, saved.getInt(k)));
        return new Bonus(Map.copyOf(want), tag.getDouble("percent"));
    }
}
