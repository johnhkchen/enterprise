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
    /** Demand before a restock that only a locker asked for; null for ordinary restocks. */
    @Unique
    private int[] enterprise$demandBefore;

    @Inject(method = "restock", at = @At("HEAD"))
    private void enterprise$rememberDemand(CallbackInfo ci) {
        enterprise$demandBefore = LockerTrading.demandIfUnused(((Villager) (Object) this).getOffers());
    }

    /** Locker trades run right after a restock. */
    @Inject(method = "restock", at = @At("TAIL"))
    private void enterprise$tradeWithLocker(CallbackInfo ci) {
        Villager villager = (Villager) (Object) this;
        if (enterprise$demandBefore != null) {
            LockerTrading.floorCooling(villager.getOffers(), enterprise$demandBefore);
            enterprise$demandBefore = null;
        }
        LockerTrading.afterRestock(villager);
    }

    /** Vanilla restocks only after a player trade; a villager with a locker restocks on schedule anyway. */
    @Inject(method = "needsToRestock", at = @At("RETURN"), cancellable = true)
    private void enterprise$restockForLocker(CallbackInfoReturnable<Boolean> cir) {
        if (!cir.getReturnValueZ() && LockerTrading.hasLocker((Villager) (Object) this)) {
            cir.setReturnValue(true);
        }
    }
}
