package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.HandView;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Response;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.npc.Villager;
import net.neoforged.neoforge.event.entity.living.LivingDeathEvent;

/**
 * Villagers as the core's hands (docs/binding-plan.md 1.3 and 2, concern 2): every hand on a town's roster in the core
 * has an id of its own that it keeps while it lives, wherever it goes (a caravan's trip, a party that settles another
 * town), never its place in the roster, which shifts as others leave. A villager is tied to its hand by its UUID, which
 * the core keeps beside the hand as the host's name for it ({@code AddHand}'s tag), so this adapter keeps no ids of its
 * own: its lookup from villager to hand is the core's {@code Query.Hands} for every town with a point, read again when
 * the host starts and after every change.
 *
 * <p>A villager joins its town's roster here ({@link #join}: the Merchant at a desk, by the Merchant's screen) or by its
 * town's census ({@link PeopleAdapter}, in core mode), and leaves it when it dies ({@link #onDeath}) or when no census
 * has seen it for ten days (the core's rule). A hand away from
 * home (with a caravan, at war) is off the world and the core's to bring back: a death the host sees is never one.
 */
public final class HandsAdapter {
    /** A hand as the core keeps it: its id, the villager's UUID (null if it has none), its trade and level. */
    public record Hand(long id, UUID villager, String trade, int level, long xp, boolean baby, boolean away, boolean merchant) {}

    /** A town's hands in its roster's order, and how many lockers its desk's Merchant manages (none without one). */
    public record Roster(long town, long lockers, List<Hand> hands) {
        /** Whether the town's desk has a Merchant among its hands. */
        public boolean hasMerchant() {
            return hands.stream().anyMatch(Hand::merchant);
        }
    }

    /** Where a villager's hand is: its town and its id. */
    public record Where(long town, long hand) {}

    private final CoreHost host;
    /** Each villager the core has a hand for, by UUID: rebuilt from the core, never kept. */
    private Map<UUID, Where> byVillager = Map.of();
    /** A look has run since the lookup was read (a census, a birth, a death in it): read again before it's used. */
    private boolean stale;

    public HandsAdapter(CoreHost host) {
        this.host = host;
    }

    /** Reads every town's hands from the core again (the towns with a point: the ones a host made). */
    public void reload() {
        Map<UUID, Where> found = new HashMap<>();
        for (long town : host.points().towns()) {
            for (Hand h : roster(town).hands()) {
                if (h.villager() != null) {
                    found.put(h.villager(), new Where(town, h.id()));
                }
            }
        }
        byVillager = Map.copyOf(found);
        stale = false;
    }

    /** The core's hands may have changed (a look ran): the lookup is read again when next used. */
    public void stale() {
        stale = true;
    }

    private Map<UUID, Where> lookup() {
        if (stale) {
            reload();
        }
        return byVillager;
    }

    /** A town's hands as the core has them now. */
    public Roster roster(long town) {
        if (host.ask(List.of(new Query.Hands(town))).get(0) instanceof Answer.Hands a) {
            List<Hand> hands = new ArrayList<>(a.hands().size());
            for (HandView h : a.hands()) {
                hands.add(new Hand(h.hand(), uuid(h.tag()), h.trade(), h.level(), h.xp(), h.baby(), h.away(), h.merchant()));
            }
            return new Roster(a.town(), a.lockers(), List.copyOf(hands));
        }
        throw new IllegalStateException("the core answered Hands with something else");
    }

    /** Where the villager's hand is, if the core has one for it. */
    public Where of(UUID villager) {
        return lookup().get(villager);
    }

    /**
     * The villager comes to live at {@code town}: a hand of its profession and level, named by its UUID. A villager
     * the core has a hand for already keeps it.
     *
     * @return the hand's id
     * @throws IllegalStateException when the core refuses it (a profession the core has no trade for)
     */
    public long join(long town, Villager villager) {
        Where known = lookup().get(villager.getUUID());
        if (known != null) {
            return known.hand();
        }
        var data = villager.getVillagerData();
        String trade = BuiltInRegistries.VILLAGER_PROFESSION.getKey(data.getProfession()).toString();
        Response r = host.apply(List.of(new Input.AddHand(town, trade, Math.clamp(data.getLevel(), 1, 5),
                villager.getUUID().toString())));
        if (r instanceof Response.Refused refused) {
            throw new IllegalStateException("the core wouldn't take " + villager.getUUID() + " (" + trade + ") at town "
                    + town + ": " + refused.why());
        }
        reload();
        Where joined = byVillager.get(villager.getUUID());
        if (joined == null) {
            throw new IllegalStateException("the core took " + villager.getUUID() + " but doesn't list it");
        }
        return joined.hand();
    }

    /**
     * The villager left the society for good (it died): its hand off its town's roster. False when the core has no
     * hand for it, or its hand is away (the core's to bring home).
     */
    public boolean lose(UUID villager) {
        Where where = lookup().get(villager);
        if (where == null) {
            return false;
        }
        Response r = host.apply(List.of(new Input.HandLost(where.hand())));
        reload();
        return r instanceof Response.Done;
    }

    /**
     * How many lockers the Merchant at a desk manages, as the core counts it: the desk's town's, the villager joined
     * as its hand first if the core hasn't it yet (computed on touch). Null when the desk isn't a town in the core, or
     * the core won't take the villager.
     */
    public Long lockersOf(long town, Villager merchant) {
        try {
            join(town, merchant);
        } catch (IllegalStateException e) {
            Enterprise.LOGGER.warn("The core didn't take the Merchant {}: {}", merchant.getUUID(), e.getMessage());
            return null;
        }
        Roster roster = roster(town);
        return roster.hasMerchant() ? roster.lockers() : null;
    }

    /**
     * A villager died: if it's one of the core's hands, it's lost from its town's roster. Heard last ({@code
     * EventPriority.LOWEST}, cancelled deaths not heard: a totem's save is no death); failures logged, never thrown.
     */
    public static void onDeath(LivingDeathEvent event) {
        if (event.getEntity() instanceof Villager villager && villager.level() instanceof ServerLevel level) {
            CoreHost host = CoreHost.of(level.getServer());
            if (host != null && host.pausedWhy() == null) {
                try {
                    host.hands().lose(villager.getUUID());
                } catch (RuntimeException e) {
                    Enterprise.LOGGER.error("The core didn't hear villager {} die", villager.getUUID(), e);
                }
            }
        }
    }

    private static UUID uuid(String tag) {
        try {
            return tag.isEmpty() ? null : UUID.fromString(tag);
        } catch (IllegalArgumentException e) {
            return null;
        }
    }
}
