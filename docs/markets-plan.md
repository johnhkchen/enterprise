# Markets: an order book in every town's shop

John, 2026-10-31: "Let's use supply and demand as the core mechanism ... letting the Create Numismatics shops mediate
trade between settlements ... Paper is emerging to be the first major trade good, and we actually have many settlements
that are significantly desert/sand biome based ... the mechanism should be generalizable: once we add the cotton mod, it
should be feasible to inject buying demand into said shop system to cause villages to shift production towards modded
cotton, or whatever crop they deem suitably profitable." Decided with him the same day: an order book in the shops (not
a price set from days of cover), crops defined in datapack JSON, and a quality multiplier on comforts.

The brief's spot market ("bales, yarn, beans and sugar trade on a real order book ... villagers and brokers price from
it"; "a town's best crop is rarely the one it wants, so regions specialize and trade") is this, built for villages first.

## Why now: seed 1717's reading

The 240-day run read 1,159 books of the 27,093 its villagers needed (4%).

- **Demand.** A villager needs 0.1 emeralds' worth of reading a day and a book (worth 0.25) is the only reading: a book
  every 2.5 days each. At day 240, 637 villagers needed 255 books a day: 765 paper and 255 leather.
- **Supply.** The run grew 28,128 sugar cane in all (173 a day by day 240), at most 35% of the need had every cane
  been bound.
- **The cane was planted and not worked.** Towns held 27 to 132 cane plots each, and their Farmers tended almost none
  (2,394 plots gave 173 a day; tended, they'd give 1,197). Farmers work crops by fixed worth, and cane (0.042) loses to
  wheat and potatoes however short of paper the town is. Nothing in a price tells them otherwise.
- **The parts sell for more than the book.** 3 paper and 1 leather are worth 0.29 at the vanilla trades' prices, the
  book 0.25: Merchants send paper and leather to the lockers that buy them, not to the Librarians that bind.

A price that moves with what the network is short of fixes all four: scarce paper is dear, dear paper is worth tending
cane for, and cane grows best on the sand the desert towns have plenty of.

## What exists

- Fixed worth per good (`prices.rs`, the vanilla villagers' prices); a town's eagerness for a comfort it lacks, up to 2x
  worth (`desires::will_pay`).
- **Listings** (`desires::Listings`): a town's shop lists up to three goods it wants, at what it would pay; a wagon
  calling reads them, and that's news Merchants elsewhere price from. Half a book: bids only, three of them, comforts
  and locker goods only.
- Players' buying shops as standing orders and selling shops as supply, for a master Merchant (`orders.rs`); signals for
  what no market holds.
- Farms: what Farmers tend and what new fields grow follow the town's wants (its lockers' demand, its workshop's draw),
  ranked by fixed worth (`farms::plan`, `rules/farms.rs` `field_wanted`). The crops are a fixed list in code (`CROPS`,
  `ROTATION`, `SOLD_AS`, `CONVERSIONS`), and land doesn't enter: wheat grows on sand as well as on loam.

## The design

### 1. The book

Each town's shop keeps a book: for each good, its **bid** (what it will pay, for how many) and its **ask** (what it
will sell at, how many). One bid and one ask per good per town, kept as a few numbers, moved on touch.

- **Bids come from use.** A comfort the town is short of (today's `will_pay`); an input its workshop would turn into
  something worth more (a Librarian's paper bid is what the book it makes would fetch, less the leather and its work);
  goods owed under contract; a player's buying vendor (its price, its purse). The bid's ceiling is the use's value
  to the town; the town never pays more than the good is worth to it.
- **Asks come from surplus.** What it holds beyond its par, at no less than its floor (what the good cost it: a crop's
  wages; a made good's inputs).
- **Prices move toward clearing.** A bid unfilled for a day creeps up toward its ceiling; an ask unsold creeps down
  toward its floor; a fill marks the price. Each good's **mark** (the last price it traded at in the town) is the town's
  price for it: what its Farmers, workshop and Merchant read.
- **The shop shows it.** The town's Numismatics vendors show its top bids and asks (the host's), so a player sees what
  the town wants and what it pays, and can sell to it or buy from it.

### 2. News travels with carriers

A town knows another's book as of the last wagon or letter from there: today's `Listings::seen`, widened to the whole
book (bids and asks, with the day seen). Nothing crosses distance without a carrier: a far town's high bid reaches a
desert town only when a wagon from somewhere that saw it does.

### 3. Caravans trade the spread

A Merchant plans a trip by the spread it knows: buy at an ask, sell to a bid, less the road. Goods go where the bid
net of haul is highest, not where "a trade it has switched on takes it". At the far end the wagon fills the bids it
can, at the bid, and buys what asks beat a bid it knows elsewhere.

### 4. Farms follow prices, on their land

- **Each crop's price to a farm** is the best of its town's mark and the bids it knows elsewhere net of the haul there,
  counting a crop by what it's sold as or converted into (cane as paper, wheat as bread).
- **Farmers tend** crops by price x yield per plot, not fixed worth.
- **New fields** go to the crop that earns most per plot on the land the field lies on.
- **Land suits crops.** Each crop has a yield factor per land (cane 1.0 on desert and shore, by water; wheat 0.3 on
  desert). A desert town's sand plots earn little in wheat and well in cane, so it plants cane once paper pays.

### 5. Crops are data

`data/enterprise/crops/<id>.json`: the item, its yield factor per land, whether it needs water, what it's sold as and
what it converts into (with which trade, at what ratio). The mod reads them and sends them to the core as content; the
core's fixed lists go. Adding cotton, tobacco, indigo, Farmer's Delight's crops or flowers is adding files: a crop with
a file and a bid somewhere is a crop villages can grow for money.

### 6. Quality

Each comfort good carries a quality (1.0 for plain). Comfort met is weighted by the quality of what was used, so a day
of clothing met by jeans (quality 2) lifts mood more than one met by leather boots, up to 2x a plain day's. The decision
"each quality tier is its own item" gives each tier its own quality.

### 7. Players inject demand

A player's buying vendor is a bid in its town's book, at its price, paid from its purse. Its news spreads by wagon, and
towns whose land suits the crop shift to it while the bid is filled at a price that beats their other crops. A Create
jeans factory buying cotton at a good price, sustained by its sales, is the bid; villages growing cotton for it are
the answer. When the bid stops, the price falls and the fields go back.

### Footprint

One bid and one ask per good per town (a good with neither keeps nothing); prices move on touch; news is a copy of the
book's numbers. Nothing per tick.

## Phases

Each phase lands on its own, gated, with a short study sized to what it checks.

1. **Built 2026-10-31 (see the decisions).** **The book in the core.** Bids and asks from today's wants and surplus, creeping prices, marks from fills; wagons sell
   to bids and buy from asks; `Listings` becomes the book's news. Trace tables `book` and `fills`. Measure: a 30-day
   lane on seed 1717 (about 10 minutes); paper's mark rises where it's short. About 4 hours.
2. **Farms follow prices.** Tending and fields by price x land yield; the core learns each field's land. Measure: seed
   1717's desert towns tend cane and plant it; paper and reading over 60 days. About 2 hours and a 20-minute run.
3. **Crops as data.** The datapack files, the wire, the fixed lists gone; game tests for a crop added by file. About 3
   hours.
4. **Players' shops as bids.** A player's buying vendor in its town's book (the binding); a study flag to inject one
   (`--bid TOWN:GOOD:PRICE:COUNT`). Measure: a 3x paper bid at a hub turns far towns to cane. About 2 hours.
5. **Quality.** The multiplier, in content and mood. About an hour.

Later, once the mods are in the pack: cotton, tobacco and indigo files; the Weaver; a jeans item of quality 2 from a
Create line.
