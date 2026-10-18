package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.binding.ModPack;
import dev.b28.enterprise.binding.Society;
import dev.b28.enterprise.binding.adapters.DialAdapter;
import dev.b28.enterprise.binding.adapters.HandsAdapter;
import dev.b28.enterprise.binding.adapters.PeopleAdapter;
import dev.b28.enterprise.binding.adapters.PointsAdapter;
import dev.b28.enterprise.core.SaveStore;
import dev.b28.enterprise.wire.Answer;
import dev.b28.enterprise.wire.ContentPack;
import dev.b28.enterprise.wire.GoodFlag;
import dev.b28.enterprise.wire.Input;
import dev.b28.enterprise.wire.Output;
import dev.b28.enterprise.wire.PackGood;
import dev.b28.enterprise.wire.Query;
import dev.b28.enterprise.wire.Response;
import dev.b28.enterprise.merchant.Merchant;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.stream.Stream;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.core.Direction;
import net.minecraft.world.level.block.BedBlock;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.properties.BedPart;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.level.block.entity.ChestBlockEntity;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/**
 * The Rust core in the mod, end to end (docs/binding-plan.md B2.1): a Merchant's desk and its chest in the world, a
 * town in the core, its chests counted into a stocktake, its stock answered by the core through the binding. Each test
 * runs a host of its own on a folder of its own, so the server's host (and the other tests' desks) never meet it.
 */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class CoreBindingTests {
    // Test coordinates start at the structure block, one below the arena; the floor is y=1.
    private static final BlockPos DESK = new BlockPos(3, 2, 3);

    /** A desk with a chest of wheat, bread, an emerald and dirt (neither is one of the core's goods). */
    private static ChestBlockEntity market(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        ChestBlockEntity chest = (ChestBlockEntity) helper.getLevel().getBlockEntity(helper.absolutePos(DESK.east()));
        chest.setItem(0, new ItemStack(Items.WHEAT, 30));
        chest.setItem(1, new ItemStack(Items.BREAD, 5));
        chest.setItem(2, new ItemStack(Items.EMERALD, 3));
        chest.setItem(3, new ItemStack(Items.DIRT, 64));
        return chest;
    }

    @GameTest(template = "floor")
    public static void aDesksStocktakeComesBackFromTheCore(GameTestHelper helper) {
        ChestBlockEntity chest = market(helper);
        var level = helper.getLevel();
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk.pos(), "Testford");
            helper.assertTrue(host.stock().townAt(desk) == town, "the desk isn't its town's point in the core");
            Map<String, Double> first = host.stock().stockAt(desk);
            helper.assertTrue(first.equals(Map.of("minecraft:wheat", 30.0, "minecraft:bread", 5.0)),
                    "the core holds " + first + " after the desk's first stocktake, not its chest's 30 wheat and 5 bread");

            // A player takes 10 wheat and puts 2 books in; the next look counts the chest again.
            chest.setItem(0, new ItemStack(Items.WHEAT, 20));
            chest.setItem(4, new ItemStack(Items.BOOK, 2));
            host.look(level.getServer(), level.getGameTime());
            Map<String, Double> after = host.stock().stockAt(desk);
            helper.assertTrue(after.equals(Map.of("minecraft:wheat", 20.0, "minecraft:bread", 5.0, "minecraft:book", 2.0)),
                    "the core holds " + after + " after a look, not the chest's 20 wheat, 5 bread and 2 books");
        } finally {
            delete(dir);
        }
        if (System.getenv("ENTERPRISE_CORE_MEASURE") != null) {
            measureALookAndASave(helper);
        }
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theCoreSurvivesASaveAndLoad(GameTestHelper helper) {
        ChestBlockEntity chest = market(helper);
        var level = helper.getLevel();
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try {
            Map<String, Double> saved;
            long town;
            try (CoreHost host = CoreHost.open(dir, 7)) {
                town = host.stock().found(level, desk.pos(), "Testford");
                chest.setItem(4, new ItemStack(Items.BOOK, 2));
                host.look(level.getServer(), level.getGameTime());
                saved = host.stock().stockAt(desk);
                helper.assertTrue(host.save(), "the host didn't save");
            }
            helper.assertTrue(Files.isRegularFile(dir.resolve(SaveStore.FILE)), "no " + SaveStore.FILE + " in the world's folder");

            // A fresh host on the same folder (another seed: it must load, not make a society) knows the desk and its
            // goods from the core alone, and answers as the saved one did.
            try (CoreHost loaded = CoreHost.open(dir, 8)) {
                helper.assertTrue(loaded.pausedWhy() == null, "the loaded host is paused: " + loaded.pausedWhy());
                Long again = loaded.stock().townAt(desk);
                helper.assertTrue(again != null && again == town, "the loaded core doesn't keep the desk as town " + town);
                Map<String, Double> answered = loaded.stock().stockAt(desk);
                helper.assertTrue(answered.equals(saved), "the loaded core holds " + answered + ", the saved one " + saved);
                helper.assertTrue(saved.equals(Map.of("minecraft:wheat", 30.0, "minecraft:bread", 5.0, "minecraft:book", 2.0)),
                        "the saved core held " + saved);

                // It counts on from the saved stocktake: the chest unchanged books nothing, a bread taken is taken.
                chest.setItem(1, new ItemStack(Items.BREAD, 4));
                loaded.look(level.getServer(), level.getGameTime() + 1);
                Map<String, Double> on = loaded.stock().stockAt(desk);
                helper.assertTrue(on.equals(Map.of("minecraft:wheat", 30.0, "minecraft:bread", 4.0, "minecraft:book", 2.0)),
                        "after the load a look counts " + on + ", not on from the saved stocktake");
            }
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * The mod's content pack (B4.1): goods with their comforts, crops, meat and weights, the trades, recipes and herds,
     * and the dials, checked by the core when it makes a society from it. A pack the core refused would pause the host,
     * so a host that isn't paused took the whole of it; the dials it holds are the Java catalogue's, one for one but
     * the mod's own, at the mod's defaults.
     */
    @GameTest(template = "floor")
    public static void theModsContentPackIsTakenWholeByTheCore(GameTestHelper helper) {
        ContentPack pack = ModPack.of();
        Map<String, PackGood> goods = new java.util.HashMap<>();
        pack.goods().forEach(g -> goods.put(g.name(), g));
        helper.assertTrue(goods.get("minecraft:iron_ingot").weight() == 2.0 && goods.get("minecraft:cobblestone").weight() == 3.0
                && goods.get("minecraft:wheat").weight() == 1.0, "the pack's weights: iron " + goods.get("minecraft:iron_ingot"));
        helper.assertTrue(goods.get("minecraft:wheat").flags().contains(GoodFlag.Crop)
                && goods.get("minecraft:cooked_beef").flags().contains(GoodFlag.Meat), "a crop and a meat aren't flagged");
        helper.assertTrue(goods.get("minecraft:book").comfort().equals(java.util.Optional.of("reading"))
                && goods.get("enterprise:pipe_tobacco").comfort().equals(java.util.Optional.of("leisure")), "comforts aren't the tags'");
        helper.assertTrue(goods.containsKey("minecraft:white_wool") && goods.containsKey("minecraft:cod"),
                "goods the recipes and herds use aren't in the pack although the mod doesn't price them");
        helper.assertTrue(pack.comforts().equals(List.of("clothing", "furnishings", "leisure", "reading", "treats")),
                "kinds of comfort: " + pack.comforts());
        helper.assertTrue(pack.trades().size() == 16 && pack.trades().get(0).equals("merchant") && pack.trades().contains("farmer")
                && pack.trades().get(14).equals("none") && pack.trades().get(15).equals("nitwit"), "trades: " + pack.trades());
        helper.assertTrue(pack.recipes().size() == dev.b28.enterprise.territory.Workshop.CONVERSIONS.size()
                && pack.animals().size() == dev.b28.enterprise.territory.Husbandry.KINDS.size(), "recipes or animals missing");

        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            helper.assertTrue(host.pausedWhy() == null, "the core refused the mod's pack: " + host.pausedWhy());
            List<String> names = ((Answer.Goods) host.ask(List.of(new Query.Goods())).get(0)).goods();
            helper.assertTrue(names.equals(pack.goods().stream().map(PackGood::name).toList()), "the core's goods aren't the pack's");
            Map<String, DialAdapter.Dial> dials = new java.util.HashMap<>();
            DialAdapter.list(host).forEach(d -> dials.put(d.name(), d));
            for (var dial : dev.b28.enterprise.Dials.all()) {
                if (ModPack.JAVA_ONLY.contains(dial.name)) {
                    helper.assertTrue(!dials.containsKey(dial.name), "the core has the mod's own dial " + dial.name);
                    continue;
                }
                DialAdapter.Dial view = dials.get(dial.name);
                helper.assertTrue(view != null, "the core has no dial " + dial.name + " (the mod's, with no rule of its own in the core)");
                helper.assertTrue(view.value() == dial.initial && view.initial() == dial.initial,
                        "dial " + dial.name + " starts at " + view.value() + " (default " + view.initial() + "), the mod's is " + dial.initial);
            }
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /** A dial set in the core is the world's, kept by its save, and put back to the pack's own setting, not the core's. */
    @GameTest(template = "floor")
    public static void aDialSetInTheCoreIsKeptByItsSaveAndResetToThePacks(GameTestHelper helper) {
        Path dir = folder();
        try {
            try (CoreHost host = CoreHost.open(dir, 7)) {
                helper.assertTrue(dial(host, "birth_mood").value() == 0.5, "the pack's birth_mood isn't the mod's 0.5 (the core's is 1)");
                helper.assertTrue(DialAdapter.set(host, "shelf_days", 7.0) == null, "shelf_days wasn't set");
                helper.assertTrue(DialAdapter.set(host, "birth_mood", 0.9) == null, "birth_mood wasn't set");
                helper.assertTrue(dial(host, "shelf_days").value() == 7.0 && dial(host, "shelf_days").initial() == 5.0,
                        "shelf_days is " + dial(host, "shelf_days"));
                String why = DialAdapter.set(host, "no_such_dial", 1.0);
                helper.assertTrue(why != null && why.contains("no dial called"), "an unknown dial: " + why);
                why = DialAdapter.set(host, "wagon_slots", -1.0);
                helper.assertTrue(why != null && why.contains("isn't a count"), "a count below nothing: " + why);
                helper.assertTrue(dial(host, "shelf_days").value() == 7.0, "a refused dial moved another");
                helper.assertTrue(host.save(), "the host didn't save");
            }
            try (CoreHost loaded = CoreHost.open(dir, 8)) {
                helper.assertTrue(dial(loaded, "shelf_days").value() == 7.0, "the loaded core lost shelf_days: " + dial(loaded, "shelf_days"));
                helper.assertTrue(DialAdapter.set(loaded, "birth_mood", null) == null, "birth_mood wasn't reset");
                helper.assertTrue(dial(loaded, "birth_mood").value() == 0.5,
                        "birth_mood went back to " + dial(loaded, "birth_mood").value() + ", not the pack's 0.5");
                helper.assertTrue(DialAdapter.set(loaded, "shelf_days", null) == null && dial(loaded, "shelf_days").value() == 5.0,
                        "shelf_days wasn't put back");
            }
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * {@code /enterprise dial} is routed (B4.1): in core mode the core's dial moves and the Java catalogue (no rule of
     * which runs) doesn't; in Java mode the Java rules' dial moves and the core beside them follows. Either way a
     * reset puts both back, within the test, so no other test sees the change.
     */
    @GameTest(template = "floor")
    public static void theDialCommandIsRoutedToTheCoreOrTheJavaRules(GameTestHelper helper) {
        var server = helper.getLevel().getServer();
        CoreHost host = CoreHost.of(server);
        helper.assertTrue(host != null && host.pausedWhy() == null, "the server's core isn't running");
        var dial = dev.b28.enterprise.Dials.SHELF_DAYS;
        boolean core = Society.isCore();
        try {
            server.getCommands().performPrefixedCommand(server.createCommandSourceStack(), "enterprise dial shelf_days 7");
            helper.assertTrue(dial(host, "shelf_days").value() == 7.0, "the core's shelf_days is " + dial(host, "shelf_days").value());
            helper.assertTrue(dial.get() == (core ? 5.0 : 7.0), "the Java dial is " + dial.get() + " in " + (core ? "core" : "Java") + " mode");
            // A name no one has is told so; nothing moves.
            server.getCommands().performPrefixedCommand(server.createCommandSourceStack(), "enterprise dial no_such_dial 3");
            helper.assertTrue(dial(host, "shelf_days").value() == 7.0, "an unknown dial moved another");
        } finally {
            server.getCommands().performPrefixedCommand(server.createCommandSourceStack(), "enterprise dial shelf_days reset");
        }
        helper.assertTrue(dial(host, "shelf_days").value() == 5.0 && dial.get() == 5.0,
                "shelf_days isn't back to 5: core " + dial(host, "shelf_days").value() + ", Java " + dial.get());
        helper.succeed();
    }

    /**
     * Trade points are the core's (B4.2): a desk founds a town, and a locker, a shop, a banker and a depot of its are
     * points of that town in the core; one where one stands already, or for a town that isn't there, is refused; one
     * broken is gone. A fresh host on the saved folder rebuilds the same lookup from the core alone: the mod keeps no
     * society ids of its own.
     */
    @GameTest(template = "floor")
    public static void aTownsTradePointsAreTheCoresThroughASaveAndLoad(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        GlobalPos locker = GlobalPos.of(level.dimension(), helper.absolutePos(DESK.west(2)));
        GlobalPos shop = GlobalPos.of(level.dimension(), helper.absolutePos(DESK.north(2)));
        GlobalPos banker = GlobalPos.of(level.dimension(), helper.absolutePos(DESK.south(2)));
        GlobalPos depot = GlobalPos.of(level.dimension(), helper.absolutePos(DESK.east(2)));
        Path dir = folder();
        try {
            long town;
            Map<GlobalPos, PointsAdapter.Placed> saved;
            try (CoreHost host = CoreHost.open(dir, 7)) {
                town = host.stock().found(level, desk.pos(), "Testford");
                PointsAdapter points = host.points();
                helper.assertTrue(points.add(locker, PointsAdapter.Kind.LOCKER, town) == null, "the locker wasn't taken");
                helper.assertTrue(points.add(shop, PointsAdapter.Kind.SHOP, town) == null, "the shop wasn't taken");
                helper.assertTrue(points.add(banker, PointsAdapter.Kind.BANKER, town) == null, "the banker wasn't taken");
                helper.assertTrue(points.add(depot, PointsAdapter.Kind.DEPOT, town) == null, "the depot wasn't taken");
                helper.assertTrue(points.add(locker, PointsAdapter.Kind.SHOP, town) != null, "a point where one stands was taken");
                GlobalPos nowhere = GlobalPos.of(level.dimension(), helper.absolutePos(DESK.above(2)));
                helper.assertTrue(points.add(nowhere, PointsAdapter.Kind.SHOP, town + 99) != null && points.at(nowhere) == null,
                        "a point of no town was taken");
                helper.assertTrue(points.at(banker).equals(new PointsAdapter.Placed(PointsAdapter.Kind.BANKER, town)),
                        "the banker is " + points.at(banker));
                helper.assertTrue(points.remove(shop) && points.at(shop) == null, "the shop broken is still a point");
                helper.assertTrue(host.stock().townAt(locker) == null, "a locker is taken for a desk");
                saved = Map.copyOf(points.all());
                helper.assertTrue(saved.size() == 4 && points.towns().equals(List.of(town)), "the core keeps " + saved);
                helper.assertTrue(host.save(), "the host didn't save");
            }
            try (CoreHost loaded = CoreHost.open(dir, 8)) {
                helper.assertTrue(loaded.points().all().equals(saved), "after a load the core keeps " + loaded.points().all()
                        + ", not " + saved);
                Long again = loaded.stock().townAt(desk);
                helper.assertTrue(again != null && again == town, "the desk isn't town " + town + " after the load");
            }
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * Villagers are the core's hands by ids that never shift (B4.2): the Merchant at a desk joins its town's roster as
     * its Merchant when the core is first asked what it manages (two lockers a level), a Farmer and a Librarian as they
     * come; the Farmer dies and the Librarian behind it keeps its id; a fresh host on the saved folder finds each
     * villager's hand again by its UUID, which the core keeps (the mod keeps none).
     */
    @GameTest(template = "floor")
    public static void aVillagerIsItsTownsHandByAnIdThatOutlivesItsNeighbours(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Villager merchant = villager(helper, Merchant.PROFESSION.get(), 2, DESK.east());
        Villager farmer = villager(helper, VillagerProfession.FARMER, 3, DESK.west());
        Villager librarian = villager(helper, VillagerProfession.LIBRARIAN, 1, DESK.north());
        Path dir = folder();
        try {
            long town;
            long m;
            long l;
            try (CoreHost host = CoreHost.open(dir, 7)) {
                town = host.stock().found(level, desk.pos(), "Testford");
                HandsAdapter hands = host.hands();
                helper.assertTrue(hands.roster(town).hands().isEmpty(), "a new town has hands");
                Long lockers = hands.lockersOf(town, merchant);
                helper.assertTrue(lockers != null && lockers == 4, "a Journeyman Merchant manages " + lockers + " lockers, not 4");
                m = hands.of(merchant.getUUID()).hand();
                long f = hands.join(town, farmer);
                l = hands.join(town, librarian);
                helper.assertTrue(hands.join(town, farmer) == f, "a villager joined twice has two hands");
                var roster = hands.roster(town);
                helper.assertTrue(roster.hands().stream().map(HandsAdapter.Hand::id).toList().equals(List.of(m, f, l)),
                        "the hands are " + roster.hands());
                helper.assertTrue(roster.hands().stream().map(HandsAdapter.Hand::trade).toList()
                        .equals(List.of("merchant", "farmer", "librarian")), "the trades are " + roster.hands());
                helper.assertTrue(roster.hands().stream().map(HandsAdapter.Hand::level).toList().equals(List.of(2, 3, 1))
                        && roster.hands().get(0).merchant() && !roster.hands().get(1).merchant(), "levels or the Merchant: " + roster);
                helper.assertTrue(roster.hands().get(1).villager().equals(farmer.getUUID()), "the Farmer's hand isn't its villager's");

                helper.assertTrue(hands.lose(farmer.getUUID()), "the Farmer's death wasn't the core's");
                helper.assertTrue(!hands.lose(farmer.getUUID()) && hands.of(farmer.getUUID()) == null, "the Farmer died twice");
                var after = hands.roster(town);
                helper.assertTrue(after.hands().stream().map(HandsAdapter.Hand::id).toList().equals(List.of(m, l)),
                        "after the Farmer's death the hands are " + after.hands());
                helper.assertTrue(host.save(), "the host didn't save");
            }
            try (CoreHost loaded = CoreHost.open(dir, 8)) {
                HandsAdapter hands = loaded.hands();
                helper.assertTrue(new HandsAdapter.Where(town, l).equals(hands.of(librarian.getUUID())),
                        "after the load the Librarian is " + hands.of(librarian.getUUID()) + ", not hand " + l);
                helper.assertTrue(hands.of(merchant.getUUID()) != null && hands.of(merchant.getUUID()).hand() == m,
                        "after the load the Merchant is " + hands.of(merchant.getUUID()));
                helper.assertTrue(hands.of(farmer.getUUID()) == null, "the dead Farmer is a hand after the load");
            }
        } finally {
            delete(dir);
            merchant.discard();
            farmer.discard();
            librarian.discard();
        }
        helper.succeed();
    }

    // A settlement's bell at the arena's middle, so a census 4 blocks round it stays inside the arena; its desk beside.
    private static final BlockPos BELL = new BlockPos(3, 2, 3);
    private static final BlockPos TOWN_DESK = new BlockPos(3, 2, 5);
    private static final int CENSUS_RADIUS = 4;
    private static final long DAY = 24_000;

    /** A bell, a desk beside it with a chest of 64 bread, and {@code beds} beds round the bell (at most 4). */
    private static BlockPos settlement(GameTestHelper helper, int beds) {
        helper.setBlock(BELL, Blocks.BELL);
        helper.setBlock(TOWN_DESK, Merchant.DESK.get());
        helper.setBlock(TOWN_DESK.east(), Blocks.CHEST);
        ChestBlockEntity chest = (ChestBlockEntity) helper.getLevel().getBlockEntity(helper.absolutePos(TOWN_DESK.east()));
        chest.setItem(0, new ItemStack(Items.BREAD, 64));
        List<BlockPos> feet = List.of(new BlockPos(0, 2, 0), new BlockPos(0, 2, 1), new BlockPos(4, 2, 1), new BlockPos(4, 2, 0));
        var state = Blocks.RED_BED.defaultBlockState().setValue(BedBlock.FACING, Direction.EAST);
        for (BlockPos foot : feet.subList(0, beds)) {
            helper.setBlock(foot, state.setValue(BedBlock.PART, BedPart.FOOT));
            helper.setBlock(foot.east(), state.setValue(BedBlock.PART, BedPart.HEAD));
        }
        return helper.absolutePos(TOWN_DESK);
    }

    /** The core brought to {@code tick} (as a look's {@code Advance}, without a look's own happenings). */
    private static Response advance(CoreHost host, long tick) {
        return host.apply(List.of(new Input.Advance(tick)));
    }

    /**
     * A town's census is its roster (B4.5): the Merchant, a Farmer and a child seen round the bell join the core's town
     * as hands under their UUIDs (the child as a child), and seen again join nothing; the Farmer wanders off (its
     * villager gone from the world, no death) and is still a hand five days on, but no census has seen it for eleven and
     * it has left. A Librarian that joined on touch and was never counted is gone from the world too, but nobody knew
     * where to look for it (as a villager in a chunk that isn't loaded): it's sent as the core has it, and stays. The
     * core decides all of it (`census.rs`); the mod only counts.
     */
    @GameTest(template = "floor")
    public static void aTownsCensusIsItsRosterAndAVillagerUnseenForTenDaysHasLeft(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos desk = settlement(helper, 3);
        BlockPos bell = helper.absolutePos(BELL);
        Villager merchant = villager(helper, Merchant.PROFESSION.get(), 2, new BlockPos(2, 2, 4));
        Villager farmer = villager(helper, VillagerProfession.FARMER, 3, new BlockPos(4, 2, 4));
        Villager child = helper.spawn(EntityType.VILLAGER, new BlockPos(2, 2, 2));
        child.setAge(-24000);
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk, "Testford");
            helper.assertTrue(PeopleAdapter.centre(level, desk).equals(bell), "the town's people aren't counted at its bell");
            PeopleAdapter people = host.people();
            HandsAdapter hands = host.hands();
            people.countNow(level, town, bell, CENSUS_RADIUS);
            var roster = hands.roster(town).hands();
            helper.assertTrue(roster.size() == 3, "the census made " + roster.size() + " hands of three villagers: " + roster);
            for (Villager v : List.of(merchant, farmer, child)) {
                helper.assertTrue(hands.of(v.getUUID()) != null && hands.of(v.getUUID()).town() == town,
                        v.getUUID() + " isn't its town's hand");
            }
            var byVillager = roster.stream().collect(java.util.stream.Collectors.toMap(HandsAdapter.Hand::villager, h -> h));
            helper.assertTrue(byVillager.get(merchant.getUUID()).merchant() && byVillager.get(merchant.getUUID()).level() == 2,
                    "the Merchant is " + byVillager.get(merchant.getUUID()));
            helper.assertTrue(byVillager.get(farmer.getUUID()).trade().equals("farmer") && byVillager.get(farmer.getUUID()).level() == 3,
                    "the Farmer is " + byVillager.get(farmer.getUUID()));
            helper.assertTrue(byVillager.get(child.getUUID()).baby(), "the child is " + byVillager.get(child.getUUID()));
            people.countNow(level, town, bell, CENSUS_RADIUS);
            helper.assertTrue(hands.roster(town).hands().size() == 3, "seen again, a villager joined twice");
            measureACensus(host, level, town, bell);
            Villager stray = villager(helper, VillagerProfession.LIBRARIAN, 1, new BlockPos(1, 2, 3));
            hands.join(town, stray);
            stray.discard();

            farmer.discard();
            long t = level.getGameTime();
            advance(host, t + 5 * DAY);
            people.countNow(level, town, bell, CENSUS_RADIUS);
            helper.assertTrue(hands.of(farmer.getUUID()) != null, "five days unseen and the Farmer has left");
            advance(host, t + 11 * DAY);
            people.countNow(level, town, bell, CENSUS_RADIUS);
            helper.assertTrue(hands.of(farmer.getUUID()) == null && hands.roster(town).hands().size() == 3,
                    "eleven days unseen and the Farmer is still a hand: " + hands.roster(town).hands());
            helper.assertTrue(hands.of(stray.getUUID()) != null, "a villager nobody could look for has left");
        } finally {
            delete(dir);
            merchant.discard();
            farmer.discard();
            child.discard();
        }
        helper.succeed();
    }

    /**
     * A child is born in the core and the binding spawns it (B4.5): a town of two with four beds and bread, where the
     * dials make children come easily, has a child within five days; the core's {@code Born} is a hand with no villager,
     * and the people adapter spawns a baby by the bell for it and names it, so the core's hand is that baby's and the
     * next census sees it as that hand, not a newcomer.
     */
    @GameTest(template = "floor")
    public static void aChildBornInTheCoreIsSpawnedByItsTownsBellAndNamed(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos desk = settlement(helper, 4);
        BlockPos bell = helper.absolutePos(BELL);
        Villager merchant = villager(helper, Merchant.PROFESSION.get(), 1, new BlockPos(2, 2, 4));
        Villager farmer = villager(helper, VillagerProfession.FARMER, 1, new BlockPos(4, 2, 4));
        AABB near = new AABB(bell).inflate(CENSUS_RADIUS - 1);
        List<Villager> babies = List.of();
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk, "Testford");
            Response dials = host.apply(List.of(new Input.SetDial("birth_mood", java.util.Optional.of(-100.0)),
                    new Input.SetDial("birth_cost", java.util.Optional.of(0.0))));
            helper.assertTrue(dials instanceof Response.Done, "the dials weren't set: " + dials);
            host.people().countNow(level, town, bell, CENSUS_RADIUS);
            helper.assertTrue(host.hands().roster(town).hands().size() == 2, "two villagers aren't two hands");
            helper.assertTrue(level.getEntitiesOfClass(Villager.class, near, Villager::isBaby).isEmpty(), "a child before any was born");

            Response r = advance(host, level.getGameTime() + 5 * DAY);
            helper.assertTrue(r instanceof Response.Done done && done.outputs().stream().filter(o -> o instanceof Output.Born).count() == 1,
                    "no child born in five days: " + r);
            babies = level.getEntitiesOfClass(Villager.class, near, Villager::isBaby);
            helper.assertTrue(babies.size() == 1, babies.size() + " babies by the bell, not one");
            host.people().nameNow();
            var hands = host.hands();
            HandsAdapter.Where baby = hands.of(babies.get(0).getUUID());
            helper.assertTrue(baby != null && baby.town() == town, "the baby isn't the core's child: " + hands.roster(town).hands());
            helper.assertTrue(hands.roster(town).hands().stream().filter(HandsAdapter.Hand::baby).count() == 1
                    && hands.roster(town).hands().stream().allMatch(h -> h.villager() != null), "the hands are " + hands.roster(town));
            host.people().countNow(level, town, bell, CENSUS_RADIUS);
            helper.assertTrue(hands.roster(town).hands().size() == 3, "the baby counted again is a newcomer: " + hands.roster(town));
            helper.assertTrue(level.getEntitiesOfClass(Villager.class, near, Villager::isBaby).size() == 1, "a second baby spawned");
        } finally {
            delete(dir);
            merchant.discard();
            farmer.discard();
            babies.forEach(Villager::discard);
        }
        helper.succeed();
    }

    /**
     * The core's word on a hand is written to its villager (B4.5): a trade taken up and a level earned in the core
     * ({@code HandLevel}) become the villager's profession, level and XP; a hand the core took from the world ({@code
     * HandGone}) takes its villager away, with no death for the core to hear.
     */
    @GameTest(template = "floor")
    public static void theCoresWordOnAHandIsWrittenToItsVillager(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        Villager farmer = villager(helper, VillagerProfession.FARMER, 1, DESK.west());
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, helper.absolutePos(DESK), "Testford");
            long hand = host.hands().join(town, farmer);
            String tag = farmer.getUUID().toString();
            host.people().show(new Output.HandLevel(hand, tag, "librarian", 3, 160));
            var data = farmer.getVillagerData();
            helper.assertTrue(data.getProfession() == VillagerProfession.LIBRARIAN && data.getLevel() == 3
                    && farmer.getVillagerXp() == 160, "the villager is " + data + " with " + farmer.getVillagerXp() + " XP");
            host.people().show(new Output.HandGone(hand, tag));
            helper.assertTrue(farmer.isRemoved(), "the villager of a hand gone still stands");
        } finally {
            delete(dir);
            farmer.discard();
        }
        helper.succeed();
    }

    /**
     * What a look's census costs the server's tick, logged: counting the villagers and beds a settlement's reach (48
     * blocks) round the bell (here the neighbouring tests' too) into a {@code Census}, and the core taking the arena's
     * own (4 blocks) and answering its hands, as a look's happenings and its views do. Nothing of the wide count is sent.
     */
    private static void measureACensus(CoreHost host, net.minecraft.server.level.ServerLevel level, long town, BlockPos bell) {
        int n = 50;
        int wide = dev.b28.enterprise.settlement.CensusTaker.RADIUS;
        if (!PeopleAdapter.entitiesLoaded(level, bell, wide)) {
            Enterprise.LOGGER.info("people census: a settlement's reach round the arena isn't loaded; not measured");
            return;
        }
        long a = System.nanoTime();
        int residents = 0;
        for (int i = 0; i < n; i++) {
            residents = ((Input.Census) host.people().census(level, town, bell, wide)).residents().size();
        }
        long counted = System.nanoTime() - a;
        a = System.nanoTime();
        for (int i = 0; i < n; i++) {
            host.apply(List.of(host.people().census(level, town, bell, CENSUS_RADIUS)));
            host.hands().roster(town);
        }
        long sent = System.nanoTime() - a;
        Enterprise.LOGGER.info("people census: counting {} villagers in a settlement's reach {} ms; a census sent and the"
                + " hands asked {} ms", residents, "%.3f".formatted(counted / 1e6 / n), "%.3f".formatted(sent / 1e6 / n));
    }

    /** The core's number for the good called {@code name}. */
    private static long good(CoreHost host, String name) {
        List<String> goods = ((Answer.Goods) host.ask(List.of(new Query.Goods())).get(0)).goods();
        int i = goods.indexOf(name);
        if (i < 0) {
            throw new AssertionError("the core has no good " + name);
        }
        return i;
    }

    /** What the core says a town holds of the good called {@code name}. */
    private static double held(CoreHost host, long town, String name) {
        Answer.Stock stock = (Answer.Stock) host.ask(List.of(new Query.Stock(town))).get(0);
        return stock.stock().get((int) good(host, name));
    }

    /** Items of {@code item} in a container, loose and in the shulker boxes in it. */
    private static int inChest(ChestBlockEntity chest, net.minecraft.world.item.Item item) {
        int n = 0;
        for (int slot = 0; slot < chest.getContainerSize(); slot++) {
            ItemStack stack = chest.getItem(slot);
            if (stack.is(item)) {
                n += stack.getCount();
            } else if (stack.is(Items.SHULKER_BOX)) {
                n += inBox(stack, item);
            }
        }
        return n;
    }

    private static int inBox(ItemStack box, net.minecraft.world.item.Item item) {
        var items = net.minecraft.core.NonNullList.withSize(27, ItemStack.EMPTY);
        box.getOrDefault(net.minecraft.core.component.DataComponents.CONTAINER,
                net.minecraft.world.item.component.ItemContainerContents.EMPTY).copyInto(items);
        return items.stream().filter(i -> i.is(item)).mapToInt(ItemStack::getCount).sum();
    }

    private static ChestBlockEntity chestAt(GameTestHelper helper, BlockPos at) {
        return (ChestBlockEntity) helper.getLevel().getBlockEntity(helper.absolutePos(at));
    }

    /**
     * A step of a test that runs over ticks with a host of its own, {@code ticks} from now: a failure closes the host
     * and its folder before it fails the test, as the last step does when it passes.
     */
    private static void later(GameTestHelper helper, CoreHost host, Path dir, long ticks, Runnable step) {
        helper.runAfterDelay(ticks, () -> {
            try {
                step.run();
            } catch (RuntimeException | AssertionError e) {
                host.close();
                delete(dir);
                throw e;
            }
        });
    }

    /**
     * The core's own moves are written to a desk's chests (B4.3): bread delivered to the town outside its chests (as a
     * pallet received would be) is put in at the end of the look that counted them, merging with the bread there; the
     * stack its master Merchant puts on its own shop's shelf comes out of them, loose wheat first, then the shulker box's.
     * After each look the chests hold what the town does, in whole items, shulker box and all.
     */
    @GameTest(template = "floor")
    public static void theCoresMovesAreWrittenToADesksChestsAndItsShulkers(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        ChestBlockEntity chest = chestAt(helper, DESK.east());
        chest.setItem(0, new ItemStack(Items.WHEAT, 32));
        ItemStack box = new ItemStack(Items.SHULKER_BOX);
        box.set(net.minecraft.core.component.DataComponents.CONTAINER, net.minecraft.world.item.component.ItemContainerContents
                .fromItems(List.of(new ItemStack(Items.WHEAT, 64), new ItemStack(Items.WHEAT, 64), new ItemStack(Items.WHEAT, 64),
                        new ItemStack(Items.WHEAT, 64))));
        chest.setItem(1, box);
        chest.setItem(2, new ItemStack(Items.BREAD, 10));
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk.pos(), "Testford");
            helper.assertTrue(held(host, town, "minecraft:wheat") == 288, "the shulker box's wheat isn't counted: "
                    + host.stock().stockAt(desk));
            long t = level.getGameTime();
            host.apply(List.of(new Input.Deliver(town, good(host, "minecraft:bread"), 40)));
            helper.assertTrue(chest.getItem(2).getCount() == 10, "the chests were written before they were counted");
            host.look(level.getServer(), t + 1);
            host.settle();
            helper.assertTrue(chest.getItem(2).is(Items.BREAD) && chest.getItem(2).getCount() == 50,
                    "the delivered bread isn't in the chest with its bread: " + chest.getItem(2));
            // A master Merchant and its own shop: a day on, it puts a stack of its surplus on the shelf.
            host.apply(List.of(new Input.AddHand(town, "merchant", 5, ""), new Input.OwnShop(town)));
            host.look(level.getServer(), t + 24_000);
            host.settle();
            double wheat = held(host, town, "minecraft:wheat");
            helper.assertTrue(wheat < 288, "the master Merchant took no wheat for its shelf: " + wheat);
            helper.assertTrue(inChest(chest, Items.WHEAT) == wheat, "the chest holds " + inChest(chest, Items.WHEAT)
                    + " wheat, the town " + wheat);
            helper.assertTrue(288 - wheat < 32 || !chest.getItem(0).is(Items.WHEAT), "the loose wheat wasn't taken first");
            helper.assertTrue(inBox(chest.getItem(1), Items.WHEAT) == Math.min(256, wheat), "the shulker box holds "
                    + inBox(chest.getItem(1), Items.WHEAT));
            helper.assertTrue(inChest(chest, Items.BREAD) == (int) held(host, town, "minecraft:bread"), "the bread drifted");
            var seen = host.stock().chestsAt(desk);
            helper.assertTrue(seen.held().get("minecraft:wheat") == wheat && seen.shortBy().isEmpty(),
                    "the core takes the chests to hold " + seen);
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * The plan's race (docs/binding-plan.md 4): a hopper fills the chest's last slot between the look's count and the
     * core's write. The bread that doesn't fit goes back unmoved and stays the town's, on its books, with no room
     * counted to move it into; once a player clears the slot the next look puts it in. Nothing made, nothing lost.
     */
    @GameTest(template = "floor", timeoutTicks = 200)
    public static void aHopperFillingTheChestDuringAMoveLeavesWhatDidntFitOnTheTownsBooks(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        helper.setBlock(DESK.east().above(), Blocks.HOPPER); // feeding the chest below it, not touching the desk
        ChestBlockEntity chest = chestAt(helper, DESK.east());
        for (int slot = 0; slot < chest.getContainerSize() - 1; slot++) {
            chest.setItem(slot, new ItemStack(Items.DIRT, 64));
        }
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        CoreHost host = CoreHost.open(dir, 7);
        long town = host.stock().found(level, desk.pos(), "Testford");
        long t = level.getGameTime();
        host.apply(List.of(new Input.Deliver(town, good(host, "minecraft:bread"), 64)));
        host.look(level.getServer(), t + 1); // counted now: one slot free, so the core moves a stack in
        var hopper = (net.minecraft.world.level.block.entity.HopperBlockEntity) level.getBlockEntity(
                helper.absolutePos(DESK.east().above()));
        hopper.setItem(0, new ItemStack(Items.DIRT, 1));
        later(helper, host, dir, 20, () -> {
            helper.assertTrue(chest.getItem(chest.getContainerSize() - 1).is(Items.DIRT), "the hopper didn't fill the slot");
            host.settle(); // the move shown now: no room for it
            helper.assertTrue(inChest(chest, Items.BREAD) == 0, "bread went in where there was no room");
            host.look(level.getServer(), t + 2); // what wasn't moved told, the chest counted again: still no room
            host.settle();
            helper.assertTrue(held(host, town, "minecraft:bread") == 64, "the town lost the bread that didn't fit");
            helper.assertTrue(!host.stock().chestsAt(desk).held().containsKey("minecraft:bread"),
                    "the core takes the chest to hold bread: " + host.stock().chestsAt(desk));
            // A player clears the slot: the next look moves the bread in.
            chest.setItem(chest.getContainerSize() - 1, ItemStack.EMPTY);
            host.look(level.getServer(), t + 3);
            host.settle();
            helper.assertTrue(inChest(chest, Items.BREAD) == 64 && held(host, town, "minecraft:bread") == 64,
                    "the chest holds " + inChest(chest, Items.BREAD) + " bread, the town " + held(host, town, "minecraft:bread"));
            host.close();
            delete(dir);
            helper.succeed();
        });
    }

    /**
     * A storehouse's chests are the town's depots (B4.3): registered from its floorplan, each a point of the town's,
     * counted into its stock at once and at every look; what the desk's chests have no room for goes into them.
     */
    @GameTest(template = "floor")
    public static void aDepotsChestsAreItsTownsStoreForWhatTheDeskHasNoRoomFor(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos at = new BlockPos(1, 2, 1);
        helper.setBlock(at, Merchant.DESK.get());
        helper.setBlock(at.east(), Blocks.CHEST);
        ChestBlockEntity deskChest = chestAt(helper, at.east());
        for (int slot = 0; slot < deskChest.getContainerSize() - 1; slot++) {
            deskChest.setItem(slot, new ItemStack(Items.DIRT, 64));
        }
        helper.setBlock(new BlockPos(5, 2, 5), Blocks.CHEST); // the storehouse: two chests, apart
        helper.setBlock(new BlockPos(5, 2, 3), Blocks.CHEST);
        ChestBlockEntity a = chestAt(helper, new BlockPos(5, 2, 5));
        ChestBlockEntity b = chestAt(helper, new BlockPos(5, 2, 3));
        a.setItem(0, new ItemStack(Items.WHEAT, 10));
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(at));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk.pos(), "Testford");
            var depots = host.stock().addDepots(level, desk.pos(), helper.absolutePos(new BlockPos(4, 2, 3)),
                    helper.absolutePos(new BlockPos(6, 3, 5)));
            helper.assertTrue(depots.size() == 2 && host.stock().depots().size() == 2, "the storehouse's depots: " + depots);
            helper.assertTrue(held(host, town, "minecraft:wheat") == 10, "the depot's wheat isn't the town's");
            helper.assertTrue(host.stock().addDepots(level, desk.pos(), helper.absolutePos(new BlockPos(4, 2, 3)),
                    helper.absolutePos(new BlockPos(6, 3, 5))).isEmpty(), "a depot registered twice");
            long t = level.getGameTime();
            host.apply(List.of(new Input.Deliver(town, good(host, "minecraft:bread"), 200)));
            host.look(level.getServer(), t + 1);
            host.settle();
            helper.assertTrue(inChest(deskChest, Items.BREAD) == 64, "the desk's free slot holds "
                    + inChest(deskChest, Items.BREAD) + " bread");
            helper.assertTrue(inChest(a, Items.BREAD) + inChest(b, Items.BREAD) == 136, "the depots hold "
                    + (inChest(a, Items.BREAD) + inChest(b, Items.BREAD)) + " bread, not the 136 the desk had no room for");
            // A player takes wheat out of a depot: the next look books it.
            a.setItem(0, new ItemStack(Items.WHEAT, 5));
            host.look(level.getServer(), t + 2);
            host.settle();
            helper.assertTrue(held(host, town, "minecraft:wheat") == 5 && held(host, town, "minecraft:bread") == 200,
                    "the town holds " + host.stock().stockAt(desk));
            helper.assertTrue(host.stock().removeDepots(level, helper.absolutePos(new BlockPos(4, 2, 3)),
                    helper.absolutePos(new BlockPos(6, 3, 5))) == 2 && host.stock().depots().isEmpty(), "the depots weren't removed");
            helper.assertTrue(held(host, town, "minecraft:wheat") == 0 && held(host, town, "minecraft:bread") == 64,
                    "the depots' goods didn't go with them: " + host.stock().stockAt(desk));
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * A desk broken takes its chests' goods with it, and put back where it stood is the same town (B4.3): its chests
     * counted in once, not on top of what the town held (binding-progress' double count, settled).
     */
    @GameTest(template = "floor")
    public static void aDeskBrokenAndPutBackIsItsTownAgainWithItsGoodsCountedOnce(GameTestHelper helper) {
        market(helper);
        var level = helper.getLevel();
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk.pos(), "Testford");
            host.stock().forget(level, desk.pos());
            helper.assertTrue(host.stock().townAt(desk) == null, "the broken desk is still a point");
            helper.assertTrue(held(host, town, "minecraft:wheat") == 0 && held(host, town, "minecraft:bread") == 0,
                    "the chests' goods stayed with the town when its desk went");
            long again = host.stock().found(level, desk.pos(), "Newton");
            helper.assertTrue(again == town, "the desk put back is town " + again + ", not " + town);
            helper.assertTrue(host.stock().stockAt(desk).equals(Map.of("minecraft:wheat", 30.0, "minecraft:bread", 5.0)),
                    "the town holds " + host.stock().stockAt(desk) + ", its chest 30 wheat and 5 bread");
            helper.assertTrue(host.points().towns().equals(List.of(town)), "another town: " + host.points().all());
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * Write-back safety (docs/decisions.md, lockers; docs/binding-plan.md 1.5): bread delivered to the town while its
     * desk isn't counted (its chunk unloaded) isn't written blind; when the chunk loads the chest is counted first, a
     * player's taking meanwhile booked, and only then is the bread put in.
     */
    @GameTest(template = "floor")
    public static void aChestChangedWhileItWasntCountedIsCountedBeforeItsWritten(GameTestHelper helper) {
        ChestBlockEntity chest = market(helper);
        var level = helper.getLevel();
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk.pos(), "Testford");
            host.apply(List.of(new Input.Deliver(town, good(host, "minecraft:bread"), 20)));
            chest.setItem(0, new ItemStack(Items.WHEAT, 20)); // a player took 10 wheat
            helper.assertTrue(chest.getItem(1).getCount() == 5, "the chest was written without being counted");
            host.stock().loaded(level, new net.minecraft.world.level.ChunkPos(desk.pos()));
            host.catchUp(level.getServer());
            host.settle();
            helper.assertTrue(chest.getItem(0).getCount() == 20 && chest.getItem(1).getCount() == 25,
                    "the chest holds " + chest.getItem(0) + " and " + chest.getItem(1) + ", not 20 wheat and 25 bread");
            helper.assertTrue(held(host, town, "minecraft:wheat") == 20 && held(host, town, "minecraft:bread") == 25,
                    "the town holds " + host.stock().stockAt(desk));
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * In Java mode the Java rules own the chests (B4.3): the core beside them counts the desk but never writes to it, so
     * bread it was handed stays on its town's books, sent back unmoved each time, and nothing is booked twice.
     */
    @GameTest(template = "floor")
    public static void inJavaModeTheCoreCountsADesksChestsButNeverWritesThem(GameTestHelper helper) {
        ChestBlockEntity chest = market(helper);
        var level = helper.getLevel();
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7, false)) {
            long town = host.stock().found(level, desk.pos(), "Testford");
            host.apply(List.of(new Input.Deliver(town, good(host, "minecraft:bread"), 20)));
            long t = level.getGameTime();
            for (int i = 1; i <= 3; i++) {
                host.look(level.getServer(), t + i);
                host.settle();
            }
            helper.assertTrue(chest.getItem(1).getCount() == 5, "the core wrote to a Java-mode chest: " + chest.getItem(1));
            helper.assertTrue(held(host, town, "minecraft:bread") == 25, "the town holds " + host.stock().stockAt(desk));
            helper.assertTrue(host.stock().chestsAt(desk).held().get("minecraft:bread") == 5.0,
                    "the core takes the chest to hold " + host.stock().chestsAt(desk));
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * A desk placed where no event saw it ({@code setBlock}, as a datapack or a structure places one) is taken as a town
     * once its chunk loads (B4.3): the chunk's desks found by their point of interest, counted at once. The server's
     * host does it for every chunk; a host of the test's own, told to, so the neighbours' desks it finds stay in its
     * society, not the server's.
     */
    @GameTest(template = "floor")
    public static void aDeskPlacedUnseenIsTakenWhenItsChunkLoads(GameTestHelper helper) {
        ChestBlockEntity chest = market(helper);
        var level = helper.getLevel();
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            host.stock().loaded(level, new net.minecraft.world.level.ChunkPos(desk.pos()));
            host.catchUp(level.getServer());
            host.settle();
            helper.assertTrue(host.stock().townAt(desk) == null, "a host not told to took a desk it was never shown");
            host.stock().takeUnseenDesks();
            host.stock().loaded(level, new net.minecraft.world.level.ChunkPos(desk.pos()));
            host.catchUp(level.getServer());
            host.settle();
            helper.assertTrue(host.stock().townAt(desk) != null, "the desk wasn't taken when its chunk loaded");
            helper.assertTrue(host.stock().stockAt(desk).equals(Map.of("minecraft:wheat", 30.0, "minecraft:bread", 5.0)),
                    "the town holds " + host.stock().stockAt(desk) + ", its chest " + chest.getItem(0) + " and " + chest.getItem(1));
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /** A villager of {@code trade} at {@code level}, kept in it without a workstation (one XP). */
    private static Villager villager(GameTestHelper helper, VillagerProfession trade, int level, BlockPos at) {
        Villager v = helper.spawn(EntityType.VILLAGER, at);
        v.setVillagerData(v.getVillagerData().setProfession(trade).setLevel(level));
        v.setVillagerXp(1);
        return v;
    }

    private static DialAdapter.Dial dial(CoreHost host, String name) {
        return DialAdapter.list(host).stream().filter(d -> d.name().equals(name)).findFirst()
                .orElseThrow(() -> new AssertionError("the core has no dial " + name));
    }

    /**
     * The proof's measurements (docs/binding-plan.md B2.1), taken at the end of {@link #aDesksStocktakeComesBackFromTheCore}
     * only with {@code ENTERPRISE_CORE_MEASURE=1 just gametest} (a test of their own would change the suite's count).
     * Four desks round the arena's corners, each with five chests full (540 stacks of 15 goods), in a host of their own:
     * what a look costs the server's tick (counting the chests and handing the core its {@code Apply}; the look itself
     * runs on the core's thread) against a tick without the core (nothing), what the core's thread spends on a look, and
     * what a save of the society costs the tick that saves the world. Written to the log.
     */
    private static void measureALookAndASave(GameTestHelper helper) {
        var level = helper.getLevel();
        var server = level.getServer();
        List<net.minecraft.world.item.Item> goods = List.of(Items.WHEAT, Items.BREAD, Items.BOOK, Items.PAPER, Items.COAL,
                Items.OAK_LOG, Items.COBBLESTONE, Items.IRON_INGOT, Items.LEATHER, Items.STRING, Items.APPLE, Items.CARROT,
                Items.POTATO, Items.FLINT, Items.FEATHER);
        List<BlockPos> desks = List.of(new BlockPos(1, 2, 1), new BlockPos(1, 2, 5), new BlockPos(5, 2, 1),
                new BlockPos(5, 2, 5));
        int stacks = 0;
        for (BlockPos desk : desks) {
            helper.setBlock(desk, Merchant.DESK.get());
            for (BlockPos at : List.of(desk.east(), desk.west(), desk.north(), desk.south(), desk.above())) {
                helper.setBlock(at, Blocks.CHEST);
                ChestBlockEntity chest = (ChestBlockEntity) level.getBlockEntity(helper.absolutePos(at));
                for (int slot = 0; slot < chest.getContainerSize(); slot++) {
                    var good = goods.get(stacks++ % goods.size());
                    chest.setItem(slot, new ItemStack(good, good.getDefaultMaxStackSize()));
                }
            }
        }
        Path dir = folder();
        StringBuilder out = new StringBuilder("core measurements:");
        try (CoreHost host = CoreHost.open(dir, 7)) {
            for (int i = 0; i < desks.size(); i++) {
                host.stock().found(level, helper.absolutePos(desks.get(i)), "Measure " + i);
            }
            long t = level.getGameTime();
            int looks = 400;
            long[] tick = new long[looks];
            long start = System.nanoTime();
            long waited = 0;
            for (int i = 0; i < looks; i++) {
                // a stack moved each look, so every stocktake books something
                ChestBlockEntity chest = (ChestBlockEntity) level.getBlockEntity(helper.absolutePos(desks.get(i % desks.size()).east()));
                chest.setItem(0, new ItemStack(goods.get(0), 1 + i % 64));
                // The last look shown first (as a minute's ticks would have): a look with one in flight counts nothing.
                long w = System.nanoTime();
                host.settle();
                waited += System.nanoTime() - w;
                long a = System.nanoTime();
                host.look(server, t + 1200L * (i + 1));
                host.poll();
                tick[i] = System.nanoTime() - a;
            }
            host.settle();
            long all = System.nanoTime() - start;
            java.util.Arrays.sort(tick);
            double mean = java.util.Arrays.stream(tick).average().orElse(0) / 1e6;
            out.append("\n  %d desks, %d stacks: a look on the server's tick %.3f ms mean, p50 %.3f, p99 %.3f, max %.3f (without the core: 0)"
                    .formatted(desks.size(), stacks, mean, tick[looks / 2] / 1e6, tick[looks * 99 / 100] / 1e6, tick[looks - 1] / 1e6));
            out.append("\n  the core's thread: %d looks (a game minute each) in %.1f ms, %.3f ms a look (%.1f ms of it waited for)"
                    .formatted(looks, all / 1e6, all / 1e6 / looks, waited / 1e6));
            // A move written (stock-adapter): a stack of wheat put into and taken out of a desk's five chests, one of
            // them emptied for room, as the core's Moved would have it, on the tick that shows it.
            ChestBlockEntity room = (ChestBlockEntity) level.getBlockEntity(helper.absolutePos(desks.get(0).east()));
            room.clearContent();
            var point = dev.b28.enterprise.binding.adapters.StockAdapter.point(GlobalPos.of(level.dimension(),
                    helper.absolutePos(desks.get(0))));
            long wheat = good(host, "minecraft:wheat");
            int moves = 400;
            long[] move = new long[moves];
            for (int i = 0; i < moves; i++) {
                var moved = new dev.b28.enterprise.wire.Output.Moved(point, wheat, i % 2 == 0 ? 64 : -64);
                long a = System.nanoTime();
                host.stock().show(moved);
                move[i] = System.nanoTime() - a;
            }
            java.util.Arrays.sort(move);
            out.append("\n  a move written (64 wheat in or out of a desk's 5 chests, 135 slots): %.3f ms mean, p50 %.3f, max %.3f"
                    .formatted(java.util.Arrays.stream(move).average().orElse(0) / 1e6, move[moves / 2] / 1e6, move[moves - 1] / 1e6));
            helper.assertTrue(host.stock().pending().isEmpty(), "a measured move didn't fit");
            long[] saves = new long[20];
            for (int i = 0; i < saves.length; i++) {
                long a = System.nanoTime();
                host.save();
                saves[i] = System.nanoTime() - a;
            }
            java.util.Arrays.sort(saves);
            long bytes;
            try {
                bytes = Files.size(dir.resolve(SaveStore.FILE));
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
            out.append("\n  a save (%d bytes): %.3f ms p50, %.3f max, on the tick that saves the world"
                    .formatted(bytes, saves[saves.length / 2] / 1e6, saves[saves.length - 1] / 1e6));
        } finally {
            delete(dir);
        }
        Enterprise.LOGGER.info(out.toString());
    }

    private static Path folder() {
        try {
            return Files.createTempDirectory("enterprise-core-test");
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    private static void delete(Path dir) {
        try (Stream<Path> all = Files.walk(dir)) {
            all.sorted(Comparator.reverseOrder()).forEach(p -> p.toFile().delete());
        } catch (IOException e) {
            // a temporary folder left behind
        }
    }
}
