use once_cell::sync::Lazy;
use std::sync::{Mutex, atomic::{AtomicU64, Ordering}};
use std::collections::HashMap;
use rapier3d::geometry::CollisionEvent;
use spacetimedb::{ReducerContext, Identity, Timestamp, Table};
use crate::physics::PhysicsContext;
use crate::tables::physics_body::physics_body;
use crate::tables::contact_duration::{ContactDuration, contact_duration};
use crate::physics::types::{ContactPair, PhysicsBodyId};

// Track ongoing contact start times: key = (region, ContactPair)
static CONTACT_STARTS: Lazy<Mutex<HashMap<(u32, ContactPair), Timestamp>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
// Unique ID generator for ContactDuration rows
static CONTACT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Handle a single collision event for contact-duration tracking
pub fn handle_event(ctx: &ReducerContext, region: u32, event: &CollisionEvent, world: &PhysicsContext) {
    match event {
        CollisionEvent::Started(h1, h2, _) => {
            if let (Some(c1), Some(c2)) = (
                world.colliders.get(*h1),
                world.colliders.get(*h2)
            ) {
                if let (Some(rb1), Some(rb2)) = (
                    world.bodies.get(c1.parent().unwrap()),
                    world.bodies.get(c2.parent().unwrap()),
                ) {
                    let id1 = PhysicsBodyId::from(Identity::from_u256(rb1.user_data.into()));
                    let id2 = PhysicsBodyId::from(Identity::from_u256(rb2.user_data.into()));
                    // Only track collisions involving at least one dynamic body
                    let dyn1 = ctx.db.physics_body().entity_id().find(id1.0)
                        .map(|r| r.body_type == super::DYNAMIC_BODY_TYPE).unwrap_or(false);
                    let dyn2 = ctx.db.physics_body().entity_id().find(id2.0)
                        .map(|r| r.body_type == super::DYNAMIC_BODY_TYPE).unwrap_or(false);
                    if !(dyn1 || dyn2) {
                        return;
                    }
                    // Create ordered ContactPair
                    let pair = ContactPair::new(id1, id2);
                    CONTACT_STARTS.lock().unwrap()
                        .entry((region, pair))
                        .or_insert(ctx.timestamp);
                }
            }
        }
        CollisionEvent::Stopped(h1, h2, _) => {
            if let (Some(c1), Some(c2)) = (
                world.colliders.get(*h1),
                world.colliders.get(*h2)
            ) {
                if let (Some(rb1), Some(rb2)) = (
                    world.bodies.get(c1.parent().unwrap()),
                    world.bodies.get(c2.parent().unwrap()),
                ) {
                    let id1 = PhysicsBodyId::from(Identity::from_u256(rb1.user_data.into()));
                    let id2 = PhysicsBodyId::from(Identity::from_u256(rb2.user_data.into()));
                    // Only finalize durations for dynamic contacts
                    let dyn1 = ctx.db.physics_body().entity_id().find(id1.0)
                        .map(|r| r.body_type == super::DYNAMIC_BODY_TYPE).unwrap_or(false);
                    let dyn2 = ctx.db.physics_body().entity_id().find(id2.0)
                        .map(|r| r.body_type == super::DYNAMIC_BODY_TYPE).unwrap_or(false);
                    if !(dyn1 || dyn2) {
                        return;
                    }
                    // Create ordered ContactPair
                    let pair = ContactPair::new(id1, id2);
                    if let Some(start_ts) = CONTACT_STARTS.lock().unwrap().remove(&(region, pair)) {
                        let end_ts = ctx.timestamp;
                        let duration = end_ts.to_micros_since_unix_epoch()
                            .saturating_sub(start_ts.to_micros_since_unix_epoch());
                        let id = CONTACT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
                        let rec = ContactDuration { id, entity_1: id1.0, entity_2: id2.0, region, started_at: start_ts, duration_micros: duration };
                        ctx.db.contact_duration().insert(rec);
                    }
                }
            }
        }
    }
}

/// Remove any contact entries involving the given entity
pub fn remove_entity_contacts(entity_id: &Identity) {
    let pid = PhysicsBodyId(*entity_id);
    CONTACT_STARTS.lock().unwrap().retain(|&(_, pair), _| pair.0 != pid && pair.1 != pid);
}

/// Get the number of active contact pairs being tracked
pub fn get_active_contact_count() -> usize {
    CONTACT_STARTS.lock().unwrap().len()
}
