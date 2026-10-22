package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.market.RentDeeds;
import dev.b28.enterprise.market.Shops;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.DeedLine;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.Output;
import dev.b28.enterprise.wire.Point;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.ShopLine;
import dev.b28.enterprise.wire.VendorMode;
import dev.b28.enterprise.wire.VendorRead;
import dev.ithundxr.createnumismatics.Numismatics;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import dev.ithundxr.createnumismatics.content.backend.ReasonHolder;
import dev.ithundxr.createnumismatics.content.bank.CardItem;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.Block;
import net.neoforged.neoforge.server.ServerLifecycleHooks;

/**
 * Numismatics vendors, the town's shop board and rent deeds, from the core (docs/binding-plan.md 2, concern 9). The
 * shops' rules are the core's ({@code orders.rs}, {@code rules/orders.rs}, {@code desires.rs}, {@code harvest.rs},
 * {@code shop_host.rs}): a master Merchant fills a player's buying vendor under a contract, buys from a selling vendor,
 * stocks the town's own shop with its surplus, lists on the board what the town is after, and a freight hub sells and
 * buys back the deeds of the rents it holds. What the world has of them is vendors, and this adapter is how:
 *
 * <ul>
 *   <li><b>Counted</b> ({@link #happenings}, at each look in core mode, before the look's time): every vendor the core
 *       keeps as a {@code Shop} point of a town whose desk is loaded is read for what it is (its mode, filter, lot and
 *       price, the items in it, the room it has, the bank card's balance, and the coin its box holds, which is taken out:
 *       it is the town's now). The core says whether it is a player's order, a stall, the town's own shop, the board's or a
 *       deed's. A vendor within reach of a loaded desk that the core doesn't keep is registered as a point first ({@link
 *       #place}); a point whose vendor is gone (its chunk loaded, no vendor there) is removed.
 *   <li><b>Shown</b> ({@link #show}): an order filled has its goods put in the vendor and its price taken off its card
 *       ({@code OrderFilled}); a stall bought from has the lots taken out and paid into its box ({@code StallBought}); the
 *       town's own and its board's vendors are set to the listing and have goods added to their shelf or cleared
 *       ({@code Shelf}); a deed's vendor sells the deed at the ask or buys it back at the bid, its dealer's card funded
 *       ({@code Deed}). The goods the core moved off the town's stock are written to the desk's chests by the stock
 *       adapter's {@code Moved}, the same look, and the money by the money adapter's {@code Cash}: the vendor's half is
 *       all that's done here.
 *   <li><b>Built</b> ({@code BuildShop}, done at the next look): a vendor touching the desk, on open floor, for the
 *       board; registered as the town's board vendor, which costs the town four emeralds.
 *   <li><b>Deeds</b> ({@link #present}): a deed brought to the hub's desk is handed to the core ({@code DeedBrought}) and
 *       the crop it pays out comes back for the player ({@code DeedPaid}). The hub's rents, one vendor in the row by
 *       its desk for each, are asked of the core ({@code Query.Deeds}) and the vendors put up.
 * </ul>
 *
 * <p>The crown's gold shop is the core's to name ({@code Input.CrownShop}): counted here like any vendor, the core says it
 * is the crown's and {@code CrownAdapter} shows it. Not here: enchanted commissions (a vendor whose filter asks for
 * enchantments deals in nothing the core knows), and vendors for the metal exchange, the requisitions and the freight hubs
 * (the core's exchange and requisitions deal with the markets' own Merchants; no mod puts such vendors up in core mode). In Java mode the
 * Java rules ({@code Broker}, {@code ShopBoard}, {@code RentDeeds}) keep the vendors and this adapter does nothing.
 */
public final class ShopAdapter {
    /** The coin a vendor's box holds comes out as the town's; a deed vendor's dealer card is a Numismatics red card. */
    private static final String DEALER_CARD = "numismatics:red_card";

    private final CoreHost host;
    /** Whether the core's shops are written to the vendors (core mode, a test's host); Java mode shows nothing. */
    private final boolean writes;
    /** The vendors the core sells rents' deeds at, as its {@code Query.Deeds} said at the last look: their boxes are the town's. */
    private final Set<GlobalPos> deedShops = new java.util.HashSet<>();
    /** Towns the core asked another board vendor for, for the next look. */
    private final Set<Long> toBuild = new LinkedHashSet<>();
    /** Who is bringing a deed to a desk now, and what they were handed. */
    private Player presenting;
    private int handed;

    public ShopAdapter(CoreHost host, boolean writes) {
        this.host = host;
        this.writes = writes;
    }

    /** Whether the core's shops are written to the vendors here. */
    public boolean writes() {
        return writes;
    }

    // ---- registering ----------------------------------------------------------------------------------------------

    /**
     * Before a look: the vendors near each loaded desk registered with the core, the vendors it asked for for a board put
     * up, the shops of its hubs' rents put up. Nothing in Java mode.
     */
    public void place(MinecraftServer server) {
        if (!writes) {
            return;
        }
        Map<Long, GlobalPos> desks = loadedDesks(server);
        deedShops.clear();
        for (var e : desks.entrySet()) {
            GlobalPos desk = e.getValue();
            ServerLevel level = server.getLevel(desk.dimension());
            for (VendorBlockEntity vendor : Shops.near(level, desk.pos())) {
                GlobalPos at = GlobalPos.of(level.dimension(), vendor.getBlockPos());
                if (host.points().at(at) == null) {
                    String why = host.points().add(at, PointsAdapter.Kind.SHOP, e.getKey());
                    if (why != null) {
                        Enterprise.LOGGER.warn("The core wouldn't take a shop at {}: {}", at.pos().toShortString(), why);
                    }
                }
            }
            if (toBuild.remove(e.getKey())) {
                buildBoardVendor(level, desk, e.getKey());
            }
        }
        if (!desks.isEmpty()) {
            List<Query> queries = new ArrayList<>();
            desks.keySet().forEach(t -> queries.add(new Query.Deeds(t)));
            List<Answer> answers = host.ask(queries);
            int i = 0;
            for (var e : desks.entrySet()) {
                if (answers.get(i++) instanceof Answer.Deeds deeds && deeds.hub()) {
                    ServerLevel level = server.getLevel(e.getValue().dimension());
                    for (DeedLine line : deeds.deeds()) {
                        keepDeedShop(level, e.getValue(), line);
                    }
                }
            }
        }
    }

    private Map<Long, GlobalPos> loadedDesks(MinecraftServer server) {
        Map<Long, GlobalPos> desks = new LinkedHashMap<>();
        for (var desk : host.points().of(PointsAdapter.Kind.DESK).entrySet()) {
            ServerLevel level = server.getLevel(desk.getKey().dimension());
            if (level != null && level.isLoaded(desk.getKey().pos())) {
                desks.putIfAbsent(desk.getValue(), desk.getKey());
            }
        }
        return desks;
    }

    private static Block vendorBlock() {
        return BuiltInRegistries.BLOCK.get(ResourceLocation.parse("numismatics:vendor"));
    }

    /** A vendor touching the desk, on open floor, for the town's board; registered with the core as the board's. */
    private void buildBoardVendor(ServerLevel level, GlobalPos desk, long town) {
        // Nearest first: touching the desk is what makes it the board's neighbour (and a vendor the town's own).
        List<int[]> around = new ArrayList<>();
        for (int dx = -Shops.OWN; dx <= Shops.OWN; dx++) {
            for (int dz = -Shops.OWN; dz <= Shops.OWN; dz++) {
                if (dx != 0 || dz != 0) {
                    around.add(new int[] {dx, dz});
                }
            }
        }
        around.sort(java.util.Comparator.comparingInt(d -> d[0] * d[0] + d[1] * d[1]));
        {
            for (int[] d : around) {
                int dx = d[0];
                int dz = d[1];
                BlockPos pos = desk.pos().offset(dx, 0, dz);
                if (!level.isLoaded(pos) || !level.getBlockState(pos).isAir() || level.getBlockState(pos.below()).isAir()
                        || !pos.closerThan(desk.pos(), Shops.OWN + 0.5)) {
                    continue;
                }
                level.setBlock(pos, vendorBlock().defaultBlockState(), 3);
                if (level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                    shop.setMode(VendorBlockEntity.Mode.SELL);
                    GlobalPos at = GlobalPos.of(level.dimension(), pos);
                    String why = host.points().at(at) != null ? null : host.points().add(at, PointsAdapter.Kind.SHOP, town);
                    if (why != null) {
                        Enterprise.LOGGER.warn("The core wouldn't take a shop at {}: {}", pos.toShortString(), why);
                        return;
                    }
                    host.apply(List.of(new Input.Board(PointsAdapter.point(PointsAdapter.Kind.SHOP, at))));
                    Enterprise.LOGGER.debug("Market at {} builds a shop at {}", desk.pos().toShortString(), pos.toShortString());
                    return;
                }
            }
        }
    }

    /** The vendor for a rent's deed: put up in the row by the desk if the rent has none, or its broken one forgotten. */
    private void keepDeedShop(ServerLevel level, GlobalPos desk, DeedLine line) {
        if (line.point().isPresent()) {
            GlobalPos at = PointsAdapter.place(line.point().get());
            if (!level.isLoaded(at.pos()) || level.getBlockEntity(at.pos()) instanceof VendorBlockEntity) {
                deedShops.add(at);
                return;
            }
            host.apply(List.of(new Input.DeedShop(line.rent(), Optional.empty()))); // broken: put up another
            host.points().remove(at);
        }
        for (BlockPos pos : RentDeeds.sites(desk.pos())) {
            if (!level.isLoaded(pos) || !level.getBlockState(pos).isAir()) {
                continue;
            }
            level.setBlock(pos, vendorBlock().defaultBlockState(), 3);
            if (level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                shop.setMode(VendorBlockEntity.Mode.SELL);
                GlobalPos at = GlobalPos.of(level.dimension(), pos);
                if (host.points().at(at) == null && host.points().add(at, PointsAdapter.Kind.SHOP,
                        host.points().townAt(desk)) != null) {
                    return;
                }
                host.apply(List.of(new Input.DeedShop(line.rent(), Optional.of(PointsAdapter.point(PointsAdapter.Kind.SHOP, at)))));
                deedShops.add(at);
                return;
            }
        }
    }

    // ---- counting -------------------------------------------------------------------------------------------------

    /**
     * Every vendor the core keeps as a town's shop, counted (a look's, before its time): those whose town's desk and own
     * chunk are loaded. A vendor gone from a loaded chunk has its point removed. Nothing in Java mode.
     */
    public List<Input> happenings(MinecraftServer server) {
        if (!writes) {
            return List.of();
        }
        Map<Long, GlobalPos> desks = loadedDesks(server);
        List<Input> out = new ArrayList<>();
        List<GlobalPos> gone = new ArrayList<>();
        for (var e : host.points().of(PointsAdapter.Kind.SHOP).entrySet()) {
            GlobalPos desk = desks.get(e.getValue());
            ServerLevel level = server.getLevel(e.getKey().dimension());
            if (desk == null || level == null || !level.isLoaded(e.getKey().pos())) {
                continue;
            }
            if (level.getBlockEntity(e.getKey().pos()) instanceof VendorBlockEntity vendor) {
                out.add(new Input.Vendor(PointsAdapter.point(PointsAdapter.Kind.SHOP, e.getKey()),
                        read(vendor, desk.pos(), deedShops.contains(e.getKey()))));
            } else {
                gone.add(e.getKey());
            }
        }
        gone.forEach(host.points()::remove);
        return out;
    }

    /**
     * What a vendor holds and asks, as facts; its coin box emptied when what's in it is the town's: a vendor touching the
     * desk (the core's own shop) or one the core sells a deed at ({@code deedShop}, its {@code Query.Deeds}), never a
     * player's, whatever its filter holds.
     */
    VendorRead read(VendorBlockEntity vendor, BlockPos desk, boolean deedShop) {
        ItemStack filter = vendor.getFilterItem();
        boolean touches = Shops.own(vendor, desk);
        Optional<Integer> deed = RentDeeds.rentOf(filter);
        int good = deed.isEmpty() && !filter.isEmpty() && !Shops.enchanted(filter) ? host.stock().good(filter.getItem()) : -1;
        VendorMode mode = filter.isEmpty() || deed.isEmpty() && good < 0 ? VendorMode.Idle
                : vendor.getMode() == VendorBlockEntity.Mode.BUY ? VendorMode.Buy : VendorMode.Sell;
        long stock = filter.isEmpty() ? 0 : vendor.items.stream().filter(vendor::matchesFilterItem).mapToLong(ItemStack::getCount).sum();
        long purse = Shops.purse(vendor).map(a -> (long) a.getBalance()).orElse(0L);
        long coins = touches || deedShop ? Shops.collect(vendor) : 0;
        return new VendorRead(mode, good < 0 ? Optional.empty() : Optional.of((long) good),
                deed.isPresent() ? Optional.of((long) deed.get()) : Optional.empty(), Math.max(1, filter.getCount()),
                Math.max(0, vendor.getTotalPrice()), stock, mode == VendorMode.Buy ? Shops.roomFor(vendor) : 0, purse, coins,
                touches);
    }

    // ---- showing --------------------------------------------------------------------------------------------------

    /** Shows what the core did at a shop. Nothing in Java mode, where the Java rules keep the vendors. */
    public void show(Output output) {
        if (!writes) {
            return;
        }
        switch (output) {
            case Output.OrderFilled o -> filled(o);
            case Output.StallBought o -> bought(o);
            case Output.Shelf o -> shelf(o);
            case Output.Deed o -> deed(o);
            case Output.DeedPaid o -> paid(o);
            case Output.BuildShop o -> toBuild.add(o.town());
            case Output.Signal o -> Enterprise.LOGGER.debug("Town {} wants {} of good {} that no market has, paying {} spurs each",
                    o.town(), o.count(), o.good(), o.spursEach());
            default -> {}
        }
    }

    private VendorBlockEntity vendorAt(Point point) {
        MinecraftServer server = ServerLifecycleHooks.getCurrentServer();
        GlobalPos at = PointsAdapter.place(point);
        ServerLevel level = server == null ? null : server.getLevel(at.dimension());
        if (level != null && level.isLoaded(at.pos()) && level.getBlockEntity(at.pos()) instanceof VendorBlockEntity v) {
            return v;
        }
        Enterprise.LOGGER.debug("No loaded vendor at {} to show a shop", at.pos().toShortString());
        return null;
    }

    /** An order filled: its lots handed to the vendor, and what they cost taken off its card. */
    private void filled(Output.OrderFilled o) {
        if (o.point().isEmpty()) {
            return;
        }
        VendorBlockEntity vendor = vendorAt(o.point().get());
        if (vendor == null) {
            return;
        }
        ItemStack lot = vendor.getFilterItem();
        for (long i = 0; i < o.lots(); i++) {
            Shops.sellTo(vendor, lot.copy(), 0);
        }
        Shops.purse(vendor).ifPresent(account -> account.deduct((int) Math.round(o.spurs()), new ReasonHolder()));
    }

    /** A stall bought from: its lots out of the vendor, their price paid into its box. */
    private void bought(Output.StallBought o) {
        VendorBlockEntity vendor = vendorAt(o.point());
        if (vendor == null) {
            return;
        }
        for (long i = 0; i < o.lots(); i++) {
            Shops.buyFrom(vendor); // the goods are the town's in the core; its chests are written by Moved
        }
    }

    /** The town's own or its board's shop: what it shows, with goods added to its shelf or cleared out of it. */
    private void shelf(Output.Shelf o) {
        VendorBlockEntity vendor = vendorAt(o.point());
        if (vendor == null) {
            return;
        }
        Item item = o.good().isPresent() ? host.stock().item(o.good().get()) : net.minecraft.world.item.Items.AIR;
        if (item == net.minecraft.world.item.Items.AIR) {
            Shops.offer(vendor, ItemStack.EMPTY, 0);
        } else {
            ItemStack filter = vendor.getFilterItem();
            if (!filter.is(item) || filter.getCount() != o.lot() || vendor.getMode() != VendorBlockEntity.Mode.SELL) {
                Shops.offer(vendor, new ItemStack(item, (int) o.lot()), (int) o.price());
            } else {
                Shops.setPrice(vendor, (int) o.price());
            }
        }
        if (o.clear()) {
            Shops.clearOthers(vendor); // the town has the old shelf back in its stock (Moved writes its chests)
        }
        if (o.add() > 0 && item != net.minecraft.world.item.Items.AIR) {
            Shops.stock(vendor, new ItemStack(item, (int) Math.min(o.add(), Integer.MAX_VALUE)));
        } else if (o.add() < 0) {
            long take = -o.add();
            for (int i = 0; i < vendor.items.size() && take > 0; i++) {
                ItemStack slot = vendor.items.get(i);
                if (!slot.isEmpty() && vendor.matchesFilterItem(slot)) {
                    take -= slot.split((int) Math.min(take, slot.getCount())).getCount();
                }
            }
            vendor.setChanged();
        }
    }

    /** A deed's vendor: sells the deed at the ask, or buys it back at the bid from the dealer's card, funded. */
    private void deed(Output.Deed o) {
        VendorBlockEntity vendor = vendorAt(o.point());
        if (vendor == null) {
            return;
        }
        Optional<DeedLine> line = deedLine(vendor, o.rent());
        if (line.isPresent()) {
            showDeed(vendor, o, line.get());
        }
    }

    /** The vendor of a rent's deed (the rent as the core's {@code Query.Deeds} lists it) set to what the core says. */
    public void showDeed(VendorBlockEntity vendor, Output.Deed o, DeedLine line) {
        ItemStack deed = RentDeeds.deed((int) o.rent(), summary(line));
        if (o.selling()) {
            vendor.cardContainer.setItem(0, ItemStack.EMPTY);
            ItemStack filter = vendor.getFilterItem();
            if (vendor.getMode() != VendorBlockEntity.Mode.SELL || !vendor.matchesFilterItem(deed) || filter.isEmpty()) {
                Shops.offer(vendor, deed, (int) o.price());
            } else {
                Shops.setPrice(vendor, (int) o.price());
            }
            if (vendor.items.stream().noneMatch(vendor::matchesFilterItem)) {
                Shops.stock(vendor, deed.copy());
            }
        } else {
            UUID id = dealerAccount(PointsAdapter.place(o.point()));
            BankAccount account = Numismatics.BANK.getOrCreateAccount(id, BankAccount.Type.PLAYER);
            if (o.fund() > 0) {
                account.deposit((int) Math.min(Integer.MAX_VALUE, o.fund()));
            }
            if (vendor.getMode() != VendorBlockEntity.Mode.BUY || !vendor.matchesFilterItem(deed)) {
                vendor.setMode(VendorBlockEntity.Mode.BUY);
                vendor.filterContainer.setItem(0, deed);
                ItemStack card = new ItemStack(BuiltInRegistries.ITEM.get(ResourceLocation.parse(DEALER_CARD)));
                CardItem.set(card, id);
                vendor.cardContainer.setItem(0, card);
            }
            Shops.setPrice(vendor, (int) o.price());
        }
    }

    /** The dealer's bank account for a hub's deed shops: one per hub, named by where the hub's desk is (nothing to keep). */
    private UUID dealerAccount(GlobalPos shop) {
        Long town = host.points().townAt(shop);
        GlobalPos desk = town == null ? shop : host.points().of(PointsAdapter.Kind.DESK).entrySet().stream()
                .filter(e -> e.getValue().equals(town)).map(Map.Entry::getKey).findFirst().orElse(shop);
        return UUID.nameUUIDFromBytes(("enterprise:dealer:" + desk.dimension().location() + desk.pos().asLong())
                .getBytes(java.nio.charset.StandardCharsets.UTF_8));
    }

    private Optional<DeedLine> deedLine(VendorBlockEntity vendor, long rent) {
        Long town = host.points().townAt(GlobalPos.of(vendor.getLevel().dimension(), vendor.getBlockPos()));
        if (town == null) {
            return Optional.empty();
        }
        return ((Answer.Deeds) host.ask(List.of(new Query.Deeds(town))).get(0)).deeds().stream()
                .filter(d -> d.rent() == rent).findFirst();
    }

    private String summary(DeedLine line) {
        return "%d %s a season from %s".formatted((long) line.quantity(),
                BuiltInRegistries.ITEM.getKey(host.stock().item(line.crop())).getPath(), line.farmName());
    }

    /** The crop a deed pays out, for the player who brought it. */
    private void paid(Output.DeedPaid o) {
        if (presenting == null) {
            Enterprise.LOGGER.debug("A deed paid {} of good {} with no one to hand it to", o.count(), o.good());
            return;
        }
        handed = handOver(presenting, o);
    }

    /** What a deed pays out put in a player's hands (the inventory, else dropped), and told them. Returns how many. */
    public int handOver(Player player, Output.DeedPaid o) {
        Item crop = host.stock().item(o.good());
        long left = o.count();
        while (left > 0) {
            ItemStack piece = new ItemStack(crop, (int) Math.min(left, crop.getDefaultMaxStackSize()));
            left -= piece.getCount();
            if (!player.getInventory().add(piece)) {
                player.drop(piece, false);
            }
        }
        player.displayClientMessage(Component.literal("%d %s handed over for your rent%s".formatted(o.count(),
                BuiltInRegistries.ITEM.getKey(crop).getPath(), o.owed() > 0 ? "; %d more owed, when it's in".formatted(o.owed()) : "")),
                false);
        return (int) o.count();
    }

    /**
     * A player brings a rent's deed to a hub's desk: the crop collected for it handed over, as far as the town's stock goes
     * (the desk's chests counted first, so the goods leave them the same moment). Returns how many.
     */
    public int present(ServerLevel level, BlockPos desk, ItemStack deed, Player player) {
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        Long town = host.stock().townAt(at);
        Optional<Integer> rent = RentDeeds.rentOf(deed);
        if (town == null || rent.isEmpty()) {
            return 0;
        }
        presenting = player;
        handed = 0;
        try {
            Response r = host.apply(List.of(host.stock().stocktake(level, PointsAdapter.Kind.DESK, at),
                    new Input.DeedBrought(town, rent.get())));
            if (r instanceof Response.Refused refused) {
                Enterprise.LOGGER.warn("The core wouldn't hand over for a deed: {}", refused.why());
            }
            return handed;
        } finally {
            presenting = null;
        }
    }

    // ---- asking ---------------------------------------------------------------------------------------------------

    /** A town's shops as the core has them, in words ({@code /enterprise core shops <desk>}, {@code /enterprise market}). */
    public List<String> lines(long town) {
        Answer.Shops a = (Answer.Shops) host.ask(List.of(new Query.Shops(town))).get(0);
        List<String> lines = new ArrayList<>();
        var t = a.tally();
        lines.add(("The core's town %d: %.0f lots sold to players' shops for %.0f spurs, %.0f bought from them for %.0f, its own "
                + "shops took %.0f").formatted(town, t.lotsSold(), t.spursFromSales(), t.lotsBought(), t.spursOnPurchases(),
                t.spursFromOwnShops()));
        for (ShopLine s : a.shops()) {
            String good = s.good().map(g -> BuiltInRegistries.ITEM.getKey(host.stock().item(g)).getPath()).orElse("nothing");
            String where = "%d %d %d".formatted(s.point().x(), s.point().y(), s.point().z());
            lines.add(switch (s.role()) {
                case Order -> "  order at %s: %.0f %s at %.0f spurs a lot, %.0f delivered for %.0f, %.0f open, %.0f left on its card"
                        .formatted(where, s.lot(), good, s.price(), s.delivered(), s.paid(), s.open(), s.purse());
                case Stall -> "  stall at %s: %.0f %s in lots of %.0f at %.0f spurs, %.0f on its shelf"
                        .formatted(where, s.stock(), good, s.lot(), s.price(), s.stock());
                case Own -> "  the town's own shop at %s: %s in lots of %.0f at %.0f spurs, %.0f on its shelf"
                        .formatted(where, good, s.lot(), s.price(), s.stock());
                case Board -> "  the board's shop at %s: %s in lots of %.0f at %.0f spurs, %.0f on its shelf"
                        .formatted(where, good, s.lot(), s.price(), s.stock());
                case Deed -> "  deed shop at %s for rent %s".formatted(where, s.rent().map(String::valueOf).orElse("?"));
                case Idle -> "  a shop at %s the core deals in nothing with".formatted(where);
            });
        }
        a.wanted().forEach(c -> lines.add("  ordered: %.0f %s, the region's caravans bring it".formatted(c.count(),
                BuiltInRegistries.ITEM.getKey(host.stock().item(c.good())).getPath())));
        a.signals().forEach(s -> lines.add("  wanted, and no market it knows has it: %.0f %s, paying %.0f spurs each (for %s)"
                .formatted(s.count(), BuiltInRegistries.ITEM.getKey(host.stock().item(s.good())).getPath(), s.spursEach(), s.why())));
        a.listings().forEach(l -> lines.add("  the board lists %d %s at %d spurs".formatted(l.lot(),
                BuiltInRegistries.ITEM.getKey(host.stock().item(l.good())).getPath(), l.spurs())));
        return lines;
    }
}
