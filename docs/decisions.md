# Decisions

Decisions made after the brief (`docs/brief.md`). Where this file and the brief disagree, this file wins. Newest changes are dated.

## Name (2026-10-13)

- **The modpack is called Enterprise**, not Staples. The brief still uses the old name.
- Core mod: `mod_id=enterprise`, package `dev.b28.enterprise`.

## Platform (2026-10-13)

- **Minecraft 1.21.1 on NeoForge, Java 21.** Create, Numismatics and BankSystem all top out at 1.21.1. Create's 26.1 port is in progress with no date, so expect a port later.
- **Each quality tier is its own item** (`cotton_bale_fine`), not a component on one item. Create recipes then work unchanged, and a build-time generator writes the per-tier recipes.
- **Content modules are JSON manifests turned into code at build time.** Datapacks cannot register items or blocks, so "adding tea is data, not code" means "write JSON, rebuild", not "drop in a datapack".

## The logistics rule (2026-10-13)

**Nothing crosses distance without a carrier.** Goods, coins and price news all travel. This is why StockMarket is out (its trades are instant across the map), and why banks are per settlement.

## Dependencies and money (2026-10-13)

- **Numismatics is a core dependency** (LGPL-3.0). It is the early-era money: physical coins, vendors, depositors.
- **StockMarket is dropped.** It has no license file, so it cannot be forked; we write our own exchange from scratch.
- **BankSystem is not a dependency.** We write our own bank on Numismatics and lift pieces of BankSystem (MIT) with credit where useful.
- **Ledger / Ledger Databases are not dependencies.** They are Fabric-only grief loggers. Their append-only event log on H2/MySQL/Postgres is a reference for our own ledger storage.
- **Eras of money:** Numismatics coins first, our bank as an unlock, our futures exchange on top of that.

## Coinage (2026-10-13)

- **Banks are per settlement.** No account is reachable from everywhere. Money moves between towns by courier or by bill of exchange.
- **Each faction mints its own coinage.** A coin's value is its metal content (a gold/silver standard) times trust in the issuer, so exchange rates differ slightly between factions.
- **The Merchant is the money changer.** It converts between coinages and keeps the spread.
- Faction coins are one coin item per denomination with a "mint" component naming the faction. Item variety stays bounded by factions × denominations.
- Numismatics coins act as the neutral standard (bullion, or the trade coin everyone accepts).
- **Coinage is the first graded good.** A coinage is a graded batch of metal: its fineness is its grade. Assaying is inspection, and clipping or debasement is adulteration. The grade, inspection and reputation grammar from the brief starts with money, before any crop.

## Lockers (2026-10-13, revised 2026-10-14)

- A **locker** is a vanilla chest, double chest or barrel touching a workstation. Its villager is whoever has claimed that workstation.
- **Lockers only work when a Merchant manages them.** No Merchant in the village, no locker trading.
- **Trades run right after the villager restocks** (vanilla: at most twice a day). Each enabled trade runs until its uses run out, the inputs run out, or the locker is full.
- **Which trades run is set on the Merchant's screen**, per locker.
- **No trading at inflated prices.** If demand has pushed a trade's price above its base, the locker skips that trade until demand cools off. Automation can't run a price into the ground.
- **Price:** the standard price, with a discount that grows with the Merchant's level. A top-level Merchant gets close to the discount a cured villager gives.
- **XP:** each locker trade gives XP to both the villager and the Merchant.
- **Locked-up villagers get lockers too.** Confinement affects settlement value, not lockers.
- **Trades run while unloaded.** One pure settlement function takes (contents snapshot, cached trades, elapsed time) and returns the new state. First it runs on load, replaying what was missed. Later the ledger runs the same function while the village is unloaded, and the result is written back when the village loads.
- **Catch-up on load (built 2026-10-14):** when a villager with a locker loads back in, it waits 2 seconds for the Merchant's desk and chests nearby to load, then replays the restocks it missed, two per whole day, through the normal locker trade (supply, trade, collect, sell). Measured: a market unloaded for 3 days replayed exactly the 48 trades it makes in 3 loaded days.
- **Catch-up cap: 60 days** of missed restocks, as a guard on load time. The locker's size, trade uses per restock and the inflated-price rule already bound what accumulates well before that.
- **Rounds (built 2026-10-14):** a market whose villagers aren't running (unloaded, or loaded but idle like spawn chunks with no player near) gets a short visit when the server has room. Every 10 seconds, if the average tick is under 30 ms, the most overdue desk (no round for an in-game day) has its footprint loaded on a ticket that expires after 15 seconds: the desk, its locker workstations and their neighbouring chunks, at most 25. Villagers loading in, or already loaded but idle, get catch-up. One round at a time.
- **Catch-up is budgeted:** at most 8 replayed restocks per tick across all villagers, none while ticks run long. A villager's restock clock advances by exactly the days replayed, so part-days carry over to the next round instead of being lost. Measured: a market left with nobody near keeps its loaded pace, one day settled per daily round.
- **Still to come:** settlement without loading anything (pure ledger), so remote views see results between rounds.
- **Write-back safety:** a snapshot records the locker's contents at unload. If the chest doesn't match the snapshot on load (rollback, hand edit), the ledger's result is discarded and a fresh snapshot is taken. No duplication either way.

## Merchant (2026-10-13)

- New villager profession with its own job-site block.
- **Its job site is the Merchant's Desk.**
- **It is the village's trading desk.** It is the counterparty for players, villagers and arriving caravans.
- **Its stock is the chest(s) touching its desk**, the same rule as lockers, so hoppers and Create work with it. It can reach into shulker boxes kept in those chests.
- **Reach:** until settlements exist, it manages lockers within 48 blocks of its desk. Its level sets how many, from 2 at Novice to 10 at Master.
- **It manages the town's lockers.** It fills them best deal first; a higher level manages more lockers. Its screen lists the lockers it manages and which trades each may run.
- It gains XP from every locker trade it manages, and its level drives the locker discount.
- **Merchant XP mirrors a trading player's (2026-10-14).** Each locker trade gives the Merchant what a player would pick up: 3 to 6 XP, plus 5 when the villager levels up. It levels at the points a player has at levels 15, 30, 45 and 55 (315, 1395, 4020, 6895). Tuned by experiment (`dev/experiments/merchant_ramp.py`): a pen of four villagers fed 18 shulkers of goods is fully managed by about day 2 and has a Master Merchant in about 7 to 9 in-game days, depending on which trades the villagers roll. Thresholds at player levels 5 to 20 took about 1 day, which skipped the "earn your partnerships" arc.
- **Right-clicking a Merchant opens its screen** (it has no trades of its own): its lockers, and per locker the villager's trades as on/off switches. Every trade starts off; the player opts in.
- **A trade is identified by its items** (cost A, cost B, result), not its position in the list, so a choice survives level-ups and price changes. Two trades with the same items (two enchanted books) share one switch for now.

## Merchant money (2026-10-14)

- **First loop:** the player fills the Merchant's stock chest with shulker boxes of raw goods. The Merchant feeds lockers from them, villagers trade, the Merchant collects the emeralds, then converts them to Numismatics coins.
- **The Merchant converts emeralds at its faction's floating rate.** The rate starts at a base (spurs per emerald), drops as emeralds pour in, and recovers over time toward the base. Computed on touch: a pressure number that decays with elapsed game time. Until factions exist, every Merchant uses one default faction.
- **A Blaze Banker touching the Merchant's Desk is the settlement's bank account.** Converted coins are deposited into it. Later, wages and purchases are paid out of it.
- **Collecting earnings (2026-10-14): in person, at the settlement's Blaze Banker.** The Merchant deposits into it; the owner and their trust list withdraw through the Banker's own menu. A Banker placed with no owner belongs to the first player who uses it. A Bank Card link (money arriving anywhere instantly) was considered and left out: it breaks "nothing crosses distance without a carrier".
- **No Banker: coins land as physical coins in the stock chest.** Each payout re-mints the loose coins already there together with the new ones as the fewest coins, so they stay a handful of stacks. If even that doesn't fit, the Merchant keeps the emeralds for later.
- **Rate numbers (2026-10-14):** base 16 spurs (one sprocket) per emerald; a full shulker of recent selling (1,728 emeralds) halves the rate; selling pressure halves every 3 in-game days. A batch is priced along the falling curve, not at its opening rate. Measured: a stocked four-villager pen pushes the rate from 16 to about 12 over a week and earns about 17,700 spurs in 10 days.
- **Working reserve:** the Merchant keeps 64 emeralds unsold for trades that cost emeralds. With every trade switched on, it spends what it earns and sells little.
- **Player shops:** Numismatics vendors only trade with a clicking player, so the Merchant deals with them by reading the vendor's mode, price and stock and moving goods and coins itself.
- **It surveys routes and runs caravans.**
- Wages are paid in coins.

## Settlements (2026-10-13)

- **A settlement is anchored by a bell plus at least 3 beds and 3 workstations** within 48 blocks (built 2026-10-14). The Merchant's Desk counts as a workstation.
- A census reads the villagers, beds (home points), workstations (job points) and the bell (meeting point). The result stays in the ledger.
- **The census is polled, never constant.** Every 10 seconds, when the server has headroom, the bell within 64 blocks of a player that has gone longest without a census (at most once per in-game day each) is counted, with path checks spread over ticks (two villagers per tick). `/enterprise census <bell>` counts on the spot. Results live in world save data. Still to come: census on rounds, after big block changes, and path checks off the main thread on a snapshot.
- **Free villagers count; prisoners barely do.** A villager is free when it can walk to its own bed, its workstation (if it has a job) and the bell. Settlement value counts free villagers fully and confined ones at 0.1. The path check uses its own path finder under villager rules (doors open), so a villager that is jumping, sleeping or just spawned is judged the same as one standing still. Goal: a thriving village is worth more than a trading hall with villagers locked in cells. The bell check stops a cell that just has a bed and workstation inside it from counting as free.
- Farmers produce food. Caravans eat food per day of travel, so villages are the natural resupply stops along a route.

## Territory: a hex grid (2026-10-13)

- **Civilization-style.** A settlement earns hexes with money or culture growth, and its population works them.
- **The hex is the ledger's unit of place.** Ownership, security, detection and tolls are per hex. A route is a list of hexes.
- **Size: a hex is about 80% of a naturally generated village's core**, roughly 80 blocks across to start, set in config. A normal village covers its own hex plus up to about 2 rings around it (up to 19 hexes). We'll tune this by experiment.
- **Working a hex:** in player towns, the real farms, mines and buildings on the hex count. Simulated towns get abstract yields.
- **Culture comes from villagers of certain professions working a hex that has an improvement**, as in Civilization. Improvements cost money to build, which makes them a money sink.
- **A garrison on a hex lets its owner levy tolls**, given the right kind of unit. Garrisons make blockades and sieges possible, and chokepoints worth fighting over.
- **Garrison units:** a militia of villagers, where each profession gives its own boost (like caravan squads), plus a new raider-style unit for settlements (knights and soldiers), the village's answer to pillagers. Players' role in garrisons is decided later.

## Hex grid (built 2026-10-14)

- Pointy-top axial hexes, 80 blocks across the flats (centre to corner 46.2). `Hex.at(pos)`, distance, neighbours, and the straight line of hexes between two points. A route reports its length in hexes; tolls, detection and encounters will be per hex. `/enterprise hex` shows the hex you stand in.

## Territory, farms and tolls (built 2026-10-14)

- **Territory grows by culture, Civilization-style.** A settlement claims its bell's hex at its first census. Each census adds culture: settlement value times days since the last census. When culture covers the next hex (5 x hexes held), the border grows into the unclaimed neighbouring hex nearest the bell, out to two rings (19 hexes). A held hex stays with its owner. `/enterprise territory <bell>`; `/enterprise hex` shows a hex's owner.
- **Farmers work hexes.** At each census, the settlement's farmers are assigned its hexes, most-planted first; a farmer tends 32 plots per level. The crops actually growing set the produce: half an item per plot per day (wheat, carrots, potatoes, beetroot, pumpkins and melons from stems, sugar cane, cocoa, berries, nether wart). Real farms are only counted, never harvested. Produce accrues on the ledger (up to 60 days) and lands in the settlement Merchant's chest whenever the market is touched: a locker session, a caravan stop, a census. A farming waystation feeds caravans and supplies trade goods.
- **Fields are tracked per chunk.** Each hex remembers, chunk by chunk, the crops last counted there. A census recounts the chunks that are loaded and keeps the last count for the rest; nothing reads as zero just for being unloaded. Rounds load a settlement's known field chunks, so its census reads them fresh.
- **Farmers draw wages (2026-10-14).** A farmer's wage is a share of the value of what it grows (starting at half), at the crop's local buying price (carrots 22 to the emerald, wheat 20, and so on: what villagers pay). Wages accrue daily and are paid whenever the market is touched: emeralds from the Merchant's chest first, then its coins at the region's current spurs-per-emerald, then the settlement's Blaze Banker. Wages leave the economy. Unpaid days are unworked: the harvest is delivered only for the share of wages paid; unpaid wages are forgiven, not carried as debt. The Merchant keeps 3 days of wages in emeralds instead of selling them to the mint.
- **Wages absorb farm profit.** The farmers' share floats between 25% and 100% of the harvest's value, toward the fraction of the harvest that actually sells for emeralds (crop sales are counted from locker trades turning crops into emeralds, averaged over 3 days; the share closes half its gap every 2 days). A farm whose produce all sells ends up paying its whole sale value in wages: passive farming pays for itself, not for you. Unsold produce costs the 25% floor, so a harvest is cheap material for caravans, waystations and factories, never free. Profit has to come from processing and trade.
- **Measured** (`dev/experiments/farm_wages.py`, 480 plots, about 240 carrots a day, 32 emeralds of capital, 20 days each in about 20 seconds of tick warp): with one buyer, the farmers' share climbed from 50% to about 95% and the town's money levelled off near 75 emeralds by day 12, gaining nothing more; with no buyer, the share sank to the 25% floor, the capital lasted 9 days, then the farm stopped.
- **Why farms can't print emeralds:** emeralds only enter through villagers buying, capped per villager per day (inflated prices are skipped); every item grown costs half its value in wages whether it sells or not; and a settlement with no income can't pay, so its farms stop. A farm's best size is what local buyers take; growing past that loses emeralds, and caravans can carry surplus to other towns' buyers.
- **Tolls are a cut of coins in transit.** A caravan bringing coins home pays each other settlement whose territory its path crosses that town's toll rate on what it carries (once per settlement, one after another); the route's own two settlements charge nothing. The toll lands in the owner's Blaze Banker or Merchant's chest at its next touch.
- **Tolls are collected at posts (2026-10-14, replacing tolls on guarded shares of land).** A town collects its toll only from caravans that pass one of its toll posts: a hex of its land with a guard on it. The toll is the town's rate (0% to 25%) on the coins carried, once per town. Caravans plan around posts: passing one costs its town's toll on what the route usually carries home, so a caravan steps around a post on open ground for a hex's food and pays only where going around costs more. Each guard eats 6 food points a day from the town's warehouse and takes 0.5 emerald a day; guards who go unfed or unpaid desert in proportion to what was missing, the last posted first. Guards are a ledger count for now; militia professions, knights and blockades come later.
- **Towns work out whether they hold a chokepoint (2026-10-14, replacing the step-a-point toll search).** Each caravan crossing a town's land shows the town what guarding that land would do. The town tries a post on each of its hexes on the caravan's path and keeps the one that makes going around dearest; on a tie, the narrowest (most impassable neighbours), then the nearest home. Then it tries again on the cheapest path around the posts so far, until the path keeps off its land, there is no way at all, or 6 posts. Each try records the detour's cost in road food and other towns' tolls. Every 3 days the town picks the toll and posts that bring in the most after the guards' pay. It asks at most 80% of a route's detour, so caravans keep coming, and at most 25%. It drops posts it no longer wants at once and hires one guard a review for a new one. Routes unseen for 4 reviews are forgotten. `/enterprise territory <bell>` shows each route's posts and what going around them costs.
- **Water makes chokepoints (2026-10-14).** Each chunk is surveyed when it loads and unloads: of 16 surface columns, how many are water or another fluid. Chunks never loaded are guessed from their biome: ocean is water, a river is half water. A hex counts the chunks whose middle lies in it. A hex more than half water can't be crossed by a caravan; less, and it costs up to twice the road food (1 + 2 x the water share). Only surveys that differ from the biome's guess are saved. `/enterprise hex` shows a hex's water.
- **Measured** (`network_study.py --river`, 15 days): a river across the toll town's row of hexes, with the town's two hexes left dry as bridges. By day 4 the town had a guard on one bridge at 25%, and by day 7 a guard on each, collecting about 107 spurs a day for about 21 spurs of guards. Without the river (open ground) it posts no guard: a single post is stepped around for one hex's food. The road to S2 now runs 1,000 blocks over the bridge instead of 600 straight.
- **Bridges (2026-10-14): one of the oldest chokepoints, and a town improvement.** A town can bridge a hex it holds that is too wet to cross; a bridged hex is open ground to caravans. At each toll review, a town with no bridge under way looks at each impassable hex it holds and asks what a bridge would bring in: on each known route it would carry (not the town's own), the lesser of the most toll it may ask and 80% of what the bridge saves over the cheapest way without it, with the town's own tolls left out (a route with no way at all would save its whole trade), one trip a day. It builds the bridge that repays its masons' wages soonest, if within 30 days and it can pay half the wages now. Leaving out its own tolls stops a town bridging beside its own bridge just to let caravans dodge its post.
- **Masons build bridges for wages, with stone from the town's land.** Two masons at 1 emerald a day each lay 32 blocks a day each, but the town's land supplies only 16 blocks of stone a day per hex it holds, so a small town builds slowly. Unpaid masons don't work and aren't owed. A bridge's deck runs from the hex's edge facing one bank, through its centre, to the edge facing the other, and 6 blocks past each, 3 wide with a railing either side, about 450 blocks. Once finished, the deck is laid on every wet column at the water's surface the next time its chunks can be loaded: stone bricks with stone brick walls, spruce in forests, sandstone in deserts and on beaches. The ledger is the bridge; the blocks are its view. `/enterprise territory` lists a town's bridges, `/enterprise hex` says whether a hex is bridged.
- **Measured** (`network_study.py --river-bridge`, 25 days): a river through the row south of the toll town with no crossing at all, so the hub's road to S3 had no way through and S3 made no trades. On day 8 the toll town started a bridge on its river hex (repaid in about 2 days, by its estimate), finished it for 15 emeralds of wages, and laid 301 wet columns of deck. It then posted a guard there at 25%: neither route it carries has any way around. It collected about 42 spurs a day, and S3 traded again from about day 12. Before the fix above, the town also bridged the next hex, which halved its own chokepoint.
- **Route distance** is the straight distance plus a hex's width for each hex a detour adds over the straight line of hexes.
- **Warehouses keep 5 slots free** for locker results and coins: farm produce and caravan deliveries stop short of them, and a good's par is capped so wanted goods fill at most 60% of the warehouse. A full chest of unsold produce had stopped towns trading after about 9 days.
- **Rounds hold their chunks until the work is done.** A round loads its footprint on a ticket with no timeout, waits until the chunks and their villagers have loaded, then runs catch-up and the settlement's census, holds 200 ticks, and releases the chunks; it gives up after 30 seconds of real time. A tick-based timeout failed under `/tick sprint`, where 300 ticks pass in milliseconds while chunks take real time to load.
- **Rounds load the market, not just the desk:** chunks within 16 blocks of the desk and of the bell (where the Merchant and villagers spend their time), the locker workstations, and the settlement's field chunks, at most 25. A round that loaded only the desk's chunk missed a Merchant standing in the next chunk, and its lockers went unmanaged that day.
- **Censuses guard against half-loaded villages:** a round-loaded settlement is counted only once entities have loaded around its bell, and a census finding under half the villagers of one from the last 3 days is treated as incomplete and retried.
- **The census keeps a roster (2026-10-14).** Each census records every resident: where it was seen, its bed, its trade and level, whether it could walk freely, whether a baby. A resident not seen this time still counts if its chunk isn't loaded (asleep in a cottage past the market) or it's out with a caravan; one whose chunk is loaded and isn't there has gone; one unseen for 10 days is dropped. Babies count as residents (they need beds), not as free workers. Rounds load where residents sleep and were last seen as well as the market, up to 36 chunks. Measured over 25 days: no town's count dipped, and every count matched its births; before, a town of 6 was sometimes counted as 3 or 4.
- **A Merchant with few slots manages lockers with trades switched on first**, then the nearest.
- **Measured** (`caravan_network.py`, two runs): with five settlements left unloaded for 8 days, every farm produced 16 carrots a day, territories grew to 3-4 hexes, the hub banked 3,580-3,975 spurs, and the route crossing the toll town paid it 27-31 spurs.

## Mood and comforts (built 2026-10-14)

- **Comforts are what towns spend spare money on.** A kind of comfort is an item tag under `enterprise:comforts/`: to start, treats (cookies, cake, pumpkin pie, honey), clothing (leather armour), furnishings (carpets, candles, paintings, flower pots, lanterns) and reading (books). A pack adds a kind (tea, cigarettes, another mod's finished clothes) by adding a tag.
- **Villagers use them up.** Each villager uses a tenth of an emerald's worth of each kind a day from the town's warehouse, at local prices, cheapest goods first and whole items only (a cake covers several days). A ration that can't be met is gone without, not owed.
- **Mood is Civilization's happiness.** Each kind fully supplied adds 1. Every 8 villagers take 1 away (bigger towns want more), each villager without a bed takes 0.5, and wholly unpaid wages take 2. Mood closes half its gap to that every 3 days. A content town's farms yield a tenth more per point of mood (between half and one and a half times) and it gathers a quarter more culture per point (between a quarter and double), so it grows its territory faster. `/enterprise territory <bell>` shows mood, what's met and the effects.
- **Towns buy comforts by caravan.** A town aims to hold 4 days of each kind (villagers x 0.1 x 4 emeralds' worth), buying whole items, at least one when short. It pays only from money beyond its wage float and 3 days of guards' pay. A caravan loads comforts from the hub for what the town wanted at its last stocktake (taken at each caravan visit), half again over, before it fills the rest of its cargo.
- **Towns keep their trading capital (2026-10-14).** Before spending on comforts, homes, children or works, a town keeps back its wage float, 3 days of guards' pay and 2 days of what its lockers actually sell (averaged, at local value). A town that spent it couldn't buy the goods that earn its money and stayed poor. Caravans judge what comforts a town wants from its last stocktake less what its villagers will have used by arrival. Comforts are never eaten as road food or rations.
- **Measured over 40 days with everything on:** see `docs/experiments/full-study-2026-10-14.md`.
- **Measured** (`network_study.py --comforts`, 20 days, the hub stocking one shulker of comforts and its books): the three supplied towns reached mood +3.3 (all four kinds met; farms x1.33, culture x1.83). The two towns without a route sank to -0.6. Town money stopped climbing: S1 peaked at 125 emeralds on day 10, when the hub ran out of trade goods, and fell to 96 by day 20 while it kept buying comforts.

## Homes and children (built 2026-10-14)

- **Content towns grow.** When a town is counted while loaded, it works toward a home if every bed is taken and its mood is at least 0.5, and toward a child if a bed is free and its mood is at least 1: about one of each every 4 days. The ledger keeps the progress between counts.
- **A home is a real cottage**, 8 emeralds from money beyond the town's reserve: 5 x 5, cobblestone floor, plank walls with log corners, a south door, two windows, a slab roof, a bed and a lantern. It goes on the nearest flat, open, dry ground 8 to 32 blocks from the bell (inside the census's 48), with a block of clearance and never on a field.
- **A child costs 36 food points and 3 emeralds** (food beyond the caravans' par, money beyond the reserve) and is born as a baby villager by the bell. Babies only age while their chunk ticks, so a child loaded back in is aged by the time it was away.
- **Mood caps growth.** Every 8 villagers take 1 from mood and a child needs mood 1, so a town supplied with all four kinds of comfort stops growing around 24 villagers. A pack that adds kinds of comfort lets towns grow bigger.
- **Measured** (`network_study.py --comforts`, 20 days): the three supplied towns built 1-2 cottages each and had 1-2 children, growing from 5 villagers to 6-7; the first child had grown up by day 20.

## Work for idle hands, and fields (built 2026-10-14)

- **Babies never work; trained adults crew caravans first.** When a town is counted while loaded, it trains its squad first (see Caravans); only when its squad isn't short does it find work for an unemployed adult.
- **Work stalls.** An unemployed adult on the town's roster gets a stall: the workstation of the trade, not already in the town (by its roster), whose villagers would buy most of what the town has to sell (4 days of its farms' output and its warehouse's stock, at local value), with a chest against it so it's a locker. 6 emeralds beyond the reserve, on a 3 x 3 patch of open ground at least 6 blocks from the bell. The villager takes the job at once (waiting for it to walk to the workstation is chancy in an unloaded town), and its trades that buy goods for emeralds are switched on. Trades: farmer (crops), librarian and cartographer (paper and books), fletcher (sticks, flint, string, feathers), fisherman, shepherd (wool), cleric, butcher, leatherworker, mason, armourer, toolsmith, weaponsmith. Nothing to sell, no stall.
- **Fields for farmers with hands to spare.** After a census while loaded, if the town's farmers could work 54 more plots than they do (32 a level each), it plants a 9 x 9 field on open ground in its home hex, for 4 emeralds beyond the reserve, of what its market wants: the crop its lockers can take the most of a day by value, counting sugar cane as the paper it's pressed into; carrots if nothing sells but its food is under twice the caravans' par. Crops grow on farmland round a water block in the middle; cane along three water channels.
- **The warehouse presses cane into paper,** one for one as at a crafting table, at each touch, while paper is under its par and some locker buys paper. So a town with Librarians or Cartographers and a cane field sells its own paper.
- **No town is stranded (2026-10-14).** Each census notes the village's demand: every resident's trade that buys one good for emeralds, switched on or not (its cost times its uses, twice a day). Fields grow the crop with the most unmet demand by value (cane counting as paper); a crop the village wants and doesn't grow at all gets a field even when the farmers are busy (a farmer always plants for its own trades), up to two fields a farmer level. Farmers tend the crops the village buys first, the most valuable demand first, then the rest. The Merchant mediates: every locker trade in the village that buys a crop the town grows (or what its trades make of one) is switched on. Sweet berry bushes are a crop, for Master Butchers.
- **Trades convert raw goods (see `Workshop`).** Librarians and Cartographers press sugar cane into paper (1 for 1) while paper is under par and some locker buys it; Farmers bake wheat into bread (3 for 1) while food is under twice the caravans' par. A conversion runs only if a villager of that trade lives in the town.
- **Measured** (`standalone_town.py`, 20 days: one settlement, no caravans, nothing switched on by hand): the village's demand was paper (its Librarian and Cartographer), so it planted sugar cane, its Novice farmer worked the cane first (16 a day, pressed into paper), and it traded on its own, 10 trades and +7 emeralds in 20 days. Before, the same town grew only carrots nobody in it bought and never traded. With no comforts it stays at mood -0.6 and doesn't grow: stagnant, not stranded.
- **Measured** (S1 staged with four spare farmers, an unemployed resident, and string and coal no one in town bought, 6 days): it put up a Fisherman stall (fishermen buy string and coal) and planted a 9 x 9 sugar cane field for the paper its Librarian and Cartographer buy; its farms then grew 27 cane a day beside 40 carrots. In the plain network study no town needed either: caravans bring only what a town's lockers take, and one Novice farmer is busy with the starting field.

## Herds (built 2026-10-14)

- **Towns keep herds on the ledger:** sheep, cows, pigs and chickens, counted per town (fractions carried) and tended at every touch. A town's first Shepherd brings 3 sheep and its first Butcher 2 pigs and 2 chickens, once only.
- **Animals eat from the warehouse:** sheep and cows a quarter of a wheat a day, pigs a quarter of a carrot, potato or beetroot, chickens a tenth of a wheat. Their feed joins the village's demand, and comes first: a field for feed the town doesn't grow enough of is planted before anything that sells, and farmers tend feed before other crops. Fed (90% or more), a herd grows by 15% a day up to 16 animals a pasture; hungry, it dwindles by up to 10% a day.
- **What herds give.** A Shepherd shears half a wool a sheep a day; a Butcher keeps 4 of each kind to breed from and slaughters half the rest a day for meat (1.5 mutton, 2 beef, 2 pork, 1 chicken each) and leather from cows. So a Butcher's chain needs farmers for feed and a Shepherd's flock for mutton; a Master Butcher's sweet-berry trade makes berries a crop the village asks its farmers for. Shepherds weave wool into carpets (2 for 3) while the town is short of furnishings, a comfort.
- **The pasture is real:** a fenced 9 x 9 with a gate on open ground in the home hex (4 emeralds), fenced once the town keeps animals. Its animals are a view of the ledger: when the town is counted while loaded, missing animals are spawned in it and ones the ledger no longer has (slaughtered, starved) are removed.
- **Measured** (`standalone_town.py --herders`, 20 days, no caravans): the town fenced a pasture and planted wheat first; its herds, which had started to starve, recovered (sheep 1.5 to 4, chickens 1 to 4); the Butcher held pigs near the breeding stock; and the Shepherd's carpets met the town's furnishings in full with no caravan. Before feed came first, the town planted two cane fields and the flock starved.

## Leatherworkers and Fishermen (built 2026-10-14)

- **Leatherworkers** bring 2 cows (once, like Shepherds' sheep), cull the cows beyond the breeding stock for hides (and beef) as a Butcher would, and make leather boots (4 leather a pair) while the town is short of clothing: a town can now dress itself. On a caravan they carry **packs**: 9 more stacks, taken along when the destination wants more than a wagon holds.
- **Fishermen are boat captains.** A caravan with a Fisherman aboard can sail water, each water hex costing 0.8 of a hex of open ground; the squad plans both ways and takes the boat when it's the only way or the cheaper one. Boats slip past bridges and their toll posts. **A fishing fleet:** on water, every Fisherman to spare goes (up to a full squad of 4), and each catches 8 cod on each water hex sailed, carried as cargo. **Towns buy food when short:** under twice the caravans' food par, a town buys food from a caravan to get there, so a fleet sells its catch.
- **Docks.** A town with a Fisherman builds a pier where its land meets water (four planks out, a lantern at the end, 4 emeralds), and its Fishermen land 4 cod a day per level, which they smoke into cooked cod while food is short. Water is no longer land a town can't use.
- **A squad is never more than 4,** however many trades would help. **A town's own shopping trip goes only if the shopping is worth twice its road food:** the toll town had spent 393 spurs of road food to buy 68 spurs of comforts.
- **Measured** (`network_study.py --river-bridge --fleet`, 12 days, three trained Fishermen and a Leatherworker added to the hub's squad): before any bridge existed, a Cartographer and three Fishermen sailed the river to S3, which traded from day 3 (it made none until the bridge before); that route caught 312 cod, the road to S2 152.

## Walls (built 2026-10-14)

- **A wall turns a town's land and its masons' wages into an improvement.** A content town (mood at least 0.5) with nothing that pays better to build (no bridge worth it, see Bridges) and 12 emeralds to spare beyond its reserve starts a wall. Masons build it as they build bridges: two at 1 emerald a day, with stone quarried from the town's land at 16 blocks a day per hex, one project at a time.
- **It's a real wall:** a ring 40 blocks from the bell (past the homes, inside the census), 3 high, with gates 3 wide at the four points of the compass. It's of what the land offers: a log palisade on grass and dirt (spruce in taiga and snow), sandstone on sand, cobblestone on stone. Where the ground is a field, a path, water or something built, or the air above isn't clear, it leaves a gap, so villagers' paths become gates. It goes up in the world a quarter at a time as the masons lay it, loading its chunks to do so.
- **A walled town feels safe:** up to +1 mood and 2 culture a day when the wall is whole, in proportion to how much stands. Defence against raids comes with raiding.
- **Measured** (`network_study.py --comforts`, 25 days): the three content towns each started a wall about day 8 and finished it about day 20 for 24 emeralds of wages, raising all 252 columns on the flat test world. With the wall's culture and good mood their land grew to 10-11 hexes (7-8 without walls). Their money fell to 28-70 emeralds as walls, homes and comforts took it.

## Contracts (built 2026-10-14: standing orders, shipping terms, shop contracts, sourcing)

The foundation for contracts and, later, futures. Built to grow:

- **A contract is a list of promises between parties, on a schedule, at a locked price.** Parties are Merchant's Desks for now (players and factions later). Each promise is a term of a registered kind (`Term.TYPES`), so later eras add kinds without changing the book: delivery (built), and later passage through a toll town for a fee, escort, villagers lent, insurance against a raid. The schedule is a start, an interval and a count of installments. Contracts save their terms by kind; one that can't be read is skipped, not fatal.
- **News travels with carriers.** A party acts on a contract only once it holds a copy. Drafted at the destination, the destination holds one at once; the home desk gets its copy when the caravan that drafted it is home, and only then is it in force. Later carriers (telegraph, trains) will bring copies faster; nothing else needs to change.
- **Standing orders (built).** A travelling Merchant (a Merchant in the squad) that sold a good well drafts one with the resident Merchant: the destination takes, each round trip (at least a day), what its lockers actually sell in one, in sixteens, at this visit's bargained price. Ten installments, or as many as the seller can see it can cover (what's aboard and what its home warehouse held), at least three. One contract per good per pair. The resident won't promise more than it sells; the travelling Merchant won't promise more than it has.
- **Caravans settle them.** Goods owed under contracts in force go first into cargo. At the destination they're delivered before any other selling, whatever the warehouse's par (but keeping its free slots), and paid at the locked price whatever the market says now: a forward contract. A buyer that can't pay 90% loses trust.
- **Trust between each pair of parties** starts at a half: each installment settled in full by its grace (2 days) raises it by 0.05, each missed lowers it by 0.2, three misses break the contract, and below 0.3 the two draft nothing new. `/enterprise contracts [desk]` shows each contract's terms, copies, state, record and the parties' trust.
- **Measured** (`network_study.py --comforts --inject 15`, 30 days): the spring's first paper order to S1 ran its ten installments in full (trust to 1.0) and its later orders grew with the towns' sales (272, then 384 paper a delivery); the hub's early order broke when its finite stock ran out (trust 0.10, nothing new drafted). Towns stayed healthy (S1 130, S2 150 emeralds by day 30). Before orders were sized to what the buyer sells, and kept off the warehouse's free slots, they flooded S1's warehouse and it nearly went broke.
- **Next, planned:** contracts as paper you can hold and trade (assignable: a futures market, priced by the gap between the locked and the market price); deposits held in a Blaze Banker as escrow when trust is low; passage agreements with toll towns; contracts drafted by players at the desk; a Contracts tab on the desk screen.

## Services (built 2026-10-14)

What each trade does for a town: its own, or the one its caravan reaches. Built to grow, like contract terms:

- **A service is registered by name** (`Services`): the trade that does it, where (at home, abroad, or on the road), what it does in plain words, whether the town served wants it now, and doing it. Later eras add theirs (a Toolsmith mending the wagon, an Armorer fitting guards) without changing who runs them. Each is done by the most skilled of its trade who is there: at home, a resident not away; abroad, a squad member. Skill (level) sets how much.
- **Abroad, the town served pays** from its money beyond its reserve, into the caravan's purse, and each paid service raises the two towns' trust by 0.02. **Who goes is decided by the services:** before a trip, each trade at home is asked whether its services would be wanted there, from the ledgers alone. Clerics and Farmers now go along, but never lead (towns don't train them for squads).
- **Built:** a Merchant's deals (haggling, standing orders); a Shepherd's droving; a Cartographer's survey of the road and **map swap** (each town learns of the markets the other knows; the one that learns more pays 0.25 an extra market; ours rides home with the squad); a Librarian's copy of the books and **bookbinding** (3 paper and a leather make a book while the town is short of reading, at home, or abroad for a town without a Librarian at 0.05 a book); a **Cleric's blessing** (+0.5 to where the mood is heading: a visiting Cleric blesses a town without one for 2 days and a day a level at 2 emeralds, a resident blesses its own town while it stays); a **Farmer's help with the harvest** (a day's harvest from fields no one tends, as many plots as a Farmer of its level, for the town's farmhands' share of its worth; only a Farmer its own fields can spare goes); a Cartographer at home **charts** the country, learning of markets within 800 blocks every 10 days. On the road: a Leatherworker's packs, a Fisherman's boat and catch.
- **The census keeps what untended plots would yield** (fields beyond what the farmers can work), which is what a visiting Farmer brings in.
- `/enterprise services` lists every trade's services; `/enterprise services <desk>` shows what its town's trades do, what's been done for and by it, and the latest. `/enterprise routes` shows what each route earned from services.
- **Measured** (`network_study.py --comforts --services`, 20 days, a Cleric and a Farmer added to the hub's squad, against the same run without): the Farmer helped S1 with its harvest 19 days running for 22.8 emeralds, the Cleric blessed S1 and S2 five times each and S3 once (2 emeralds a time), and two map swaps were paid for. On day 20 the served towns were happier (S1 +4.3 against +3.2, S2 +4.5 against +4.1, S3 +4.1 against +3.8); S1 and S3 were richer (70 and 34 emeralds against 20 and 6: the extra harvest sold), S2 poorer (25 against 46: it paid for its blessings). Runs differ by a few emeralds by chance, so read these as directions.
- **Not yet:** services on the desk screen; a Cleric's shrine, potions and healing; Farmers sowing new fields abroad; Librarians copying enchanted books.

## The metal chain (built 2026-10-14)

Mine, smelter, smiths, and what the iron is for. All of it is services (see Services), so each trade also does its work abroad for a town without that trade, for a fee an item.

- **A Toolsmith digs a mine.** A town with a Toolsmith digs a mine head near its bell (a 3 x 3 cobblestone collar, a laddered shaft, a lantern; 6 emeralds beyond its reserve). Each Toolsmith at home brings up 2 raw iron and half a coal a day per level, half again with pickaxes.
- **An Armorer smelts:** 8 raw iron and a coal make 8 iron ingots (abroad, 0.02 an ingot, for a town with ore and no Armorer).
- **Smiths make kit from the iron,** for whoever lacks it: a Toolsmith hoes for the farmers and pickaxes for the miners, a Weaponsmith swords for the guards, an Armorer chestplates for them. Iron a locker wants is kept to its par. Kit wears out: a hoe or pickaxe a worker every 16 days (twice as long while a Toolsmith mends them, at home or for a few days after a visit), a sword every 20, a chestplate every 40.
- **What kit does:** with hoes for all, farmers tend half again as many plots; with pickaxes, miners bring up half again as much. **Armed guards** let a town ask up to 95% of what going around its posts would cost (from 80%); **armoured guards** let its toll go up to 35% (from 25%). In proportion to the share of guards equipped. Raids will test the same kit later.
- `/enterprise territory` shows the mine and each kit's share; `network_study.py --metal` sets the chain up.
- **Measured** (`network_study.py --river --comforts --metal`, 20 days, against the same without): S1's Toolsmith dug a mine, visiting Armorers (the hub's and T's) smelted its ore 9 times, and it forged a hoe and a pickaxe from the iron; S1 brought in 693 items against 403 (ore, and a farmer tending half again as much). T, with the whole chain, armed and armoured both bridge guards, set its toll at 35% and collected 692 spurs of tolls against 458. T's three extra smiths crowded its beds (8 villagers, 5 beds), so its mood was worse: a real cost of taking on trades without homes.
- **Not yet:** ore depending on the land (stone and hills richer than plains); an escort of armed crew on caravans (with raids); iron tools and armour as goods towns buy from each other.

## Labour, Masons, Fletchers and masters (built 2026-10-14)

The rest of the trades, as services (see Services), and a master's service for each.

- **Hands without a trade** (Nitwits, and adults waiting for work) each work one of the town's hexes and gather what its biome gives: logs, sticks, apples and string in woods; spruce and berries in taiga; sand and cactus in deserts; clay, cane and ink by water; stone and flint in hills; seeds, flowers, sticks and flint on open ground. These are the goods no trade makes. A Nitwit gathers half again as much. Gathering stops at 16 of a good the town doesn't use (64 of food, comforts and what its lockers buy), and while the warehouse is short of free slots: it must not crowd out the caravans' goods (it did, in a one-chest town, until this rule).
- **Militia:** hands stand watch at the town's posts first, fed but unpaid. **Porters:** when a caravan's cargo is more than the wagon and a Leatherworker's packs hold, hands go along carrying 9 stacks each, and walk home once it's delivered.
- **Fletchers hunt** the town's land by biome (rabbits and hides in desert, snow and taiga; beef, pork, fowl, feathers and leather elsewhere), a quarter as much without a bow, and **make bows** of sticks and string, for themselves and then for the guards: spare bows arm guards as swords do. Kit for the town's own hands comes before what its lockers sell.
- **Masons build** (`Buildings`), a money sink with something to show: a square (+0.5 mood), then halls as its own skill and the trade's allow: a temple (Cleric 3: blessings count double, children twice as often), a library (Librarian 3: 3 culture a day), a smithy (a smith 3: mine +50%, kit lasts a quarter longer), a watchtower (Fletcher 3: half the guards count as armed), stone docks (Mason 4, Fisherman 3: twice the catch), a caravanserai (master Mason, Merchant 3: word of the town carries 800 blocks, 1 culture a day). Materials up front (logs in the warehouse stand in for timber), wages while building. A town builds only when content and holding twice the whole cost beyond its reserve: at first a town built itself out of its comforts. A visiting Mason lays a day's work on a town's works for a day's wage.
- **Masters,** most with another trade's help:
  - Cleric: enchants the town's kit (10 levels as Journeyman, 20 Expert, 30 Master), laying on an enchanted book's enchantment, or at the table with lapis (from a deep mine) and an emerald of bottles o' enchanting per 10 levels. Enchanted kit works better (each level +15%, up to half again) and Unbreaking makes it last (1 + level) times as long.
  - Librarian: copies an enchanted book with a book and a lapis, while kit wants enchanting.
  - Toolsmith: sinks the mine deep, with a Mason to shore it: lapis too.
  - Armorer: builds a blast furnace, with a Mason: twice the iron per coal.
  - Weaponsmith: keeps an armoury, with an Armorer: guards' kit lasts twice as long.
  - Farmer: plants flower beds on the Mason's square (8 flowers, which the hands gather): +0.5 more mood.
  - Shepherd: weaves banners (6 wool and a stick furnish like twelve carpets).
  - Butcher: smokes meat with coal while food is short.
  - Leatherworker: saddles of leather and iron; a saddled caravan rides a third faster.
  - Cartographer: charts twice as far. Fisherman: half again the fleet's catch. Fletcher: a hunting party, twice the game. Mason: the caravanserai and stone docks.
- `/enterprise territory` shows hands, militia, buildings, hunters with bows and guards armed.
- **Measured** (`network_study.py --river --comforts --trades`, 20 days, against the same without): S1, given two Nitwits, a Mason, a Cleric, a Fletcher and a master Farmer (with beds), built a square, got flower beds and its Cleric's blessing daily, grew to 11 villagers (5 without) at a better mood (+3.3 against +3.1) and brought in 2,242 items against 403 (gathering and hunting); it had 20 emeralds to the other's 43, and a temple waits on its savings. Its one chest filled, so its hands stopped gathering, and its Fletcher never got the string for a bow. T's two Nitwits stood watch at its bridges unpaid and its spare bows armed both guards; it kept 60 emeralds to 44.
- **Not yet:** a master Farmer sowing fields abroad; arrows; dyes for banners; buildings on the desk screen.

## The master Merchant and players' shops (built 2026-10-14)

A master Merchant (level 5) at a desk deals with the Numismatics vendors (shops) within 16 blocks of it, each minute its market is loaded, as agent and market maker. Players use Numismatics' own shops; nothing new to learn.

- **A player's buying shop is a standing order:** its filter is the lot, its price what it pays, its bank card the purse. The Merchant fills it from what the warehouse can spare beyond its par, at the shop's price (if that's at least 90% of the goods' worth), and banks the coins. What it can't fill stays an **open order**, and the region works on it: caravans bound for this desk bring it (their cargo now covers orders there as well as lockers), and this desk's own caravans buy it up as surplus on their way home.
- **An order for enchanted goods is a commission,** worked by the town's master Librarian and a Cleric (3 or better), a few steps each minute: for each enchantment the order wants, a book at its level, made by copying a book (a book and a lapis), combining two at the same level into one a level up (an emerald of bottles o' enchanting), or, if no book has it, the Cleric enchanting books at the table (lapis and bottles, at 10 to 30 levels by its skill) to see what comes. Then the books are laid on the plain good (an emerald each) and it's handed over. A level beyond the enchantment's maximum can't be made, and the readout says so.
- **A player's selling shop is supply:** the Merchant buys from it what its orders and its lockers want, at up to the goods' worth, and up to half again over worth for goods it has posted as wanted.
- **Signals:** what the desk wants (open orders, its lockers' shortfalls, commissions waiting on a base good) that no market it knows has at its last stocktake, and no player's shop here sells, is posted with the premium it will pay: work for players to bring from another region or make.
- **A shop touching the desk is the town's own:** the Merchant sells its biggest surplus there (no food, no money, nothing ordered) in lots of 16 at 10% over worth, keeps a stack on the shelf, and banks the takings.
- `/enterprise market <desk>` has the Merchant look now, then shows its trade with the shops, open orders, commissions and signals. `network_study.py --master` makes the hub's Merchant a master.
- **Verified:** game tests on real vendor blocks and the Numismatics bank (an order filled, an order it can't fill posted to the region and as a signal, a stall bought from to fill an order at a profit, an Efficiency II pickaxe made from one Efficiency I book by copying, combining and laying on, the town's shop set up and its takings banked, an order riding out with a caravan). Live on the headless server: a vendor placed by the hub's master Merchant was set to sell its compasses within the minute. Not seen: a player using a shop the Merchant runs, through Numismatics' screen.
- **Not yet:** the desk's screen showing orders and signals; a player drafting a contract at the desk (the Contracts tab); the town's own shop buying signals (Numismatics pays buy orders from a bank card, which a town would need).

## Shipping terms, shop contracts and supply chains (built 2026-10-14)

- **Every contract has shipping terms** (`Incoterm`, after the trade's own; tolls stand for duties):
  - **EXW, Ex Works:** the buyer collects at the seller's. Its caravans to the seller carry the money and load what's owed, paying the locked price there. An installment the seller had in its warehouse (at its last stocktake) but nobody came for is the buyer's miss.
  - **CPT, Carriage Paid To:** the seller's caravans carry it, but it's the buyer's once on the road: it counts as delivered when it leaves. The buyer pays the tolls on top.
  - **DAP, Delivered At Place:** the seller carries it and bears the road: it counts only once it's there. The buyer pays the tolls on top: what the posts on the road will take of the payment going home.
  - **DDP, Delivered Duty Paid:** the seller carries it, bears the road and pays the tolls. The travelling Merchants' standing orders are DDP, as they always were in effect.
  - A price is set at the seller's, then carriage is added where the seller carries, and tolls where it pays them. FCA, CIF and CIP wait on rail and on raids (insurance).
- **Misses are the seller's or the buyer's:** goods not sent or not had are the seller's; goods set aside and not collected, or not paid for, the buyer's. Three by either break the contract, and both cost the pair's trust.
- **Copies ride with any caravan between the parties,** each way: a contract drafted at one desk is in force once a caravan between the two has carried it to the other. Before, only the drafting caravan carried them.
- **Goods made to order:** a delivery can name enchantments, and only goods enchanted just so count.
- **A player's buying shop now gets a real contract** with the master Merchant's desk, drafted in person at the shop (so in force at once), Ex Works (the shop is at the seller's market). It runs as many installments as the shop's purse would pay for, up to 10, one a day (commissions every two). The price is the shop's, locked. Each installment is delivered as it falls due and paid from the shop's card at the locked price; if the card can't pay, that's the buyer's miss. The commissions of the earlier round are worked under these contracts.
- **Supply chains, back to back:** when a desk can't fill what a contract wants (a player's order, a commission's base tool, lapis or books), its Merchant contracts a supplier: the nearest market it knows that held the goods at its last stocktake, with a road between them. The terms are Ex Works if its own caravans go there (they collect), else Delivered At Place (the supplier's caravans bring it). The price is the worth plus 10%, then carriage and tolls by the terms. A town sells at most half its kit. One open contract per good. With no road, the want stays a signal: a gap a player could fill.
- `/enterprise contracts` shows each contract's terms, goods, what's on the road, and whose misses.
- **Verified** in game tests: Ex Works goods collected and paid for by the buyer's caravan, a contract's copy carried to the far end putting it in force, Carriage Paid To goods on the road not a miss where Delivered At Place ones are, uncollected Ex Works goods the buyer's miss, prices by terms, a shop's contract delivering a lot a day and completing, sourcing a supplier Ex Works. Live, on a 10-day network, the spring's and hub's standing orders ran as before (three contracts, none missed).
- **Next:** contracts drafted by players at the desk (the Contracts tab), so a player can buy Ex Works and carry it themselves; escrow when trust is low; passage agreements with toll towns; contracts as paper to trade.

## Crates and bills of lading (built 2026-10-14)

- **The crate** (`enterprise:crate`) is a wooden box of nine stacks with a tenth slot for one bundle of documents: a common shulker box. It's crafted from 4 planks and 4 sticks (planks at the corners, sticks between, the middle empty). Broken, it keeps its goods and papers. It doesn't go inside another container. A packed crate is one of a kind (stacks to 1); empty ones stack to 16.
- **Documents** (`enterprise:documents`, from 2 paper and a string) hold papers, each a kind and its particulars, so later eras add kinds freely:
  - the **bill of lading**: shipper, consignee, goods, contract, terms, route, day issued; the receipt for the goods and the title to them;
  - a **copy of the contract**;
  - bills of exchange to come.
- **Shipments under contracts go in crates with their papers:** a bill of lading and a copy of the contract in each crate.
  - The shipper uses its own empty crates first, and pays an eighth of an emerald for each new one (conjured for now; later a carpenter's, with nails and wood).
  - With no crate and no money, goods go loose, without papers.
  - A crate takes the cargo space of what's in it (at least one stack).
- **The consignee's Merchant opens crates consigned to it,** whoever brought them:
  - It takes the contract copy as its own.
  - It delivers the goods under the contract the bill of lading names.
  - It keeps a stack of the empty crates for its own shipping, and hands the rest back to the caravan that brought them, as returnable crates.
- Ex Works goods are crated by the seller, made out to the buyer, and opened when the buyer's caravan is home.
- **A player can carry a crate:** put it in the consignee's warehouse chest and the Merchant opens it and takes delivery. The payment is owed to the seller until a caravan between the two carries it, whichever way it runs.
- **Measured live** (`network_study.py --comforts --inject 3`, 8 days): contracted goods shipped in crates and were delivered under their contracts. At first the empty crates filled S1's one-chest warehouse, a slot each: 80 sticks rode back and forth undelivered and that contract broke, and S1 traded 12 times. With empties stacking and returning, S1 traded 44 times and its paper contract delivered every installment.
- **Not seen:** the crate's screen and textures in a client.

## Bills of exchange (built 2026-10-14)

- **A buyer short of coin pays with paper:** a bill of exchange drawn on its own treasury, for the debt and 2% for the wait, due in 4 days. It happens when a caravan collects Ex Works without the money, or a buyer can't pay on delivery, where the two trust each other.
- The bill is a document, filed with the payee's other bills on the same drawee (one bundle per drawee).
- **It's paid only when presented, by a caravan between the two:** the drawee's caravan arriving where bills on it lie pays those due from its purse (it carries the money for them). The holder's caravan carries bills on its destination there and brings the coin home.
- Bills not yet due go back. Unpaid 4 days past due, a bill is dishonoured, and the pair's trust falls.

## War footing (built 2026-10-14)

A well-funded sovereign shifting the region onto a war economy, through the region's own institutions rather than conjured demand.

- **The capital and the treasury:** the sovereign's capital is a desk whose Blaze Banker is the royal treasury. It mints coin into it each day. Minting debases the spur: a shulker of emeralds' worth minted (27,648 spurs) doubles the spurs an emerald fetches, working its way out with a 30-day half-life.
- **Mobilising:** it builds a stockpile of kit in the capital's warehouse:
  - swords (wood, stone, iron, diamond), chestplates (leather, chainmail, iron, diamond), shields, bows, arrows;
  - the capital's master Merchant contracts towns for them at 1.5 times their worth (2.5 times at war);
  - it buys gold through a Numismatics shop by the capital at 3 times its worth.
- **Contracts name the tier, not the enchantments.** Any iron sword counts. Bonus clauses pay for quality without requiring it: Unbreaking 1 and Sharpness 2 or better earns 25% more on a sword; Protection 2 earns 25% more on armour.
- **What no town can supply is posted as wanted, and towns respond:**
  - they put idle hands to the trades the war calls for (a Weaponsmith for swords; iron goods want a Toolsmith's mine and an Armorer first);
  - they make what they've contracted to sell;
  - their Merchants switch on their own villagers' trades to buy it.
  - Smiths trade their way up the levels at their stalls. Diamond gear is a master smith's, sold for emeralds.
- **At war, levies:** a levy contract with each town the capital's caravans reach raises soldiers: its idle hands and a fifth of the rest, one to four. Idle hands go first, then a trade it has others of, then the least skilled; never a child, nor its Merchant. The recruits ride to the capital with its caravan.
- **Campaigning:** each day a soldier draws what kit it lacks from the capital's warehouse, the best there is, and wears it out (enchanted with Unbreaking, slower). It is paid 0.25 emeralds a day, owed to its town and carried there by the capital's caravans.
- **The fallen:** a soldier may fall, 2% a day before kit. A weapon, armour and shield cut it, better tiers more, and enchantments suited to the enemy more again: Sharpness and Protection against a faction, Smite against the undead, fire against the Nether. The fallen are gone from their town for good, and it grieves: half a point of mood each, for 10 days.
- **At peace** the soldiers walk home and the minting stops.
- **An army eats:** 8 food points a soldier a day, from the capital's warehouse. At war the sovereign keeps four days' rations stocked, sourced like the kit; Farmers sell bread, and towns supply what food they have beyond their caravans' needs. An unfed soldier runs half again the risk, and deserts at 10% a day (walking home, its kit lost). **Good food is good morale:** the army eats its heartiest food first, and a meal's heartiness (its saturation per point of food above plain bread's: cooked meat, stews, golden carrots) makes it fight up to a quarter harder, fall 15% less, and desert half as readily when short. The stockpile holds cooked beef and pork as well as bread; Butchers sell them.
- **The tally** (`/enterprise war tally`): by town, soldiers raised, fallen and deserted, soldier-days (and hungry ones), strength (soldier-days weighted by kit and its fitness for the enemy), kit delivered and emeralds earned; in all, the kit issued by kind and enchantment levels, food eaten, gold bought, coin minted, the spur's rate.
- **Measured** (`network_study.py --comforts --war 1,31,71`, 90 days: mobilising 30, at war 40, peace 20; three towns on the capital's roads):
  - **The region barely moved onto a war footing.** One town, S2, answered: it grew from 5 to 13 villagers and took up Leatherworker, Toolsmith and Weaponsmith stalls for the war's wants. Its Fletcher, through its own villager trades, supplied all 280 arrows delivered.
  - **Nothing else was made:** swords, armour, shields, bows and gold went unmet all 90 days.
  - **The army was raised but never armed:** 4 soldiers, 3 fallen, no kit issued.
  - **The treasury piled up 67,600 spurs it couldn't spend,** while minting took the spur from 16 to 29 an emerald.
  - **Why:**
    1. Trust froze contracting: small towns missed early installments (0.2 trust each), and by the war the capital's trust with S2 was 0.12 and with S1 0.00, below the 0.3 needed to draft. S2's Weaponsmith never got a sword contract.
    2. Towns of five or six have no hands to spare.
    3. No town completed the metal chain (mine, Armorer), so no iron, no gold.
    4. The sovereign's money had no way to reach a town before it could deliver.
  - **What's missing:** advances against contracts (money up front to hire, build and train), patience for a supplier ramping up (lead time, a softer first miss), and a way for coin to become people at work. That is the war finance to come: loans, bonds, taxes.
- **The long runs got 10 times faster** (2 seconds a game day; 90 days in 3 minutes): the study now keeps one RCON connection open, and reads the debug log from where it left off. The tick warp itself was never the slow part.

## Trust, advances and the capital's port (built 2026-10-14)

What run 1 of the war lacked: patience with new suppliers, money before production, and metal to train smiths on.

- **Trust is a track record**, not a meter. Each pair keeps installments kept and missed; trust is `(kept + 2) / (kept + missed + 4)`, a half for strangers.
  - **Consistency builds it:** each kept installment adds less as the record grows, and one bad week doesn't undo a long record (20 kept, 2 missed: still 0.79).
  - **Disruptions are forgiven:** a miss's weight halves every 20 days; kept installments every 120.
  - **New suppliers ramp up gently:** until a pair has kept 3 installments, a miss counts a quarter.
  - **Part deliveries count in part:** 27 of 30 is 0.9 kept. Only an installment short by half or more counts toward breaking the contract (3 such break it).
  - **Trust never freezes trade:** below 0.3 the two still deal, on trial terms (a lot of 4, nothing up front), so a record can be rebuilt. A bill paid when presented counts half a kept installment, a dishonoured one a whole miss; a paid service abroad a tenth.
  - `/enterprise trust` lists every pair's record.
- **Money before the goods: advances.** A contract to make goods carries an advance term: a third of its worth paid up front, carried by the buyer's caravan with the copy (coin from its purse, the rest a bill of exchange on the buyer). The seller works it off at the same share of each delivery's price. If the contract breaks, what isn't worked off is owed back and collected by the next caravan, as far as the seller can spare.
- **Work to make comes in small installments:** four, a day apart, after a three-day lead, rather than one lot of 16 due in two days. Steady deliveries build the record.
- **Inputs follow contracts.** A town contracted to make swords wants the iron for them: caravans carry a contract's inputs first, and the town buys up to what it needs. (With more wanted goods than cargo slots, a caravan used to carry nothing at all; it now carries a stack each, inputs first.)
- **The capital and its port** (`/enterprise war capital <desk> <spurs a day>`): a capital can be named in peacetime. Its treasury takes a peacetime revenue (minted for now; taxes will replace it), and its port opens a **metal exchange**:
  - it aims to hold iron, coal and flint (192/192/96) and diamonds (12), twice that while mobilising, three times at war;
  - ships bring what it lacks, three days at sea, paid from the treasury at 1.5 times worth;
  - it posts a bid and an ask a tenth either side of a middle price that rises (up to half again) as its stock runs short. The capital's caravans sell its metal to towns at the ask and buy towns' metal (gold too: the war chest) at the bid. Its bids stand as signals, so towns take up mines;
  - `/enterprise metals` shows its book.
- **Training smiths.** A villager the Merchant puts to work is its apprentice: as it levels, its new buying trades are switched on too (coal, iron, flint, diamonds). A smith with nothing to buy in the warehouse (a Novice Weaponsmith may buy nothing) has its cheapest work bought instead, while the town can spare it. At master its buying trades go off: its metal is kept for its contracts.
- **Fixes the runs turned up:**
  - Villagers in a town loaded only for a round never levelled up. Vanilla waits 40 of the villager's own ticks to apply a level, and a round may not give it those. Locker trades now level a villager at once.
  - Rounds skipped a market whose desk is loaded, so a villager that had wandered just past the loaded chunks stood frozen and never traded. A loaded market's stranded villagers now catch up once a day where they stand, and Procurement finds a villager anywhere in the market's reach.
  - Squads no longer take villagers who work a locker: the court's smiths kept leaving on caravans.
  - The Merchant changes coin for emeralds at the mint (5% over the rate, from money beyond its reserve) to pay its villagers. Caravan purses had drained the capital's emeralds, so it couldn't buy from its own smiths.
- **Measured** (`network_study.py --comforts --war 31,61,101`, 120 days: the capital named on day 1, mobilising from day 31, war from 61, peace from 101):
  - **The court's three smiths were masters by day 10**, trained on imported coal, iron, flint and diamonds. S3's Armorer reached master; S1's Level 2.
  - **251 pieces of kit delivered** (44 arrows in run 1): S2 179 for 833 emeralds, S1 72. The army was issued 3 enchanted diamond chestplates (Fire Protection, Unbreaking), 6 iron chestplates and 2 enchanted iron swords.
  - **Trust held:** 0.62, 0.42 and 0.81 at the end (0.12 and 0.00 in run 1), with 1,053 emeralds paid up front.
  - **Still short:**
    1. No diamond swords: the master Weaponsmith offers them, but none were bought.
    2. S2 sat on 814 emeralds, money it couldn't turn into more hands; S1 took 267 up front and earned 34.
    3. The army's meals were still 0% hearty: no meat reached the depot.
    4. The treasury ended with 76,000 spurs unspent, and the spur fell from 16 to 34 an emerald.

## Why training broke, and where the swords went (2026-10-14)

Run 16 trained three masters, broke eight training contracts, and issued five diamond swords. From its contracts and
locker logs:

- **Five of nine smith stalls never traded once.** A Novice Merchant manages two lockers, and town Merchants stayed
  Novices: a Merchant's level-up waited on its own tick timer, which a town loaded only for a round never runs (the
  smiths had the same bug, fixed earlier). Merchants now level up at once. After the fix the "beyond its locker
  limit" message never appeared.
- **A contract for a second master had no one to train.** A town with a master Weaponsmith that took a contract for
  another never hired a second smith: a trade was taken up only when the town lacked it. Now it is taken up whenever
  every villager of it is already at the level.
- **A master's goods were never bought.** The Merchant switched on its villagers' selling trades (diamond armour
  from a master Armorer) only when a caravan came, and then only for villagers loaded at that moment. S2's stalls
  stood in other chunks than its desk, so S2's master Armorer and Weaponsmith (masters by day 40) sold nothing: 30
  contracts for 48 diamond swords and 208 diamond armour pieces broke. The Merchant now does this at each villager's
  own restock, when that villager is certainly there.
- **The stockpile asked for little.** It wanted 4 diamond swords, for an army of about six. It now equips a planned
  army of 24: 16 diamond swords and 8 iron, 8 of each diamond armour piece and 16 of each iron one, shields for half.
- **Run 17** (the first three fixes in): 5 of 6 training contracts finished; 8 masters by day 120, S2's Armorer,
  Toolsmith and Weaponsmith all by day 40. **Run 18** (and buying at the restock): S2 delivered **148 pieces of
  diamond gear** (26 swords, 25 chestplates, 25 helmets, 36 leggings, 36 boots) and earned 9,250 emeralds; every
  soldier at the front was issued a full enchanted diamond set and a diamond sword. 5 training contracts finished,
  3 broke.
- **Now the army is the limit:** five to seven soldiers (small towns' levies), and half of them deserted hungry.

## Pacing and the crown's bank (built 2026-10-14)

Once room stopped blocking them, Merchants were blocked by cash. S2's Merchant tried to buy 405 emeralds' worth of
diamond gear from its master smiths at once, holding about 17.

- **Pacing:** a Merchant buys from its villagers for what its contracts want within two days (and its open orders,
  and the sovereign's shortfall there), not the whole of a long contract at once. A pupil's lessons still run a
  restock's worth.
- **Banks** (`Bank`, `/enterprise bank`; the capital opens one when it's named, and `/enterprise bank open <desk>`
  opens one anywhere, a barracks town say). A bank deals only through its own caravans, from its market's treasury,
  so money still goes only where a caravan carries it. Where its caravan calls:
  - **Discounting:** it buys the town's bills not yet due for coin now, less 1% and 1% a day to their due day. A bill
    drawn on the bank's own market is redeemed early; any other is endorsed to the bank and carried home to present.
  - **Lending against contracts:** a town with a contract in force and less money than half the goods still to come
    are worth is lent up to that half (less what it was paid up front and lent already), at 3%, secured on the
    contract. When the bank's caravan pays for those goods, or pays a training bonus, the loan comes out first. A loan
    left on a contract that's done or broken is collected from what the town can spare.
  - A bank's caravan carries up to 48 emeralds more for its dealings.
  So a bill in hand, or a contract with goods coming, is money today.
- **Run 16:** the crown's bank bought 51 bills (642 emeralds paid for paper worth 665 when due) and made 33 loans:
  389 emeralds lent, 396 repaid, 5 still owed at the end. Emerald purchases refused for want of money fell from 446
  to 38. Five enchanted diamond swords (Sharpness) were issued to the army. Training finished three masters (S1's
  Weaponsmith by day 40, S2's Armorer by day 60, S1's Armorer by day 80), with 8 training contracts broken against
  2 in run 15: single runs vary too much to judge a change by one.

## Inventory waste, cold storage and the apprentices' work (built 2026-10-14)

Training a smith by paying for its work makes things nobody wants: a Novice Weaponsmith's iron axes, one to a slot.
In the run before this, S1's one-chest warehouse was 25 slots of 27 full (18 of them axes), so its Merchant couldn't
take in the emeralds to pay for the next lesson, and its Armorer sat at Novice with 0 XP for 70 days. This is
vendoring greys, and the Merchant now does what a shopkeeper would.

- **Crates are storage.** A packed crate with no papers is part of the warehouse, nine stacks in one slot; the stock
  reaches into it like a shulker box. A crate with a bill of lading is someone's shipment and stays shut.
- **Storekeeping** (`ColdStore`), whenever a town is touched:
  1. hands working its land without a tool get one (an axe first): they gather half again as much, and tools wear
     out in 16 days. So axes have a use in the populace, and towns whose hands lack tools want them;
  2. short of room, the Merchant packs goods it isn't using into crates (those that don't stack first), and whole
     stacks of what it uses beyond a stack over its par (a standing order that brings more paper than its Librarian
     takes);
  3. the crates go to **cold storage**, a double chest it sets up by the desk the first time (1 emerald, the
     cheapest warehouse there is): a store that holds only crates;
  4. still short, it gives idle tools to its hands;
  5. last, it throws out greys (goods worth under 0.1 emerald each), least worth first, counted as waste. Anything
     better is kept, room or no.
- **The sovereign buys the apprentices' work** (an offtake). Its training tenders carry an offtake: the buyer takes
  what the town pays its pupil to make, at its worth. Under one, the pupil's paid work is crated at once, made out to
  the buyer with a bill of lading (title passes), and the town draws a bill of exchange on the buyer for it. The
  buyer's next caravan pays the bill and takes the crates home. Any caravan now fetches crates made out to its home.
- **Axes are lesser weapons** (iron axe: power 5, a little more risk than an iron sword). The front's quartermaster
  counts any weapon toward a weapon each, and any piece of armour toward each place. It contracts for the best the
  capital holds first: diamond from master smiths, then iron, then the apprentices' axes.
- **The Merchant trains smarter:**
  - lockers its contracts need come first in its locker list. A Novice Merchant manages two lockers, and a new
    smith's stall used to fall outside them, so it never traded at all;
  - it pays for a pupil's work only if the work has somewhere to go (an offtake, or the town wants it, or there's
    room to keep a restock's worth);
  - a training contract is for a new master. It records how many the town already had, so one master can't collect
    every tender. Before this, S1's single master Weaponsmith "completed" eight;
  - a Merchant takes a tender only with the money (beyond its reserve, with the advance) and the room (or a cold
    store) for it. It carries a tender on only to towns that can't already see the original board, and the
    sovereign counts tenders carried on as its own, so it doesn't post again for them.
- **Fixes from the analysis:** helmets, leggings, boots and axes were missing from the price table (worth 1/32
  emerald by default, so towns were contracted for iron helmets at 0.05). A town now counts as able to make goods
  only if a villager of it sells them, or a trade of it makes them from inputs it, or its buyer, holds. No more gold
  contracts without a mine.
- **Measured** (three 120-day runs as the fixes went in; capital named day 1, mobilising 31, war 61, peace 101):
  - **Masters, at last:** with the Merchant tending contract lockers first, run 10 trained S1's Armorer, Toolsmith
    and Weaponsmith and S2's Armorer to master. Run 11, after the new-master rule, trained four genuine masters: S2's
    Toolsmith and Weaponsmith and S1's Armorer by day 40, in peacetime. Diamond swords and chestplates (4 each)
    reached the capital from day 60. Run 12 trained only one (S3's Weaponsmith, by day 40), with 9 training
    contracts broken: results swing a lot between runs (which trades villagers roll, when caravans arrive).
  - **Waste:** run 11 threw out 384 raw iron at S2 and 192 coal at the capital. With greys-only and the exchange's
    metal counted as in use, run 12 threw out nothing; every town set up cold storage, and S2 gave its hands 4 axes.
  - **The army** was issued iron swords, chestplates, boots and helmets. No diamond gear reached the front in
    run 12: it had no master selling diamond swords by the war.
  - **Why most training still broke, from run 12's contracts and locker logs:**
    1. **Cash, not will.** S3's master Weaponsmith had its diamond-sword trade switched on, but a restock's three
       swords cost about 75 emeralds and S3 had 0 to 4 to spare. Its four diamond-sword contracts broke with none
       delivered, though each paid 26 emeralds up front: the advance sat in the treasury behind the town's reserve.
       And they were priced below cost: 19.8 a sword, where a master asks about 26 (the table had 12).
    2. **Room.** S2's warehouse is one 27-slot chest, and 13 slots were paper: its Librarian takes 384 a day, so two
       days' par is 768. Cold storage had packed only what was beyond par, so the Merchant still had no room to take
       in emeralds.
  - **So:** money paid up front and not yet worked off counts as money the Merchant can spend on its contracts'
    work. It pays for a lesson as soon as it can afford one, not a whole restock. Diamond gear is priced at what
    masters ask (sword 22, chestplate 21, leggings 19, axe 18). Short of room, it packs full stacks of anything it
    holds (one loose stack of each kept to hand), since crates are still stock.
  - **Run 13:** three masters (S2's Toolsmith and S3's Armorer by day 60, S1's Armorer by day 80), and the first
    diamond gear at the front: enchanted diamond leggings and boots. Weaponsmiths still stalled, short of room.
  - **Run 14,** with packing for room: **five masters, one training contract broken.** S2's Toolsmith by day 40, its
    Weaponsmith by day 60 and its Armorer by day 100; S1's Weaponsmith and S3's Armorer by day 60, mostly in
    peacetime. **Five enchanted diamond swords** (Sharpness, Fire Aspect) were issued to the army, and diamond boots
    reached the depot. Trust held between 0.52 and 0.83, and towns threw out under 5 emeralds' worth each.
  - **Still open:** soldiers went hungry 21 of 45 days, and meals are never hearty: no meat reaches the front.
- **Goods stay crated from shipper to buyer.** Shipments used to be opened on arrival and poured loose into the
  warehouse, then packed into crates again once it filled up. Now:
  - a crate made out to a Merchant goes into store as it comes, its papers filed;
  - a caravan packs what it carries for sale in crates of one good each (the hub's empties first, else new ones);
  - a buyer takes a whole crate when it wants the whole lot, and otherwise takes what it wants out, leaving the rest
    crated aboard;
  - goods come out of a crate only when used, and the emptied crate stays to ship with.
  Counts of what's aboard a caravan look inside crates.
  - **Run 15:** S2's main chest held 6 of 27 slots and its cold store 16 crates, against 24 of 27 slots, half of it
    paper, in run 13. Five masters again (two training contracts broken); diamond helmets and leggings were issued
    to soldiers, and 3 diamond swords waited in the depot at the end.
  - But towns packed for room about as often as in run 14 (36 to 69 times each over 120 days): most of the clutter is
    what they make and gather themselves. What still blocks a Merchant from paying its smiths is cash, not room: S2's
    Merchant tried to buy 405 emeralds' worth for its master smiths' diamond-gear trades at once, holding about 17.

## Everything runs on the logistics framework (built 2026-10-14)

No special routes: goods, money, soldiers and work reach where they're needed by contract, caravan and warehouse, as
everything else does. Only what happens at the ends is special.

- **Markets off the map** (`OffMap`): a place the region trades with that no one builds or visits. It sits at a real
  position, so roads to it are planned like any other, but its warehouse is kept in save data, its money in a bank
  account, and its Merchant is never seen. Caravans, contracts, bills and the treasury treat it like any desk.
  - **The front** opens when war is declared, 1,500 blocks north of the capital, with the treasury's account. Its
    quartermaster contracts the capital for what the army will want (whole kit sets, arrows, four days' rations),
    delivered by the capital's caravans. Levied recruits wait at the capital and ride out with those caravans. The army
    draws, wears out, eats and dies off the front's own stock: the end of the line. The front sells nothing; caravans
    going there carry their own food home.
  - **The market across the sea**, 2,400 blocks west, is a source: each day its stock of metal (and bread for
    travellers) is topped up.
- **Warehouses** (`Depots`, `/enterprise depot add <desk> <kind> <from> <to>`): a building whose chests, every one in
  its floorplan, are one store, part of its market's stock. What it's for decides what it holds: an **armoury** takes
  war goods, a **barracks** war goods and rations, a **storehouse** anything. Goods go to a building meant for them
  first, then to the desk's chests. A barracks' beds let its town put that many more men under arms. The capital has
  an armoury, and S2 has been promoted with a barracks.
- **The metal exchange buys by contract**, nearest holder first: a town with a mine, else the market across the sea,
  collected by the capital's caravans. It pays up to half again over worth as it runs short, and has buying shops at
  the port for players. The ships, and the price overrides in caravan buying and selling, are gone.
- **Training is a contract, and the market distributes it.** A training term ("train a Weaponsmith to level 5 for 24
  emeralds") is judged by the seller's census and paid when it shows the master, a third up front. The sovereign posts
  what it wants (2 master Weaponsmiths, 2 Armorers, 1 Toolsmith) as **tenders** on its board. Caravans carry each
  market's board both ways. A Merchant that reads a tender weighs it for its own town: the price, against a stall for
  a trade it lacks and coal and iron for each level to go, and whether there's time. It takes the tender if that pays.
  A Merchant whose town can't do it, but which has roads to towns of its own, carries the tender on, posting it on its
  own board for 15% less. Its contract with the first buyer is done when its town's is. A tender nobody takes lapses
  and is posted again for a quarter more.
- **The Merchant trains only under contract** (`Apprentices`): for each training contract, its pupil is the town's
  villager of that trade furthest along. The Merchant switches on what the pupil buys, pays for its work while the
  warehouse has none of what it buys (what's wanted first), and switches all of it off at the level. Jobs takes up a
  trade a training contract calls for.
- **Contract inputs are ordinary wants:** what a town needs to make what it has contracted to sell raises its
  warehouse's par (the stock it aims to keep), so caravans carry and sell it as they would anything it uses.
- **Armour comes in sets.** A soldier wears a helmet, chestplate, leggings and boots. Protection is the armour points
  worn: a full iron set is what a chestplate alone gave before, a full diamond set halves the risk. The stockpile and
  the quartermaster ask for every piece; Armorers sell them.
- **Game tests start from a fresh world:** `just gametest` wipes it first. Saved data (the mint's debasement, trust)
  carried over between runs and broke two market tests.
- **Measured** (120 days; capital named on day 1, mobilising from day 31, war from 61, peace from 101):
  - **The flows work:** the market across the sea delivered 3,625 metal by contract. The front received 313 pieces
    of kit and food, and issued 5 iron chestplates and 7 iron swords. War goods gathered in the armoury.
  - **Towns took the tenders and hired for them:** S1, S2 and S3 took up Armorer, Weaponsmith and Toolsmith stalls,
    and S1 carried a tender on to S2. **But no smith got past Level 2:** 13 training contracts broke.
  - **Why** (still to fix):
    1. S1 took 336 emeralds up front, broke contracts, and paid the advances back. Broke, it can't buy coal, so it
       can't train, so it breaks more.
    2. Its Merchant couldn't change coin for emeralds to pay its pupil, with 60 to 89 emeralds to spare; most likely
       its warehouse is too full to take them.
    3. Tenders are taken on a rough cost (2 emeralds a level, 2 days a level), not on whether coal and iron will
       actually reach the town in time.
  - Soldiers went hungry 34 of 87 days, and 3 deserted: the front's rations ran short between caravans.

## Travel cost between hexes (2026-10-13)

Travel cost must react to real building. A long wall, a tunnel, a bridge or a wide canal should each change routes and costs.

- **Two levels, like hierarchical pathfinding (HPA\*).** Inside a hex, a block-level 3D walk grid. Between hexes, a small graph.
- **Each hex side is split into entrances**, the open stretches of its border. A wall along a border with one gate leaves one entrance.
- **Per hex, we cache the cost from each entrance to every other**, per mode: on foot, cart, boat. Routes are A\* over the entrances graph, which is small and cheap.
- **A hex's costs are rebuilt on touch**: marked dirty when its blocks change, and rebuilt on the next round or census, off the main thread on a chunk snapshot.
- **Modes read different things:** foot needs 2 blocks of headroom and a 1-block step; carts need a road surface and width; boats need water depth and width (which is how a wide canal opens freighter routes).
- **Light sets cost underground.** Dark caves are slow and dangerous, so natural caves don't count as free tunnels, while a lit, built tunnel is cheap.

## Caravans (2026-10-13)

**Built so far (2026-10-14): caravans as ledger records, no entities yet.**
- **A route runs from a hub Merchant's desk to a settlement's Merchant's desk** (`/enterprise route add <from> <to>`). One caravan per route; at most one departure per route per in-game day; the hub's Merchant must be at its desk.
- **Outbound:** the hub loads up to 27 stacks of what the destination's switched-on trades take, from its stock and shulkers, plus a purse for the return leg's road food (1.5x what bread would cost). No purse, no caravan.
- **Caravans trade, they don't take (2026-10-14).** The travelling Merchant deals only with the resident Merchant, whose chest is the town's warehouse. It sells cargo into the warehouse up to par, buys the town's surplus for the trip home, and buys the return leg's road food at the local price. A town's coins are no longer swept up. **Home:** the caravan banks what it carries, after tolls, in the hub's Blaze Banker (or its chest); leftover and bought cargo goes into the hub's warehouse.
- **Warehouse pars.** Each locker session records what its switched-on trades can take in a day (price times uses per restock). A town's par for a good is two days of that capacity across its lockers, at least a stack for any good a switched-on trade takes. Capacity, not past use: par learned from use kept a short town short. Below par the resident buys; above par is surplus for sale. It also keeps 320 food points for caravans' road food and its emerald float.
- **Caravans are staffed by squads (2026-10-14).** A caravan takes one to four trained villagers from its home town: Merchants (not the one working the desk), Cartographers, Librarians and Shepherds, trained meaning they keep their trade without a workstation. Babies and untrained villagers stay home. One member is enough to drive the wagon (27 stacks, whatever the squad's size); each more goes only when its trade helps this trip and the town can spare its road food beyond the caravans' par, and every other route waiting at that desk keeps a lead. Squad members leave the world while on the road (their entity data rides in the ledger) and come back with the caravan; the census counts them as residents while away.
- **A squad comes from the desk's community (2026-10-14).** Only villagers its Merchant knows are mustered or trained: residents of its town (its census roster), workers at the lockers it manages, villagers who have gone out on its caravans before, and anyone standing in its market (within 8 blocks of the desk).
- **Any town sends caravans (2026-10-14).** The Merchant's Desk screen has a Caravans tab: the markets this desk knows, nearest first, with distance and direction, a Send caravans / Stop caravans button for each within a leg's food, the route's status, and who is at home to send. A route stopped while its caravan is out comes off the books when the caravan is home, so no squad or cargo is stranded.
- **News travels with carriers: each desk keeps an atlas.** A market knows those within 400 blocks by word of mouth; both ends of a route know each other once a caravan has run it; a town whose land a caravan crosses learns both its ends; Cartographers will map more.
- **Towns commission their own caravans.** At a toll review, a town with no route of its own, short of some kind of comfort, with 8 emeralds to spare beyond its reserve, starts a route to the nearest market it knows (within a leg's food) whose last known stock has that kind. Its caravan carries a shopping purse (the comforts it lacks, at full value, from money beyond its reserve) and buys them from the destination's stock beyond the destination's own par, bargained up to full value since they're for home. Leftover money comes home.
- **Towns train their squads.** A town with routes of its own trains one villager per count while loaded, up to a lead per route and one spare (at most 4): an unemployed adult of its community learns the trade the squad lacks (Merchant, Cartographer, Librarian, Shepherd, in that order), for 4 emeralds beyond its reserve. Trained, it keeps its trade without a workstation.
- **Catch-up waits for the Merchant (2026-10-14).** A caravan stop loads only the desk's chunk, and a locker villager loaded there used to replay its missed restocks while its Merchant, a few blocks over in an unloaded chunk, was missing: those days' trades were lost, and a town could go a whole run without trading (seen at S1, then S2). Now a villager waits to replay until its Merchant is loaded and managing its locker, looking again every 100 ticks; a round loads the whole market.
- **Cartographers map the road (2026-10-14).** A squad takes a Cartographer when its road hasn't been mapped in 10 days. At the destination it notes every settlement within 2 hexes of the road, which the home desk now knows of, and draws a map of the destination's country (512 blocks across, from what grows where, like an explorer map: nothing is loaded), the destination marked with an X and the towns seen marked as villages, named "Map of the road to x, z". The map rides home into the warehouse. The Caravans tab notes a mapped road and how many towns were seen; `/enterprise atlas <desk>` lists the markets a desk knows. Measured over 12 days: the hub's three roads were mapped once each, and the hub learned of the toll town beside the road to S3.
- **Measured** (`network_study.py --comforts`, 25 days): the toll town, which no route served, heard of the hub from caravans crossing its land, commissioned a caravan there for all four kinds of comfort, and its mood rose from -0.6 to +3.2; with no income of its own it spent its savings doing so. S1 commissioned one too, to S3, for the clothing S3 had bought from the hub. After the catch-up fix every town traded, and no locker went unmanaged for want of a loaded Merchant.
- **What each trade brings:** a Merchant haggles at its own level (without one, the squad haggles as a Novice); a Librarian brings back the books, the destination's stocktake, which the next caravan uses to know what to bring (taken along when the books are over 3 days old); a Cartographer walks the road (taken when it hasn't been mapped in 10 days; its maps come with the atlas); a Shepherd drives up to 8 grown farm animals from the market pen (within 8 blocks of the desk) and sells them where a town's pasture is short (2 animals a villager within 32 blocks of its bell), turning them out by its bell. The Shepherd is the first test of the fourth slot.
- **Measured:** sharing one squad of four among a hub's three routes, a squad took the whole of it and two routes sat idle; with one lead per route and extras only where they help, all three ran, with squads of one or two. Tying cargo to squad size (9 stacks a member) cut a town's trade from about 160 to 10 trades in 12 days, so cargo is per caravan.
- **Merchants bargain by level, not a fixed wholesale rate.** The seller won't go below half the goods' local value; the buyer will pay up to the full value with an empty shelf, scaling down to half at par (no deal at or above par). The price lands in that gap by level: the seller takes its level / (both levels) of it. The travelling Merchant's level is the hub Merchant's.
- **Travelling Merchants buy surplus only to fill a want.** A town's surplus is bought only for goods another town on the hub's routes wants, up to the sum of their pars less what the hub holds (from a stocktake the hub takes as each caravan leaves), at a ceiling of 75% of local value since it has to resell.
- **Prices** live in one table (`Prices`): what villagers pay for goods and crops, and what they charge for food, in items per emerald. Farm wages, wholesale and road food all use it.
- **Money moves through one treasury helper:** withdrawals take emeralds, then coins with change, then the Banker, and break an emerald at the mint when a part-emerald is owed.
- **Speed:** 1,200 blocks per in-game day. Arrivals load the desk's chunks on a short self-expiring ticket if nobody is there.
- **Food limits travel.** A caravan eats 1 food point (nutrition) per 10 blocks and carries at most 320 (a stack of bread), so a leg is at most 3,200 blocks; longer routes need a settlement in between as a waystation. **Each settlement's Merchant's chest is its food supplier:** a caravan eats the outbound leg's food at the hub and the return leg's at the destination. No food: it doesn't leave, or it waits at the destination.
- **Measured** (`dev/experiments/caravan_network.py`, pars and bargaining, each town starting with 64 emeralds): in 8 days the hub sold 7,916 items for 3,168 spurs and banked 3,404; its towns made 79-123 villager trades each and grew their money to 90-101 emeralds. A 4,000-block route was refused.
- **Working reserve refined:** a Merchant keeps 64 emeralds only if one of its switched-on trades costs emeralds; a selling-only settlement converts every emerald, so caravans have coins to collect.
- **Still to come:** encounters (spawning caravans near players, raiding), stealth and detection, tolls per hex, routes chained through waystations, surveying.

**Planned:**
- **A caravan is a cloud, not a position.** A route has traffic, and no one's location is simulated.
- When a player is in a route's corridor, an encounter roll runs, per hex:
  `chance = traffic × hex detection × (1 − merchant stealth)`.
- A hit spawns a real caravan with real cargo, checked out from its record. The record owns the cargo and the spawned caravan is a view of it, so it can never be duplicated. If the caravan crashes or gets stuck, the record wins.
- Merchant level adds stealth. Territories can raise a hex's detection. At full detection every caravan must show itself, so a guarded chokepoint enforces its own tolls.
- Raiding works on multiplayer: kill the caravan, loot the cargo, and the loss goes on the record.

## Build order (2026-10-13)

1. Lockers
2. Settlements (census, free-villager check)
3. Hex grid and territory
4. Route network (settlements as stops, food as travel supply)
5. Merchant (lockers, stock, counterparty, wages)
6. Caravans (encounters, stealth vs. detection, tolls)
7. Coinage and per-settlement banks

The cotton chain, chocolate, the cold chain and the exchange follow later.

## Open questions

- **Hex size tuning.** Measure real generated villages to confirm the starting size.
- **Improvements in player towns.** Is an improvement a recognized block structure (a farm plot, a mine entrance), something bought, or both?
- **Toll units.** Which units count as "the right type" to levy tolls?
- **Debasement.** Can a faction cut the metal in its coin to fund a war, at the cost of its exchange rate?
- **Guard mods.** Check whether an existing guard-villager mod for 1.21.1 NeoForge could supply the militia, or whether we write our own.
- **Scope.** Hexes, factions and garrisons push this toward a 4X game. The build order keeps each step playable on its own.
