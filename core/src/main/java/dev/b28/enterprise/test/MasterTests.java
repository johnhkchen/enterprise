package dev.b28.enterprise.test;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.settlement.Census;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Buildings;
import dev.b28.enterprise.territory.Smithing;
import dev.b28.enterprise.territory.Territory;
import dev.b28.enterprise.territory.Works;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.gametest.framework.GameTest;
import net.minecraft.gametest.framework.GameTestHelper;
import net.minecraft.world.item.EnchantedBookItem;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.enchantment.EnchantmentInstance;
import net.minecraft.world.item.enchantment.Enchantments;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;
import net.neoforged.neoforge.gametest.GameTestHolder;
import net.neoforged.neoforge.gametest.PrefixGameTestTemplate;

/** Masters' services: a five-villager town with its desk and chest in the arena, residents at the levels given. */
@GameTestHolder(Enterprise.MOD_ID)
@PrefixGameTestTemplate(false)
public final class MasterTests {
    private static final BlockPos BELL = new BlockPos(3, 2, 3);
    private static final BlockPos DESK = new BlockPos(5, 2, 5);

    /** {@code trades}: profession, level, profession, level... */
    private static GlobalPos town(GameTestHelper helper, Territory territory, Object... trades) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        GlobalPos bell = GlobalPos.of(helper.getLevel().dimension(), helper.absolutePos(BELL));
        Map<UUID, Settlements.Resident> roster = new java.util.LinkedHashMap<>();
        for (int i = 0; i < trades.length; i += 2) {
            UUID id = UUID.randomUUID();
            roster.put(id, new Settlements.Resident(id, 0, Optional.empty(), (String) trades[i], (Integer) trades[i + 1],
                    true, false, 0, false));
        }
        Settlements.get(helper.getLevel().getServer()).setRoster(bell, roster);
        territory.afterCensus(helper.getLevel(), helper.absolutePos(BELL), new Census(5, 5, 5, 5, 0), 1, List.of());
        return bell;
    }

    private static MerchantStock stock(GameTestHelper helper) {
        return MerchantStock.at(helper.getLevel(), helper.absolutePos(DESK));
    }

    private static void touch(GameTestHelper helper, Territory territory, long at) {
        territory.touch(helper.getLevel(), helper.absolutePos(DESK), stock(helper), at);
    }

    private static void forget(GameTestHelper helper, GlobalPos bell) {
        Settlements.get(helper.getLevel().getServer()).setRoster(bell, Map.of());
        Works.get(helper.getLevel().getServer()).forgetForTest(bell);
    }

    private static ItemStack book(GameTestHelper helper, net.minecraft.resources.ResourceKey<net.minecraft.world.item.enchantment.Enchantment> key,
            int level) {
        var holder = helper.getLevel().registryAccess().registryOrThrow(Registries.ENCHANTMENT).getHolderOrThrow(key);
        return EnchantedBookItem.createForEnchantment(new EnchantmentInstance(holder, level));
    }

    @GameTest(template = "floor")
    public static void aMasterClericEnchantsWithLapis(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, "minecraft:cleric", 5);
        stock(helper).insert(new ItemStack(Items.IRON_SWORD, 1));
        stock(helper).insert(new ItemStack(Items.LAPIS_LAZULI, 3));
        stock(helper).insert(new ItemStack(Items.EMERALD, 10));
        touch(helper, territory, helper.getLevel().getGameTime());
        var sword = stock(helper).copies(new ItemCost(Items.IRON_SWORD, 1));
        helper.assertTrue(sword.size() == 1 && sword.get(0).isEnchanted(), "the sword is enchanted at 30 levels");
        helper.assertTrue(stock(helper).count(new ItemCost(Items.LAPIS_LAZULI, 1)) == 0, "for three lapis");
        helper.assertTrue(stock(helper).count(new ItemCost(Items.EMERALD, 1)) == 7, "and three emeralds of bottles: "
                + stock(helper).count(new ItemCost(Items.EMERALD, 1)));
        // (the table's roll may give only Unbreaking: no better a blade, but it lasts)
        helper.assertTrue(Smithing.quality(sword.get(0)) > 1 || Smithing.wearRate(sword.get(0)) < 1,
                "enchanted kit is better kit: it works better or lasts longer, " + sword.get(0).getEnchantments());
        forget(helper, bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aClericLaysABooksEnchantmentOn(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, "minecraft:cleric", 3);
        stock(helper).insert(new ItemStack(Items.IRON_PICKAXE, 1));
        stock(helper).insert(book(helper, Enchantments.UNBREAKING, 3));
        stock(helper).insert(new ItemStack(Items.EMERALD, 10));
        touch(helper, territory, helper.getLevel().getGameTime());
        var pickaxe = stock(helper).copies(new ItemCost(Items.IRON_PICKAXE, 1)).get(0);
        var unbreaking = helper.getLevel().registryAccess().registryOrThrow(Registries.ENCHANTMENT)
                .getHolderOrThrow(Enchantments.UNBREAKING);
        helper.assertTrue(pickaxe.getEnchantments().getLevel(unbreaking) == 3, "Unbreaking III from the book");
        helper.assertTrue(stock(helper).count(new ItemCost(Items.ENCHANTED_BOOK, 1)) == 0, "the book is used");
        forget(helper, bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void unbreakingKitLastsLonger(GameTestHelper helper) {
        helper.setBlock(DESK, Merchant.DESK.get());
        helper.setBlock(DESK.east(), Blocks.CHEST);
        var unbreaking = helper.getLevel().registryAccess().registryOrThrow(Registries.ENCHANTMENT)
                .getHolderOrThrow(Enchantments.UNBREAKING);
        for (int i = 0; i < 2; i++) {
            ItemStack hoe = new ItemStack(Items.IRON_HOE);
            hoe.enchant(unbreaking, 3);
            stock(helper).insert(hoe);
        }
        int used = Smithing.wear(stock(helper), new HashMap<>(), 16, 2, 0, 0, false);
        helper.assertTrue(used == 0, "Unbreaking III: four times as long, " + used + " used in 16 days");
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aMasterLibrarianCopiesABook(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, "minecraft:librarian", 5);
        stock(helper).insert(book(helper, Enchantments.SHARPNESS, 2));
        stock(helper).insert(new ItemStack(Items.BOOK, 1));
        stock(helper).insert(new ItemStack(Items.LAPIS_LAZULI, 1));
        stock(helper).insert(new ItemStack(Items.IRON_SWORD, 1));
        stock(helper).insert(new ItemStack(Items.IRON_SWORD, 1));
        touch(helper, territory, helper.getLevel().getGameTime());
        helper.assertTrue(stock(helper).count(new ItemCost(Items.ENCHANTED_BOOK, 1)) == 2, "a copy for the second sword");
        forget(helper, bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aMasterToolsmithSinksTheMineDeep(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, Territory.TOOLSMITH, 5, "minecraft:mason", 3);
        territory.setMineForTest(bell, helper.absolutePos(new BlockPos(1, 1, 1)));
        stock(helper).insert(new ItemStack(Items.EMERALD, 30));
        long t = helper.getLevel().getGameTime();
        touch(helper, territory, t);
        helper.assertTrue(territory.deepMine(bell), "sunk deep, the Mason shoring it");
        touch(helper, territory, t + 24000);
        int lapis = stock(helper).count(new ItemCost(Items.LAPIS_LAZULI, 1));
        helper.assertTrue(lapis == 2, "a master brings up 2.5 lapis a day: " + lapis);
        forget(helper, bell);
        helper.succeed();
    }

    @GameTest(template = "floor")
    public static void aMasterFarmerPlantsFlowerBeds(GameTestHelper helper) {
        Territory territory = new Territory();
        GlobalPos bell = town(helper, territory, "minecraft:farmer", 5);
        Works works = Works.get(helper.getLevel().getServer());
        works.finish(works.startBuilding(bell, Buildings.SQUARE, helper.absolutePos(new BlockPos(1, 1, 1))));
        stock(helper).insert(new ItemStack(Items.DANDELION, 8));
        touch(helper, territory, helper.getLevel().getGameTime());
        helper.assertTrue(territory.flowerBeds(bell), "flower beds on the square");
        helper.assertTrue(stock(helper).count(new ItemCost(Items.DANDELION, 1)) == 0, "of the eight dandelions");
        forget(helper, bell);
        helper.succeed();
    }
}
