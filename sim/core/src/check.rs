//! Checks that must hold however the rules change: what's in the world is what came into it, less what left it, and
//! no town is in debt. A study runs them every day and stops at the first one broken.

use bevy_ecs::prelude::*;

use crate::society::Society;
use crate::state::*;

/// The world's goods and money at a moment, with the ledger then, to check against later.
#[derive(Clone, Debug)]
pub struct Totals {
    pub goods: Vec<f64>,
    pub money: f64,
    ledger: Ledger,
}

impl Society {
    /// Every good held or on the road, and all money held or carried.
    pub fn totals(&mut self) -> Totals {
        let n = self.content().goods.len();
        let mut goods = vec![0.0; n];
        let mut money = 0.0;
        let mut towns = self.world.query::<(&Stock, &Money, &People)>();
        for (stock, m, people) in towns.iter(&self.world) {
            for g in 0..n {
                goods[g] += stock.0[g];
            }
            money += m.0 + people.purse;
        }
        let mut routes = self.world.query::<&RouteState>();
        for state in routes.iter(&self.world) {
            if let Some(w) = &state.wagon {
                for g in 0..n {
                    goods[g] += w.cargo[g];
                }
                money += w.purse;
            }
        }
        Totals { goods, money, ledger: self.world.resource::<Ledger>().clone() }
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
        let expected = before.money + (b.income - a.income) - (b.road - a.road) - (b.spent - a.spent);
        if (now.money - expected).abs() > 1e-6 * expected.abs().max(1.0) {
            return Err(format!("money: {} in the world, {} expected", now.money, expected));
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
