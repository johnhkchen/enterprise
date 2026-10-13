package dev.b28.enterprise.mixin;

import dev.b28.enterprise.locker.LockerTrading;
import net.minecraft.world.entity.npc.Villager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(Villager.class)
abstract class VillagerMixin {
    /** Locker trades run right after a restock. */
    @Inject(method = "restock", at = @At("TAIL"))
    private void enterprise$tradeWithLocker(CallbackInfo ci) {
        LockerTrading.afterRestock((Villager) (Object) this);
    }

    /** Vanilla restocks only after a player trade; a villager with a locker restocks on schedule anyway. */
    @Inject(method = "needsToRestock", at = @At("RETURN"), cancellable = true)
    private void enterprise$restockForLocker(CallbackInfoReturnable<Boolean> cir) {
        if (!cir.getReturnValueZ() && LockerTrading.hasLocker((Villager) (Object) this)) {
            cir.setReturnValue(true);
        }
    }
}
