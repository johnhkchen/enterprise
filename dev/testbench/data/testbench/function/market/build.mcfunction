# Test market, built from your feet toward +X/+Z:
#   centre: Merchant's Desk with its stock chest (east of the desk)
#   corners: librarian, cartographer, fletcher, farmer, each with a locker chest touching its workstation
fill ~-1 ~-1 ~-1 ~15 ~-1 ~15 minecraft:smooth_stone
fill ~-1 ~ ~-1 ~15 ~4 ~15 minecraft:air
# Four lines, not "outline": on a one-block-tall region, outline fills every block.
fill ~-1 ~ ~-1 ~15 ~ ~-1 minecraft:oak_fence
fill ~-1 ~ ~15 ~15 ~ ~15 minecraft:oak_fence
fill ~-1 ~ ~-1 ~-1 ~ ~15 minecraft:oak_fence
fill ~15 ~ ~-1 ~15 ~ ~15 minecraft:oak_fence

# A bell and a bed per villager inside the pen, so the market is a settlement (bell, 3+ beds, 3+ workstations)
# whose villagers are free: each can walk to its bed, its workstation and the bell.
setblock ~7 ~ ~4 minecraft:bell[attachment=floor]
setblock ~4 ~ ~10 minecraft:red_bed[facing=south,part=foot]
setblock ~4 ~ ~11 minecraft:red_bed[facing=south,part=head]
setblock ~5 ~ ~10 minecraft:red_bed[facing=south,part=foot]
setblock ~5 ~ ~11 minecraft:red_bed[facing=south,part=head]
setblock ~6 ~ ~10 minecraft:red_bed[facing=south,part=foot]
setblock ~6 ~ ~11 minecraft:red_bed[facing=south,part=head]
setblock ~8 ~ ~10 minecraft:red_bed[facing=south,part=foot]
setblock ~8 ~ ~11 minecraft:red_bed[facing=south,part=head]
setblock ~9 ~ ~10 minecraft:red_bed[facing=south,part=foot]
setblock ~9 ~ ~11 minecraft:red_bed[facing=south,part=head]

setblock ~7 ~ ~7 enterprise:merchants_desk
setblock ~8 ~ ~7 minecraft:chest[facing=south]

setblock ~2 ~ ~2 minecraft:lectern[facing=south]
setblock ~2 ~ ~1 minecraft:chest[facing=south]
setblock ~12 ~ ~2 minecraft:cartography_table
setblock ~12 ~ ~1 minecraft:chest[facing=south]
setblock ~2 ~ ~12 minecraft:fletching_table
setblock ~2 ~ ~13 minecraft:chest[facing=north]
setblock ~12 ~ ~12 minecraft:composter
setblock ~12 ~ ~13 minecraft:chest[facing=north]

summon minecraft:villager ~7.5 ~ ~8.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Merchant"',VillagerData:{profession:"enterprise:merchant",level:1,type:"minecraft:plains"}}
summon minecraft:villager ~3.5 ~ ~3.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Librarian"',VillagerData:{profession:"minecraft:librarian",level:1,type:"minecraft:plains"}}
summon minecraft:villager ~11.5 ~ ~3.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Cartographer"',VillagerData:{profession:"minecraft:cartographer",level:1,type:"minecraft:plains"}}
summon minecraft:villager ~3.5 ~ ~11.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Fletcher"',VillagerData:{profession:"minecraft:fletcher",level:1,type:"minecraft:plains"}}
summon minecraft:villager ~11.5 ~ ~11.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Farmer"',VillagerData:{profession:"minecraft:farmer",level:1,type:"minecraft:plains"}}

function testbench:market/stock
tellraw @s {"text":"Test market built. Villagers claim their workstations within a few seconds.","color":"green"}
