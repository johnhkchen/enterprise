//! Taxes (John, 2026-11-03: "Next let's talk taxation": withheld payroll and income tax, land value tax, VAT; "Towns
//! and crown split"). The core's households are one purse a town, so a tax here moves money between a town's purse and
//! its treasury (or the crown's), not between rich and poor: what it does is put idle money where works are paid from.
//!
//! - **Income tax, withheld:** of every wage a town's treasury pays its households, and of what its villagers earn by
//!   their work (`hand_wage`), `income_tax` goes back to the treasury at the source, the same posting: nothing to evade.
//! - **Land value tax, the crown's:** each day a town on the capital's roads owes the rent of the hexes it holds
//!   ([`land_rent`]), whatever it does with them; the capital's wagons collect it where they call, and the town's own
//!   carry it to the capital (`rules/taxes.rs`). It pays for the king's highways (`bonds.rs`).
//! - **VAT, the towns':** a town's treasury is every firm in it (it buys the wagons' goods, keeps the warehouse, sells
//!   to its people), so the invoice chain's credits net to nothing inside a town and the tax falls where the chain ends:
//!   on what its households buy, their comforts from its warehouse and what they buy off a wagon there, `vat` of the
//!   price besides, to its treasury ([`vat`]). One rate on every good, the households of every town alike.
//! - **Harbour fees:** a ship of another town landing at a port pays it `harbour_fee` a stack it landed, out of its
//!   purse, for the quay and the jetty (John: "vessel/harbor fees for ships").

use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::fixed::Fixed;
use crate::state::Ledger;

pub fn income_tax_reason() -> Reason {
    Reason::Other("income tax".into())
}

/// Withholds the town's income tax on `gross` its households were just paid: from their purse to its treasury.
#[allow(clippy::too_many_arguments)]
pub fn withhold(content: &Content, ledger: &mut Ledger, book: &mut CashBook, town: usize, treasury: &mut Fixed,
        purse: &mut Fixed, gross: Fixed, now: u64, day: u64) {
    let tax = Fixed::floor(gross.to_f64() * content.dials.income_tax).min(purse.at_least_zero());
    if tax > Fixed::ZERO {
        post(ledger, book, Side::households(town, purse), Side::treasury(town, treasury), tax, income_tax_reason(), now, day);
    }
}

pub fn vat_reason() -> Reason {
    Reason::Other("VAT".into())
}

/// The town's VAT on `price` its households just paid for what they bought: from their purse to its treasury, as far
/// as the purse goes.
#[allow(clippy::too_many_arguments)]
pub fn vat(content: &Content, ledger: &mut Ledger, book: &mut CashBook, town: usize, treasury: &mut Fixed,
        purse: &mut Fixed, price: Fixed, now: u64, day: u64) {
    let tax = Fixed::floor(price.to_f64() * content.dials.vat).min(purse.at_least_zero());
    if tax > Fixed::ZERO {
        post(ledger, book, Side::households(town, purse), Side::treasury(town, treasury), tax, vat_reason(), now, day);
    }
}

/// A hex's rent a day, as the crown's assessor reckons it (the core sees every hex, so assessment is no trouble): the
/// dial `land_tax` for a hex all dry land its best crop suits fully, so much less for its water and a poorer fit, and
/// twice that for a town's home hex (its bell, its market, its houses).
pub fn land_rent(content: &Content, map: &crate::map::Map, hex: crate::hex::Hex, home: bool) -> f64 {
    let g = map.ground(hex);
    let at = match g.land {
        crate::map::Land::Plains => 0, crate::map::Land::Woods => 1, crate::map::Land::Taiga => 2,
        crate::map::Land::Desert => 3, crate::map::Land::Shore => 4, crate::map::Land::Hills => 5,
        crate::map::Land::Water => 6,
    };
    let fit = content.crops.iter().map(|c| c.fit[at]).fold(None, |m: Option<f64>, f| Some(m.map_or(f, |m| m.max(f))))
        .unwrap_or(1.0).clamp(0.0, 1.0);
    let dry = (1.0 - g.water_share).clamp(0.0, 1.0);
    content.dials.land_tax * dry * fit * if home { 2.0 } else { 1.0 }
}

pub fn harbour_reason() -> Reason {
    Reason::Other("harbour fees".into())
}

pub fn land_tax_reason() -> Reason {
    Reason::Other("land tax".into())
}

/// The crown's land value tax: what each town owes on the hexes it holds, assessed each day while it's on the
/// capital's roads, and what it has paid.
#[derive(bevy_ecs::prelude::Resource, Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Taxes {
    pub land_owed: std::collections::BTreeMap<usize, f64>,
    pub land_paid: std::collections::BTreeMap<usize, Fixed>,
    /// When the land was last assessed.
    pub assessed: Option<u64>,
}

impl Taxes {
    /// What a town owes of its land tax, in whole thousandths.
    pub fn owed(&self, town: usize) -> Fixed {
        Fixed::floor(self.land_owed.get(&town).copied().unwrap_or(0.0))
    }

    pub fn paid(&mut self, town: usize, amount: Fixed) {
        *self.land_owed.entry(town).or_default() -= amount.to_f64();
        *self.land_paid.entry(town).or_default() += amount;
    }
}

impl crate::society::Society {
    /// What each town owes of its land tax, and has paid.
    pub fn land_tax(&self, town: usize) -> (Fixed, Fixed) {
        let t = self.world.resource::<Taxes>();
        (t.owed(town), t.land_paid.get(&town).copied().unwrap_or_default())
    }
}
