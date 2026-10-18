package dev.b28.enterprise.binding;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.adapters.PeopleAdapter;
import dev.b28.enterprise.wire.Output;

/**
 * Where each of the core's outputs goes (docs/binding-plan.md 1.5): an exhaustive switch over {@link Output}, one line a
 * variant, each to the adapter that shows it in the world. A variant the core adds doesn't compile here until it's
 * routed, so no outcome is dropped unseen.
 *
 * <p>The people's outputs (a child born, a hand settled, departed or gone, a hand's level) go to {@code
 * adapters/PeopleAdapter}. The wagons', the master Merchant's and the pallets' are routed to {@link #unshown} until their
 * adapters land (the wagons' and pallets' with {@code wagon-adapter} and {@code pallet-views}, the orders' with {@code
 * shop-adapter}).
 */
final class Outcomes {
    private final PeopleAdapter people;
    private long unshown;

    Outcomes(PeopleAdapter people) {
        this.people = people;
    }

    void show(Output output) {
        switch (output) {
            case Output.WagonLeft o -> unshown(o);
            case Output.WagonSold o -> unshown(o);
            case Output.WagonHome o -> unshown(o);
            case Output.OrderFilled o -> unshown(o);
            case Output.Signal o -> unshown(o);
            case Output.PalletPacked o -> unshown(o);
            case Output.PalletMoved o -> unshown(o);
            case Output.PalletOpened o -> unshown(o);
            case Output.Born o -> people.show(o);
            case Output.Settled o -> people.show(o);
            case Output.Departed o -> people.show(o);
            case Output.HandGone o -> people.show(o);
            case Output.HandLevel o -> people.show(o);
        }
    }

    /** Outputs that came with no adapter to show them, so far. */
    long unshown() {
        return unshown;
    }

    private void unshown(Output output) {
        unshown++;
        Enterprise.LOGGER.debug("The core's {} has no adapter yet: {}", output.getClass().getSimpleName(), output);
    }
}
