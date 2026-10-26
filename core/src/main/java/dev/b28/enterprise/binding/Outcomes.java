package dev.b28.enterprise.binding;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.adapters.CrownAdapter;
import dev.b28.enterprise.binding.adapters.LandAdapter;
import dev.b28.enterprise.binding.adapters.MoneyAdapter;
import dev.b28.enterprise.binding.adapters.PalletAdapter;
import dev.b28.enterprise.binding.adapters.PeopleAdapter;
import dev.b28.enterprise.binding.adapters.ShopAdapter;
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
 * {@code adapters/WagonAdapter}, the pallets' (packed, moved, opened) to {@code adapters/PalletAdapter}, which draws them
 * as tagged crates in the desks' chests, and a master Merchant's shops' (an order filled, a stall bought from, a shelf,
 * a deed, a vendor to build) to {@code adapters/ShopAdapter}, and the crown's (war declared, peace made, the gold shop to
 * build, show or take down) to {@code adapters/CrownAdapter}.
 */
final class Outcomes {
    private final PeopleAdapter people;
    private final StockAdapter stock;
    private final MoneyAdapter money;
    private final LandAdapter land;
    private final WagonAdapter wagons;
    private final PalletAdapter pallets;
    private final ShopAdapter shops;
    private final CrownAdapter crown;
    private long unshown;

    Outcomes(PeopleAdapter people, StockAdapter stock, MoneyAdapter money, LandAdapter land, WagonAdapter wagons,
            PalletAdapter pallets, ShopAdapter shops, CrownAdapter crown) {
        this.people = people;
        this.stock = stock;
        this.money = money;
        this.land = land;
        this.wagons = wagons;
        this.pallets = pallets;
        this.shops = shops;
        this.crown = crown;
    }

    void show(Output output) {
        switch (output) {
            case Output.Moved o -> stock.show(o);
            case Output.Cash o -> money.show(o);
            case Output.WagonLeft o -> wagons.show(o);
            case Output.WagonSold o -> wagons.show(o);
            case Output.WagonHome o -> wagons.show(o);
            case Output.OrderFilled o -> shops.show(o);
            case Output.Signal o -> shops.show(o);
            case Output.StallBought o -> shops.show(o);
            case Output.Shelf o -> shops.show(o);
            case Output.Deed o -> shops.show(o);
            case Output.DeedPaid o -> shops.show(o);
            case Output.BuildShop o -> shops.show(o);
            case Output.WarDeclared o -> crown.show(o);
            case Output.PeaceMade o -> crown.show(o);
            case Output.BuildCrownShop o -> crown.show(o);
            case Output.CrownShop o -> crown.show(o);
            case Output.CrownShopDown o -> crown.show(o);
            case Output.PalletPacked o -> pallets.show(o);
            case Output.PalletMoved o -> pallets.show(o);
            case Output.PalletOpened o -> pallets.show(o);
            case Output.Born o -> people.show(o);
            case Output.Settled o -> people.show(o);
            case Output.Departed o -> people.show(o);
            case Output.HandGone o -> people.show(o);
            case Output.HandLevel o -> people.show(o);
            case Output.Stall o -> people.show(o);
            case Output.Build o -> land.show(o);
            case Output.Found o -> land.show(o);
            case Output.Trail o -> land.show(o);
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
