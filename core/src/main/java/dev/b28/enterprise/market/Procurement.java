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
            Map.entry(Items.SHIELD, Set.of("minecraft:armorer")),
            Map.entry(Items.BOW, Set.of("minecraft:fletcher")),
            Map.entry(Items.ARROW, Set.of("minecraft:fletcher")),
            Map.entry(Items.ENCHANTED_BOOK, Set.of("minecraft:librarian")),
            Map.entry(Items.LAPIS_LAZULI, Set.of("minecraft:cleric")),
            Map.entry(Items.BOOK, Set.of("minecraft:librarian")),
            Map.entry(Items.BREAD, Set.of("minecraft:farmer")));

    /** The level a villager must reach to sell it: diamond gear is a master smith's (enchanted, for emeralds). */
    public static final Map<Item, Integer> SELLER_LEVEL = Map.of(
            Items.DIAMOND_SWORD, 5, Items.DIAMOND_CHESTPLATE, 5, Items.CHAINMAIL_CHESTPLATE, 2, Items.SHIELD, 3,
            Items.BOW, 2, Items.ENCHANTED_BOOK, 1);

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

    /**
     * Switches the desk's lockers' selling trades on for what's wanted (paid in emeralds), and off again for what isn't
     * (those it switched on). Needs the lockers' villagers loaded. Returns the trades switched on now.
     */
    public static int procure(ServerLevel level, GlobalPos desk, dev.b28.enterprise.merchant.MerchantStock stock) {
        Map<Item, Integer> wanted = wanted(level, desk, stock);
        MerchantBook book = MerchantBook.get(level.getServer());
        Exchange exchange = Exchange.get(level.getServer());
        int switched = 0;
        int limit = Merchant.workingAt(level, desk.pos(), Lockers.MERCHANT_RANGE)
                .map(m -> Merchant.lockerLimit(m.getVillagerData().getLevel())).orElse(0);
        for (var locker : Lockers.near(level, desk.pos(), Lockers.MERCHANT_RANGE, limit)) {
            var villager = Merchant.workingAt(level, locker.workstation(), 8);
            if (villager.isEmpty()) {
                continue;
            }
            for (var offer : villager.get().getOffers()) {
                if (!offer.getBaseCostA().is(Items.EMERALD)) {
                    continue; // only what it sells for emeralds
                }
                String key = TradeKeys.of(offer);
                Item result = offer.getResult().getItem();
                boolean want = wanted.containsKey(result);
                boolean on = book.isEnabled(desk, locker.workstation(), key);
                if (want && !on) {
                    book.setEnabled(desk, locker.workstation(), key, true);
                    exchange.procured(desk, locker.workstation(), key, true);
                    switched++;
                } else if (!want && on && exchange.isProcured(desk, locker.workstation(), key)) {
                    book.setEnabled(desk, locker.workstation(), key, false);
                    exchange.procured(desk, locker.workstation(), key, false);
                }
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
