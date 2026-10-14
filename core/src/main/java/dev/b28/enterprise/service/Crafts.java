package dev.b28.enterprise.service;

import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.territory.Smithing;
import dev.b28.enterprise.territory.Territory;
import dev.b28.enterprise.territory.Workshop;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.ToIntFunction;
import net.minecraft.core.GlobalPos;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;

/**
 * A trade making goods from a town's stock while the town wants them: at home for its own town, and abroad for a town
 * without that trade of its own, which pays so much an item. Inputs a locker uses are kept to its par. Bookbinding,
 * smelting, and the smiths' tools, swords and armour.
 */
public final class Crafts {
    /**
     * {@code inputs} make {@code out} of {@code output}, {@code perLevel} batches a level each time at most, while the
     * town wants {@code wanted} more (items); abroad at {@code feeEach} emeralds an item.
     */
    public record Recipe(String id, String trade, String does, String doesAbroad, Map<Item, Integer> inputs, Item output,
            int out, int perLevel, double feeEach, ToIntFunction<Visit> wanted, int minLevel) {
        public Recipe(String id, String trade, String does, String doesAbroad, Map<Item, Integer> inputs, Item output,
                int out, int perLevel, double feeEach, ToIntFunction<Visit> wanted) {
            this(id, trade, does, doesAbroad, inputs, output, out, perLevel, feeEach, wanted, 1);
        }
    }

    public static final String SHEPHERD = "minecraft:shepherd";
    public static final String BUTCHER = "minecraft:butcher";
    public static final String LEATHERWORKER = "minecraft:leatherworker";

    public static final String ARMORER = "minecraft:armorer";
    public static final String WEAPONSMITH = "minecraft:weaponsmith";

    private Crafts() {}

    static void register() {
        recipe(new Recipe("enterprise:bookbinding", TownServices.LIBRARIAN,
                "binds books from paper and leather while its town is short of reading",
                "binds books for a town short of reading, from its paper and leather",
                ordered(Items.PAPER, 3, Items.LEATHER, 1), Items.BOOK, 1, 16, 0.05, Crafts::booksWanted));
        recipe(new Recipe("enterprise:smelting", ARMORER, "smelts its town's ore: raw iron and coal into iron",
                "smelts a town's ore, for one with a mine and no Armorer",
                ordered(Items.RAW_IRON, Smithing.RAW_PER_COAL, Items.COAL, 1), Items.IRON_INGOT, Smithing.RAW_PER_COAL, 4,
                0.02, visit -> blast(visit) ? 0 : count(visit, Items.RAW_IRON)));
        recipe(new Recipe("enterprise:gold_smelting", ARMORER, "smelts its town's gold ore: raw gold and coal into gold",
                "smelts a town's gold ore, for one with a mine and no Armorer",
                ordered(Items.RAW_GOLD, Smithing.RAW_PER_COAL, Items.COAL, 1), Items.GOLD_INGOT, Smithing.RAW_PER_COAL, 4,
                0.02, visit -> count(visit, Items.RAW_GOLD)));
        recipe(new Recipe("enterprise:arrows", TownServices.FLETCHER,
                "fletches arrows of flint, sticks and feathers, for what its town has contracted to sell",
                "fletches arrows for a town's contracts from its flint, sticks and feathers",
                Map.of(Items.FLINT, 1, Items.STICK, 1, Items.FEATHER, 1), Items.ARROW, 4, 4, 0.01,
                visit -> contracted(visit, Items.ARROW)));
        recipe(new Recipe("enterprise:stone_swords", WEAPONSMITH,
                "makes stone swords, for what its town has contracted to sell", "makes stone swords for a town's contracts",
                ordered(Items.COBBLESTONE, 2, Items.STICK, 1), Items.STONE_SWORD, 1, 2, 0.05,
                visit -> contracted(visit, Items.STONE_SWORD)));
        recipe(new Recipe("enterprise:wooden_swords", WEAPONSMITH,
                "makes wooden swords, for what its town has contracted to sell", "makes wooden swords for a town's contracts",
                ordered(Items.OAK_LOG, 1, Items.STICK, 2), Items.WOODEN_SWORD, 2, 2, 0.02,
                visit -> contracted(visit, Items.WOODEN_SWORD)));
        recipe(new Recipe("enterprise:shields", ARMORER,
                "makes shields of wood and iron, for what its town has contracted to sell",
                "makes shields for a town's contracts from its logs and iron",
                ordered(Items.OAK_LOG, 2, Items.IRON_INGOT, 1), Items.SHIELD, 1, 2, 0.1,
                visit -> contracted(visit, Items.SHIELD)));
        recipe(new Recipe("enterprise:blast_smelting", ARMORER,
                "smelts in its town's blast furnace: twice the iron for a coal, twice as fast",
                "smelts in a town's blast furnace, for one without an Armorer",
                ordered(Items.RAW_IRON, 2 * Smithing.RAW_PER_COAL, Items.COAL, 1), Items.IRON_INGOT,
                2 * Smithing.RAW_PER_COAL, 4, 0.02, visit -> blast(visit) ? count(visit, Items.RAW_IRON) : 0));
        recipe(kit("enterprise:hoes", TownServices.TOOLSMITH, "forges hoes for its town's farmers: they tend more",
                "forges hoes for a town's farmers from its iron", Smithing.HOE, 0.1));
        recipe(kit("enterprise:pickaxes", TownServices.TOOLSMITH, "forges pickaxes for its town's miners: more ore",
                "forges pickaxes for a town's miners from its iron", Smithing.PICKAXE, 0.15));
        recipe(kit("enterprise:swords", WEAPONSMITH,
                "arms its town's guards: the town can ask more of what going around would cost",
                "arms a town's guards from its iron", Smithing.SWORD, 0.1));
        recipe(kit("enterprise:armour", ARMORER, "armours its town's guards: the town can ask a higher toll",
                "armours a town's guards from its iron", Smithing.ARMOUR, 0.4));
        recipe(new Recipe("enterprise:banners", SHEPHERD,
                "weaves banners while its town is short of furnishings: a banner furnishes like twelve carpets",
                "weaves banners for a town short of furnishings, from its wool and sticks",
                ordered(Items.WHITE_WOOL, 6, Items.STICK, 1), Items.WHITE_BANNER, 1, 2, 0.1,
                visit -> comfortWanted(visit, "furnishings", Items.WHITE_BANNER), 5));
        for (var meat : List.of(Map.entry(Items.BEEF, Items.COOKED_BEEF), Map.entry(Items.PORKCHOP, Items.COOKED_PORKCHOP),
                Map.entry(Items.CHICKEN, Items.COOKED_CHICKEN), Map.entry(Items.MUTTON, Items.COOKED_MUTTON),
                Map.entry(Items.RABBIT, Items.COOKED_RABBIT))) {
            String name = net.minecraft.core.registries.BuiltInRegistries.ITEM.getKey(meat.getKey()).getPath();
            recipe(new Recipe("enterprise:smoking_" + name, BUTCHER,
                    "smokes " + name + " in its town's smokehouse while food is short: coal for fuel",
                    "smokes a town's " + name + " while its food is short",
                    ordered(meat.getKey(), 8, Items.COAL, 1), meat.getValue(), 8, 2, 0.01, Crafts::foodWanted, 5));
        }
        recipe(new Recipe("enterprise:saddles", LEATHERWORKER,
                "makes saddles of leather and iron: its caravans ride, a third faster",
                "makes saddles for a town's caravans from its leather and iron",
                ordered(Items.LEATHER, 5, Items.IRON_INGOT, 1), Items.SADDLE, 1, 1, 0.25, Crafts::saddlesWanted, 5));
        recipe(kit("enterprise:bows", TownServices.FLETCHER,
                "makes bows of sticks and string: for its hunters, then its guards (armed, as with swords)",
                "makes bows for a town's hunters and guards from its sticks and string", Smithing.BOW, 0.1));
    }

    private static Map<Item, Integer> ordered(Item a, int x, Item b, int y) {
        Map<Item, Integer> inputs = new LinkedHashMap<>();
        inputs.put(a, x);
        inputs.put(b, y);
        return inputs;
    }

    /** Kit for each of its wearers who lacks one, of iron. */
    private static Recipe kit(String id, String trade, String does, String doesAbroad, Smithing.Kit kit, double fee) {
        return new Recipe(id, trade, does, doesAbroad, kit.inputs(), kit.item(), 1, 2, fee,
                visit -> visit.bell().map(bell -> {
                    Territory territory = visit.territory();
                    Map<String, Integer> workers = Territory.workers(visit.server(), bell);
                    int miners = territory.mine(bell).isPresent() ? workers.getOrDefault(TownServices.TOOLSMITH, 0) : 0;
                    int wearers = Smithing.wearers(kit, workers.getOrDefault(TownServices.FARMER, 0), miners,
                            territory.guards(bell), workers.getOrDefault(TownServices.FLETCHER, 0));
                    // Its own wearers' kit, and what it has contracted to sell.
                    return Math.max(0, wearers + contracted(visit, kit.item()) - count(visit, kit.item()));
                }).orElse(0));
    }

    private static final List<Recipe> RECIPES = new java.util.ArrayList<>();

    /** The trades whose recipes make {@code item}. */
    public static java.util.Set<String> makers(Item item) {
        Services.all(); // registered
        java.util.Set<String> trades = new java.util.LinkedHashSet<>();
        RECIPES.stream().filter(r -> r.output() == item).forEach(r -> trades.add(r.trade()));
        return trades;
    }

    /** Registers the recipe at home, and abroad (its id + "_abroad") for towns without the trade. */
    static void recipe(Recipe recipe) {
        RECIPES.add(recipe);
        Services.register(new Service(recipe.id(), recipe.trade(), Service.Where.HOME, recipe.does(),
                visit -> wanted(recipe, visit), visit -> make(recipe, visit)));
        Services.register(new Service(recipe.id() + "_abroad", recipe.trade(), Service.Where.ABROAD,
                recipe.doesAbroad() + ", " + recipe.feeEach() + " emeralds each",
                visit -> wanted(recipe, visit), visit -> make(recipe, visit)));
    }

    /** What the town holds of {@code item}: its stock, or while planning, its last stocktake. */
    static int count(Visit visit, Item item) {
        return visit.planning() ? Warehouse.get(visit.server()).onHand(visit.at(), item)
                : visit.stock().count(new ItemCost(item, 1));
    }

    /** As {@link #count}, less what its lockers want of it. */
    static int spare(Visit visit, Item item) {
        Warehouse warehouse = Warehouse.get(visit.server());
        int kept = warehouse.capacity(visit.at(), item) > 0 ? warehouse.par(visit.at(), item, true, visit.now()) : 0;
        return Math.max(0, count(visit, item) - kept);
    }

    static boolean hasOwn(Visit visit, String trade) {
        return visit.bell().map(bell -> Services.residents(visit.level(), bell).containsKey(trade)).orElse(false);
    }

    /** Batches it would make now: as many as are wanted, its skill allows and the inputs cover (and abroad, paid for). */
    static int batches(Recipe recipe, Visit visit) {
        int batches = Math.min((recipe.wanted().applyAsInt(visit) + recipe.out() - 1) / recipe.out(),
                recipe.perLevel() * visit.skill());
        // Kit for the town's own hands comes before what its lockers sell; anything else leaves the lockers their share.
        boolean kit = Smithing.KITS.stream().anyMatch(k -> k.item() == recipe.output());
        for (Map.Entry<Item, Integer> input : recipe.inputs().entrySet()) {
            int have = kit ? count(visit, input.getKey()) : spare(visit, input.getKey());
            batches = Math.min(batches, have / input.getValue());
        }
        batches = Math.min(batches, 64 / recipe.out()); // at most a stack's worth at a time
        if (visit.abroad() && !visit.planning()) {
            batches = Math.min(batches, (int) Math.floor(visit.budget() / (recipe.feeEach() * recipe.out())));
        }
        return Math.max(0, batches);
    }

    static boolean wanted(Recipe recipe, Visit visit) {
        if (visit.bell().isEmpty() || visit.abroad() && hasOwn(visit, recipe.trade()) || visit.skill() < recipe.minLevel()) {
            return false;
        }
        return batches(recipe, visit) > 0;
    }

    static double make(Recipe recipe, Visit visit) {
        int batches = batches(recipe, visit);
        if (batches <= 0) {
            return 0;
        }
        var stock = visit.stock();
        Map<Item, Integer> taken = new LinkedHashMap<>();
        for (Map.Entry<Item, Integer> input : recipe.inputs().entrySet()) {
            taken.put(input.getKey(), stock.take(new ItemCost(input.getKey(), 1), batches * input.getValue()).stream()
                    .mapToInt(ItemStack::getCount).sum());
        }
        int made = batches;
        for (Map.Entry<Item, Integer> input : recipe.inputs().entrySet()) {
            made = Math.min(made, taken.get(input.getKey()) / input.getValue());
        }
        for (Map.Entry<Item, Integer> input : recipe.inputs().entrySet()) {
            int left = taken.get(input.getKey()) - made * input.getValue();
            if (left > 0) {
                stock.insert(new ItemStack(input.getKey(), left));
            }
        }
        int items = 0;
        int max = recipe.output().getDefaultMaxStackSize();
        for (int left = made * recipe.out(); left > 0; ) {
            int piece = Math.min(max, left);
            ItemStack rest = stock.insert(new ItemStack(recipe.output(), piece));
            items += piece - rest.getCount();
            left -= piece;
            if (!rest.isEmpty()) {
                // No room (unstackable kit takes a slot each): give back the inputs for what couldn't be stored.
                int unmade = (left + rest.getCount()) / recipe.out();
                recipe.inputs().forEach((input, n) -> {
                    for (int back = unmade * n; back > 0; back -= input.getDefaultMaxStackSize()) {
                        stock.insert(new ItemStack(input, Math.min(back, input.getDefaultMaxStackSize())));
                    }
                });
                break;
            }
        }
        return visit.abroad() ? items * recipe.feeEach() : 0;
    }

    /**
     * What a town needs of each input to make what it has contracted to sell (by the first recipe that makes each
     * good): what the caravans that serve it should bring it.
     */
    public static Map<Item, Integer> contractInputs(net.minecraft.server.MinecraftServer server, GlobalPos desk) {
        Services.all(); // registered
        var book = dev.b28.enterprise.contract.ContractBook.get(server);
        java.util.Set<Item> goods = new java.util.LinkedHashSet<>();
        for (var contract : book.of(desk)) {
            if (contract.inForce() && contract.seller().equals(desk) && contract.knownBy(desk)) {
                contract.delivery().filter(d -> !d.toOrder()).ifPresent(d -> goods.add(d.item()));
            }
        }
        Map<Item, Integer> needs = new LinkedHashMap<>();
        for (Item item : goods) {
            int owed = book.toDeliver(desk, item);
            RECIPES.stream().filter(r -> r.output() == item).findFirst().ifPresent(r -> {
                int batches = (owed + r.out() - 1) / r.out();
                r.inputs().forEach((input, n) -> needs.merge(input, batches * n, Integer::sum));
            });
        }
        return needs;
    }

    /** What the town has contracted to sell of {@code item} and must still deliver: work for its trades. */
    static int contracted(Visit visit, Item item) {
        int owed = dev.b28.enterprise.contract.ContractBook.get(visit.server()).toDeliver(visit.at(), item);
        return owed <= 0 ? 0 : Math.max(0, owed - (visit.planning() ? 0 : visit.stock().count(new ItemCost(item, 1))));
    }

    /** Has the town a blast furnace (a master Armorer's)? */
    private static boolean blast(Visit visit) {
        return visit.bell().map(bell -> visit.territory().blastFurnace(bell)).orElse(false);
    }

    /** Comforts of a kind the town could use, as items of {@code output}: while short, up to its par. */
    private static int comfortWanted(Visit visit, String kind, Item output) {
        if (visit.planning()) {
            return visit.bell().map(bell -> visit.territory().comfortsMet(bell).getOrDefault(kind, 0.0) < 1 ? 4 : 0)
                    .orElse(0);
        }
        return Workshop.comfortWanted(visit.territory(), visit.at(), visit.stock(), kind, output);
    }

    /** Cooked meat while the town is short of food (under twice the caravans' par). */
    private static int foodWanted(Visit visit) {
        if (visit.planning()) {
            return 0;
        }
        return Math.max(0, (2 * Warehouse.FOOD_PAR_POINTS - visit.stock().foodNutrition()) / 6);
    }

    /** A saddle for each of its routes, less those it has. */
    private static int saddlesWanted(Visit visit) {
        long routes = dev.b28.enterprise.caravan.CaravanLedger.get(visit.server()).routes().stream()
                .filter(r -> r.from().equals(visit.at())).count();
        return (int) Math.max(0, routes - count(visit, Items.SADDLE));
    }

    /** Books the town could use: while it's short of reading, up to its par. */
    private static int booksWanted(Visit visit) {
        GlobalPos bell = visit.bell().orElse(null);
        if (bell == null) {
            return 0;
        }
        if (visit.planning()) {
            return visit.territory().comfortsMet(bell).getOrDefault("reading", 0.0) < 1 ? 16 : 0;
        }
        return Workshop.comfortWanted(visit.territory(), visit.at(), visit.stock(), "reading", Items.BOOK);
    }
}
