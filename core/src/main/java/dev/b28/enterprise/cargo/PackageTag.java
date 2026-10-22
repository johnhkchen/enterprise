package dev.b28.enterprise.cargo;

import com.mojang.serialization.Codec;
import com.mojang.serialization.codecs.RecordCodecBuilder;
import io.netty.buffer.ByteBuf;
import java.util.List;
import net.minecraft.ChatFormatting;
import net.minecraft.network.chat.Component;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;

/**
 * The tag on an item that is one package of one of the core's pallets (docs/pallets.md section 4): the pallet's id,
 * the package's place on it and how many it has, its kind of packing, where the pallet is bound and under which
 * contract ({@code -1}: none), the pallet as it was when its packages were put in the world (the tick it came to its
 * warehouse and its digest of bill and packages), and the package's good (the core's number) and count. A pallet stays
 * invisible: these tags are the only trace of it on its
 * crates. No rules read them but the core's: the binding's {@code adapters/PalletAdapter} writes them from the core's
 * pallets and tells the core what it finds of them (a pallet carried whole or in part to a desk, a package lost); a tag
 * whose {@code since} isn't its pallet's any more, or whose good and count aren't among its packages, is of a pallet that
 * has moved or changed since, and is taken out where it's found.
 */
public record PackageTag(long pallet, int place, int of, String kind, String to, int contract, long since, long digest, long good,
        double count) {
    public static final Codec<PackageTag> CODEC = RecordCodecBuilder.create(i -> i.group(
            Codec.LONG.fieldOf("pallet").forGetter(PackageTag::pallet),
            Codec.INT.fieldOf("place").forGetter(PackageTag::place),
            Codec.INT.fieldOf("of").forGetter(PackageTag::of),
            Codec.STRING.fieldOf("kind").forGetter(PackageTag::kind),
            Codec.STRING.fieldOf("to").forGetter(PackageTag::to),
            Codec.INT.fieldOf("contract").forGetter(PackageTag::contract),
            Codec.LONG.fieldOf("since").forGetter(PackageTag::since),
            Codec.LONG.fieldOf("digest").forGetter(PackageTag::digest),
            Codec.LONG.fieldOf("good").forGetter(PackageTag::good),
            Codec.DOUBLE.fieldOf("count").forGetter(PackageTag::count)).apply(i, PackageTag::new));
    public static final StreamCodec<ByteBuf, PackageTag> STREAM_CODEC = ByteBufCodecs.fromCodec(CODEC);

    /** The same package, at {@code place} of {@code of} on its pallet as it is now ({@code since}, {@code digest}). */
    public PackageTag now(int place, int of, long since, long digest) {
        return new PackageTag(pallet, place, of, kind, to, contract, since, digest, good, count);
    }

    /** "1 of 3, to Wheatfell, contract 7" (and the kind, if it isn't a crate), for a tooltip. */
    public List<Component> describe() {
        Component line = contract >= 0
                ? Component.translatable("item.enterprise.package.under", place + 1, of, to, contract)
                : Component.translatable("item.enterprise.package.of", place + 1, of, to);
        Component pallet = Component.translatable("item.enterprise.package.pallet", this.pallet, kind);
        return List.of(line.copy().withStyle(ChatFormatting.GOLD), pallet.copy().withStyle(ChatFormatting.DARK_GRAY));
    }
}
