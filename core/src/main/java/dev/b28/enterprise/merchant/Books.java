package dev.b28.enterprise.merchant;

import dev.b28.enterprise.territory.Buildings;
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
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * A market's books, kept by its Merchant: each day's dealings, good by good, with whom. What it bought and sold, at
 * what price, what its caravans carried out and brought home, what its shop sold. A caravan keeps its own log of what
 * it sold and bought at the far end, and hands it in when it's home: the books know of a far market only what a
 * caravan brought back. A page a day of dealings takes stationery (see {@link dev.b28.enterprise.cargo.Paperwork}).
 *
 * How far back they go is the town's learning: a Merchant alone keeps {@link #MERCHANT_DAYS} days; with a Librarian in
 * the town, {@link #LIBRARIAN_DAYS}; with a library, {@link #LIBRARY_DAYS}. A Merchant values a good at a market by
 * the last price its books show there (see {@link dev.b28.enterprise.caravan.Desires}).
 *
 * And its reasons: why it sent a good or didn't, why the far end wouldn't buy, why its workshop stood idle. These are
 * the Merchant's own reckoning, not dealings: no page, no stationery. They're kept as a tally (how often, how many
 * items, last day) for as long as the books last, so a whole run's reasons show at once.
 */
public final class Books extends SavedData {
    private static final String NAME = "enterprise_books";

    public static final int MERCHANT_DAYS = 10;
    public static final int LIBRARIAN_DAYS = 60;
    public static final int LIBRARY_DAYS = 365;

    /** What kind of dealing: bought and sold here, carried out and brought home, sold in the shop; or a reason. */
    public enum Kind { BOUGHT, SOLD, SENT, HOME, SHOP, WHY }

    /**
     * One line: on {@code day}, {@code count} of {@code item} for {@code spurs}, with {@code with} (none: the town). A
     * reason ({@link Kind#WHY}) says {@code why}.
     */
    public record Entry(long day, Kind kind, GlobalPos with, Item item, int count, int spurs, String why) {
        public Entry(long day, Kind kind, GlobalPos with, Item item, int count, int spurs) {
            this(day, kind, with, item, count, spurs, null);
        }

        public static Entry why(long now, GlobalPos with, Item item, String why, int count) {
            return new Entry(now / 24000, Kind.WHY, with, item, count, 0, why);
        }

        public CompoundTag save() {
            CompoundTag tag = new CompoundTag();
            tag.putLong("day", day);
            tag.putString("kind", kind.name());
            if (with != null) {
                GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, with).ifSuccess(t -> tag.put("with", t));
            }
            tag.putString("item", BuiltInRegistries.ITEM.getKey(item).toString());
            tag.putInt("count", count);
            tag.putInt("spurs", spurs);
            if (why != null) {
                tag.putString("why", why);
            }
            return tag;
        }

        public static Entry load(CompoundTag tag) {
            GlobalPos with = tag.contains("with")
                    ? GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("with")).result().orElse(null) : null;
            return new Entry(tag.getLong("day"), Kind.valueOf(tag.getString("kind")), with,
                    BuiltInRegistries.ITEM.get(ResourceLocation.parse(tag.getString("item"))), tag.getInt("count"),
                    tag.getInt("spurs"), tag.contains("why") ? tag.getString("why") : null);
        }
    }

    private final Map<GlobalPos, List<Entry>> books = new HashMap<>();
    private final Map<GlobalPos, Long> lastPage = new HashMap<>();
    /** Reasons, by market: "with | good | why" -> {times, items, last day}. */
    private final Map<GlobalPos, Map<String, long[]>> whys = new HashMap<>();

    public static Books get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Books::new, Books::load, null), NAME);
    }

    /** Days the market's books go back: its town's learning. */
    public static int keeps(MinecraftServer server, GlobalPos desk) {
        Territory territory = Territory.get(server);
        Optional<GlobalPos> bell = territory.bellOf(desk);
        if (bell.isEmpty()) {
            return OffMap.get(server).is(desk) ? LIBRARIAN_DAYS : MERCHANT_DAYS;
        }
        if (dev.b28.enterprise.territory.Works.get(server).built(bell.get(), Buildings.LIBRARY.id())) {
            return LIBRARY_DAYS;
        }
        boolean librarian = dev.b28.enterprise.settlement.Settlements.get(server).roster(bell.get()).values().stream()
                .anyMatch(r -> r.profession().equals("minecraft:librarian"));
        return librarian ? LIBRARIAN_DAYS : MERCHANT_DAYS;
    }

    /** Enters a line in the market's books, at game time {@code now}. */
    public void enter(MinecraftServer server, GlobalPos desk, long now, Kind kind, GlobalPos with, Item item, int count,
            int spurs) {
        if (count <= 0 && spurs <= 0) {
            return;
        }
        file(server, desk, now, List.of(new Entry(now / 24000, kind, with, item, count, spurs)));
    }

    /** A reason, in the market's tally: why {@code count} of {@code item} went (or didn't) with {@code with}. */
    public void why(GlobalPos desk, long now, GlobalPos with, Item item, String why, int count) {
        tally(desk, Entry.why(now, with, item, why, count), false);
    }

    /** A standing reason, noted each time it's reckoned: the tally keeps its latest count, not the sum. */
    public void whyNow(GlobalPos desk, long now, GlobalPos with, Item item, String why, int count) {
        tally(desk, Entry.why(now, with, item, why, count), true);
    }

    private void tally(GlobalPos desk, Entry e) {
        tally(desk, e, false);
    }

    private void tally(GlobalPos desk, Entry e, boolean latest) {
        String with = e.with() == null ? "the town" : dev.b28.enterprise.settlement.Names.label(e.with());
        long[] t = whys.computeIfAbsent(desk, d -> new HashMap<>()).computeIfAbsent(
                with + " | " + (e.item() == net.minecraft.world.item.Items.AIR ? "-"
                        : BuiltInRegistries.ITEM.getKey(e.item()).getPath()) + " | " + e.why(), k -> new long[3]);
        t[0]++;
        t[1] = latest ? Math.max(0, e.count()) : t[1] + Math.max(0, e.count());
        t[2] = Math.max(t[2], e.day());
        setDirty();
    }

    /** The market's reasons: "with | good | why" -> {times, items, last day}, most often first. */
    public Map<String, long[]> whys(GlobalPos desk) {
        Map<String, long[]> out = new LinkedHashMap<>();
        whys.getOrDefault(desk, Map.of()).entrySet().stream()
                .sorted((a, b) -> Long.compare(b.getValue()[0], a.getValue()[0]))
                .forEach(e -> out.put(e.getKey(), e.getValue().clone()));
        return out;
    }

    /** Files lines in the market's books (a caravan's log, handed in), keeping only as far back as it can. */
    public void file(MinecraftServer server, GlobalPos desk, long now, List<Entry> lines) {
        List<Entry> entries = new ArrayList<>();
        for (Entry e : lines) {
            if (e.kind() == Kind.WHY) {
                tally(desk, e); // its reasons: no page
            } else {
                entries.add(e);
            }
        }
        if (entries.isEmpty()) {
            return;
        }
        long today = now / 24000;
        List<Entry> book = books.computeIfAbsent(desk, d -> new ArrayList<>());
        book.addAll(entries);
        long oldest = today - keeps(server, desk);
        book.removeIf(e -> e.day() < oldest);
        if (lastPage.getOrDefault(desk, -1L) < today) {
            lastPage.put(desk, today);
            dev.b28.enterprise.cargo.Paperwork.drawn(desk, 1); // a page of the day's dealings
        }
        setDirty();
    }

    public List<Entry> of(GlobalPos desk) {
        return List.copyOf(books.getOrDefault(desk, List.of()));
    }

    /** The last price the books show for one of {@code item} sold to (or bought from) {@code market}, in spurs each. */
    public Optional<Double> lastPrice(GlobalPos desk, GlobalPos market, Item item) {
        List<Entry> book = books.getOrDefault(desk, List.of());
        for (int i = book.size() - 1; i >= 0; i--) {
            Entry e = book.get(i);
            if (e.item() == item && market.equals(e.with()) && e.count() > 0 && e.spurs() > 0
                    && (e.kind() == Kind.SOLD || e.kind() == Kind.BOUGHT)) {
                return Optional.of((double) e.spurs() / e.count());
            }
        }
        return Optional.empty();
    }

    /** Totals by dealing, counterparty and good: {kind, with, item} -> {count, spurs}. */
    public Map<String, int[]> summary(MinecraftServer server, GlobalPos desk) {
        Map<String, int[]> out = new LinkedHashMap<>();
        for (Entry e : books.getOrDefault(desk, List.of())) {
            String with = e.with() == null ? "the town" : dev.b28.enterprise.settlement.Names.label(e.with());
            String key = e.kind().name().toLowerCase() + " | " + with + " | " + BuiltInRegistries.ITEM.getKey(e.item()).getPath();
            int[] t = out.computeIfAbsent(key, k -> new int[2]);
            t[0] += e.count();
            t[1] += e.spurs();
        }
        return out;
    }

    /** Counts of a caravan's cargo by good (crates opened): for a line per good. */
    public static Map<Item, Integer> byGood(List<ItemStack> cargo) {
        Map<Item, Integer> out = new LinkedHashMap<>();
        for (ItemStack stack : dev.b28.enterprise.cargo.Crates.contents(cargo)) {
            if (!stack.isEmpty()) {
                out.merge(stack.getItem(), stack.getCount(), Integer::sum);
            }
        }
        return out;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        books.forEach((desk, entries) -> {
            CompoundTag e = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> e.put("desk", t));
            ListTag lines = new ListTag();
            entries.forEach(entry -> lines.add(entry.save()));
            e.put("lines", lines);
            e.putLong("lastPage", lastPage.getOrDefault(desk, -1L));
            list.add(e);
        });
        tag.put("books", list);
        ListTag reasons = new ListTag();
        whys.forEach((desk, tally) -> {
            CompoundTag e = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> e.put("desk", t));
            CompoundTag lines = new CompoundTag();
            tally.forEach((key, t) -> lines.putLongArray(key, t));
            e.put("lines", lines);
            reasons.add(e);
        });
        tag.put("whys", reasons);
        return tag;
    }

    public static Books load(CompoundTag tag, HolderLookup.Provider registries) {
        Books out = new Books();
        for (Tag t : tag.getList("books", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("desk")).result().ifPresent(desk -> {
                List<Entry> lines = new ArrayList<>();
                e.getList("lines", Tag.TAG_COMPOUND).forEach(l -> lines.add(Entry.load((CompoundTag) l)));
                out.books.put(desk, lines);
                out.lastPage.put(desk, e.getLong("lastPage"));
            });
        }
        for (Tag t : tag.getList("whys", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("desk")).result().ifPresent(desk -> {
                Map<String, long[]> tally = new HashMap<>();
                CompoundTag lines = e.getCompound("lines");
                lines.getAllKeys().forEach(key -> tally.put(key, lines.getLongArray(key)));
                out.whys.put(desk, tally);
            });
        }
        return out;
    }
}
