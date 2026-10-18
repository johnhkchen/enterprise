package dev.b28.enterprise.binding;

import dev.b28.enterprise.Dials;
import dev.b28.enterprise.army.Army;
import dev.b28.enterprise.caravan.Squad;
import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.territory.Comforts;
import dev.b28.enterprise.territory.Husbandry;
import dev.b28.enterprise.territory.Labour;
import dev.b28.enterprise.territory.Territory;
import dev.b28.enterprise.territory.Workshop;
import dev.b28.enterprise.wire.ContentPack;
import dev.b28.enterprise.wire.GoodFlag;
import dev.b28.enterprise.wire.PackAmount;
import dev.b28.enterprise.wire.PackAnimal;
import dev.b28.enterprise.wire.PackDial;
import dev.b28.enterprise.wire.PackGood;
import dev.b28.enterprise.wire.PackRecipe;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.TreeSet;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

/**
 * The mod's content pack, as the core takes it at a new world (docs/architecture.md: "content packs are data, checked
 * by the core"): everything the game brings that the core doesn't know by itself, and nothing the core decides.
 *
 * <ul>
 * <li><b>Goods</b>: Minecraft's day, and a good for every item the mod prices ({@link Prices}) or its recipes and herds
 * use, in the order of their ids: worth what the mod values it at, a stack of what the item stacks to, its food if it's
 * food, the kind of comfort it is (the item tags under {@code enterprise:comforts/}, {@link Comforts}), whether it's a
 * crop farmers grow ({@link Territory#isCrop}) or hearty meat ({@link Army#MEATS}), and what a stack weighs
 * ({@link #WEIGHTS}: Minecraft has no weights, the pack says; a plain good's stack weighs 1, which is what a slot
 * holds, so weight binds only where the pack says a good is heavy).</li>
 * <li><b>Comforts and trades</b>: the kinds of comfort, and the trades a villager can have (the squad's, {@code none}
 * and {@code nitwit}), by name without the namespace.</li>
 * <li><b>Recipes and herds</b>: what the trades make ({@link Workshop#CONVERSIONS}) and the animals a town keeps
 * ({@link Husbandry#KINDS}), by the names of their goods. One list for the Java rules (Java mode) and the pack, until
 * the delete wave moves them here.</li>
 * <li><b>Dials</b>: the mod's settings ({@link Dials}) as the world starts with them. Where the mod's default differs
 * from the core's (the families' {@code birth_mood}, 0.5 here) the pack says so; a dial the mod has and the core has no
 * rule for ({@link #JAVA_ONLY}) isn't sent.</li>
 * </ul>
 *
 * <p>The core checks the whole pack ({@code Content::from_pack}) and refuses it with everything wrong in it: a recipe
 * naming a good the pack lacks, a comfort no tag defines, a dial the core hasn't. A pack is read once, when the society
 * is made: a loaded world keeps the pack it was made with, and the host reads its goods back from the core
 * ({@code Query.Goods}), never from this. A mod change that moves the pack therefore reaches new worlds only; a
 * running world's goods and recipes are the save's until the core's format moves.
 */
public final class ModPack {
    /** Minecraft's day, in ticks. */
    public static final long DAY_TICKS = 24_000;

    /**
     * What a full stack of an item weighs, where it isn't 1 (docs/pallets.md: a pallet holds 9 and a wagon 27). Ore,
     * coal, bricks, ingots, logs and hay blocks are a little heavy (2) and stone and sand heavier (3), mildly at first: a wagon of iron ingots carries
     * thirteen stacks and one of wool twenty-seven. Tools and armour stack to one and weigh a stack of one each.
     */
    public static final Map<String, Double> WEIGHTS = Map.ofEntries(
            Map.entry("minecraft:cobblestone", 3.0),
            Map.entry("minecraft:stone", 3.0),
            Map.entry("minecraft:sand", 3.0),
            Map.entry("minecraft:red_sand", 3.0),
            Map.entry("minecraft:raw_iron", 2.0),
            Map.entry("minecraft:raw_gold", 2.0),
            Map.entry("minecraft:coal", 2.0),
            Map.entry("minecraft:brick", 2.0),
            Map.entry("minecraft:hay_block", 2.0),
            Map.entry("minecraft:iron_ingot", 2.0),
            Map.entry("minecraft:gold_ingot", 2.0),
            Map.entry("minecraft:oak_log", 2.0),
            Map.entry("minecraft:spruce_log", 2.0),
            Map.entry("minecraft:jungle_log", 2.0),
            Map.entry("minecraft:acacia_log", 2.0));

    /**
     * The mod's dials that the core has no rule for yet, so no dial of that name: they stay the Java rules' own (Java
     * mode) and {@code /enterprise dial} in core mode doesn't list them. A new mod dial the core hasn't makes the core
     * refuse the pack, loudly, until it's added there or here.
     */
    public static final Set<String> JAVA_ONLY = Set.of("dealer_take_profit", "discount_days", "correspondent_floor", "wagon_pays",
            "call_start", "pastures_max", "feed_days");

    private ModPack() {}

    /** The name of a game id in the core: without its namespace ({@code minecraft:farmer} is {@code farmer}). */
    static String bare(String id) {
        return id.substring(id.indexOf(':') + 1);
    }

    private static String id(Item item) {
        return BuiltInRegistries.ITEM.getKey(item).toString();
    }

    public static ContentPack of() {
        List<PackRecipe> recipes = recipes();
        List<PackAnimal> animals = animals();

        // The items the pack's goods are: the mod's priced ones, and any a recipe or a herd uses that it doesn't price.
        TreeSet<String> ids = new TreeSet<>(Prices.listed());
        for (PackRecipe r : recipes) {
            r.inputs().forEach(a -> ids.add(a.good()));
            ids.add(r.output());
        }
        for (PackAnimal a : animals) {
            ids.addAll(a.feed());
            a.meat().ifPresent(ids::add);
            a.wool().ifPresent(ids::add);
            a.hide().ifPresent(ids::add);
        }
        ids.add(id(Items.HAY_BLOCK)); // a wagon's team eats wheat or hay

        List<PackGood> goods = new ArrayList<>();
        for (String id : ids) {
            Item item = BuiltInRegistries.ITEM.get(ResourceLocation.parse(id));
            if (item == Items.AIR) {
                continue; // priced, but not in this game (a mod that isn't loaded)
            }
            goods.add(good(id, item));
        }
        // A recipe or a herd may name an item this game lacks: the core would refuse the pack for the good it can't find.
        List<String> kinds = Comforts.kinds().stream().map(Comforts::name).toList();
        if (kinds.isEmpty()) {
            throw new IllegalStateException("no item tags under enterprise:comforts/: the tags aren't loaded yet");
        }
        return new ContentPack(DAY_TICKS, goods, kinds, trades(), recipes, animals, dials());
    }

    private static PackGood good(String id, Item item) {
        ItemStack one = item.getDefaultInstance();
        FoodProperties food = one.get(DataComponents.FOOD);
        List<GoodFlag> flags = new ArrayList<>();
        if (Territory.isCrop(id)) {
            flags.add(GoodFlag.Crop);
        }
        if (Army.MEATS.contains(item)) {
            flags.add(GoodFlag.Meat);
        }
        Optional<String> comfort = Comforts.kindOf(item).map(Comforts::name);
        return new PackGood(id, 1 / Prices.itemsPerEmerald(id), one.getMaxStackSize(),
                food == null ? 0 : food.nutrition(), food == null ? 0 : food.saturation(), comfort, flags,
                WEIGHTS.getOrDefault(id, 1.0));
    }

    /** The trades a villager can have: the squad's, in its order, then the two with no work. */
    static List<String> trades() {
        List<String> trades = new ArrayList<>(Squad.TRADES.stream().map(ModPack::bare).toList());
        trades.add(bare(Labour.NONE));
        trades.add(bare(Labour.NITWIT));
        return trades;
    }

    /** What the trades make: the mod's workshop conversions, in their order. */
    static List<PackRecipe> recipes() {
        List<PackRecipe> recipes = new ArrayList<>();
        for (Workshop.Conversion c : Workshop.CONVERSIONS) {
            List<PackAmount> inputs = new ArrayList<>();
            inputs.add(new PackAmount(id(c.input()), c.in()));
            if (c.input2() != null) {
                inputs.add(new PackAmount(id(c.input2()), c.in2()));
            }
            recipes.add(new PackRecipe(c.trades().stream().map(ModPack::bare).sorted().toList(), inputs, id(c.output()), c.out()));
        }
        return recipes;
    }

    /** The animals a town keeps: the mod's herds, a sheep's wool and a cow's hide as the mod's Shepherd and Butcher have them. */
    static List<PackAnimal> animals() {
        List<PackAnimal> animals = new ArrayList<>();
        for (Husbandry.Kind k : Husbandry.KINDS) {
            Optional<String> wool = k.id().equals("minecraft:sheep") ? Optional.of(id(Items.WHITE_WOOL)) : Optional.empty();
            Optional<String> hide = k.id().equals("minecraft:cow") ? Optional.of(id(Items.LEATHER)) : Optional.empty();
            animals.add(new PackAnimal(bare(k.id()), k.feed().stream().map(ModPack::id).toList(), k.feedPerDay(),
                    Optional.of(id(k.meat())), k.meatEach(), wool, hide));
        }
        return animals;
    }

    /** The mod's dials at their defaults, those the core has a rule for: where it differs from the core's, the core follows. */
    static List<PackDial> dials() {
        return Dials.all().stream().filter(d -> !JAVA_ONLY.contains(d.name)).map(d -> new PackDial(d.name, d.initial)).toList();
    }
}
