package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.market.Shops;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.MetalLine;
import dev.b28.enterprise.wire.Output;
import dev.b28.enterprise.wire.Point;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.RealmLine;
import dev.b28.enterprise.wire.Response;
import dev.ithundxr.createnumismatics.Numismatics;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import dev.ithundxr.createnumismatics.content.bank.CardItem;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.server.ServerLifecycleHooks;

/**
 * The sovereign, its realms and the markets off the map, from the core (docs/binding-plan.md 2, concern 10). The crown's
 * rules are the core's ({@code crown.rs}, {@code army.rs}, {@code realms.rs}, {@code sourcing.rs}, {@code
 * crown_host.rs}): it mints, stockpiles, levies, fights and trades on the books, and its soldiers are hands (stowed and
 * let out by {@code WagonAdapter}). What the world has of it is commands, a vendor and points, and this adapter is how:
 *
 * <ul>
 *   <li><b>Commands</b> ({@link #capital}, {@link #mobilise}, {@link #declare}, {@link #peace}, {@link #openRealm}): a
 *       desk of the core's named the capital, the crown mobilised, war declared on a theatre, peace made, a realm opened
 *       off the map at a block with its roads to the desks it trades with. Each is an input; the core says what came of it.
 *   <li><b>Markets off the map are points</b>: a realm's block is a {@code Market} point of the core's ({@link
 *       PointsAdapter.Kind#MARKET}), with a town and no chests and no one to see; the front and the market across the sea
 *       the core opens are listed with them.
 *   <li><b>The gold shop</b> ({@code BuildCrownShop}, {@code CrownShop}, {@code CrownShopDown}): mobilising, the core asks for
 *       a vendor by the capital's desk that buys gold; it is put up on open floor near the desk and registered as the
 *       crown's, counted each look like any vendor ({@code ShopAdapter}), shown what the core says (the gold, the lot, the
 *       price, a card funded from the treasury) and relieved of the gold players sold into it, which is the crown's.
 *       At peace it comes down.
 *   <li><b>The books</b> ({@link #realms}, {@link #metals}): what {@code /enterprise realms} and {@code metals} show.
 * </ul>
 *
 * <p>The Java rules ({@code Sovereign}, {@code Requisitions}, {@code Army}, {@code Realms}, {@code OffMap}) keep all of this
 * in Java mode, and this adapter does nothing then.
 */
public final class CrownAdapter {
    /** The gold shop stands within this many blocks of the capital's desk ({@code Sovereign.mobilise}: ten north of it). */
    public static final int REACH = 10;
    private static final String CARD = "numismatics:red_card";

    private final CoreHost host;
    /** Whether the core's crown is written to the world (core mode, a test's host); Java mode shows nothing. */
    private final boolean writes;
    /** Towns the core asked a gold shop of, for the next look (the capital's desk wasn't loaded, or just now asked). */
    private final Set<Long> toBuild = new LinkedHashSet<>();
    /** What the last command's outputs said: the towns levied, the soldiers sent home. */
    private int levies;
    private int home;

    public CrownAdapter(CoreHost host, boolean writes) {
        this.host = host;
        this.writes = writes;
    }

    // ---- commands -------------------------------------------------------------------------------------------------

    /** The core's town whose desk is at {@code desk}, or a sentence on why there is none. */
    private Object town(ServerLevel level, BlockPos desk) {
        Long town = host.stock().townAt(GlobalPos.of(level.dimension(), desk));
        return town == null ? "The core keeps no town at " + desk.toShortString() : town;
    }

    private Response send(ServerLevel level, Input... inputs) {
        List<Input> all = new ArrayList<>();
        all.add(new Input.Advance(level.getServer().overworld().getGameTime()));
        all.addAll(List.of(inputs));
        Response r = host.apply(all);
        host.points().reload(); // the front and the market across the sea the core opened are points too
        return r;
    }

    /** Names the desk's town the capital, its treasury taking {@code perDay} spurs a day. Null when done, else why not. */
    public String capital(ServerLevel level, BlockPos desk, long perDay) {
        if (!(town(level, desk) instanceof Long town)) {
            return (String) town(level, desk);
        }
        return refusal(send(level, new Input.Capital(town, perDay)));
    }

    /** The crown mobilises at the desk's town, minting {@code perDay} spurs a day. Null when done, else why not. */
    public String mobilise(ServerLevel level, BlockPos desk, long perDay) {
        if (!(town(level, desk) instanceof Long town)) {
            return (String) town(level, desk);
        }
        return refusal(send(level, new Input.Mobilise(town, perDay)));
    }

    /** War on {@code theatre} (faction, undead or nether). Returns the towns levied, or -1 with why in {@code why[0]}. */
    public int declare(ServerLevel level, String theatre, String[] why) {
        levies = 0;
        String refused = refusal(send(level, new Input.Declare(theatre.toLowerCase(Locale.ROOT))));
        if (refused != null) {
            why[0] = refused;
            return -1;
        }
        return levies;
    }

    /** Peace. Returns the soldiers and recruits sent home. */
    public int peace(ServerLevel level) {
        home = 0;
        send(level, new Input.Peace());
        return home;
    }

    /**
     * Opens a realm ({@code coast}, {@code march}) off the map at {@code home} with 20,000 spurs, trading with the core's
     * desks at {@code partners}. Null when done, else why not.
     */
    public String openRealm(ServerLevel level, String realm, BlockPos home, List<BlockPos> partners) {
        List<Long> towns = new ArrayList<>();
        for (BlockPos p : partners) {
            if (!(town(level, p) instanceof Long t)) {
                return (String) town(level, p);
            }
            towns.add(t);
        }
        String name = "coast".equals(realm) ? "the Coast" : "march".equals(realm) ? "the March" : realm;
        return openMarket(level, name, realm, home, true, 20_000, towns);
    }

    /** A market off the map at {@code at}: a {@code Market} point of the core's with a town of its own. Null when done. */
    public String openMarket(ServerLevel level, String name, String realm, BlockPos at, boolean sells, long spurs, List<Long> partners) {
        Point point = PointsAdapter.point(PointsAdapter.Kind.MARKET, GlobalPos.of(level.dimension(), at));
        String why = refusal(send(level, new Input.OffMap(name, point, sells, realm, spurs, partners)));
        host.points().reload();
        return why;
    }

    private static String refusal(Response r) {
        return r instanceof Response.Refused refused ? refused.why().toString() : r == null ? "The core isn't running" : null;
    }

    // ---- the books ------------------------------------------------------------------------------------------------

    /** Each realm's trade, in the words of {@code Realms.report}. */
    public List<String> realms() {
        Answer.Realms a = (Answer.Realms) host.ask(List.of(new Query.Realms())).get(0);
        List<String> lines = new ArrayList<>();
        for (RealmLine r : a.realms()) {
            lines.add("%s: %d trips to %d markets, %.0f spurs sold, %.0f bought, %.0f customs paid; %.0f goods made; %.0f spurs in hand"
                    .formatted(r.name(), r.trips(), r.partners(), r.soldSpurs(), r.boughtSpurs(), r.customsSpurs(), r.made(),
                            r.inHandSpurs()));
        }
        return lines;
    }

    /** The metal exchange: its desk, the phase, and each metal. Empty when no capital is named. */
    public List<String> metals() {
        Answer.Metals a = (Answer.Metals) host.ask(List.of(new Query.Metals())).get(0);
        List<String> lines = new ArrayList<>();
        if (a.desk().isEmpty()) {
            return lines;
        }
        lines.add("Metal exchange at the core's town %d (%s)".formatted(a.desk().get(), a.phase()));
        for (MetalLine m : a.metals()) {
            lines.add("  %s: holds %.0f of %.0f, %.0f coming under contract; pays %.2f times its worth".formatted(
                    m.name().replaceFirst("^[^:]*:", ""), m.held(), m.target(), m.coming(), m.premium()));
        }
        return lines;
    }

    // ---- the gold shop --------------------------------------------------------------------------------------------

    /** Before a look: the gold shop the core asked for put up, if its capital's desk is loaded. Nothing in Java mode. */
    public void place(MinecraftServer server) {
        if (!writes || toBuild.isEmpty()) {
            return;
        }
        for (Long town : List.copyOf(toBuild)) {
            GlobalPos desk = host.stock().desks().entrySet().stream().filter(e -> e.getValue().equals(town))
                    .map(java.util.Map.Entry::getKey).findFirst().orElse(null);
            ServerLevel level = desk == null ? null : server.getLevel(desk.dimension());
            if (level == null) {
                toBuild.remove(town);
                continue;
            }
            if (!level.isLoaded(desk.pos())) {
                continue;
            }
            toBuild.remove(town);
            BlockPos pos = site(level, desk.pos());
            if (pos == null) {
                Enterprise.LOGGER.warn("No open floor near the capital's desk at {} for the crown's gold shop", desk.pos().toShortString());
                continue;
            }
            level.setBlock(pos, BuiltInRegistries.BLOCK.get(ResourceLocation.parse("numismatics:vendor")).defaultBlockState(), 3);
            if (level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                shop.setMode(VendorBlockEntity.Mode.BUY);
                GlobalPos at = GlobalPos.of(level.dimension(), pos);
                String why = host.points().at(at) != null ? null : host.points().add(at, PointsAdapter.Kind.SHOP, town);
                if (why != null) {
                    Enterprise.LOGGER.warn("The core wouldn't take the crown's shop at {}: {}", pos.toShortString(), why);
                    continue;
                }
                host.apply(List.of(new Input.CrownShop(PointsAdapter.point(PointsAdapter.Kind.SHOP, at))));
                Enterprise.LOGGER.info("The crown's gold shop stands at {}", pos.toShortString());
            }
        }
    }

    /**
     * Where the gold shop goes: open floor, loaded, nearest to the desk first, past what makes a vendor the town's own
     * ({@link Shops#OWN}) and within {@link #REACH} of it. Null if there's none.
     */
    private static BlockPos site(ServerLevel level, BlockPos desk) {
        BlockPos best = null;
        double bestDistance = Double.MAX_VALUE;
        for (int dx = -REACH; dx <= REACH; dx++) {
            for (int dz = -REACH; dz <= REACH; dz++) {
                BlockPos pos = desk.offset(dx, 0, dz);
                double distance = Math.sqrt(dx * dx + dz * dz);
                if (distance < Shops.OWN + 1 || distance > REACH || distance >= bestDistance || !level.isLoaded(pos)
                        || !level.getBlockState(pos).isAir() || level.getBlockState(pos.below()).isAir()) {
                    continue;
                }
                best = pos;
                bestDistance = distance;
            }
        }
        return best;
    }

    // ---- showing --------------------------------------------------------------------------------------------------

    /** Shows what the core did about the crown. Nothing in Java mode, where the Java rules keep it. */
    public void show(Output output) {
        switch (output) {
            case Output.WarDeclared o -> levies = (int) o.levies();
            case Output.PeaceMade o -> home = (int) o.home();
            case Output.BuildCrownShop o -> {
                if (writes) {
                    toBuild.add(o.town());
                }
            }
            case Output.CrownShop o -> {
                if (writes) {
                    shop(o);
                }
            }
            case Output.CrownShopDown o -> {
                if (writes) {
                    down(o);
                }
            }
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
        return null;
    }

    /** The gold shop: set to buy the lot of gold at the price, its card funded, what players sold into it taken out. */
    private void shop(Output.CrownShop o) {
        VendorBlockEntity vendor = vendorAt(o.point());
        if (vendor == null) {
            return;
        }
        Item gold = host.stock().item(o.good());
        UUID id = UUID.nameUUIDFromBytes(("enterprise:crown:" + o.point().dim() + o.point().x() + "," + o.point().y() + ","
                + o.point().z()).getBytes(java.nio.charset.StandardCharsets.UTF_8));
        BankAccount account = Numismatics.BANK.getOrCreateAccount(id, BankAccount.Type.PLAYER);
        if (o.fund() > 0) {
            account.deposit((int) Math.min(Integer.MAX_VALUE, o.fund()));
        }
        if (vendor.getMode() != VendorBlockEntity.Mode.BUY || !vendor.getFilterItem().is(gold)
                || vendor.getFilterItem().getCount() != o.lot()) {
            vendor.setMode(VendorBlockEntity.Mode.BUY);
            vendor.filterContainer.setItem(0, new ItemStack(gold, (int) o.lot()));
        }
        ItemStack card = new ItemStack(BuiltInRegistries.ITEM.get(ResourceLocation.parse(CARD)));
        CardItem.set(card, id);
        vendor.cardContainer.setItem(0, card);
        Shops.setPrice(vendor, (int) Math.min(Integer.MAX_VALUE, o.price()));
        long take = o.taken();
        for (int i = 0; i < vendor.items.size() && take > 0; i++) {
            ItemStack slot = vendor.items.get(i);
            if (!slot.isEmpty() && vendor.matchesFilterItem(slot)) {
                take -= slot.split((int) Math.min(take, slot.getCount())).getCount();
            }
        }
        vendor.setChanged();
    }

    /** Peace: the vendor comes down, and the core's point for it is removed. */
    private void down(Output.CrownShopDown o) {
        MinecraftServer server = ServerLifecycleHooks.getCurrentServer();
        GlobalPos at = PointsAdapter.place(o.point());
        ServerLevel level = server == null ? null : server.getLevel(at.dimension());
        if (level != null && level.isLoaded(at.pos()) && level.getBlockEntity(at.pos()) instanceof VendorBlockEntity) {
            level.setBlock(at.pos(), Blocks.AIR.defaultBlockState(), 3);
        }
        host.points().remove(at);
    }
}
