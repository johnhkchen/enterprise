package dev.b28.enterprise.contract;

import java.util.ArrayList;
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
import net.minecraft.nbt.StringTag;
import net.minecraft.nbt.Tag;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Rents: what medieval towns and abbeys bought with their savings (a census, a rente). The holder pays the farm town
 * a capital once; the farm owes so much of a crop every season, for good, until it buys the rent back at the same
 * capital. Each season the rent issues a coupon: a forward contract for that season's crop (and any arrears), already
 * paid for, delivered within its harvest window (see {@link Harvest}). A thin season's shortfall rides on the next
 * coupon; arrears past {@code rent_arrears_seasons} seasons are a default. Paid in kind, a rent keeps its worth when the
 * coin is debased.
 */
public final class Rents extends SavedData {
    private static final String NAME = "enterprise_rents";

    /** A season: a coupon falls due each (a dial, see {@link dev.b28.enterprise.Dials}). */
    public static long seasonTicks() {
        return dev.b28.enterprise.Dials.SEASON_DAYS.ticks();
    }

    public enum State { ACTIVE, REDEEMED, DEFAULTED }

    /** A rent: the farm owes the holder {@code quantity} of {@code crop} a season, bought for {@code capital}. */
    public static final class Rent {
        final int id;
        final GlobalPos farm;
        GlobalPos holder; // whoever holds it now: it can be sold on
        final Item crop;
        final int quantity;
        final double capital;
        final long bought;
        long nextCoupon;
        int coupon = -1; // the coupon in force
        int coupons; // issued, in all
        int arrears; // of the crop, owed from past seasons
        long delivered; // of the crop, in all
        State state = State.ACTIVE;
        boolean exWorks; // its holder's caravans collect the coupons at the farm (a dealer's), not the farm delivering
        boolean bearer; // its deed is a player's: the holder (a dealer) collects the coupons in its keeping
        int accrued; // of the crop, collected for the bearer and not yet handed over
        long shop = Long.MIN_VALUE; // the dealer's vendor quoting it (packed position), if any
        GlobalPos previous; // who held it before the last sale
        double paid; // what its holder paid for it
        long traded = Long.MIN_VALUE / 2; // when it last changed hands
        final List<String> history = new ArrayList<>();

        Rent(int id, GlobalPos farm, GlobalPos holder, Item crop, int quantity, double capital, long bought) {
            this.id = id;
            this.farm = farm;
            this.holder = holder;
            this.crop = crop;
            this.quantity = quantity;
            this.capital = capital;
            this.bought = bought;
            this.nextCoupon = bought;
        }

        public int id() { return id; }
        public GlobalPos farm() { return farm; }
        public GlobalPos holder() { return holder; }
        public Item crop() { return crop; }
        public int quantity() { return quantity; }
        public double capital() { return capital; }
        public int coupon() { return coupon; }
        public int coupons() { return coupons; }
        public int arrears() { return arrears; }
        public long delivered() { return delivered; }
        public State state() { return state; }
        public boolean exWorks() { return exWorks; }
        public boolean bearer() { return bearer; }
        public int accrued() { return accrued; }
        public double paid() { return paid; }
        public long traded() { return traded; }
        public java.util.Optional<GlobalPos> previous() { return java.util.Optional.ofNullable(previous); }
        public java.util.Optional<net.minecraft.core.BlockPos> shop() {
            return shop == Long.MIN_VALUE ? java.util.Optional.empty()
                    : java.util.Optional.of(net.minecraft.core.BlockPos.of(shop));
        }
        public List<String> history() { return List.copyOf(history); }

        void note(String line) {
            history.add(line);
            while (history.size() > 10) {
                history.remove(0);
            }
        }
    }

    private final Map<Integer, Rent> rents = new LinkedHashMap<>();
    private int nextId = 1;

    public static Rents get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Rents::new, Rents::load, null), NAME);
    }

    /** A rent's price: {@code rent_purchase_seasons} seasons' crop at {@code each} emeralds. */
    public static double capital(int quantity, double each) {
        return dev.b28.enterprise.Dials.PURCHASE_SEASONS.get() * quantity * each;
    }

    /** Sold: the holder has paid the capital; the first coupon is issued at once. */
    public Rent sell(GlobalPos farm, GlobalPos holder, Item crop, int quantity, double capital, ContractBook book,
            long now) {
        Rent rent = new Rent(nextId++, farm, holder, crop, quantity, capital, now);
        rent.paid = capital;
        rent.traded = now;
        rent.note("day %d: sold, %d %s a season for %.1f emeralds".formatted(now / 24000, quantity,
                BuiltInRegistries.ITEM.getKey(crop).getPath(), capital));
        rents.put(rent.id, rent);
        issue(rent, book, now);
        setDirty();
        return rent;
    }

    /**
     * Each season: the last coupon's shortfall goes into the arrears (its contract closed), and a new coupon is
     * issued for the season's crop and the arrears. Arrears past {@code rent_arrears_seasons} seasons: a default.
     */
    public void review(ContractBook book, long now) {
        for (Rent rent : rents.values()) {
            if (rent.state != State.ACTIVE) {
                // Bought back or defaulted: no coupon stands (one saved before redeeming closed its coupon, closes now).
                book.contract(rent.coupon).filter(Contract::inForce).ifPresent(c -> book.close(c, "the rent is over", now));
                continue;
            }
            if (now < rent.nextCoupon) {
                continue;
            }
            issue(rent, book, now);
            setDirty();
        }
    }

    private void issue(Rent rent, ContractBook book, long now) {
        int owed = 0;
        Optional<Contract> last = rent.coupon < 0 ? Optional.empty() : book.contract(rent.coupon);
        if (last.isPresent()) {
            Contract c = last.get();
            int due = c.delivery().map(Delivery::quantity).orElse(0);
            rent.delivered += c.delivered();
            if (rent.bearer) {
                rent.accrued += c.delivered(); // in the dealer's keeping, for whoever brings the deed
            }
            // What the holder's caravan has collected and carries home is the holder's already.
            owed = Math.max(0, due - c.delivered() - (c.incoterm().sellerBearsRoad ? 0 : c.shipped()));
            book.close(c, owed > 0 ? owed + " short, on the next coupon" : "delivered", now);
        }
        rent.arrears = owed;
        if (rent.arrears > dev.b28.enterprise.Dials.RENT_ARREARS.get() * rent.quantity) {
            rent.state = State.DEFAULTED;
            book.missed(rent.farm, rent.holder, 3, now);
            rent.note("day %d: defaulted, %d in arrears".formatted(now / 24000, rent.arrears));
            return;
        }
        // The coupon: this season's crop and the arrears, already paid for, due at the season's end, both parties
        // holding the rent's deed (so it's in force at once).
        Contract coupon = book.draft(rent.farm, rent.holder, List.of(
                        new Delivery(rent.crop, rent.quantity + rent.arrears, 0),
                        new Harvest(dev.b28.enterprise.Dials.HARVEST_WINDOW.getInt(), true)),
                rent.exWorks ? Incoterm.EXW : Incoterm.DDP, now + seasonTicks(), seasonTicks(), 1, now, rent.farm, true);
        rent.coupon = coupon.id();
        rent.coupons++;
        rent.nextCoupon = now + seasonTicks() + dev.b28.enterprise.Dials.HARVEST_WINDOW.ticks();
        rent.note("day %d: coupon %d, %d due by day %d".formatted(now / 24000, rent.coupons,
                rent.quantity + rent.arrears, (now + seasonTicks()) / 24000));
    }

    /**
     * Sold on: the rent is the new holder's from now (the farm's deed endorsed over). The coupon in force closes, what
     * it lacked carried on, and a new season's coupon is made out to the new holder; {@code exWorks}, its caravans
     * collect at the farm.
     */
    public void assign(Rent rent, GlobalPos holder, boolean exWorks, double price, ContractBook book, long now) {
        GlobalPos was = rent.holder;
        rent.previous = was;
        rent.holder = holder;
        rent.exWorks = exWorks;
        rent.paid = price;
        rent.traded = now;
        rent.note("day %d: sold on by %s for %.1f emeralds".formatted(now / 24000,
                dev.b28.enterprise.settlement.Names.label(was), price));
        // The season's coupon goes with it, endorsed over: the season runs on.
        book.contract(rent.coupon).ifPresent(c -> book.endorse(c, holder, exWorks ? Incoterm.EXW : Incoterm.DDP, now));
        setDirty();
    }

    /** The deed changed hands with a player: {@code bearer}, it's the player's, in the holder's keeping. */
    public void setBearer(Rent rent, boolean bearer, double price, long now) {
        rent.bearer = bearer;
        rent.note("day %d: %s for %.1f emeralds".formatted(now / 24000, bearer ? "its deed sold to a player"
                : "its deed bought back from a player", price));
        setDirty();
    }

    /** Hands over up to {@code count} of what's accrued for the bearer; returns how many. */
    public int takeAccrued(Rent rent, int count) {
        int taken = Math.min(count, rent.accrued);
        rent.accrued -= taken;
        setDirty();
        return taken;
    }

    public void setShop(Rent rent, java.util.Optional<net.minecraft.core.BlockPos> shop) {
        rent.shop = shop.map(net.minecraft.core.BlockPos::asLong).orElse(Long.MIN_VALUE);
        setDirty();
    }

    // hub -> the bank account its rent shops pay players from
    private final Map<GlobalPos, java.util.UUID> dealerAccounts = new LinkedHashMap<>();

    /** The bank account a dealer's buying shops pay from (made the first time it's wanted). */
    public java.util.UUID dealerAccount(GlobalPos hub) {
        java.util.UUID id = dealerAccounts.computeIfAbsent(hub, h -> java.util.UUID.randomUUID());
        setDirty();
        return id;
    }

    /** Rents in force this desk holds. */
    public List<Rent> heldBy(GlobalPos holder) {
        return rents.values().stream().filter(r -> r.state == State.ACTIVE && r.holder.equals(holder)).toList();
    }

    /** Bought back: the farm has paid the holder its capital, and owes nothing more: its coupon closes. */
    public void redeem(Rent rent, ContractBook book, long now) {
        book.contract(rent.coupon).ifPresent(c -> book.close(c, "the rent bought back", now));
        rent.state = State.REDEEMED;
        rent.note("day %d: bought back for %.1f emeralds".formatted(now / 24000, rent.capital));
        setDirty();
    }

    /** Rents in force a farm owes this holder. */
    public List<Rent> owed(GlobalPos farm, GlobalPos holder) {
        return rents.values().stream().filter(r -> r.state == State.ACTIVE && r.farm.equals(farm)
                && r.holder.equals(holder)).toList();
    }

    public List<Rent> all() {
        return List.copyOf(rents.values());
    }

    public Optional<Rent> rent(int id) {
        return Optional.ofNullable(rents.get(id));
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        for (Rent r : rents.values()) {
            CompoundTag e = new CompoundTag();
            e.putInt("id", r.id);
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, r.farm).ifSuccess(t -> e.put("farm", t));
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, r.holder).ifSuccess(t -> e.put("holder", t));
            e.putString("crop", BuiltInRegistries.ITEM.getKey(r.crop).toString());
            e.putInt("quantity", r.quantity);
            e.putDouble("capital", r.capital);
            e.putLong("bought", r.bought);
            e.putLong("nextCoupon", r.nextCoupon);
            e.putInt("coupon", r.coupon);
            e.putInt("coupons", r.coupons);
            e.putInt("arrears", r.arrears);
            e.putLong("delivered", r.delivered);
            e.putString("state", r.state.name());
            e.putBoolean("exWorks", r.exWorks);
            e.putBoolean("bearer", r.bearer);
            e.putInt("accrued", r.accrued);
            e.putLong("shop", r.shop);
            e.putDouble("paid", r.paid);
            e.putLong("traded", r.traded);
            if (r.previous != null) {
                GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, r.previous).ifSuccess(t -> e.put("previous", t));
            }
            ListTag lines = new ListTag();
            r.history.forEach(line -> lines.add(StringTag.valueOf(line)));
            e.put("history", lines);
            list.add(e);
        }
        tag.put("rents", list);
        tag.putInt("nextId", nextId);
        ListTag accounts = new ListTag();
        dealerAccounts.forEach((hub, id) -> {
            CompoundTag e = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, hub).ifSuccess(t -> e.put("hub", t));
            e.putUUID("account", id);
            accounts.add(e);
        });
        tag.put("dealerAccounts", accounts);
        return tag;
    }

    public static Rents load(CompoundTag tag, HolderLookup.Provider registries) {
        Rents book = new Rents();
        for (Tag t : tag.getList("rents", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos farm = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("farm")).getOrThrow();
            GlobalPos holder = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("holder")).getOrThrow();
            Rent r = new Rent(e.getInt("id"), farm, holder,
                    BuiltInRegistries.ITEM.get(ResourceLocation.parse(e.getString("crop"))), e.getInt("quantity"),
                    e.getDouble("capital"), e.getLong("bought"));
            r.nextCoupon = e.getLong("nextCoupon");
            r.coupon = e.getInt("coupon");
            r.coupons = e.getInt("coupons");
            r.arrears = e.getInt("arrears");
            r.delivered = e.getLong("delivered");
            r.state = State.valueOf(e.getString("state"));
            r.exWorks = e.getBoolean("exWorks");
            r.bearer = e.getBoolean("bearer");
            r.accrued = e.getInt("accrued");
            r.shop = e.contains("shop") ? e.getLong("shop") : Long.MIN_VALUE;
            r.paid = e.contains("paid") ? e.getDouble("paid") : r.capital;
            r.traded = e.contains("traded") ? e.getLong("traded") : r.bought;
            r.previous = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("previous")).result().orElse(null);
            e.getList("history", Tag.TAG_STRING).forEach(l -> r.history.add(l.getAsString()));
            book.rents.put(r.id, r);
        }
        book.nextId = Math.max(1, tag.getInt("nextId"));
        for (Tag t : tag.getList("dealerAccounts", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("hub")).ifSuccess(hub ->
                    book.dealerAccounts.put(hub, e.getUUID("account")));
        }
        return book;
    }
}
