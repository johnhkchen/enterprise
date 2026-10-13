package dev.b28.enterprise.settlement;

import net.minecraft.nbt.CompoundTag;

/**
 * One count of a settlement around its bell: villagers (and how many are free), beds, workstations, the value that
 * makes, and when it was taken. A free villager can walk to its own bed, its workstation and the bell.
 */
public record Census(int villagers, int free, int beds, int workstations, long gameTime) {
    /** What a confined villager is worth next to a free one. A thriving village beats a trading hall. */
    public static final double CONFINED_WEIGHT = 0.1;

    public static final int MIN_BEDS = 3;
    public static final int MIN_WORKSTATIONS = 3;

    public boolean isSettlement() {
        return beds >= MIN_BEDS && workstations >= MIN_WORKSTATIONS;
    }

    public int confined() {
        return villagers - free;
    }

    public double value() {
        return free + CONFINED_WEIGHT * confined();
    }

    CompoundTag save() {
        CompoundTag tag = new CompoundTag();
        tag.putInt("villagers", villagers);
        tag.putInt("free", free);
        tag.putInt("beds", beds);
        tag.putInt("workstations", workstations);
        tag.putLong("gameTime", gameTime);
        return tag;
    }

    static Census load(CompoundTag tag) {
        return new Census(tag.getInt("villagers"), tag.getInt("free"), tag.getInt("beds"),
                tag.getInt("workstations"), tag.getLong("gameTime"));
    }
}
