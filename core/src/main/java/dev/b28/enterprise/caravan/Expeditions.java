package dev.b28.enterprise.caravan;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.hex.Hex;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.MoneyChanger;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.settlement.CensusTaker;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Focus;
import dev.b28.enterprise.territory.Growth;
import dev.b28.enterprise.territory.Jobs;
import dev.b28.enterprise.territory.Labour;
import dev.b28.enterprise.territory.Terrain;
import dev.b28.enterprise.territory.Territory;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.TicketType;
import net.minecraft.tags.StructureTags;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiTypes;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.block.BellBlock;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.properties.BellAttachType;
import net.minecraft.world.level.saveddata.SavedData;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * Settlers: a town grown past growing (see {@link Focus}) with hands to spare and money beyond its reserve sends some
 * of its people out to make a new market. To a village nobody trades with yet (a naturally grown one, with no
 * Merchant's Desk), a travelling Merchant goes alone, sets up a desk by its bell with a little capital, and works it.
 * Where there's none in reach, a Merchant and a settler go to the best open ground the town knows of, between
 * {@link #MIN_HEXES} and {@link #MAX_HEXES} hexes out and that far from any other market (flat, dry, unclaimed), and
 * found a town: a bell, a desk and its chest, a Farmer's stall for the settler (who goes as its Farmer, so the town
 * grows its food from the first day), a lectern and a cartography table, and two family cottages. The settlers leave the old town's roll for the new one's; the old town opens a route to it.
 *
 * The trip is one way and has no market at the far end yet to trade with, so it isn't a caravan route: it's planned
 * over the same hexes as one (the same terrain), takes as long as a caravan would, and its people are off the world
 * on the road just as a squad is.
 */
public final class Expeditions extends SavedData {
    private static final String NAME = "enterprise_expeditions";

    /** What settling a village costs the town: a desk (4) and the capital it takes (10), in emeralds. */
    public static final double SETTLING_EMERALDS = 14;
    public static final double SETTLING_CAPITAL = 10;
    /** What founding a town costs: the bell, desk, workstations and cottages (18) and its capital (16). */
    public static final double FOUNDING_EMERALDS = 34;
    public static final double FOUNDING_CAPITAL = 16;
    /** Bread a party takes for the road and the first days: till the Farmer's first harvest, for a new town. */
    public static final int BREAD = 64;
    private static final Jobs.Trade FARMER = Jobs.TRADES.stream()
            .filter(t -> t.profession() == VillagerProfession.FARMER).findFirst().orElseThrow();
    /** A new market goes this many hexes from any other, and at most so far from home. */
    public static final int MIN_HEXES = 4;
    public static final int MAX_HEXES = 8;
    /** A town sends settlers at most once in this many days. */
    public static final double DAYS_BETWEEN = 8;

    private static final long POLL_TICKS = 100;
    private static final TicketType<ChunkPos> LANDING =
            TicketType.create("enterprise_landing", Comparator.comparingLong(ChunkPos::toLong), 300);

    public enum Kind { VILLAGE, FOUND }

    /** A party on the road: where from, where to, who (stowed), and the capital and bread it carries. */
    static final class Party {
        int id;
        GlobalPos from;
        BlockPos target;
        Kind kind;
        List<CompoundTag> crew = new ArrayList<>();
        int emeralds;
        int spurs;
        int bread;
        long departs;
        long arrives;
    }

    private final List<Party> parties = new ArrayList<>();
    private final Map<GlobalPos, Long> lastSent = new HashMap<>();
    private final List<String> founded = new ArrayList<>(); // what came of each, for the books
    private int nextId = 1;

    public static Expeditions get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Expeditions::new, Expeditions::load, null), NAME);
    }

    /**
     * Is the town raising a party of settlers: grown past growing, none of its parties on the road, its last long
     * gone? While it is, its Merchant keeps the hands waiting for work for the party rather than finding them jobs.
     */
    public boolean raising(GlobalPos desk, Focus focus, int villagers, long now) {
        return focus != Focus.GROW && villagers >= Focus.GROWN
                && parties.stream().noneMatch(p -> p.from.equals(desk))
                && now - lastSent.getOrDefault(desk, Long.MIN_VALUE / 2) >= DAYS_BETWEEN * 24000;
    }

    /**
     * At a census while loaded: does the town send settlers? Grown past growing, its last party long gone, money to
     * spare, and the hands: one waiting for work (who signs on as a travelling Merchant) for a village, two for a new
     * town. Returns where they're bound.
     */
    public Optional<BlockPos> consider(ServerLevel level, GlobalPos desk, GlobalPos bell, MerchantStock stock,
            Territory territory, Focus focus, int villagers, long now) {
        if (!raising(desk, focus, villagers, now)) {
            return Optional.empty();
        }
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, now);
        double spare = Treasury.available(level, desk.pos(), stock, rate) - territory.reserve(desk);
        List<Villager> hands = Squad.available(level, desk, Squad.reach()).stream()
                .filter(v -> Squad.trade(v).equals(Labour.NONE)).toList();
        if (hands.isEmpty() || spare < SETTLING_EMERALDS) {
            return Optional.empty();
        }
        Optional<BlockPos> village = unclaimedVillage(level, desk);
        Kind kind;
        BlockPos target;
        if (village.isPresent()) {
            kind = Kind.VILLAGE;
            target = village.get();
        } else {
            Optional<BlockPos> site = hands.size() >= 2 && spare >= FOUNDING_EMERALDS
                    ? openGround(level, desk, territory) : Optional.empty();
            if (site.isEmpty()) {
                return Optional.empty();
            }
            kind = Kind.FOUND;
            target = site.get();
        }
        // The road: planned over the hexes as a caravan's would be; out of reach or no way there, nobody goes.
        var plan = RoutePlanner.plan(territory, desk.dimension(), Hex.at(desk.pos()), Hex.at(target), Set.of(desk),
                Caravans.perHex(rate), Caravans.terrain(level.getServer(), desk.dimension()), 0, Set.of());
        int straight = (int) Math.ceil(Math.sqrt(desk.pos().distSqr(target)));
        int extra = Math.max(0, plan.path().size() - Hex.line(Hex.at(desk.pos()), Hex.at(target)).size());
        int distance = straight + (int) Math.round(extra * Hex.ACROSS);
        if (!plan.found() || !Caravans.inRange(distance)) {
            return Optional.empty();
        }
        double setUp0 = (kind == Kind.VILLAGE ? SETTLING_EMERALDS : FOUNDING_EMERALDS)
                - (kind == Kind.VILLAGE ? SETTLING_CAPITAL : FOUNDING_CAPITAL);
        if (!dev.b28.enterprise.money.Accountant.approve(level, desk, stock, "settlers", setUp0 + capital(kind, spare - setUp0), now)) {
            return Optional.empty();
        }
        Party party = new Party();
        party.id = nextId++;
        party.from = desk;
        party.target = target;
        party.kind = kind;
        Villager lead = hands.get(0);
        lead.setVillagerData(lead.getVillagerData().setProfession(Merchant.PROFESSION.get()).setLevel(1));
        lead.setVillagerXp(1); // trained: it keeps the trade on the road
        List<Villager> going = kind == Kind.VILLAGE ? List.of(lead) : List.of(lead, hands.get(1));
        if (kind == Kind.FOUND) {
            // The settler goes as its Farmer: a new town grows its own food from the first day.
            Villager farmer = hands.get(1);
            farmer.setVillagerData(farmer.getVillagerData().setProfession(VillagerProfession.FARMER).setLevel(1));
            farmer.setVillagerXp(1);
        }
        Mint mint = Mint.get(level.getServer());
        double least = kind == Kind.VILLAGE ? SETTLING_CAPITAL : FOUNDING_CAPITAL;
        double setUp = (kind == Kind.VILLAGE ? SETTLING_EMERALDS : FOUNDING_EMERALDS) - least; // desk, stalls, homes
        Treasury.withdraw(level, desk.pos(), stock, setUp, mint, now);
        double capital0 = capital(kind, spare - setUp);
        var capital = Treasury.withdraw(level, desk.pos(), stock, capital0, mint, now);
        party.emeralds = capital.emeralds();
        party.spurs = capital.spurs();
        party.bread = stock.take(new ItemCost(Items.BREAD, 1), BREAD).stream().mapToInt(ItemStack::getCount).sum();
        Settlements settlements = Settlements.get(level.getServer());
        for (Villager v : going) {
            settlements.remove(bell, v.getUUID()); // off the old town's roll: they won't be back
            party.crew.add(Squad.stow(v));
        }
        party.departs = now;
        party.arrives = now + Caravans.travelTicks(distance);
        parties.add(party);
        lastSent.put(desk, now);
        setDirty();
        Enterprise.LOGGER.info("{} sends {} to {} at {}: {} blocks, {} days on the road",
                dev.b28.enterprise.settlement.Names.label(desk),
                kind == Kind.VILLAGE ? "a travelling Merchant" : "a Merchant and a settler",
                kind == Kind.VILLAGE ? "a village nobody trades with" : "found a town", target.toShortString(), distance,
                "%.1f".formatted((party.arrives - now) / 24000.0));
        return Optional.of(target);
    }

    /**
     * What the party carries to start the new market on: half what the town can spare beyond its set-up, up to the
     * {@code founding_capital} (or {@code settling_capital}) dial's jumpstart (a new town's first stalls, pasture, home
     * and children, and its Farmer's wages until the harvest sells), and never less than the least it takes.
     */
    static double capital(Kind kind, double spare) {
        double least = kind == Kind.VILLAGE ? SETTLING_CAPITAL : FOUNDING_CAPITAL;
        double jumpstart = (kind == Kind.VILLAGE ? dev.b28.enterprise.Dials.SETTLING_CAPITAL
                : dev.b28.enterprise.Dials.FOUNDING_CAPITAL).get();
        return Math.max(least, Math.min(jumpstart, spare / 2));
    }

    /** The nearest village in reach with no market within {@link #MIN_HEXES} hexes, and no party already bound there. */
    Optional<BlockPos> unclaimedVillage(ServerLevel level, GlobalPos desk) {
        int chunks = (int) Math.ceil(MAX_HEXES * Hex.ACROSS / 16);
        BlockPos found = level.findNearestMapStructure(StructureTags.VILLAGE, desk.pos(), chunks, false);
        if (found == null) {
            return Optional.empty();
        }
        BlockPos at = new BlockPos(found.getX(), desk.pos().getY(), found.getZ());
        return clear(level.getServer(), desk.dimension(), Hex.at(at)) && Hex.at(at).distance(Hex.at(desk.pos())) <= MAX_HEXES
                ? Optional.of(at) : Optional.empty();
    }

    /**
     * The best open ground the town knows of: hexes {@link #MIN_HEXES} to {@link #MAX_HEXES} out in each of the six
     * directions, unclaimed, clear of other markets, and as flat and dry as the survey says (fresh water near is a
     * plus); the hex's middle, at the town's height.
     */
    Optional<BlockPos> openGround(ServerLevel level, GlobalPos desk, Territory territory) {
        Hex home = Hex.at(desk.pos());
        Terrain terrain = Terrain.get(level.getServer());
        Hex best = null;
        double bestScore = Double.NEGATIVE_INFINITY;
        for (Hex step : new Hex(0, 0).neighbors()) {
            for (int d = MIN_HEXES + 1; d <= MAX_HEXES - 1; d += 2) {
                Hex hex = new Hex(home.q() + d * step.q(), home.r() + d * step.r());
                if (!clear(level.getServer(), desk.dimension(), hex) || territory.owner(desk.dimension(), hex).isPresent()) {
                    continue;
                }
                double wet = terrain.waterShare(level, hex);
                int relief = terrain.relief(level, hex);
                if (wet > 0.3 || relief >= Terrain.RELIEF_PER_FACTOR) {
                    continue;
                }
                double score = -relief - 20 * wet + (wet > 0.02 ? 4 : 0) - 0.5 * d;
                if (score > bestScore) {
                    bestScore = score;
                    best = hex;
                }
            }
        }
        return Optional.ofNullable(best)
                .map(hex -> new BlockPos((int) Math.floor(hex.centerX()), desk.pos().getY(), (int) Math.floor(hex.centerZ())));
    }

    /** No market, settlement or party's goal within {@link #MIN_HEXES} hexes of this one. */
    private boolean clear(MinecraftServer server, net.minecraft.resources.ResourceKey<net.minecraft.world.level.Level> dimension,
            Hex hex) {
        boolean marketNear = Atlas.markets(server).stream()
                .anyMatch(m -> m.dimension().equals(dimension) && Hex.at(m.pos()).distance(hex) < MIN_HEXES);
        boolean townNear = Settlements.get(server).all().keySet().stream()
                .anyMatch(b -> b.dimension().equals(dimension) && Hex.at(b.pos()).distance(hex) < MIN_HEXES);
        boolean bound = parties.stream().anyMatch(p -> Hex.at(p.target).distance(hex) < MIN_HEXES);
        return !marketNear && !townNear && !bound;
    }

    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        long now = server.overworld().getGameTime();
        if (now % POLL_TICKS == 0) {
            get(server).poll(server, now);
        }
    }

    void poll(MinecraftServer server, long now) {
        for (Party party : List.copyOf(parties)) {
            if (now < party.arrives) {
                continue;
            }
            ServerLevel level = server.getLevel(party.from.dimension());
            if (level == null) {
                continue;
            }
            // The ground there loaded (people and all) before they settle on it.
            ChunkPos chunk = new ChunkPos(party.target);
            level.getChunkSource().addRegionTicket(LANDING, chunk, 3, chunk);
            if (!level.isLoaded(party.target) || !level.areEntitiesLoaded(chunk.toLong())) {
                continue;
            }
            parties.remove(party);
            founded.add(arrive(level, party, now));
            setDirty();
        }
    }

    /** A party of these villagers arriving at {@code target} now, from {@code from}: for tests and commands. */
    public String settle(ServerLevel level, GlobalPos from, BlockPos target, Kind kind, List<Villager> crew, int emeralds,
            int censusRadius, long now) {
        Party party = new Party();
        party.id = nextId++;
        party.from = from;
        party.target = target;
        party.kind = kind;
        crew.forEach(v -> party.crew.add(Squad.stow(v)));
        party.emeralds = emeralds;
        return arrive(level, party, censusRadius, now);
    }

    /** What a party builds where it founds a town: the bell, the Merchant's desk (a chest by it), the Farmer's stall. */
    public record Founding(BlockPos bell, BlockPos desk, BlockPos farm) {}

    /**
     * A new town built round {@code target} (core mode's founding view too, `binding/adapters/LandAdapter`): a bell on
     * open ground, the Merchant's desk with a chest by it, a Farmer's stall, a lectern and a cartography table, and two
     * cottages. Empty, with nothing built, where there's no open ground for the bell or no room for the desk by it.
     */
    public static Optional<Founding> buildTown(ServerLevel level, BlockPos target) {
        Optional<BlockPos> square = Growth.site(level, target, 3, 0);
        if (square.isEmpty()) {
            return Optional.empty();
        }
        BlockPos bell = square.get().offset(1, 1, 1);
        Optional<BlockPos> stall = Growth.site(level, bell, 2, 3);
        if (stall.isEmpty()) {
            return Optional.empty();
        }
        level.setBlock(bell, Blocks.BELL.defaultBlockState().setValue(BellBlock.ATTACHMENT, BellAttachType.FLOOR), 3);
        BlockPos desk = stall.get().above();
        level.setBlock(desk, Merchant.DESK.get().defaultBlockState(), 3);
        level.setBlock(desk.east(), Blocks.CHEST.defaultBlockState(), 3);
        BlockPos farm = Growth.site(level, bell, 3, 4).map(plot -> Jobs.buildStall(level, plot, FARMER)).orElse(null);
        for (var workstation : List.of(Blocks.LECTERN, Blocks.CARTOGRAPHY_TABLE)) {
            Growth.site(level, bell, 1, 4).ifPresent(at -> level.setBlock(at.above(), workstation.defaultBlockState(), 3));
        }
        for (int i = 0; i < 2; i++) {
            Growth.homeSite(level, bell).ifPresent(at -> Growth.buildHome(level, at));
        }
        return Optional.of(new Founding(bell, desk, farm));
    }

    /** The party settles: by a village's bell, or on its open ground. What came of it, for the books. */
    String arrive(ServerLevel level, Party party, long now) {
        return arrive(level, party, CensusTaker.RADIUS, now);
    }

    private String arrive(ServerLevel level, Party party, int censusRadius, long now) {
        BlockPos bell = party.kind == Kind.VILLAGE
                ? level.getPoiManager().findClosest(type -> type.is(PoiTypes.MEETING), party.target, 64,
                        PoiManager.Occupancy.ANY).orElse(null)
                : null;
        Kind kind = bell == null ? Kind.FOUND : Kind.VILLAGE;
        if (kind == Kind.FOUND && party.crew.size() < 2) {
            // A Merchant alone, bound for a village that isn't there: no town without a Farmer to feed it.
            return homeAgain(level, party, "no village at " + party.target.toShortString() + ", and no Farmer along");
        }
        BlockPos desk;
        BlockPos farm = null; // the Farmer's stall, where the settler works
        if (kind == Kind.FOUND) {
            Optional<Founding> town = buildTown(level, party.target);
            if (town.isEmpty()) {
                return homeAgain(level, party, "no open ground at " + party.target.toShortString());
            }
            bell = town.get().bell();
            desk = town.get().desk();
            farm = town.get().farm();
        } else {
            Optional<BlockPos> stall = Growth.site(level, bell, 2, 3);
            if (stall.isEmpty()) {
                return homeAgain(level, party, "no room for a desk by the bell at " + bell.toShortString());
            }
            desk = stall.get().above();
            level.setBlock(desk, Merchant.DESK.get().defaultBlockState(), 3);
            level.setBlock(desk.east(), Blocks.CHEST.defaultBlockState(), 3);
        }
        MerchantStock stock = MerchantStock.at(level, desk);
        if (party.emeralds > 0) {
            stock.insert(new ItemStack(Items.EMERALD, party.emeralds));
        }
        MoneyChanger.coins(party.spurs).forEach(stock::insert);
        if (party.bread > 0) {
            stock.insert(new ItemStack(Items.BREAD, party.bread));
        }
        GlobalPos market = GlobalPos.of(level.dimension(), desk);
        boolean first = true;
        for (CompoundTag member : party.crew) {
            Optional<Entity> back = Squad.release(level, desk, member);
            if (first && back.isPresent() && back.get() instanceof Villager merchant) {
                // The Merchant takes the desk it set up.
                level.getPoiManager().take(type -> type.is(Merchant.DESK_POI.getKey()), (type, at) -> at.equals(desk), desk, 1);
                merchant.getBrain().setMemory(MemoryModuleType.JOB_SITE, market);
            } else if (!first && farm != null && back.isPresent() && back.get() instanceof Villager farmer) {
                Jobs.hire(level, farmer, farm, FARMER, market); // and the Farmer its stall
            }
            first = false;
        }
        CensusTaker.countNow(level, bell, censusRadius);
        CaravanLedger.get(level.getServer()).addRoute(party.from, market);
        Atlas.get(level.getServer()).learnBoth(party.from, market, now);
        String name = dev.b28.enterprise.settlement.Names.of(level.getServer(), market);
        String what = (kind == Kind.VILLAGE ? "%s's Merchant settles %s, a village nobody traded with"
                : "%s's settlers found %s").formatted(dev.b28.enterprise.settlement.Names.label(party.from), name)
                + " at " + desk.toShortString();
        Enterprise.LOGGER.info(what);
        return what;
    }

    /** No place to settle: they come home (put back by the old desk) with the capital and bread. */
    private String homeAgain(ServerLevel level, Party party, String why) {
        for (CompoundTag member : party.crew) {
            Squad.release(level, party.from.pos(), member);
        }
        MerchantStock stock = MerchantStock.at(level, party.from.pos());
        if (party.emeralds > 0) {
            stock.insert(new ItemStack(Items.EMERALD, party.emeralds));
        }
        MoneyChanger.coins(party.spurs).forEach(stock::insert);
        if (party.bread > 0) {
            stock.insert(new ItemStack(Items.BREAD, party.bread));
        }
        String what = "%s's settlers came home: %s".formatted(dev.b28.enterprise.settlement.Names.label(party.from), why);
        Enterprise.LOGGER.info(what);
        return what;
    }

    /** The parties on the road and what came of those that arrived. */
    public List<String> report(long now) {
        List<String> lines = new ArrayList<>();
        parties.forEach(p -> lines.add("on the road from %s to %s (%s), %.1f days to go".formatted(
                dev.b28.enterprise.settlement.Names.label(p.from), p.target.toShortString(),
                p.kind == Kind.VILLAGE ? "a village" : "to found a town", Math.max(0, p.arrives - now) / 24000.0)));
        lines.addAll(founded);
        return lines;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        for (Party p : parties) {
            CompoundTag e = new CompoundTag();
            e.putInt("id", p.id);
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, p.from).ifSuccess(t -> e.put("from", t));
            e.putLong("target", p.target.asLong());
            e.putString("kind", p.kind.name());
            ListTag crew = new ListTag();
            crew.addAll(p.crew);
            e.put("crew", crew);
            e.putInt("emeralds", p.emeralds);
            e.putInt("spurs", p.spurs);
            e.putInt("bread", p.bread);
            e.putLong("departs", p.departs);
            e.putLong("arrives", p.arrives);
            list.add(e);
        }
        tag.put("parties", list);
        ListTag sent = new ListTag();
        lastSent.forEach((desk, at) -> {
            CompoundTag e = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> e.put("desk", t));
            e.putLong("at", at);
            sent.add(e);
        });
        tag.put("lastSent", sent);
        ListTag done = new ListTag();
        founded.forEach(line -> done.add(net.minecraft.nbt.StringTag.valueOf(line)));
        tag.put("founded", done);
        tag.putInt("nextId", nextId);
        return tag;
    }

    public static Expeditions load(CompoundTag tag, HolderLookup.Provider registries) {
        Expeditions out = new Expeditions();
        for (Tag t : tag.getList("parties", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            Party p = new Party();
            p.id = e.getInt("id");
            p.from = GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("from")).getOrThrow();
            p.target = BlockPos.of(e.getLong("target"));
            p.kind = Kind.valueOf(e.getString("kind"));
            e.getList("crew", Tag.TAG_COMPOUND).forEach(c -> p.crew.add((CompoundTag) c));
            p.emeralds = e.getInt("emeralds");
            p.spurs = e.getInt("spurs");
            p.bread = e.getInt("bread");
            p.departs = e.getLong("departs");
            p.arrives = e.getLong("arrives");
            out.parties.add(p);
        }
        for (Tag t : tag.getList("lastSent", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("desk")).result().ifPresent(d -> out.lastSent.put(d, e.getLong("at")));
        }
        tag.getList("founded", Tag.TAG_STRING).forEach(t -> out.founded.add(t.getAsString()));
        out.nextId = Math.max(1, tag.getInt("nextId"));
        return out;
    }
}
