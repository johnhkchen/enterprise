package dev.b28.enterprise.contract;

import net.minecraft.nbt.CompoundTag;

/**
 * Training: the seller brings one of its villagers of {@code profession} up to {@code level} (a Weaponsmith to master,
 * say), and the buyer pays {@code bonus} emeralds when it has. Judged by the seller's census, by its due day; a town
 * still training then has missed it once, and has as long again before the contract breaks. With an {@link Advance},
 * part of the bonus comes up front, to pay for the coal, iron and work the training takes.
 */
public record Training(String profession, int level, double bonus) implements Term {
    public static final String TYPE = "training";

    @Override
    public String type() {
        return TYPE;
    }

    public String describe() {
        return "train a %s to level %d for %.1f emeralds".formatted(profession.replaceFirst(".*:", ""), level, bonus);
    }

    @Override
    public CompoundTag save() {
        CompoundTag tag = new CompoundTag();
        tag.putString("type", TYPE);
        tag.putString("profession", profession);
        tag.putInt("level", level);
        tag.putDouble("bonus", bonus);
        return tag;
    }

    static Training load(CompoundTag tag) {
        return new Training(tag.getString("profession"), tag.getInt("level"), tag.getDouble("bonus"));
    }
}
