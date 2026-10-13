# The standing test market near spawn: Merchant's Desk at 27 -60 27, Blaze Banker west of it,
# 18 shulkers of trade goods in the stock chest. Selling trades switch on once villagers settle.
execute positioned 20 -60 20 run function testbench:market/build
execute positioned 20 -60 20 run function testbench:market/stock_shulkers
setblock 26 -60 27 numismatics:blaze_banker
schedule function testbench:fixture/switch 200t replace
tellraw @a {"text":"Test market built at 27 -60 27; selling trades switch on in 10 seconds.","color":"green"}
