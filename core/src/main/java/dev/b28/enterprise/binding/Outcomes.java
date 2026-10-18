package dev.b28.enterprise.binding;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.adapters.LandAdapter;
import dev.b28.enterprise.binding.adapters.MoneyAdapter;
import dev.b28.enterprise.binding.adapters.PeopleAdapter;
import dev.b28.enterprise.binding.adapters.StockAdapter;
import dev.b28.enterprise.binding.adapters.WagonAdapter;
import dev.b28.enterprise.wire.Output;

/**
 * Where each of the core's outputs goes (docs/binding-plan.md 1.5): an exhaustive switch over {@link Output}, one line a
 * variant, each to the adapter that shows it in the world. A variant the core adds doesn't compile here until it's
 * routed, so no outcome is dropped unseen.
 *
 * <p>{@code Moved} goes to {@code adapters/StockAdapter}, which writes it to a desk's or a depot's chests; {@code Cash}
 * to {@code adapters/MoneyAdapter}, which writes it to a desk's chests and Blaze Banker. The people's
 * outputs (a child born, a hand settled, departed or gone, a hand's level) go to {@code adapters/PeopleAdapter}, a town's
 * works to lay ({@code Build}) to {@code adapters/LandAdapter}, a wagon's and its crew's (a hand away, a hand back) to
 * {@code adapters/WagonAdapter}. The rest are the master Merchant's and the pallets', each routed to {@link #unshown}
 * until its adapter lands (the pallets' with {@code pallet-views}: the mod's crates and their papers drawn from these,
 * with no pallet rules in Java; the orders' with {@code shop-adapter}).
 */
final class Outcomes {
    private final PeopleAdapter people;
    private final StockAdapter stock;
    private final MoneyAdapter money;
    private final LandAdapter land;
    private final WagonAdapter wagons;
    private long unshown;

    Outcomes(PeopleAdapter people, StockAdapter stock, MoneyAdapter money, LandAdapter land, WagonAdapter wagons) {
        this.people = people;
        this.stock = stock;
        this.money = money;
        this.land = land;
        this.wagons = wagons;
    }

    void show(Output output) {
        switch (output) {
            case Output.Moved o -> stock.show(o);
            case Output.Cash o -> money.show(o);
            case Output.WagonLeft o -> wagons.show(o);
            case Output.WagonSold o -> wagons.show(o);
            case Output.WagonHome o -> wagons.show(o);
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
            case Output.HandAway o -> wagons.show(o);
            case Output.HandBack o -> wagons.show(o);
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
