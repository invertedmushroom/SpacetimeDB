use crate::physics::rapier_common::*;
use rapier3d::prelude::*;
use spacetimedb::{ReducerContext, Identity, Table};
use crate::tables::contact_event::ContactEvent;
use crate::tables::contact_event::contact_event;

pub use crate::physics::PHYSICS_CONTEXTS;
pub use crate::physics::PhysicsContext;

static CONTACT_EVENT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Map each Rapier collider handle to the originating skill (or player) Identity
static OPTION_OF_COLLIDER: Lazy<Mutex<HashMap<ColliderHandle, Identity>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// External call to tag a collider with some data
pub fn register_option(handle: ColliderHandle, option_id: Identity) {
    OPTION_OF_COLLIDER.lock().unwrap().insert(handle, option_id);
}

/// New: domain events extracted from raw Rapier collisions
#[derive(Clone, Debug)]
pub enum PhysicsContact {
    Start { handle: ColliderHandle, option_id: Identity, source_id_u64: u32, target_id_u64: u32, region: u32, object_function: u8, flag: bool, source_hit_count: u8 },
    /// Ongoing contact per source-target pair (fired each tick)
    Continue { handle: ColliderHandle, option_id: Identity, source_id_u64: u32, target_id_u64: u32, region: u32, tick_count: u8 },
    End   { source_id_u64: u32, target_id_u64: u32, region: u32 },
}

/// State for each active contact (source-target): per-tick and hit counts
#[derive(Default)]
struct ContactState {
    pub tick_count: u8,
}
/// Track active contacts per skill-instance to source-target for sustained detection
static ACTIVE_CONTACTS: Lazy<Mutex<HashMap<(ColliderHandle, Identity, u32, u32), ContactState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Collect and normalize raw Rapier events into PhysicsContact instances
pub fn collect_events(
    events: &[CollisionEvent],
    world: &PhysicsContext,
    region: u32,
) -> Vec<PhysicsContact> {
    let mut contacts = Vec::new();
    for ev in events {
        match ev {
            CollisionEvent::Started(h1, h2, _) => {
                if let (Some(c1), Some(c2)) = (
                    world.colliders.get(*h1),
                    world.colliders.get(*h2)
                ) {
                    // Skip if neither is a sensor or if both are sensors.
                    // if (!c1.is_sensor() && !c2.is_sensor()) || (c1.is_sensor() && c2.is_sensor()) {
                    //     continue;
                    // }
                    // Check which body is the sensor
                    let (source_collider_handle, target_collider_handle) = if c1.is_sensor() {
                        (*h1, *h2)
                    } else {
                        (*h2, *h1)
                    };
                    let skill_collider = world.colliders.get(source_collider_handle).unwrap();
                    let target_collider = world.colliders.get(target_collider_handle).unwrap();
                    
                    // Unpack ID and body_type from Rapier user_data (u128)
                    
                    let data1 = skill_collider.user_data;
                    let data2 = target_collider.user_data;

                    let source_id_u64   = get_raw_id(data1);
                    let target_id_u64   = get_raw_id(data2);
                    let source_hit_count = get_hit_count(data1);
                    let object_function = get_object_function(data1);
                    let flag            = get_flag(data1);

                    // Lookup the tag for collider on either handle
                    let map = OPTION_OF_COLLIDER.lock().unwrap();
                    let option_id = map.get(&source_collider_handle)
                        .or_else(|| map.get(&target_collider_handle))
                        .cloned()
                        .unwrap_or_default();
                    contacts.push(PhysicsContact::Start { handle: source_collider_handle, option_id, source_id_u64, target_id_u64, region, object_function, flag, source_hit_count });
                    
                }
            }
            CollisionEvent::Stopped(h1, h2, _) => {
                if let (Some(c1), Some(c2)) = (
                    world.colliders.get(*h1),
                    world.colliders.get(*h2)
                ) {
                    // if (!c1.is_sensor() && !c2.is_sensor()) || (c1.is_sensor() && c2.is_sensor()) {
                    //     continue;
                    // }

                    let (skill_collider_handle, target_collider_handle) = if c1.is_sensor() {
                        (*h1, *h2)
                    } else {
                        (*h2, *h1)
                    };
                    let skill_collider = world.colliders.get(skill_collider_handle).unwrap();
                    let target_collider = world.colliders.get(target_collider_handle).unwrap();

                    // Get the IDs from the *rigid bodies* associated with these colliders
                    // as your PhysicsContact::End expects PhysicsBodyId (which comes from rb user_data).
                    if let (Some(rb_skill), Some(rb_target)) = (
                        world.bodies.get(skill_collider.parent().unwrap()),
                        world.bodies.get(target_collider.parent().unwrap())
                    ) {
                        // Extract IDs via helper
                        let source_id_u64 = get_raw_id(rb_skill.user_data);
                        let target_id_u64 = get_raw_id(rb_target.user_data);
                        contacts.push(PhysicsContact::End { source_id_u64, target_id_u64, region });
                    }
                }
            }

        }

    }
    contacts
}

/// Process raw collision events into Start, End, and per-tick Continue events
pub fn process_contacts(
    events: &[CollisionEvent],
    world: &PhysicsContext,
    region: u32,
) -> Vec<PhysicsContact> {
    let raw = collect_events(events, world, region);
    let mut result = Vec::new();
    let mut map = ACTIVE_CONTACTS.lock().unwrap();

    // Handle Start and End events
    for contact in raw.into_iter() {
        match &contact {
            PhysicsContact::Start { handle, option_id, source_id_u64, target_id_u64, .. } => {
                result.push(contact.clone());
                map.entry((*handle, option_id.clone(), *source_id_u64, *target_id_u64)).or_insert(ContactState::default());
            }
            PhysicsContact::End { source_id_u64, target_id_u64, .. } => {
                result.push(contact.clone());
                let to_remove: Vec<_> = map.keys()
                    .filter(|(_, _, sid, tid)| *sid == *source_id_u64 && *tid == *target_id_u64)
                    .cloned()
                    .collect();
                for key in to_remove {
                    map.remove(&key);
                }
            }
            _ => {}
        }
    }

    // // Purge any entries whose target bodies no longer exist (despawned)
    // {
    //     let mut stale = Vec::new();
    //     for ((opt, tid), _) in map.iter() {
    //         if !world.id_to_body.contains_key(tid) {
    //             stale.push((opt.clone(), *tid));
    //         }
    //     }
    //     for key in stale {
    //         map.remove(&key);
    //     }
    // }
    // Emit Continue events for all sustaining contacts per source-target
    for ((handle, opt, sid, tid), state) in map.iter_mut() {
        state.tick_count = state.tick_count.saturating_add(1);
        result.push(PhysicsContact::Continue {
            handle: *handle,
            option_id: opt.clone(),
            source_id_u64: *sid,
            target_id_u64: *tid,
            region,
            tick_count: state.tick_count,
        });
    }

    result
}

/// Handle a normalized PhysicsContact by creating or deleting ContactEvent rows, and mutate collider user_data
pub fn handle_event(ctx: &ReducerContext, world: &mut PhysicsContext, contact: PhysicsContact) {
    match contact {
        PhysicsContact::Start { handle: _handle, option_id, source_id_u64, target_id_u64, region, object_function, flag, source_hit_count } => {
            let ce_id = CONTACT_EVENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
            // Extract actual PhysicsBodyIds from unpacked user_data, this should be same as entity_id for the body in DB
            let source = source_id_u64.into_body_id();
            let target = target_id_u64.into_body_id();
            let ev = ContactEvent { id: ce_id, option_id, entity_1: source.into(), entity_2: target.into(), region, started_at: ctx.timestamp };
            ctx.db.contact_event().insert(ev);
            // Debug/logging for object_function, flag, tick_count
            log::debug!("Contact Start: opt={}, src={}, tgt={}, func={}, flag={}, hits dealt={}",
                option_id.to_hex(), source.0, target.0, object_function, flag, source_hit_count);
            // TODO: future logic: update tick_count or dispatch option-specific handlers
         },
         PhysicsContact::Continue { handle, option_id, source_id_u64, target_id_u64, tick_count, .. } => {
            if tick_count % 5 == 0 {
                log::debug!("5 ticks - > one hit");
                if let Some(collider) = world.colliders.get_mut(handle) {
                    // increment collider user_data hit count
                    let data = collider.user_data;
                    let new_hits = get_hit_count(data).saturating_add(1);
                    collider.user_data = set_hit_count(data, new_hits);
                    if new_hits >= 30 {
                        ACTIVE_CONTACTS.lock().unwrap()
                            .remove(&(handle, option_id.clone(), source_id_u64, target_id_u64));
                        log::debug!("Contact Continue: collider hit_count={} - removing contact", new_hits);
                    }
                }
            }
            // other user_data flags available on collider.user_data
         },
         PhysicsContact::End { source_id_u64, target_id_u64, region } => {
            // Convert raw IDs directly into PhysicsBodyId via helper trait
            let source = source_id_u64.into_body_id();
            let target = target_id_u64.into_body_id();
             if let Some(row) = ctx.db.contact_event().iter()
                .find(|e| e.entity_1 == source.into() && e.entity_2 == target.into() && e.region == region)
             {
                 ctx.db.contact_event().id().delete(row.id);
                 log::debug!("Contact End: src={}, tgt={}, region={}", source.0, target.0, region);
             }
         }
     }
}