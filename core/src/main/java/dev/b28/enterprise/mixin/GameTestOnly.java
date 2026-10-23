package dev.b28.enterprise.mixin;

import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.Set;
import java.util.stream.Collectors;
import net.minecraft.gametest.framework.GameTestRegistry;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * A game test run limited to some test classes ({@code just gametest LedgerTests,LockerTests}, John, 2026-10-23: a
 * change's own tests while it's built, the whole suite once at landing): the system property {@value #PROPERTY} names
 * them, by simple class name, and a test method of any other class isn't registered. Unset or blank, every test is.
 */
@Mixin(GameTestRegistry.class)
abstract class GameTestOnly {
    @Unique
    private static final String PROPERTY = "enterprise.gametest.only";

    @Unique
    private static final Set<String> ONLY = Arrays.stream(System.getProperty(PROPERTY, "").split(","))
            .map(String::trim).filter(s -> !s.isEmpty()).collect(Collectors.toUnmodifiableSet());

    @Inject(method = "register(Ljava/lang/reflect/Method;Ljava/util/Set;)V", at = @At("HEAD"), cancellable = true)
    private static void enterprise$only(Method testMethod, Set<String> allowedNamespaces, CallbackInfo ci) {
        if (!ONLY.isEmpty() && !ONLY.contains(testMethod.getDeclaringClass().getSimpleName())) {
            ci.cancel();
        }
    }
}
