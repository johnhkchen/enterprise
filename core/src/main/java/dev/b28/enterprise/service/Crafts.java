package dev.b28.enterprise.service;

import dev.b28.enterprise.merchant.Warehouse;
import dev.b28.enterprise.territory.Smithing;
import dev.b28.enterprise.territory.Territory;
import dev.b28.enterprise.territory.Workshop;
import java.util.LinkedHashMap;
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
            int out, int perLevel, double feeEach, ToIntFunction<Visit> wanted) {}

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
                0.02, visit -> count(visit, Items.RAW_IRON)));
        recipe(kit("enterprise:hoes", TownServices.TOOLSMITH, "forges hoes for its town's farmers: they tend more",
                "forges hoes for a town's farmers from its iron", Smithing.HOE, 0.1));
        recipe(kit("enterprise:pickaxes", TownServices.TOOLSMITH, "forges pickaxes for its town's miners: more ore",
                "forges pickaxes for a town's miners from its iron", Smithing.PICKAXE, 0.15));
        recipe(kit("enterprise:swords", WEAPONSMITH,
                "arms its town's guards: the town can ask more of what going around would cost",
                "arms a town's guards from its iron", Smithing.SWORD, 0.1));
        recipe(kit("enterprise:armour", ARMORER, "armours its town's guards: the town can ask a higher toll",
                "armours a town's guards from its iron", Smithing.ARMOUR, 0.4));
    }

    private static Map<Item, Integer> ordered(Item a, int x, Item b, int y) {
        Map<Item, Integer> inputs = new LinkedHashMap<>();
        inputs.put(a, x);
        inputs.put(b, y);
        return inputs;
    }

    /** Kit for each of its wearers who lacks one, of iron. */
    private static Recipe kit(String id, String trade, String does, String doesAbroad, Smithing.Kit kit, double fee) {
        return new Recipe(id, trade, does, doesAbroad, Map.of(Items.IRON_INGOT, kit.ingots()), kit.item(), 1, 2, fee,
                visit -> visit.bell().map(bell -> {
                    Territory territory = visit.territory();
                    Map<String, Integer> workers = Territory.workers(visit.server(), bell);
                    int miners = territory.mine(bell).isPresent() ? workers.getOrDefault(TownServices.TOOLSMITH, 0) : 0;
                    int wearers = Smithing.wearers(kit, workers.getOrDefault(TownServices.FARMER, 0), miners,
                            territory.guards(bell));
                    return Math.max(0, wearers - count(visit, kit.item()));
                }).orElse(0));
    }

    /** Registers the recipe at home, and abroad (its id + "_abroad") for towns without the trade. */
    static void recipe(Recipe recipe) {
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
        for (Map.Entry<Item, Integer> input : recipe.inputs().entrySet()) {
            batches = Math.min(batches, spare(visit, input.getKey()) / input.getValue());
        }
        batches = Math.min(batches, 64 / recipe.out()); // at most a stack's worth at a time
        if (visit.abroad() && !visit.planning()) {
            batches = Math.min(batches, (int) Math.floor(visit.budget() / (recipe.feeEach() * recipe.out())));
        }
        return Math.max(0, batches);
    }

    static boolean wanted(Recipe recipe, Visit visit) {
        if (visit.bell().isEmpty() || visit.abroad() && hasOwn(visit, recipe.trade())) {
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
