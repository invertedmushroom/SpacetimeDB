use crate::physics::rapier_common::*;
use rapier3d::prelude::*;
use spacetimedb::{ReducerContext, Identity, Table, Timestamp};
use crate::tables::contact_event::ContactEvent;
use crate::tables::contact_event::contact_event;
use crate::tables::player_buffs::player_buffs;
use crate::tables::physics_body::physics_body;
use crate::physics::skills::{apply_damage, apply_buff};


pub use crate::physics::PHYSICS_CONTEXTS;
pub use crate::physics::PhysicsContext;

static CONTACT_EVENT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Map each Rapier collider handle to the originating player Identity
/// Not used
/// Current implementation uses the collider's user_data to store entity_id
static OWNER_OF_COLLIDER: Lazy<Mutex<HashMap<ColliderHandle, Identity>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// External call to tag a collider with some data
pub fn register_owner(handle: ColliderHandle, option_id: Identity) {
    OWNER_OF_COLLIDER.lock().unwrap().insert(handle, option_id);
}

/// Domain events extracted from raw Rapier collisions
#[derive(Clone, Debug)]
pub enum PhysicsContact {
    Start { source_handle: ColliderHandle, target_handle: ColliderHandle, unpacked_source_id: u32, unpacked_target_id: u32, object_function: u8},
    /// Ongoing contact per source-target pair (fired each tick)
    Continue { source_handle: ColliderHandle, target_handle: ColliderHandle, unpacked_source_id: u32, unpacked_target_id: u32, object_function: u8, tick_count: u8 },
    End   { source_handle: ColliderHandle, target_handle: ColliderHandle, unpacked_source_id: u32, unpacked_target_id: u32, object_function: u8 },
}

/// State for each active contact
#[derive(Default)]
struct ContactState {
    pub tick_count: u8,
    pub buff_id: Option<u64>, // remember applied aura buff row id
}
/// Track active contacts per skill-instance to source-target for sustained detection
static ACTIVE_CONTACTS: Lazy<Mutex<HashMap<(ColliderHandle, ColliderHandle, u32, u32, u8), ContactState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Collect and normalize raw Rapier events into PhysicsContact instances
#[allow(unused_variables)]
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
                    let source_collider = world.colliders.get(source_collider_handle).unwrap();
                    let target_collider = world.colliders.get(target_collider_handle).unwrap();
                    
                    // Unpack ID and body_type from collider user_data (u128)
                    
                    let data1 = source_collider.user_data;
                    let data2 = target_collider.user_data;

                    let unpacked_source_id   = unpack_id(data1);
                    let unpacked_target_id   = unpack_id(data2);
                    let object_function = get_object_function(data1);
                    contacts.push(PhysicsContact::Start { source_handle: source_collider_handle, target_handle: target_collider_handle, unpacked_source_id, unpacked_target_id, object_function });
                    
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

                    let (source_handle, target_handle) = if c1.is_sensor() {
                        (*h1, *h2)
                    } else {
                        (*h2, *h1)
                    };
                    let source_collider = world.colliders.get(source_handle).unwrap();
                    let target_collider = world.colliders.get(target_handle).unwrap();

                    // Unpack ID and body_type from collider user_data (u128)
                    let data1 = source_collider.user_data;
                    let data2 = target_collider.user_data;

                    let unpacked_source_id   = unpack_id(data1);
                    let unpacked_target_id   = unpack_id(data2);
                    let object_function = get_object_function(data1);

                    contacts.push(PhysicsContact::End { source_handle, target_handle, unpacked_source_id, unpacked_target_id, object_function });
                    
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
            PhysicsContact::Start { source_handle,target_handle, unpacked_source_id, unpacked_target_id, object_function } => {
                result.push(contact.clone());
                map.entry((*source_handle, *target_handle, *unpacked_source_id, *unpacked_target_id, *object_function)).or_insert(ContactState::default());
            }
            PhysicsContact::End { unpacked_source_id, unpacked_target_id, object_function, .. } => {
                result.push(contact.clone());
                let to_remove: Vec<_> = map.keys()
                    .filter(|(_, _, sid, tid, func)| *sid == *unpacked_source_id && *tid == *unpacked_target_id && *func == *object_function)
                    .cloned()
                    .collect();
                for key in to_remove {
                    map.remove(&key);
                }
            }
            _ => {}
        }
    }

    for ((source_handle, target_handle, sid, tid, object_function), state) in map.iter_mut() {
        state.tick_count = state.tick_count.saturating_add(1);
        result.push(PhysicsContact::Continue {
            source_handle: *source_handle,
            target_handle: *target_handle,
            unpacked_source_id: *sid,
            unpacked_target_id: *tid,
            object_function: *object_function,
            tick_count: state.tick_count,
        });
    }

    result
}

pub fn handle_event(ctx: &ReducerContext, world: &mut PhysicsContext, contact: PhysicsContact) {
    match contact {
        PhysicsContact::Start { source_handle, target_handle, unpacked_source_id, unpacked_target_id, object_function } => {
            if object_function == 2 {
                // apply aura buff and record its row ID
                if let Some(pb) = ctx.db.physics_body().entity_id().find(unpacked_target_id) {
                    let player = pb.owner_id;
                    let expires = Timestamp::from_micros_since_unix_epoch(i64::MAX);
                    let buff_id = apply_buff(ctx, player, object_function, 1.0, expires);
                    // store buff_id in active-contact state
                    let key = (source_handle, target_handle, unpacked_source_id, unpacked_target_id, object_function);
                    if let Some(state) = ACTIVE_CONTACTS.lock().unwrap().get_mut(&key) {
                        state.buff_id = Some(buff_id);
                    }
                }
            }
         
         let ce_id = CONTACT_EVENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
            let ev = ContactEvent { id: ce_id, entity_1: unpacked_source_id, entity_2: unpacked_target_id, started_at: ctx.timestamp };
            ctx.db.contact_event().insert(ev);
            log::debug!("Contact Start: src={}, tgt={}, func={}",
                unpacked_source_id, unpacked_target_id, object_function);
            // TODO: future logic: update tick_count or dispatch option-specific handlers
         },
         #[allow(unused_variables)]
         PhysicsContact::Continue { source_handle, target_handle, unpacked_source_id, unpacked_target_id, object_function, tick_count } => {
            if object_function == 1 {
                if tick_count % 5 == 0 {
                    log::debug!("5 ticks -> one hit");
                    // centralize damage: accumulate and emit event
                    apply_damage(ctx, object_function, unpacked_target_id, 1);

                    if let Some(collider) = world.colliders.get_mut(source_handle) {
                        // increment collider userData hit count
                        let data = collider.user_data;
                        let new_hits = get_hit_count(data).saturating_add(1);
                        collider.user_data = set_hit_count(data, new_hits);
                        if new_hits >= 30 {
                            // Hits go over 30
                            ACTIVE_CONTACTS.lock().unwrap()
                                //.remove(&(handle, option_id.clone(), source_id_u64, target_id_u64));
                                .retain(|(h, _, _, _, _), _| *h != source_handle);
                            log::debug!("Contact Continue: collider hit_count={} - removed all contact entries for handle {:?}", new_hits, source_handle);
                        }
                    }
                }
            }
         },
         PhysicsContact::End { source_handle, target_handle, unpacked_source_id, unpacked_target_id, object_function } => {
            if object_function == 2 {
                // delete the specific aura buff instance recorded earlier
                let key = (source_handle, target_handle, unpacked_source_id, unpacked_target_id, object_function);
                if let Some(state) = ACTIVE_CONTACTS.lock().unwrap().get(&key) {
                    if let Some(bid) = state.buff_id {
                        ctx.db.player_buffs().id().delete(bid);
                    }
                }
            }

             if let Some(row) = ctx.db.contact_event().iter()
                .find(|e| e.entity_1 == unpacked_source_id && e.entity_2 == unpacked_target_id)
             {
                 ctx.db.contact_event().id().delete(row.id);
                 log::debug!("Contact End: src={}, tgt={}", unpacked_source_id, unpacked_target_id);
             }
         }
     }
}