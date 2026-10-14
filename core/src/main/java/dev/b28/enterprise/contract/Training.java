package dev.b28.enterprise.contract;

import net.minecraft.nbt.CompoundTag;

/**
 * Training: the seller brings one of its villagers of {@code profession} up to {@code level} (a Weaponsmith to master,
 * say), and the buyer pays {@code bonus} emeralds when it has. Judged by the seller's census, by its due day; a town
 * still training then has missed it once, and has as long again before the contract breaks. With an {@link Advance},
 * part of the bonus comes up front, to pay for the coal, iron and work the training takes. With an offtake, the buyer
 * takes the pupil's paid work too (the axes a Novice smith is paid to make), at {@code offtake} of its worth: the
 * seller crates it, made out to the buyer, and draws a bill on the buyer for it. It's for a new one: the seller
 * already had {@code already} of the trade at the level when it was drafted, and is done when it has one more.
 */
public record Training(String profession, int level, double bonus, double offtake, int already) implements Term {
    public static final String TYPE = "training";

    public Training(String profession, int level, double bonus) {
        this(profession, level, bonus, 0, 0);
    }

    public Training(String profession, int level, double bonus, double offtake) {
        this(profession, level, bonus, offtake, 0);
    }

    /** The same work, for a seller that already has {@code n} of the trade at the level. */
    public Training forSeller(int n) {
        return new Training(profession, level, bonus, offtake, n);
    }

    @Override
    public String type() {
        return TYPE;
    }

    public String describe() {
        return "train a %s to level %d for %.1f emeralds%s".formatted(profession.replaceFirst(".*:", ""), level, bonus,
                offtake > 0 ? ", its work bought at %d%% of worth".formatted(Math.round(offtake * 100)) : "");
    }

    @Override
    public CompoundTag save() {
        CompoundTag tag = new CompoundTag();
        tag.putString("type", TYPE);
        tag.putString("profession", profession);
        tag.putInt("level", level);
        tag.putDouble("bonus", bonus);
        tag.putDouble("offtake", offtake);
        tag.putInt("already", already);
        return tag;
    }

    static Training load(CompoundTag tag) {
        return new Training(tag.getString("profession"), tag.getInt("level"), tag.getDouble("bonus"),
                tag.getDouble("offtake"), tag.getInt("already"));
    }
}
