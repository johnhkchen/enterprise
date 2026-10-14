# A caravan squad for a hub or spring (run at its corner): a trained Merchant, Cartographer, Librarian and Shepherd in
# the pen (Xp 1 keeps their trades without workstations), and 4 sheep in the market pen for the Shepherd to drive.
summon minecraft:villager ~0.5 ~ ~0.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Travelling Merchant"',Xp:1,VillagerData:{profession:"enterprise:merchant",level:2,type:"minecraft:plains"}}
summon minecraft:villager ~0.5 ~ ~3.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Cartographer"',Xp:1,VillagerData:{profession:"minecraft:cartographer",level:1,type:"minecraft:plains"}}
summon minecraft:villager ~3.5 ~ ~0.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Librarian"',Xp:1,VillagerData:{profession:"minecraft:librarian",level:1,type:"minecraft:plains"}}
summon minecraft:villager ~4.5 ~ ~4.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Shepherd"',Xp:1,VillagerData:{profession:"minecraft:shepherd",level:1,type:"minecraft:plains"}}
summon minecraft:sheep ~0.5 ~ ~1.5
summon minecraft:sheep ~1.5 ~ ~0.5
summon minecraft:sheep ~3.5 ~ ~4.5
summon minecraft:sheep ~4.5 ~ ~3.5
