package dev.b28.enterprise.locker;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.TradeKeys;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.ai.memory.MemoryModuleType;
import net.minecraft.world.entity.ai.village.poi.PoiManager;
import net.minecraft.world.entity.ai.village.poi.PoiRecord;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.item.trading.MerchantOffer;
import net.minecraft.world.item.trading.MerchantOffers;
import net.neoforged.neoforge.items.IItemHandler;
import net.neoforged.neoforge.items.ItemHandlerHelper;

/** Runs a villager's enabled trades against its locker when it restocks, if a Merchant manages that locker. */
public final class LockerTrading {
    /**
     * Reputation a Merchant brings to locker trades, by Merchant level. Vanilla prices drop by reputation times the
     * trade's price multiplier; curing a zombie villager gives about 125, so a Master Merchant comes close.
     */
    private static final int[] MERCHANT_REPUTATION = {0, 30, 55, 80, 110};

    /**
     * Merchant XP needed for Apprentice, Journeyman, Expert and Master: the points a player has at levels 15, 30, 45
     * and 55. Tuned by dev/experiments/merchant_ramp.py: a pen of four villagers fed 18 shulkers of goods manages all
     * four by about day 2 and makes its Merchant a Master in about 7 to 9 days (player levels 5 to 20 took about 1).
     */
    private static final int[] MERCHANT_LEVEL_XP = {315, 1395, 4020, 6895};

    private LockerTrading() {}

    /** The Merchant at {@code desk} manages this locker. */
    public record Manager(BlockPos desk, Villager merchant) {}

    /** Cheap check for the restock hook: does this villager's workstation have a container touching it? */
    public static boolean hasLocker(Villager villager) {
        return villager.level() instanceof ServerLevel level
                && !Merchant.isMerchant(villager)
                && jobSite(villager, level).map(site -> !Lockers.touchingContainers(level, site).isEmpty()).orElse(false);
    }

    /**
     * Each offer's demand if no offer has been used since the last restock, else null. Vanilla only restocks a
     * villager whose trades were used; any other restock is one a locker asked for.
     */
    public static int[] demandIfUnused(MerchantOffers offers) {
        return offers.stream().anyMatch(offer -> offer.getUses() > 0)
                ? null
                : offers.stream().mapToInt(MerchantOffer::getDemand).toArray();
    }

    /**
     * After a restock only a locker asked for: demand may cool to zero but not below, and demand that was already
     * negative stays put. Otherwise idle trades would sink without limit and never inflate for players again.
     */
    public static void floorCooling(MerchantOffers offers, int[] before) {
        for (int i = 0; i < offers.size() && i < before.length; i++) {
            MerchantOffer offer = offers.get(i);
            offer.demand = Math.max(offer.getDemand(), Math.min(before[i], 0));
        }
    }

    public static void afterRestock(Villager villager) {
        if (!(villager.level() instanceof ServerLevel level) || Merchant.isMerchant(villager)) {
            return;
        }
        jobSite(villager, level).ifPresent(workstation -> findManager(level, workstation, Lockers.MERCHANT_RANGE)
                .ifPresent(manager -> trade(level, villager, workstation, manager)));
    }

    /** The nearest Merchant whose managed lockers include this workstation. */
    public static Optional<Manager> findManager(ServerLevel level, BlockPos workstation, int range) {
        PoiManager pois = level.getPoiManager();
        pois.ensureLoadedAndValid(level, workstation, range);
        return pois.getInRange(type -> type.is(Merchant.DESK_POI.getKey()), workstation, range, PoiManager.Occupancy.ANY)
                .map(PoiRecord::getPos)
                .sorted(Comparator.comparingDouble((BlockPos desk) -> desk.distSqr(workstation)).thenComparing(desk -> desk))
                .flatMap(desk -> Merchant.workingAt(level, desk, range).stream()
                        .filter(merchant -> Lockers.near(level, desk, range, Merchant.lockerLimit(
                                        merchant.getVillagerData().getLevel()))
                                .stream()
                                .anyMatch(locker -> locker.workstation().equals(workstation)))
                        .map(merchant -> new Manager(desk, merchant)))
                .findFirst();
    }

    /**
     * The Merchant fills the locker from its stock, the villager runs every enabled, fairly priced trade until it
     * runs out of uses, inputs or room, and the Merchant carries the results back to stock. Returns trades made.
     */
    public static int trade(ServerLevel level, Villager villager, BlockPos workstation, Manager manager) {
        Set<String> enabled = MerchantBook.get(level.getServer())
                .enabledTrades(GlobalPos.of(level.dimension(), manager.desk()), workstation);
        if (enabled.isEmpty()) {
            return 0;
        }
        IItemHandler locker = Lockers.inventory(level, Lockers.touchingContainers(level, workstation));
        MerchantStock stock = MerchantStock.at(level, manager.desk());
        int reputation = merchantReputation(manager.merchant().getVillagerData().getLevel());
        // Best deal first: the fewest inputs per item received.
        List<MerchantOffer> offers = villager.getOffers().stream()
                .filter(offer -> enabled.contains(TradeKeys.of(offer)) && !isInflated(offer))
                .sorted(Comparator.comparingDouble(offer -> (double) price(offer, reputation) / offer.getResult().getCount()))
                .toList();
        int made = 0;
        for (MerchantOffer offer : offers) {
            int priceA = price(offer, reputation);
            int uses = offer.getMaxUses() - offer.getUses();
            supply(stock, locker, offer.getItemCostA(), priceA * uses);
            offer.getItemCostB().ifPresent(costB -> supply(stock, locker, costB, costB.count() * uses));
            while (!offer.isOutOfStock() && tradeOnce(locker, offer, priceA)) {
                offer.increaseUses();
                boolean levelledUp = rewardTradeXp(villager, offer.getXp());
                rewardMerchantXp(manager.merchant(), levelledUp);
                made++;
            }
        }
        for (MerchantOffer offer : offers) {
            collect(locker, stock, offer.getResult());
        }
        if (made > 0) {
            Enterprise.LOGGER.debug("Locker trades: {} at {} by a level {} villager", made,
                    workstation.toShortString(), villager.getVillagerData().getLevel());
        }
        return made;
    }

    /** Tops the locker up to {@code wanted} items matching {@code cost}, from the Merchant's stock. */
    private static void supply(MerchantStock stock, IItemHandler locker, ItemCost cost, int wanted) {
        int missing = wanted - count(locker, cost);
        if (missing <= 0) {
            return;
        }
        for (ItemStack piece : stock.take(cost, missing)) {
            ItemStack rest = ItemHandlerHelper.insertItemStacked(locker, piece, false);
            if (!rest.isEmpty()) {
                stock.insert(rest); // the locker is full: what didn't fit goes back
            }
        }
    }

    /** Moves every item like {@code result} from the locker to the Merchant's stock, as far as stock has room. */
    private static void collect(IItemHandler locker, MerchantStock stock, ItemStack result) {
        for (int slot = 0; slot < locker.getSlots(); slot++) {
            ItemStack stack = locker.getStackInSlot(slot);
            if (!ItemStack.isSameItemSameComponents(stack, result)) {
                continue;
            }
            ItemStack rest = stock.insert(locker.extractItem(slot, stack.getCount(), false));
            if (!rest.isEmpty()) {
                locker.insertItem(slot, rest, false);
                return;
            }
        }
    }

    /** Demand has pushed the price above its base. Lockers wait for it to cool off. */
    public static boolean isInflated(MerchantOffer offer) {
        return Mth.floor(offer.getItemCostA().count() * offer.getDemand() * offer.getPriceMultiplier()) > 0;
    }

    /** The first cost's count after the Merchant's discount, never below 1. */
    public static int price(MerchantOffer offer, int reputation) {
        ItemCost costA = offer.getItemCostA();
        int discount = Mth.floor(reputation * offer.getPriceMultiplier());
        return Mth.clamp(costA.count() - discount, 1, costA.itemStack().getMaxStackSize());
    }

    public static int merchantReputation(int merchantLevel) {
        return MERCHANT_REPUTATION[Mth.clamp(merchantLevel, 1, 5) - 1];
    }

    private static boolean tradeOnce(IItemHandler locker, MerchantOffer offer, int priceA) {
        ItemCost costA = offer.getItemCostA();
        Optional<ItemCost> costB = offer.getItemCostB();
        if (count(locker, costA) < priceA || costB.map(b -> count(locker, b) < b.count()).orElse(false)) {
            return false;
        }
        ItemStack result = offer.assemble();
        if (!ItemHandlerHelper.insertItemStacked(locker, result, true).isEmpty()) {
            return false;
        }
        take(locker, costA, priceA);
        costB.ifPresent(b -> take(locker, b, b.count()));
        ItemHandlerHelper.insertItemStacked(locker, result, false);
        return true;
    }

    private static int count(IItemHandler handler, ItemCost cost) {
        int total = 0;
        for (int slot = 0; slot < handler.getSlots(); slot++) {
            ItemStack stack = handler.getStackInSlot(slot);
            if (cost.test(stack)) {
                total += stack.getCount();
            }
        }
        return total;
    }

    private static void take(IItemHandler handler, ItemCost cost, int amount) {
        for (int slot = 0; slot < handler.getSlots() && amount > 0; slot++) {
            if (cost.test(handler.getStackInSlot(slot))) {
                amount -= handler.extractItem(slot, amount, false).getCount();
            }
        }
    }

    /**
     * Vanilla's trade XP, minus the XP orbs: level-ups land a couple of seconds later, as after a player trade.
     * Returns true if this trade earned the villager its next level.
     */
    private static boolean rewardTradeXp(Villager villager, int xp) {
        villager.setVillagerXp(villager.getVillagerXp() + xp);
        if (villager.shouldIncreaseLevel() && !villager.increaseProfessionLevelOnUpdate) {
            villager.updateMerchantTimer = 40;
            villager.increaseProfessionLevelOnUpdate = true;
            return true;
        }
        return false;
    }

    /** The XP a player would have picked up from this trade: 3 to 6, plus 5 when the villager levels up. */
    private static void rewardMerchantXp(Villager merchant, boolean villagerLevelledUp) {
        int xp = 3 + merchant.getRandom().nextInt(4) + (villagerLevelledUp ? 5 : 0);
        merchant.setVillagerXp(merchant.getVillagerXp() + xp);
        int level = merchant.getVillagerData().getLevel();
        if (level < 5 && merchant.getVillagerXp() >= merchantXpForNextLevel(level)
                && !merchant.increaseProfessionLevelOnUpdate) {
            merchant.updateMerchantTimer = 40;
            merchant.increaseProfessionLevelOnUpdate = true;
        }
    }

    /** Total Merchant XP needed to leave {@code level} (1 = Novice); 0 past Expert. */
    public static int merchantXpForNextLevel(int level) {
        return level >= 1 && level <= 4 ? MERCHANT_LEVEL_XP[level - 1] : 0;
    }

    private static Optional<BlockPos> jobSite(Villager villager, ServerLevel level) {
        return villager.getBrain()
                .getMemory(MemoryModuleType.JOB_SITE)
                .filter(site -> site.dimension() == level.dimension())
                .map(GlobalPos::pos);
    }
}
