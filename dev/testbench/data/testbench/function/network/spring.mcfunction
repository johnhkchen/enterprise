# A spring: an artificially productive node. A Merchant's Desk at ~2 ~ ~2 in a small pen with two warehouse
# chests (east and west), refilled every day by network/spring_refill, and 64 emeralds for caravans' purses.
fill ~-1 ~-1 ~-1 ~5 ~-1 ~5 minecraft:smooth_stone
fill ~-1 ~ ~-1 ~5 ~3 ~5 minecraft:air
fill ~-1 ~ ~-1 ~5 ~ ~-1 minecraft:oak_fence
fill ~-1 ~ ~5 ~5 ~ ~5 minecraft:oak_fence
fill ~-1 ~ ~-1 ~-1 ~ ~5 minecraft:oak_fence
fill ~5 ~ ~-1 ~5 ~ ~5 minecraft:oak_fence
setblock ~2 ~ ~2 enterprise:merchants_desk
setblock ~3 ~ ~2 minecraft:chest[facing=west]
setblock ~1 ~ ~2 minecraft:chest[facing=east]
item replace block ~1 ~ ~2 container.26 with minecraft:emerald 64
summon minecraft:villager ~3.5 ~ ~3.5 {PersistenceRequired:1b,Tags:["testbench"],CustomName:'"Spring Merchant"',Xp:1,VillagerData:{profession:"enterprise:merchant",level:1,type:"minecraft:plains"}}
function testbench:network/spring_refill
# Its caravan squad.
function testbench:network/squad
