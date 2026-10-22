package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.binding.ModPack;
import dev.b28.enterprise.cargo.Cargo;
import dev.b28.enterprise.cargo.Crates;
import dev.b28.enterprise.cargo.PackageTag;
import dev.b28.enterprise.cargo.Papers;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Count;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.ManifestLine;
import dev.b28.enterprise.wire.Output;
import dev.b28.enterprise.wire.PackageLine;
import dev.b28.enterprise.wire.PalletView;
import dev.b28.enterprise.wire.Place;
import dev.b28.enterprise.wire.Point;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.RouteLine;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.component.DataComponents;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.component.ItemContainerContents;
import net.neoforged.neoforge.event.entity.EntityLeaveLevelEvent;
import net.neoforged.neoforge.server.ServerLifecycleHooks;

/**
 * The core's pallets in the world (docs/pallets.md section 4, docs/binding-plan.md task {@code pallet-views}): the core
 * implements pallets, the mod draws them (John, 2026-10-18), with no pallet rules here. A pallet stays invisible: in the
 * world it is its packages, one item each (a crate, or the item of its kind of packing from the content pack: {@code
 * Cargo.packageItem}), holding its goods for show and tagged with the pallet ({@link PackageTag}: "1 of 3, to Wheatfell,
 * contract 7"), the first carrying the bill of lading in its documents slot. They are in the world only where the pallet
 * stands in a town's warehouse: its desk's chests. In Java mode the Java rules own the chests and nothing is drawn.
 *
 * <p>At each look, before the chests are counted ({@link #happenings}; what it comes to is said after the look's time
 * has passed, so a pallet a player brings in parts has until the next look for the rest):
 * <ul>
 * <li><b>Losses first.</b> A package item destroyed since (burnt, blown up, fallen out of the world, left to despawn:
 * {@link #onLeave}) is told the core as {@code PackageLost} if it was of its pallet as the pallet is, applied at once;
 * the core books the loss (used, reason lost) and breaks the seal, and the pallet goes on short where it is.</li>
 * <li><b>What the desks' chests hold of pallets</b>, asked of the core by pallet: a package of a pallet as it is now
 * (the tick it came to its warehouse is the tag's, and its good and count are among the pallet's packages: {@link
 * #places}) is the pallet's, its tag written again where a package before it was lost; one of a pallet moved or
 * repacked since, or gone, is stale and taken out. A pallet of this desk's town whose packages are here, shown nowhere, is said to
 * be here ({@code Stowed}). A pallet of another town's warehouse whose packages a player carried here is brought: all of
 * them, {@code PalletBrought} (made out to this town, received: its crates taken in; made out elsewhere, it waits here,
 * and is said to be here); some of a pallet made out to this town, {@code PackageBrought} for each, and the core receives
 * it short at its next look if the rest don't come (the seal broken, the rest a loss). The core judges; nothing here
 * decides what's received or lost.</li>
 * <li><b>What goes in</b>, asked of the core with each desk's empty slots ({@code Query.Stow}): a chest takes a pallet
 * whole or not at all, and the core says which; each is put in, a slot an item, and told ({@code Stowed}).</li>
 * </ul>
 * The core's {@code PalletOpened} and {@code PalletMoved} (to a wagon, a hub's custody, a consolidation) take a pallet's
 * packages out of every loaded desk's chests at once ({@link #show}); a {@code PalletPacked} or a {@code PalletMoved} to
 * a town's warehouse is drawn at the next look, when its desk's room is counted.
 */
public final class PalletAdapter {
    private final CoreHost host;
    /** Whether the core's pallets are drawn in the chests (core mode, a test's host). */
    private final boolean writes;
    /** Package items destroyed since the last look, for the next one. */
    private final List<PackageTag> destroyed = new ArrayList<>();

    public PalletAdapter(CoreHost host, boolean writes) {
        this.host = host;
        this.writes = writes;
    }

    /** A loaded desk of the core's and its chests. */
    private record Desk(GlobalPos at, long town, Point point, StockChests chests) {}

    /** The core's desks whose chests are loaded, with their chests, in the points' order. */
    private List<Desk> desks(MinecraftServer server) {
        List<Desk> out = new ArrayList<>();
        if (server == null) {
            return out;
        }
        for (Map.Entry<GlobalPos, Long> e : host.stock().desks().entrySet()) {
            ServerLevel level = server.getLevel(e.getKey().dimension());
            if (level != null && StockAdapter.loaded(level, e.getKey().pos())) {
                out.add(new Desk(e.getKey(), e.getValue(), StockAdapter.point(e.getKey()),
                        StockChests.at(level, PointsAdapter.Kind.DESK, e.getKey().pos())));
            }
        }
        return out;
    }

    /**
     * A look's pallet happenings: losses, then what the loaded desks' chests hold of pallets, then the pallets the core
     * offers each desk put in its chests (as the class says). Nothing in Java mode.
     */
    public List<Input> happenings(MinecraftServer server) {
        List<Input> inputs = new ArrayList<>();
        if (!writes) {
            destroyed.clear();
            return inputs;
        }
        // Applied now, before the chests are read: the crates found are matched to the pallets as they are after it.
        List<Input> lost = losses();
        if (!lost.isEmpty()) {
            host.apply(lost);
        }
        List<Desk> desks = desks(server);
        if (desks.isEmpty()) {
            return inputs;
        }
        Map<Desk, List<StockChests.Packed>> found = new LinkedHashMap<>();
        Set<Long> ids = new TreeSet<>();
        for (Desk d : desks) {
            List<StockChests.Packed> here = d.chests().packages();
            found.put(d, here);
            here.forEach(p -> ids.add(p.tag().pallet()));
        }
        Map<Long, Optional<PalletView>> pallets = pallets(ids);
        for (Desk d : desks) {
            Map<Long, List<StockChests.Packed>> byPallet = new TreeMap<>();
            for (StockChests.Packed p : found.get(d)) {
                byPallet.computeIfAbsent(p.tag().pallet(), k -> new ArrayList<>()).add(p);
            }
            byPallet.forEach((id, packed) -> inputs.addAll(found(d, pallets.getOrDefault(id, Optional.empty()), packed)));
        }
        List<Query> stow = desks.stream().<Query>map(d -> new Query.Stow(d.point(), d.chests().free())).toList();
        List<Answer> answers = host.ask(stow);
        for (int i = 0; i < desks.size(); i++) {
            Desk d = desks.get(i);
            if (answers.get(i) instanceof Answer.Stow s) {
                for (PalletView v : s.pallets()) {
                    if (d.chests().putWhole(items(v))) {
                        inputs.add(new Input.Stowed(d.point(), v.id()));
                    }
                }
            }
        }
        return inputs;
    }

    /**
     * The places on a pallet as it is now ({@code v}) of package items tagged as {@code tags}, by item, in their order:
     * an item of the pallet since the tick it came where it is (any tick, {@code anySince}: crates the core shows where
     * they are, carried there when it came), whose good and count are a package's not yet matched (the place its tag
     * names first: a place moves up when a package before it is lost), or -1 for a stale one.
     */
    static int[] places(PalletView v, List<PackageTag> tags, boolean anySince) {
        int[] places = new int[tags.size()];
        boolean[] taken = new boolean[v.packages().size()];
        for (int i = 0; i < tags.size(); i++) {
            PackageTag t = tags.get(i);
            places[i] = -1;
            if (t.since() != v.since() && !anySince) {
                continue;
            }
            if (t.place() < taken.length && !taken[t.place()] && same(v.packages().get(t.place()), t)) {
                places[i] = t.place();
            } else {
                for (int p = 0; p < taken.length; p++) {
                    if (!taken[p] && same(v.packages().get(p), t)) {
                        places[i] = p;
                        break;
                    }
                }
            }
            if (places[i] >= 0) {
                taken[places[i]] = true;
            }
        }
        return places;
    }

    private static boolean same(PackageLine line, PackageTag t) {
        return line.good() == t.good() && line.count() == t.count();
    }

    /** What the packages of one pallet found in a desk's chests come to (as the class says). */
    private List<Input> found(Desk d, Optional<PalletView> pallet, List<StockChests.Packed> packed) {
        List<Input> inputs = new ArrayList<>();
        if (pallet.isEmpty() || !(pallet.get().at() instanceof Place.Town at)) {
            packed.forEach(StockChests.Packed::clear); // opened, lost, or on a wagon or in custody: its crates here are stale
            return inputs;
        }
        PalletView v = pallet.get();
        // A pallet carried here and said to be here came at another tick than its crates were tagged: they're its own.
        boolean shownHere = at.town() == d.town() && v.shown().filter(d.point()::equals).isPresent();
        int[] places = places(v, packed.stream().map(StockChests.Packed::tag).toList(), shownHere);
        List<StockChests.Packed> current = new ArrayList<>();
        Set<Long> here = new TreeSet<>();
        for (int i = 0; i < packed.size(); i++) {
            StockChests.Packed p = packed.get(i);
            if (places[i] < 0) {
                p.clear(); // of the pallet as it was before it moved or was repacked
                continue;
            }
            PackageTag t = p.tag();
            if (t.place() != places[i] || t.of() != v.items() || t.digest() != v.digest() || t.since() != v.since()) {
                p.retag(t.now(places[i], (int) v.items(), v.since(), v.digest())); // its place and pallet as they are now
            }
            current.add(p);
            here.add((long) places[i]);
        }
        if (current.isEmpty()) {
            return inputs;
        }
        if (at.town() == d.town()) {
            if (v.shown().isEmpty()) {
                inputs.add(new Input.Stowed(d.point(), v.id())); // its crates are here: the core's showing of it
            }
            return inputs;
        }
        // Carried here from another town's warehouse by a player.
        boolean ours = v.consignee() == d.town();
        if (here.size() == v.items()) {
            inputs.add(new Input.PalletBrought(d.town(), v.id()));
            if (ours) {
                current.forEach(StockChests.Packed::clear); // received: its goods into the town's stock, shown as goods
            } else {
                inputs.add(new Input.Stowed(d.point(), v.id())); // it waits here, its crates as they are
            }
        } else if (ours) {
            for (long place : here) {
                if (!v.arriving().contains(place)) {
                    inputs.add(new Input.PackageBrought(d.town(), v.id(), place));
                }
            }
        }
        return inputs;
    }

    /** Each pallet asked of the core, by id: as it is now, or gone. */
    private Map<Long, Optional<PalletView>> pallets(Set<Long> ids) {
        Map<Long, Optional<PalletView>> out = new TreeMap<>();
        if (ids.isEmpty()) {
            return out;
        }
        List<Answer> answers = host.ask(ids.stream().<Query>map(Query.Pallet::new).toList());
        for (Answer a : answers) {
            if (a instanceof Answer.Pallet p) {
                out.put(p.id(), p.pallet());
            }
        }
        return out;
    }

    /**
     * The package items destroyed since the last look, as {@code PackageLost} for each that was of its pallet as the
     * pallet is (a stale crate's loss is no loss: its pallet has moved or been repacked since), at its place now, the
     * higher places of a pallet first so the lower ones stay where they are.
     */
    private List<Input> losses() {
        if (destroyed.isEmpty()) {
            return List.of();
        }
        List<PackageTag> tags = List.copyOf(destroyed);
        destroyed.clear();
        Set<Long> ids = new TreeSet<>();
        tags.forEach(t -> ids.add(t.pallet()));
        Map<Long, Optional<PalletView>> pallets = pallets(ids);
        Map<Long, List<PackageTag>> byPallet = new TreeMap<>();
        tags.forEach(t -> byPallet.computeIfAbsent(t.pallet(), k -> new ArrayList<>()).add(t));
        Map<Long, TreeSet<Long>> lost = new TreeMap<>();
        byPallet.forEach((id, of) -> pallets.getOrDefault(id, Optional.empty()).filter(v -> v.at() instanceof Place.Town)
                .ifPresent(v -> {
                    for (int place : places(v, of, false)) {
                        if (place >= 0) {
                            lost.computeIfAbsent(id, k -> new TreeSet<>()).add((long) place);
                        }
                    }
                }));
        List<Input> inputs = new ArrayList<>();
        lost.forEach((id, places) -> places.descendingSet().forEach(place -> inputs.add(new Input.PackageLost(id, place))));
        return inputs;
    }

    /** A package item destroyed in the world (its tag), told the core at the next look. */
    public void destroyed(ItemStack stack) {
        PackageTag tag = stack.get(Cargo.PACKAGE.get());
        if (tag != null && writes) {
            destroyed.add(tag);
        }
    }

    /**
     * An item entity leaving the world: a package of a pallet that was destroyed (burnt, blown up, cactus, the void, left
     * to despawn: removed killed or discarded with its stack still in it) is lost. One picked up, or put in a hopper,
     * has its stack emptied first; one in a chunk unloading or changing dimension is still in the world.
     */
    public static void onLeave(EntityLeaveLevelEvent event) {
        if (!(event.getEntity() instanceof ItemEntity item) || event.getLevel().isClientSide()) {
            return;
        }
        Entity.RemovalReason why = item.getRemovalReason();
        ItemStack stack = item.getItem();
        if ((why == Entity.RemovalReason.KILLED || why == Entity.RemovalReason.DISCARDED) && !stack.isEmpty()
                && Cargo.isPackage(stack)) {
            CoreHost host = CoreHost.of(event.getLevel().getServer());
            if (host != null && host.pausedWhy() == null) {
                host.pallets().destroyed(stack);
            }
        }
    }

    /**
     * The core's pallet outputs: a pallet opened, or moved off a town's warehouse (onto a wagon, into a hub's custody or
     * a consolidation), has its packages taken out of every loaded desk's chests now; a pallet packed or moved into a
     * town's warehouse is drawn in its desk's chests at the next look.
     */
    public void show(Output output) {
        long gone = switch (output) {
            case Output.PalletOpened o -> o.id();
            case Output.PalletMoved o when !(o.to() instanceof Place.Town) -> o.id();
            default -> -1;
        };
        if (gone < 0 || !writes) {
            return;
        }
        for (Desk d : desks(ServerLifecycleHooks.getCurrentServer())) {
            d.chests().packages().stream().filter(p -> p.tag().pallet() == gone).forEach(StockChests.Packed::clear);
        }
    }

    /** The items a pallet is shown as, one a package, in its order: the first with the bill of lading in its documents slot. */
    public List<ItemStack> items(PalletView v) {
        List<ItemStack> items = new ArrayList<>();
        for (int place = 0; place < v.packages().size(); place++) {
            PackageLine line = v.packages().get(place);
            String kind = kindName(line.kind());
            Item good = host.stock().item(line.good());
            List<ItemStack> slots = new ArrayList<>();
            long left = (long) Math.floor(line.count());
            int max = Math.max(1, good.getDefaultMaxStackSize());
            while (left > 0 && good != Items.AIR) {
                int n = (int) Math.min(left, max);
                slots.add(new ItemStack(good, n));
                left -= n;
            }
            while (slots.size() < Crates.GOODS) {
                slots.add(ItemStack.EMPTY);
            }
            slots.add(place == 0 ? Crates.documents(Papers.NONE.with(bill(v))) : ItemStack.EMPTY);
            ItemStack item = new ItemStack(Cargo.packageItem(kind));
            item.set(DataComponents.CONTAINER, ItemContainerContents.fromItems(slots));
            item.set(DataComponents.MAX_STACK_SIZE, 1);
            item.set(Cargo.PACKAGE.get(), new PackageTag(v.id(), place, v.packages().size(), kind, v.to(),
                    v.contract().map(Long::intValue).orElse(-1), v.since(), v.digest(), line.good(), line.count()));
            items.add(item);
        }
        return items;
    }

    /** The name of the kind of packing at {@code kind} in the content's list (the mod's table; the crate if past it). */
    private static String kindName(long kind) {
        return kind >= 0 && kind < ModPack.PACKAGES.size() ? ModPack.PACKAGES.get((int) kind).name() : "crate";
    }

    /** A pallet's bill of lading as a paper, from the core's view of it: the goods, who ships them to whom, under what. */
    private Papers.Document bill(PalletView v) {
        CompoundTag tag = new CompoundTag();
        tag.putLong("pallet", v.id());
        tag.putInt("contract", v.contract().map(Long::intValue).orElse(-1));
        deskOf(v.shipper()).ifPresent(p -> Papers.put(tag, "shipper", p));
        deskOf(v.consignee()).ifPresent(p -> Papers.put(tag, "consignee", p));
        tag.putString("goods", goods(v.packages()));
        tag.putString("terms", v.paid() ? "paid, from " + v.from() + " to " + v.to() : "from " + v.from() + " to " + v.to());
        tag.putInt("route", -1);
        tag.putLong("issued", v.issued());
        tag.putBoolean("settled", v.paid());
        return new Papers.Document(Papers.BILL_OF_LADING, tag);
    }

    private Optional<GlobalPos> deskOf(long town) {
        return host.stock().desks().entrySet().stream().filter(e -> e.getValue() == town).map(Map.Entry::getKey).findFirst();
    }

    /** "1200 paper" for a pallet's packages, a good at a time in the order they come. */
    private String goods(List<PackageLine> packages) {
        Map<Long, Double> byGood = new LinkedHashMap<>();
        packages.forEach(p -> byGood.merge(p.good(), p.count(), Double::sum));
        List<String> parts = new ArrayList<>();
        byGood.forEach((good, n) -> parts.add("%s %s".formatted(trim(n), name(good))));
        return String.join(", ", parts);
    }

    private String name(long good) {
        Item item = host.stock().item(good);
        return item == Items.AIR ? "good " + good : net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(item).getPath();
    }

    private static String trim(double n) {
        return n == Math.rint(n) ? Long.toString((long) n) : "%.2f".formatted(n);
    }

    /**
     * A player has the Merchant at the desk of {@code town} pack {@code goods} (item counts) from its stock for {@code
     * consignee} ({@code Input.Pack}): pallets in its warehouse, drawn in its desk's chests at the next look. The pallets
     * made, by id.
     */
    public List<Long> pack(long town, long consignee, Map<Item, Long> goods) {
        List<Count> counts = new ArrayList<>();
        goods.forEach((item, n) -> {
            Long good = host.stock().number(item);
            if (good != null) {
                counts.add(new Count(good, n.doubleValue()));
            }
        });
        Response r = host.apply(List.of(new Input.Pack(town, consignee, counts)));
        if (!(r instanceof Response.Done done)) {
            throw new IllegalStateException("the core wouldn't pack: " + (r instanceof Response.Refused f ? f.why() : r));
        }
        return done.outputs().stream().filter(o -> o instanceof Output.PalletPacked).map(o -> ((Output.PalletPacked) o).id()).toList();
    }

    /** A town's pallets as the core has them: held in its warehouse, made out to it elsewhere, and the registry's tallies. */
    public Answer.Pallets town(long town) {
        if (host.ask(List.of(new Query.Pallets(town))).get(0) instanceof Answer.Pallets p) {
            return p;
        }
        throw new IllegalStateException("the core answered Pallets with something else");
    }

    /** A pallet as the core has it now, or empty: gone. */
    public Optional<PalletView> pallet(long id) {
        return pallets(Set.of(id)).getOrDefault(id, Optional.empty());
    }

    /** A route's wagon's manifest. */
    public Answer.Manifest manifest(long route) {
        if (host.ask(List.of(new Query.Manifest(route))).get(0) instanceof Answer.Manifest m) {
            return m;
        }
        throw new IllegalStateException("the core answered Manifest with something else");
    }

    /** A pallet in a line: its id, bill, packages, seal and where it is shown. */
    public String describe(PalletView v) {
        String where = switch (v.at()) {
            case Place.Town t -> "in " + townName(v, t.town()) + "'s warehouse";
            case Place.Custody c -> "in " + townName(v, c.town()) + "'s custody";
            case Place.Wagon w -> "on route " + w.route() + "'s wagon";
            case Place.Container c -> "in consolidation " + c.container();
        };
        return "pallet %d: %s, %s to %s%s, %d package%s, weight %.2f, %s, %s%s".formatted(v.id(), goods(v.packages()), v.from(),
                v.to(), v.contract().map(c -> " under contract " + c).orElse(""), v.items(), v.items() == 1 ? "" : "s", v.weight(),
                where, v.sealed() ? (v.intact() ? "sealed" : "seal broken") : "open",
                v.shown().map(p -> ", its crates at " + p.x() + " " + p.y() + " " + p.z()).orElse(""));
    }

    /** A town by the name a pallet's bill gives it, or its number. */
    private static String townName(PalletView v, long town) {
        return town == v.shipper() ? v.from() : town == v.consignee() ? v.to() : "town " + town;
    }

    /**
     * What {@code /enterprise core pallets <desk>} prints: the town's pallets held and inbound, its wagons' manifests
     * (with whether each needs a superbill), and the goods lost off pallets in the world.
     */
    public List<String> lines(long town) {
        Answer.Pallets p = town(town);
        List<String> lines = new ArrayList<>();
        lines.add("Town %d: %d pallet%s held, %d inbound; %d received and %d lost whole in the world".formatted(town,
                p.held().size(), p.held().size() == 1 ? "" : "s", p.inbound().size(), p.received(), p.lost()));
        p.held().forEach(v -> lines.add("  held " + describe(v)));
        p.inbound().forEach(v -> lines.add("  inbound " + describe(v)));
        for (RouteLine r : host.wagons().town(town).routes()) {
            if (r.from() != town) {
                continue;
            }
            Answer.Manifest m = manifest(r.route());
            if (!m.lines().isEmpty()) {
                lines.add("  route %d's wagon: %d line%s%s".formatted(r.route(), m.lines().size(), m.lines().size() == 1 ? "" : "s",
                        m.superbill() ? ", under a superbill" : ""));
                for (ManifestLine l : m.lines()) {
                    lines.add("    " + describe(l.pallet()) + l.drop().map(t -> ", drops at town " + t).orElse("")
                            + l.container().map(c -> ", in consolidation " + c).orElse(""));
                }
            }
        }
        if (!p.lostGoods().isEmpty()) {
            List<String> lost = new ArrayList<>();
            p.lostGoods().forEach(c -> lost.add(trim(c.count()) + " " + name(c.good())));
            lines.add("  lost off pallets in the world: " + String.join(", ", lost));
        }
        return lines;
    }
}
