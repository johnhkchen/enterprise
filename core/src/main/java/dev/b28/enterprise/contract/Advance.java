package dev.b28.enterprise.contract;

import net.minecraft.nbt.CompoundTag;

/**
 * Money before the goods: the buyer pays {@code share} of the contract's worth up front, carried with its copy (in
 * coin as far as the carrier's purse goes, the rest a bill on the buyer), so the seller can hire, buy its inputs and
 * train its hands before there's anything to deliver. The advance is worked off at the same share of each delivery's
 * price; if the contract breaks, what isn't worked off is owed back.
 */
public record Advance(double share) implements Term {
    public static final String TYPE = "advance";

    @Override
    public String type() {
        return TYPE;
    }

    public String describe() {
        return "%d%% paid up front".formatted(Math.round(share * 100));
    }

    @Override
    public CompoundTag save() {
        CompoundTag tag = new CompoundTag();
        tag.putString("type", TYPE);
        tag.putDouble("share", share);
        return tag;
    }

    static Advance load(CompoundTag tag) {
        return new Advance(tag.getDouble("share"));
    }
}
