package dev.b28.enterprise.market;

import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.locker.Lockers;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.TradeKeys;
import dev.b28.enterprise.service.Crafts;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;

/**
 * Buying from the town's own villagers: much of what a town has contracted to sell, or been ordered, a villager of
 * the town sells for emeralds (a Weaponsmith swords, a Fletcher bows and arrows, an Armorer armour and shields, a
 * Librarian enchanted books, a Cleric lapis). The Merchant switches on its lockers' trades that sell what's wanted, so
 * the lockers buy it with the warehouse's emeralds at each restock, and switches them off again when it's no longer
 * wanted (only those it switched on itself).
 */
public final class Procurement {
    /** Who sells what in a village, of the goods a town is asked for: so a town with one can supply them. */
    public static final Map<Item, Set<String>> SELLERS = Map.ofEntries(
            Map.entry(Items.IRON_SWORD, Set.of("minecraft:weaponsmith")),
            Map.entry(Items.DIAMOND_SWORD, Set.of("minecraft:weaponsmith")),
            Map.entry(Items.IRON_CHESTPLATE, Set.of("minecraft:armorer")),
            Map.entry(Items.CHAINMAIL_CHESTPLATE, Set.of("minecraft:armorer")),
            Map.entry(Items.DIAMOND_CHESTPLATE, Set.of("minecraft:armorer")),
            Map.entry(Items.LEATHER_CHESTPLATE, Set.of("minecraft:leatherworker")),
            Map.entry(Items.IRON_HELMET, Set.of("minecraft:armorer")),
            Map.entry(Items.IRON_LEGGINGS, Set.of("minecraft:armorer")),
            Map.entry(Items.IRON_BOOTS, Set.of("minecraft:armorer")),
            Map.entry(Items.DIAMOND_HELMET, Set.of("minecraft:armorer")),
            Map.entry(Items.DIAMOND_LEGGINGS, Set.of("minecraft:armorer")),
            Map.entry(Items.DIAMOND_BOOTS, Set.of("minecraft:armorer")),
            Map.entry(Items.SHIELD, Set.of("minecraft:armorer")),
            Map.entry(Items.BOW, Set.of("minecraft:fletcher")),
            Map.entry(Items.ARROW, Set.of("minecraft:fletcher")),
            Map.entry(Items.ENCHANTED_BOOK, Set.of("minecraft:librarian")),
            Map.entry(Items.LAPIS_LAZULI, Set.of("minecraft:cleric")),
            Map.entry(Items.BOOK, Set.of("minecraft:librarian")),
            Map.entry(Items.BREAD, Set.of("minecraft:farmer")),
            Map.entry(Items.COOKED_PORKCHOP, Set.of("minecraft:butcher")), // an Apprentice's, for emeralds
            Map.entry(Items.COOKED_CHICKEN, Set.of("minecraft:butcher")),
            Map.entry(Items.RABBIT_STEW, Set.of("minecraft:butcher")));

    /** The level a villager must reach to sell it: diamond gear is a master smith's (enchanted, for emeralds). */
    public static final Map<Item, Integer> SELLER_LEVEL = Map.ofEntries(
            Map.entry(Items.DIAMOND_SWORD, 5), Map.entry(Items.DIAMOND_CHESTPLATE, 5), Map.entry(Items.DIAMOND_HELMET, 5),
            Map.entry(Items.DIAMOND_LEGGINGS, 4), Map.entry(Items.DIAMOND_BOOTS, 4),
            Map.entry(Items.CHAINMAIL_CHESTPLATE, 2), Map.entry(Items.SHIELD, 3), Map.entry(Items.BOW, 2),
            Map.entry(Items.ENCHANTED_BOOK, 1), Map.entry(Items.COOKED_PORKCHOP, 2), Map.entry(Items.COOKED_CHICKEN, 2));

    public static int sellerLevel(Item item) {
        return SELLER_LEVEL.getOrDefault(item, 1);
    }

    private Procurement() {}

    /** The trades that make or sell {@code item}: a town with one of them can supply it. */
    public static Set<String> suppliers(Item item) {
        Set<String> trades = new LinkedHashSet<>(SELLERS.getOrDefault(item, Set.of()));
        trades.addAll(Crafts.makers(item));
        return trades;
    }

    /** What the desk wants of each good beyond what it holds: its open orders and what it has contracted to deliver. */
    public static Map<Item, Integer> wanted(ServerLevel level, GlobalPos desk, dev.b28.enterprise.merchant.MerchantStock stock) {
        Map<Item, Integer> wanted = new LinkedHashMap<>(Exchange.get(level.getServer()).wanted(desk));
        // At the capital, what the sovereign's stockpile lacks: its own smiths and farmers sell to the depot.
        Sovereign.get(level.getServer()).shortfall(desk).forEach((item, n) ->
                wanted.merge(item, n + stock.count(new ItemCost(item, 1)), Integer::sum)); // (held is taken off below)
        ContractBook book = ContractBook.get(level.getServer());
        for (var contract : book.of(desk)) {
            contract.delivery().filter(d -> contract.seller().equals(desk) && contract.inForce() && !d.toOrder())
                    .ifPresent(d -> wanted.merge(d.item(), book.toDeliver(desk, d.item()), Math::max));
        }
        if (!Exchange.get(level.getServer()).jobs(desk).isEmpty()) {
            wanted.merge(Items.ENCHANTED_BOOK, 2, Math::max); // for commissions
            wanted.merge(Items.LAPIS_LAZULI, 4, Math::max);
        }
        wanted.replaceAll((item, count) -> Math.max(0, count - stock.count(new ItemCost(item, 1))));
        wanted.values().removeIf(count -> count <= 0);
        return wanted;
    }

    /** The Merchant buys for what its contracts want this soon, no further ahead. */
    public static final long PACE_TICKS = 2 * 24000L;

    /**
     * What the desk wants soon of each good beyond what it holds: its open orders, the sovereign's shortfall here, and
     * what its contracts in force want delivered within {@link #PACE_TICKS}. What its Merchant buys from its villagers
     * now, and spends coin on: not the whole of a long contract at once.
     */
    public static Map<Item, Integer> soon(ServerLevel level, GlobalPos desk, dev.b28.enterprise.merchant.MerchantStock stock) {
        Map<Item, Integer> soon = new LinkedHashMap<>(Exchange.get(level.getServer()).wanted(desk));
        Sovereign.get(level.getServer()).shortfall(desk).forEach((item, n) ->
                soon.merge(item, n + stock.count(new ItemCost(item, 1)), Integer::sum));
        long by = level.getGameTime() + PACE_TICKS;
        for (var contract : ContractBook.get(level.getServer()).of(desk)) {
            contract.delivery().filter(d -> contract.seller().equals(desk) && contract.inForce() && !d.toOrder())
                    .ifPresent(d -> soon.merge(d.item(), contract.owed(by), Integer::sum));
        }
        soon.replaceAll((item, count) -> Math.max(0, count - stock.count(new ItemCost(item, 1))));
        soon.values().removeIf(count -> count <= 0);
        return soon;
    }

    /**
     * Switches the desk's lockers' selling trades on for what's wanted (paid in emeralds), and off again for what isn't
     * (those it switched on). Needs the lockers' villagers loaded. Returns the trades switched on now.
     */
    /**
     * At one villager's restock (it's loaded, wherever its stall is): the Merchant switches on what it sells that's
     * wanted, and off what it switched on that isn't. Returns trades switched on.
     */
    public static int procureAt(ServerLevel level, GlobalPos desk, net.minecraft.core.BlockPos workstation,
            net.minecraft.world.entity.npc.Villager villager, dev.b28.enterprise.merchant.MerchantStock stock) {
        return procure(desk, workstation, villager, wanted(level, desk, stock), MerchantBook.get(level.getServer()),
                Exchange.get(level.getServer()));
    }

    private static int procure(GlobalPos desk, net.minecraft.core.BlockPos workstation,
            net.minecraft.world.entity.npc.Villager villager, Map<Item, Integer> wanted, MerchantBook book,
            Exchange exchange) {
        int switched = 0;
        for (var offer : villager.getOffers()) {
            if (!offer.getBaseCostA().is(Items.EMERALD)) {
                continue; // only what it sells for emeralds
            }
            String key = TradeKeys.of(offer);
            Item result = offer.getResult().getItem();
            boolean want = wanted.containsKey(result);
            boolean on = book.isEnabled(desk, workstation, key);
            if (want && !on) {
                book.setEnabled(desk, workstation, key, true);
                exchange.procured(desk, workstation, key, true);
                switched++;
            } else if (!want && on && exchange.isProcured(desk, workstation, key)) {
                book.setEnabled(desk, workstation, key, false);
                exchange.procured(desk, workstation, key, false);
            }
        }
        return switched;
    }

    public static int procure(ServerLevel level, GlobalPos desk, dev.b28.enterprise.merchant.MerchantStock stock) {
        if (dev.b28.enterprise.merchant.OffMap.get(level.getServer()).is(desk)) {
            return 0; // no villagers there
        }
        Map<Item, Integer> wanted = wanted(level, desk, stock);
        MerchantBook book = MerchantBook.get(level.getServer());
        Exchange exchange = Exchange.get(level.getServer());
        int switched = 0;
        int limit = Merchant.workingAt(level, desk.pos(), Lockers.MERCHANT_RANGE)
                .map(m -> Merchant.lockerLimit(m.getVillagerData().getLevel())).orElse(0);
        for (var locker : Lockers.near(level, desk.pos(), Lockers.MERCHANT_RANGE, limit)) {
            var villager = Merchant.workingAt(level, locker.workstation(), Lockers.MERCHANT_RANGE); // wherever it stands
            if (villager.isPresent()) {
                switched += procure(desk, locker.workstation(), villager.get(), wanted, book, exchange);
            }
        }
        if (switched > 0) {
            exchange.note(desk, level.getGameTime(), "switched on %d of its villagers' trades to buy %s".formatted(switched,
                    wanted.keySet().stream().map(i -> BuiltInRegistries.ITEM.getKey(i).getPath())
                            .collect(java.util.stream.Collectors.joining(", "))));
        }
        return switched;
    }
}
