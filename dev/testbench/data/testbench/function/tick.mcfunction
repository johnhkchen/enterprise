execute as @a unless score @s tb_seen matches 1 run function testbench:welcome
execute as @a[scores={tb_left=1..}] run function testbench:welcome
