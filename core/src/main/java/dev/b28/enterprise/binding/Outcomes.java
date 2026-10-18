package dev.b28.enterprise.binding;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.adapters.LandAdapter;
import dev.b28.enterprise.binding.adapters.MoneyAdapter;
import dev.b28.enterprise.binding.adapters.PeopleAdapter;
import dev.b28.enterprise.binding.adapters.StockAdapter;
import dev.b28.enterprise.wire.Output;

/**
 * Where each of the core's outputs goes (docs/binding-plan.md 1.5): an exhaustive switch over {@link Output}, one line a
 * variant, each to the adapter that shows it in the world. A variant the core adds doesn't compile here until it's
 * routed, so no outcome is dropped unseen.
 *
 * <p>{@code Moved} goes to {@code adapters/StockAdapter}, which writes it to a desk's or a depot's chests; {@code Cash}
 * to {@code adapters/MoneyAdapter}, which writes it to a desk's chests and Blaze Banker. The people's
 * outputs (a child born, a hand settled, departed or gone, a hand's level) go to {@code adapters/PeopleAdapter}, a town's
 * works to lay ({@code Build}) to {@code adapters/LandAdapter}. The
 * rest are the wagons', the master Merchant's and the pallets', each routed to {@link #unshown} until its adapter lands
 * (the wagons' with {@code wagon-adapter}, the pallets' with {@code pallet-views}: the mod's crates drawn from these,
 * with no pallet rules in Java; the orders' with {@code shop-adapter}).
 */
final class Outcomes {
    private final PeopleAdapter people;
    private final StockAdapter stock;
    private final MoneyAdapter money;
    private final LandAdapter land;
    private long unshown;

    Outcomes(PeopleAdapter people, StockAdapter stock, MoneyAdapter money, LandAdapter land) {
        this.people = people;
        this.stock = stock;
        this.money = money;
        this.land = land;
    }

    void show(Output output) {
        switch (output) {
            case Output.Moved o -> stock.show(o);
            case Output.Cash o -> money.show(o);
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
            case Output.Build o -> land.show(o);
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
