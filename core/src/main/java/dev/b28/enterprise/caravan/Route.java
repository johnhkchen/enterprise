package dev.b28.enterprise.caravan;

import dev.b28.enterprise.hex.Hex;
import net.minecraft.core.GlobalPos;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtOps;

/** A caravan route from a hub Merchant's desk to a settlement's Merchant's desk, with its running totals. */
public final class Route {
    final int id;
    final GlobalPos from;
    final GlobalPos to;
    long lastDeparture = Long.MIN_VALUE / 2;
    int trips;
    long spursCollected;
    long foodEaten;
    long itemsDelivered;
    long tollsPaid;
    long salesSpurs;
    long foodSpurs;
    String status = "new";

    Route(int id, GlobalPos from, GlobalPos to) {
        this.id = id;
        this.from = from;
        this.to = to;
    }

    public int id() {
        return id;
    }

    public GlobalPos from() {
        return from;
    }

    public GlobalPos to() {
        return to;
    }

    public int trips() {
        return trips;
    }

    public long spursCollected() {
        return spursCollected;
    }

    public long foodEaten() {
        return foodEaten;
    }

    public long itemsDelivered() {
        return itemsDelivered;
    }

    public long tollsPaid() {
        return tollsPaid;
    }

    /** Spurs the destination paid for cargo, in all. */
    public long salesSpurs() {
        return salesSpurs;
    }

    /** Spurs paid to the destination for road food, in all. */
    public long foodSpurs() {
        return foodSpurs;
    }

    public String status() {
        return status;
    }

    /** Straight-line blocks between the desks, across the ground. */
    public int distance() {
        double dx = from.pos().getX() - to.pos().getX();
        double dz = from.pos().getZ() - to.pos().getZ();
        return (int) Math.ceil(Math.sqrt(dx * dx + dz * dz));
    }

    /** Hexes the route passes through, both ends included. */
    public int hexes() {
        return Hex.line(Hex.at(from.pos()), Hex.at(to.pos())).size();
    }

    CompoundTag save() {
        CompoundTag tag = new CompoundTag();
        tag.putInt("id", id);
        GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, from).ifSuccess(t -> tag.put("from", t));
        GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, to).ifSuccess(t -> tag.put("to", t));
        tag.putLong("lastDeparture", lastDeparture);
        tag.putInt("trips", trips);
        tag.putLong("spursCollected", spursCollected);
        tag.putLong("foodEaten", foodEaten);
        tag.putLong("itemsDelivered", itemsDelivered);
        tag.putLong("tollsPaid", tollsPaid);
        tag.putLong("salesSpurs", salesSpurs);
        tag.putLong("foodSpurs", foodSpurs);
        tag.putString("status", status);
        return tag;
    }

    static Route load(CompoundTag tag) {
        GlobalPos from = GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("from")).getOrThrow();
        GlobalPos to = GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("to")).getOrThrow();
        Route route = new Route(tag.getInt("id"), from, to);
        route.lastDeparture = tag.getLong("lastDeparture");
        route.trips = tag.getInt("trips");
        route.spursCollected = tag.getLong("spursCollected");
        route.foodEaten = tag.getLong("foodEaten");
        route.itemsDelivered = tag.getLong("itemsDelivered");
        route.tollsPaid = tag.getLong("tollsPaid");
        route.salesSpurs = tag.getLong("salesSpurs");
        route.foodSpurs = tag.getLong("foodSpurs");
        route.status = tag.getString("status");
        return route;
    }
}
