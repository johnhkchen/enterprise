package dev.b28.enterprise.caravan;

import dev.b28.enterprise.hex.Hex;
import java.util.List;
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
    long serviceSpurs; // paid for the squad's services at the far end
    int portersSent; // porters who carried a load out and walked home
    int shortRations; // trips home on short rations, after waiting at the far end for food it couldn't get
    int foodFromShops; // food points bought at the far end's shops for the road home
    double saddleWear; // share of a saddle worn since one was last used up
    long billSpurs; // paid on bills of exchange presented on this route
    long advanceSpurs; // paid up front under contracts, in coin, on this route
    long foodSpurs;
    long itemsBought;
    long purchaseSpurs;
    long animalsSold;
    long mappedAt = -1; // when a Cartographer last walked it
    boolean stopping; // no more departures; off the books once its caravan is home
    int townsMapped; // towns a Cartographer has seen along it
    long fishCaught; // by Fishermen sailing it
    String status = "new";
    List<Hex> path = List.of(); // the planned hex path, from the last departure

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

    public long advanceSpurs() {
        return advanceSpurs;
    }

    public long billSpurs() {
        return billSpurs;
    }

    public int shortRations() {
        return shortRations;
    }

    public int foodFromShops() {
        return foodFromShops;
    }

    public int portersSent() {
        return portersSent;
    }

    public long serviceSpurs() {
        return serviceSpurs;
    }

    /** Surplus bought from the destination for the trip home, in items and spurs. */
    public long itemsBought() {
        return itemsBought;
    }

    public long purchaseSpurs() {
        return purchaseSpurs;
    }

    /** When a Cartographer last walked it (-1: never), and the most towns it saw along the way. */
    public long mappedAt() {
        return mappedAt;
    }

    public int townsMapped() {
        return townsMapped;
    }

    public long fishCaught() {
        return fishCaught;
    }

    /** Animals a Shepherd sold at the destination, in all. */
    public long animalsSold() {
        return animalsSold;
    }

    /** Spurs paid to the destination for road food, in all. */
    public long foodSpurs() {
        return foodSpurs;
    }

    public String status() {
        return status;
    }

    /** Straight-line blocks between the desks, across the ground. */
    public int straightDistance() {
        double dx = from.pos().getX() - to.pos().getX();
        double dz = from.pos().getZ() - to.pos().getZ();
        return (int) Math.ceil(Math.sqrt(dx * dx + dz * dz));
    }

    /**
     * Blocks travelled: the straight distance, plus a hex's width for each hex the planned path adds over the
     * straight line of hexes (a detour). Counting hex steps alone would overcharge short routes across hex edges.
     */
    public int distance() {
        int straightHexes = Hex.line(Hex.at(from.pos()), Hex.at(to.pos())).size();
        int extra = Math.max(0, path.size() - straightHexes);
        return straightDistance() + (int) Math.round(extra * Hex.ACROSS);
    }

    /** The hex path caravans take: the planned one, or the straight line before any planning. */
    public List<Hex> path() {
        return path.isEmpty() ? Hex.line(Hex.at(from.pos()), Hex.at(to.pos())) : path;
    }

    /** Hexes the route passes through, both ends included. */
    public int hexes() {
        return path().size();
    }

    /** Coins a caravan on this route has carried home per trip, on average (a guess before the first trip). */
    public double expectedValue() {
        return trips == 0 ? 300 : (double) spursCollected / trips;
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
        tag.putLong("serviceSpurs", serviceSpurs);
        tag.putInt("portersSent", portersSent);
        tag.putInt("shortRations", shortRations);
        tag.putInt("foodFromShops", foodFromShops);
        tag.putDouble("saddleWear", saddleWear);
        tag.putLong("billSpurs", billSpurs);
        tag.putLong("advanceSpurs", advanceSpurs);
        tag.putLong("foodSpurs", foodSpurs);
        tag.putLong("itemsBought", itemsBought);
        tag.putLong("purchaseSpurs", purchaseSpurs);
        tag.putLong("animalsSold", animalsSold);
        tag.putLong("mappedAt", mappedAt);
        tag.putBoolean("stopping", stopping);
        tag.putInt("townsMapped", townsMapped);
        tag.putLong("fishCaught", fishCaught);
        tag.putString("status", status);
        int[] hexes = new int[path.size() * 2];
        for (int i = 0; i < path.size(); i++) {
            hexes[2 * i] = path.get(i).q();
            hexes[2 * i + 1] = path.get(i).r();
        }
        tag.putIntArray("path", hexes);
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
        route.serviceSpurs = tag.getLong("serviceSpurs");
        route.portersSent = tag.getInt("portersSent");
        route.shortRations = tag.getInt("shortRations");
        route.foodFromShops = tag.getInt("foodFromShops");
        route.saddleWear = tag.getDouble("saddleWear");
        route.billSpurs = tag.getLong("billSpurs");
        route.advanceSpurs = tag.getLong("advanceSpurs");
        route.foodSpurs = tag.getLong("foodSpurs");
        route.itemsBought = tag.getLong("itemsBought");
        route.purchaseSpurs = tag.getLong("purchaseSpurs");
        route.animalsSold = tag.getLong("animalsSold");
        route.mappedAt = tag.contains("mappedAt") ? tag.getLong("mappedAt") : -1;
        route.stopping = tag.getBoolean("stopping");
        route.townsMapped = tag.getInt("townsMapped");
        route.fishCaught = tag.getLong("fishCaught");
        route.status = tag.getString("status");
        int[] hexes = tag.getIntArray("path");
        List<Hex> path = new java.util.ArrayList<>();
        for (int i = 0; i + 1 < hexes.length; i += 2) {
            path.add(new Hex(hexes[i], hexes[i + 1]));
        }
        route.path = path;
        return route;
    }
}
