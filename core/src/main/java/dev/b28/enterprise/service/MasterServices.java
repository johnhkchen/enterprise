package dev.b28.enterprise.service;

import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.territory.Buildings;
import dev.b28.enterprise.territory.Mines;
import dev.b28.enterprise.territory.Smithing;
import dev.b28.enterprise.territory.Territory;
import dev.b28.enterprise.territory.Works;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.Holder;
import net.minecraft.core.component.DataComponentPredicate;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.Registries;
import net.minecraft.tags.EnchantmentTags;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.enchantment.Enchantment;
import net.minecraft.world.item.enchantment.EnchantmentHelper;
import net.minecraft.world.item.enchantment.ItemEnchantments;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.block.Blocks;

/**
 * What a master of each trade can do, most of it with another trade's help: a Cleric enchants the town's kit (with a
 * Librarian's books, or with lapis from a deep mine), a Librarian copies enchanted books, a Toolsmith sinks the mine
 * deep (shored by a Mason), an Armorer builds a blast furnace (with a Mason), a Weaponsmith keeps an armoury (with an
 * Armorer), a Farmer plants flower beds on the Mason's square. The masters' crafts (banners, a smokehouse, saddles)
 * are in {@link Crafts}; the master Mason's caravanserai is in {@link dev.b28.enterprise.territory.Buildings}.
 */
public final class MasterServices {
    public static final String CLERIC = "minecraft:cleric";
    public static final String ARMORER = "minecraft:armorer";
    public static final String WEAPONSMITH = "minecraft:weaponsmith";

    /** A Cleric enchants at this level for its own: 10 as a Journeyman, 20 as an Expert, 30 as a Master. */
    static int enchantingLevel(int skill) {
        return skill >= 5 ? 30 : skill >= 4 ? 20 : skill >= 3 ? 10 : 0;
    }

    /** An enchantment at the table costs a lapis and an emerald (bottles o' enchanting) for each 10 levels. */
    public static final double SERVICE_EMERALDS = 0.5;

    /** A master Armorer's blast furnace costs this much; flower beds take this many flowers. */
    public static final double BLAST_FURNACE_EMERALDS = 6;
    public static final int FLOWERS = 8;

    static final List<Item> FLOWER_ITEMS = List.of(Items.DANDELION, Items.POPPY, Items.BLUE_ORCHID, Items.ALLIUM,
            Items.AZURE_BLUET, Items.OXEYE_DAISY, Items.CORNFLOWER, Items.LILY_OF_THE_VALLEY);

    private MasterServices() {}

    static void register() {
        Services.register(new Service("enterprise:enchanting", CLERIC, Service.Where.HOME,
                "enchants its town's kit (from 10 levels as a Journeyman to 30 as a Master): with an enchanted book, or "
                        + "with lapis from a deep mine and bottles o' enchanting; enchanted kit works better and lasts",
                MasterServices::enchantingWanted, MasterServices::enchant));
        Services.register(new Service("enterprise:enchanting_abroad", CLERIC, Service.Where.ABROAD,
                "enchants the kit of a town without a Cleric, for its bottles o' enchanting and " + SERVICE_EMERALDS,
                MasterServices::enchantingWanted, MasterServices::enchant));
        Services.register(new Service("enterprise:copying", TownServices.LIBRARIAN, Service.Where.HOME,
                "as a Master, copies an enchanted book with a book and a lapis, while its kit wants enchanting",
                MasterServices::copyingWanted, MasterServices::copy));
        Services.register(new Service("enterprise:deep_mine", TownServices.TOOLSMITH, Service.Where.HOME,
                "as a Master, with a Mason to shore it, sinks the mine deep: lapis too, for enchanting ("
                        + Mines.DEEP_EMERALDS + " emeralds)",
                visit -> investWanted(visit, 5, TownServices.MASON, Mines.DEEP_EMERALDS)
                        && visit.territory().mine(visit.bell().get()).isPresent()
                        && !visit.territory().deepMine(visit.bell().get()),
                visit -> invest(visit, Mines.DEEP_EMERALDS, bell -> visit.territory().sinkDeepMine(bell))));
        Services.register(new Service("enterprise:blast_furnace", ARMORER, Service.Where.HOME,
                "as a Master, with a Mason, builds a blast furnace: twice the iron for a coal (" + BLAST_FURNACE_EMERALDS
                        + " emeralds)",
                visit -> investWanted(visit, 5, TownServices.MASON, BLAST_FURNACE_EMERALDS)
                        && !visit.territory().blastFurnace(visit.bell().get()),
                visit -> invest(visit, BLAST_FURNACE_EMERALDS, bell -> visit.territory().buildBlastFurnace(bell))));
        Services.register(new Service("enterprise:armoury", WEAPONSMITH, Service.Where.HOME,
                "as a Master, with an Armorer, keeps an armoury: its guards' swords, bows and armour last twice as long",
                visit -> visit.skill() >= 5 && visit.bell().filter(bell -> skill(visit, bell, ARMORER) >= 3
                        && visit.territory().armouryUntil(bell) < visit.now() + 12000).isPresent(),
                visit -> {
                    visit.territory().keepArmoury(visit.bell().orElseThrow(), visit.now() + 24000);
                    return 0;
                }));
        Services.register(new Service("enterprise:flower_beds", TownServices.FARMER, Service.Where.HOME,
                "as a Master, plants flower beds on the town's square (" + FLOWERS + " flowers): happier still",
                MasterServices::flowersWanted, MasterServices::plantFlowers));
        Services.register(Service.passive("enterprise:hunting_party", Territory.FLETCHER, Service.Where.HOME,
                "as a Master, leads a hunting party: twice the game"));
        Services.register(Service.passive("enterprise:deep_water", "minecraft:fisherman", Service.Where.ROAD,
                "as a Master, aboard a fishing fleet: half again the catch"));
        Services.register(Service.passive("enterprise:far_charting", TownServices.CARTOGRAPHER, Service.Where.HOME,
                "as a Master, charts twice as far: markets within " + 2 * TownServices.CHART_RANGE + " blocks"));
    }

    private static int skill(Visit visit, GlobalPos bell, String trade) {
        return Services.residents(visit.level(), bell).getOrDefault(trade, 0);
    }

    private static double spare(Visit visit) {
        return Treasury.available(visit.level(), visit.at().pos(), visit.stock(), visit.rate())
                - visit.territory().reserve(visit.at());
    }

    /** A master's one-time investment at home: at {@code level}, with a {@code partner} of skill 3, and the money. */
    private static boolean investWanted(Visit visit, int level, String partner, double emeralds) {
        return !visit.abroad() && !visit.planning() && visit.skill() >= level
                && visit.bell().filter(bell -> skill(visit, bell, partner) >= 3).isPresent() && spare(visit) >= emeralds;
    }

    private static double invest(Visit visit, double emeralds, java.util.function.Consumer<GlobalPos> done) {
        Treasury.withdraw(visit.level(), visit.at().pos(), visit.stock(), emeralds, Mint.get(visit.server()), visit.now());
        done.accept(visit.bell().orElseThrow());
        return 0;
    }

    // A Cleric's enchanting.

    /** Kit with no enchantment yet. */
    private static ItemCost plain(Item item) {
        return new ItemCost(item.builtInRegistryHolder(), 1,
                DataComponentPredicate.builder().expect(DataComponents.ENCHANTMENTS, ItemEnchantments.EMPTY).build());
    }

    /** What it would enchant: the first plain kit there is, else (for a master Librarian to copy) a book. */
    private static Optional<Item> target(Visit visit) {
        for (Smithing.Kit kit : Smithing.KITS) {
            if (visit.stock().count(plain(kit.item())) > 0) {
                return Optional.of(kit.item());
            }
        }
        boolean copier = visit.bell().filter(bell -> skill(visit, bell, TownServices.LIBRARIAN) >= 5).isPresent();
        return copier && visit.stock().count(new ItemCost(Items.ENCHANTED_BOOK, 1)) == 0
                && visit.stock().count(new ItemCost(Items.BOOK, 1)) > 0 ? Optional.of(Items.BOOK) : Optional.empty();
    }

    /** An enchanted book in stock with an enchantment for {@code stack}. */
    private static Optional<ItemStack> bookFor(Visit visit, ItemStack stack) {
        return visit.stock().copies(new ItemCost(Items.ENCHANTED_BOOK, 1)).stream()
                .filter(book -> book.getOrDefault(DataComponents.STORED_ENCHANTMENTS, ItemEnchantments.EMPTY).keySet()
                        .stream().anyMatch(e -> e.value().canEnchant(stack)))
                .findFirst();
    }

    private static double cost(Visit visit) {
        return enchantingLevel(visit.skill()) / 10.0 + (visit.abroad() ? SERVICE_EMERALDS : 0);
    }

    private static boolean enchantingWanted(Visit visit) {
        if (visit.planning() || visit.bell().isEmpty() || enchantingLevel(visit.skill()) == 0
                || visit.abroad() && Crafts.hasOwn(visit, CLERIC)) {
            return false;
        }
        Optional<Item> target = target(visit);
        if (target.isEmpty()) {
            return false;
        }
        boolean book = target.get() != Items.BOOK && bookFor(visit, new ItemStack(target.get())).isPresent();
        boolean lapis = visit.stock().count(new ItemCost(Items.LAPIS_LAZULI, 1)) >= enchantingLevel(visit.skill()) / 10;
        double money = visit.abroad() ? visit.budget() : spare(visit);
        return (book || lapis) && money >= cost(visit);
    }

    private static double enchant(Visit visit) {
        Item item = target(visit).orElseThrow();
        var stock = visit.stock();
        ItemStack stack = item == Items.BOOK ? stock.take(new ItemCost(Items.BOOK, 1), 1).get(0)
                : stock.take(plain(item), 1).get(0);
        Optional<ItemStack> book = item == Items.BOOK ? Optional.empty() : bookFor(visit, stack);
        int level = enchantingLevel(visit.skill());
        if (book.isPresent()) {
            // The book's enchantments that suit it, as an anvil would lay them on.
            stock.take(new ItemCost(Items.ENCHANTED_BOOK.builtInRegistryHolder(), 1,
                    DataComponentPredicate.builder().expect(DataComponents.STORED_ENCHANTMENTS,
                            book.get().get(DataComponents.STORED_ENCHANTMENTS)).build()), 1);
            for (var entry : book.get().get(DataComponents.STORED_ENCHANTMENTS).entrySet()) {
                Holder<Enchantment> enchantment = entry.getKey();
                if (enchantment.value().canEnchant(stack)
                        && EnchantmentHelper.isEnchantmentCompatible(stack.getEnchantments().keySet(), enchantment)) {
                    stack.enchant(enchantment, entry.getIntValue());
                }
            }
        } else {
            stock.take(new ItemCost(Items.LAPIS_LAZULI, 1), level / 10);
            var table = visit.level().registryAccess().registryOrThrow(Registries.ENCHANTMENT)
                    .getTag(EnchantmentTags.IN_ENCHANTING_TABLE);
            stack = EnchantmentHelper.enchantItem(visit.level().random, stack, level, visit.level().registryAccess(), table);
        }
        ItemStack rest = stock.insert(stack);
        if (!rest.isEmpty()) {
            dev.b28.enterprise.Enterprise.LOGGER.warn("No room for an enchanted {} at {}", item, visit.at().pos());
        }
        double cost = cost(visit);
        if (!visit.abroad()) {
            Treasury.withdraw(visit.level(), visit.at().pos(), stock, cost, Mint.get(visit.server()), visit.now());
            return 0;
        }
        return cost;
    }

    // A master Librarian's copies.

    private static boolean copyingWanted(Visit visit) {
        if (visit.planning() || visit.skill() < 5) {
            return false;
        }
        var stock = visit.stock();
        int books = stock.count(new ItemCost(Items.ENCHANTED_BOOK, 1));
        int plainKit = Smithing.KITS.stream().mapToInt(kit -> stock.count(plain(kit.item()))).sum();
        return books > 0 && books < plainKit && stock.count(new ItemCost(Items.BOOK, 1)) > 0
                && stock.count(new ItemCost(Items.LAPIS_LAZULI, 1)) > 0;
    }

    private static double copy(Visit visit) {
        var stock = visit.stock();
        ItemStack original = stock.copies(new ItemCost(Items.ENCHANTED_BOOK, 1)).get(0).copyWithCount(1);
        stock.take(new ItemCost(Items.BOOK, 1), 1);
        stock.take(new ItemCost(Items.LAPIS_LAZULI, 1), 1);
        stock.insert(original);
        return 0;
    }

    // A master Farmer's flower beds.

    private static int flowers(Visit visit) {
        return FLOWER_ITEMS.stream().mapToInt(f -> visit.stock().count(new ItemCost(f, 1))).sum();
    }

    private static boolean flowersWanted(Visit visit) {
        return !visit.planning() && !visit.abroad() && visit.skill() >= 5
                && visit.bell().filter(bell -> Works.get(visit.server()).built(bell, Buildings.SQUARE.id())
                        && !visit.territory().flowerBeds(bell)).isPresent()
                && flowers(visit) >= FLOWERS;
    }

    private static double plantFlowers(Visit visit) {
        GlobalPos bell = visit.bell().orElseThrow();
        int left = FLOWERS;
        List<ItemStack> taken = new java.util.ArrayList<>();
        for (Item flower : FLOWER_ITEMS) {
            for (ItemStack piece : visit.stock().take(new ItemCost(flower, 1), left)) {
                left -= piece.getCount();
                taken.add(piece);
            }
        }
        var square = Works.get(visit.server()).buildingsOf(bell).get(Buildings.SQUARE.id());
        if (square != null && square.placed()) {
            // Beds in the square's four inner corners and the middle of each side.
            int n = Buildings.SQUARE.size();
            BlockPos c = square.corner();
            List<BlockPos> beds = List.of(c.offset(1, 0, 1), c.offset(n - 2, 0, 1), c.offset(1, 0, n - 2),
                    c.offset(n - 2, 0, n - 2), c.offset(n / 2, 0, 1), c.offset(1, 0, n / 2), c.offset(n - 2, 0, n / 2),
                    c.offset(n / 2, 0, n - 2));
            for (int i = 0; i < beds.size(); i++) {
                Item flower = taken.isEmpty() ? Items.POPPY : taken.get(i % taken.size()).getItem();
                visit.level().setBlock(beds.get(i), Blocks.GRASS_BLOCK.defaultBlockState(), 3);
                if (flower instanceof net.minecraft.world.item.BlockItem block) {
                    visit.level().setBlock(beds.get(i).above(), block.getBlock().defaultBlockState(), 3);
                }
            }
        }
        visit.territory().plantFlowerBeds(bell);
        return 0;
    }
}
