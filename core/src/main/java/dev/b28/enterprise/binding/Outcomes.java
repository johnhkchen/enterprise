package dev.b28.enterprise.binding;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.wire.Output;

/**
 * Where each of the core's outputs goes (docs/binding-plan.md 1.5): an exhaustive switch over {@link Output}, one line a
 * variant, each to the adapter that shows it in the world. A variant the core adds doesn't compile here until it's
 * routed, so no outcome is dropped unseen.
 *
 * <p>The proof (B2.1) has one adapter, {@code adapters/StockAdapter} (no output of its own yet: {@code Moved} comes with
 * {@code stock-adapter}), and the core's outputs today are the wagons', the master
 * Merchant's and the pallets': no input the proof sends makes one, so each is routed to {@link #unshown} until its
 * adapter lands (the wagons' and pallets' with {@code wagon-adapter} and {@code pallet-views}, the orders' with
 * {@code shop-adapter}).
 */
final class Outcomes {
    private long unshown;

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
