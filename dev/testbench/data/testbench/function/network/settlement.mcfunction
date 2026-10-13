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
