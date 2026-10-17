//! Local prices as a content table, in items per emerald: what villagers pay for goods and crops, and what they charge
//! for food (`money/Prices.java`). The core keeps a good's worth in [`crate::Good::value`]; a host whose goods are the
//! mod's seeds it from here ([`emeralds_each`], [`crate::Good::priced`]). Unknown goods are worth 32 to the emerald.

/// Unlisted goods: 32 to the emerald (`Prices.DEFAULT`).
pub const DEFAULT_ITEMS_PER_EMERALD: f64 = 32.0;

/// The sixteen dye colours, for carpets (4 to the emerald, shepherds) and dyed candles (as plain ones: 3).
pub const DYE_COLORS: [&str; 16] = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray",
    "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"];

/// The mod's table, by item id (`Prices.ITEMS_PER_EMERALD`).
pub const ITEMS_PER_EMERALD: &[(&str, f64)] = &[
    // Crops villagers buy (farmers; clerics buy nether wart)
    ("minecraft:wheat", 20.0),
    ("minecraft:carrot", 22.0),
    ("minecraft:potato", 26.0),
    ("minecraft:beetroot", 15.0),
    ("minecraft:pumpkin", 6.0),
    ("minecraft:melon", 4.0),
    ("minecraft:nether_wart", 22.0),
    ("minecraft:sugar_cane", 24.0), // as paper
    ("minecraft:cocoa_beans", 20.0),
    ("minecraft:sweet_berries", 10.0),
    // Goods villagers buy
    ("minecraft:paper", 24.0),
    ("minecraft:book", 4.0),
    ("minecraft:ink_sac", 5.0),
    ("minecraft:writable_book", 2.0),
    ("minecraft:stick", 32.0),
    ("minecraft:flint", 26.0),
    ("minecraft:string", 14.0),
    ("minecraft:feather", 24.0),
    ("minecraft:tripwire_hook", 8.0),
    ("minecraft:glass_pane", 11.0),
    ("minecraft:compass", 1.0),
    ("minecraft:coal", 15.0),
    ("minecraft:raw_iron", 5.0),
    // What hands gather (no villager buys most of it; caravans carry it where it's wanted)
    ("minecraft:oak_log", 8.0),
    ("minecraft:spruce_log", 8.0),
    ("minecraft:jungle_log", 8.0),
    ("minecraft:acacia_log", 8.0),
    ("minecraft:sand", 32.0),
    ("minecraft:red_sand", 32.0),
    ("minecraft:cactus", 16.0),
    ("minecraft:cobblestone", 32.0),
    ("minecraft:snowball", 32.0),
    ("minecraft:dandelion", 16.0),
    ("minecraft:poppy", 16.0),
    ("minecraft:wheat_seeds", 32.0),
    ("minecraft:iron_ingot", 4.0),
    ("minecraft:diamond", 1.0), // an Expert smith buys one for an emerald
    ("minecraft:clay_ball", 10.0),
    ("minecraft:leather", 6.0),
    // Food villagers sell
    ("minecraft:bread", 6.0),
    ("minecraft:apple", 4.0),
    ("minecraft:cookie", 18.0),
    ("minecraft:cooked_porkchop", 5.0),
    ("minecraft:cooked_chicken", 8.0),
    ("minecraft:baked_potato", 8.0),
    // Comforts (what villagers sell them for, or near it)
    ("minecraft:cake", 1.0),
    ("minecraft:pumpkin_pie", 4.0),
    ("minecraft:honey_bottle", 3.0),
    ("enterprise:pipe_tobacco", 4.0), // a comfort: leisure
    ("enterprise:tobacco_leaf", 8.0), // the coast's raw ware
    ("minecraft:leather_helmet", 0.2),
    ("minecraft:leather_chestplate", 1.0 / 7.0),
    ("minecraft:leather_leggings", 1.0 / 3.0),
    ("minecraft:leather_boots", 0.25),
    ("minecraft:painting", 0.5),
    ("minecraft:flower_pot", 4.0),
    ("minecraft:lantern", 1.0),
    ("minecraft:candle", 3.0),
    // Smiths' goods (what villagers sell them for, or near it)
    ("minecraft:iron_hoe", 1.0),
    ("minecraft:iron_pickaxe", 2.0 / 3.0),
    ("minecraft:iron_sword", 2.0 / 3.0),
    ("minecraft:iron_chestplate", 1.0 / 9.0),
    ("minecraft:bow", 0.5),
    ("minecraft:arrow", 16.0),
    ("minecraft:wooden_sword", 2.0),
    ("minecraft:stone_sword", 1.0),
    ("minecraft:diamond_sword", 1.0 / 22.0), // a master's, enchanted: about what one asks
    ("minecraft:chainmail_chestplate", 0.25),
    ("minecraft:iron_helmet", 0.2),
    ("minecraft:iron_leggings", 1.0 / 7.0),
    ("minecraft:iron_boots", 0.25),
    ("minecraft:chainmail_helmet", 1.0),
    ("minecraft:chainmail_leggings", 1.0 / 3.0),
    ("minecraft:chainmail_boots", 1.0),
    ("minecraft:diamond_helmet", 1.0 / 13.0),
    ("minecraft:diamond_leggings", 1.0 / 19.0),
    ("minecraft:diamond_boots", 1.0 / 13.0),
    ("minecraft:iron_axe", 1.0 / 3.0),
    ("minecraft:stone_axe", 1.0),
    ("minecraft:diamond_axe", 1.0 / 18.0),
    ("minecraft:iron_shovel", 0.5),
    ("minecraft:stone_shovel", 1.0),
    ("minecraft:stone_pickaxe", 1.0),
    ("minecraft:diamond_pickaxe", 1.0 / 13.0),
    ("minecraft:diamond_shovel", 0.2),
    ("minecraft:bell", 1.0 / 36.0),
    ("minecraft:diamond_chestplate", 1.0 / 21.0),
    ("minecraft:shield", 0.2),
    ("minecraft:raw_gold", 3.0),
    ("minecraft:gold_ingot", 3.0),
    ("minecraft:saddle", 1.0 / 6.0),
    ("minecraft:white_banner", 1.0 / 3.0),
    ("minecraft:enchanted_book", 0.1),
    ("minecraft:lapis_lazuli", 1.0),
    // Game and meat
    ("minecraft:rabbit", 8.0),
    ("minecraft:rabbit_hide", 9.0),
    ("minecraft:beef", 8.0),
    ("minecraft:porkchop", 7.0),
    ("minecraft:chicken", 14.0),
    ("minecraft:mutton", 7.0),
    ("minecraft:cooked_beef", 5.0),
    ("minecraft:cooked_mutton", 5.0),
    ("minecraft:cooked_rabbit", 5.0),
];

/// The table's entry for a good, by its id with or without the namespace (`wheat`, `minecraft:wheat`): the listed
/// ones, then every colour's carpet and candle (the mod adds them to the table).
fn lookup(id: &str) -> Option<f64> {
    let id = if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") };
    if let Some(&(_, n)) = ITEMS_PER_EMERALD.iter().find(|(k, _)| *k == id) {
        return Some(n);
    }
    let bare = id.strip_prefix("minecraft:")?;
    DYE_COLORS.iter().find_map(|c| {
        if bare == format!("{c}_carpet") {
            Some(4.0)
        } else if bare == format!("{c}_candle") {
            Some(3.0)
        } else {
            None
        }
    })
}

/// How many of a good make an emerald (`Prices.itemsPerEmerald`): 32 if it isn't listed.
pub fn items_per_emerald(id: &str) -> f64 {
    lookup(id).unwrap_or(DEFAULT_ITEMS_PER_EMERALD)
}

/// One of a good's local value in emeralds (`Prices.emeraldsEach`).
pub fn emeralds_each(id: &str) -> f64 {
    1.0 / items_per_emerald(id)
}

/// Is the good in the table (`Prices.isListed`)?
pub fn is_listed(id: &str) -> bool {
    lookup(id).is_some()
}

impl crate::Good {
    /// A good of the mod's, worth what the table says (`id` with or without its namespace), `stack` to a slot.
    pub fn priced(id: &str, stack: u32) -> Self {
        crate::Good::new(id, emeralds_each(id), stack)
    }
}
