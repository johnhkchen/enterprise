package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.caravan.Atlas;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.service.Services;
import dev.b28.enterprise.service.Visit;
import dev.b28.enterprise.settlement.Census;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Territory;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** A five-villager town with its desk and chest in the arena; each test keeps its own Territory, and forgets its roster. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class ServiceTests {
    private static final BlockPos BELL = new BlockPos(3, 2, 3);
    private static final BlockPos DESK = new BlockPos(5, 2, 5);
    private static final Census FIVE = new Census(5, 5, 5, 5, 0);

    private static GlobalPos town(GameTestHelper helper, Territory territory, String... trades) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        GlobalPos bell = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL));
        Map<UUID, Settlements.Resident> roster = new java.util.LinkedHashMap<>();
        for (String trade : trades) {
            UUID id = UUID.randomUUID();
            roster.put(id, new Settlements.Resident(id, 0, Optional.empty(), trade, 2, true, false, 0, false));
        }
        Settlements.get(helper.getLevel().getServer()).setRoster(bell, roster);
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), FIVE, 1, List.of());
        return bell;
    }

    private static GlobalPos desk(GameTestHelper helper) {
        return GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(DESK));
    }

    private static MerchantStock stock(GameTestHelper helper) {
        return MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
    }

    private static void forget(GameTestHelper helper, GlobalPos bell) {
        Settlements.get(helper.getLevel().getServer()).setRoster(bell, Map.of());
    }

    @GameTest(template = "floor")
    public static void everyTradeSaysWhatItDoes(GameTestHelper helper) {
        helper.assertTrue(Services.all().stream().allMatch(s -> !s.does().isBlank()), "each service says what it does");
        for (String trade : List.of("enterprise:merchant", "minecraft:cartographer", "minecraft:librarian",
                "minecraft:shepherd", "minecraft:leatherworker", "minecraft:fisherman", "minecraft:cleric",
                "minecraft:farmer", "minecraft:toolsmith", "minecraft:armorer", "minecraft:weaponsmith")) {
            helper.assertFalse(Services.of(trade).isEmpty(), trade + " does something");
        }
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aClericAtHomeBlessesItsTown(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, "minecraft:cleric");
        long t = helper.getLevel().getGameTime();
        territory.touch(helper.getLevel(), desk(helper).pos(), stock(helper), t);
        helper.assertTrue(territory.blessedUntil(bell) > t, "blessed while the Cleric is home");
        forget(helper, bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aLibrarianBindsBooksWhileReadingIsShort(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, "minecraft:librarian");
        stock(helper).insert(new ItemStack(Items.PAPER, 9));
        stock(helper).insert(new ItemStack(Items.LEATHER, 5));
        territory.touch(helper.getLevel(), desk(helper).pos(), stock(helper), helper.getLevel().getGameTime());
        int books = stock(helper).count(new ItemCost(Items.BOOK, 1));
        helper.assertTrue(books == 3, "three books from nine paper: " + books);
        helper.assertTrue(stock(helper).count(new ItemCost(Items.LEATHER, 1)) == 2, "and three leather");
        forget(helper, bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aVisitingFarmerBringsInTheUntendedHarvest(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory);
        territory.setIdleForTest(bell, Map.of("minecraft:wheat", 40.0), 0); // 80 plots no one tends
        GlobalPos home = GlobalPos.of(bell.dimension(), bell.pos().offset(500, 0, 0));
        long t = helper.getLevel().getGameTime();
        var help = Services.get("enterprise:harvest_help").orElseThrow();
        var route = new dev.b28.enterprise.caravan.CaravanLedger().addRoute(home, desk(helper));
        helper.assertTrue(help.wanted().test(Visit.planning(helper.getLevel(), territory, route, 1, t)),
                "fields no one tends: a Farmer is wanted there");
        double fee = help.perform().applyAsDouble(
                Visit.abroad(helper.getLevel(), territory, home, desk(helper), 1, stock(helper), 10, null, null, Map.of(), t));
        territory.touch(helper.getLevel(), desk(helper).pos(), stock(helper), t);
        int wheat = stock(helper).count(new ItemCost(Items.WHEAT, 1));
        helper.assertTrue(wheat == 16, "a Novice tends 32 plots for a day: 16 wheat, " + wheat);
        helper.assertTrue(fee > 0 && fee < 16 / dev.b28.enterprise.money.Prices.itemsPerEmerald("minecraft:wheat"),
                "for a farmhand's share of its worth: " + fee);
        forget(helper, bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aMapSwapTeachesBothTowns(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory);
        var server = helper.getLevel().getServer();
        GlobalPos home = GlobalPos.of(bell.dimension(), bell.pos().offset(3000, 0, 0));
        GlobalPos ours = GlobalPos.of(bell.dimension(), bell.pos().offset(9000, 0, 0));
        GlobalPos theirs = GlobalPos.of(bell.dimension(), bell.pos().offset(-9000, 0, 0));
        long t = helper.getLevel().getGameTime();
        Atlas.get(server).learn(home, ours, t);
        Atlas.get(server).learn(desk(helper), theirs, t);
        List<GlobalPos> heard = new java.util.ArrayList<>();
        var swap = Services.get("enterprise:map_swap").orElseThrow();
        Visit visit = Visit.abroad(helper.getLevel(), territory, home, desk(helper), 1, stock(helper), 10, null, null, Map.of(), t);
        helper.assertTrue(swap.wanted().test(visit), "they know different markets: worth a swap");
        swap.perform().applyAsDouble(visit);
        helper.assertTrue(Atlas.get(server).known(server, desk(helper)).contains(ours), "they learn of ours");
        helper.assertTrue(visit.heardForTest().contains(theirs), "and we carry home theirs");
        helper.assertFalse(Atlas.get(server).known(server, home).contains(theirs), "not before the Cartographer is home");
        forget(helper, bell);
        helper.succeed();
    }
}
