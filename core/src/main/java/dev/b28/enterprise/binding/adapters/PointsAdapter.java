package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.Point;
import dev.b28.enterprise.wire.PointKind;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.wire.TownPoint;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.Level;

/**
 * Trade points, from the core (docs/binding-plan.md 1.3 and 2, concern 2; {@code docs/architecture.md} convention 3):
 * the places in the world where the society meets it, a Merchant's desk, a villager's locker, a shop, a banker, a
 * depot, or a market off the map (a realm's, the front: no chests, nowhere to see), each a town's. The core keeps them ({@code AddTown}, {@code AddPoint}, {@code RemovePoint}) and this adapter
 * keeps none of its own: its lookup by place is the core's {@code Query.Points}, read again when the host starts (a
 * world loaded) and after every change, so the core's save is the only record of which block is which town's.
 *
 * <p>The other adapters ask here which town a block is ({@link #townAt}) and register the points of their concern
 * here ({@link #add}, {@link #remove}): desks by the stock adapter, the rest as their concerns move to the core
 * (depots with {@code stock-adapter}, bankers with {@code money-adapter}, lockers with {@code locker-adapter}, shops
 * with {@code shop-adapter}).
 */
public final class PointsAdapter {
    /** What a trade point is: the adapters' own names for the core's kinds. */
    public enum Kind { DESK, LOCKER, SHOP, BANKER, DEPOT, MARKET }

    /** A point: what it is, and its town's number in the core. */
    public record Placed(Kind kind, long town) {}

    private final CoreHost host;
    /** Every point the core keeps, by place, in the core's order (dimension, then x, y, z). */
    private Map<GlobalPos, Placed> points = Map.of();

    public PointsAdapter(CoreHost host) {
        this.host = host;
    }

    /** Reads the points from the core again. */
    public void reload() {
        if (host.ask(List.of(new Query.Points())).get(0) instanceof Answer.Points p) {
            Map<GlobalPos, Placed> byPlace = new LinkedHashMap<>();
            for (TownPoint tp : p.points()) {
                byPlace.put(place(tp.point()), new Placed(kind(tp.point().kind()), tp.town()));
            }
            points = byPlace;
        }
    }

    /** The point at {@code at}, if the core keeps one there. */
    public Placed at(GlobalPos at) {
        return points.get(at);
    }

    /** The town of the point at {@code at}, if the core keeps one there (of any kind). */
    public Long townAt(GlobalPos at) {
        Placed p = points.get(at);
        return p == null ? null : p.town();
    }

    /** Every point the core keeps, with what it is and its town. */
    public Map<GlobalPos, Placed> all() {
        return points;
    }

    /** The points of one kind, with their towns. */
    public Map<GlobalPos, Long> of(Kind kind) {
        Map<GlobalPos, Long> out = new LinkedHashMap<>();
        points.forEach((at, p) -> {
            if (p.kind() == kind) {
                out.put(at, p.town());
            }
        });
        return out;
    }

    /** Every town that has a point, by number. */
    public List<Long> towns() {
        return new ArrayList<>(new TreeSet<>(points.values().stream().map(Placed::town).toList()));
    }

    /**
     * A town founded at the desk at {@code desk}, called {@code name}: the core brought to the overworld's time first,
     * so the town is founded at this tick, not at the last look's. A desk the core keeps already is left as it is.
     *
     * @return the desk's town
     * @throws IllegalStateException when the core refuses it
     */
    public long found(ServerLevel level, BlockPos desk, String name) {
        GlobalPos at = GlobalPos.of(level.dimension(), desk);
        Placed known = points.get(at);
        if (known != null && known.kind() == Kind.DESK) {
            return known.town();
        }
        long now = level.getServer().overworld().getGameTime();
        Response r = host.apply(List.of(new Input.Advance(now), new Input.AddTown(name, point(Kind.DESK, at))));
        if (r instanceof Response.Refused refused) {
            throw new IllegalStateException("the core wouldn't take a town at " + desk.toShortString() + ": " + refused.why());
        }
        reload();
        Placed placed = points.get(at);
        if (placed == null) {
            throw new IllegalStateException("the core took a town at " + desk.toShortString() + " but doesn't list it");
        }
        return placed.town();
    }

    /**
     * A point of {@code kind} placed at {@code at} for {@code town}. Returns null when the core took it, or why it
     * wouldn't (no such town, a point there already).
     */
    public String add(GlobalPos at, Kind kind, long town) {
        Response r = host.apply(List.of(new Input.AddPoint(point(kind, at), town)));
        reload();
        return r instanceof Response.Refused refused ? refused.why().toString() : null;
    }

    /** The point at {@code at} broken: no longer the core's (its town keeps what it held). False if there was none. */
    public boolean remove(GlobalPos at) {
        Placed p = points.get(at);
        if (p == null) {
            return false;
        }
        host.apply(List.of(new Input.RemovePoint(point(p.kind(), at))));
        reload();
        return true;
    }

    /** A point as the core keeps it: what it is, the dimension's id and the block. */
    public static Point point(Kind kind, GlobalPos at) {
        BlockPos p = at.pos();
        return new Point(wire(kind), at.dimension().location().toString(), p.getX(), p.getY(), p.getZ());
    }

    static GlobalPos place(Point point) {
        ResourceKey<Level> dim = ResourceKey.create(Registries.DIMENSION, ResourceLocation.parse(point.dim()));
        return GlobalPos.of(dim, new BlockPos(point.x(), point.y(), point.z()));
    }

    private static PointKind wire(Kind kind) {
        return switch (kind) {
            case DESK -> PointKind.Desk;
            case LOCKER -> PointKind.Locker;
            case SHOP -> PointKind.Shop;
            case BANKER -> PointKind.Banker;
            case DEPOT -> PointKind.Depot;
            case MARKET -> PointKind.Market;
        };
    }

    private static Kind kind(PointKind kind) {
        return switch (kind) {
            case Desk -> Kind.DESK;
            case Locker -> Kind.LOCKER;
            case Shop -> Kind.SHOP;
            case Banker -> Kind.BANKER;
            case Depot -> Kind.DEPOT;
            case Market -> Kind.MARKET;
        };
    }
}
