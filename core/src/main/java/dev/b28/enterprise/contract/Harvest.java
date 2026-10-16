package dev.b28.enterprise.contract;

import net.minecraft.nbt.CompoundTag;

/**
 * A forward sale of a crop, as farms have always sold theirs: each installment may be delivered any time within
 * {@code windowDays} of its day, not by a fixed grace; a short one rolls over to the next (the harvest was late, or
 * thin), costing a little trust, not a miss. Only arrears beyond {@code harvest_arrears} installments (see
 * {@link dev.b28.enterprise.Dials}), or what's still owed at the last, count as misses. Early farms keep their contracts through a bad season.
 */
public record Harvest(int windowDays, boolean carried) implements Term {
    public static final String TYPE = "harvest";

    /** A forward sale's harvest term; {@code carried}: a rent's coupon, whose shortfall the rent carries on. */
    public Harvest(int windowDays) {
        this(windowDays, false);
    }

    public long windowTicks() {
        return windowDays * 24000L;
    }

    @Override
    public String type() {
        return TYPE;
    }

    public String describe() {
        return "delivered within %d days of each day, shortfalls rolled over".formatted(windowDays);
    }

    @Override
    public CompoundTag save() {
        CompoundTag tag = new CompoundTag();
        tag.putString("type", TYPE);
        tag.putInt("windowDays", windowDays);
        tag.putBoolean("carried", carried);
        return tag;
    }

    static Harvest load(CompoundTag tag) {
        return new Harvest(tag.getInt("windowDays"), tag.getBoolean("carried"));
    }
}
