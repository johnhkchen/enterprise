package dev.b28.enterprise.cargo;

import com.mojang.serialization.Codec;
import com.mojang.serialization.codecs.RecordCodecBuilder;
import io.netty.buffer.ByteBuf;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;

/**
 * A bundle of documents, the papers that travel with goods: a bill of lading (who shipped what to whom, under which
 * contract and terms: the receipt for the goods and the title to them), a copy of the contract itself, and later
 * bills of exchange and the like. Each document is a kind and its particulars, so later eras add kinds freely.
 */
public record Papers(List<Papers.Document> documents) {
    public static final String BILL_OF_LADING = "bill_of_lading";
    public static final String CONTRACT = "contract";
    public static final String BILL_OF_EXCHANGE = "bill_of_exchange";

    public static final Papers NONE = new Papers(List.of());

    /** One document: its kind, and its particulars. */
    public record Document(String kind, CompoundTag data) {
        public static final Codec<Document> CODEC = RecordCodecBuilder.create(i -> i.group(
                Codec.STRING.fieldOf("kind").forGetter(Document::kind),
                CompoundTag.CODEC.fieldOf("data").forGetter(Document::data)).apply(i, Document::new));
        public static final StreamCodec<ByteBuf, Document> STREAM_CODEC = StreamCodec.composite(
                ByteBufCodecs.STRING_UTF8, Document::kind, ByteBufCodecs.COMPOUND_TAG, Document::data, Document::new);
    }

    public static final Codec<Papers> CODEC = Document.CODEC.listOf().xmap(Papers::new, Papers::documents);
    public static final StreamCodec<ByteBuf, Papers> STREAM_CODEC =
            Document.STREAM_CODEC.apply(ByteBufCodecs.list()).map(Papers::new, Papers::documents);

    public Papers {
        documents = List.copyOf(documents);
    }

    public Papers with(Document document) {
        List<Document> more = new ArrayList<>(documents);
        more.add(document);
        return new Papers(more);
    }

    public Optional<Document> first(String kind) {
        return documents.stream().filter(d -> d.kind().equals(kind)).findFirst();
    }

    /** A bill of lading: the goods (a line of item counts), from the shipper to the consignee, under a contract. */
    public static Document billOfLading(int contract, GlobalPos shipper, GlobalPos consignee, String goods, String terms,
            int route, long issued) {
        Paperwork.drawn(shipper, 1);
        CompoundTag tag = new CompoundTag();
        tag.putInt("contract", contract);
        put(tag, "shipper", shipper);
        put(tag, "consignee", consignee);
        tag.putString("goods", goods);
        tag.putString("terms", terms);
        tag.putInt("route", route);
        tag.putLong("issued", issued);
        return new Document(BILL_OF_LADING, tag);
    }

    /** A copy of a contract: enough for the party it reaches to hold it and act on it. */
    public static Document contractCopy(int contract, String summary) {
        CompoundTag tag = new CompoundTag();
        tag.putInt("contract", contract);
        tag.putString("summary", summary);
        return new Document(CONTRACT, tag);
    }

    public static void put(CompoundTag tag, String key, GlobalPos pos) {
        GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, pos).ifSuccess(t -> tag.put(key, t));
    }

    public static Optional<GlobalPos> pos(CompoundTag tag, String key) {
        return GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get(key)).result();
    }

    /** The consignee a bill of lading names, if any. */
    public Optional<GlobalPos> consignee() {
        return first(BILL_OF_LADING).flatMap(d -> pos(d.data(), "consignee"));
    }

    /** The contract a bill of lading ships under, if any. */
    public Optional<Integer> contract() {
        return first(BILL_OF_LADING).map(d -> d.data().getInt("contract"));
    }

    /** One line per document, for a tooltip or the record. */
    public List<String> describe() {
        List<String> lines = new ArrayList<>();
        for (Document d : documents) {
            CompoundTag t = d.data();
            switch (d.kind()) {
                case BILL_OF_LADING -> lines.add("Bill of lading: %s, from %s to %s, contract #%d, %s, day %d".formatted(
                        t.getString("goods"), pos(t, "shipper").map(p -> p.pos().toShortString()).orElse("?"),
                        pos(t, "consignee").map(p -> p.pos().toShortString()).orElse("?"), t.getInt("contract"),
                        t.getString("terms"), t.getLong("issued") / 24000));
                case CONTRACT -> lines.add("Contract #%d: %s".formatted(t.getInt("contract"), t.getString("summary")));
                case BILL_OF_EXCHANGE -> lines.add("Bill of exchange #%d: %.2f emeralds, drawn on %s to pay %s, due day %d"
                        .formatted(t.getInt("bill"), t.getDouble("emeralds"),
                                pos(t, "drawee").map(p -> p.pos().toShortString()).orElse("?"),
                                pos(t, "payee").map(p -> p.pos().toShortString()).orElse("?"), t.getLong("due") / 24000));
                default -> lines.add(d.kind().replace('_', ' '));
            }
        }
        return lines;
    }
}
