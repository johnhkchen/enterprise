package dev.b28.enterprise.locker;

import dev.b28.enterprise.merchant.Merchant;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.TradeKeys;
import java.util.Comparator;
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
import net.neoforged.neoforge.items.IItemHandler;
import net.neoforged.neoforge.items.ItemHandlerHelper;

/** Runs a villager's enabled trades against its locker when it restocks, if a Merchant manages that locker. */
public final class LockerTrading {
    /**
     * Reputation a Merchant brings to locker trades, by Merchant level. Vanilla prices drop by reputation times the
     * trade's price multiplier; curing a zombie villager gives about 125, so a Master Merchant comes close.
     */
    private static final int[] MERCHANT_REPUTATION = {0, 30, 55, 80, 110};

    private LockerTrading() {}

    /** The Merchant at {@code desk} manages this locker. */
    public record Manager(BlockPos desk, Villager merchant) {}

    /** Cheap check for the restock hook: does this villager's workstation have a container touching it? */
    public static boolean hasLocker(Villager villager) {
        return villager.level() instanceof ServerLevel level
                && !Merchant.isMerchant(villager)
                && jobSite(villager, level).map(site -> !Lockers.touchingContainers(level, site).isEmpty()).orElse(false);
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

    /** Runs every enabled, fairly priced trade until it runs out of uses, inputs or room. Returns trades made. */
    public static int trade(ServerLevel level, Villager villager, BlockPos workstation, Manager manager) {
        Set<String> enabled = MerchantBook.get(level.getServer())
                .enabledTrades(GlobalPos.of(level.dimension(), manager.desk()), workstation);
        if (enabled.isEmpty()) {
            return 0;
        }
        IItemHandler locker = Lockers.inventory(level, Lockers.touchingContainers(level, workstation));
        int reputation = merchantReputation(manager.merchant().getVillagerData().getLevel());
        int made = 0;
        for (MerchantOffer offer : villager.getOffers()) {
            if (!enabled.contains(TradeKeys.of(offer)) || isInflated(offer)) {
                continue;
            }
            int priceA = price(offer, reputation);
            while (!offer.isOutOfStock() && tradeOnce(locker, offer, priceA)) {
                offer.increaseUses();
                rewardTradeXp(villager, offer.getXp());
                rewardTradeXp(manager.merchant(), offer.getXp());
                made++;
            }
        }
        return made;
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

    /** Vanilla's trade XP, minus the XP orbs: level-ups land a couple of seconds later, as after a player trade. */
    private static void rewardTradeXp(Villager villager, int xp) {
        villager.setVillagerXp(villager.getVillagerXp() + xp);
        if (villager.shouldIncreaseLevel()) {
            villager.updateMerchantTimer = 40;
            villager.increaseProfessionLevelOnUpdate = true;
        }
    }

    private static Optional<BlockPos> jobSite(Villager villager, ServerLevel level) {
        return villager.getBrain()
                .getMemory(MemoryModuleType.JOB_SITE)
                .filter(site -> site.dimension() == level.dimension())
                .map(GlobalPos::pos);
    }
}
