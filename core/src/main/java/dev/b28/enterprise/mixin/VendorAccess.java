package dev.b28.enterprise.mixin;

import dev.ithundxr.createnumismatics.content.coins.DiscreteCoinBag;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The coins a Numismatics vendor holds from its sales: a town collects what its own shops took. */
@Mixin(value = VendorBlockEntity.class, remap = false)
public interface VendorAccess {
    @Accessor("inventory")
    DiscreteCoinBag enterprise$coins();
}
