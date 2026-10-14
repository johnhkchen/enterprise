package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.market.Broker;
import dev.b28.enterprise.market.Exchange;
import dev.b28.enterprise.market.Shops;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.settlement.Census;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Territory;
import dev.ithundxr.createnumismatics.Numismatics;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import dev.ithundxr.createnumismatics.content.backend.Coin;
import dev.ithundxr.createnumismatics.content.bank.CardItem;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.item.EnchantedBookItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.enchantment.EnchantmentInstance;
import net.minecraft.world.item.enchantment.Enchantments;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** A master Merchant at its desk, players' shops (Numismatics vendors) in the arena. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class MarketTests {
    private static final BlockPos DESK = new BlockPos(3, 2, 3);
    private static final BlockPos CHEST = new BlockPos(4, 2, 3);
    private static final BlockPos BELL = new BlockPos(1, 2, 1);

    private static GlobalPos market(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(CHEST, Blocks.CHEST);
        Villager merchant = helper.spawn(EntityType.VILLAGER, DESK.south());
        merchant.setVillagerData(merchant.getVillagerData().setProfession(Merchant.PROFESSION.get()).setLevel(5));
        LockerTradingTests.claim(helper, merchant, DESK);
        return GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(DESK));
    }

    private static final net.minecraft.world.level.block.Block VENDOR = net.minecraft.core.registries.BuiltInRegistries.BLOCK
            .get(net.minecraft.resources.ResourceLocation.parse("numismatics:vendor"));

    private static MerchantStock stock(GameTestHelper helper) {
        return MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
    }

    /** A player's shop at {@code at}: buying or selling lots of {@code lot} for {@code spurs}. */
    private static VendorBlockEntity shop(GameTestHelper helper, BlockPos at, VendorBlockEntity.Mode mode, ItemStack lot,
            int spurs) {
        helper.setBlock(at, VENDOR);
        VendorBlockEntity vendor = (VendorBlockEntity) helper.getLevel().getBlockEntity(helper.absolutePos(at));
        vendor.setMode(mode);
        vendor.filterContainer.setItem(0, lot);
        Shops.setPrice(vendor, spurs);
        return vendor;
    }

    /** A bank card in the shop's card slot, its account holding {@code spurs}. */
    private static BankAccount purse(VendorBlockEntity vendor, int spurs) {
        BankAccount account = Numismatics.BANK.getOrCreateAccount(UUID.randomUUID(), BankAccount.Type.PLAYER);
        account.deposit(spurs);
        ItemStack card = new ItemStack(net.minecraft.core.registries.BuiltInRegistries.ITEM
                .get(net.minecraft.resources.ResourceLocation.parse("numismatics:white_card")));
        CardItem.set(card, account.id);
        vendor.cardContainer.setItem(0, card);
        return account;
    }

    private static int held(VendorBlockEntity vendor, net.minecraft.world.item.Item item) {
        return vendor.items.stream().filter(s -> s.is(item)).mapToInt(ItemStack::getCount).sum();
    }

    @GameTest(template = "floor")
    public static void aMasterMerchantFillsAPlayersOrder(GameTestHelper helper) {
        GlobalPos desk = market(helper);
        stock(helper).insert(new ItemStack(Items.PAPER, 64));
        VendorBlockEntity order = shop(helper, new BlockPos(6, 2, 6), VendorBlockEntity.Mode.BUY,
                new ItemStack(Items.PAPER, 16), 16);
        BankAccount account = purse(order, 40);
        helper.assertTrue(Broker.broker(helper.getLevel(), desk, new Territory(), helper.getLevel().getGameTime()),
                "a master Merchant brokers");
        helper.assertTrue(held(order, Items.PAPER) == 32, "two lots, all the purse pays for: " + held(order, Items.PAPER));
        helper.assertTrue(account.getBalance() == 8, "paid 32 spurs: " + account.getBalance());
        helper.assertTrue(stock(helper).count(new ItemCost(Items.PAPER, 1)) == 32, "from the warehouse");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void whatItCantFillIsAskedOfTheRegion(GameTestHelper helper) {
        GlobalPos desk = market(helper);
        VendorBlockEntity order = shop(helper, new BlockPos(6, 2, 6), VendorBlockEntity.Mode.BUY,
                new ItemStack(Items.NAUTILUS_SHELL, 2), 64);
        purse(order, 640);
        Broker.broker(helper.getLevel(), desk, new Territory(), helper.getLevel().getGameTime());
        var exchange = Exchange.get(helper.getLevel().getServer());
        helper.assertTrue(exchange.wanted(desk).getOrDefault(Items.NAUTILUS_SHELL, 0) == 20,
                "ten lots open, for caravans to bring: " + exchange.wanted(desk));
        helper.assertTrue(exchange.signals(desk).stream().anyMatch(s -> s.item() == Items.NAUTILUS_SHELL),
                "and no market has them: posted as wanted");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void itBuysFromAPlayersStallForItsOrders(GameTestHelper helper) {
        GlobalPos desk = market(helper);
        stock(helper).insert(new ItemStack(Items.EMERALD, 10));
        VendorBlockEntity order = shop(helper, new BlockPos(6, 2, 6), VendorBlockEntity.Mode.BUY,
                new ItemStack(Items.IRON_INGOT, 4), 24);
        purse(order, 48);
        VendorBlockEntity stall = shop(helper, new BlockPos(6, 2, 1), VendorBlockEntity.Mode.SELL,
                new ItemStack(Items.IRON_INGOT, 4), 12);
        Shops.stock(stall, new ItemStack(Items.IRON_INGOT, 8));
        long t = helper.getLevel().getGameTime();
        Broker.broker(helper.getLevel(), desk, new Territory(), t);
        helper.assertTrue(Shops.lots(stall) == 0, "it buys the stall's two lots for the order");
        Broker.broker(helper.getLevel(), desk, new Territory(), t + Broker.TICKS);
        helper.assertTrue(held(order, Items.IRON_INGOT) == 8, "and fills the order: " + held(order, Items.IRON_INGOT));
        var tally = Exchange.get(helper.getLevel().getServer()).tally(desk);
        helper.assertTrue(tally.spursFromSales() - tally.spursOnPurchases() >= 24, "at a profit: "
                + tally.spursFromSales() + " in, " + tally.spursOnPurchases() + " out");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aCommissionIsEnchantedToOrder(GameTestHelper helper) {
        GlobalPos desk = market(helper);
        Territory territory = new Territory();
        GlobalPos bell = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL));
        Map<UUID, Settlements.Resident> roster = new java.util.LinkedHashMap<>();
        for (var trade : List.of(Map.entry("minecraft:librarian", 5), Map.entry("minecraft:cleric", 3))) {
            UUID id = UUID.randomUUID();
            roster.put(id, new Settlements.Resident(id, 0, Optional.empty(), trade.getKey(), trade.getValue(), true,
                    false, 0, false));
        }
        Settlements.get(helper.getLevel().getServer()).setRoster(bell, roster);
        territory.afterCensus(helper.getLevel(), bell.pos(), new Census(5, 5, 5, 5, 0), 1, List.of());
        var efficiency = helper.getLevel().registryAccess().registryOrThrow(Registries.ENCHANTMENT)
                .getHolderOrThrow(Enchantments.EFFICIENCY);
        stock(helper).insert(new ItemStack(Items.IRON_PICKAXE));
        stock(helper).insert(EnchantedBookItem.createForEnchantment(new EnchantmentInstance(efficiency, 1)));
        stock(helper).insert(new ItemStack(Items.BOOK));
        stock(helper).insert(new ItemStack(Items.LAPIS_LAZULI));
        stock(helper).insert(new ItemStack(Items.EMERALD, 10));
        ItemStack wanted = new ItemStack(Items.IRON_PICKAXE);
        wanted.enchant(efficiency, 2);
        VendorBlockEntity order = shop(helper, new BlockPos(6, 2, 6), VendorBlockEntity.Mode.BUY, wanted, 64);
        purse(order, 100);
        long t = helper.getLevel().getGameTime();
        Broker.broker(helper.getLevel(), desk, territory, t);
        Broker.broker(helper.getLevel(), desk, territory, t + Broker.TICKS);
        helper.assertTrue(held(order, Items.IRON_PICKAXE) == 1 && order.matchesFilterItem(order.items.get(0)),
                "copied, combined to Efficiency II, laid on and handed over: "
                        + Exchange.get(helper.getLevel().getServer()).recent(desk));
        helper.assertTrue(Exchange.get(helper.getLevel().getServer()).tally(desk).commissions() == 1, "one commission");
        Settlements.get(helper.getLevel().getServer()).setRoster(bell, Map.of());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void itsOwnShopSellsTheSurplus(GameTestHelper helper) {
        GlobalPos desk = market(helper);
        stock(helper).insert(new ItemStack(Items.STICK, 64));
        helper.setBlock(DESK.north(), VENDOR);
        VendorBlockEntity own = (VendorBlockEntity) helper.getLevel().getBlockEntity(helper.absolutePos(DESK.north()));
        long t = helper.getLevel().getGameTime();
        Broker.broker(helper.getLevel(), desk, new Territory(), t);
        helper.assertTrue(own.getMode() == VendorBlockEntity.Mode.SELL && own.getFilterItem().is(Items.STICK)
                && held(own, Items.STICK) == 64, "the town's shop sells its sticks: " + own.getFilterItem());
        own.addCoin(Coin.SPUR, 9); // a player bought a lot
        Broker.broker(helper.getLevel(), desk, new Territory(), t + Broker.TICKS);
        helper.assertTrue(Exchange.get(helper.getLevel().getServer()).tally(desk).spursFromOwnShops() == 9,
                "and the town banks the takings");
        helper.succeed();
    }
}
