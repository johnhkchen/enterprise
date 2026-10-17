//! Wheat a farm sells its mill in 120 days, daily wagons against one every four days, as the mill's cash grows:
//!     cargo run --release --example budget
use enterprise_core::{Content, Good, Society};
fn main() {
    for money in [8.4, 10.0, 16.4, 26.4, 56.4] {
        let sold = |every: f64, compound: bool| {
            let content = Content::new(vec![Good::new("wheat", 0.05, 64)], 24_000);
            let mut s = Society::new(content);
            let farm = s.add_town("farm", 20.0); s.content_mut().dials.standing_orders = false;
            let mill = s.add_town("mill", money);
            s.set_trade(farm, 0, 128.0, 0.0);
            s.set_trade(mill, 0, 0.0, 64.0);
            s.add_route(farm, mill, 1.0, 0.5, every);
            if !compound { s.content_mut().dials.max_budget_days = 1.0; }
            s.advance(120 * 24_000);
            s.route(0).sold
        };
        println!("mill money {money:>4}: daily {:>6.0}  four-day {:>6.0}  four-day without compounding {:>6.0}", sold(1.0, true), sold(4.0, true), sold(4.0, false));
    }
}
