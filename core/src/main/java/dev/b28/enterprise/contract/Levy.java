package dev.b28.enterprise.contract;

import net.minecraft.nbt.CompoundTag;

/**
 * A levy: the town sends {@code soldiers} to the sovereign's army, who serve until peace, and the sovereign pays
 * {@code payPerDay} emeralds a soldier-day, owed to the town (sent home) until a caravan carries it.
 */
public record Levy(int soldiers, double payPerDay) implements Term {
    public static final String TYPE = "levy";

    @Override
    public String type() {
        return TYPE;
    }

    @Override
    public CompoundTag save() {
        CompoundTag tag = new CompoundTag();
        tag.putString("type", TYPE);
        tag.putInt("soldiers", soldiers);
        tag.putDouble("payPerDay", payPerDay);
        return tag;
    }

    static Levy load(CompoundTag tag) {
        return new Levy(tag.getInt("soldiers"), tag.getDouble("payPerDay"));
    }
}
