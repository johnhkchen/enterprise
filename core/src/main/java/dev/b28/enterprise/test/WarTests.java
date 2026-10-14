package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.cargo.Papers;
import dev.b28.enterprise.contract.Bills;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.market.Sovereign;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.nbt.CompoundTag;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** Bills of exchange and the sovereign's war shops. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class WarTests {
    private static GlobalPos far(GameTestHelper helper, int x, int z) {
        return GlobalPos.of(helper.getLevel().dimension(), new BlockPos(x, 64, z));
    }

    @GameTest(template = "floor")
    public static void aBillIsPaidWhenPresentedOnceDue(GameTestHelper helper) {
        var server = helper.getLevel().getServer();
        Bills bills = new Bills();
        GlobalPos drawee = far(helper, 8_000_000, 8_000_000);
        GlobalPos payee = far(helper, 8_000_400, 8_000_000);
        long t = 1_000_000;
        Papers.Document bill = bills.draw(drawee, payee, 10, t);
        var bundle = List.of(dev.b28.enterprise.cargo.Crates.documents(Papers.NONE.with(bill)));
        var early = bills.present(bundle, 100, t + 24000, new ContractBook());
        helper.assertTrue(early.paid() == 0 && early.returned().size() == 1, "not due yet: it goes back");
        var due = bills.present(early.returned(), 100, t + Bills.TERM_TICKS, new ContractBook());
        helper.assertTrue(Math.abs(due.paid() - 10 * (1 + Bills.INTEREST)) < 1e-9 && due.returned().isEmpty(),
                "due: paid with its interest, " + due.paid());
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aBillLongUnpaidIsDishonoured(GameTestHelper helper) {
        Bills bills = new Bills();
        ContractBook book = new ContractBook();
        GlobalPos drawee = far(helper, 8_100_000, 8_000_000);
        GlobalPos payee = far(helper, 8_100_400, 8_000_000);
        long t = 1_000_000;
        var bundle = List.of(dev.b28.enterprise.cargo.Crates.documents(Papers.NONE.with(bills.draw(drawee, payee, 10, t))));
        bills.present(bundle, 0, t + Bills.TERM_TICKS + Bills.DISHONOUR_TICKS + 1, book);
        helper.assertTrue(bills.all().get(0).state() == Bills.State.DISHONOURED, "dishonoured");
        helper.assertTrue(book.trust(drawee, payee) < ContractBook.START_TRUST, "and trust falls");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void mobilisingTheSovereignMintsAndBuysGoldThroughAShop(GameTestHelper helper) {
        var server = helper.getLevel().getServer();
        BlockPos desk = new BlockPos(3, 2, 3);
        helper.setBlock(desk, Merchant.DESK.get());
        Sovereign sovereign = new Sovereign();
        long t = helper.getLevel().getGameTime();
        double before = Mint.get(server).debasement(Mint.DEFAULT_FACTION, t);
        sovereign.mobilise(helper.getLevel(), GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(desk)), 1000);
        helper.assertTrue(sovereign.phase() == Sovereign.Phase.MOBILISING, "mobilising");
        var shop = (VendorBlockEntity) helper.getLevel().getBlockEntity(sovereign.shops().get(0));
        helper.assertTrue(shop.getMode() == VendorBlockEntity.Mode.BUY && shop.getFilterItem().is(Items.GOLD_INGOT)
                && dev.b28.enterprise.market.Shops.purse(shop).map(a -> a.getBalance() >= 1000).orElse(false),
                "a shop buying gold, paid from a treasury with a day's minting in it");
        helper.assertTrue(Mint.get(server).debasement(Mint.DEFAULT_FACTION, t) > before, "the minting debases the spur");
        helper.assertTrue(sovereign.targets(dev.b28.enterprise.army.Army.get(server)).get(Items.DIAMOND_SWORD) > 0,
                "it stockpiles diamond swords too");
        BlockPos at = sovereign.shops().get(0);
        sovereign.peace(helper.getLevel());
        helper.assertTrue(helper.getLevel().getBlockState(at).isAir(), "peace: the shop comes down");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void bonusClausesPayForQualityWithoutRequiringIt(GameTestHelper helper) {
        var book = new ContractBook();
        GlobalPos a = far(helper, 8_200_000, 8_000_000);
        GlobalPos b = far(helper, 8_200_400, 8_000_000);
        var contract = book.draft(a, b, List.of(new dev.b28.enterprise.contract.Delivery(Items.IRON_SWORD, 4, 2),
                        new dev.b28.enterprise.contract.Bonus(java.util.Map.of("minecraft:unbreaking", 1,
                                "minecraft:sharpness", 2), 25)),
                dev.b28.enterprise.contract.Incoterm.EXW, 0, 24000, 1, 0, a, true);
        var registry = helper.getLevel().registryAccess().registryOrThrow(net.minecraft.core.registries.Registries.ENCHANTMENT);
        ItemStack plain = new ItemStack(Items.IRON_SWORD);
        ItemStack sharp = new ItemStack(Items.IRON_SWORD);
        sharp.enchant(registry.getHolderOrThrow(net.minecraft.world.item.enchantment.Enchantments.SHARPNESS), 3);
        sharp.enchant(registry.getHolderOrThrow(net.minecraft.world.item.enchantment.Enchantments.UNBREAKING), 1);
        helper.assertTrue(contract.delivery().get().matches(sharp, helper.getLevel().registryAccess()),
                "any iron sword counts, enchanted or not");
        helper.assertTrue(contract.priceOf(plain) == 2 && contract.priceOf(sharp) == 2.5,
                "a plain one at the price, a sharp one with 25% on top");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void anArmyEatsFromTheDepot(GameTestHelper helper) {
        helper.setBlock(new BlockPos(3, 2, 3), Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), Blocks.CHEST);
        var depot = MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)));
        depot.insert(new ItemStack(Items.BREAD, 64));
        GlobalPos capital = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(new BlockPos(3, 2, 3)));
        var army = new dev.b28.enterprise.army.Army();
        for (int i = 0; i < 5; i++) {
            CompoundTag villager = new CompoundTag();
            villager.putUUID("UUID", java.util.UUID.randomUUID());
            army.enlist(villager, far(helper, 8_400_000, 8_000_000), 0);
        }
        long t = 1_000_000;
        army.campaign(helper.getLevel(), depot, capital, t);
        army.campaign(helper.getLevel(), depot, capital, t + 24000);
        helper.assertTrue(army.eaten() >= 5 * dev.b28.enterprise.army.Army.RATION,
                "five soldiers ate a day's rations: " + army.eaten());
        helper.assertTrue(depot.count(new net.minecraft.world.item.trading.ItemCost(Items.BREAD, 1)) == 64 - 8,
                "eight loaves: " + depot.count(new net.minecraft.world.item.trading.ItemCost(Items.BREAD, 1)));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theFrontKeepsRationsForTheRoad(GameTestHelper helper) {
        int near = dev.b28.enterprise.army.Army.rationDays(0);
        int far = dev.b28.enterprise.army.Army.rationDays(1500);
        helper.assertTrue(far > near && far >= 6, "a far front keeps more days of food: " + near + " vs " + far);
        var army = new dev.b28.enterprise.army.Army();
        army.muster(new net.minecraft.nbt.CompoundTag());
        army.muster(new net.minecraft.nbt.CompoundTag());
        int bread = army.wants(far).get(Items.BREAD);
        helper.assertTrue(bread * 5 >= 2 * dev.b28.enterprise.army.Army.RATION * far,
                "rations for the recruits on their way too: " + bread);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theArmyEatsItsHeartiestFoodFirst(GameTestHelper helper) {
        helper.setBlock(new BlockPos(3, 2, 3), Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), Blocks.CHEST);
        var depot = MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)));
        depot.insert(new ItemStack(Items.BREAD, 16));
        depot.insert(new ItemStack(Items.COOKED_BEEF, 1));
        double[] meal = dev.b28.enterprise.army.Army.messForTest(depot, 8);
        helper.assertTrue(meal[0] == 8 && depot.count(new net.minecraft.world.item.trading.ItemCost(Items.COOKED_BEEF, 1)) == 0
                && depot.count(new net.minecraft.world.item.trading.ItemCost(Items.BREAD, 1)) == 16,
                "a steak before bread: " + meal[0]);
        helper.assertTrue(meal[1] / meal[0] > 0, "a hearty meal");
        helper.assertTrue(dev.b28.enterprise.army.Army.heartiness(Items.COOKED_CHICKEN) > 0
                && dev.b28.enterprise.army.Army.heartiness(Items.BREAD) == 0, "any meat is hearty, bread isn't");
        helper.assertTrue(dev.b28.enterprise.army.Army.heartiness(1.2) == 0, "bread is plain fare");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void soldiersFallLessInBetterKit(GameTestHelper helper) {
        var theatre = dev.b28.enterprise.army.Army.Theatre.FACTION;
        double bare = dev.b28.enterprise.army.Army.risk(ItemStack.EMPTY, ItemStack.EMPTY, ItemStack.EMPTY, theatre);
        double iron = dev.b28.enterprise.army.Army.risk(new ItemStack(Items.IRON_SWORD), new ItemStack(Items.IRON_CHESTPLATE),
                new ItemStack(Items.SHIELD), theatre);
        double diamond = dev.b28.enterprise.army.Army.risk(new ItemStack(Items.DIAMOND_SWORD),
                new ItemStack(Items.DIAMOND_CHESTPLATE), new ItemStack(Items.SHIELD), theatre);
        var registry = helper.getLevel().registryAccess().registryOrThrow(net.minecraft.core.registries.Registries.ENCHANTMENT);
        ItemStack sharp = new ItemStack(Items.DIAMOND_SWORD);
        sharp.enchant(registry.getHolderOrThrow(net.minecraft.world.item.enchantment.Enchantments.SHARPNESS), 5);
        double enchanted = dev.b28.enterprise.army.Army.risk(sharp, new ItemStack(Items.DIAMOND_CHESTPLATE),
                new ItemStack(Items.SHIELD), theatre);
        helper.assertTrue(bare > iron && iron > diamond && diamond > enchanted,
                "bare %.4f > iron %.4f > diamond %.4f > sharpened %.4f".formatted(bare, iron, diamond, enchanted));
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void theFallenAreGoneAndTheirTownGrieves(GameTestHelper helper) {
        helper.setBlock(new BlockPos(3, 2, 3), Merchant.DESK.get());
        helper.setBlock(new BlockPos(4, 2, 3), Blocks.CHEST);
        var depot = MerchantStock.at(helper.getLevel(), helper.absolutePos(new BlockPos(3, 2, 3)));
        GlobalPos capital = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(new BlockPos(3, 2, 3)));
        GlobalPos home = far(helper, 8_300_000, 8_000_000);
        var army = new dev.b28.enterprise.army.Army();
        for (int i = 0; i < 20; i++) {
            CompoundTag villager = new CompoundTag();
            villager.putUUID("UUID", java.util.UUID.randomUUID());
            army.enlist(villager, home, 0);
        }
        long t = 1_000_000;
        army.campaign(helper.getLevel(), depot, capital, t);
        army.campaign(helper.getLevel(), depot, capital, t + 30 * 24000L);
        var record = army.records().get(home);
        helper.assertTrue(record.fallen() > 0 && army.size() == 20 - record.fallen() - record.deserted(),
                "unarmed for a month, some fall and are gone: " + record.fallen());
        helper.assertTrue(record.days() > 0, "the rest serve on");
        helper.assertTrue(record.hungry() > 0 && record.deserted() >= 0, "unfed: they go hungry, and some desert");
        helper.succeed();
    }
}
