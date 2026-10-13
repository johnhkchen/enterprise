# A farm town: the network settlement plus a 24x20 carrot field south of the pen (480 plots) and three Master
# farmers to work it (5 x 32 plots each, with the town's own farmer: about 256 carrots a day). 32 emeralds of
# starting capital in the Merchant's chest.
function testbench:network/settlement
fill ~0 ~-1 ~18 ~23 ~-1 ~37 minecraft:farmland
fill ~0 ~ ~18 ~23 ~ ~37 minecraft:carrots[age=7]
summon minecraft:villager ~4.5 ~ ~8.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Field hand"',VillagerData:{profession:"minecraft:farmer",level:5,type:"minecraft:plains"},Xp:250}
summon minecraft:villager ~5.5 ~ ~8.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Field hand"',VillagerData:{profession:"minecraft:farmer",level:5,type:"minecraft:plains"},Xp:250}
summon minecraft:villager ~6.5 ~ ~8.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Field hand"',VillagerData:{profession:"minecraft:farmer",level:5,type:"minecraft:plains"},Xp:250}
item replace block ~8 ~ ~7 container.2 with minecraft:emerald 32
# Run once villagers have settled (the field hands compete for the composter, and a farmer re-hired after
# losing its job rolls new trades): every farmer in town buys carrots, 22 to the emerald, 16 a restock.
