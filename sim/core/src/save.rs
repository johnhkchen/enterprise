//! The society's state as one blob, for the game to keep with its own save (`docs/architecture.md`: "its state is
//! one blob the game saves"). Reload an older blob and the society goes back with it; load, then run, and it comes out
//! the same as if it had never been saved, to the last bit.
//!
//! The mod keeps each piece in its own `SavedData` (`Dials.java` `save`/`load`, `ContractBook`, `Bills`...); the core
//! keeps one [`Saved`]: every resource, then every town's components (in `Town.id` order) and every route's (in
//! `Route.seq` order). A load spawns them again in the order they were first spawned, so the `Towns` and routes lists,
//! and the entities' ids, line up with the saved world's; a route's ends (`Route.from`, `Route.to`) are linked to
//! their towns' entities again from `from_id` and `to_id`, so nothing saved depends on an entity id.
//!
//! **Every type that's state derives `serde::{Serialize, Deserialize}`** (and `Default`): components, resources and
//! what they hold. A slice that adds one adds a line to the `saved!` list in `slices.rs`: a resource under `resources`
//! (with `= value` if a new society starts with it), a town's component under `town`, a route's under `route`; a test
//! (`tests/save.rs`) fails while any state in a world isn't listed. Each is optional, so a component only some towns have (or a
//! resource only some worlds have) saves as it is. An `Entity` held in state is `#[serde(skip)]` and linked again in
//! [`load`], from a number that is saved. A new kind of entity gets its own list in the macro, like towns and routes.
//!
//! The format is postcard (compact, no schema, builds for `wasm32-wasip1`), after a magic and a [`FORMAT`] number.
//! Postcard isn't self-describing: a blob saved before a field was added won't load after it. Bump [`FORMAT`] when the
//! state changes shape after a release (the integrator's job; until the first, it stays 1), with a migration
//! ([`migrate`]: one step a format, run on load from the blob's format up to today's, so an old world opens in a newer
//! core). A blob of a newer format, or of one the chain no longer reaches, is refused, not misread.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::state::*;

pub mod migrate;

/// The blob's first bytes.
pub const MAGIC: [u8; 4] = *b"ENTS";
/// The shape of the state; a blob of another is refused.
pub const FORMAT: u32 = 1;

/// What's saved: resources, and a town's and a route's components, each by a name of its own.
#[doc(hidden)]
#[macro_export]
macro_rules! saved {
    (
        resources { $($r:ident: $R:ty $(= $init:expr)?),* $(,)? }
        town { $($t:ident: $T:ty),* $(,)? }
        route { $($q:ident: $Q:ty),* $(,)? }
    ) => {
        /// Every resource the society keeps (the `Towns` list aside: a load rebuilds it).
        #[derive(Clone, Debug, Default, Serialize, Deserialize)]
        pub struct Resources {
            $(pub $r: Option<$R>,)*
        }

        /// A town's components.
        #[derive(Clone, Debug, Default, Serialize, Deserialize)]
        pub struct SavedTown {
            /// Where it came in the spawning, among towns and routes.
            pub spawned: u32,
            $(pub $t: Option<$T>,)*
        }

        /// A route's components.
        #[derive(Clone, Debug, Default, Serialize, Deserialize)]
        pub struct SavedRoute {
            pub spawned: u32,
            $(pub $q: Option<$Q>,)*
        }

        impl Resources {
            pub(crate) fn take(world: &World) -> Self {
                Resources { $($r: world.get_resource::<$R>().cloned(),)* }
            }

            pub(crate) fn put(self, world: &mut World) {
                $(if let Some(r) = self.$r { world.insert_resource(r); })*
            }

            /// Puts every resource listed with `= value` that the world hasn't yet into it, in the list's order: what
            /// a new society starts with.
            pub(crate) fn seed(world: &mut World) {
                $($(if !world.contains_resource::<$R>() { world.insert_resource::<$R>($init); })?)*
            }
        }

        /// The types `saved!` lists: resources, a town's components, a route's.
        pub(crate) fn saved_types() -> [Vec<core::any::TypeId>; 3] {
            use core::any::TypeId;
            [vec![$(TypeId::of::<$R>()),*], vec![$(TypeId::of::<$T>()),*], vec![$(TypeId::of::<$Q>()),*]]
        }

        impl SavedTown {
            pub(crate) fn take(e: EntityRef) -> Self {
                SavedTown { spawned: e.id().index_u32(), $($t: e.get::<$T>().cloned(),)* }
            }

            pub(crate) fn put(self, e: &mut EntityWorldMut) {
                $(if let Some(c) = self.$t { e.insert(c); })*
            }
        }

        impl SavedRoute {
            pub(crate) fn take(e: EntityRef) -> Self {
                SavedRoute { spawned: e.id().index_u32(), $($q: e.get::<$Q>().cloned(),)* }
            }

            pub(crate) fn put(self, e: &mut EntityWorldMut) {
                $(if let Some(c) = self.$q { e.insert(c); })*
            }
        }
    };
}

pub use crate::slices::{Resources, SavedRoute, SavedTown};
use crate::slices::saved_types;

/// The whole society: its resources, its towns in `Town.id` order and its routes in `Route.seq` order.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Saved {
    pub resources: Resources,
    pub towns: Vec<SavedTown>,
    pub routes: Vec<SavedRoute>,
}

/// The world's routes' entities, in `Route.seq` order.
pub fn routes_in_order(world: &mut World) -> Vec<Entity> {
    let mut q = world.query::<(Entity, &Route)>();
    crate::rules::in_order(q.iter(world))
}

/// The world as a [`Saved`].
pub fn saved(world: &mut World) -> Saved {
    let routes = routes_in_order(world);
    let towns = world.resource::<Towns>().0.clone();
    Saved {
        resources: Resources::take(world),
        towns: towns.iter().map(|&e| SavedTown::take(world.entity(e))).collect(),
        routes: routes.iter().map(|&e| SavedRoute::take(world.entity(e))).collect(),
    }
}

/// The world as a blob.
pub fn save(world: &mut World) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&FORMAT.to_le_bytes());
    postcard::to_extend(&saved(world), bytes).expect("the state serializes")
}

/// The format a blob says it is, if it is one (its magic is right and the number is there): the first thing to read of
/// a blob, to decide whether the core can read it at all ([`migrate::reads`]).
pub fn format_of(bytes: &[u8]) -> Result<u32, String> {
    if bytes.len() < 8 || bytes[..4] != MAGIC {
        return Err("not an Enterprise save".to_string());
    }
    Ok(u32::from_le_bytes(bytes[4..8].try_into().unwrap()))
}

/// A blob of any format the chain reaches, as today's state: the payload run through [`migrate::CHAIN`] from the
/// blob's format.
pub fn upgraded(bytes: &[u8]) -> Result<Saved, String> {
    let format = format_of(bytes)?;
    let payload = if format == FORMAT {
        std::borrow::Cow::Borrowed(&bytes[8..])
    } else {
        std::borrow::Cow::Owned(migrate::upgrade(format, &bytes[8..]).map_err(|e| e.to_string())?)
    };
    postcard::from_bytes(&payload).map_err(|e| format!("a broken save: {e}"))
}

/// A world from a blob, with its towns' and routes' entities in order. A blob of an older format is brought forward
/// through [`migrate::CHAIN`]; a newer one is refused.
pub fn load(bytes: &[u8]) -> Result<(World, Vec<Entity>, Vec<Entity>), String> {
    restore(upgraded(bytes)?)
}

/// A world from a [`Saved`]: resources put back, towns and routes spawned in their first order, routes' ends linked.
pub fn restore(saved: Saved) -> Result<(World, Vec<Entity>, Vec<Entity>), String> {
    let mut world = World::new();
    saved.resources.put(&mut world);
    world.init_resource::<bevy_ecs::schedule::Schedules>(); // as `Society::new` does: the same places in the spawning
    if !world.contains_resource::<Towns>() {
        world.insert_resource(Towns::default());
    }
    for (i, t) in saved.towns.iter().enumerate() {
        if t.town.as_ref().map(|t| t.id) != Some(i) {
            return Err(format!("town {i} is out of place"));
        }
    }
    for (i, r) in saved.routes.iter().enumerate() {
        match &r.route {
            Some(route) if route.seq == i && route.from_id < saved.towns.len() && route.to_id < saved.towns.len() => {}
            _ => return Err(format!("route {i} is out of place or goes nowhere")),
        }
    }
    // Spawned again in the order they first were: towns in id order and routes in seq order, interleaved as before.
    let mut order: Vec<(u32, bool, usize)> = saved.towns.iter().enumerate().map(|(i, t)| (t.spawned, true, i))
        .chain(saved.routes.iter().enumerate().map(|(i, r)| (r.spawned, false, i)))
        .collect();
    order.sort();
    let mut town_entities = vec![Entity::PLACEHOLDER; saved.towns.len()];
    let mut route_entities = vec![Entity::PLACEHOLDER; saved.routes.len()];
    let (mut towns, mut routes): (Vec<Option<SavedTown>>, Vec<Option<SavedRoute>>) =
        (saved.towns.into_iter().map(Some).collect(), saved.routes.into_iter().map(Some).collect());
    for (_, is_town, i) in order {
        let mut e = world.spawn_empty();
        if is_town {
            town_entities[i] = e.id();
            towns[i].take().unwrap().put(&mut e);
        } else {
            route_entities[i] = e.id();
            routes[i].take().unwrap().put(&mut e);
        }
    }
    for &e in &route_entities {
        let mut route = world.get_mut::<Route>(e).unwrap();
        route.from = town_entities[route.from_id];
        route.to = town_entities[route.to_id];
    }
    world.resource_mut::<Towns>().0 = town_entities.clone();
    Ok((world, town_entities, route_entities))
}

/// What in `world` isn't in `saved!`, in words: a resource, or a town's or a route's component, that a save would lose,
/// or an entity that is neither a town nor a route. Empty when everything is kept.
pub fn unsaved(world: &World) -> Vec<String> {
    use core::any::TypeId;
    let [resources, town, route] = saved_types();
    // Rebuilt by a load, or Bevy's own.
    let rebuilt = [TypeId::of::<Towns>(), TypeId::of::<bevy_ecs::entity_disabling::DefaultQueryFilters>(),
        TypeId::of::<bevy_ecs::schedule::Schedules>()];
    let name = |id| world.components().get_info(id).map_or_else(|| format!("{id:?}"), |i| i.name().to_string());
    let type_of = |id| world.components().get_info(id).and_then(|i| i.type_id());
    let mut out = Vec::new();
    let mut holders = Vec::new();
    for (id, e) in world.resource_entities().iter() {
        holders.push(e);
        if !type_of(id).is_some_and(|t| resources.contains(&t) || rebuilt.contains(&t)) {
            out.push(format!("the resource {}", name(id)));
        }
    }
    for e in world.iter_entities() {
        if holders.contains(&e.id()) {
            continue;
        }
        let (kind, kept) = if e.contains::<Town>() {
            ("a town's", &town)
        } else if e.contains::<Route>() {
            ("a route's", &route)
        } else {
            let all: Vec<String> = e.archetype().components().iter().map(|&id| name(id)).collect();
            out.push(format!("an entity neither a town nor a route ({})", all.join(", ")));
            continue;
        };
        for &id in e.archetype().components() {
            if !type_of(id).is_some_and(|t| kept.contains(&t)) {
                out.push(format!("{kind} component {}", name(id)));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
