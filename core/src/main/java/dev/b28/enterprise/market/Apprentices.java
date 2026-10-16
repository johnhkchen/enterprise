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
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.npc.Villager;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
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

    /**
     * The trades this desk's contracts need at work: those it's contracted to train, and those that make or sell what
     * it has contracted to deliver. Their lockers are its Merchant's first care.
     */
    public static java.util.Set<String> neededTrades(net.minecraft.server.MinecraftServer server, GlobalPos desk) {
        java.util.Set<String> trades = new java.util.HashSet<>();
        ContractBook book = ContractBook.get(server);
        book.trainingFor(desk).forEach(c -> trades.add(c.training().get().profession()));
        book.of(desk).stream().filter(c -> c.inForce() && c.seller().equals(desk))
                .forEach(c -> c.delivery().ifPresent(d -> trades.addAll(Procurement.suppliers(d.item()))));
        return trades;
    }

    /** The share of worth the buyer of this desk's training contract for this trade pays for the pupil's work (0: none). */
    static double offtake(ServerLevel level, GlobalPos desk, String profession) {
        return contractFor(level, desk, profession).flatMap(c -> c.training()).map(dev.b28.enterprise.contract.Training::offtake)
                .orElse(0.0);
    }

    static Optional<dev.b28.enterprise.contract.Contract> contractFor(ServerLevel level, GlobalPos desk, String profession) {
        return ContractBook.get(level.getServer()).trainingFor(desk).stream()
                .filter(c -> c.training().get().profession().equals(profession) && c.training().get().offtake() > 0)
                .findFirst();
    }

    /**
     * After a pupil's locker trades: what the Merchant paid it to make, under a training contract with an offtake, is
     * the buyer's. It's crated with a bill of lading made out to the buyer (stored, not stock, until the buyer's caravan
     * comes for it), and the Merchant draws a bill of exchange on the buyer for it, at the agreed share of its worth,
     * to be paid when the buyer's caravan is next here.
     */
    public static void afterTrade(ServerLevel level, BlockPos workstation, BlockPos deskPos, Map<String, ItemStack> madeBy) {
        GlobalPos desk = GlobalPos.of(level.dimension(), deskPos);
        MerchantBook merchants = MerchantBook.get(level.getServer());
        var villager = dev.b28.enterprise.merchant.Merchant.workingAt(level, workstation, dev.b28.enterprise.locker.Lockers.MERCHANT_RANGE);
        if (villager.isEmpty()) {
            return;
        }
        String profession = BuiltInRegistries.VILLAGER_PROFESSION.getKey(villager.get().getVillagerData().getProfession())
                .toString();
        var contract = contractFor(level, desk, profession);
        if (contract.isEmpty()) {
            return;
        }
        MerchantStock stock = MerchantStock.at(level, deskPos);
        List<ItemStack> work = new java.util.ArrayList<>();
        madeBy.forEach((key, made) -> {
            if (merchants.isTraining(desk, workstation, key) && !made.is(Items.EMERALD)) {
                work.addAll(stock.take(new ItemCost(made.getItem(), 1), made.getCount()));
            }
        });
        if (work.isEmpty()) {
            return;
        }
        GlobalPos buyer = contract.get().buyer();
        long now = level.getGameTime();
        if (!collectable(level.getServer(), desk, buyer)) {
            return; // no caravan of the buyer's calls, and no hub the town reaches runs there: the work is the town's
        }
        double worth = work.stream().mapToDouble(w -> dev.b28.enterprise.money.Prices.emeraldsEach(w.getItem()) * w.getCount())
                .sum() * contract.get().training().get().offtake();
        var bill = dev.b28.enterprise.cargo.Papers.billOfLading(contract.get().id(), desk, buyer,
                dev.b28.enterprise.cargo.Crates.describe(work), "EXW", 0, now);
        bill.data().putBoolean("settled", true); // paid for by the bill of exchange drawn with it
        for (ItemStack crate : dev.b28.enterprise.cargo.Crates.pack(level, desk, stock, work,
                dev.b28.enterprise.cargo.Papers.NONE.with(bill), now)) {
            stock.insert(crate);
        }
        dev.b28.enterprise.contract.Bills.file(stock, dev.b28.enterprise.contract.Bills.get(level.getServer())
                .draw(buyer, desk, worth, now, contract.get().id()));
        ContractBook.get(level.getServer()).noteOfftake(contract.get(), work.stream().mapToInt(ItemStack::getCount).sum(),
                worth, now);
    }

    /**
     * Can the buyer get the work, and the town its money: a caravan of the buyer's calls at the town, or the town's
     * caravans reach a freight hub that runs to the buyer (it will buy the bill and forward the goods).
     */
    public static boolean collectable(net.minecraft.server.MinecraftServer server, GlobalPos desk, GlobalPos buyer) {
        if (Stranded.calls(server, buyer, desk)) {
            return true;
        }
        for (var route : dev.b28.enterprise.caravan.CaravanLedger.get(server).routes()) {
            if (!route.from().equals(desk) || route.stopping()) {
                continue;
            }
            var hub = Freight.isHub(server, route.to()) ? java.util.Optional.of(route.to())
                    : Freight.stopOn(server, route).map(Freight.Stop::hub);
            if (hub.filter(h -> Stranded.reaches(server, h, buyer)).isPresent()) {
                return true;
            }
        }
        return false;
    }

    /**
     * The town's villager of this trade furthest along short of {@code target}, by its census: the one its training
     * contract is for.
     */
    static Optional<UUID> pupil(ServerLevel level, GlobalPos desk, String profession, int target) {
        return Territory.get(level.getServer()).bellOf(desk)
                .flatMap(bell -> Settlements.get(level.getServer()).roster(bell).values().stream()
                        .filter(r -> !r.baby() && r.profession().equals(profession) && r.level() < target)
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
                && pupil(level, desk, profession, target).map(villager.getUUID()::equals).orElse(true);
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
        // Nothing to buy in the warehouse (the coal and iron it learns on are wanted now; caravans bring them): the
        // Merchant pays for its work instead, if what it makes has somewhere to go. Bought by the contract's buyer
        // (an offtake), or wanted here, or at least with room to keep a restock's worth; what's wanted first.
        Map<Item, Integer> wanted = Procurement.wanted(level, desk, stock);
        boolean offtake = offtake(level, desk, profession) > 0;
        Optional<MerchantOffer> work = villager.getOffers().stream()
                .filter(o -> o.getBaseCostA().is(Items.EMERALD) && o.getItemCostB().isEmpty())
                .filter(o -> offtake || wanted.containsKey(o.getResult().getItem())
                        || stock.hasRoomFor(o.getResult().copyWithCount(o.getResult().getCount() * o.getMaxUses())))
                .min(Comparator.comparing((MerchantOffer o) -> !wanted.containsKey(o.getResult().getItem()))
                        .thenComparingInt(o -> LockerTrading.price(o, reputation)));
        if (work.isEmpty()) {
            for (MerchantOffer offer : villager.getOffers()) { // nowhere for its work to go: none bought
                String key = TradeKeys.of(offer);
                if (!key.endsWith(">minecraft:emerald") && merchants.isTraining(desk, workstation, key)) {
                    merchants.setEnabled(desk, workstation, key, false);
                    merchants.training(desk, workstation, key, false);
                }
            }
            return;
        }
        String key = TradeKeys.of(work.get());
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        // As far as it can spare (with what it was paid up front): a lesson at least.
        double spare = Treasury.available(level, deskPos, stock, rate) - Territory.get(level.getServer()).reserve(desk)
                + ContractBook.get(level.getServer()).advancesHeld(desk);
        boolean pay = !canBuy && spare >= LockerTrading.price(work.get(), reputation);
        if (pay && !merchants.isEnabled(desk, workstation, key)) {
            merchants.setEnabled(desk, workstation, key, true);
            merchants.training(desk, workstation, key, true);
        } else if (!pay && merchants.isTraining(desk, workstation, key)) {
            merchants.setEnabled(desk, workstation, key, false);
            merchants.training(desk, workstation, key, false);
        }
    }
}
