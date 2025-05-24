use std::sync::atomic::{AtomicU64, Ordering};
use rapier3d::geometry::CollisionEvent;
use spacetimedb::{ReducerContext, Identity, Table};
use crate::physics::PhysicsContext;
use crate::tables::contact_event::ContactEvent;
use crate::tables::contact_event::contact_event;
use crate::physics::types::PhysicsBodyId;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::collections::HashMap;
use rapier3d::geometry::ColliderHandle;

static CONTACT_EVENT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Map each Rapier collider handle to the originating skill (or player) Identity
static OPTION_OF_COLLIDER: Lazy<Mutex<HashMap<ColliderHandle, Identity>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// External call to tag a collider with some data
pub fn register_option(handle: ColliderHandle, option_id: Identity) {
    OPTION_OF_COLLIDER.lock().unwrap().insert(handle, option_id);
}

/// New: domain events extracted from raw Rapier collisions
pub enum PhysicsContact {
    Start { option_id: Identity, source_id_u64: u64, target_id_u64: u64, region: u32, object_function: u8, flag: bool, tick_count: u8 },
    End   { source_id_u64: u64, target_id_u64: u64, region: u32 },
}

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
                    if (!c1.is_sensor() && !c2.is_sensor()) || (c1.is_sensor() && c2.is_sensor()) {
                        continue;
                    }
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
                    //let skill_body_type1 = (data1 >> 120) as u8; //static, dynamic, kinematic, player, projectile

                    let data2 = target_collider.user_data;
                    //let target_body_type2 = (data2 >> 120) as u8;
                    // Only track if either is dynamic
                    // if body_type1 != DYNAMIC_BODY_TYPE && body_type2 != DYNAMIC_BODY_TYPE {
                    //     continue;
                    // }
                    let source_id_u64 = ((data1 >> 8) & ((1 << 64) - 1)) as u64;
                    let target_id_u64 = ((data2 >> 8) & ((1 << 64) - 1)) as u64;

                    let object_function = ((data1 >> 112) & 0xFF) as u8;
                    let flag = ((data1 >> 111) & 0x1) != 0;
                    let tick_count = (data1 & 0xFF) as u8;

                    // let object_function = ((data1 >> 112) & 0xFF) as u8;
                    // let flag = ((data1 >> 111) & 0x1) != 0;
                    // let tick_count = (data1 & 0xFF) as u8;

                    // let object_function2 = ((data2 >> 112) & 0xFF) as u8;
                    // let flag2 = ((data2 >> 111) & 0x1) != 0;
                    // let tick_count2 = (data2 & 0xFF) as u8;

                    // Lookup the tag for collider on either handle
                    let map = OPTION_OF_COLLIDER.lock().unwrap();
                    let option_id = map.get(&source_collider_handle)
                        .or_else(|| map.get(&target_collider_handle))
                        .cloned()
                        .unwrap_or_default();
                    contacts.push(PhysicsContact::Start { option_id, source_id_u64, target_id_u64, region, object_function, flag, tick_count});
                    
                }
            }
            CollisionEvent::Stopped(h1, h2, _) => {
                if let (Some(c1), Some(c2)) = (
                    world.colliders.get(*h1),
                    world.colliders.get(*h2)
                ) {
                    if (!c1.is_sensor() && !c2.is_sensor()) || (c1.is_sensor() && c2.is_sensor()) {
                        continue;
                    }

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
                        // // Ensure PhysicsBodyId unpacking matches how you pack it into rb.user_data
                        // let source_body_id = PhysicsBodyId::from(Identity::from_u256((rb_skill.user_data & ((1<<120)-1)).into()));
                        // let target_body_id = PhysicsBodyId::from(Identity::from_u256((rb_target.user_data & ((1<<120)-1)).into()));
                        let source_id_u64= ((rb_skill.user_data >> 8) & ((1 << 64) - 1)) as u64;
                        let target_id_u64= ((rb_target.user_data >> 8) & ((1 << 64) - 1)) as u64;

                        contacts.push(PhysicsContact::End { source_id_u64, target_id_u64, region });
                    }
                }
            }

        }

    }
    contacts
}

/// Handle a normalized PhysicsContact by creating or deleting ContactEvent rows
pub fn handle_event(ctx: &ReducerContext, contact: PhysicsContact) {
    match contact {
        #[allow(unused_variables)]
        PhysicsContact::Start { option_id, source_id_u64, target_id_u64, region, object_function, flag, tick_count} => {
            let ce_id = CONTACT_EVENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
            // Extract actual PhysicsBodyIds from unpacked user_data, this should be same as entity_id for the body in DB
            let source = PhysicsBodyId::from(Identity::from_u256(source_id_u64.into()));
            let target = PhysicsBodyId::from(Identity::from_u256(target_id_u64.into()));
            let ev = ContactEvent { id: ce_id, option_id, entity_1: source.0, entity_2: target.0, region, started_at: ctx.timestamp };
            ctx.db.contact_event().insert(ev);
        }
        PhysicsContact::End { source_id_u64, target_id_u64, region } => {
            // // Ensure PhysicsBodyId unpacking matches how you pack it into rb.user_data so we can find it in DB
            let source_body_id = PhysicsBodyId::from(Identity::from_u256(source_id_u64.into()));
            let target_body_id = PhysicsBodyId::from(Identity::from_u256(target_id_u64.into()));
            if let Some(row) = ctx.db.contact_event().iter()
                .find(|e| e.entity_1 == source_body_id.0 && e.entity_2 == target_body_id.0 && e.region == region)
            {
                ctx.db.contact_event().id().delete(row.id);
            }
        }
    }
}