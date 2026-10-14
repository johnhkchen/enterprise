package dev.b28.enterprise.cargo;

import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Treasury;
import java.util.HashMap;
import java.util.Map;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * Paperwork: every document a Merchant draws up (a contract and its copy, a bill of lading, a bill of exchange) takes a
 * sheet of paper, an eighth of an ink sac, and a thirty-second of a quill (a feather). It's reckoned up when the market
 * is next touched: from its warehouse, as far as it holds them; for what it lacks, it pays a scribe, who brings his
 * own (a tenth of an emerald a document). So a busy market wants paper, ink and feathers, and caravans bring them:
 * ink from the coast (Fishermen's squid), feathers from the hunters.
 */
public final class Paperwork extends SavedData {
    private static final String NAME = "enterprise_paperwork";

    public static final double INK_PER_DOCUMENT = 1 / 8.0;
    public static final double QUILL_PER_DOCUMENT = 1 / 32.0;
    public static final double SCRIBE_EMERALDS = 0.1;
    /** A market with this many documents a day or more to draw up wants stationery on hand. */
    public static final double BUSY = 0.5;

    static final class Desk {
        double owed; // documents drawn up, not yet reckoned
        double inkOwed;
        double quillOwed;
        long documents;
        long paper;
        long ink;
        long quills;
        double scribeFees;
        long since = -1; // when it first drew up a document
    }

    private final Map<GlobalPos, Desk> desks = new HashMap<>();

    public static Paperwork get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Paperwork::new, Paperwork::load, null), NAME);
    }

    /** {@code count} documents drawn up at this market (reckoned when it's next touched). */
    public static void drawn(GlobalPos desk, int count) {
        MinecraftServer server = net.neoforged.neoforge.server.ServerLifecycleHooks.getCurrentServer();
        if (server == null || desk == null) {
            return;
        }
        Paperwork paperwork = get(server);
        Desk d = paperwork.desks.computeIfAbsent(desk, k -> new Desk());
        d.owed += count;
        d.documents += count;
        if (d.since < 0) {
            d.since = server.overworld().getGameTime();
        }
        paperwork.setDirty();
    }

    /** Does this market draw up enough papers to keep stationery on hand? */
    public boolean busy(GlobalPos desk, long now) {
        Desk d = desks.get(desk);
        if (d == null || d.since < 0) {
            return false;
        }
        double days = Math.max(1, (now - d.since) / 24000.0);
        return d.documents / days >= BUSY;
    }

    /**
     * The market's paperwork reckoned: paper, ink and quills from its warehouse for the documents since; a scribe paid
     * for those it lacked stationery for.
     */
    public void settle(ServerLevel level, GlobalPos desk, MerchantStock stock, long now) {
        Desk d = desks.get(desk);
        if (d == null || d.owed < 1) {
            return;
        }
        int documents = (int) Math.floor(d.owed);
        d.owed -= documents;
        d.inkOwed += documents * INK_PER_DOCUMENT;
        d.quillOwed += documents * QUILL_PER_DOCUMENT;
        int paper = take(stock, Items.PAPER, documents);
        int inkWanted = (int) Math.floor(d.inkOwed);
        int ink = take(stock, Items.INK_SAC, inkWanted);
        int quillsWanted = (int) Math.floor(d.quillOwed);
        int quills = take(stock, Items.FEATHER, quillsWanted);
        d.inkOwed -= inkWanted;
        d.quillOwed -= quillsWanted;
        d.paper += paper;
        d.ink += ink;
        d.quills += quills;
        int lacking = Math.max(documents - paper,
                Math.max((int) Math.round((inkWanted - ink) / INK_PER_DOCUMENT),
                        (int) Math.round((quillsWanted - quills) / QUILL_PER_DOCUMENT)));
        if (lacking > 0) {
            double fee = Math.min(lacking, documents) * SCRIBE_EMERALDS;
            Treasury.withdraw(level, desk.pos(), stock, fee, Mint.get(level.getServer()), now);
            d.scribeFees += fee;
        }
        setDirty();
    }

    private static int take(MerchantStock stock, Item item, int count) {
        if (count <= 0) {
            return 0;
        }
        return stock.take(new ItemCost(item, 1), count).stream().mapToInt(ItemStack::getCount).sum();
    }

    /** {documents, paper, ink, quills, scribe fees in emeralds} for a market. */
    public double[] book(GlobalPos desk) {
        Desk d = desks.get(desk);
        return d == null ? new double[5] : new double[] {d.documents, d.paper, d.ink, d.quills, d.scribeFees};
    }

    public Map<GlobalPos, double[]> books() {
        Map<GlobalPos, double[]> out = new HashMap<>();
        desks.keySet().forEach(desk -> out.put(desk, book(desk)));
        return out;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        desks.forEach((desk, d) -> {
            CompoundTag e = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> e.put("desk", t));
            e.putDouble("owed", d.owed);
            e.putDouble("inkOwed", d.inkOwed);
            e.putDouble("quillOwed", d.quillOwed);
            e.putLong("documents", d.documents);
            e.putLong("paper", d.paper);
            e.putLong("ink", d.ink);
            e.putLong("quills", d.quills);
            e.putDouble("scribeFees", d.scribeFees);
            e.putLong("since", d.since);
            list.add(e);
        });
        tag.put("desks", list);
        return tag;
    }

    public static Paperwork load(CompoundTag tag, HolderLookup.Provider registries) {
        Paperwork out = new Paperwork();
        for (Tag t : tag.getList("desks", Tag.TAG_COMPOUND)) {
            CompoundTag e = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, e.get("desk")).result().ifPresent(desk -> {
                Desk d = new Desk();
                d.owed = e.getDouble("owed");
                d.inkOwed = e.getDouble("inkOwed");
                d.quillOwed = e.getDouble("quillOwed");
                d.documents = e.getLong("documents");
                d.paper = e.getLong("paper");
                d.ink = e.getLong("ink");
                d.quills = e.getLong("quills");
                d.scribeFees = e.getDouble("scribeFees");
                d.since = e.contains("since") ? e.getLong("since") : -1;
                out.desks.put(desk, d);
            });
        }
        return out;
    }
}
