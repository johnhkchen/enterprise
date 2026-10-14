package dev.b28.enterprise.contract;

import java.util.HashMap;
import java.util.Map;
import java.util.function.Function;
import net.minecraft.nbt.CompoundTag;

/**
 * One promise in a contract. Each kind is registered by name so contracts save and load whatever terms later eras
 * add (passage through a toll town, an escort, villagers lent, insurance against a raid) without changing the book.
 */
public interface Term {
    /** The registered name of this kind of term. */
    String type();

    CompoundTag save();

    /** Kinds of term by name, and how to read each back. */
    Map<String, Function<CompoundTag, Term>> TYPES = new HashMap<>(Map.of(Delivery.TYPE, Delivery::load));

    static Term load(CompoundTag tag) {
        Function<CompoundTag, Term> reader = TYPES.get(tag.getString("type"));
        if (reader == null) {
            throw new IllegalArgumentException("Unknown contract term " + tag.getString("type"));
        }
        return reader.apply(tag);
    }
}
