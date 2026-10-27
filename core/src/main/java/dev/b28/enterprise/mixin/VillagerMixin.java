package dev.b28.enterprise.mixin;

import dev.b28.enterprise.locker.LockerTrading;
import net.minecraft.world.entity.npc.Villager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(Villager.class)
abstract class VillagerMixin {
    /** The Java rules run the lockers (Java mode); in core mode the core does ({@code binding/adapters/LockerAdapter}). */
    @Unique
    private static boolean javaLockers() {
        return LockerTrading.javaRules();
    }

    /** Demand before a restock that only a locker asked for; null for ordinary restocks. */
    @Unique
    private int[] enterprise$demandBefore;

    @Inject(method = "restock", at = @At("HEAD"))
    private void enterprise$rememberDemand(CallbackInfo ci) {
        if (javaLockers()) {
            enterprise$demandBefore = LockerTrading.demandIfUnused(((Villager) (Object) this).getOffers());
        }
    }

    /** Locker trades run right after a restock (the Java rules'; in core mode the core trades the lockers). */
    @Inject(method = "restock", at = @At("TAIL"))
    private void enterprise$tradeWithLocker(CallbackInfo ci) {
        if (!javaLockers()) {
            return;
        }
        Villager villager = (Villager) (Object) this;
        if (enterprise$demandBefore != null) {
            LockerTrading.floorCooling(villager.getOffers(), enterprise$demandBefore);
            enterprise$demandBefore = null;
        }
        LockerTrading.afterRestock(villager);
    }

    /** Set while a villager's own brain ticks: the game's hiring (`AssignProfessionFromJobSite`) runs only there. */
    @Unique
    private static final ThreadLocal<Boolean> enterprise$ownTick = ThreadLocal.withInitial(() -> false);

    @Inject(method = "customServerAiStep", at = @At("HEAD"))
    private void enterprise$ownTickStarts(CallbackInfo ci) {
        enterprise$ownTick.set(true);
    }

    /** In core mode a town's hand takes a trade only by the core ({@code PeopleAdapter.keepToTheCore}). */
    @Inject(method = "customServerAiStep", at = @At("TAIL"))
    private void enterprise$keepToTheCore(CallbackInfo ci) {
        enterprise$ownTick.set(false);
        Villager villager = (Villager) (Object) this;
        if (javaLockers() || !(villager.level() instanceof net.minecraft.server.level.ServerLevel level)
                || villager.getBrain().getMemory(net.minecraft.world.entity.ai.memory.MemoryModuleType.POTENTIAL_JOB_SITE).isEmpty()) {
            return;
        }
        var host = dev.b28.enterprise.binding.CoreHost.of(level.getServer());
        if (host != null) {
            dev.b28.enterprise.binding.adapters.PeopleAdapter.keepToTheCore(level, villager, host.hands());
        }
    }

    /**
     * The game hiring a town's unemployed hand in its own brain tick (it spotted a free workstation and stood by it in
     * one tick): refused in core mode, and the job site it took let go. The binding's hirings never run there.
     */
    @Inject(method = "setVillagerData", at = @At("HEAD"), cancellable = true)
    private void enterprise$refuseTheGamesHiring(net.minecraft.world.entity.npc.VillagerData data, CallbackInfo ci) {
        Villager villager = (Villager) (Object) this;
        if (!enterprise$ownTick.get() || javaLockers()
                || villager.getVillagerData().getProfession() != net.minecraft.world.entity.npc.VillagerProfession.NONE
                || data.getProfession() == net.minecraft.world.entity.npc.VillagerProfession.NONE
                || data.getProfession() == net.minecraft.world.entity.npc.VillagerProfession.NITWIT
                || !dev.b28.enterprise.binding.adapters.PeopleAdapter.isHand(villager)) {
            return;
        }
        ci.cancel();
        if (villager.level() instanceof net.minecraft.server.level.ServerLevel level) {
            dev.b28.enterprise.binding.adapters.PeopleAdapter.keepToTheCore(level, villager,
                    dev.b28.enterprise.binding.CoreHost.of(level.getServer()).hands());
        }
    }

    /**
     * Vanilla restocks only after a player trade; a villager with a locker restocks on schedule anyway (the Java rules'
     * lockers: in core mode the core restocks a hand's locker by its own clock, and the villager's restocks are vanilla's).
     */
    @Inject(method = "needsToRestock", at = @At("RETURN"), cancellable = true)
    private void enterprise$restockForLocker(CallbackInfoReturnable<Boolean> cir) {
        if (javaLockers() && !cir.getReturnValueZ() && LockerTrading.hasLocker((Villager) (Object) this)) {
            cir.setReturnValue(true);
        }
    }
}
