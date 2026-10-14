package dev.b28.enterprise.market;

import dev.b28.enterprise.Enterprise;
import dev.b28.enterprise.money.Mint;
import dev.b28.enterprise.money.Prices;
import dev.ithundxr.createnumismatics.Numismatics;
import dev.ithundxr.createnumismatics.content.backend.BankAccount;
import dev.ithundxr.createnumismatics.content.bank.CardItem;
import dev.ithundxr.createnumismatics.content.vendor.VendorBlockEntity;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.enchantment.Enchantment;
import net.minecraft.world.item.enchantment.Enchantments;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.saveddata.SavedData;
import net.neoforged.neoforge.event.tick.ServerTickEvent;

/**
 * A sovereign at war, buying what it needs as anyone would: through Numismatics shops by a Merchant's Desk, set to buy
 * gold and war goods at war prices (more for enchanted goods, the stronger the better), paid from the royal treasury,
 * a bank account the sovereign mints coin into each day. The shops are ordinary vendors; what happens next is the
 * region's own doing: the desk's master Merchant takes the shops' orders on as contracts and sources them.
 */
public final class Sovereign extends SavedData {
    private static final String NAME = "enterprise_sovereign";

    /** A war order: so many of an item (enchanted so) a lot, at so many times its worth. */
    public record Order(Item item, int lot, double premium, List<java.util.Map.Entry<ResourceKey<Enchantment>, Integer>> enchantments) {}

    public static final List<Order> WAR_ORDERS = List.of(
            new Order(Items.GOLD_INGOT, 8, 3, List.of()),
            new Order(Items.IRON_SWORD, 1, 2, List.of()),
            new Order(Items.IRON_SWORD, 1, 4, List.of(java.util.Map.entry(Enchantments.SHARPNESS, 2))),
            new Order(Items.IRON_SWORD, 1, 6, List.of(java.util.Map.entry(Enchantments.SHARPNESS, 3))),
            new Order(Items.SHIELD, 1, 2, List.of()),
            new Order(Items.IRON_CHESTPLATE, 1, 2, List.of()),
            new Order(Items.IRON_CHESTPLATE, 1, 4, List.of(java.util.Map.entry(Enchantments.PROTECTION, 2))),
            new Order(Items.BOW, 1, 2, List.of()),
            new Order(Items.BOW, 1, 4, List.of(java.util.Map.entry(Enchantments.POWER, 2))),
            new Order(Items.ARROW, 16, 2, List.of()));

    private GlobalPos desk;
    private UUID treasury;
    private int mintPerDay;
    private long lastMint = -1;
    private final List<BlockPos> shops = new ArrayList<>();
    private final java.util.Map<String, Integer> received = new java.util.TreeMap<>(); // sent to the front, by good

    public static Sovereign get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Sovereign::new, Sovereign::load, null), NAME);
    }

    public Optional<GlobalPos> desk() {
        return Optional.ofNullable(desk);
    }

    public Optional<BankAccount> treasury() {
        return treasury == null ? Optional.empty() : Optional.ofNullable(Numismatics.BANK.getAccount(treasury));
    }

    public List<BlockPos> shops() {
        return List.copyOf(shops);
    }

    public int mintPerDay() {
        return mintPerDay;
    }

    /** What the war has taken from the shops, by good (enchantments named). */
    public java.util.Map<String, Integer> received() {
        return java.util.Map.copyOf(received);
    }

    /** The lot an order's shop asks for. */
    public static ItemStack lot(ServerLevel level, Order order) {
        ItemStack stack = new ItemStack(order.item(), order.lot());
        var registry = level.registryAccess().registryOrThrow(Registries.ENCHANTMENT);
        order.enchantments().forEach(e -> stack.enchant(registry.getHolderOrThrow(e.getKey()), e.getValue()));
        return stack;
    }

    /**
     * War: the royal treasury opens with a day's minting, and a row of buying shops goes up beside the desk, ten
     * blocks north of it, each with a card on the treasury. Returns the shops placed.
     */
    public int declare(ServerLevel level, GlobalPos at, int perDay) {
        peace(level);
        desk = at;
        mintPerDay = perDay;
        treasury = UUID.randomUUID();
        Numismatics.BANK.getOrCreateAccount(treasury, BankAccount.Type.PLAYER);
        mint(level, level.getGameTime());
        double rate = Mint.get(level.getServer()).rate(Mint.DEFAULT_FACTION, level.getGameTime());
        Block vendor = BuiltInRegistries.BLOCK.get(ResourceLocation.parse("numismatics:vendor"));
        Item card = BuiltInRegistries.ITEM.get(ResourceLocation.parse("numismatics:red_card"));
        for (int i = 0; i < WAR_ORDERS.size(); i++) {
            Order order = WAR_ORDERS.get(i);
            BlockPos pos = at.pos().offset(-9 + 2 * i, 0, -10);
            level.setBlock(pos, vendor.defaultBlockState(), 3);
            if (!(level.getBlockEntity(pos) instanceof VendorBlockEntity shop)) {
                continue;
            }
            ItemStack lot = lot(level, order);
            shop.setMode(VendorBlockEntity.Mode.BUY);
            shop.filterContainer.setItem(0, lot);
            Shops.setPrice(shop, (int) Math.ceil(Prices.emeraldsEach(order.item()) * order.lot() * order.premium() * rate));
            ItemStack bound = new ItemStack(card);
            CardItem.set(bound, treasury);
            shop.cardContainer.setItem(0, bound);
            shop.setChanged();
            shops.add(pos);
        }
        setDirty();
        Enterprise.LOGGER.info("War: the sovereign buys at {} through {} shops, minting {} spurs a day",
                at.pos().toShortString(), shops.size(), perDay);
        return shops.size();
    }

    /** Peace: the shops come down and the minting stops. */
    public void peace(ServerLevel level) {
        shops.forEach(pos -> level.setBlock(pos, Blocks.AIR.defaultBlockState(), 3));
        shops.clear();
        mintPerDay = 0;
        setDirty();
    }

    void mint(ServerLevel level, long now) {
        // What the shops bought goes to the front, leaving them room to buy more.
        for (BlockPos pos : shops) {
            if (level.isLoaded(pos) && level.getBlockEntity(pos) instanceof VendorBlockEntity shop) {
                for (int i = 0; i < shop.items.size(); i++) {
                    ItemStack stack = shop.items.get(i);
                    if (!stack.isEmpty()) {
                        received.merge(dev.b28.enterprise.contract.Delivery.of(stack, 1, 0).describe(), stack.getCount(),
                                Integer::sum);
                        shop.items.set(i, ItemStack.EMPTY);
                    }
                }
                shop.setChanged();
            }
        }
        treasury().ifPresent(account -> {
            account.deposit(mintPerDay);
            Mint.get(level.getServer()).mint(Mint.DEFAULT_FACTION, mintPerDay, now);
        });
        lastMint = now;
        setDirty();
    }

    /** The sovereign mints a day's coin into its treasury, each day of the war. */
    public static void onServerTick(ServerTickEvent.Post event) {
        MinecraftServer server = event.getServer();
        Sovereign sovereign = get(server);
        long now = server.overworld().getGameTime();
        if (sovereign.mintPerDay > 0 && sovereign.desk != null && now - sovereign.lastMint >= 24000) {
            ServerLevel level = server.getLevel(sovereign.desk.dimension());
            if (level != null) {
                sovereign.mint(level, now);
            }
        }
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        if (desk != null) {
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, desk).ifSuccess(t -> tag.put("desk", t));
        }
        if (treasury != null) {
            tag.putUUID("treasury", treasury);
        }
        tag.putInt("mintPerDay", mintPerDay);
        tag.putLong("lastMint", lastMint);
        tag.putLongArray("shops", shops.stream().mapToLong(BlockPos::asLong).toArray());
        CompoundTag got = new CompoundTag();
        received.forEach(got::putInt);
        tag.put("received", got);
        return tag;
    }

    public static Sovereign load(CompoundTag tag, HolderLookup.Provider registries) {
        Sovereign sovereign = new Sovereign();
        sovereign.desk = tag.contains("desk") ? GlobalPos.CODEC.parse(NbtOps.INSTANCE, tag.get("desk")).result().orElse(null)
                : null;
        sovereign.treasury = tag.hasUUID("treasury") ? tag.getUUID("treasury") : null;
        sovereign.mintPerDay = tag.getInt("mintPerDay");
        sovereign.lastMint = tag.getLong("lastMint");
        for (long pos : tag.getLongArray("shops")) {
            sovereign.shops.add(BlockPos.of(pos));
        }
        CompoundTag got = tag.getCompound("received");
        got.getAllKeys().forEach(k -> sovereign.received.put(k, got.getInt(k)));
        return sovereign;
    }
}
