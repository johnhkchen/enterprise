package dev.b28.enterprise.binding.adapters;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.settlement.CensusTaker;
import dev.b28.enterprise.territory.Growth;
import dev.b28.enterprise.territory.Jobs;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.Output;
import dev.b28.enterprise.wire.Resident;
import dev.b28.enterprise.wire.Response;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.MobSpawnType;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.entity.npc.VillagerType;
import net.minecraft.world.level.ChunkPos;
import net.neoforged.neoforge.server.ServerLifecycleHooks;

/**
 * A town's people, from the core (docs/binding-plan.md 2, concern 5): the census is the town's roster's source, and its
 * villagers are views of the core's hands. Each look in core mode, every town whose desk and settlement are loaded is
 * counted around its bell (the nearest within {@value CensusTaker#RADIUS} blocks of the desk, or the desk itself if it
 * has none): its beds and the villagers living there, each by its UUID with its profession, level and age, sent as a
 * {@code Census}; the core's rules decide who joins, who's seen and who has left ({@code census.rs}). No path checks:
 * the core has no free and confined villagers.
 *
 * <p>The core's outcomes are written to the world: a child born ({@code Born}) or a hand come to live at a town
 * ({@code Settled}) is a hand with no villager, and one is spawned for it by the bell and named ({@code TagHand}, sent
 * with the next look); a hand gone with settlers ({@code Departed}) or from the world ({@code HandGone}) takes its
 * villager away; a hand's trade, level or XP changed ({@code HandLevel}) is written to its villager. A hand with no
 * villager whose town is loaded is spawned at the next census too (a birth while the town was unloaded). The adapter
 * keeps no ids: which villager is which hand is the core's ({@link HandsAdapter}); it only remembers, until the core's
 * roster shows their names taken, the villagers it has just spawned, and where it last saw each town's villagers (a
 * villager not loaded, last seen where nothing is loaded, is sent as the core has it: nobody could look for it, as
 * {@code CensusTaker} assumes it's where it was).
 *
 * <p>In Java mode the census is the Java rules' ({@code CensusTaker}) and this adapter sends none: the core's towns then
 * keep only the hands joined by hand (the Merchant on its screen), and no child is born in them. Nor does it show the
 * core's outcomes for its people: a world that ran in core mode keeps its tagged hands and counted beds, and their
 * births, levels and departures would spawn, write or discard villagers beside the Java rules' own.
 */
public final class PeopleAdapter {
    private final CoreHost host;
    /** The core's outcomes are written to the villagers: the server's host in core mode, or a test's. */
    private final boolean ownsVillagers;
    /**
     * Villagers spawned for hands, by hand, with their town: named with every look until the core's roster shows the
     * name taken (a refused look loses none), never spawned twice meanwhile, and left out of censuses until then.
     */
    private final Map<Long, Named> naming = new LinkedHashMap<>();
    /** Where each town's census last saw each of its villagers (a chunk), by town: not saved, rebuilt as they're seen. */
    private final Map<Long, Map<UUID, Long>> lastChunk = new HashMap<>();
    /** Stalls the core paid for whose villager or bell wasn't loaded yet: put up at a later look, when they are. */
    private final List<Output.Stall> stalls = new ArrayList<>();
    /** The professions by the core's names for them (a registry path: {@code farmer}, {@code merchant}). */
    private Map<String, VillagerProfession> professions;

    private record Named(long town, UUID villager) {}

    public PeopleAdapter(CoreHost host, boolean ownsVillagers) {
        this.host = host;
        this.ownsVillagers = ownsVillagers;
    }

    /**
     * A look's happenings: (in core mode) a census of every town that can be counted whole (its desk a desk, its
     * settlement's chunks and their villagers loaded: a look never loads a chunk to count, and a census of half a town
     * would have the other half leave in ten days), a villager spawned for each of its hands with none (not in this
     * census: counted at the next), and before them the names of the villagers spawned since the last look. Each town
     * counted heals its hurt villagers too ({@link dev.b28.enterprise.settlement.Beacon}).
     */
    public List<Input> happenings(MinecraftServer server, boolean census) {
        if (ownsVillagers) {
            stalls.removeIf(o -> stall(server, o));
        }
        List<Input> counts = new ArrayList<>();
        if (census) {
            for (var desk : host.points().of(PointsAdapter.Kind.DESK).entrySet()) {
                ServerLevel level = server.getLevel(desk.getKey().dimension());
                if (level == null || !level.isLoaded(desk.getKey().pos())
                        || !level.getBlockState(desk.getKey().pos()).is(Merchant.DESK.get())) {
                    continue;
                }
                BlockPos centre = centre(level, desk.getKey().pos());
                if (entitiesLoaded(level, centre, CensusTaker.RADIUS)) {
                    HandsAdapter.Roster roster = host.hands().roster(desk.getValue());
                    named(server, desk.getValue(), roster);
                    counts.add(census(level, desk.getValue(), centre, CensusTaker.RADIUS, roster));
                    views(level, desk.getValue(), centre, roster);
                    dev.b28.enterprise.settlement.Beacon.heal(level, centre, CensusTaker.RADIUS);
                }
            }
        }
        List<Input> inputs = new ArrayList<>(names());
        inputs.addAll(counts);
        return inputs;
    }

    /**
     * The hand a villager was spawned for and isn't yet named as in the core, if it's one: taken off the names to send
     * (it died before the core took its name), so the hand can be lost.
     */
    public Long unnamed(UUID villager) {
        for (var e : naming.entrySet()) {
            if (e.getValue().villager().equals(villager)) {
                naming.remove(e.getKey());
                return e.getKey();
            }
        }
        return null;
    }

    /** The names of the villagers spawned for hands and not yet taken by the core: each hand's villager from now on. */
    private List<Input> names() {
        List<Input> inputs = new ArrayList<>(naming.size());
        naming.forEach((hand, named) -> inputs.add(new Input.TagHand(hand, named.villager().toString())));
        return inputs;
    }

    /**
     * The names the core has taken for a town's hands are done with. A hand gone before its name was taken (a child
     * that died, a stale {@code TagHand} ignored) takes the villager spawned for it away, so no census joins it anew.
     */
    private void named(MinecraftServer server, long town, HandsAdapter.Roster roster) {
        Map<Long, HandsAdapter.Hand> byId = new HashMap<>();
        roster.hands().forEach(h -> byId.put(h.id(), h));
        naming.entrySet().removeIf(e -> {
            if (e.getValue().town() != town) {
                return false;
            }
            HandsAdapter.Hand h = byId.get(e.getKey());
            if (h != null && !h.away() && h.villager() == null) {
                return false;
            }
            if ((h == null || h.away()) && find(server, e.getValue().villager().toString()) instanceof Villager v) {
                v.discard();
            }
            return true;
        });
    }

    /** A town's census: the beds and villagers within {@code radius} of {@code centre}. */
    public Input census(ServerLevel level, long town, BlockPos centre, int radius) {
        return census(level, town, centre, radius, host.hands().roster(town));
    }

    /**
     * A town's census: the beds and villagers within {@code radius} of {@code centre} (but those spawned for hands and
     * not yet named), each of the town's villagers alive farther off (wandered past its reach), and each that can't be
     * looked for (not loaded, last seen in a chunk whose entities aren't loaded: wandered off the loaded ground) as the
     * core has it, so it isn't taken to have left.
     */
    private Input census(ServerLevel level, long town, BlockPos centre, int radius, HandsAdapter.Roster roster) {
        java.util.Set<UUID> pending = new java.util.HashSet<>();
        naming.values().forEach(n -> pending.add(n.villager()));
        Map<UUID, Long> before = lastChunk.getOrDefault(town, Map.of());
        Map<UUID, Long> after = new HashMap<>();
        List<Resident> residents = new ArrayList<>();
        for (Villager v : CensusTaker.villagersAround(level, centre, radius)) {
            if (pending.contains(v.getUUID())) {
                continue;
            }
            var data = v.getVillagerData();
            residents.add(new Resident(v.getUUID().toString(),
                    BuiltInRegistries.VILLAGER_PROFESSION.getKey(data.getProfession()).toString(),
                    Math.clamp(data.getLevel(), 1, 5), v.isBaby()));
            after.put(v.getUUID(), v.chunkPosition().toLong());
        }
        for (HandsAdapter.Hand h : roster.hands()) {
            if (h.away() || h.villager() == null || after.containsKey(h.villager())) {
                continue;
            }
            Entity found = find(level.getServer(), h.villager().toString());
            if (found instanceof Villager v && v.isAlive()) {
                // Wandered past the town's reach, alive: still the town's, as it is now (it stops where the ground
                // round a player stops ticking, and would be dropped as gone).
                var data = v.getVillagerData();
                residents.add(new Resident(v.getUUID().toString(),
                        BuiltInRegistries.VILLAGER_PROFESSION.getKey(data.getProfession()).toString(),
                        Math.clamp(data.getLevel(), 1, 5), v.isBaby()));
                after.put(v.getUUID(), v.chunkPosition().toLong());
                continue;
            }
            Long chunk = before.get(h.villager());
            if (chunk != null) {
                after.put(h.villager(), chunk);
            }
            if (found == null && (chunk == null || !level.areEntitiesLoaded(chunk))) {
                residents.add(new Resident(h.villager().toString(), h.trade(), Math.clamp(h.level(), 1, 5), h.baby()));
            }
        }
        lastChunk.put(town, after);
        return new Input.Census(town, CensusTaker.beds(level, centre, radius), residents);
    }

    /**
     * A town's census taken now (a command, a test): sent to the core at once, its outcomes shown, and a villager
     * spawned by {@code centre} for each of its hands with none, named with the next look (or {@link #nameNow}).
     *
     * @throws IllegalStateException when the core refuses it (a town it hasn't)
     */
    public void countNow(ServerLevel level, long town, BlockPos centre, int radius) {
        HandsAdapter.Roster roster = host.hands().roster(town);
        named(level.getServer(), town, roster);
        Response r = host.apply(List.of(census(level, town, centre, radius, roster)));
        if (r instanceof Response.Refused refused) {
            throw new IllegalStateException("the core wouldn't take town " + town + "'s census: " + refused.why());
        }
        host.hands().reload();
        views(level, town, centre);
    }

    /** The names of the villagers just spawned, sent now rather than with the next look (a save, a command, a test). */
    public void nameNow() {
        List<Input> inputs = names();
        if (!inputs.isEmpty()) {
            host.apply(inputs);
            host.hands().reload();
        }
    }

    /** Where a town's people are counted and its children are born: the bell nearest its desk, or the desk. */
    public static BlockPos centre(ServerLevel level, BlockPos desk) {
        return CensusTaker.nearestBell(level, desk).orElse(desk);
    }

    /** Every hand of the town at home with no villager gets one, spawned by {@code centre} (unless being named). */
    private void views(ServerLevel level, long town, BlockPos centre) {
        views(level, town, centre, host.hands().roster(town));
    }

    private void views(ServerLevel level, long town, BlockPos centre, HandsAdapter.Roster roster) {
        if (!level.isLoaded(centre)) {
            return;
        }
        for (HandsAdapter.Hand h : roster.hands()) {
            if (h.villager() == null && !h.away() && !naming.containsKey(h.id())) {
                Villager v = spawn(level, centre, h);
                if (v != null) {
                    naming.put(h.id(), new Named(town, v.getUUID()));
                }
            }
        }
    }

    /** A villager for a hand: a child by the bell as the mod bears one, or an adult of its trade, level and XP. */
    private Villager spawn(ServerLevel level, BlockPos centre, HandsAdapter.Hand h) {
        if (h.baby()) {
            return Growth.bear(level, centre);
        }
        VillagerProfession profession = profession(h.trade());
        if (profession == null) {
            Enterprise.LOGGER.warn("The core's hand {} is a {}, which no profession here is", h.id(), h.trade());
            return null;
        }
        BlockPos at = dev.b28.enterprise.settlement.Grounds.standing(level, centre);
        Villager v = EntityType.VILLAGER.create(level);
        if (v == null) {
            return null;
        }
        v.moveTo(at.getX() + 0.5, at.getY(), at.getZ() + 0.5, level.random.nextFloat() * 360, 0);
        v.finalizeSpawn(level, level.getCurrentDifficultyAt(at), MobSpawnType.MOB_SUMMONED, null);
        v.setVillagerData(v.getVillagerData().setType(VillagerType.byBiome(level.getBiome(at))).setProfession(profession)
                .setLevel(Math.clamp(h.level(), 1, 5)));
        v.setVillagerXp((int) Math.min(Integer.MAX_VALUE, h.xp()));
        if (h.villager() != null) {
            v.setUUID(h.villager()); // the hand's own villager made again (its record lost): the core knows it by this
        }
        level.addFreshEntity(v);
        return v;
    }

    /**
     * A villager for a hand the core names {@code tag} (its UUID) whose villager is nowhere (a stowed record lost), by
     * {@code at}, of its trade, level and XP. Null when the tag isn't a UUID or the trade no profession here.
     */
    Villager spawnFor(ServerLevel level, BlockPos at, long hand, String tag, String trade, int lvl, long xp) {
        UUID id;
        try {
            id = UUID.fromString(tag);
        } catch (IllegalArgumentException e) {
            return null;
        }
        return spawn(level, at, new HandsAdapter.Hand(hand, id, trade, lvl, xp, false, false, false));
    }

    /** The profession the core calls {@code trade} (a registry path), the vanilla one first. */
    VillagerProfession profession(String trade) {
        if (professions == null) {
            Map<String, VillagerProfession> byName = new HashMap<>();
            for (var entry : BuiltInRegistries.VILLAGER_PROFESSION.entrySet()) {
                ResourceLocation id = entry.getKey().location();
                if (id.getNamespace().equals("minecraft") || !byName.containsKey(id.getPath())) {
                    byName.put(id.getPath(), entry.getValue());
                }
            }
            professions = Map.copyOf(byName);
        }
        return professions.get(trade.contains(":") ? trade.substring(trade.indexOf(':') + 1) : trade);
    }

    /**
     * Shows one of the core's outcomes for its people in the world. A town or villager not loaded is left: a hand with
     * no villager is spawned at its town's next census, and a villager that should be gone is taken when a census sees
     * it ({@code HandGone} again). Nothing is shown by a host that doesn't own the villagers (Java mode).
     */
    public void show(Output output) {
        MinecraftServer server = ServerLifecycleHooks.getCurrentServer();
        if (server == null || !ownsVillagers) {
            return;
        }
        switch (output) {
            case Output.Born o -> arrive(server, o.town());
            case Output.Settled o -> arrive(server, o.town());
            case Output.Departed o -> remove(server, o.tag());
            case Output.HandGone o -> {
                remove(server, o.tag());
                host.wagons().forget(o.hand()); // a hand gone while away (a soldier fallen) takes its stowed villager
            }
            case Output.HandLevel o -> write(server, o);
            case Output.Stall o -> {
                if (!stall(server, o)) {
                    stalls.add(o);
                }
            }
            default -> {}
        }
    }

    /**
     * A stall the core paid for (`Stall`): the trade's workstation with a chest against it, on a free plot by the town's
     * bell ({@link Jobs#buildStall}), and the hand's villager given the job there, as the mod's {@code Jobs.hire} does:
     * the chest makes it a locker its Merchant can manage. False while its villager or bell isn't loaded (tried again
     * at a later look); true once it's done, or there's nothing to do (no workstation for the trade, no plot free).
     */
    private boolean stall(MinecraftServer server, Output.Stall o) {
        VillagerProfession profession = profession(o.trade());
        var trade = Jobs.TRADES.stream().filter(t -> t.profession() == profession).findFirst();
        if (trade.isEmpty()) {
            return true;
        }
        if (!(find(server, o.tag()) instanceof Villager v) || !(v.level() instanceof ServerLevel level)) {
            return false;
        }
        for (var desk : host.points().of(PointsAdapter.Kind.DESK).entrySet()) {
            if (desk.getValue() != o.town() || !desk.getKey().dimension().equals(level.dimension())) {
                continue;
            }
            BlockPos bell = centre(level, desk.getKey().pos());
            if (!level.isLoaded(bell)) {
                return false;
            }
            var site = Growth.site(level, bell, Jobs.STALL_SIZE, Jobs.STALL_NEAREST);
            if (site.isEmpty()) {
                Enterprise.LOGGER.info("No room by {}'s bell for its {}'s stall", o.town(), o.trade());
                return true;
            }
            // A hand that left a trade for this one: its old workstation comes down (`Jobs.leaveTrade`).
            v.getBrain().getMemory(MemoryModuleType.JOB_SITE).filter(s -> s.dimension().equals(level.dimension()))
                    .ifPresent(old -> {
                        level.getPoiManager().release(old.pos());
                        level.setBlock(old.pos(), net.minecraft.world.level.block.Blocks.AIR.defaultBlockState(), 3);
                    });
            BlockPos workstation = Jobs.buildStall(level, site.get(), trade.get());
            takeStall(level, v, workstation, profession);
            Enterprise.LOGGER.info("Town {} puts up a {} stall at {}", o.town(), o.trade(), workstation.toShortString());
            return true;
        }
        return true;
    }

    /**
     * The hand's villager takes its stall: the workstation claimed, its job site, and the trade itself at level 1 with
     * no offers of its old one, its brain rebuilt for it. With only the job site, the game wiped it from a villager with
     * no profession, which then took the nearest free workstation: seed 8675309's Wheatfell's Mason stall made a fourth
     * Leatherworker of a village cauldron.
     */
    public static void takeStall(ServerLevel level, Villager v, BlockPos workstation, VillagerProfession profession) {
        // The game registers a new block's point of interest a task later: claimed now, there's none yet to claim, the
        // job site looks void and the game wipes it, and a level 1 villager with none and no XP loses its trade.
        level.getPoiManager().take(type -> true, (type, at) -> at.equals(workstation), workstation, 1);
        level.getServer().execute(() ->
                level.getPoiManager().take(type -> true, (type, at) -> at.equals(workstation), workstation, 1));
        v.getBrain().eraseMemory(MemoryModuleType.POTENTIAL_JOB_SITE);
        v.getBrain().setMemory(MemoryModuleType.JOB_SITE, GlobalPos.of(level.dimension(), workstation));
        if (v.getVillagerData().getProfession() != profession) {
            v.setVillagerXp(0);
            v.setOffers(new net.minecraft.world.item.trading.MerchantOffers());
        }
        v.setVillagerData(v.getVillagerData().setProfession(profession).setLevel(1));
        // Its brain checks the job site against the trade it was built for, as the game's own hiring
        // (`AssignProfessionFromJobSite`) rebuilds it: one built with no trade wipes the site at once.
        v.refreshBrain(level);
    }

    /**
     * A hand takes a trade only by the core (John: "core decides"): an unemployed villager that is a town's hand lets go
     * of a free workstation it has spotted (the game's job-site claiming), so the census reports it unemployed, as the
     * core has it, and the job draw, a glut or a contract gives it its trade, at its stall. Wheatfell's village cauldrons
     * made Leatherworkers of its idle hands with no stall or draw. A villager of no town keeps the game's way. Where
     * the game hires it in the same brain tick it spots one (standing by it), {@code VillagerMixin} refuses the trade
     * and this lets go of the job site it took too. True if it let one go.
     */
    public static boolean keepToTheCore(ServerLevel level, Villager v, HandsAdapter hands) {
        if (v.getVillagerData().getProfession() != VillagerProfession.NONE || hands == null || hands.of(v.getUUID()) == null) {
            return false;
        }
        boolean let = false;
        for (var memory : List.of(MemoryModuleType.POTENTIAL_JOB_SITE, MemoryModuleType.JOB_SITE)) {
            var site = v.getBrain().getMemory(memory);
            if (site.isPresent()) {
                if (site.get().dimension().equals(level.dimension())) {
                    level.getPoiManager().release(site.get().pos());
                }
                v.getBrain().eraseMemory(memory);
                let = true;
            }
        }
        return let;
    }

    /** Is the villager a town's hand, for the game's own hiring to be refused it ({@code VillagerMixin}, core mode)? */
    public static boolean isHand(Villager v) {
        var host = v.level() instanceof ServerLevel level ? dev.b28.enterprise.binding.CoreHost.of(level.getServer()) : null;
        return host != null && host.hands().of(v.getUUID()) != null;
    }

    /** A hand came to the town: a villager for it (and any other with none) by its bell, if the town is loaded. */
    private void arrive(MinecraftServer server, long town) {
        host.hands().reload();
        for (var desk : host.points().of(PointsAdapter.Kind.DESK).entrySet()) {
            if (desk.getValue() == town) {
                ServerLevel level = server.getLevel(desk.getKey().dimension());
                if (level != null && level.isLoaded(desk.getKey().pos())) {
                    views(level, town, centre(level, desk.getKey().pos()));
                }
                return;
            }
        }
    }

    /** A hand's villager leaves the world, if it's loaded (no death: the core has the hand as it should be). */
    private void remove(MinecraftServer server, String tag) {
        Entity e = find(server, tag);
        if (e instanceof Villager v) {
            v.discard();
        }
        host.hands().reload();
    }

    /** The core's word on a hand's trade, level and XP, written to its villager if it's loaded. */
    private void write(MinecraftServer server, Output.HandLevel o) {
        if (find(server, o.tag()) instanceof Villager v) {
            writeTo(v, o.trade(), o.level(), o.xp());
        }
    }

    /** A hand's trade (the core's name), level and XP written to its villager. */
    void writeTo(Villager v, String trade, int lvl, long xp) {
        VillagerProfession profession = profession(trade);
        var data = v.getVillagerData();
        if (profession != null && profession != data.getProfession()) {
            data = data.setProfession(profession);
        }
        v.setVillagerData(data.setLevel(Math.clamp(lvl, 1, 5)));
        v.setVillagerXp((int) Math.min(Integer.MAX_VALUE, xp));
    }

    private static Entity find(MinecraftServer server, String tag) {
        UUID id;
        try {
            id = tag.isEmpty() ? null : UUID.fromString(tag);
        } catch (IllegalArgumentException e) {
            return null;
        }
        if (id == null) {
            return null;
        }
        for (ServerLevel level : server.getAllLevels()) {
            Entity e = level.getEntity(id);
            if (e != null) {
                return e;
            }
        }
        return null;
    }

    /** The entities of every chunk within {@code radius} of {@code centre} are loaded: a census there sees them all. */
    public static boolean entitiesLoaded(ServerLevel level, BlockPos centre, int radius) {
        int x0 = (centre.getX() - radius) >> 4, x1 = (centre.getX() + radius) >> 4;
        int z0 = (centre.getZ() - radius) >> 4, z1 = (centre.getZ() + radius) >> 4;
        for (int x = x0; x <= x1; x++) {
            for (int z = z0; z <= z1; z++) {
                if (!level.areEntitiesLoaded(ChunkPos.asLong(x, z))) {
                    return false;
                }
            }
        }
        return true;
    }
}
