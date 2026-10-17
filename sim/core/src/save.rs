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
//! what they hold. A slice that adds one adds a line to `saved!` below: a resource under `resources`, a town's
//! component under `town`, a route's under `route`. Each is optional, so a component only some towns have (or a
//! resource only some worlds have) saves as it is. An `Entity` held in state is `#[serde(skip)]` and linked again in
//! [`load`], from a number that is saved. A new kind of entity gets its own list here, like towns and routes.
//!
//! The format is postcard (compact, no schema, builds for `wasm32-wasip1`), after a magic and a [`FORMAT`] number.
//! Postcard isn't self-describing: a blob saved before a field was added won't load after it. Bump [`FORMAT`] when the
//! state changes shape; a load of another format is refused, not misread.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::bills::Bills;
use crate::content::Content;
use crate::contracts::Book;
use crate::state::*;

/// The blob's first bytes.
pub const MAGIC: [u8; 4] = *b"ENTS";
/// The shape of the state; a blob of another is refused.
pub const FORMAT: u32 = 1;

/// What's saved: resources, and a town's and a route's components, each by a name of its own.
macro_rules! saved {
    (
        resources { $($r:ident: $R:ty),* $(,)? }
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
            fn take(world: &World) -> Self {
                Resources { $($r: world.get_resource::<$R>().cloned(),)* }
            }

            fn put(self, world: &mut World) {
                $(if let Some(r) = self.$r { world.insert_resource(r); })*
            }
        }

        impl SavedTown {
            fn take(e: EntityRef) -> Self {
                SavedTown { spawned: e.id().index_u32(), $($t: e.get::<$T>().cloned(),)* }
            }

            fn put(self, e: &mut EntityWorldMut) {
                $(if let Some(c) = self.$t { e.insert(c); })*
            }
        }

        impl SavedRoute {
            fn take(e: EntityRef) -> Self {
                SavedRoute { spawned: e.id().index_u32(), $($q: e.get::<$Q>().cloned(),)* }
            }

            fn put(self, e: &mut EntityWorldMut) {
                $(if let Some(c) = self.$q { e.insert(c); })*
            }
        }
    };
}

saved! {
    resources {
        content: Content,
        clock: Clock,
        ledger: Ledger,
        outbox: Outbox,
        book: Book,
        bills: Bills,
        mint: crate::mint::Mint,
    }
    town {
        town: Town,
        money: Money,
        stock: Stock,
        trades: Trades,
        touched: Touched,
        budget: Budget,
        people: People,
    }
    route {
        route: Route,
        state: RouteState,
    }
}

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

/// A world from a blob, with its towns' and routes' entities in order.
pub fn load(bytes: &[u8]) -> Result<(World, Vec<Entity>, Vec<Entity>), String> {
    if bytes.len() < 8 || bytes[..4] != MAGIC {
        return Err("not an Enterprise save".to_string());
    }
    let format = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if format != FORMAT {
        return Err(format!("a save of format {format}; this core reads {FORMAT}"));
    }
    let saved: Saved = postcard::from_bytes(&bytes[8..]).map_err(|e| format!("a broken save: {e}"))?;
    restore(saved)
}

/// A world from a [`Saved`]: resources put back, towns and routes spawned in their first order, routes' ends linked.
pub fn restore(saved: Saved) -> Result<(World, Vec<Entity>, Vec<Entity>), String> {
    let mut world = World::new();
    saved.resources.put(&mut world);
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
