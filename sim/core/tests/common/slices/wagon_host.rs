//! A host's wagons' part of the random worlds (`wagon_host.rs`): now and then a host starts wagons from one town to
//! another (`Input::Route`: a route opened if a leg's food reaches, one wound down opened again) or stops them (wound
//! down: its wagon comes home and stays). Checked, every day whatever the rules did with them: each town's routes as
//! its host asks them (`Query::Routes`) are its own, from it or to it; a wagon's crew are hands of its home away from
//! it; a market a town knows is another town, and one it's said to send wagons to has
//! an open route from it.

use super::super::*;
use enterprise_core::wire::{Answer, Input};

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if s.town_count() < 2 || r.below(5) != 0 {
        return;
    }
    let from = r.below(s.town_count() as u64) as u32;
    let to = (from + 1 + r.below(s.town_count() as u64 - 1) as u32) % s.town_count() as u32;
    s.apply(&Input::Route { from, to, send: r.below(3) != 0 });
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        let Answer::Routes { routes, markets, .. } = s.route_lines(t) else { return Err("not a routes answer".into()) };
        for line in &routes {
            if line.from as usize != t && line.to as usize != t {
                return Err(format!("town {t} lists route {} from {} to {}", line.route, line.from, line.to));
            }
            if let Some(w) = &line.wagon {
                let home = s.roster(line.from as usize);
                for id in &w.crew {
                    if !home.hands.iter().any(|h| h.id == *id && h.away) {
                        return Err(format!("route {}: crew hand {id} isn't away from its home, town {}", line.route, line.from));
                    }
                }
            }
        }
        for m in &markets {
            if m.town as usize == t {
                return Err(format!("town {t} knows itself as a market"));
            }
            if m.routed && !routes.iter().any(|l| l.from as usize == t && l.to == m.town && l.open) {
                return Err(format!("town {t} said to send wagons to {} with no open route there", m.town));
            }
        }
    }
    Ok(())
}
