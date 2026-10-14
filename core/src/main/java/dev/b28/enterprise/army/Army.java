package dev.b28.enterprise.army;

import dev.b28.enterprise.caravan.CaravanLedger;
import dev.b28.enterprise.caravan.Caravans;
import dev.b28.enterprise.contract.ContractBook;
import dev.b28.enterprise.merchant.MerchantStock;
import dev.b28.enterprise.settlement.Settlements;
import dev.b28.enterprise.territory.Territory;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.util.RandomSource;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.trading.ItemCost;
import net.minecraft.world.level.saveddata.SavedData;

/**
 * The sovereign's army, on the ledger: the soldiers its levies raised (villagers off the world while they serve), the
 * kit they draw from the capital's warehouse (its depot), what campaigning wears out, who falls, and what each town
 * gave. Each campaign day a soldier draws whatever kit it lacks, the best there is (a weapon: diamond, iron, stone or
 * wooden sword, or a bow with arrows; a chestplate; a shield), eats its ration (an army eats a lot; unfed, a soldier
 * runs more risk and may desert), wears its kit, and may fall: less often the better its kit,
 * and its enchantments suited to the enemy. The fallen are gone from their town for good, and it grieves. At peace
 * the rest walk home. Enchantments don't change what kit is; they change how it serves, and the tally counts that.
 */
public final class Army extends SavedData {
    private static final String NAME = "enterprise_army";

    /** Who the war is against: which enchantments tell. */
    public enum Theatre {
        FACTION("minecraft:sharpness", "minecraft:protection"),
        UNDEAD("minecraft:smite", "minecraft:protection"),
        NETHER("minecraft:fire_aspect", "minecraft:fire_protection");

        public final String weaponEnchant;
        public final String armourEnchant;

        Theatre(String weaponEnchant, String armourEnchant) {
            this.weaponEnchant = weaponEnchant;
            this.armourEnchant = armourEnchant;
        }
    }

    /** A campaign day's chance of falling, before kit. */
    public static final double DEATH_BASE = 0.02;
    /** An army eats: food points a soldier a day, from the capital's warehouse. */
    public static final int RATION = 8;
    /** Unfed, a soldier runs half again the risk, and may desert (walk home) at this chance a day. */
    public static final double HUNGRY_RISK = 1.5;
    public static final double DESERTION = 0.1;

    /** Arrows an archer shoots a day. */
    public static final int ARROWS_PER_DAY = 8;

    /** Weapons, best first: attack power, and how much a soldier carrying one is at risk (none: twice). */
    public static final List<Gear> WEAPONS = List.of(
            new Gear(Items.DIAMOND_SWORD, 7, 0.8, 12), new Gear(Items.IRON_SWORD, 6, 1.0, 10),
            new Gear(Items.STONE_SWORD, 5, 1.25, 6), new Gear(Items.WOODEN_SWORD, 4, 1.4, 4),
            new Gear(Items.BOW, 6, 1.1, 10));
    public static final List<Gear> ARMOUR = List.of(
            new Gear(Items.DIAMOND_CHESTPLATE, 8, 0.5, 30), new Gear(Items.IRON_CHESTPLATE, 6, 0.8, 20),
            new Gear(Items.CHAINMAIL_CHESTPLATE, 5, 1.0, 16), new Gear(Items.LEATHER_CHESTPLATE, 3, 1.2, 10));
    public static final Gear SHIELD = new Gear(Items.SHIELD, 0, 0.85, 12);
    public static final double UNARMED_RISK = 2.0;
    public static final double UNARMOURED_RISK = 1.6;

    /** A piece of kit: its power (attack, or armour points), the risk factor it carries, and the days it lasts. */
    public record Gear(Item item, int power, double risk, int days) {}

    /** A serving soldier. */
    static final class Soldier {
        CompoundTag villager;
        GlobalPos home;
        long enlisted;
        ItemStack weapon = ItemStack.EMPTY;
        ItemStack armour = ItemStack.EMPTY;
        ItemStack shield = ItemStack.EMPTY;
        double wornWeapon;
        double wornArmour;
        double wornShield;
        int arrows;
    }

    /** What a town gave the war. */
    public static final class Record {
        int raised;
        int fallen;
        int days;
        double strength; // soldier-days weighted by kit and its fitness for the enemy
        int hungry; // soldier-days unfed
        int deserted;
        public int hungry() { return hungry; }
        public int deserted() { return deserted; }
        public int raised() { return raised; }
        public int fallen() { return fallen; }
        public int days() { return days; }
        public double strength() { return strength; }
    }

    private final List<Soldier> soldiers = new ArrayList<>();
    private final Map<GlobalPos, Record> records = new LinkedHashMap<>();
    private final Map<String, Integer> issued = new java.util.TreeMap<>(); // kit issued, by item (and if enchanted)
    private final Map<String, Integer> enchantLevels = new java.util.TreeMap<>(); // levels of each enchantment issued
    private Theatre theatre = Theatre.FACTION;
    private long eaten; // food points the army has eaten
    private long lastDay = -1;

    public static Army get(MinecraftServer server) {
        return server.overworld()
                .getDataStorage()
                .computeIfAbsent(new SavedData.Factory<>(Army::new, Army::load, null), NAME);
    }

    public int size() {
        return soldiers.size();
    }

    public Theatre theatre() {
        return theatre;
    }

    public void setTheatre(Theatre theatre) {
        this.theatre = theatre;
        setDirty();
    }

    public Map<GlobalPos, Record> records() {
        return Map.copyOf(records);
    }

    public long eaten() {
        return eaten;
    }

    public Map<String, Integer> issued() {
        return Map.copyOf(issued);
    }

    public Map<String, Integer> enchantLevels() {
        return Map.copyOf(enchantLevels);
    }

    /** Soldiers serving from a town. */
    public int servingFrom(GlobalPos home) {
        return (int) soldiers.stream().filter(s -> s.home.equals(home)).count();
    }

    /** A recruit joins: off the world, its town's to count. */
    public void enlist(CompoundTag villager, GlobalPos home, long now) {
        Soldier soldier = new Soldier();
        soldier.villager = villager;
        soldier.home = home;
        soldier.enlisted = now;
        soldiers.add(soldier);
        records.computeIfAbsent(home, h -> new Record()).raised++;
        setDirty();
    }

    /** What kit the army will want on hand: a set for each soldier and half again, and arrows for its archers. */
    public Map<Item, Integer> wants() {
        Map<Item, Integer> wants = new LinkedHashMap<>();
        int men = soldiers.size();
        wants.put(Items.IRON_SWORD, men);
        wants.put(Items.IRON_CHESTPLATE, men);
        wants.put(Items.SHIELD, men / 2);
        wants.put(Items.ARROW, men * ARROWS_PER_DAY * 2);
        wants.put(Items.BREAD, men * RATION * 4 / 5); // four days' rations, as bread
        return wants;
    }

    /** A campaign day's chance of falling, in this kit, against this enemy. */
    public static double risk(ItemStack weapon, ItemStack armour, ItemStack shield, Theatre theatre) {
        Optional<Gear> w = gear(WEAPONS, weapon);
        Optional<Gear> a = gear(ARMOUR, armour);
        int edge = w.map(g -> level(weapon, g.item() == Items.BOW ? "minecraft:power" : theatre.weaponEnchant)).orElse(0);
        int guard = a.map(g -> level(armour, theatre.armourEnchant)).orElse(0);
        return DEATH_BASE
                * w.map(Gear::risk).orElse(UNARMED_RISK) * Math.max(0.6, 1 - 0.05 * edge)
                * a.map(Gear::risk).orElse(UNARMOURED_RISK) * Math.max(0.5, 1 - 0.06 * guard)
                * (shield.isEmpty() ? 1 : SHIELD.risk());
    }

    private static int nutrition(Item item) {
        var food = item.getDefaultInstance().get(net.minecraft.core.component.DataComponents.FOOD);
        return food == null ? 0 : food.nutrition();
    }

    static int level(ItemStack stack, String enchantment) {
        int level = 0;
        for (var entry : stack.getEnchantments().entrySet()) {
            if (entry.getKey().unwrapKey().map(k -> k.location().toString().equals(enchantment)).orElse(false)) {
                level = entry.getIntValue();
            }
        }
        return level;
    }

    static Optional<Gear> gear(List<Gear> kinds, ItemStack stack) {
        return kinds.stream().filter(g -> stack.is(g.item())).findFirst();
    }

    /** Draws the best of these kinds the depot has, enchanted ones first. */
    private ItemStack draw(MerchantStock depot, List<Gear> kinds) {
        for (Gear gear : kinds) {
            List<ItemStack> there = depot.copies(new ItemCost(gear.item(), 1));
            if (there.isEmpty()) {
                continue;
            }
            ItemStack best = there.stream().max(java.util.Comparator.comparingInt(s -> s.getEnchantments().size()))
                    .orElseThrow();
            var taken = depot.take(new ItemCost(gear.item().builtInRegistryHolder(), 1,
                    net.minecraft.core.component.DataComponentPredicate.allOf(best.getComponents())), 1);
            ItemStack piece = taken.isEmpty() ? depot.take(new ItemCost(gear.item(), 1), 1).stream().findFirst()
                    .orElse(ItemStack.EMPTY) : taken.get(0);
            if (!piece.isEmpty()) {
                String name = BuiltInRegistries.ITEM.getKey(piece.getItem()).getPath()
                        + (piece.isEnchanted() ? " (enchanted)" : "");
                issued.merge(name, 1, Integer::sum);
                piece.getEnchantments().entrySet().forEach(e -> e.getKey().unwrapKey().ifPresent(k ->
                        enchantLevels.merge(k.location().getPath(), e.getIntValue(), Integer::sum)));
                return piece;
            }
        }
        return ItemStack.EMPTY;
    }

    /**
     * The campaign's days since the last: each soldier draws what it lacks from the depot, wears it, serves (its
     * town's pay owed under its levy), and may fall.
     */
    public void campaign(ServerLevel level, MerchantStock depot, GlobalPos capital, long now) {
        if (lastDay < 0) {
            lastDay = now;
            setDirty();
            return;
        }
        int days = (int) ((now - lastDay) / 24000);
        if (days <= 0) {
            return;
        }
        lastDay += days * 24000L;
        MinecraftServer server = level.getServer();
        ContractBook book = ContractBook.get(server);
        Territory territory = Territory.get(server);
        RandomSource random = level.random;
        CaravanLedger ledger = CaravanLedger.get(server);
        for (int day = 0; day < days; day++) {
            // The day's rations, from the depot: as far as they go, the army is fed.
            int wanted = soldiers.size() * RATION;
            int got = wanted == 0 ? 0 : depot.eat(wanted).entrySet().stream()
                    .mapToInt(e -> e.getValue() * nutrition(e.getKey())).sum();
            eaten += got;
            double fed = wanted == 0 ? 1 : Math.min(1, (double) got / wanted);
            int fedSoldiers = (int) Math.floor(soldiers.size() * fed);
            int index = 0;
            for (var it = soldiers.iterator(); it.hasNext(); index++) {
                Soldier soldier = it.next();
                boolean hungry = index >= fedSoldiers;
                if (hungry && random.nextDouble() < DESERTION) {
                    // Deserted: it walks home, its kit with it lost to the army.
                    records.computeIfAbsent(soldier.home, h -> new Record()).deserted++;
                    it.remove();
                    int distance = (int) Math.sqrt(soldier.home.pos().distSqr(capital.pos()));
                    ledger.walkHome(soldier.home, soldier.villager, now + Caravans.travelTicks(distance));
                    continue;
                }
                if (soldier.weapon.isEmpty() || soldier.weapon.is(Items.BOW) && soldier.arrows <= 0) {
                    if (soldier.weapon.is(Items.BOW)) {
                        soldier.arrows += depot.take(new ItemCost(Items.ARROW, 1), ARROWS_PER_DAY * 4).stream()
                                .mapToInt(ItemStack::getCount).sum();
                    }
                    if (soldier.weapon.isEmpty()) {
                        soldier.weapon = draw(depot, WEAPONS);
                        soldier.wornWeapon = 0;
                        if (soldier.weapon.is(Items.BOW)) {
                            soldier.arrows += depot.take(new ItemCost(Items.ARROW, 1), ARROWS_PER_DAY * 4).stream()
                                    .mapToInt(ItemStack::getCount).sum();
                        }
                    }
                }
                if (soldier.armour.isEmpty()) {
                    soldier.armour = draw(depot, ARMOUR);
                    soldier.wornArmour = 0;
                }
                if (soldier.shield.isEmpty() && !soldier.weapon.is(Items.BOW)) {
                    soldier.shield = draw(depot, List.of(SHIELD));
                    soldier.wornShield = 0;
                }
                // How it serves today, and the risk it runs.
                Optional<Gear> weapon = gear(WEAPONS, soldier.weapon)
                        .filter(g -> g.item() != Items.BOW || soldier.arrows > 0);
                Optional<Gear> armour = gear(ARMOUR, soldier.armour);
                int edge = weapon.map(g -> level(soldier.weapon, g.item() == Items.BOW ? "minecraft:power"
                        : theatre.weaponEnchant)).orElse(0);
                double risk = risk(weapon.isPresent() ? soldier.weapon : ItemStack.EMPTY, soldier.armour, soldier.shield,
                        theatre) * (hungry ? HUNGRY_RISK : 1);
                Record record = records.computeIfAbsent(soldier.home, h -> new Record());
                record.days++;
                if (hungry) {
                    record.hungry++;
                }
                record.strength += weapon.map(Gear::power).orElse(1) * (1 + 0.1 * edge)
                        * (armour.map(Gear::power).orElse(0) + 4) / 10.0;
                for (var contract : book.of(soldier.home)) {
                    contract.levy().filter(l -> contract.inForce() && contract.buyer().equals(capital))
                            .ifPresent(l -> book.owe(contract, l.payPerDay()));
                }
                // Wear: unbreaking makes it last.
                if (weapon.isPresent()) {
                    if (soldier.weapon.is(Items.BOW)) {
                        soldier.arrows = Math.max(0, soldier.arrows - ARROWS_PER_DAY);
                    }
                    soldier.wornWeapon += 1.0 / (weapon.get().days() * (1 + level(soldier.weapon, "minecraft:unbreaking")));
                    if (soldier.wornWeapon >= 1) {
                        soldier.weapon = ItemStack.EMPTY;
                    }
                }
                if (armour.isPresent()) {
                    soldier.wornArmour += 1.0 / (armour.get().days() * (1 + level(soldier.armour, "minecraft:unbreaking")));
                    if (soldier.wornArmour >= 1) {
                        soldier.armour = ItemStack.EMPTY;
                    }
                }
                if (!soldier.shield.isEmpty()) {
                    soldier.wornShield += 1.0 / (SHIELD.days() * (1 + level(soldier.shield, "minecraft:unbreaking")));
                    if (soldier.wornShield >= 1) {
                        soldier.shield = ItemStack.EMPTY;
                    }
                }
                if (random.nextDouble() < risk) {
                    // Fallen: gone from its town for good, which grieves.
                    record.fallen++;
                    it.remove();
                    java.util.UUID id = soldier.villager.getUUID("UUID");
                    territory.bellOf(soldier.home).ifPresent(bell -> {
                        Settlements.get(server).remove(bell, id);
                        territory.mourn(bell, now);
                    });
                    dev.b28.enterprise.Enterprise.LOGGER.debug("A soldier from {} fell", soldier.home.pos().toShortString());
                }
            }
        }
        setDirty();
    }

    /** Peace: the soldiers walk home, as long as the road takes; their kit stays with the army. */
    public int demobilise(MinecraftServer server, GlobalPos capital, long now) {
        CaravanLedger ledger = CaravanLedger.get(server);
        int home = 0;
        for (Soldier soldier : soldiers) {
            int distance = (int) Math.sqrt(soldier.home.pos().distSqr(capital.pos()));
            ledger.walkHome(soldier.home, soldier.villager, now + Caravans.travelTicks(distance));
            home++;
        }
        soldiers.clear();
        lastDay = -1;
        setDirty();
        return home;
    }

    @Override
    public CompoundTag save(CompoundTag tag, HolderLookup.Provider registries) {
        ListTag list = new ListTag();
        for (Soldier s : soldiers) {
            CompoundTag entry = new CompoundTag();
            entry.put("villager", s.villager);
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, s.home).ifSuccess(t -> entry.put("home", t));
            entry.putLong("enlisted", s.enlisted);
            if (!s.weapon.isEmpty()) entry.put("weapon", s.weapon.save(registries));
            if (!s.armour.isEmpty()) entry.put("armour", s.armour.save(registries));
            if (!s.shield.isEmpty()) entry.put("shield", s.shield.save(registries));
            entry.putDouble("wornWeapon", s.wornWeapon);
            entry.putDouble("wornArmour", s.wornArmour);
            entry.putDouble("wornShield", s.wornShield);
            entry.putInt("arrows", s.arrows);
            list.add(entry);
        }
        tag.put("soldiers", list);
        ListTag recs = new ListTag();
        records.forEach((home, r) -> {
            CompoundTag entry = new CompoundTag();
            GlobalPos.CODEC.encodeStart(NbtOps.INSTANCE, home).ifSuccess(t -> entry.put("home", t));
            entry.putInt("raised", r.raised);
            entry.putInt("fallen", r.fallen);
            entry.putInt("days", r.days);
            entry.putDouble("strength", r.strength);
            entry.putInt("hungry", r.hungry);
            entry.putInt("deserted", r.deserted);
            recs.add(entry);
        });
        tag.put("records", recs);
        CompoundTag kit = new CompoundTag();
        issued.forEach(kit::putInt);
        tag.put("issued", kit);
        CompoundTag ench = new CompoundTag();
        enchantLevels.forEach(ench::putInt);
        tag.put("enchantLevels", ench);
        tag.putString("theatre", theatre.name());
        tag.putLong("lastDay", lastDay);
        tag.putLong("eaten", eaten);
        return tag;
    }

    public static Army load(CompoundTag tag, HolderLookup.Provider registries) {
        Army army = new Army();
        for (Tag t : tag.getList("soldiers", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            Soldier s = new Soldier();
            s.villager = entry.getCompound("villager");
            s.home = GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("home")).result().orElse(null);
            if (s.home == null) {
                continue;
            }
            s.enlisted = entry.getLong("enlisted");
            s.weapon = entry.contains("weapon") ? ItemStack.parse(registries, entry.get("weapon")).orElse(ItemStack.EMPTY) : ItemStack.EMPTY;
            s.armour = entry.contains("armour") ? ItemStack.parse(registries, entry.get("armour")).orElse(ItemStack.EMPTY) : ItemStack.EMPTY;
            s.shield = entry.contains("shield") ? ItemStack.parse(registries, entry.get("shield")).orElse(ItemStack.EMPTY) : ItemStack.EMPTY;
            s.wornWeapon = entry.getDouble("wornWeapon");
            s.wornArmour = entry.getDouble("wornArmour");
            s.wornShield = entry.getDouble("wornShield");
            s.arrows = entry.getInt("arrows");
            army.soldiers.add(s);
        }
        for (Tag t : tag.getList("records", Tag.TAG_COMPOUND)) {
            CompoundTag entry = (CompoundTag) t;
            GlobalPos.CODEC.parse(NbtOps.INSTANCE, entry.get("home")).ifSuccess(home -> {
                Record r = new Record();
                r.raised = entry.getInt("raised");
                r.fallen = entry.getInt("fallen");
                r.days = entry.getInt("days");
                r.strength = entry.getDouble("strength");
                r.hungry = entry.getInt("hungry");
                r.deserted = entry.getInt("deserted");
                army.records.put(home, r);
            });
        }
        CompoundTag kit = tag.getCompound("issued");
        kit.getAllKeys().forEach(k -> army.issued.put(k, kit.getInt(k)));
        CompoundTag ench = tag.getCompound("enchantLevels");
        ench.getAllKeys().forEach(k -> army.enchantLevels.put(k, ench.getInt(k)));
        army.theatre = tag.contains("theatre") ? Theatre.valueOf(tag.getString("theatre")) : Theatre.FACTION;
        army.lastDay = tag.contains("lastDay") ? tag.getLong("lastDay") : -1;
        army.eaten = tag.getLong("eaten");
        return army;
    }
}
