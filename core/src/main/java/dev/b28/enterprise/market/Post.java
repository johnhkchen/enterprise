package dev.b28.enterprise.market;

import dev.b28.enterprise.Dials;
import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.cargo.Cargo;
import dev.b28.enterprise.contract.Bills;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.merchant.Depots;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.OffMap;
import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.territory.Territory;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * The crown's post: riders who carry letters between markets on the post days, every {@code post_days}, faster than
 * any wagon. A letter costs its sender the postage ({@code postage}); the crown keeps the riders. A letter waits at
 * the desk it's for until its Merchant next looks (so a town no one has loaded still gets its mail).
 * <ul>
 *   <li><b>Come call:</b> a town with goods a hub wants, worth the hub's trip, writes to say so. The hub weighs the trip
 *   against goods it now knows are there, and books the call into a day with a wagon free (one call a day: it never
 *   promises more visits than it can make), or declines. Its run there leaves only on a booked day, with a purse for
 *   what's offered. The crown pays half the road food of a hub's first visit to a town, to get a route going.</li>
 *   <li><b>Bills by post:</b> a town holding bills on a market no caravan of its goes to sends them by post; the
 *   drawee pays when it reads them, and the coin comes back by post, with any bill it couldn't pay yet.</li>
 * </ul>
 */
public final class Post extends SavedData {
    private static final String NAME = "enterprise_post";

    public enum Kind { CALL, REPLY, BILLS, REMIT }

    /** A letter: what it is, from and to, when it arrives, and what it carries. */
    static final class Letter {
        Kind kind;
        GlobalPos from;
        GlobalPos to;
        long posted;
        long arrives;
        double value; // a call: what the goods offered are worth; a remittance: spurs paid
        long day = -1; // a reply: the day booked (or -1, declined)
        List<ItemStack> papers = new ArrayList<>(); // bills sent, or returned
    }

    /** A call a hub has booked: the town, the day, and the worth of the goods offered. */
    public record Call(GlobalPos hub, GlobalPos town, long day, double value) {}

    private final List<Letter> letters = new ArrayList<>();
    private final List<Call> calls = new ArrayList<>();
    private final Map<GlobalPos, Long> lastWrote = new HashMap<>();
    private long sent;

    public static Post get(MinecraftServer server) {
        return server.overworld().getDataStorage().computeIfAbsent(new SavedData.Factory<>(Post::new, Post::load, null), NAME);
    }

    /** When a letter posted now arrives: it leaves on the next post day, and a rider covers {@code post_speed} a day. */
    static long arrival(GlobalPos from, GlobalPos to, long now) {
        long every = Math.max(1, Dials.POST_DAYS.getInt()) * 24000L;
        long leaves = (now + every - 1) / every * every;
        double blocks = Math.sqrt(from.pos().distSqr(to.pos()));
        return leaves + Math.max(2400, (long) (blocks / Dials.POST_SPEED.get() * 24000));
    }

    /** Posts a letter, the sender paying the postage. */
    void post(ServerLevel level, MerchantStock stock, Letter letter, long now) {
        Treasury.withdraw(level, letter.from.pos(), stock, Dials.POSTAGE.get(), Mint.get(level.getServer()), now);
        letter.posted = now;
        letter.arrives = arrival(letter.from, letter.to, now);
        letters.add(letter);
        sent++;
        setDirty();
    }

    /** The market at {@code desk} was touched: its Merchant reads its mail, and writes what it has to. */
    public static void atDesk(ServerLevel level, GlobalPos desk, MerchantStock stock, long now) {
        if (OffMap.get(level.getServer()).is(desk) || dev.b28.enterprise.trade.Realms.get(level.getServer()).isForeign(desk)) {
            return; // the crown's riders don't go beyond its border
        }
        Post post = get(level.getServer());
        post.read(level, desk, stock, now);
        if (post.lastWrote.getOrDefault(desk, -1L) != now / 24000) {
            post.lastWrote.put(desk, now / 24000);
            post.write(level, desk, stock, now);
        }
    }

    private void read(ServerLevel level, GlobalPos desk, MerchantStock stock, long now) {
        List<Letter> mine = letters.stream().filter(l -> l.to.equals(desk) && l.arrives <= now).toList();
        for (Letter letter : mine) {
            letters.remove(letter);
            switch (letter.kind) {
                case CALL -> callIn(level, letter, stock, now);
                case BILLS -> honour(level, letter, stock, now);
                case REMIT -> {
                    if (letter.value > 0) {
                        Treasury.deposit(level, desk.pos(), stock, (int) Math.round(letter.value));
                    }
                    letter.papers.forEach(stock::insert);
                }
                case REPLY -> { } // the town knows when to have its goods ready
            }
            setDirty();
        }
    }

    private void write(ServerLevel level, GlobalPos desk, MerchantStock stock, long now) {
        offer(level, desk, stock, now);
        mailBills(level, desk, stock, now);
    }

    /** The hubs the town's riders reach: freight hubs within a wagon's range, nearest first. */
    static List<GlobalPos> hubsFor(MinecraftServer server, GlobalPos town) {
        return Depots.get(server).all().stream().filter(d -> d.kind() == Depots.Kind.STOREHOUSE).map(Depots.Depot::desk)
                .filter(h -> !h.equals(town) && h.dimension().equals(town.dimension())
                        && Caravans.inRange((int) Math.sqrt(h.pos().distSqr(town.pos()))))
                .distinct()
                .sorted(java.util.Comparator.comparingDouble((GlobalPos h) -> h.pos().distSqr(town.pos()))).toList();
    }

    /** What the town holds beyond its own par of goods {@code hub} wants, in emeralds, a wagon's worth at most. */
    static double offered(MinecraftServer server, GlobalPos town, GlobalPos hub, MerchantStock stock, long now) {
        Warehouse warehouse = Warehouse.get(server);
        var wanted = Caravans.wantedAt(server, hub);
        var own = Caravans.wantedAt(server, town);
        double worth = 0;
        int stacks = 0;
        for (Map.Entry<String, Integer> held : stock.totals().entrySet()) {
            Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(held.getKey()));
            if (!wanted.contains(item)) {
                continue;
            }
            int spare = held.getValue() - warehouse.par(town, item, own.contains(item), now);
            if (spare <= 0) {
                continue;
            }
            int room = (Caravans.CARGO_SLOTS - stacks) * item.getDefaultMaxStackSize();
            int going = Math.min(spare, Math.max(0, room));
            stacks += (going + item.getDefaultMaxStackSize() - 1) / item.getDefaultMaxStackSize();
            worth += going * Prices.emeraldsEach(item);
        }
        return worth;
    }

    /** What a hub reckons it makes on goods offered, against what its trip there costs. */
    static double tripCost(GlobalPos hub, GlobalPos town) {
        return Freight.runCost((int) Math.sqrt(hub.pos().distSqr(town.pos())));
    }

    /**
     * A town with no run of its own to a hub, and none of the hub's that calls on its own: when what it has for the hub
     * would pay for its trip by {@code call_start}, it writes to come call (unless a call's already asked or booked).
     */
    private void offer(ServerLevel level, GlobalPos town, MerchantStock stock, long now) {
        MinecraftServer server = level.getServer();
        if (Freight.isHub(server, town) || Sovereign.get(server).desk().filter(town::equals).isPresent()) {
            return; // a hub, or the capital: their own caravans go out
        }
        CaravanLedger ledger = CaravanLedger.get(server);
        for (GlobalPos hub : hubsFor(server, town)) {
            boolean served = ledger.route(town, hub).isPresent()
                    || ledger.route(hub, town).filter(r -> !r.onCall()).isPresent();
            boolean asked = letters.stream().anyMatch(l -> l.kind == Kind.CALL && l.from.equals(town) && l.to.equals(hub))
                    || calls.stream().anyMatch(c -> c.hub().equals(hub) && c.town().equals(town))
                    || ledger.route(hub, town).flatMap(r -> ledger.caravan(r.id())).isPresent();
            if (served || asked) {
                continue;
            }
            double worth = offered(server, town, hub, stock, now);
            if (worth * Dials.CALL_MARGIN.get() >= Dials.CALL_START.get() * tripCost(hub, town)) {
                Letter letter = new Letter();
                letter.kind = Kind.CALL;
                letter.from = town;
                letter.to = hub;
                letter.value = worth;
                post(level, stock, letter, now);
                dev.b28.enterprise.Enterprise.LOGGER.info("{} writes to {}: come call, {} emeralds of goods ready",
                        dev.b28.enterprise.settlement.Names.label(town), dev.b28.enterprise.settlement.Names.label(hub),
                        Math.round(worth));
                return; // one hub at a time
            }
        }
    }

    /**
     * A hub reads a town's letter: it books the call if what's offered pays for the trip ({@code call_stop}; half the
     * road food from the crown on a first visit), on the first day with no other call booked, and runs there on call.
     */
    private void callIn(ServerLevel level, Letter letter, MerchantStock stock, long now) {
        MinecraftServer server = level.getServer();
        GlobalPos hub = letter.to;
        GlobalPos town = letter.from;
        CaravanLedger ledger = CaravanLedger.get(server);
        var route = ledger.route(hub, town);
        boolean first = route.map(r -> r.trips() == 0).orElse(true);
        double cost = tripCost(hub, town);
        double aid = first ? Dials.CROWN_AID.get() * cost : 0;
        Letter reply = new Letter();
        reply.kind = Kind.REPLY;
        reply.from = hub;
        reply.to = town;
        if (letter.value * Dials.CALL_MARGIN.get() < Dials.CALL_STOP.get() * (cost - aid)) {
            post(level, stock, reply, now); // declined
            dev.b28.enterprise.Enterprise.LOGGER.info("{} declines {}'s call: {} emeralds of goods don't pay its trip",
                    dev.b28.enterprise.settlement.Names.label(hub), dev.b28.enterprise.settlement.Names.label(town),
                    Math.round(letter.value));
            return;
        }
        // The day: once the town has heard (in time to have its goods ready), and one with a wagon free; it never books
        // more calls a day than {@code calls_per_day}.
        long[] booked = {arrival(hub, town, now) / 24000};
        while (calls.stream().filter(c -> c.hub().equals(hub) && c.day() == booked[0]).count()
                >= Math.max(1, Dials.CALLS_PER_DAY.getInt())) {
            booked[0]++;
        }
        long day = booked[0];
        calls.add(new Call(hub, town, day, letter.value));
        if (route.isEmpty()) {
            ledger.addRoute(hub, town).setOnCall(true);
        }
        if (aid > 0) {
            payAid(level, hub, stock, aid, now);
        }
        reply.day = day;
        post(level, stock, reply, now);
        dev.b28.enterprise.Enterprise.LOGGER.info("{} books a call at {} on day {}: {} emeralds of goods{}",
                dev.b28.enterprise.settlement.Names.label(hub), dev.b28.enterprise.settlement.Names.label(town), day,
                Math.round(letter.value), aid > 0 ? ", the crown paying half the road food" : "");
        setDirty();
    }

    /** The crown pays the hub its aid for a first visit, out of the royal treasury, as far as it holds it. */
    private static void payAid(ServerLevel level, GlobalPos hub, MerchantStock stock, double emeralds, long now) {
        var treasury = Sovereign.get(level.getServer()).treasury();
        if (treasury.isEmpty()) {
            return;
        }
        int spurs = (int) Math.min(treasury.get().getBalance(),
                Math.round(emeralds * Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now)));
        if (spurs > 0) {
            treasury.get().setBalance(treasury.get().getBalance() - spurs);
            Treasury.deposit(level, hub.pos(), stock, spurs);
        }
    }

    /** Is a call booked for the hub's run to the town by today? */
    public boolean due(GlobalPos hub, GlobalPos town, long now) {
        return calls.stream().anyMatch(c -> c.hub().equals(hub) && c.town().equals(town) && c.day() <= now / 24000);
    }

    /** The worth of goods offered on the calls due today for that run: its purse. */
    public double purse(GlobalPos hub, GlobalPos town, long now) {
        return calls.stream().filter(c -> c.hub().equals(hub) && c.town().equals(town) && c.day() <= now / 24000)
                .mapToDouble(Call::value).sum();
    }

    /** The run has left: its calls due are made. */
    public void made(GlobalPos hub, GlobalPos town, long now) {
        if (calls.removeIf(c -> c.hub().equals(hub) && c.town().equals(town) && c.day() <= now / 24000)) {
            setDirty();
        }
    }

    /**
     * Bills the town holds on markets none of its caravans go to (and none of theirs come from), once due: sent by post
     * to their drawee.
     */
    private void mailBills(ServerLevel level, GlobalPos town, MerchantStock stock, long now) {
        MinecraftServer server = level.getServer();
        Bills ledger = Bills.get(server);
        CaravanLedger routes = CaravanLedger.get(server);
        Map<GlobalPos, List<ItemStack>> byDrawee = new LinkedHashMap<>();
        List<ItemStack> bundles = stock.removeLoose(s -> s.is(Cargo.DOCUMENTS.get()) && !Bills.ids(s).isEmpty());
        for (ItemStack bundle : bundles) {
            List<Bills.Bill> in = ledger.in(bundle);
            Optional<GlobalPos> drawee = in.stream().map(Bills.Bill::drawee).findFirst();
            boolean mail = drawee.isPresent() && in.stream().allMatch(b -> b.drawee().equals(drawee.get())
                    && b.state() == Bills.State.OUTSTANDING && b.payee().equals(town))
                    && !drawee.get().equals(town)
                    && !OffMap.get(server).is(drawee.get())
                    && !dev.b28.enterprise.trade.Realms.get(server).isForeign(drawee.get())
                    && routes.route(town, drawee.get()).isEmpty() && routes.route(drawee.get(), town).isEmpty()
                    && in.stream().anyMatch(b -> b.due() <= now); // due: till then a bank may still buy it
            if (mail) {
                byDrawee.computeIfAbsent(drawee.get(), d -> new ArrayList<>()).add(bundle);
            } else {
                stock.insert(bundle);
            }
        }
        byDrawee.forEach((drawee, sending) -> {
            Letter letter = new Letter();
            letter.kind = Kind.BILLS;
            letter.from = town;
            letter.to = drawee;
            letter.papers = sending;
            post(level, stock, letter, now);
        });
    }

    /** The drawee pays the bills it's sent as far as it can; the coin, and any bill it can't pay yet, go back by post. */
    private void honour(ServerLevel level, Letter letter, MerchantStock stock, long now) {
        MinecraftServer server = level.getServer();
        Territory territory = Territory.get(server);
        Mint mint = Mint.get(server);
        double rate = mint.rate(Mint.DEFAULT_FACTION, now);
        double cash = Math.max(0, Treasury.available(level, letter.to.pos(), stock, rate) - territory.reserve(letter.to));
        double can = cash + territory.depositsOnCall(letter.to);
        var presented = Bills.get(server).present(letter.papers, can, now, ContractBook.get(server));
        if (presented.paid() > cash) {
            territory.borrowDeposits(level, letter.to, stock, presented.paid() - cash);
        }
        Letter back = new Letter();
        back.kind = Kind.REMIT;
        back.from = letter.to;
        back.to = letter.from;
        if (presented.paid() > 0) {
            Treasury.Withdrawal paid = Treasury.withdraw(level, letter.to.pos(), stock, presented.paid(), mint, now);
            back.value = paid.spurs() + mint.sell(Mint.DEFAULT_FACTION, paid.emeralds(), now);
        }
        back.papers = new ArrayList<>(presented.returned());
        post(level, stock, back, now);
    }

    /** For tests: a letter, as posted now (postage paid by {@code stock}). */
    public void sendForTest(ServerLevel level, MerchantStock stock, Kind kind, GlobalPos from, GlobalPos to, double value,
            long now) {
        Letter letter = new Letter();
        letter.kind = kind;
        letter.from = from;
        letter.to = to;
        letter.value = value;
        post(level, stock, letter, now);
    }

    /** For tests: every letter on the road arrives now. */
    public void deliverNowForTest(long now) {
        letters.forEach(l -> l.arrives = Math.min(l.arrives, now));
    }

    /** Letters on the road to {@code to} of this kind. */
    public long onTheRoad(GlobalPos to, Kind kind) {
        return letters.stream().filter(l -> l.to.equals(to) && l.kind == kind).count();
    }

    /** The calls booked at a hub. */
    public List<Call> calls(GlobalPos hub) {
        return calls.stream().filter(c -> c.hub().equals(hub)).toList();
    }

    /** The post's report: letters on the road, calls booked. */
    public List<String> report(MinecraftServer server) {
        List<String> lines = new ArrayList<>();
        lines.add("The crown's post: %d letters sent, %d on the road; %d calls booked".formatted(sent, letters.size(),
                calls.size()));
        for (Letter l : letters) {
            lines.add("  %s from %s to %s, arrives day %d%s".formatted(l.kind.name().toLowerCase(),
                    dev.b28.enterprise.settlement.Names.label(l.from), dev.b28.enterprise.settlement.Names.label(l.to),
                    l.arrives / 24000, l.kind == Kind.CALL ? ": " + Math.round(l.value) + " emeralds of goods" : ""));
        }
        for (Call c : calls) {
            lines.add("  call booked: %s at %s on day %d, %d emeralds of goods".formatted(
                    dev.b28.enterprise.settlement.Names.label(c.hub()), dev.b28.enterprise.settlement.Names.label(c.town()),
                    c.day(), Math.round(c.value())));
        }
        return lines;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        for (Letter l : letters) {
            CompoundTag e = new CompoundTag();
            e.putString("kind", l.kind.name());
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, l.from).ifSuccess(t -> e.put("from", t));
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, l.to).ifSuccess(t -> e.put("to", t));
            e.putLong("posted", l.posted);
            e.putLong("arrives", l.arrives);
            e.putDouble("value", l.value);
            e.putLong("day", l.day);
            ListTag papers = new ListTag();
            l.papers.forEach(p -> papers.add(p.save(registries)));
            e.put("papers", papers);
            list.add(e);
        }
        tag.put("letters", list);
        ListTag callList = new ListTag();
        for (Call c : calls) {
            CompoundTag e = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, c.hub()).ifSuccess(t -> e.put("hub", t));
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, c.town()).ifSuccess(t -> e.put("town", t));
            e.putLong("day", c.day());
            e.putDouble("value", c.value());
            callList.add(e);
        }
        tag.put("calls", callList);
        tag.putLong("sent", sent);
        return tag;
    }

    public static Post load(CompoundTag tag, HolderLookup.Provider registries) {
        Post out = new Post();
        for (Tag t : tag.getList("letters", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            Letter l = new Letter();
            l.kind = Kind.valueOf(e.getString("kind"));
            l.from = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("from")).result().orElse(null);
            l.to = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("to")).result().orElse(null);
            l.posted = e.getLong("posted");
            l.arrives = e.getLong("arrives");
            l.value = e.getDouble("value");
            l.day = e.getLong("day");
            for (Tag p : e.getList("papers", Tag.TAG_COMPOUND)) {
                ItemStack.parse(registries, p).ifPresent(l.papers::add);
            }
            if (l.from != null && l.to != null) {
                out.letters.add(l);
            }
        }
        for (Tag t : tag.getList("calls", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            var hub = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("hub")).result();
            var town = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("town")).result();
            if (hub.isPresent() && town.isPresent()) {
                out.calls.add(new Call(hub.get(), town.get(), e.getLong("day"), e.getDouble("value")));
            }
        }
        out.sent = tag.getLong("sent");
        return out;
    }
}
