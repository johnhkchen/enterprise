package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.binding.CoreHost;
import dev.b28.enterprise.binding.ModPack;
import dev.b28.enterprise.binding.Society;
import dev.b28.enterprise.binding.VillagerCrafts;
import dev.b28.enterprise.wire.PackRecipe;
import dev.b28.enterprise.binding.adapters.DialAdapter;
import dev.b28.enterprise.binding.adapters.HandsAdapter;
import dev.b28.enterprise.binding.adapters.LandAdapter;
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
        int own = dev.b28.enterprise.territory.Workshop.CONVERSIONS.size();
        helper.assertTrue(pack.recipes().size() > own + VillagerCrafts.EXTRAS.size()
                && pack.animals().size() == dev.b28.enterprise.territory.Husbandry.KINDS.size(), "recipes or animals missing");
        // The game's recipes the villagers work: a Librarian breaks bookshelves into books, a Fletcher (who sells arrows)
        // fletches them from the game's recipe; every one is worked by a trade that deals in what it makes.
        helper.assertTrue(pack.recipes().stream().anyMatch(r -> r.output().equals("minecraft:book") && r.trades().equals(List.of("librarian"))
                && r.inputs().equals(List.of(new dev.b28.enterprise.wire.PackAmount("minecraft:bookshelf", 1)))), "no bookshelf into books");
        helper.assertTrue(pack.recipes().stream().anyMatch(r -> r.output().equals("minecraft:arrow") && r.trades().contains("fletcher")),
                "the Fletcher doesn't fletch arrows: " + pack.recipes().stream().map(PackRecipe::output).toList());
        var dealsIn = VillagerCrafts.dealsIn(helper.getLevel().getServer(), new java.util.HashSet<>(pack.trades()));
        for (PackRecipe r : pack.recipes().subList(own + VillagerCrafts.EXTRAS.size(), pack.recipes().size())) {
            helper.assertTrue(r.trades().stream().allMatch(t -> dealsIn.getOrDefault(r.output(), java.util.Set.of()).contains(t)),
                    r + ": made by a trade that doesn't deal in it");
        }

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
     * A villager spawned for a hand is that hand's in the save (B4.5): a child born in the core and spawned by the bell
     * is named with the next look, but a save before it sends the name first, so a world stopped in between loads with
     * the child as the core's hand (not a newcomer for the next census, with the hand spawned again).
     */
    @GameTest(template = "floor")
    public static void aVillagerSpawnedForAHandIsItsHandsInTheSave(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos desk = settlement(helper, 4);
        BlockPos bell = helper.absolutePos(BELL);
        Villager merchant = villager(helper, Merchant.PROFESSION.get(), 1, new BlockPos(2, 2, 4));
        Villager farmer = villager(helper, VillagerProfession.FARMER, 1, new BlockPos(4, 2, 4));
        AABB near = new AABB(bell).inflate(CENSUS_RADIUS - 1);
        List<Villager> babies = List.of();
        Path dir = folder();
        try {
            long town;
            try (CoreHost host = CoreHost.open(dir, 7)) {
                town = host.stock().found(level, desk, "Testford");
                host.apply(List.of(new Input.SetDial("birth_mood", java.util.Optional.of(-100.0)),
                        new Input.SetDial("birth_cost", java.util.Optional.of(0.0))));
                host.people().countNow(level, town, bell, CENSUS_RADIUS);
                advance(host, level.getGameTime() + 5 * DAY);
                babies = level.getEntitiesOfClass(Villager.class, near, Villager::isBaby);
                helper.assertTrue(babies.size() == 1, babies.size() + " babies by the bell, not one");
                helper.assertTrue(host.save(), "the core didn't save");
            }
            try (CoreHost loaded = CoreHost.open(dir, 8)) {
                HandsAdapter.Where baby = loaded.hands().of(babies.get(0).getUUID());
                helper.assertTrue(baby != null && baby.town() == town,
                        "the saved core doesn't have the baby as its child: " + loaded.hands().roster(town).hands());
                helper.assertTrue(loaded.hands().roster(town).hands().stream().allMatch(h -> h.villager() != null),
                        "a hand saved with no villager: " + loaded.hands().roster(town).hands());
            }
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

    // A Librarian's lectern with its locker chest touching it, beside the desk and its chest (LockerTradingTests' market).
    private static final BlockPos LECTERN = new BlockPos(1, 2, 1);
    private static final BlockPos LOCKER = new BlockPos(0, 2, 1);

    /** A desk with an empty chest, a Librarian at its lectern and a Merchant at the desk, both claimed at once. */
    private static List<Villager> lockerMarket(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        helper.setBlock(LECTERN, Blocks.LECTERN);
        Villager librarian = villager(helper, VillagerProfession.LIBRARIAN, 1, new BlockPos(1, 2, 2));
        Villager merchant = villager(helper, Merchant.PROFESSION.get(), 1, new BlockPos(4, 2, 4));
        LockerTradingTests.claim(helper, librarian, LECTERN);
        LockerTradingTests.claim(helper, merchant, DESK);
        return List.of(librarian, merchant);
    }

    /** A look's counts and time as the host sends them: the desk's and the locker's chests, then time to {@code now}. */
    private static Response lockerLook(CoreHost host, net.minecraft.server.level.ServerLevel level, long now) {
        List<Input> inputs = new java.util.ArrayList<>();
        host.points().all().forEach((at, p) -> {
            if (p.kind() == PointsAdapter.Kind.DESK || p.kind() == PointsAdapter.Kind.LOCKER) {
                inputs.add(host.stock().stocktake(level, p.kind(), at));
            }
        });
        inputs.add(new Input.Advance(now));
        return host.apply(inputs);
    }

    /**
     * A locker is a chest touching a villager's workstation, and the core runs it (B4.6): a Librarian with no chest by
     * its lectern has no locker the Merchant manages; with one, the next look ties its locker there, and the Merchant's
     * screen shows it with the core's offers, all off. A switch flipped there is the core's. The 64 paper a player left
     * in the locker are the town's, carried to the desk's chest by the look that counts them; two days on, the core's
     * restocks sold two lots of 24 (the town's paper, from the desk's chest) and the Librarian and the Merchant have
     * their XP. Saved and loaded, the locker and its switch stand; the chest broken, no locker.
     */
    @GameTest(template = "floor")
    public static void aLockerIsAWorkstationsChestAndTheCoreRunsItsSwitchedOnTrades(GameTestHelper helper) {
        var level = helper.getLevel();
        List<Villager> villagers = lockerMarket(helper);
        Villager librarian = villagers.get(0);
        Villager merchant = villagers.get(1);
        BlockPos desk = helper.absolutePos(DESK);
        BlockPos lectern = helper.absolutePos(LECTERN);
        Path dir = folder();
        try {
            long town;
            String paperKey;
            try (CoreHost host = CoreHost.open(dir, 7)) {
                town = host.stock().found(level, desk, "Testford");
                long hand = host.hands().join(town, librarian);
                host.hands().join(town, merchant);
                var lockers = host.lockers();
                lockers.placeNow(level, town);
                helper.assertTrue(lockers.town(town).of(hand).map(l -> !l.managed() && l.at().isEmpty()).orElse(false),
                        "a Librarian with no chest by its lectern has a locker: " + lockers.town(town));
                helper.assertTrue(lockers.view(level, desk).isEmpty(), "the screen shows a locker that isn't there");
                helper.setBlock(LOCKER, Blocks.CHEST);
                ChestBlockEntity locker = chestAt(helper, LOCKER);
                locker.setItem(0, new ItemStack(Items.PAPER, 64));
                lockers.placeNow(level, town);
                var view = lockers.view(level, desk);
                helper.assertTrue(view.size() == 1 && view.get(0).workstation().equals(lectern)
                        && !view.get(0).trades().isEmpty() && view.get(0).trades().stream().noneMatch(
                                dev.b28.enterprise.merchant.MerchantView.TradeView::enabled),
                        "the screen shows " + view);
                paperKey = view.get(0).trades().stream().filter(t -> t.costA().is(Items.PAPER)).findFirst()
                        .orElseThrow(() -> new AssertionError("no paper trade on the screen: " + lockers.view(level, desk)))
                        .key();
                helper.assertTrue(lockers.flip(paperKey, true), "the core didn't take the switch " + paperKey);
                String key = paperKey;
                helper.assertTrue(lockers.view(level, desk).get(0).trades().stream().anyMatch(t -> t.key().equals(key) && t.enabled()),
                        "switched on, the screen shows it off");
                ChestBlockEntity stock = chestAt(helper, DESK.east());
                long t = level.getGameTime();
                lockerLook(host, level, t);
                helper.assertTrue(inChest(locker, Items.PAPER) == 0 && inChest(stock, Items.PAPER) == 64,
                        "the locker's paper isn't in the desk's chest: " + inChest(locker, Items.PAPER) + " in the locker, "
                                + inChest(stock, Items.PAPER) + " at the desk");
                lockerLook(host, level, t + 2 * DAY);
                helper.assertTrue(inChest(stock, Items.PAPER) == 16 && held(host, town, "minecraft:paper") == 16,
                        "two lots of 24 weren't sold: " + inChest(stock, Items.PAPER) + " paper at the desk, the town "
                                + held(host, town, "minecraft:paper"));
                helper.assertTrue(librarian.getVillagerXp() == 4, "the Librarian has " + librarian.getVillagerXp() + " XP, not 4");
                int xp = merchant.getVillagerXp();
                helper.assertTrue(xp >= 6 && xp <= 12, "the Merchant earns 3 to 6 XP a trade, like a player; it has " + xp);
                helper.assertTrue(host.save(), "the core didn't save");
            }
            try (CoreHost loaded = CoreHost.open(dir, 8)) {
                var view = loaded.lockers().view(level, desk);
                helper.assertTrue(view.size() == 1 && view.get(0).workstation().equals(lectern)
                        && view.get(0).trades().stream().anyMatch(t -> t.key().equals(paperKey) && t.enabled()),
                        "loaded, the locker or its switch is gone: " + view);
                helper.setBlock(LOCKER, Blocks.AIR);
                loaded.lockers().placeNow(level, town);
                helper.assertTrue(loaded.lockers().view(level, desk).isEmpty()
                        && loaded.points().of(PointsAdapter.Kind.LOCKER).isEmpty(), "its chest broken, the locker stands");
            }
        } finally {
            delete(dir);
            villagers.forEach(Villager::discard);
        }
        helper.succeed();
    }

    /**
     * What {@code Rounds} and {@code LockerCatchUp} did for a market nobody was near (loaded its chunks, replayed its
     * villagers' restocks as they loaded), the core does at its looks with nothing loaded (B4.6;
     * LockerTradingTests.unloadedDaysAreReplayedOnLoad's twin): three and a half days with no count are six restocks of
     * the Librarian's locker, every other one inflated, so 48 trades of 24 paper, from a shulker box in the desk's
     * chest; the next look that counts the chest writes it. And what a look's lockers cost the server's tick, logged.
     */
    @GameTest(template = "floor")
    public static void aLockersMissedDaysAreTradedAtTheCoresLooksWithNothingLoaded(GameTestHelper helper) {
        var level = helper.getLevel();
        List<Villager> villagers = lockerMarket(helper);
        helper.setBlock(LOCKER, Blocks.CHEST);
        ChestBlockEntity stock = chestAt(helper, DESK.east());
        ItemStack box = new ItemStack(Items.SHULKER_BOX);
        box.set(net.minecraft.core.component.DataComponents.CONTAINER, net.minecraft.world.item.component.ItemContainerContents
                .fromItems(java.util.Collections.nCopies(27, new ItemStack(Items.PAPER, 64))));
        stock.setItem(0, box);
        BlockPos desk = helper.absolutePos(DESK);
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk, "Testford");
            host.hands().join(town, villagers.get(0));
            host.hands().join(town, villagers.get(1));
            var lockers = host.lockers();
            lockers.placeNow(level, town);
            var view = lockers.view(level, desk);
            helper.assertTrue(view.size() == 1, "no locker on the screen: " + view);
            String paper = view.get(0).trades().stream().filter(t -> t.costA().is(Items.PAPER)).findFirst()
                    .orElseThrow(() -> new AssertionError("no paper trade")).key();
            lockers.flip(paper, true);
            long t = level.getGameTime();
            lockerLook(host, level, t);
            helper.assertTrue(inChest(stock, Items.PAPER) == 27 * 64, "the box's paper moved: " + inChest(stock, Items.PAPER));
            Response r = host.apply(List.of(new Input.Advance(t + 3 * DAY + DAY / 2)));
            helper.assertTrue(r instanceof Response.Done, "the core refused the days: " + r);
            helper.assertTrue(inChest(stock, Items.PAPER) == 27 * 64, "a chest nobody counted was written");
            double left = held(host, town, "minecraft:paper");
            helper.assertTrue(left == 27 * 64 - 48 * 24, "48 trades of 24 paper weren't made unseen: the town holds " + left);
            lockerLook(host, level, t + 3 * DAY + DAY / 2);
            helper.assertTrue(inChest(stock, Items.PAPER) == left, "the chest shows " + inChest(stock, Items.PAPER)
                    + " paper, the town " + left);
            measureALocker(host, level, town);
        } finally {
            delete(dir);
            villagers.forEach(Villager::discard);
        }
        helper.succeed();
    }

    /**
     * What a look's lockers cost the server's tick, logged: where each loaded villager's locker stands, asked of the core
     * and found in the world (nothing to tell it: as at most looks), and a locker's chests counted.
     */
    private static void measureALocker(CoreHost host, net.minecraft.server.level.ServerLevel level, long town) {
        int n = 50;
        long a = System.nanoTime();
        for (int i = 0; i < n; i++) {
            nothingToTell(host.lockers().placeNow(level, town).isEmpty());
        }
        long placed = System.nanoTime() - a;
        GlobalPos at = host.points().of(PointsAdapter.Kind.LOCKER).keySet().iterator().next();
        a = System.nanoTime();
        for (int i = 0; i < n; i++) {
            host.stock().stocktake(level, PointsAdapter.Kind.LOCKER, at);
        }
        long counted = System.nanoTime() - a;
        Enterprise.LOGGER.info("lockers: a town's lockers placed (asked and looked for, nothing to tell) {} ms; a locker's"
                + " chests counted {} ms", "%.3f".formatted(placed / 1e6 / n), "%.3f".formatted(counted / 1e6 / n));
    }

    private static void nothingToTell(boolean none) {
        if (!none) {
            throw new AssertionError("a town's lockers changed with nothing moved in the world");
        }
    }

    // Two markets four blocks apart in one arena: a wagon's leg is 80 ticks, a trip three looks.
    private static final BlockPos HUB_DESK = new BlockPos(1, 2, 3);
    private static final BlockPos FAR_DESK = new BlockPos(5, 2, 3);

    /** The two markets' towns, the hub's Merchant and Librarian and the far end's Merchant, joined as their hands. */
    private record Wagons(long hub, long far, Villager merchant, Villager librarian, Villager farMerchant) {
        void discard() {
            merchant.discard();
            librarian.discard();
            farMerchant.discard();
        }
    }

    /**
     * A hub whose people are short of books, with bread, money, its Merchant and a trained Librarian to lead its
     * wagon; a far end with books, bread, money and its Merchant. Founded in the core and joined as its hands.
     */
    private static Wagons wagons(GameTestHelper helper, CoreHost host) {
        var level = helper.getLevel();
        helper.setBlock(HUB_DESK, Merchant.DESK.get());
        helper.setBlock(FAR_DESK, Merchant.DESK.get());
        Villager merchant = villager(helper, Merchant.PROFESSION.get(), 1, new BlockPos(1, 2, 1));
        Villager librarian = villager(helper, VillagerProfession.LIBRARIAN, 2, new BlockPos(2, 2, 5));
        Villager farMerchant = villager(helper, Merchant.PROFESSION.get(), 1, new BlockPos(5, 2, 5));
        long hub = host.stock().found(level, helper.absolutePos(HUB_DESK), "Hubford");
        long far = host.stock().found(level, helper.absolutePos(FAR_DESK), "Farley");
        host.hands().join(hub, merchant);
        host.hands().join(hub, librarian);
        host.hands().join(far, farMerchant);
        long bread = good(host, "minecraft:bread");
        Response r = host.apply(List.of(new Input.Deliver(hub, bread, 128), new Input.Deliver(far, bread, 128),
                new Input.Deliver(far, good(host, "minecraft:book"), 64), new Input.Pay(hub, 50_000), new Input.Pay(far, 50_000)));
        helper.assertTrue(r instanceof Response.Done, "the markets weren't stocked: " + r);
        return new Wagons(hub, far, merchant, librarian, farMerchant);
    }

    /**
     * What a wagon's crew costs the server's tick, logged: a villager stowed and let out by the desk (a {@code HandAway}
     * and a {@code HandBack} shown), the look's catch-up with nothing waiting, and the screen's question of a town's routes.
     */
    private static void measureAWagon(GameTestHelper helper, CoreHost host, long hub) {
        var server = helper.getLevel().getServer();
        int n = 20;
        long stowed = 0;
        long released = 0;
        for (int i = 0; i < n; i++) {
            Villager v = villager(helper, VillagerProfession.LIBRARIAN, 2, new BlockPos(3, 2, 1));
            String tag = v.getUUID().toString();
            long a = System.nanoTime();
            host.wagons().show(new Output.HandAway(hub, 1_000_000 + i, tag));
            long b = System.nanoTime();
            host.wagons().show(new Output.HandBack(hub, 1_000_000 + i, tag, "librarian", 2, 1));
            long c = System.nanoTime();
            stowed += b - a;
            released += c - b;
            if (helper.getLevel().getEntity(v.getUUID()) != null) {
                helper.getLevel().getEntity(v.getUUID()).discard();
            }
        }
        long a = System.nanoTime();
        for (int i = 0; i < 1000; i++) {
            host.wagons().catchUp(server);
        }
        long caught = System.nanoTime() - a;
        a = System.nanoTime();
        for (int i = 0; i < n; i++) {
            host.wagons().town(hub);
        }
        long asked = System.nanoTime() - a;
        Enterprise.LOGGER.info("wagons: a villager stowed {} ms, let out by the desk {} ms; a look's catch-up with nothing "
                + "waiting {} ms; a town's routes asked {} ms", "%.3f".formatted(stowed / 1e6 / n),
                "%.3f".formatted(released / 1e6 / n), "%.4f".formatted(caught / 1e6 / 1000), "%.3f".formatted(asked / 1e6 / n));
    }

    /** Looks one at a time from now until {@code until} holds (at most {@code looks}): whether it did. */
    private static boolean looksUntil(CoreHost host, net.minecraft.server.level.ServerLevel level, int looks,
            java.util.function.BooleanSupplier until) {
        long t = level.getGameTime();
        for (int i = 1; i <= looks && !until.getAsBoolean(); i++) {
            advance(host, t + i * 1200L);
        }
        return until.getAsBoolean();
    }

    private static boolean away(CoreHost host, long town, Villager v) {
        return host.hands().roster(town).hands().stream().anyMatch(h -> v.getUUID().equals(h.villager()) && h.away());
    }

    /**
     * A wagon's crew leaves the world with it and comes home by the desk (B4.8; the core-mode twin of
     * CaravanTests.theSquadLeavesTheWorldAndComesBack): a route started from the hub to the far end (the Merchant's
     * screen's "Send caravans", here the adapter's), the core sends a wagon shopping for the books the hub's people lack,
     * led by its Librarian; the core's {@code HandAway} has the villager stowed (off the world, kept by its hand's id), and
     * its {@code HandBack} three looks later lets it out by the hub's desk, the same villager, still a Librarian.
     */
    @GameTest(template = "floor")
    public static void aWagonsCrewLeavesTheWorldWithItAndComesHomeByTheDesk(GameTestHelper helper) {
        var level = helper.getLevel();
        Path dir = folder();
        Wagons w = null;
        try (CoreHost host = CoreHost.open(dir, 7)) {
            w = wagons(helper, host);
            var started = host.wagons().send(w.hub(), w.far(), true);
            helper.assertTrue(started.isPresent(), "no route from the hub to the far end: " + host.wagons().lines(w.hub()));
            java.util.UUID librarian = w.librarian().getUUID();
            Wagons m = w;
            helper.assertTrue(looksUntil(host, level, 3, () -> away(host, m.hub(), m.librarian())),
                    "no wagon left with the Librarian: " + host.wagons().lines(w.hub()));
            helper.assertTrue(level.getEntity(librarian) == null && w.librarian().isRemoved(),
                    "the Librarian is in the world while its wagon is on the road");
            helper.assertTrue(host.wagons().kept()[0] == 1, "the Librarian isn't kept while away");
            helper.assertTrue(host.wagons().town(w.hub()).routes().get(0).wagon()
                    .filter(x -> x.crew().size() == 1).isPresent(), "the route shows no crew: " + host.wagons().lines(w.hub()));
            helper.assertTrue(looksUntil(host, level, 4, () -> level.getEntity(librarian) != null),
                    "the Librarian isn't back: " + host.wagons().lines(w.hub()));
            helper.assertTrue(level.getEntity(librarian) instanceof Villager back
                            && back.getVillagerData().getProfession() == VillagerProfession.LIBRARIAN
                            && back.blockPosition().distManhattan(helper.absolutePos(HUB_DESK)) <= 6,
                    "home again, still a Librarian, by the desk: " + level.getEntity(librarian));
            helper.assertTrue(!away(host, w.hub(), (Villager) level.getEntity(librarian)) && host.wagons().kept()[0] == 0,
                    "the core still has it away");
            measureAWagon(helper, host, w.hub());
        } finally {
            delete(dir);
            if (w != null) {
                w.discard();
                if (level.getEntity(w.librarian().getUUID()) != null) {
                    level.getEntity(w.librarian().getUUID()).discard();
                }
            }
        }
        helper.succeed();
    }

    /**
     * A crew on the road is kept through a save (B4.8): the hub's Librarian away with its wagon when the world saves is
     * in the view's file beside the core's ({@value dev.b28.enterprise.binding.adapters.WagonAdapter#FILE}), and a host
     * loaded from them lets it out by the desk when the core brings its wagon home.
     */
    @GameTest(template = "floor")
    public static void aCrewOnTheRoadIsKeptThroughASaveAndLetOutWhenItsHome(GameTestHelper helper) {
        var level = helper.getLevel();
        Path dir = folder();
        Wagons w = null;
        try {
            java.util.UUID librarian;
            long hub;
            try (CoreHost host = CoreHost.open(dir, 7)) {
                w = wagons(helper, host);
                host.wagons().send(w.hub(), w.far(), true);
                Wagons m = w;
                helper.assertTrue(looksUntil(host, level, 3, () -> away(host, m.hub(), m.librarian())), "no wagon left");
                librarian = w.librarian().getUUID();
                hub = w.hub();
                helper.assertTrue(host.save(), "the core didn't save");
            }
            helper.assertTrue(Files.exists(dir.resolve(dev.b28.enterprise.binding.adapters.WagonAdapter.FILE)),
                    "no stowed villagers saved");
            try (CoreHost loaded = CoreHost.open(dir, 8)) {
                helper.assertTrue(loaded.wagons().kept()[0] == 1, "the loaded host doesn't keep the Librarian");
                helper.assertTrue(looksUntil(loaded, level, 4, () -> level.getEntity(librarian) != null),
                        "the Librarian isn't back after the load: " + loaded.wagons().lines(hub));
                helper.assertTrue(loaded.hands().of(librarian) != null && loaded.hands().of(librarian).town() == hub,
                        "the Librarian back isn't the hub's hand");
            }
        } finally {
            delete(dir);
            if (w != null) {
                w.discard();
                if (level.getEntity(w.librarian().getUUID()) != null) {
                    level.getEntity(w.librarian().getUUID()).discard();
                }
            }
        }
        helper.succeed();
    }

    /**
     * Caravans are commissioned at the desk through the core (B4.8; the core-mode twin of
     * CaravanTests.caravansAreCommissionedAtTheDesk): the screen shows the far end (a market the core says the hub
     * knows, within a leg's food) and the Librarian at home to send; "Send caravans" starts the core's route, "Stop"
     * winds it down, and a market the screen didn't show is refused.
     */
    @GameTest(template = "floor")
    public static void caravansAreCommissionedAtTheDeskThroughTheCore(GameTestHelper helper) {
        var level = helper.getLevel();
        Path dir = folder();
        Wagons w = null;
        try (CoreHost host = CoreHost.open(dir, 7)) {
            w = wagons(helper, host);
            BlockPos hub = helper.absolutePos(HUB_DESK);
            BlockPos far = helper.absolutePos(FAR_DESK);
            var view = host.wagons().caravans(level, hub);
            helper.assertTrue(view.towns().stream().anyMatch(t -> t.desk().equals(far) && t.inRange() && !t.routed()),
                    "the desk doesn't know its neighbour: " + view);
            helper.assertTrue(view.squad().equals(List.of("minecraft:librarian")), "a Librarian at home to send: " + view.squad());
            var wagons = host.wagons();
            long farTown = w.far();
            helper.assertTrue(wagons.commission(level, hub, far, true)
                    && wagons.town(w.hub()).routes().stream().anyMatch(l -> l.to() == farTown && l.open()), "Send caravans: no route");
            helper.assertTrue(wagons.caravans(level, hub).towns().stream().anyMatch(t -> t.desk().equals(far) && t.routed()),
                    "the screen doesn't show it sending");
            helper.assertTrue(wagons.commission(level, hub, far, false)
                    && wagons.town(w.hub()).routes().stream().noneMatch(l -> l.open()), "Stop caravans: still open");
            helper.assertFalse(wagons.commission(level, hub, far.offset(500, 0, 0), true), "only markets the screen showed");
        } finally {
            delete(dir);
            if (w != null) {
                w.discard();
            }
        }
        helper.succeed();
    }

    /**
     * A look of a test's own host as the server's would run it for the pallets and the chests (CoreHost.look without the
     * census, the land and the money, whose emeralds and coin would take the chests' slots): the pallets' crates seen to
     * (lost, carried, stale or put in), the points counted, time passed to {@code tick}, then what the crates came to;
     * its outputs shown.
     */
    private static Response palletLook(CoreHost host, net.minecraft.server.level.ServerLevel level, long tick) {
        var server = level.getServer();
        List<Input> pallets = host.pallets().happenings(server);
        List<Input> inputs = new java.util.ArrayList<>(host.stock().stocktakes(server));
        inputs.add(new Input.Advance(tick));
        inputs.addAll(pallets);
        Response r = host.apply(inputs);
        if (!(r instanceof Response.Done)) {
            throw new AssertionError("the core refused a look: " + r);
        }
        return r;
    }

    /** The packages of pallets in a chest, in slot order. */
    private static List<ItemStack> packages(ChestBlockEntity chest) {
        List<ItemStack> out = new java.util.ArrayList<>();
        for (int slot = 0; slot < chest.getContainerSize(); slot++) {
            if (dev.b28.enterprise.cargo.Cargo.isPackage(chest.getItem(slot))) {
                out.add(chest.getItem(slot));
            }
        }
        return out;
    }

    private static dev.b28.enterprise.cargo.PackageTag tag(ItemStack stack) {
        return stack.get(dev.b28.enterprise.cargo.Cargo.PACKAGE.get());
    }

    /** Takes the package at {@code place} of pallet {@code pallet} out of a chest. */
    private static ItemStack takePackage(ChestBlockEntity chest, long pallet, int place) {
        for (int slot = 0; slot < chest.getContainerSize(); slot++) {
            var t = tag(chest.getItem(slot));
            if (t != null && t.pallet() == pallet && t.place() == place) {
                return chest.removeItemNoUpdate(slot);
            }
        }
        throw new AssertionError("no package " + place + " of pallet " + pallet + " in the chest");
    }

    private static void putInEmptySlot(ChestBlockEntity chest, ItemStack stack) {
        for (int slot = 0; slot < chest.getContainerSize(); slot++) {
            if (chest.getItem(slot).isEmpty()) {
                chest.setItem(slot, stack);
                return;
            }
        }
        throw new AssertionError("a full chest");
    }

    /**
     * A pallet packed at a desk is tagged crates in its chests and goes with the wagon that fetches it (pallet-views; the
     * core-mode twin of CargoTests' crates with their papers, now the core's pallets): Far's Merchant packs 100 bread for
     * Hub (Input.Pack, a player's doing at the desk), and at the next look its crate is in Far's chest, tagged "1 of 1, to
     * Hubford", its bread inside and the bill of lading in its documents slot, the core showing it there; it isn't put in
     * twice. Hub's wagon, sent to Far, fetches it (sealed, the core moves it aboard: the crate leaves Far's chest), and
     * home again Hub receives it: the pallet is gone, Hub's chest shows its bread as the core has it.
     */
    @GameTest(template = "floor", timeoutTicks = 400)
    public static void aPalletPackedAtADeskIsTaggedCratesThatGoWithTheWagonThatFetchesIt(GameTestHelper helper) {
        var level = helper.getLevel();
        Path dir = folder();
        Wagons w = null;
        try (CoreHost host = CoreHost.open(dir, 7)) {
            w = wagons(helper, host);
            helper.setBlock(HUB_DESK.north(), Blocks.CHEST);
            helper.setBlock(FAR_DESK.north(), Blocks.CHEST);
            ChestBlockEntity hubChest = chestAt(helper, HUB_DESK.north());
            ChestBlockEntity farChest = chestAt(helper, FAR_DESK.north());
            long t = level.getGameTime();
            palletLook(host, level, t + 1);
            List<Long> ids = host.pallets().pack(w.far(), w.hub(), Map.of(Items.BREAD, 100L));
            helper.assertTrue(ids.size() == 1, "one pallet of one crate: " + ids);
            long id = ids.get(0);
            helper.assertTrue(packages(farChest).isEmpty(), "drawn at the next look, not at once");
            palletLook(host, level, t + 2);
            List<ItemStack> crates = packages(farChest);
            helper.assertTrue(crates.size() == 1, "Far's chest holds " + crates.size() + " crates");
            var tag = tag(crates.get(0));
            helper.assertTrue(tag.pallet() == id && tag.place() == 0 && tag.of() == 1 && tag.to().equals("Hubford")
                    && tag.contract() == -1 && crates.get(0).is(dev.b28.enterprise.cargo.Cargo.CRATE_ITEM.get()), "tagged " + tag);
            int bread = dev.b28.enterprise.cargo.Crates.goods(crates.get(0)).stream().filter(s -> s.is(Items.BREAD))
                    .mapToInt(ItemStack::getCount).sum();
            helper.assertTrue(bread == 100, "the crate holds " + bread + " bread");
            var bill = dev.b28.enterprise.cargo.Crates.papers(crates.get(0)).first(dev.b28.enterprise.cargo.Papers.BILL_OF_LADING);
            helper.assertTrue(bill.isPresent() && bill.get().data().getLong("pallet") == id
                    && dev.b28.enterprise.cargo.Crates.papers(crates.get(0)).consignee().isPresent(), "no bill of lading in its documents slot");
            helper.assertTrue(host.pallets().pallet(id).flatMap(v -> v.shown()).isPresent(), "the core doesn't show it at Far's desk");
            helper.assertTrue(inChest(farChest, Items.BREAD) == held(host, w.far(), "minecraft:bread"),
                    "Far's chest shows its stock's bread beside the crate");
            palletLook(host, level, t + 3);
            helper.assertTrue(packages(farChest).size() == 1, "put in twice");
            long received = host.pallets().town(w.hub()).received();
            host.wagons().send(w.hub(), w.far(), true);
            long at = t + 3;
            for (int i = 0; i < 6 && !packages(farChest).isEmpty(); i++) {
                palletLook(host, level, at += 1200);
            }
            helper.assertTrue(packages(farChest).isEmpty(), "the wagon never fetched the crate: " + host.pallets().lines(w.hub()));
            for (int i = 0; i < 6 && host.pallets().pallet(id).isPresent(); i++) {
                palletLook(host, level, at += 1200);
            }
            helper.assertTrue(host.pallets().pallet(id).isEmpty() && host.pallets().town(w.hub()).received() == received + 1,
                    "Hub never received it: " + host.pallets().lines(w.hub()));
            palletLook(host, level, at + 1200);
            helper.assertTrue(inChest(hubChest, Items.BREAD) == held(host, w.hub(), "minecraft:bread") && packages(hubChest).isEmpty(),
                    "Hub's chest shows " + inChest(hubChest, Items.BREAD) + " bread, the core " + held(host, w.hub(), "minecraft:bread"));
        } finally {
            delete(dir);
            if (w != null) {
                w.discard();
                if (level.getEntity(w.librarian().getUUID()) != null) {
                    level.getEntity(w.librarian().getUUID()).discard();
                }
            }
        }
        helper.succeed();
    }

    /**
     * What the pallets cost the server's tick at a look, logged: a look's pallet happenings with two desks loaded and a
     * pallet of three crates shown (its chests read, the crates' pallet asked, the desks' room offered: two crossings),
     * and a pallet of three crates drawn as items.
     */
    private static void measurePallets(CoreHost host, net.minecraft.server.level.ServerLevel level) {
        int n = 50;
        long a = System.nanoTime();
        for (int i = 0; i < n; i++) {
            if (!host.pallets().happenings(level.getServer()).isEmpty()) {
                throw new AssertionError("a look found something to say with nothing moved");
            }
        }
        long looked = System.nanoTime() - a;
        var v = host.pallets().town(0).held().get(0);
        a = System.nanoTime();
        for (int i = 0; i < n; i++) {
            host.pallets().items(v);
        }
        long drawn = System.nanoTime() - a;
        Enterprise.LOGGER.info("pallets: a look's happenings (two desks, a pallet of three crates shown) {} ms; a pallet of "
                + "three crates drawn {} ms", "%.3f".formatted(looked / 1e6 / n), "%.3f".formatted(drawn / 1e6 / n));
    }

    /** Two towns in one arena: Ashby at {@link #DESK} with a chest east of it, Bexley at (1, 2, 1) with one south of it. */
    private record TwoTowns(long a, long b, ChestBlockEntity aChest, ChestBlockEntity bChest) {}

    private static final BlockPos B_DESK = new BlockPos(1, 2, 1);

    /**
     * Ashby and Bexley, money in Ashby for crates, 1,200 wheat delivered to it, pallets of up to 27 stacks (so one holds
     * three crates), and Ashby's Merchant packing the wheat for Bexley: one pallet of three crates (576, 576, 48).
     */
    private static TwoTowns twoTowns(GameTestHelper helper, CoreHost host) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        helper.setBlock(B_DESK, Merchant.DESK.get());
        helper.setBlock(B_DESK.south(), Blocks.CHEST);
        long a = host.stock().found(level, helper.absolutePos(DESK), "Ashby");
        long b = host.stock().found(level, helper.absolutePos(B_DESK), "Bexley");
        Response r = host.apply(List.of(new Input.SetDial("pallet_slots", java.util.Optional.of(27.0)),
                new Input.SetDial("pallet_weight", java.util.Optional.of(27.0)), new Input.Pay(a, 20_000),
                new Input.Deliver(a, good(host, "minecraft:wheat"), 1200)));
        helper.assertTrue(r instanceof Response.Done, "the towns weren't set up: " + r);
        return new TwoTowns(a, b, chestAt(helper, DESK.east()), chestAt(helper, B_DESK.south()));
    }

    /**
     * A chest takes a pallet whole or not at all (pallet-views; the race the pallets plan ends): Ashby's pallet of three
     * crates waits while its desk's chest has two slots free (the core, asked with the free slots, offers none of it: no
     * crate of it goes in), and the look after a third slot is cleared all three go in, tagged 1, 2 and 3 of 3.
     */
    @GameTest(template = "floor")
    public static void aDesksChestTakesAPalletWholeOrNotAtAll(GameTestHelper helper) {
        var level = helper.getLevel();
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            TwoTowns w = twoTowns(helper, host);
            for (int slot = 0; slot < 25; slot++) {
                w.aChest().setItem(slot, new ItemStack(Items.DIRT, 64));
            }
            List<Long> ids = host.pallets().pack(w.a(), w.b(), Map.of(Items.WHEAT, 1200L));
            helper.assertTrue(ids.size() == 1 && host.pallets().pallet(ids.get(0)).get().items() == 3,
                    "one pallet of three crates: " + host.pallets().lines(w.a()));
            long id = ids.get(0);
            long t = level.getGameTime();
            palletLook(host, level, t + 1);
            helper.assertTrue(packages(w.aChest()).isEmpty(), "part of the pallet went in: " + packages(w.aChest()).size());
            helper.assertTrue(host.pallets().pallet(id).get().shown().isEmpty(), "the core shows it with no room for it");
            w.aChest().setItem(24, ItemStack.EMPTY);
            palletLook(host, level, t + 2);
            List<ItemStack> crates = packages(w.aChest());
            helper.assertTrue(crates.size() == 3, "the chest holds " + crates.size() + " crates of the pallet, not 3");
            for (int place = 0; place < 3; place++) {
                int p = place;
                helper.assertTrue(crates.stream().anyMatch(c -> tag(c).place() == p && tag(c).of() == 3 && tag(c).pallet() == id
                        && tag(c).to().equals("Bexley")), "no crate " + (place + 1) + " of 3");
            }
            helper.assertTrue(host.pallets().pallet(id).get().shown().isPresent(), "the core doesn't show it in the chest");
            measurePallets(host, level);
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * A missing crate is the core's to judge (pallet-views; pallet-messages' short pallet in the world): of Ashby's three
     * crates for Bexley, a player burns one (its item destroyed: PackageLost, the core books its 576 wheat lost and the
     * pallet goes on short) and carries another to Bexley's desk (PackageBrought); the third never comes. At Bexley's next
     * look the core receives the pallet short: Bexley takes the 576 that came, into its chest as wheat, and the 48 left
     * behind are lost too (the seal broken), the crates gone from both chests.
     */
    @GameTest(template = "floor")
    public static void aMissingCrateBooksALossInTheCore(GameTestHelper helper) {
        var level = helper.getLevel();
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            TwoTowns w = twoTowns(helper, host);
            long id = host.pallets().pack(w.a(), w.b(), Map.of(Items.WHEAT, 1200L)).get(0);
            long t = level.getGameTime();
            palletLook(host, level, t + 1);
            helper.assertTrue(packages(w.aChest()).size() == 3, "the pallet's crates aren't in Ashby's chest");
            // A player burns the second crate: its item destroyed, as PalletAdapter.onLeave hears it (and hands it to the
            // server's host; this test's own is told here).
            ItemStack burnt = takePackage(w.aChest(), id, 1);
            host.pallets().destroyed(burnt);
            // And carries the first to Bexley's desk.
            putInEmptySlot(w.bChest(), takePackage(w.aChest(), id, 0));
            palletLook(host, level, t + 2);
            var pallets = host.pallets().town(w.b());
            long wheat = good(host, "minecraft:wheat");
            helper.assertTrue(pallets.lostGoods().equals(List.of(new dev.b28.enterprise.wire.Count(wheat, 576.0))),
                    "the burnt crate isn't booked lost: " + pallets.lostGoods());
            var v = host.pallets().pallet(id).orElseThrow();
            helper.assertTrue(v.items() == 2 && !v.sealed() && v.arriving().equals(List.of(0L)),
                    "the pallet short, its first crate arriving at Bexley: " + host.pallets().describe(v));
            helper.assertTrue(tag(packages(w.aChest()).get(0)).place() == 1 && tag(packages(w.aChest()).get(0)).of() == 2,
                    "the crate left at Ashby isn't retagged 2 of 2");
            palletLook(host, level, t + 1202);
            helper.assertTrue(host.pallets().pallet(id).isEmpty(), "Bexley didn't receive it short at its next look");
            pallets = host.pallets().town(w.b());
            helper.assertTrue(pallets.lostGoods().equals(List.of(new dev.b28.enterprise.wire.Count(wheat, 624.0))),
                    "the crate that never came isn't booked lost: " + pallets.lostGoods());
            helper.assertTrue(held(host, w.b(), "minecraft:wheat") == 576, "Bexley holds " + held(host, w.b(), "minecraft:wheat"));
            helper.assertTrue(inChest(w.bChest(), Items.WHEAT) == 576 && packages(w.bChest()).isEmpty() && packages(w.aChest()).isEmpty(),
                    "the chests show " + inChest(w.bChest(), Items.WHEAT) + " wheat at Bexley, crates " + packages(w.bChest()).size()
                    + " and " + packages(w.aChest()).size());
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * The commands' books are the core's (B4.11): a town's cash book, books, reasons, trust and contracts come from
     * {@code Query.CashBook}, {@code Books}, {@code Whys}, {@code Trust} and {@code Contracts}, set out in the lines the
     * commands and the studies read. A town that's paid in has money in its cash book; nobody has dealt, so no trust
     * record and no contract.
     */
    @GameTest(template = "floor")
    public static void aTownsBooksAreTheCoresForTheCommands(GameTestHelper helper) {
        Path dir = folder();
        Wagons w = null;
        try (CoreHost host = CoreHost.open(dir, 7)) {
            w = wagons(helper, host);
            var books = host.books();
            var cash = books.cashBook(w.hub(), 10);
            helper.assertTrue(cash.get(0).equals("Cash book of Hubford:") && cash.stream().anyMatch(l -> l.contains("paid in")),
                    "the cash book doesn't show the money paid in: " + cash);
            helper.assertTrue(books.books(w.hub()).get(0).startsWith("Books of Hubford (kept "), "the books: " + books.books(w.hub()));
            helper.assertTrue(books.whys(w.hub()).get(0).startsWith("Reasons of Hubford ("), "the reasons: " + books.whys(w.hub()));
            helper.assertTrue(books.trust().equals(List.of("0 record(s)")), "trust: " + books.trust());
            helper.assertTrue(books.contracts(null).equals(List.of("0 contract(s)", "0 rent(s)")), "contracts: " + books.contracts(null));
            helper.assertTrue(books.contracts(w.far()).equals(List.of("0 contract(s)", "0 rent(s)")),
                    "a town's contracts: " + books.contracts(w.far()));
            // The cash book's money in and out are the core's sums, not the adapter's.
            Answer.CashBook book = (Answer.CashBook) host.ask(List.of(new Query.CashBook(w.hub(), 10))).get(0);
            helper.assertTrue(book.received() - book.paid() == book.lines().stream().mapToLong(l -> l.amount()).sum(),
                    "the cash book's totals aren't its lines': " + book);
        } finally {
            delete(dir);
            if (w != null) {
                w.discard();
            }
        }
        helper.succeed();
    }

    /**
     * The commands that answer for the studies in core mode ({@code /enterprise mint}, {@code territory}, {@code war},
     * {@code routes}, {@code atlas}, {@code name}, {@code depot}, {@code autoswitch}) are the core's answers, in words the
     * scripts' regexes ({@code dev/experiments/network_study.py}) read: each pattern the study reads from a town's territory
     * is found in the core's lines, and the war's line, the mint's and a route's totals match the study's too.
     */
    @GameTest(template = "floor")
    public static void aTownsTerritoryAndTheWarAreTheCoresForTheStudies(GameTestHelper helper) {
        Path dir = folder();
        Wagons w = null;
        try (CoreHost host = CoreHost.open(dir, 7)) {
            w = wagons(helper, host);
            var commands = host.commands();
            helper.assertTrue(java.util.regex.Pattern.compile("([\\d.]+) spurs per emerald").matcher(commands.mint().get(0)).find(),
                    "the mint: " + commands.mint());
            String territory = String.join("\n", commands.territory(w.hub()));
            helper.assertTrue(territory.startsWith("Settlement at Hubford holds "), "territory: " + territory);
            for (String pattern : List.of("treasury ([\\d.]+)", "holds (\\d+) hex", "(\\d+) guard\\(s\\) posted", "posted at \\[(.*?)\\]",
                    "toll ([\\d.]+)%", "\\((\\d+) spurs collected last review\\)", "traffic (\\d+) spurs a day", "(\\d+) deserted",
                    "\\): (\\d+) villagers", "villagers, (\\d+) beds", "(\\d+) home\\(s\\) built", "(\\d+) child\\(ren\\) born",
                    "culture ([\\d.]+) of", "mood ([+-][\\d.]+)", "locker trades: (\\d+)")) {
                helper.assertTrue(java.util.regex.Pattern.compile(pattern).matcher(territory).find(),
                        "the study's " + pattern + " isn't in the core's territory:\n" + territory);
            }
            String war = commands.war();
            for (String pattern : List.of("treasury (\\d+) spurs", "minted (\\d+)", "contracts (\\d+)", "delivered (\\d+)",
                    "army (\\d+) serving", "(\\d+) fallen", "bills ([\\d.]+)")) {
                helper.assertTrue(java.util.regex.Pattern.compile(pattern).matcher(war).find(),
                        "the study's " + pattern + " isn't in the core's war line: " + war);
            }
            helper.assertTrue(commands.warTally().get(0).startsWith("War tally, against "), "the tally: " + commands.warTally());
            var made = host.wagons().addRoute(w.hub(), w.far());
            helper.assertTrue(made.getKey() >= 0, "no route opened: " + made.getValue());
            var route = commands.routes().get(1);
            helper.assertTrue(java.util.regex.Pattern.compile("#(\\d+) ([-\\d, ]+) -> ([-\\d, ]+): (\\d+) blocks, (\\d+) trips, "
                    + "(\\d+) items sold for (\\d+) spurs, (\\d+) for services, (\\d+) bought for (\\d+), (\\d+) spurs on road food, "
                    + "(\\d+) tolls paid, (\\d+) spurs brought home").matcher(route).find(), "the route's line: " + route);
            helper.assertTrue(commands.atlas(w.hub()).get(0).startsWith("The desk of Hubford knows "), "atlas: " + commands.atlas(w.hub()));
            helper.assertTrue(commands.names().stream().anyMatch(l -> l.startsWith("  Hubford at ")), "names: " + commands.names());
            helper.assertTrue(commands.depots().get(0).endsWith("building(s)"), "depots: " + commands.depots());
            // Autoswitch turns on what earns emeralds at the lockers with nothing on yet, through the core at once: asked
            // again, every locker has something on and nothing more is switched.
            int switched = commands.autoswitch(w.hub(), false);
            helper.assertTrue(switched > 0 && commands.autoswitch(w.hub(), false) == 0,
                    "autoswitch switched " + switched + " offer(s), then switched more: " + host.lockers().town(w.hub()).lockers());
            // An offer off at a locker with another on (as one a villager unlocks by levelling up): only `unlocked` takes it.
            var both = host.lockers().town(w.hub()).lockers().stream()
                    .filter(l -> l.offers().stream().filter(o -> o.on() && !o.sells()).count() >= 2).findFirst();
            helper.assertTrue(both.isPresent(), "no locker with two offers on: " + host.lockers().town(w.hub()).lockers());
            {
                var off = both.get().offers().stream().filter(o -> o.on() && !o.sells()).findFirst().orElseThrow();
                helper.assertTrue(host.lockers().flip(dev.b28.enterprise.binding.adapters.LockerAdapter.key(both.get().hand(), off), false), "switched off");
                helper.assertTrue(commands.autoswitch(w.hub(), false) == 0, "the locker has another on: autoswitch leaves it");
                helper.assertTrue(commands.autoswitch(w.hub(), false, true) == 1, "unlocked switches it back on");
            }
            // A desk the core keeps no town at has no town near it.
            helper.assertTrue(commands.townNear(GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(new BlockPos(40, 2, 40))), 8) == null,
                    "a town near nothing");
        } finally {
            delete(dir);
            if (w != null) {
                w.discard();
            }
        }
        helper.succeed();
    }

    // ---- shops (B4.9) -----------------------------------------------------------------------------------------------

    private static final net.minecraft.world.level.block.Block VENDOR = net.minecraft.core.registries.BuiltInRegistries.BLOCK
            .get(net.minecraft.resources.ResourceLocation.parse("numismatics:vendor"));

    /** A vendor at {@code at}, buying or selling lots of {@code lot} for {@code spurs}. */
    private static dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity vendor(GameTestHelper helper, BlockPos at,
            dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity.Mode mode, ItemStack lot, int spurs) {
        helper.setBlock(at, VENDOR);
        var vendor = (dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity) helper.getLevel()
                .getBlockEntity(helper.absolutePos(at));
        vendor.setMode(mode);
        vendor.filterContainer.setItem(0, lot);
        dev.b28.enterprise.market.Shops.setPrice(vendor, spurs);
        return vendor;
    }

    /** A bank card in the vendor's card slot, its account holding {@code spurs}. */
    private static dev.ithundxr.createnumismatics.content.backend.BankAccount card(
            dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity vendor, int spurs) {
        var account = dev.ithundxr.createnumismatics.Numismatics.BANK.getOrCreateAccount(java.util.UUID.randomUUID(),
                dev.ithundxr.createnumismatics.content.backend.BankAccount.Type.PLAYER);
        account.deposit(spurs);
        ItemStack card = new ItemStack(net.minecraft.core.registries.BuiltInRegistries.ITEM
                .get(net.minecraft.resources.ResourceLocation.parse("numismatics:white_card")));
        dev.ithundxr.createnumismatics.content.bank.CardItem.set(card, account.id);
        vendor.cardContainer.setItem(0, card);
        return account;
    }

    private static int inVendor(dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity vendor, net.minecraft.world.item.Item item) {
        return vendor.items.stream().filter(i -> i.is(item)).mapToInt(ItemStack::getCount).sum();
    }

    /** The shops the core has for a town, by the vendor's place. */
    private static Answer.Shops shopsOf(CoreHost host, long town) {
        return (Answer.Shops) host.ask(List.of(new Query.Shops(town))).get(0);
    }

    /**
     * A player's buying vendor is a standing order the core's master Merchant fills (B4.9; MarketTests.aMasterMerchantFillsAPlayersOrder's
     * twin): a vendor buying 16 paper for 16 spurs with 40 spurs on its card, by a desk whose chest holds 64 paper. The look
     * that counts it drafts the contract and fills the first lot (the vendor holds 16 paper, its card 24, the desk's chest 48),
     * the next day's look the second (32 paper, 8 spurs, 32 in the chest). A vendor with no room for a lot is handed none; one
     * whose block is broken is gone from the core's shops.
     */
    @GameTest(template = "floor")
    public static void aBuyingVendorIsAStandingOrderTheCoreFillsAndTheVendorIsHandedTheGoods(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        ChestBlockEntity chest = chestAt(helper, DESK.east());
        chest.setItem(0, new ItemStack(Items.PAPER, 64));
        Villager merchant = villager(helper, Merchant.PROFESSION.get(), 5, new BlockPos(3, 2, 5));
        var order = vendor(helper, new BlockPos(6, 2, 6), dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity.Mode.BUY,
                new ItemStack(Items.PAPER, 16), 16);
        var account = card(order, 40);
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, helper.absolutePos(DESK), "Testford");
            host.hands().join(town, merchant);
            long t = level.getGameTime();
            host.look(level.getServer(), t + 1_200);
            host.settle();
            helper.assertTrue(host.points().of(PointsAdapter.Kind.SHOP).containsKey(GlobalPos.of(level.dimension(), helper.absolutePos(new BlockPos(6, 2, 6)))),
                    "the vendor isn't a shop point of the core's");
            helper.assertTrue(inVendor(order, Items.PAPER) == 16 && account.getBalance() == 24 && inChest(chest, Items.PAPER) == 48,
                    "the first lot: " + inVendor(order, Items.PAPER) + " in the vendor, " + account.getBalance() + " on its card, "
                            + inChest(chest, Items.PAPER) + " in the chest");
            var line = shopsOf(host, town).shops().get(0);
            helper.assertTrue(line.role() == dev.b28.enterprise.wire.ShopRole.Order && line.delivered() == 16 && line.paid() == 16,
                    "the core's order: " + line);
            host.look(level.getServer(), t + 1_200 + DAY);
            host.settle();
            helper.assertTrue(inVendor(order, Items.PAPER) == 32 && account.getBalance() == 8 && inChest(chest, Items.PAPER) == 32,
                    "the second lot tomorrow: " + inVendor(order, Items.PAPER) + " in the vendor, " + account.getBalance()
                            + " on its card, " + inChest(chest, Items.PAPER) + " in the chest");
            helper.setBlock(new BlockPos(6, 2, 6), Blocks.AIR);
            host.look(level.getServer(), t + 1_200 + DAY + 1_200);
            helper.assertTrue(shopsOf(host, town).shops().isEmpty() && host.points().of(PointsAdapter.Kind.SHOP).isEmpty(),
                    "its block broken, the core still has a shop: " + shopsOf(host, town).shops());
            if (System.getenv("ENTERPRISE_CORE_MEASURE") != null) {
                helper.setBlock(new BlockPos(6, 2, 6), VENDOR);
                host.look(level.getServer(), t + 1_200 + DAY + 2_400);
                host.settle();
                measureAShop(host, level.getServer(), town);
            }
        } finally {
            delete(dir);
            merchant.discard();
        }
        helper.succeed();
    }

    /**
     * A player's selling vendor is supply (B4.9; MarketTests.itBuysFromAPlayersStallForItsOrders' twin): a stall of 8 iron in
     * lots of 4 for 12 spurs, beside an order for iron the desk's treasury can pay for. The look takes the stall's two lots
     * out of the vendor and pays 24 spurs into its box; the iron is the town's, in the desk's chest.
     */
    @GameTest(template = "floor")
    public static void aSellingVendorIsBoughtFromForTheOrdersAndPaidIntoItsBox(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        ChestBlockEntity chest = chestAt(helper, DESK.east());
        chest.setItem(0, new ItemStack(Items.EMERALD, 10));
        Villager merchant = villager(helper, Merchant.PROFESSION.get(), 5, new BlockPos(3, 2, 5));
        var order = vendor(helper, new BlockPos(6, 2, 6), dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity.Mode.BUY,
                new ItemStack(Items.IRON_INGOT, 4), 24);
        card(order, 48);
        var stall = vendor(helper, new BlockPos(6, 2, 1), dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity.Mode.SELL,
                new ItemStack(Items.IRON_INGOT, 4), 12);
        dev.b28.enterprise.market.Shops.stock(stall, new ItemStack(Items.IRON_INGOT, 8));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, helper.absolutePos(DESK), "Testford");
            host.hands().join(town, merchant);
            long t = level.getGameTime();
            host.look(level.getServer(), t + 1_200);
            host.settle();
            helper.assertTrue(dev.b28.enterprise.market.Shops.lots(stall) == 0 && inVendor(stall, Items.IRON_INGOT) == 0,
                    "it buys the stall's two lots for the order: " + inVendor(stall, Items.IRON_INGOT) + " left");
            int spurs = dev.b28.enterprise.market.Shops.box(stall);
            helper.assertTrue(spurs == 24, "and pays 24 spurs into its box: " + spurs);
            host.look(level.getServer(), t + 1_200 + DAY);
            host.settle();
            helper.assertTrue(inVendor(order, Items.IRON_INGOT) >= 4, "and fills the order, a lot a day: " + inVendor(order, Items.IRON_INGOT));
            var tally = shopsOf(host, town).tally();
            helper.assertTrue(tally.spursFromSales() - tally.spursOnPurchases() >= 24 - 1e-9 && tally.lotsBought() == 2,
                    "at a profit: " + tally);
        } finally {
            delete(dir);
            merchant.discard();
        }
        helper.succeed();
    }

    /**
     * The town's own shop (B4.9; MarketTests.itsOwnShopSellsTheSurplus' twin): a vendor touching the desk is shown the
     * town's sticks (the surplus beside a master Merchant), a stack of them from the desk's chest. A player bought a lot
     * (16 sticks gone, nine spurs in its box): the next look takes the coin out of the box and the town banks it.
     */
    @GameTest(template = "floor")
    public static void theTownsOwnShopIsShownItsSurplusAndItsTakingsAreBanked(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        ChestBlockEntity chest = chestAt(helper, DESK.east());
        chest.setItem(0, new ItemStack(Items.STICK, 64));
        Villager merchant = villager(helper, Merchant.PROFESSION.get(), 5, new BlockPos(3, 2, 5));
        helper.setBlock(DESK.north(), VENDOR);
        var own = (dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity) level.getBlockEntity(helper.absolutePos(DESK.north()));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, helper.absolutePos(DESK), "Testford");
            host.hands().join(town, merchant);
            long t = level.getGameTime();
            host.look(level.getServer(), t + 1_200);
            host.settle();
            helper.assertTrue(own.getMode() == dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity.Mode.SELL
                            && own.getFilterItem().is(Items.STICK) && inVendor(own, Items.STICK) == 64 && inChest(chest, Items.STICK) == 0,
                    "the town's shop sells its sticks: " + own.getFilterItem() + ", " + inVendor(own, Items.STICK) + " on the shelf, "
                            + inChest(chest, Items.STICK) + " in the chest");
            helper.assertTrue(own.getTotalPrice() == 9 && own.getFilterItem().getCount() == 16, "in lots of 16 at nine spurs");
            own.addCoin(dev.ithundxr.createnumismatics.content.backend.Coin.SPUR, 9); // a player bought a lot
            for (int i = 0; i < own.items.size() && i < 1; i++) {
                own.items.get(i).shrink(16);
            }
            host.look(level.getServer(), t + 2_400);
            host.settle();
            int afterTwo = dev.b28.enterprise.market.Shops.box(own);
            host.look(level.getServer(), t + 3_600);
            host.settle();
            var tally = shopsOf(host, town).tally();
            helper.assertTrue(afterTwo == 0 && dev.b28.enterprise.market.Shops.box(own) == 0 && tally.spursFromOwnShops() == 9,
                    "the box was to be emptied into the town's and banked once: " + afterTwo + " spurs left after the look, "
                            + dev.b28.enterprise.market.Shops.box(own) + " after the next, banked " + tally.spursFromOwnShops() + ", the vendor "
                            + own.getMode() + " " + own.getFilterItem() + " at " + own.getTotalPrice());
        } finally {
            delete(dir);
            merchant.discard();
        }
        helper.succeed();
    }

    /**
     * The board's vendor (B4.9; MoodTests.theShopListsWhatTheTownIsAfterAtWhatItWouldPay's twin, in part): a town the core
     * asks a vendor for (`BuildShop`) has one put up touching its desk at the next look, on open floor, registered as the
     * board's, and the vendor stands.
     */
    @GameTest(template = "floor")
    public static void aVendorTheCoreAsksForIsPutUpByTheDeskAsTheBoards(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, helper.absolutePos(DESK), "Testford");
            host.shops().show(new Output.BuildShop(town));
            long t = level.getGameTime();
            host.look(level.getServer(), t + 1_200);
            host.settle();
            var shops = shopsOf(host, town).shops();
            helper.assertTrue(shops.size() == 1 && shops.get(0).role() == dev.b28.enterprise.wire.ShopRole.Board,
                    "the core has no board vendor: " + shops);
            var p = shops.get(0).point();
            BlockPos at = new BlockPos(p.x(), p.y(), p.z());
            helper.assertTrue(level.getBlockEntity(at) instanceof dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity v
                    && dev.b28.enterprise.market.Shops.own(v, helper.absolutePos(DESK)), "no vendor touches the desk");
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * A rent's deed shop (B4.9; MarketTests.aPlayerBuysARentsDeedCollectsItsCropAndSellsItBack's twin, the vendor's half: the
     * core's rents are made by its rules, so the outputs are the ones its tests show). Told to sell, the vendor offers the
     * deed document at the ask with one on its shelf; told to buy it back, it takes the dealer's red card, funded, at the bid; a
     * player given the crop gets it in the inventory.
     */
    @GameTest(template = "floor")
    public static void aDeedShopSellsTheDeedAtTheAskBuysItBackAtTheBidAndTheCropIsHandedOver(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(new BlockPos(6, 2, 6), VENDOR);
        var shop = (dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity) level.getBlockEntity(helper.absolutePos(new BlockPos(6, 2, 6)));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            var point = PointsAdapter.point(PointsAdapter.Kind.SHOP, GlobalPos.of(level.dimension(), helper.absolutePos(new BlockPos(6, 2, 6))));
            var line = new dev.b28.enterprise.wire.DeedLine(3, 1, "Farley", good(host, "minecraft:wheat"), 64.0, 1_290, 1_056, false, 0.0,
                    java.util.Optional.of(point));
            host.shops().showDeed(shop, new Output.Deed(point, 3, true, 1_290, 0), line);
            ItemStack deed = dev.b28.enterprise.market.RentDeeds.deed(3, "64 wheat a season from Farley");
            helper.assertTrue(shop.getMode() == dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity.Mode.SELL
                            && shop.getTotalPrice() == 1_290 && dev.b28.enterprise.market.Shops.lots(shop) == 1 && shop.matchesFilterItem(deed),
                    "the deed on sale at the ask: " + shop.getTotalPrice() + ", " + dev.b28.enterprise.market.Shops.lots(shop) + " lots");
            ItemStack bought = dev.b28.enterprise.market.Shops.buyFrom(shop); // a player buys it
            helper.assertTrue(dev.b28.enterprise.market.RentDeeds.rentOf(bought).orElse(-1) == 3, "the deed names the rent");
            host.shops().showDeed(shop, new Output.Deed(point, 3, false, 1_056, 1_100), line);
            helper.assertTrue(shop.getMode() == dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity.Mode.BUY
                            && shop.getTotalPrice() == 1_056 && dev.b28.enterprise.market.Shops.purse(shop).map(a -> a.getBalance()).orElse(0) >= 1_056,
                    "bought back at the bid from a funded card: " + shop.getTotalPrice());
            int paid = dev.b28.enterprise.market.Shops.sellTo(shop, bought); // the player sells it back
            helper.assertTrue(paid == 1_056, "the vendor paid for it from its card: " + paid);
            var player = helper.makeMockPlayer(net.minecraft.world.level.GameType.SURVIVAL);
            int n = host.shops().handOver(player, new Output.DeedPaid(0, 3, good(host, "minecraft:wheat"), 40, 24));
            helper.assertTrue(n == 40 && player.getInventory().countItem(Items.WHEAT) == 40, "the crop is handed over: " + n);
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /** What the shops cost the server's tick, logged: the vendors near a desk registered, counted, and a town's shops asked. */
    private static void measureAShop(CoreHost host, net.minecraft.server.MinecraftServer server, long town) {
        int n = 50;
        long a = System.nanoTime();
        for (int i = 0; i < n; i++) {
            host.shops().place(server);
        }
        long placed = System.nanoTime() - a;
        a = System.nanoTime();
        int read = 0;
        for (int i = 0; i < n; i++) {
            read = host.shops().happenings(server).size();
        }
        long counted = System.nanoTime() - a;
        a = System.nanoTime();
        for (int i = 0; i < n; i++) {
            host.shops().lines(town);
        }
        long asked = System.nanoTime() - a;
        Enterprise.LOGGER.info("shops: a look's vendors registered (the desks' reach scanned, the deeds asked) {} ms; {} vendor(s)"
                + " counted {} ms; a town's shops asked {} ms", "%.3f".formatted(placed / 1e6 / n), read, "%.3f".formatted(counted / 1e6 / n),
                "%.3f".formatted(asked / 1e6 / n));
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
     * A storehouse's floorplan keeps its containers the town's depots (B4.3): a chest broken is gone from the town at the
     * next look, put back it's a depot again, and a chest added to the building since joins it, each counted as it joins.
     * The floorplan is the building's record ({@code merchant/Depots}); the core's depot points follow it.
     */
    @GameTest(template = "floor")
    public static void aChestPutBackInAStorehouseIsItsTownsDepotAgain(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos at = new BlockPos(1, 2, 1);
        BlockPos first = new BlockPos(5, 2, 5);
        BlockPos second = new BlockPos(5, 2, 3);
        helper.setBlock(at, Merchant.DESK.get());
        helper.setBlock(first, Blocks.CHEST);
        chestAt(helper, first).setItem(0, new ItemStack(Items.WHEAT, 10));
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(at));
        var buildings = dev.b28.enterprise.merchant.Depots.get(level.getServer());
        dev.b28.enterprise.merchant.Depots.Depot building = null;
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk.pos(), "Testford");
            building = buildings.add(level, desk, dev.b28.enterprise.merchant.Depots.Kind.STOREHOUSE,
                    helper.absolutePos(new BlockPos(4, 2, 3)), helper.absolutePos(new BlockPos(6, 3, 5)));
            long t = level.getGameTime();
            host.look(level.getServer(), t + 1);
            host.settle();
            helper.assertTrue(host.stock().depots().size() == 1 && held(host, town, "minecraft:wheat") == 10,
                    "the storehouse's chest isn't the town's depot: " + host.stock().depots() + ", " + host.stock().stockAt(desk));
            chestAt(helper, first).clearContent();
            helper.setBlock(first, Blocks.AIR);
            host.look(level.getServer(), t + 2);
            host.settle();
            helper.assertTrue(host.stock().depots().isEmpty() && held(host, town, "minecraft:wheat") == 0,
                    "the chest broken is still the town's: " + host.stock().depots() + ", " + host.stock().stockAt(desk));
            helper.setBlock(first, Blocks.CHEST);
            chestAt(helper, first).setItem(0, new ItemStack(Items.WHEAT, 7));
            helper.setBlock(second, Blocks.CHEST);
            chestAt(helper, second).setItem(0, new ItemStack(Items.WHEAT, 3));
            host.look(level.getServer(), t + 3);
            host.settle();
            helper.assertTrue(host.stock().depots().size() == 2 && held(host, town, "minecraft:wheat") == 10,
                    "the chest put back and the one added aren't the town's depots: " + host.stock().depots() + ", "
                            + host.stock().stockAt(desk));
            host.look(level.getServer(), t + 4);
            host.settle();
            helper.assertTrue(host.stock().depots().size() == 2 && held(host, town, "minecraft:wheat") == 10,
                    "a depot joined twice: " + host.stock().stockAt(desk));
        } finally {
            if (building != null) {
                buildings.remove(building.id());
            }
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

    // ------------------------------------------------------------------------------------------------ money (B4.4)

    private static final BlockPos BANKER = new BlockPos(3, 2, 4);
    private static final String COMMONS = dev.b28.enterprise.money.Mint.DEFAULT_FACTION;

    /** A desk with a chest and nothing in it. */
    private static ChestBlockEntity emptyDesk(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        return chestAt(helper, DESK.east());
    }

    /** Spurs of Numismatics coin loose in a container, and the stacks they take. */
    private static int coinValue(ChestBlockEntity chest) {
        List<ItemStack> stacks = new java.util.ArrayList<>();
        for (int slot = 0; slot < chest.getContainerSize(); slot++) {
            stacks.add(chest.getItem(slot));
        }
        return dev.b28.enterprise.money.MoneyChanger.value(stacks);
    }

    private static long coinStacks(ChestBlockEntity chest) {
        long n = 0;
        for (int slot = 0; slot < chest.getContainerSize(); slot++) {
            if (chest.getItem(slot).getItem() instanceof dev.ithundxr.createnumismatics.content.coins.CoinItem) {
                n++;
            }
        }
        return n;
    }

    /** {@code spurs} of coin put in the chest as the fewest coins, from {@code slot} on. */
    private static void giveCoins(ChestBlockEntity chest, int spurs, int slot) {
        for (ItemStack stack : dev.b28.enterprise.money.MoneyChanger.coins(spurs)) {
            chest.setItem(slot++, stack);
        }
    }

    private static void giveEmeralds(ChestBlockEntity chest, int count) {
        for (int slot = 0; count > 0; slot++, count -= 64) {
            chest.setItem(slot, new ItemStack(Items.EMERALD, Math.min(64, count)));
        }
    }

    /**
     * A desk's emeralds are its town's money, and with no trade of the town's that costs emeralds, all of them are sold
     * to the mint for coin in the chest (B4.4; MoneyTests.surplusEmeraldsBecomeCoinsInStock and sellingOnlyMerchantsKeep
     * NoReserve in core mode). The coin the core mints is the Java mint's quote for the same sale, loose coin already in the
     * chest is minted again with it into a few stacks (MoneyTests.coinsAreRemintedIntoFewStacks), and the town's treasury
     * is the same before and after: selling at the mint changes what its money is held in, not what it is worth.
     */
    @GameTest(template = "floor")
    public static void aDesksEmeraldsAreSoldToTheMintForCoinsInItsChest(GameTestHelper helper) {
        var level = helper.getLevel();
        ChestBlockEntity chest = emptyDesk(helper);
        giveEmeralds(chest, 100);
        chest.setItem(5, dev.b28.enterprise.money.MoneyChanger.coins(64).get(0).copyWithCount(3)); // three loose cogs
        chest.setItem(6, dev.b28.enterprise.money.MoneyChanger.coins(1).get(0).copyWithCount(40)); // forty loose spurs
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            // The desk's first count is from nothing: 100 emeralds, 192 + 40 spurs of coin at 16 spurs an emerald make 114.5
            // emeralds in the treasury, and the same look shows them: the emeralds are sold to the mint for coin.
            int quote = new dev.b28.enterprise.money.Mint().quote(COMMONS, 100, level.getGameTime());
            host.stock().found(level, desk.pos(), "Testford");
            long t = level.getGameTime();
            helper.assertTrue(inChest(chest, Items.EMERALD) == 0, "the emeralds are still in the chest: " + inChest(chest, Items.EMERALD));
            helper.assertTrue(coinValue(chest) == 232 + quote, "coin worth " + (232 + quote) + " spurs in the chest, found "
                    + coinValue(chest));
            helper.assertTrue(coinStacks(chest) <= 6, "at most one stack of each coin, found " + coinStacks(chest));
            var after = host.money().cashAt(desk);
            helper.assertTrue(Math.abs(after.treasury() - 114.5) < 1e-9, "selling to the mint changed the treasury: " + after);
            helper.assertTrue(after.emeralds() == 0 && after.coins() == 232 + quote && after.bank() == null
                    && Math.abs(after.coinWorth() - 114.5) < 1e-9, "the core's till: " + after);
            helper.assertTrue(host.money().mint().rate() < 16, "the rate didn't fall: " + host.money().mint());
            helper.assertTrue(Math.abs(host.money().mint().pressure() - 100) < 1e-9, "the mint took no selling pressure: " + host.money().mint());
            // The next look, the chest as written, books nothing and sells nothing.
            host.look(level.getServer(), t + 2);
            host.settle();
            helper.assertTrue(coinValue(chest) == 232 + quote && Math.abs(host.money().cashAt(desk).treasury() - 114.5) < 1e-9,
                    "a second look moved money: " + host.money().cashAt(desk));
            // And the till survives a save and a load.
            helper.assertTrue(host.save(), "the host didn't save");
            try (CoreHost loaded = CoreHost.open(dir, 8)) {
                helper.assertTrue(loaded.money().cashAt(desk).equals(host.money().cashAt(desk)), "the loaded core's till: "
                        + loaded.money().cashAt(desk));
            }
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * A Blaze Banker touching the desk is the settlement's bank account (B4.4; MoneyTests.blazeBankerTakesTheDeposit and
     * firstPlayerToOpenTheBankerOwnsTheEarnings in core mode): the coin the mint pays for the desk's emeralds is deposited
     * there, none lands in the chests, and the first player to open the Banker owns the earnings.
     */
    @GameTest(template = "floor")
    public static void aBlazeBankersAccountTakesTheCoinTheMintPays(GameTestHelper helper) {
        var level = helper.getLevel();
        ChestBlockEntity chest = emptyDesk(helper);
        giveEmeralds(chest, 164);
        helper.setBlock(BANKER, net.minecraft.core.registries.BuiltInRegistries.BLOCK.get(
                net.minecraft.resources.ResourceLocation.fromNamespaceAndPath("numismatics", "blaze_banker")));
        var banker = (dev.ithundxr.createnumismatics.content.bank.blaze_banker.BlazeBankerBlockEntity) helper.getBlockEntity(BANKER);
        var account = dev.ithundxr.createnumismatics.Numismatics.BANK.getOrCreateAccount(banker.getAccountId(),
                dev.ithundxr.createnumismatics.content.backend.BankAccount.Type.BLAZE_BANKER);
        int before = account.getBalance();
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            host.stock().found(level, desk.pos(), "Testford");
            long t = level.getGameTime();
            int quote = new dev.b28.enterprise.money.Mint().quote(COMMONS, 164, t + 1);
            host.look(level.getServer(), t + 1);
            host.settle();
            helper.assertTrue(account.getBalance() - before == quote, "the balance grew by " + (account.getBalance() - before)
                    + ", the mint's quote is " + quote);
            helper.assertTrue(coinValue(chest) == 0 && inChest(chest, Items.EMERALD) == 0, "coin or emeralds left in the chest");
            var till = host.money().cashAt(desk);
            helper.assertTrue(till.bank() != null && till.bank() == account.getBalance() && till.coins() == 0,
                    "the core's till: " + till);
            helper.assertTrue(Math.abs(till.treasury() - 164) < 1e-9, "164 emeralds in the treasury: " + till);
            var owner = helper.makeMockPlayer(net.minecraft.world.level.GameType.SURVIVAL);
            helper.assertTrue(banker.ensureOwned(owner), "the first player to use the Banker claims it");
            helper.assertTrue(banker.getAccount() != null && banker.getAccount().getBalance() >= quote,
                    "the Merchant's earnings are in that Banker's account");
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * What a player puts in the desk's chests or its Banker is money paid into the town and what they take out is money
     * taken out, spurs counted at the mint's rate (B4.4). In Java mode the core only counts: nothing is sold to the mint
     * and nothing is written to the chests.
     */
    @GameTest(template = "floor")
    public static void inJavaModeTheCoreCountsADesksMoneyButMovesNone(GameTestHelper helper) {
        var level = helper.getLevel();
        ChestBlockEntity chest = emptyDesk(helper);
        giveEmeralds(chest, 10);
        giveCoins(chest, 160, 10);
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7, false)) {
            host.stock().found(level, desk.pos(), "Testford");
            helper.assertTrue(Math.abs(host.money().cashAt(desk).treasury() - 20) < 1e-9, "10 emeralds and 160 spurs: " + host.money().cashAt(desk));
            long t = level.getGameTime();
            host.look(level.getServer(), t + 1);
            host.settle();
            helper.assertTrue(inChest(chest, Items.EMERALD) == 10 && coinValue(chest) == 160, "the core wrote to a Java-mode chest");
            helper.assertTrue(host.money().mint().rate() == 16.0, "the core sold to the mint in Java mode: " + host.money().mint());
            // A player takes four emeralds and pays in 80 spurs of coin.
            chest.setItem(0, new ItemStack(Items.EMERALD, 6));
            giveCoins(chest, 80, 20);
            host.look(level.getServer(), t + 2);
            host.settle();
            var till = host.money().cashAt(desk);
            helper.assertTrue(Math.abs(till.treasury() - 21) < 1e-9 && till.emeralds() == 6 && till.coins() == coinValue(chest),
                    "four out, five in: " + till + ", the chest's coin " + coinValue(chest));
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * The town's earnings reach the chest (B4.4): money paid into the town from outside the society (a player buying at its
     * shops: Input.Pay) is put in the desk's chests as whole emeralds at the look that counts them, and sold to the mint for
     * coin in turn, so the town's treasury is the money in its chest and Banker, to the last spur.
     */
    @GameTest(template = "floor")
    public static void theTownsEarningsAreShownInItsChestAsCoin(GameTestHelper helper) {
        var level = helper.getLevel();
        ChestBlockEntity chest = emptyDesk(helper);
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk.pos(), "Testford");
            long t = level.getGameTime();
            host.apply(List.of(new Input.Pay(town, 7_400))); // 7.4 emeralds, in thousandths
            helper.assertTrue(coinValue(chest) == 0, "money reached the chest before it was counted");
            int quote = new dev.b28.enterprise.money.Mint().quote(COMMONS, 7, t + 1);
            host.look(level.getServer(), t + 1);
            host.settle();
            helper.assertTrue(inChest(chest, Items.EMERALD) == 0 && coinValue(chest) == quote,
                    "seven emeralds put in and sold for " + quote + " spurs: " + inChest(chest, Items.EMERALD) + " emeralds, coin "
                            + coinValue(chest));
            var till = host.money().cashAt(desk);
            helper.assertTrue(Math.abs(till.treasury() - 7.4) < 1e-9 && Math.abs(till.coinWorth() - 7) < 1e-9, "the till: " + till);
            // The 0.4 left on the books and 0.6 more make the eighth emerald.
            host.apply(List.of(new Input.Pay(town, 600)));
            host.look(level.getServer(), t + 2);
            host.settle();
            helper.assertTrue(Math.abs(host.money().cashAt(desk).coinWorth() - 8) < 1e-9, "the eighth emerald: " + host.money().cashAt(desk));
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * Money with no room waits on the town's books (B4.4): a chest full of dirt takes no emeralds and no coin, the town
     * keeps the money, and the next look with a slot free puts the emeralds in (no room for a set of coin: they stay
     * emeralds).
     */
    @GameTest(template = "floor")
    public static void moneyWithNoRoomInTheChestWaitsOnTheTownsBooks(GameTestHelper helper) {
        var level = helper.getLevel();
        ChestBlockEntity chest = emptyDesk(helper);
        for (int slot = 0; slot < chest.getContainerSize(); slot++) {
            chest.setItem(slot, new ItemStack(Items.DIRT, 64));
        }
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk.pos(), "Testford");
            long t = level.getGameTime();
            host.apply(List.of(new Input.Pay(town, 5_000)));
            host.look(level.getServer(), t + 1);
            host.settle();
            helper.assertTrue(inChest(chest, Items.EMERALD) == 0 && coinValue(chest) == 0, "money written to a full chest");
            helper.assertTrue(Math.abs(host.money().cashAt(desk).treasury() - 5) < 1e-9, "the town lost its money: " + host.money().cashAt(desk));
            chest.setItem(26, ItemStack.EMPTY);
            host.look(level.getServer(), t + 2);
            host.settle();
            helper.assertTrue(inChest(chest, Items.EMERALD) == 5 && coinValue(chest) == 0, "five emeralds in the one free slot: "
                    + inChest(chest, Items.EMERALD) + " emeralds, coin " + coinValue(chest));
            helper.assertTrue(host.money().mint().pressure() == 0, "sold to the mint with no room for the coin");
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /** A desk broken takes its chests' and Banker's money out of its town, and put back where it stood counts them in once (B4.4). */
    @GameTest(template = "floor")
    public static void aDeskBrokenTakesItsMoneyAndOnePutBackCountsItOnce(GameTestHelper helper) {
        var level = helper.getLevel();
        ChestBlockEntity chest = emptyDesk(helper);
        giveEmeralds(chest, 30);
        GlobalPos desk = GlobalPos.of(level.dimension(), helper.absolutePos(DESK));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7, false)) {
            long town = host.stock().found(level, desk.pos(), "Testford");
            helper.assertTrue(Math.abs(host.money().cashAt(desk).treasury() - 30) < 1e-9, "30 emeralds: " + host.money().cashAt(desk));
            host.stock().forget(level, desk.pos());
            Answer.Stock gone = (Answer.Stock) host.ask(List.of(new Query.Stock(town))).get(0);
            helper.assertTrue(host.money().cashAt(desk) == null && gone.stock().size() > 0, "the desk is still the core's");
            long again = host.stock().found(level, desk.pos(), "Testford");
            helper.assertTrue(again == town, "a desk put back is another town");
            helper.assertTrue(Math.abs(host.money().cashAt(desk).treasury() - 30) < 1e-9, "counted in twice or not at all: " + host.money().cashAt(desk));
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * A hex's ground is the survey's in the core (B4.7): a far hex flooded and cragged in a survey of its own is sent as
     * the adapter reads it (its share under water, its relief, its biome's land) and the core answers the same, and the
     * hex is no way through for a caravan (its relief past a cliff). The arena's own hex, its chunk surveyed again, goes
     * with the next look, and the core has the world's survey of it.
     */
    @GameTest(template = "floor")
    public static void aHexsGroundIsTheSurveysInTheCore(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            host.stock().found(level, helper.absolutePos(DESK), "Testford");
            dev.b28.enterprise.territory.Terrain terrain = new dev.b28.enterprise.territory.Terrain();
            dev.b28.enterprise.hex.Hex far = new dev.b28.enterprise.hex.Hex(5100, 5100);
            for (long chunk : dev.b28.enterprise.territory.Terrain.chunksOf(far)) {
                terrain.record(level, chunk, dev.b28.enterprise.territory.Terrain.SAMPLES * 3 / 4);
            }
            var chunks = dev.b28.enterprise.territory.Terrain.chunksOf(far);
            terrain.recordHeights(level, chunks.get(0), new int[] {-60, -60});
            terrain.recordHeights(level, chunks.get(1), new int[] {-60, -20});
            Input.Ground sent = LandAdapter.ground(terrain, level, far);
            helper.assertTrue(sent.water() == 0.75 && sent.relief() == 40, "the survey reads " + sent);
            host.apply(List.of(sent));
            var core = host.land().groundOf(List.of(sent.hex())).get(0);
            helper.assertTrue(core.water() == sent.water() && core.relief() == sent.relief() && core.land() == sent.land()
                    && !core.bridged(), "the core has " + core + " for the survey's " + sent);

            dev.b28.enterprise.hex.Hex home = dev.b28.enterprise.hex.Hex.at(helper.absolutePos(DESK));
            host.land().surveyed(level, new net.minecraft.world.level.ChunkPos(helper.absolutePos(DESK)));
            host.look(level.getServer(), level.getGameTime());
            host.settle();
            Input.Ground world = LandAdapter.ground(level, home);
            var had = host.land().groundOf(List.of(world.hex())).get(0);
            helper.assertTrue(had.water() == world.water() && had.relief() == world.relief() && had.land() == world.land(),
                    "after a look the core has " + had + " for the arena's hex, the world's survey " + world);
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * A town's fields are the crops counted in its land, in the core (B4.7; TerritoryTests.farmersProduceWhatGrowsInTheirHex
     * drives the Java rule): up to ten wheat plots by the bell in its home hex, counted by a look's survey of the town
     * (core mode's), are its farm's plots before it holds a hex, and once it has claimed its home hex they're still its.
     * Neighbouring tests' crops in the hex count too, so at least those planted.
     */
    @GameTest(template = "floor")
    public static void aTownsFieldsAreTheCropsCountedInItsLand(GameTestHelper helper) {
        var level = helper.getLevel();
        BlockPos desk = settlement(helper, 0);
        // Wheat in the arena where it lies in the town's home hex (the one its desk's block is in: the
        // arena may straddle a hex's edge, and a block's crops are its own hex's).
        dev.b28.enterprise.hex.Hex home = dev.b28.enterprise.hex.Hex.at((double) desk.getX(), (double) desk.getZ());
        int planted = 0;
        List<BlockPos> taken = List.of(BELL, TOWN_DESK, TOWN_DESK.east(), new BlockPos(2, 2, 4));
        for (int z = 0; z < 7 && planted < 10; z++) {
            for (int x = 0; x < 7 && planted < 10; x++) {
                BlockPos at = helper.absolutePos(new BlockPos(x, 2, z));
                if (!taken.contains(new BlockPos(x, 2, z))
                        && dev.b28.enterprise.hex.Hex.at(at.getX() + 0.5, at.getZ() + 0.5).equals(home)) {
                    helper.setBlock(new BlockPos(x, 1, z), Blocks.FARMLAND);
                    helper.setBlock(new BlockPos(x, 2, z), Blocks.WHEAT);
                    planted++;
                }
            }
        }
        helper.assertTrue(planted >= 3, "only " + planted + " of the arena's blocks lie in the desk's hex");
        Villager farmer = villager(helper, VillagerProfession.FARMER, 1, new BlockPos(2, 2, 4));
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, desk, "Testford");
            host.hands().join(town, farmer);
            LandAdapter land = host.land();
            long a = System.nanoTime();
            List<Input> first = land.happenings(level.getServer(), true);
            long firstNs = System.nanoTime() - a;
            Response r = host.apply(first);
            helper.assertTrue(r instanceof Response.Done, "the survey was refused: " + r);
            measureALandLook(host, level, firstNs, first);
            var before = land.land(town);
            double wheat = before.plots().getOrDefault("minecraft:wheat", 0.0);
            helper.assertTrue(before.hexes().isEmpty() && wheat >= planted,
                    "before its first count the town's fields are its home hex's: " + before);
            advance(host, level.getGameTime() + DAY);
            land.surveyNow(level, town, desk);
            var after = land.land(town);
            helper.assertTrue(after.hexes().size() == 1, "the town holds " + after.hexes() + ", not its home hex");
            helper.assertTrue(after.plots().getOrDefault("minecraft:wheat", 0.0) >= planted, "its fields are " + after.plots());
        } finally {
            delete(dir);
            farmer.discard();
        }
        helper.succeed();
    }

    /**
     * A work the core asks for is laid in the world and said to be (B4.7): a watchtower at a site in the arena (a hall's
     * site is found round the bell in a world; here given), its stone up seven blocks with a ladder inside, and a
     * {@code Built} for it waiting for the next look. The core takes it as a stale view (the test's town has no tower),
     * as it would a work it has already been told of.
     */
    @GameTest(template = "floor")
    public static void aWorkTheCoreAsksForIsLaidAndSaidToBeLaid(GameTestHelper helper) {
        var level = helper.getLevel();
        helper.setBlock(DESK, Merchant.DESK.get());
        Path dir = folder();
        try (CoreHost host = CoreHost.open(dir, 7)) {
            long town = host.stock().found(level, helper.absolutePos(DESK), "Testford");
            LandAdapter land = host.land();
            var tower = new dev.b28.enterprise.wire.Work.Hall("watchtower");
            BlockPos corner = helper.absolutePos(new BlockPos(0, 1, 0));
            helper.assertTrue(land.lay(level, town, helper.absolutePos(DESK), tower, 1.0, java.util.Optional.of(corner)),
                    "the tower wasn't laid");
            helper.assertBlockPresent(Blocks.STONE_BRICKS, new BlockPos(0, 1, 0));
            helper.assertBlockPresent(Blocks.STONE_BRICKS, new BlockPos(0, 7, 0));
            helper.assertBlockPresent(Blocks.LADDER, new BlockPos(1, 3, 1));
            helper.assertTrue(land.pending().equals(List.of(new Input.Built(town, tower, 1.0))),
                    "waiting to be said: " + land.pending());
            helper.assertTrue(!land.lay(level, town, helper.absolutePos(DESK), new dev.b28.enterprise.wire.Work.Hall("palace"),
                    1.0, java.util.Optional.of(corner)), "a hall the mod hasn't was laid");
            land.sayNow();
            helper.assertTrue(land.pending().isEmpty(), "still waiting: " + land.pending());
            helper.assertTrue(land.land(town).works().isEmpty(), "the core took a tower it never asked for: " + land.land(town));
        } finally {
            delete(dir);
        }
        helper.succeed();
    }

    /**
     * What a look's land costs the server's tick, logged: the first look of a run (the ground of up to 64 hexes near a
     * town first seen, its crops counted in its hexes, its survey), then a look with only the survey (the crops are
     * counted every 20 looks), and one town's crops counted on their own.
     */
    private static void measureALandLook(CoreHost host, net.minecraft.server.level.ServerLevel level, long firstNs,
            List<Input> first) {
        int n = 50;
        long a = System.nanoTime();
        for (int i = 0; i < n; i++) {
            host.land().happenings(level.getServer(), true);
        }
        long survey = (System.nanoTime() - a) / n;
        var desk = host.points().of(dev.b28.enterprise.binding.adapters.PointsAdapter.Kind.DESK).entrySet().iterator().next();
        BlockPos bell = PeopleAdapter.centre(level, desk.getKey().pos());
        a = System.nanoTime();
        int counted = 0;
        for (int i = 0; i < n; i++) {
            counted = host.land().crops(level, desk.getValue(), desk.getKey().pos(), bell).size();
        }
        long crops = (System.nanoTime() - a) / n;
        Enterprise.LOGGER.info("land look: the first {} ms ({} grounds, {} chunks of crops); a look after it {} ms (fifty averaged, the crops counted at every twentieth); one hex's crops"
                + " ({} chunks) {} ms", "%.3f".formatted(firstNs / 1e6),
                first.stream().filter(i -> i instanceof Input.Ground).count(),
                first.stream().filter(i -> i instanceof Input.Crops).count(), "%.3f".formatted(survey / 1e6), counted,
                "%.3f".formatted(crops / 1e6));
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
            // Money written (money-adapter): 64 emeralds and 1,000 spurs of coin put into and taken out of the same desk's
            // chests (the coin minted again as the fewest coins each time), as the core's Cash would have it.
            var deskPoint = dev.b28.enterprise.binding.adapters.PointsAdapter.point(
                    dev.b28.enterprise.binding.adapters.PointsAdapter.Kind.DESK, GlobalPos.of(level.dimension(), helper.absolutePos(desks.get(0))));
            long[] cash = new long[moves];
            for (int i = 0; i < moves; i++) {
                int sign = i % 2 == 0 ? 1 : -1;
                var paid = new dev.b28.enterprise.wire.Output.Cash(deskPoint, 64L * sign, 1000L * sign, 0);
                long a = System.nanoTime();
                host.money().show(paid);
                cash[i] = System.nanoTime() - a;
            }
            java.util.Arrays.sort(cash);
            out.append("\n  money written (64 emeralds and 1,000 spurs of coin in or out of a desk's 5 chests): %.3f ms mean, p50 %.3f, max %.3f"
                    .formatted(java.util.Arrays.stream(cash).average().orElse(0) / 1e6, cash[moves / 2] / 1e6, cash[moves - 1] / 1e6));
            helper.assertTrue(host.money().pending().isEmpty(), "a measured cash move didn't fit");
            long[] counts = new long[moves];
            for (int i = 0; i < moves; i++) {
                long a = System.nanoTime();
                host.money().cashcount(level, GlobalPos.of(level.dimension(), helper.absolutePos(desks.get(i % desks.size()))));
                counts[i] = System.nanoTime() - a;
            }
            java.util.Arrays.sort(counts);
            out.append("\n  a desk's money counted (emeralds, coin, the Banker: 5 chests, 135 slots): %.3f ms mean, p50 %.3f, max %.3f"
                    .formatted(java.util.Arrays.stream(counts).average().orElse(0) / 1e6, counts[moves / 2] / 1e6, counts[moves - 1] / 1e6));
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

    // ---- the crown (B4.10) ------------------------------------------------------------------------------------------

    /**
     * The crown mobilises and buys gold at a vendor (B4.10; WarTests.mobilisingTheSovereignMintsAndBuysGoldThroughAShop's
     * twin): the hub is named the capital and mobilised through the core, which asks for a gold shop; the next look puts a
     * vendor up on open floor by the desk and registers it as the crown's, the one after shows it buying 8 gold for the
     * core's price and funds its card from the treasury; 16 gold a player left in it is taken out, the crown's; peace
     * takes the vendor down.
     */
    @GameTest(template = "floor")
    public static void aMobilisedCrownBuysGoldAtAVendorItsTreasuryFunds(GameTestHelper helper) {
        Path dir = folder();
        Wagons w = null;
        try (CoreHost host = CoreHost.open(dir, 7)) {
            w = wagons(helper, host);
            var level = helper.getLevel();
            BlockPos hubDesk = helper.absolutePos(HUB_DESK);
            // The treasury is the desk's emeralds: a chest of them beside it, counted at the first look.
            helper.setBlock(HUB_DESK.north(), Blocks.CHEST);
            ChestBlockEntity treasury = chestAt(helper, HUB_DESK.north());
            for (int i = 0; i < 10; i++) {
                treasury.setItem(i, new ItemStack(Items.EMERALD, 64));
            }
            helper.assertTrue(host.crown().capital(level, hubDesk, 16) == null, "the capital wasn't named");
            helper.assertTrue(host.crown().mobilise(level, hubDesk, 480) == null, "the crown wouldn't mobilise");
            long t = level.getGameTime();
            host.look(level.getServer(), t + 1_200);
            host.settle();
            var shops = host.points().of(PointsAdapter.Kind.SHOP);
            GlobalPos at = shops.keySet().stream().filter(p -> level.getBlockEntity(p.pos())
                    instanceof dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity v && v.getFilterItem().is(Items.GOLD_INGOT))
                    .findFirst().orElse(null);
            helper.assertTrue(at != null && shops.get(at) == w.hub(), "the crown's vendor isn't the capital's shop point: " + shops);
            var vendor = (dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity) level.getBlockEntity(at.pos());
            helper.assertTrue(vendor != null && vendor.getMode() == dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity.Mode.BUY
                    && vendor.getFilterItem().is(Items.GOLD_INGOT) && vendor.getFilterItem().getCount() == 8 && vendor.getTotalPrice() > 0,
                    "the vendor doesn't buy 8 gold: " + vendor + " " + (vendor == null ? "" : vendor.getFilterItem() + " at " + vendor.getTotalPrice()));
            host.look(level.getServer(), t + 2_400);
            host.settle();
            long card = dev.b28.enterprise.market.Shops.purse(vendor).map(a -> (long) a.getBalance()).orElse(-1L);
            helper.assertTrue(card >= 2L * vendor.getTotalPrice(), "the card holds " + card + " spurs, not two lots' price of "
                    + 2L * vendor.getTotalPrice() + "; " + host.commands().war());
            // A player sells 16 gold into it: the crown's, taken out; the card made up.
            vendor.items.set(0, new ItemStack(Items.GOLD_INGOT, 16));
            vendor.setChanged();
            host.look(level.getServer(), t + 3_600);
            host.settle();
            helper.assertTrue(inVendor(vendor, Items.GOLD_INGOT) == 0, "the gold sold into it is still in it: "
                    + inVendor(vendor, Items.GOLD_INGOT));
            helper.assertTrue(host.commands().war().startsWith("war: mobilising"), "the war: " + host.commands().war());
            // Peace takes the vendor down, and the core's point for it.
            host.crown().peace(level);
            helper.assertTrue(level.getBlockState(at.pos()).isAir() && host.points().at(at) == null,
                    "the vendor stands at peace: " + level.getBlockState(at.pos()) + " " + host.points().at(at));
            helper.assertTrue(host.commands().war().startsWith("war: peace"), "the war: " + host.commands().war());
            if (System.getenv("ENTERPRISE_CORE_MEASURE") != null) {
                measureTheCrown(host, level.getServer(), t + 3_600);
            }
        } finally {
            delete(dir);
            if (w != null) {
                w.discard();
            }
        }
        helper.succeed();
    }

    /** What the crown costs a look, logged: the gold shop's place check with nothing to build, the books asked, and a look at war. */
    private static void measureTheCrown(CoreHost host, net.minecraft.server.MinecraftServer server, long from) {
        int n = 1000;
        long a = System.nanoTime();
        for (int i = 0; i < n; i++) {
            host.crown().place(server);
        }
        long placed = System.nanoTime() - a;
        int m = 50;
        a = System.nanoTime();
        for (int i = 0; i < m; i++) {
            host.crown().realms();
            host.crown().metals();
        }
        long asked = System.nanoTime() - a;
        Enterprise.LOGGER.info("crown: the gold shop's place check {} ms a look, the realms and metals asked {} ms",
                "%.5f".formatted(placed / 1e6 / n), "%.3f".formatted(asked / 1e6 / m));
    }

    /**
     * War is declared and peace made through the core (B4.10): the hub is the capital with a road to Farley, so declaring
     * levies Farley and opens the front, a market off the map that is a point of the core's; the war's line says war;
     * peace says how many walk home. A theatre the core hasn't is refused.
     */
    @GameTest(template = "floor")
    public static void warIsDeclaredLeviesTheTownsTheCapitalReachesAndPeaceEndsIt(GameTestHelper helper) {
        Path dir = folder();
        Wagons w = null;
        try (CoreHost host = CoreHost.open(dir, 7)) {
            w = wagons(helper, host);
            var level = helper.getLevel();
            BlockPos hubDesk = helper.absolutePos(HUB_DESK);
            host.crown().capital(level, hubDesk, 16);
            host.crown().mobilise(level, hubDesk, 480);
            helper.assertTrue(host.wagons().addRoute(w.hub(), w.far()).getKey() >= 0, "no road to Farley");
            String[] why = new String[1];
            helper.assertTrue(host.crown().declare(level, "sea", why) < 0 && why[0] != null, "war on the sea was declared");
            int levies = host.crown().declare(level, "faction", why);
            helper.assertTrue(levies == 1, "levies " + levies + " " + why[0]);
            helper.assertTrue(host.commands().war().startsWith("war: war"), "the war: " + host.commands().war());
            long markets = host.points().all().values().stream().filter(p -> p.kind() == PointsAdapter.Kind.MARKET).count();
            helper.assertTrue(markets == 2, "the front and the market across the sea aren't both points: " + host.points().all());
            helper.assertTrue(host.commands().warTally().get(0).equals("War tally, against faction"), "the tally: " + host.commands().warTally());
            host.crown().peace(level);
            helper.assertTrue(host.commands().war().startsWith("war: peace"), "the war: " + host.commands().war());
        } finally {
            delete(dir);
            if (w != null) {
                w.discard();
            }
        }
        helper.succeed();
    }

    /**
     * A realm opened off the map is a market point of the core's (B4.10; CaravanTests.aCaravanDeliversToAMarketOffTheMap's
     * and Realms.open's twin): the Coast, opened 2,400 blocks off with the hub and Farley as the markets it trades with, is
     * a {@code Market} point with a town of its own, and {@code /enterprise realms} tells of it. A place taken, and a
     * partner that isn't a desk of the core's, are refused.
     */
    @GameTest(template = "floor")
    public static void aRealmOpenedOffTheMapIsAMarketPointTheCoreTradesWith(GameTestHelper helper) {
        Path dir = folder();
        Wagons w = null;
        try (CoreHost host = CoreHost.open(dir, 7)) {
            w = wagons(helper, host);
            var level = helper.getLevel();
            BlockPos hubDesk = helper.absolutePos(HUB_DESK);
            BlockPos farDesk = helper.absolutePos(FAR_DESK);
            BlockPos coast = hubDesk.offset(-2_400, 0, 0);
            String why = host.crown().openRealm(level, "coast", coast, List.of(hubDesk, farDesk));
            helper.assertTrue(why == null, "the Coast wouldn't open: " + why);
            var markets = host.points().of(PointsAdapter.Kind.MARKET);
            helper.assertTrue(markets.size() == 1 && markets.containsKey(GlobalPos.of(level.dimension(), coast)),
                    "the Coast isn't a market point: " + markets);
            var realms = host.crown().realms();
            helper.assertTrue(realms.size() == 1 && realms.get(0).startsWith("the Coast: 0 trips to 2 markets, "),
                    "realms: " + realms);
            helper.assertTrue(host.crown().openRealm(level, "coast", coast, List.of(hubDesk)) != null, "a place taken was opened");
            helper.assertTrue(host.crown().openRealm(level, "march", coast.offset(0, 0, 100), List.of(hubDesk.offset(30, 0, 30))) != null,
                    "a partner that is no desk was taken");
            helper.assertTrue(host.crown().openRealm(level, "narnia", coast.offset(0, 0, 100), List.of(hubDesk)) != null,
                    "a realm the core hasn't was opened");
            helper.assertTrue(host.crown().metals().isEmpty(), "a metal exchange with no capital named");
            host.crown().capital(level, hubDesk, 16);
            var metals = host.crown().metals();
            helper.assertTrue(metals.size() > 1 && metals.get(0).endsWith("(peace)"), "metals: " + metals);
        } finally {
            delete(dir);
            if (w != null) {
                w.discard();
            }
        }
        helper.succeed();
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
