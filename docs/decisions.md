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
- **Route distance** is the straight distance plus a hex's width for each hex a detour adds over the straight line of hexes.
- **Warehouses keep 5 slots free** for locker results and coins: farm produce and caravan deliveries stop short of them, and a good's par is capped so wanted goods fill at most 60% of the warehouse. A full chest of unsold produce had stopped towns trading after about 9 days.
- **Rounds hold their chunks until the work is done.** A round loads its footprint on a ticket with no timeout, waits until the chunks and their villagers have loaded, then runs catch-up and the settlement's census, holds 200 ticks, and releases the chunks; it gives up after 30 seconds of real time. A tick-based timeout failed under `/tick sprint`, where 300 ticks pass in milliseconds while chunks take real time to load.
- **Rounds load the market, not just the desk:** chunks within 16 blocks of the desk and of the bell (where the Merchant and villagers spend their time), the locker workstations, and the settlement's field chunks, at most 25. A round that loaded only the desk's chunk missed a Merchant standing in the next chunk, and its lockers went unmanaged that day.
- **Censuses guard against half-loaded villages:** a round-loaded settlement is counted only once entities have loaded around its bell, and a census finding under half the villagers of one from the last 3 days is treated as incomplete and retried.
- **A Merchant with few slots manages lockers with trades switched on first**, then the nearest.
- **Measured** (`caravan_network.py`, two runs): with five settlements left unloaded for 8 days, every farm produced 16 carrots a day, territories grew to 3-4 hexes, the hub banked 3,580-3,975 spurs, and the route crossing the toll town paid it 27-31 spurs.

## Mood and comforts (built 2026-10-14)

- **Comforts are what towns spend spare money on.** A kind of comfort is an item tag under `enterprise:comforts/`: to start, treats (cookies, cake, pumpkin pie, honey), clothing (leather armour), furnishings (carpets, candles, paintings, flower pots, lanterns) and reading (books). A pack adds a kind (tea, cigarettes, another mod's finished clothes) by adding a tag.
- **Villagers use them up.** Each villager uses a tenth of an emerald's worth of each kind a day from the town's warehouse, at local prices, cheapest goods first and whole items only (a cake covers several days). A ration that can't be met is gone without, not owed.
- **Mood is Civilization's happiness.** Each kind fully supplied adds 1. Every 8 villagers take 1 away (bigger towns want more), each villager without a bed takes 0.5, and wholly unpaid wages take 2. Mood closes half its gap to that every 3 days. A content town's farms yield a tenth more per point of mood (between half and one and a half times) and it gathers a quarter more culture per point (between a quarter and double), so it grows its territory faster. `/enterprise territory <bell>` shows mood, what's met and the effects.
- **Towns buy comforts by caravan.** A town aims to hold 4 days of each kind (villagers x 0.1 x 4 emeralds' worth), buying whole items, at least one when short. It pays only from money beyond its wage float and 3 days of guards' pay. A caravan loads comforts from the hub for what the town wanted at its last stocktake (taken at each caravan visit), half again over, before it fills the rest of its cargo.
- **Measured** (`network_study.py --comforts`, 20 days, the hub stocking one shulker of comforts and its books): the three supplied towns reached mood +3.3 (all four kinds met; farms x1.33, culture x1.83). The two towns without a route sank to -0.6. Town money stopped climbing: S1 peaked at 125 emeralds on day 10, when the hub ran out of trade goods, and fell to 96 by day 20 while it kept buying comforts.

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
