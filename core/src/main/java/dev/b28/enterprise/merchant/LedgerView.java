package dev.b28.enterprise.merchant;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;

/**
 * A town's books in short lines, as the Merchant's Ledger tab and {@code /enterprise ledger <desk>} show them (docs/alpha-plan.md
 * 3): a heading (the town, its day, mood and people), a till line, six sections of lines, and a footer. Each line names the
 * query its figures came from ({@link Line#source}), for bug reports. Built once, on the server, by {@code
 * binding/adapters/LedgerAdapter}; it holds text only, no rule. Empty in Java mode.
 */
public record LedgerView(String heading, String till, List<Section> sections, String footer) {
    public static final LedgerView EMPTY = new LedgerView("", "", List.of(), "");

    /** A line of words and the question the core answered it from ("CashBook", "Stock", ...). */
    public record Line(String text, String source) {
        public static final StreamCodec<RegistryFriendlyByteBuf, Line> STREAM_CODEC = StreamCodec.composite(
                ByteBufCodecs.STRING_UTF8, Line::text,
                ByteBufCodecs.STRING_UTF8, Line::source,
                Line::new);
    }

    /** One section: Money, Goods, People, Wagons, Papers or Mint. */
    public record Section(String name, List<Line> lines) {
        public static final StreamCodec<RegistryFriendlyByteBuf, Section> STREAM_CODEC = StreamCodec.composite(
                ByteBufCodecs.STRING_UTF8, Section::name,
                Line.STREAM_CODEC.apply(ByteBufCodecs.list()), Section::lines,
                Section::new);
    }

    public static final StreamCodec<RegistryFriendlyByteBuf, LedgerView> STREAM_CODEC = StreamCodec.composite(
            ByteBufCodecs.STRING_UTF8, LedgerView::heading,
            ByteBufCodecs.STRING_UTF8, LedgerView::till,
            Section.STREAM_CODEC.apply(ByteBufCodecs.list()), LedgerView::sections,
            ByteBufCodecs.STRING_UTF8, LedgerView::footer,
            LedgerView::new);

    /** A ledger that is only a word: why there are no books (the society is paused, no town here). */
    public static LedgerView note(String heading) {
        return new LedgerView(heading, "", List.of(), "");
    }

    public boolean isEmpty() {
        return sections.isEmpty();
    }

    /** The section called {@code name}, or null. */
    public Section section(String name) {
        return sections.stream().filter(s -> s.name().equals(name)).findFirst().orElse(null);
    }

    /** Every line of the ledger as the command prints it: the heading, the till, each section under its name, the footer. */
    public List<String> toText() {
        List<String> out = new ArrayList<>();
        if (isEmpty()) {
            out.add(heading.isBlank() ? "No ledger: the core keeps no town here." : heading);
            return out;
        }
        out.add(heading);
        out.add(till);
        for (Section s : sections) {
            out.add("== " + s.name() + " ==");
            s.lines().forEach(l -> out.add("  " + l.text()));
        }
        out.add(footer);
        return out;
    }
}
