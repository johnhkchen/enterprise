//! Checks that must hold however the rules change: what's in the world is what came into it, less what left it, and
//! no town is in debt. A study runs them every day and stops at the first one broken.
//!
//! Money is reconciled from the cash book's postings (`cash_book.rs`): every account's balance has moved by what was
//! posted to it, and the world's money by what the outside posted in less what it was paid; the ledger's running
//! totals (`income`, `road`, `spent`) must agree with the outside's postings.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use crate::cash_book::{Account, CashBook};

use crate::society::Society;
use crate::state::*;

/// The world's goods and money at a moment, with the ledger then, to check against later.
#[derive(Clone, Debug)]
pub struct Totals {
    pub goods: Vec<f64>,
    pub money: f64,
    /// What each account holds (a wagon's purse: 0 while it's home).
    pub balances: BTreeMap<Account, f64>,
    ledger: Ledger,
    posted: BTreeMap<Account, f64>,
}

impl Society {
    /// Every good held or on the road, and all money held or carried.
    pub fn totals(&mut self) -> Totals {
        let n = self.content().goods.len();
        let mut goods = vec![0.0; n];
        let mut money = 0.0;
        let mut balances = BTreeMap::new();
        let mut towns = self.world.query::<(&Town, &Stock, &Money, &People)>();
        for (town, stock, m, people) in towns.iter(&self.world) {
            for g in 0..n {
                goods[g] += stock.0[g];
            }
            money += m.0 + people.purse;
            balances.insert(Account::Treasury(town.id), m.0);
            balances.insert(Account::Households(town.id), people.purse);
        }
        // Crops banked in the farms' stores (`farms.rs`).
        let mut farms = self.world.query::<&crate::farms::Farm>();
        for farm in farms.iter(&self.world) {
            for (g, n) in &farm.banked {
                goods[*g] += n;
            }
        }
        // Goods crated in the towns' warehouses (`cargo.rs`).
        let mut stores = self.world.query::<&crate::cargo::Shipments>();
        for store in stores.iter(&self.world) {
            for s in &store.held {
                goods[s.good] += s.count;
            }
        }
        // Crates in the forwarders' hands at the freight hubs (`freight.rs`).
        let mut hubs = self.world.query::<&crate::freight::Hub>();
        for hub in hubs.iter(&self.world) {
            for h in &hub.held {
                goods[h.shipment.good] += h.shipment.count;
            }
        }
        // Goods on the town's own shops' shelves (`orders.rs`).
        for shop in &self.world.resource::<crate::orders::Orders>().shops {
            if let Some(g) = shop.good {
                goods[g] += shop.shelf;
            }
        }
        let mut routes = self.world.query::<(&Route, &RouteState)>();
        for (route, state) in routes.iter(&self.world) {
            let purse = state.wagon.as_ref().map_or(0.0, |w| w.purse);
            if let Some(w) = &state.wagon {
                for g in 0..n {
                    goods[g] += w.cargo[g];
                }
                for s in &w.shipments {
                    goods[s.good] += s.count;
                }
                money += w.purse;
            }
            balances.insert(Account::Wagon(route.seq), purse);
        }
        Totals { goods, money, balances, ledger: self.world.resource::<Ledger>().clone(),
            posted: self.world.resource::<CashBook>().net.clone() }
    }

    /// Since `before`: goods = before + made + delivered - used; money = before + money paid in - road food - money
    /// spent out of the world (homes, children).
    pub fn conserved(&mut self, before: &Totals) -> Result<(), String> {
        let now = self.totals();
        let (a, b) = (&before.ledger, &now.ledger);
        for g in 0..now.goods.len() {
            let expected = before.goods[g] + (b.made[g] - a.made[g]) + (b.delivered[g] - a.delivered[g]) - (b.used[g] - a.used[g]);
            if (now.goods[g] - expected).abs() > 1e-6 * expected.abs().max(1.0) {
                return Err(format!("{}: {} in the world, {} expected", self.content().goods[g].name, now.goods[g], expected));
            }
        }
        let posted = |t: &Totals, a: &Account| t.posted.get(a).copied().unwrap_or(0.0);
        let close = |x: f64, y: f64| (x - y).abs() <= 1e-6 * x.abs().max(y.abs()).max(1.0);
        // The outside's postings: what it paid in less what it was paid, the world's money's whole change.
        let came_in = -(posted(&now, &Account::Outside) - posted(before, &Account::Outside));
        let expected = before.money + came_in;
        if !close(now.money, expected) {
            return Err(format!("money: {} in the world, {} expected", now.money, expected));
        }
        let by_ledger = (b.income - a.income) - (b.road - a.road) - (b.spent - a.spent);
        if !close(by_ledger, came_in) {
            return Err(format!("money: the ledger has {by_ledger} come in, the cash book {came_in}"));
        }
        // Every account moved by what was posted to it: no money moved off the books.
        for (account, held) in &now.balances {
            let was = before.balances.get(account).copied().unwrap_or(0.0);
            let moved = posted(&now, account) - posted(before, account);
            if !close(held - was, moved) {
                return Err(format!("{account:?}: holds {held}, held {was}, but {moved} was posted to it"));
            }
        }
        Ok(())
    }

    /// No town in debt, and none holding less than nothing of a good.
    pub fn solvent(&mut self) -> Result<(), String> {
        let names: Vec<String> = self.content().goods.iter().map(|g| g.name.clone()).collect();
        let mut towns = self.world.query::<(&Town, &Money, &Stock)>();
        for (t, m, stock) in towns.iter(&self.world) {
            if m.0 < -1e-9 {
                return Err(format!("{} is in debt: {}", t.name, m.0));
            }
            if let Some(g) = stock.0.iter().position(|n| *n < -1e-9) {
                return Err(format!("{} holds {} {}", t.name, stock.0[g], names[g]));
            }
        }
        Ok(())
    }
}
