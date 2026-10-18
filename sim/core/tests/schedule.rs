//! The look's schedule has no ambiguous pair: every two systems that touch the same state (a component, a resource,
//! the world) are ordered, by their steps or an `.after`/`.before`. Bevy breaks an unordered pair's tie by its graph's
//! hashes, which differ between a 64-bit host and wasm32: on main, `stranded::load` and `atlas::depart` ran one way
//! natively and the other under WebAssembly, so a wagon's stranded crates were in its Sent lines on one and not the
//! other, and the replay's digest split (`just core-wasm-replay`). This catches such a pair natively, at once.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{LogLevel, ScheduleBuildSettings};

#[test]
fn the_looks_schedule_orders_every_two_systems_that_touch_the_same_state() {
    let mut schedule = enterprise_core::rules::look_schedule();
    schedule.set_build_settings(ScheduleBuildSettings {
        ambiguity_detection: LogLevel::Warn,
        use_shortnames: false,
        report_sets: false,
        ..Default::default()
    });
    let mut world = World::new();
    let graph = schedule.graph_mut();
    graph.initialize(&mut world);
    graph.build_schedule(&mut world, &Default::default()).expect("the look's schedule builds");
    let mut pairs: Vec<String> = graph.conflicting_systems().to_string(graph, world.components())
        .map(|(a, b, on)| format!("{a} <> {b}, on {on:?}").replace("enterprise_core::", ""))
        .collect();
    pairs.sort();
    assert!(pairs.is_empty(), "{} pairs of systems touch the same state unordered (Bevy's tie-break differs by \
        target: order each as it runs natively, with `.after`/`.before`):\n{}", pairs.len(), pairs.join("\n"));
}
