package dev.b28.enterprise.binding;

import dev.b28.enterprise.money.Prices;
import dev.b28.enterprise.territory.Workshop;
import dev.b28.enterprise.wire.PackAmount;
import dev.b28.enterprise.wire.PackRecipe;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.util.RandomSource;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.entity.npc.VillagerProfession;
import net.minecraft.world.entity.npc.VillagerTrades;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.crafting.AbstractCookingRecipe;
import net.minecraft.world.item.crafting.CraftingRecipe;
import net.minecraft.world.item.crafting.Ingredient;
import net.minecraft.world.item.crafting.Recipe;
import net.minecraft.world.item.crafting.RecipeHolder;
import net.minecraft.world.item.crafting.RecipeType;
import net.minecraft.world.item.trading.MerchantOffer;

/**
 * Which of the game's own recipes a town's villagers work: a trade may craft or smelt anything it deals in (buys or
 * sells in its vanilla trades), from the game's crafting-table and furnace recipes, modded ones too. On top of the mod's
 * own conversions ({@link Workshop#CONVERSIONS}, which keep their outputs), a few the game has no recipe for
 * ({@link #EXTRAS}), less any {@link #DENIED}. The core ranks them and runs those that pay (`workshop.rs`).
 *
 * <p>Only goods the economy already deals in: what it makes and everything that goes in are goods the mod prices (or
 * its own conversions and herds use), so the game's long tail (dyed beds, glazed terracotta, a mod's recycling) stays
 * out. A recipe is also left out where an input comes back from the bench (a bucket of milk), where it would undo
 * another recipe (wheat into hay and hay into wheat: a pair that would run in circles), or where it makes what goes
 * into it. Where an ingredient takes any of several items (planks of any wood), a priced one is used, the game's own
 * first. A furnace burns an eighth of a coal an item.
 */
public final class VillagerCrafts {
    /** What the game has no recipe for: a Librarian breaks a bookshelf into its three books. */
    public static final List<PackRecipe> EXTRAS = List.of(
            new PackRecipe(List.of("librarian"), List.of(new PackAmount("minecraft:bookshelf", 1)), "minecraft:book", 3));

    /** Recipes by output that no villager works, though its trade deals in it. */
    public static final Set<String> DENIED = Set.of();

    /** A furnace's coal an item smelted (a coal smelts eight). */
    static final double COAL_PER_SMELT = 0.125;

    /** Offers sampled of each trade listing: a listing's random choice (a dyed or enchanted item) shows its items. */
    private static final int SAMPLES = 4;

    private VillagerCrafts() {}

    /** The recipes a running game's villagers work, beyond the mod's own; none without a server. */
    public static List<PackRecipe> of(MinecraftServer server, Set<String> trades) {
        if (server == null) {
            return EXTRAS;
        }
        Map<String, Set<String>> dealsIn = dealsIn(server, trades);
        Set<String> theMods = new HashSet<>();
        // The goods the economy deals in: priced, or used by the mod's conversions, these extras or the herds.
        Set<String> goods = new HashSet<>(Prices.listed());
        for (Workshop.Conversion c : Workshop.CONVERSIONS) {
            theMods.add(id(c.output()));
            goods.add(id(c.output()));
            goods.add(id(c.input()));
            if (c.input2() != null) {
                goods.add(id(c.input2()));
            }
        }
        for (PackRecipe r : EXTRAS) {
            goods.add(r.output());
            r.inputs().forEach(a -> goods.add(a.good()));
        }
        for (var a : ModPack.animals()) {
            a.meat().ifPresent(goods::add);
            a.wool().ifPresent(goods::add);
            a.hide().ifPresent(goods::add);
        }
        // By recipe id, so the list is the same each start.
        TreeMap<String, PackRecipe> found = new TreeMap<>();
        for (RecipeHolder<CraftingRecipe> h : server.getRecipeManager().getAllRecipesFor(RecipeType.CRAFTING)) {
            add(found, h.id().toString(), h.value(), server, dealsIn, theMods, goods, 0);
        }
        for (RecipeHolder<?> h : server.getRecipeManager().getAllRecipesFor(RecipeType.SMELTING)) {
            add(found, h.id().toString(), h.value(), server, dealsIn, theMods, goods, COAL_PER_SMELT);
        }
        List<PackRecipe> all = new ArrayList<>(EXTRAS);
        List<PackRecipe> fixed = new ArrayList<>(EXTRAS);
        for (Workshop.Conversion c : Workshop.CONVERSIONS) {
            List<PackAmount> in = new ArrayList<>(List.of(new PackAmount(id(c.input()), c.in())));
            if (c.input2() != null) {
                in.add(new PackAmount(id(c.input2()), c.in2()));
            }
            fixed.add(new PackRecipe(List.of(), in, id(c.output()), c.out()));
        }
        List<PackRecipe> game = new ArrayList<>(found.values());
        for (PackRecipe r : game) {
            if (!undoes(r, game) && !undoes(r, fixed)) {
                all.add(r);
            }
        }
        return all;
    }

    /** Some other recipe makes one of this one's inputs out of what this one makes. */
    private static boolean undoes(PackRecipe r, List<PackRecipe> others) {
        for (PackRecipe o : others) {
            if (o != r && o.inputs().stream().anyMatch(a -> a.good().equals(r.output()))
                    && r.inputs().stream().anyMatch(a -> a.good().equals(o.output()))) {
                return true;
            }
        }
        return false;
    }

    private static void add(Map<String, PackRecipe> found, String key, Recipe<?> recipe, MinecraftServer server,
            Map<String, Set<String>> dealsIn, Set<String> theMods, Set<String> goods, double coal) {
        if (recipe.isSpecial() || !(recipe instanceof CraftingRecipe || recipe instanceof AbstractCookingRecipe)) {
            return;
        }
        ItemStack result = recipe.getResultItem(server.registryAccess());
        if (result.isEmpty()) {
            return;
        }
        String output = id(result.getItem());
        Set<String> by = dealsIn.get(output);
        if (by == null || !goods.contains(output) || theMods.contains(output) || DENIED.contains(output)) {
            return;
        }
        Map<String, Double> inputs = new LinkedHashMap<>();
        for (Ingredient ingredient : recipe.getIngredients()) {
            if (ingredient.isEmpty()) {
                continue;
            }
            Item item = pick(ingredient, goods);
            if (item == null || item.getDefaultInstance().hasCraftingRemainingItem()) {
                return; // nothing the economy deals in to put in, or it comes back from the bench (a bucket)
            }
            inputs.merge(id(item), 1.0, Double::sum);
        }
        if (inputs.isEmpty() || inputs.containsKey(output)) {
            return;
        }
        if (coal > 0 && !goods.contains(id(Items.COAL))) {
            return;
        }
        if (coal > 0) {
            inputs.merge(id(Items.COAL), coal * result.getCount(), Double::sum);
        }
        List<PackAmount> in = inputs.entrySet().stream().map(e -> new PackAmount(e.getKey(), e.getValue())).toList();
        found.put(key, new PackRecipe(List.copyOf(by), in, output, result.getCount()));
    }

    /** An ingredient's item among the economy's goods, the game's own first; null if it takes none of them. */
    private static Item pick(Ingredient ingredient, Set<String> goods) {
        Item any = null;
        for (ItemStack s : ingredient.getItems()) {
            String id = id(s.getItem());
            if (goods.contains(id)) {
                if (id.startsWith("minecraft:")) {
                    return s.getItem();
                }
                any = any == null ? s.getItem() : any;
            }
        }
        return any;
    }

    /**
     * What each of {@code trades} deals in, by item id: the items of the vanilla offers its listings make (costs and
     * results, emeralds aside), sampled with a villager of the trade. The trades that deal in each item, sorted.
     */
    public static Map<String, Set<String>> dealsIn(MinecraftServer server, Set<String> trades) {
        Map<String, Set<String>> out = new TreeMap<>();
        Villager villager = new Villager(EntityType.VILLAGER, server.overworld());
        for (var entry : VillagerTrades.TRADES.entrySet()) {
            VillagerProfession profession = entry.getKey();
            String trade = ModPack.bare(BuiltInRegistries.VILLAGER_PROFESSION.getKey(profession).toString());
            if (!trades.contains(trade)) {
                continue;
            }
            for (VillagerTrades.ItemListing[] level : entry.getValue().values()) {
                for (VillagerTrades.ItemListing listing : level) {
                    if (listing instanceof VillagerTrades.TreasureMapForEmeralds) {
                        continue; // it searches the world for a structure
                    }
                    for (int i = 0; i < SAMPLES; i++) {
                        MerchantOffer offer;
                        try {
                            offer = listing.getOffer(villager, RandomSource.create(i));
                        } catch (RuntimeException e) {
                            continue;
                        }
                        if (offer == null) {
                            continue;
                        }
                        for (ItemStack s : List.of(offer.getCostA(), offer.getCostB(), offer.getResult())) {
                            if (!s.isEmpty() && s.getItem() != Items.EMERALD) {
                                out.computeIfAbsent(id(s.getItem()), k -> new TreeSet<>()).add(trade);
                            }
                        }
                    }
                }
            }
        }
        villager.discard();
        return out;
    }

    private static String id(Item item) {
        return BuiltInRegistries.ITEM.getKey(item).toString();
    }
}
