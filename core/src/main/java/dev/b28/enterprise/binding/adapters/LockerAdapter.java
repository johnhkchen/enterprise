package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.merchant.MerchantView;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.LockerView;
import dev.b28.enterprise.wire.OfferView;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Response;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.Direction;
import net.minecraft.core.GlobalPos;
import net.minecraft.network.chat.Component;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

/**
 * Lockers and the Merchant's screen, from the core (docs/binding-plan.md 2, concern 6). A locker is a chest touching a
 * villager's workstation ({@code docs/decisions.md}, "Lockers"); in the core it's the villager's hand's, whose trades the
 * desk's Merchant runs against the town's warehouse at each restock, two a day, loaded or not ({@code lockers.rs}). The
 * adapter keeps no lockers and no switches of its own:
 *
 * <ul>
 *   <li><b>Where each stands</b> ({@link #place}, at each look in core mode): for every loaded villager that is a hand
 *       of a town with a loaded desk, its workstation if a container touches it, sent as {@code Input.Locker} where the
 *       core has it otherwise, with the workstation registered as the town's {@code Locker} point first. A locker point
 *       no hand's locker stands at any more is removed. A villager not loaded is left as the core has it.
 *   <li><b>Its chests</b> are the stock adapter's: counted at each look with the desk's, written the core's moves (what
 *       the town used is taken from them first, and what's left the Merchant carries to the desk's chests).
 *   <li><b>The screen</b> ({@link #view}) shows the lockers the core says the Merchant manages and each one's offers
 *       with their switches ({@code Query.Lockers}); a switch the player flips goes to the core ({@link #flip}, {@code
 *       Input.Switch}), by the offer's place and good, in the key the screen was shown.
 * </ul>
 *
 * <p>So the Java rules' {@code Rounds} (chunks forced loaded for a market's villagers to catch up) and {@code
 * LockerCatchUp} (restocks replayed as a villager loads) have nothing to do in core mode, and the villagers' own
 * restocks are vanilla's ({@code VillagerMixin} runs no locker trades there).
 */
public final class LockerAdapter {
    /** A switch on the screen in core mode is named by this, then the hand, the offer's place and its good. */
    public static final String KEY = "core:";

    private final CoreHost host;
    /** Whether the host's lockers are the world's: core mode, or a test's host. In Java mode the Java rules keep them. */
    private final boolean places;

    public LockerAdapter(CoreHost host, boolean places) {
        this.host = host;
        this.places = places;
    }

    /** A town's lockers as the core has them: its Merchant's limit, whether its host places them, each hand's. */
    public record Town(long town, long limit, boolean placed, List<LockerView> lockers) {
        /** The hand's locker, if it has a trade a locker serves. */
        public Optional<LockerView> of(long hand) {
            return lockers.stream().filter(l -> l.hand() == hand).findFirst();
        }
    }

    /** A town's lockers now. */
    public Town town(long town) {
        if (host.ask(List.of(new Query.Lockers(town))).get(0) instanceof Answer.Lockers a) {
            return new Town(a.town(), a.limit(), a.placed(), a.lockers());
        }
        throw new IllegalStateException("the core answered Lockers with something else");
    }

    /**
     * Where each loaded villager's locker stands, told the core where it has it otherwise (a look's, before its
     * stocktakes): for every town whose desk is loaded, their lockers and hands asked in one question. Nothing in Java
     * mode.
     */
    public void place(MinecraftServer server) {
        if (!places) {
            return;
        }
        Map<Long, ServerLevel> towns = new java.util.LinkedHashMap<>();
        for (var desk : host.points().of(PointsAdapter.Kind.DESK).entrySet()) {
            ServerLevel level = server.getLevel(desk.getKey().dimension());
            if (level != null && level.isLoaded(desk.getKey().pos())) {
                towns.putIfAbsent(desk.getValue(), level);
            }
        }
        if (towns.isEmpty()) {
            return;
        }
        List<Query> queries = new ArrayList<>();
        towns.keySet().forEach(t -> {
            queries.add(new Query.Lockers(t));
            queries.add(new Query.Hands(t));
        });
        List<Answer> answers = host.ask(queries);
        List<Input> inputs = new ArrayList<>();
        Set<GlobalPos> added = new HashSet<>();
        int i = 0;
        for (var e : towns.entrySet()) {
            inputs.addAll(changes(e.getValue(), e.getKey(), (Answer.Lockers) answers.get(i), (Answer.Hands) answers.get(i + 1),
                    added));
            i += 2;
        }
        send(inputs);
    }

    /**
     * Where each of a town's loaded villagers' lockers stands, told the core now (a command, a test): its workstation in
     * {@code level} if a container touches it, else none; a locker point of the town's no hand's locker stands at is
     * removed. Returns the happenings sent (none when the core has every one as it stands).
     */
    public List<Input> placeNow(ServerLevel level, long town) {
        List<Answer> answers = host.ask(List.of(new Query.Lockers(town), new Query.Hands(town)));
        List<Input> inputs = changes(level, town, (Answer.Lockers) answers.get(0), (Answer.Hands) answers.get(1), new HashSet<>());
        send(inputs);
        return inputs;
    }

    private void send(List<Input> inputs) {
        if (!inputs.isEmpty()) {
            Response r = host.apply(inputs);
            host.points().reload();
            if (r instanceof Response.Refused refused) {
                Enterprise.LOGGER.warn("The core wouldn't take the lockers: {}", refused.why());
            }
        }
    }

    /**
     * What the core should be told of a town's lockers, from its answers and the world ({@code added}: the locker points
     * added in this look so far, every town's, so none is added twice).
     */
    private List<Input> changes(ServerLevel level, long town, Answer.Lockers lockers, Answer.Hands hands, Set<GlobalPos> added) {
        Map<Long, UUID> villagers = new HashMap<>();
        for (var h : hands.hands()) {
            if (!h.tag().isEmpty() && !h.away()) {
                try {
                    villagers.put(h.hand(), UUID.fromString(h.tag()));
                } catch (IllegalArgumentException e) {
                    // a tag that isn't a villager's UUID: no villager here
                }
            }
        }
        List<Input> inputs = new ArrayList<>();
        Set<GlobalPos> tied = new HashSet<>();
        for (LockerView l : lockers.lockers()) {
            GlobalPos had = l.at().map(PointsAdapter::place).orElse(null);
            UUID id = villagers.get(l.hand());
            if (id == null || !(level.getEntity(id) instanceof Villager v) || !siteLoaded(level, v)) {
                if (had != null) {
                    tied.add(had); // not loaded, or its workstation isn't: as the core has it
                }
                continue;
            }
            GlobalPos now = workstation(level, v);
            if (now != null) {
                PointsAdapter.Placed p = host.points().at(now);
                if (p != null && (p.kind() != PointsAdapter.Kind.LOCKER || p.town() != town)) {
                    now = null; // another point stands there, or another town's locker: no locker of this town's
                } else if (p == null && !added.add(now)) {
                    now = null; // another villager took it in this look (this town's or another's)
                } else if (p == null) {
                    inputs.add(new Input.AddPoint(PointsAdapter.point(PointsAdapter.Kind.LOCKER, now), town));
                }
            }
            if (now != null && !tied.add(now)) {
                now = null; // two villagers at one workstation (a stale memory): the first has it
            }
            if (!java.util.Objects.equals(had, now)) {
                inputs.add(new Input.Locker(l.hand(), Optional.ofNullable(now).map(at -> PointsAdapter.point(PointsAdapter.Kind.LOCKER, at))));
            }
        }
        for (var e : host.points().of(PointsAdapter.Kind.LOCKER).entrySet()) {
            if (e.getValue() == town && !tied.contains(e.getKey())) {
                inputs.add(new Input.RemovePoint(PointsAdapter.point(PointsAdapter.Kind.LOCKER, e.getKey())));
            }
        }
        return inputs;
    }

    /**
     * What keeps a town's villagers from having lockers, for the player (the ledger's): each loaded villager of a
     * trade with a workstation but no chest touching it ("a chest touching it makes a locker"), and each with no
     * workstation at all. A generated village's workstations have none: a player puts them there.
     */
    public List<String> noLockers(net.minecraft.server.MinecraftServer server, long town) {
        List<String> out = new ArrayList<>();
        if (server == null) {
            return out;
        }
        for (var h : host.hands().roster(town).hands()) {
            if (h.villager() == null || h.merchant() || h.away() || h.baby()) {
                continue;
            }
            for (ServerLevel level : server.getAllLevels()) {
                if (!(level.getEntity(h.villager()) instanceof Villager v)) {
                    continue;
                }
                var profession = v.getVillagerData().getProfession();
                if (profession == VillagerProfession.NONE || profession == VillagerProfession.NITWIT) {
                    break;
                }
                String trade = BuiltInRegistries.VILLAGER_PROFESSION.getKey(profession).getPath();
                var site = v.getBrain().getMemory(MemoryModuleType.JOB_SITE).filter(s -> s.dimension() == level.dimension());
                if (site.isEmpty()) {
                    out.add("a %s with no workstation".formatted(trade));
                } else if (loaded(level, site.get().pos()) && Lockers.touchingContainers(level, site.get().pos()).isEmpty()) {
                    BlockPos at = site.get().pos();
                    out.add("the %s's %s at %d %d %d: no chest touching it".formatted(trade,
                            BuiltInRegistries.BLOCK.getKey(level.getBlockState(at).getBlock()).getPath(), at.getX(), at.getY(), at.getZ()));
                }
                break;
            }
        }
        return out;
    }

    /** The villager's workstation, if it has one in this dimension, loaded, with a container touching it: its locker. */
    private static GlobalPos workstation(ServerLevel level, Villager v) {
        var site = v.getBrain().getMemory(MemoryModuleType.JOB_SITE)
                .filter(s -> s.dimension() == level.dimension()).map(GlobalPos::pos).orElse(null);
        if (site == null || !loaded(level, site) || Lockers.touchingContainers(level, site).isEmpty()) {
            return null;
        }
        return GlobalPos.of(level.dimension(), site.immutable());
    }

    /** Whether the villager's workstation, if it has one in this dimension, is loaded with the blocks touching it. */
    private static boolean siteLoaded(ServerLevel level, Villager v) {
        return v.getBrain().getMemory(MemoryModuleType.JOB_SITE)
                .filter(s -> s.dimension() == level.dimension())
                .map(s -> loaded(level, s.pos())).orElse(true);
    }

    private static boolean loaded(ServerLevel level, BlockPos pos) {
        if (!level.isLoaded(pos)) {
            return false;
        }
        for (Direction dir : Direction.values()) {
            if (!level.isLoaded(pos.relative(dir))) {
                return false;
            }
        }
        return true;
    }

    /**
     * The Merchant's screen for the town at {@code desk}: the lockers the core says its Merchant manages, each at its
     * workstation with the villager's name (where it's loaded) and its offers as the core runs them, switched on or off.
     * Null when the core keeps no town there. The rest of the screen (the emerald rate, the bank, the caravans) is the
     * other concerns' ({@link MerchantView#build}'s).
     */
    public List<MerchantView.LockerView> view(ServerLevel level, BlockPos desk) {
        Long town = host.stock().townAt(GlobalPos.of(level.dimension(), desk));
        if (town == null) {
            return null;
        }
        Map<Long, UUID> villagers = new HashMap<>();
        host.hands().roster(town).hands().forEach(h -> {
            if (h.villager() != null) {
                villagers.put(h.id(), h.villager());
            }
        });
        List<MerchantView.LockerView> out = new ArrayList<>();
        for (LockerView l : town(town).lockers()) {
            if (!l.managed() || l.at().isEmpty()) {
                continue;
            }
            UUID id = villagers.get(l.hand());
            var entity = id == null ? null : level.getEntity(id);
            Component worker = entity != null ? entity.getName()
                    : Component.translatable("screen.enterprise.merchant.unclaimed");
            List<MerchantView.TradeView> trades = new ArrayList<>();
            for (OfferView o : l.offers()) {
                // The Merchant's price is the cost side: the goods a buying offer takes, the emeralds a selling one charges.
                long goods = o.sells() ? o.count() : o.price();
                long cost = o.sells() ? o.price() : o.emeralds();
                ItemStack good = new ItemStack(item(o.good()), (int) Math.max(1, Math.min(goods, 64)));
                ItemStack emeralds = new ItemStack(Items.EMERALD, (int) Math.max(1, Math.min(cost, 64)));
                if (good.isEmpty()) {
                    continue; // a good with no item here
                }
                trades.add(o.sells()
                        ? new MerchantView.TradeView(emeralds, ItemStack.EMPTY, good, key(l.hand(), o), o.on())
                        : new MerchantView.TradeView(good, ItemStack.EMPTY, emeralds, key(l.hand(), o), o.on()));
            }
            out.add(new MerchantView.LockerView(PointsAdapter.place(l.at().get()).pos(), worker, trades));
        }
        return out;
    }

    private Item item(long good) {
        return host.stock().item(good);
    }

    /** The screen's key for a hand's offer: its place and its good. */
    public static String key(long hand, OfferView offer) {
        return KEY + hand + ":" + offer.offer() + ":" + offer.good();
    }

    /**
     * A switch flipped on the Merchant's screen in core mode: the hand's offer named by {@code key} ({@link #key})
     * switched on or off in the core. False for a key that isn't one of the core's.
     */
    public boolean flip(String key, boolean on) {
        if (!key.startsWith(KEY)) {
            return false;
        }
        String[] parts = key.substring(KEY.length()).split(":");
        if (parts.length != 3) {
            return false;
        }
        try {
            long hand = Long.parseLong(parts[0]);
            long offer = Long.parseLong(parts[1]);
            long good = Long.parseLong(parts[2]);
            return host.apply(List.of(new Input.Switch(hand, offer, good, on))) instanceof Response.Done;
        } catch (NumberFormatException e) {
            return false;
        }
    }
}
