# A settlement for the caravan network: the usual test market, with an empty Merchant's chest except one
# shulker of bread (its food supplier) and empty lockers. Its goods have to come by caravan.
function testbench:market/build
data merge block ~8 ~ ~7 {Items:[]}
data merge block ~2 ~ ~1 {Items:[]}
data merge block ~12 ~ ~1 {Items:[]}
data merge block ~2 ~ ~13 {Items:[]}
data merge block ~12 ~ ~13 {Items:[]}
item replace block ~8 ~ ~7 container.0 with minecraft:shulker_box[minecraft:container=[{slot:0,item:{id:"minecraft:bread",count:64}},{slot:1,item:{id:"minecraft:bread",count:64}},{slot:2,item:{id:"minecraft:bread",count:64}},{slot:3,item:{id:"minecraft:bread",count:64}},{slot:4,item:{id:"minecraft:bread",count:64}},{slot:5,item:{id:"minecraft:bread",count:64}},{slot:6,item:{id:"minecraft:bread",count:64}},{slot:7,item:{id:"minecraft:bread",count:64}},{slot:8,item:{id:"minecraft:bread",count:64}},{slot:9,item:{id:"minecraft:bread",count:64}},{slot:10,item:{id:"minecraft:bread",count:64}},{slot:11,item:{id:"minecraft:bread",count:64}},{slot:12,item:{id:"minecraft:bread",count:64}},{slot:13,item:{id:"minecraft:bread",count:64}},{slot:14,item:{id:"minecraft:bread",count:64}},{slot:15,item:{id:"minecraft:bread",count:64}},{slot:16,item:{id:"minecraft:bread",count:64}},{slot:17,item:{id:"minecraft:bread",count:64}},{slot:18,item:{id:"minecraft:bread",count:64}},{slot:19,item:{id:"minecraft:bread",count:64}},{slot:20,item:{id:"minecraft:bread",count:64}},{slot:21,item:{id:"minecraft:bread",count:64}},{slot:22,item:{id:"minecraft:bread",count:64}},{slot:23,item:{id:"minecraft:bread",count:64}},{slot:24,item:{id:"minecraft:bread",count:64}},{slot:25,item:{id:"minecraft:bread",count:64}},{slot:26,item:{id:"minecraft:bread",count:64}}]]
# A 9x9 carrot field outside the pen for the farmer to work: carrots are food, so the settlement feeds caravans.
fill ~18 ~-1 ~0 ~26 ~-1 ~8 minecraft:farmland
fill ~18 ~ ~0 ~26 ~ ~8 minecraft:carrots[age=7]
# 64 emeralds of starting capital: caravans sell goods, they don't give them away.
item replace block ~8 ~ ~7 container.1 with minecraft:emerald 64
# A starter cottage south-east of the pen (two beds, a chest), and a hand waiting for work living in it: like any village's
# spare hands, it gathers, carries, or signs on with a caravan; the second bed is room for a child. Built block by block
# (what `enterprise cottage` builds, territory/Growth.buildHome): that command is the Java rules' and core mode refuses it.
fill ~20 ~-1 ~23 ~24 ~-1 ~27 minecraft:cobblestone
fill ~20 ~ ~23 ~24 ~1 ~27 minecraft:oak_planks
fill ~21 ~ ~24 ~23 ~1 ~26 minecraft:air
fill ~20 ~ ~23 ~20 ~1 ~23 minecraft:oak_log
fill ~24 ~ ~23 ~24 ~1 ~23 minecraft:oak_log
fill ~20 ~ ~27 ~20 ~1 ~27 minecraft:oak_log
fill ~24 ~ ~27 ~24 ~1 ~27 minecraft:oak_log
fill ~20 ~2 ~23 ~24 ~2 ~27 minecraft:oak_slab[type=bottom]
setblock ~22 ~ ~27 minecraft:oak_door[facing=south,half=lower]
setblock ~22 ~1 ~27 minecraft:oak_door[facing=south,half=upper]
setblock ~20 ~1 ~25 minecraft:glass_pane
setblock ~24 ~1 ~25 minecraft:glass_pane
setblock ~21 ~ ~25 minecraft:white_bed[facing=north,part=head]
setblock ~21 ~ ~26 minecraft:white_bed[facing=north,part=foot]
setblock ~23 ~ ~25 minecraft:white_bed[facing=north,part=head]
setblock ~23 ~ ~26 minecraft:white_bed[facing=north,part=foot]
setblock ~21 ~ ~24 minecraft:chest
setblock ~23 ~ ~24 minecraft:lantern
summon minecraft:villager ~22.5 ~ ~24.5 {PersistenceRequired:1b,Tags:["testbench"],VillagerData:{profession:"minecraft:none",level:1,type:"minecraft:plains"}}
