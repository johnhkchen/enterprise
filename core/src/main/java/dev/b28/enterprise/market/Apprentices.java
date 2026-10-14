package dev.b28.enterprise.market;

import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.locker.LockerTrading;
import dev.b28.enterprise.merchant.MerchantBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.merchant.TradeKeys;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Treasury;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Territory;
import java.util.Comparator;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.MerchantOffer;

/**
 * A Merchant training its town's villagers, for the training contracts it holds: for each, its pupil is the town's
 * villager of that trade furthest along (by the census). At each of the pupil's restocks the Merchant switches on
 * every trade by which it buys (coal, iron, flint, diamonds: what a smith learns on), and while the warehouse holds
 * none of what it buys, pays for its work instead (what the town or its contracts want first, else the cheapest), so
 * it keeps learning, as far as the town can spare the money. Once the pupil has the level, or the contract is gone,
 * what the Merchant switched on for training goes off again; what it switched on when it hired the villager stays.
 */
public final class Apprentices {
    private Apprentices() {}

    /** The level this desk's Merchant is training this trade to, under the contracts it holds (0: none). */
    public static int target(ServerLevel level, GlobalPos desk, String profession) {
        return ContractBook.get(level.getServer()).trainingFor(desk).stream()
                .flatMap(c -> c.training().stream())
                .filter(t -> t.profession().equals(profession))
                .mapToInt(dev.b28.enterprise.contract.Training::level).max().orElse(0);
    }

    /** The town's villager of this trade furthest along, by its census: the one its training contract is for. */
    static Optional<UUID> pupil(ServerLevel level, GlobalPos desk, String profession) {
        return Territory.get(level.getServer()).bellOf(desk)
                .flatMap(bell -> Settlements.get(level.getServer()).roster(bell).values().stream()
                        .filter(r -> !r.baby() && r.profession().equals(profession))
                        .max(Comparator.comparingInt(Settlements.Resident::level)
                                .thenComparing(r -> r.id().toString(), Comparator.reverseOrder()))
                        .map(Settlements.Resident::id));
    }

    /** At a villager's restock, before its locker trades run: trains it, or stops training it. */
    public static void review(ServerLevel level, Villager villager, BlockPos workstation, BlockPos deskPos,
            int reputation) {
        GlobalPos desk = GlobalPos.of(level.dimension(), deskPos);
        MerchantBook merchants = MerchantBook.get(level.getServer());
        String profession = BuiltInRegistries.VILLAGER_PROFESSION.getKey(villager.getVillagerData().getProfession()).toString();
        int target = target(level, desk, profession);
        boolean training = villager.getVillagerData().getLevel() < target
                && pupil(level, desk, profession).map(villager.getUUID()::equals).orElse(true);
        if (!training) {
            for (MerchantOffer offer : villager.getOffers()) {
                String key = TradeKeys.of(offer);
                if (merchants.isTraining(desk, workstation, key)) {
                    merchants.setEnabled(desk, workstation, key, false);
                    merchants.training(desk, workstation, key, false);
                }
            }
            return;
        }
        MerchantStock stock = MerchantStock.at(level, deskPos);
        boolean canBuy = false;
        for (MerchantOffer offer : villager.getOffers()) {
            String key = TradeKeys.of(offer);
            if (key.endsWith(">minecraft:emerald")) {
                if (!merchants.isEnabled(desk, workstation, key)) {
                    merchants.setEnabled(desk, workstation, key, true);
                    merchants.training(desk, workstation, key, true);
                }
                canBuy |= stock.count(offer.getItemCostA()) >= LockerTrading.price(offer, reputation);
            }
        }
        // Nothing to buy in the warehouse: the Merchant pays for its work instead, what's wanted first.
        Map<Item, Integer> wanted = Procurement.wanted(level, desk, stock);
        Optional<MerchantOffer> work = villager.getOffers().stream()
                .filter(o -> o.getBaseCostA().is(Items.EMERALD) && o.getItemCostB().isEmpty())
                .min(Comparator.comparing((MerchantOffer o) -> !wanted.containsKey(o.getResult().getItem()))
                        .thenComparingInt(o -> LockerTrading.price(o, reputation)));
        if (work.isEmpty()) {
            return;
        }
        String key = TradeKeys.of(work.get());
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        double spare = Treasury.available(level, deskPos, stock, rate) - Territory.get(level.getServer()).reserve(desk);
        boolean pay = !canBuy && spare >= LockerTrading.price(work.get(), reputation) * work.get().getMaxUses();
        if (pay && !merchants.isEnabled(desk, workstation, key)) {
            merchants.setEnabled(desk, workstation, key, true);
            merchants.training(desk, workstation, key, true);
        } else if (!pay && merchants.isTraining(desk, workstation, key)) {
            merchants.setEnabled(desk, workstation, key, false);
            merchants.training(desk, workstation, key, false);
        }
    }
}
