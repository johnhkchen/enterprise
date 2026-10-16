package dev.b28.enterprise.money;

import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Each market's cash book: every emerald into and out of its treasury, by what for and by day, so a town that goes
 * broke can be read off its book. Money in and out goes through {@link Treasury} (labelled by what called it) and its
 * Merchant's lockers (emeralds paid villagers for goods, and taken from them for crops). Once a day the treasury's
 * level is noted too: what the book doesn't account for shows as "unexplained".
 */
public final class CashBook extends SavedData {
    private static final String NAME = "enterprise_cashbook";
    /** Days of entries kept. */
    public static final int KEPT_DAYS = 60;

    /** A market's book: emeralds by day and by what for (+ in, - out), and its treasury at the first touch of a day. */
    static final class Book {
        final TreeMap<Long, Map<String, Double>> days = new TreeMap<>();
        final TreeMap<Long, Double> level = new TreeMap<>();
        final TreeMap<Long, Map<String, Double>> held = new TreeMap<>(); // what the accountant held back (not kept on save)
    }

    private final Map<GlobalPos, Book> books = new HashMap<>();

    public static CashBook get(MinecraftServer server) {
        return server.overworld().getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(CashBook::new, CashBook::load, null), NAME);
    }

    /** {@code emeralds} in (positive) or out (negative) of the market at {@code desk}, for {@code what}. */
    public static void note(GlobalPos desk, String what, double emeralds) {
        MinecraftServer server = net.neoforged.neoforge.server.ServerLifecycleHooks.getCurrentServer();
        if (server == null || desk == null || Math.abs(emeralds) < 1e-9) {
            return;
        }
        CashBook cash = get(server);
        long day = server.overworld().getGameTime() / 24000;
        Book book = cash.books.computeIfAbsent(desk, d -> new Book());
        book.days.computeIfAbsent(day, d -> new LinkedHashMap<>()).merge(what, emeralds, Double::sum);
        while (!book.days.isEmpty() && book.days.firstKey() < day - KEPT_DAYS) {
            book.days.pollFirstEntry();
        }
        cash.setDirty();
    }

    /** The accountant held back {@code emeralds} the town would have spent on {@code what}: noted, not money moved. */
    public static void held(GlobalPos desk, String what, double emeralds) {
        MinecraftServer server = net.neoforged.neoforge.server.ServerLifecycleHooks.getCurrentServer();
        if (server == null || desk == null || emeralds < 1e-9) {
            return;
        }
        CashBook cash = get(server);
        long day = server.overworld().getGameTime() / 24000;
        cash.books.computeIfAbsent(desk, d -> new Book()).held.computeIfAbsent(day, d -> new LinkedHashMap<>())
                .merge(what, emeralds, Double::sum);
        cash.setDirty();
    }

    /** The treasury at the market's first touch of the day: what the book is reconciled against. */
    public void level(GlobalPos desk, long now, double emeralds) {
        Book book = books.computeIfAbsent(desk, d -> new Book());
        long day = now / 24000;
        if (book.level.putIfAbsent(day, emeralds) == null) {
            while (book.level.firstKey() < day - KEPT_DAYS) {
                book.level.pollFirstEntry();
            }
            setDirty();
        }
    }

    /** What called into the treasury, in a word: the first caller outside the money and storage plumbing. */
    static String caller() {
        return StackWalker.getInstance().walk(frames -> frames
                .filter(f -> !f.getClassName().startsWith("dev.b28.enterprise.money.")
                        && !f.getClassName().endsWith("MerchantStock"))
                .findFirst()
                .map(f -> f.getClassName().replaceAll(".*\\.", "").replaceAll("\\$.*", "") + "." + f.getMethodName())
                .orElse("?"));
    }

    /** Totals by what for, from day {@code from} to {@code to} (inclusive), largest first by size. */
    public Map<String, Double> totals(GlobalPos desk, long from, long to) {
        Map<String, Double> out = new HashMap<>();
        Book book = books.get(desk);
        if (book != null) {
            book.days.subMap(from, true, to, true).values().forEach(day -> day.forEach((k, v) -> out.merge(k, v, Double::sum)));
        }
        Map<String, Double> sorted = new LinkedHashMap<>();
        out.entrySet().stream().sorted((a, b) -> Double.compare(Math.abs(b.getValue()), Math.abs(a.getValue())))
                .forEach(e -> sorted.put(e.getKey(), e.getValue()));
        return sorted;
    }

    /** The treasury noted on day {@code day}, or the nearest day noted after it. */
    public java.util.Optional<Map.Entry<Long, Double>> levelOn(GlobalPos desk, long day) {
        Book book = books.get(desk);
        return book == null ? java.util.Optional.empty() : java.util.Optional.ofNullable(book.level.ceilingEntry(day));
    }

    /** The book's report for days {@code from} to {@code to}: totals by what for, and what it doesn't explain. */
    public List<String> report(GlobalPos desk, long from, long to) {
        List<String> lines = new java.util.ArrayList<>();
        Map<String, Double> totals = totals(desk, from, to);
        double in = totals.values().stream().filter(v -> v > 0).mapToDouble(Double::doubleValue).sum();
        double out = totals.values().stream().filter(v -> v < 0).mapToDouble(Double::doubleValue).sum();
        lines.add("days %d to %d: %.1f in, %.1f out".formatted(from, to, in, -out));
        // Reconciled between the first and last opening balances noted: what moved, less what the book explains.
        Book book = books.get(desk);
        var start = book == null ? null : book.level.ceilingEntry(from);
        var end = book == null ? null : book.level.floorEntry(to);
        if (start != null && end != null && end.getKey() > start.getKey()) {
            double booked = totals(desk, start.getKey(), end.getKey() - 1).values().stream()
                    .mapToDouble(Double::doubleValue).sum();
            lines.add("treasury %.1f at day %d's opening, %.1f at day %d's; %+.1f of it the book doesn't explain"
                    .formatted(start.getValue(), start.getKey(), end.getValue(), end.getKey(),
                            end.getValue() - start.getValue() - booked));
        }
        totals.forEach((what, v) -> lines.add("  %+.1f %s".formatted(v, what)));
        if (book != null) {
            Map<String, Double> held = new java.util.TreeMap<>();
            book.held.subMap(from, true, to, true).values().forEach(d -> d.forEach((k, v) -> held.merge(k, v, Double::sum)));
            held.forEach((what, v) -> lines.add("  held back by the accountant: %.1f for %s".formatted(v, what)));
        }
        return lines;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        books.forEach((desk, book) -> {
            CompoundTag e = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> e.put("desk", t));
            CompoundTag days = new CompoundTag();
            book.days.forEach((day, entries) -> {
                CompoundTag d = new CompoundTag();
                entries.forEach(d::putDouble);
                days.put(Long.toString(day), d);
            });
            e.put("days", days);
            CompoundTag level = new CompoundTag();
            book.level.forEach((day, v) -> level.putDouble(Long.toString(day), v));
            e.put("level", level);
            list.add(e);
        });
        tag.put("books", list);
        return tag;
    }

    public static CashBook load(CompoundTag tag, HolderLookup.Provider registries) {
        CashBook out = new CashBook();
        for (Tag t : tag.getList("books", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("desk")).result().ifPresent(desk -> {
                Book book = new Book();
                CompoundTag days = e.getCompound("days");
                for (String day : days.getAllKeys()) {
                    CompoundTag d = days.getCompound(day);
                    Map<String, Double> entries = new LinkedHashMap<>();
                    d.getAllKeys().forEach(k -> entries.put(k, d.getDouble(k)));
                    book.days.put(Long.parseLong(day), entries);
                }
                CompoundTag level = e.getCompound("level");
                level.getAllKeys().forEach(k -> book.level.put(Long.parseLong(k), level.getDouble(k)));
                out.books.put(desk, book);
            });
        }
        return out;
    }
}
