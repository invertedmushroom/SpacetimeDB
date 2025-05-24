use once_cell::sync::Lazy;
use rapier3d::prelude::*;
//use nalgebra::UnitQuaternion;
use rapier3d::na::UnitQuaternion;
use rapier3d::dynamics::IslandManager;
use rapier3d::geometry::BroadPhaseMultiSap;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

// Bounded channels for back-pressure - Using crossbeam::channel::bounded to limit event processing
use crossbeam::channel::{bounded, Receiver};
//use crossbeam::channel::unbounded;

use spacetimedb::{reducer, ReducerContext, Identity};
use crate::tables::physics_body::physics_body;
use crate::tables::scheduling::PhysicsTickSchedule;
use crate::reducers::combat::combat_melee;
use crate::physics::contact_tracker::{handle_event, collect_events};
use crate::spacetime_common::types::{PhysicsBodyId, ContactPair};
use crate::spacetime_common::spatial::calculate_chunk;

use crate::spacetime_common::types;

// Body type constants
pub const STATIC_BODY_TYPE: u8 = 0;
pub const DYNAMIC_BODY_TYPE: u8 = 1;
pub const KINEMATIC_BODY_TYPE: u8 = 2;

// Game-specific body type constants
pub const PROJECTILE_BODY_TYPE: u8 = 10;
pub const PLAYER_BODY_TYPE: u8 = 20;

pub mod contact_tracker;
pub mod spawn;
// Forward old calls to the new spawn.rs
pub use spawn::{spawn_rigid_body, despawn_rigid_body};
#[cfg(test)]
pub mod tests;


/// Physics world state for a region
pub struct PhysicsContext {
    pub pipeline: PhysicsPipeline,
    pub gravity: Vector<Real>,
    pub integration_parameters: IntegrationParameters,
    pub islands: IslandManager,
    pub broad_phase: BroadPhaseMultiSap,
    pub narrow_phase: NarrowPhase,
    pub bodies: RigidBodySet,
    pub colliders: ColliderSet,
    pub impulse_joints: ImpulseJointSet,
    pub multibody_joints: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    // Track last known transform to minimize DB updates per tick
    pub last_transforms: HashMap<RigidBodyHandle, (Vector<Real>, UnitQuaternion<Real>)>,
}

pub static PHYSICS_CONTEXTS: Lazy<Mutex<HashMap<u32, PhysicsContext>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Maximum number of collision events to process per tick
const MAX_COLLISION_EVENTS: usize = 100;

/// Toggle to enable batched collision processing (true) or immediate unbatched handling (false)
const BATCH_COLLISIONS: bool = true;

// Drain all collision events from a channel into a Vec
fn drain_collision_events(rx: &Receiver<rapier3d::geometry::CollisionEvent>) -> Vec<rapier3d::geometry::CollisionEvent> {
    let mut events = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        events.push(ev);
    }
    events
}

// Apply updated positions, rotations, velocities, and chunk coords back to the DB
fn apply_position_updates(ctx: &ReducerContext, world: &mut PhysicsContext) {
    // Only update rows when transform changed since last tick
    for (handle, body) in world.bodies.iter() {
        let pos = *body.translation();
        let rot = *body.rotation();
        // lookup last transform
        let entry = world.last_transforms.get(&handle);
        if entry.map_or(true, |(old_pos, old_rot)| *old_pos != pos || *old_rot != rot) {
            // transform changed: write back to DB
            let pbid = PhysicsBodyId::from(Identity::from_u256(body.user_data.into()));
            if let Some(mut row) = ctx.db.physics_body().entity_id().find(pbid) {
                row.pos_x = pos.x;
                row.pos_y = pos.y;
                row.pos_z = pos.z;
                // compute and assign new chunk coordinates in XY plane
                let new_chunk_x = calculate_chunk(pos.x);
                let new_chunk_y = calculate_chunk(pos.y);
                row.chunk_x = new_chunk_x;
                row.chunk_y = new_chunk_y;
                row.rot_x = rot.i;
                row.rot_y = rot.j;
                row.rot_z = rot.k;
                row.rot_w = rot.w;
                // velocities and angular velocities unchanged here or update if needed
                ctx.db.physics_body().entity_id().update(row);
                
                // print debug info for chunks
                log::info!("Called inside apply_position_updates {} moved to chunk ({}, {})", pbid.0.to_hex().chars().collect::<String>(), new_chunk_x, new_chunk_y);
                // record new transform
                world.last_transforms.insert(handle, (pos, rot));
            }
            log::info!("Updated physics body {} to position ({}, {}, {}) and rotation ({}, {}, {}, {}) via physics tick",
                pbid.0.to_hex().chars().collect::<String>(),
                pos.x, pos.y, pos.z,
                rot.i, rot.j, rot.k, rot.w);
        }
    }
}

/// Scheduled reducer for stepping physics each tick
#[reducer]
pub fn physics_tick(ctx: &ReducerContext, schedule: PhysicsTickSchedule) -> Result<(), String> {
    // Only allow scheduler to call
    if ctx.sender != ctx.identity() {
        return Err("Unauthorized".into());
    }
    
    let region = schedule.region;
    //log::debug!("Physics tick running for region {}", region);
    //log::debug!("Called at timestamp {}", ctx.timestamp);
    
    // lock and get or init context
    let mut map = PHYSICS_CONTEXTS.lock().unwrap();
    let world = map.entry(region).or_insert_with(|| {
        let gravity = vector![0.0, -9.81, 0.0];
        PhysicsContext {
            pipeline: PhysicsPipeline::new(),
            gravity,
            integration_parameters: IntegrationParameters::default(),
            islands: IslandManager::new(),
            broad_phase: BroadPhaseMultiSap::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            last_transforms: HashMap::new(),
        }
    });

    // Use bounded channels to prevent event overflow - will drop events if channel fills up
    let (collision_tx, collision_rx) = bounded(MAX_COLLISION_EVENTS);
    let (contact_tx, _) = bounded(MAX_COLLISION_EVENTS);
    let collector = ChannelEventCollector::new(collision_tx, contact_tx);

    // Step physics simulation with event handler
    world.pipeline.step(
        &world.gravity,
        &world.integration_parameters,
        &mut world.islands,
        &mut world.broad_phase,
        &mut world.narrow_phase,
        &mut world.bodies,
        &mut world.colliders,
        &mut world.impulse_joints,
        &mut world.multibody_joints,
        &mut world.ccd_solver,
        None,
        &(),
        &collector,
    );

    // Update positions, rotations, velocities, and chunk coords in DB
    apply_position_updates(ctx, world);

    // Drain collision events and warn if at capacity
    let events = drain_collision_events(&collision_rx);
    if events.len() == MAX_COLLISION_EVENTS {
        log::warn!("Reached maximum collision events ({}), some may have been dropped", MAX_COLLISION_EVENTS);
    }

    // Process contact-duration events
    // Normalize raw collision events into domain contacts and handle them
    let contacts = collect_events(&events, world, region);
    for contact in contacts {
        handle_event(ctx, contact);
    }

    // Process collision events: batched vs unbatched
    if BATCH_COLLISIONS {
        // Batched processing
        let mut projectile_hits: HashMap<PhysicsBodyId, Vec<PhysicsBodyId>> = HashMap::new();
        let mut sensor_triggers: HashSet<(PhysicsBodyId, PhysicsBodyId)> = HashSet::new();
        let mut generic_collisions: Vec<ContactPair> = Vec::new();

        // Process collected Rapier CollisionEvent from channel with batching
        for event in events.iter().filter_map(|e| {
            if let rapier3d::geometry::CollisionEvent::Started(h1, h2, _flags) = e {
                Some((*h1, *h2)) // Dereference ColliderHandles
            } else { None }
        }) {
            // Map handles back to entities
            let (b1, b2) = (world.colliders.get(event.0), world.colliders.get(event.1));
            if let (Some(col1), Some(col2)) = (b1, b2) {
                let (Some(rb1), Some(rb2)) = (
                    world.bodies.get(col1.parent().unwrap()),
                    world.bodies.get(col2.parent().unwrap()),
                ) else { continue; };
                
                let e1 = PhysicsBodyId::from(Identity::from_u256(rb1.user_data.into()));
                let e2 = PhysicsBodyId::from(Identity::from_u256(rb2.user_data.into()));
                
                // Check if either collider is a sensor
                let sensor = col1.is_sensor() || col2.is_sensor();
                
                if sensor {
                    // Handle sensor collision (e.g., pickup zone)
                    if col1.is_sensor() {
                        sensor_triggers.insert((e2, e1));
                    } else {
                        sensor_triggers.insert((e1, e2));
                    }
                } else {
                    // Look up body types
                    if let (Some(r1), Some(r2)) = (
                        ctx.db.physics_body().entity_id().find(e1.0),
                        ctx.db.physics_body().entity_id().find(e2.0),
                    ) {
                        match (r1.body_type, r2.body_type) {
                            // Batch projectile hits by target for optimized processing
                            (PROJECTILE_BODY_TYPE, PLAYER_BODY_TYPE) => {
                                projectile_hits.entry(e2).or_default().push(e1);
                            }
                            (PLAYER_BODY_TYPE, PROJECTILE_BODY_TYPE) => {
                                projectile_hits.entry(e1).or_default().push(e2);
                            }
                            // Store other collisions for generic processing
                            _ => {
                                generic_collisions.push(ContactPair::new(PhysicsBodyId(e1.0), PhysicsBodyId(e2.0)));
                            }
                        }
                    }
                }
            }
        }
        
        // Process batched projectile hits (applying damage once per player)
        for (player_id, projectiles) in projectile_hits {
            let total_damage = projectiles.len() as u32;
            if total_damage > 0 {
                combat_melee(ctx, player_id.0, total_damage)?;
                
                // Collect handles to remove to avoid borrow conflicts
                let mut remove_handles = Vec::new();
                for projectile_id in projectiles {
                    // Find handle without holding mutable borrow
                    if let Some((h, _)) = world.bodies.iter().find(|(_, b)| b.user_data == projectile_id.0.to_u256().as_u128()) {
                        remove_handles.push(h);
                    }
                    // delete row from PhysicsBody table
                    ctx.db.physics_body().entity_id().delete(projectile_id.0);
                }
                // Now remove bodies mutably
                for h in remove_handles {
                    world.bodies.remove(
                        h,
                        &mut world.islands,
                        &mut world.colliders,
                        &mut world.impulse_joints,
                        &mut world.multibody_joints,
                        true,
                    );
                }
            }
        }
        
        // Process sensor triggers (e.g., pickup zones)
        for (entity_id, sensor_id) in sensor_triggers {
            // For now just log sensor triggers
            log::info!("Entity {:?} triggered sensor {:?}", entity_id.0, sensor_id.0);
        }
        
        // Process remaining generic collisions
        for pair in generic_collisions {
            // Basic collision response for debugging
            log::debug!("Generic collision between {:?} and {:?}", pair.0, pair.1);
        }
    } else {
        // Immediate unbatched processing: dispatch each event as it arrives
        // First, collect projectile removals to avoid borrow conflicts
        let mut removals: Vec<PhysicsBodyId> = Vec::new();
        for e in events.iter() {
            if let rapier3d::geometry::CollisionEvent::Started(h1, h2, _flags) = e {
                // Map handles back to bodies and entities
                if let (Some(col1), Some(col2)) = (world.colliders.get(*h1), world.colliders.get(*h2)) {
                    let (Some(rb1), Some(rb2)) = (
                        world.bodies.get(col1.parent().unwrap()),
                        world.bodies.get(col2.parent().unwrap()),
                    ) else { continue; };
                    let e1 = PhysicsBodyId::from(Identity::from_u256(rb1.user_data.into()));
                    let e2 = PhysicsBodyId::from(Identity::from_u256(rb2.user_data.into()));
                    let sensor = col1.is_sensor() || col2.is_sensor();
                    if sensor {
                        // sensor logic
                        log::info!("Sensor trigger {:?} <-> {:?}", e1.0, e2.0);
                    } else if let (Some(r1), Some(r2)) = (
                        ctx.db.physics_body().entity_id().find(e1.0),
                        ctx.db.physics_body().entity_id().find(e2.0),
                    ) {
                        match (r1.body_type, r2.body_type) {
                            (PROJECTILE_BODY_TYPE, PLAYER_BODY_TYPE) => {
                                combat_melee(ctx, e2.0, 1)?;
                                // schedule removal of this projectile entity
                                removals.push(e1);
                            }
                            (PLAYER_BODY_TYPE, PROJECTILE_BODY_TYPE) => {
                                combat_melee(ctx, e1.0, 1)?;
                                removals.push(e2);
                            }
                            _ => {
                                // generic collision
                                log::debug!("Collision {:?} <-> {:?}", e1.0, e2.0);
                            }
                        }
                    }
                }
            }
        }
        // Now perform removals in a separate pass
        for ent in removals {
            // Isolate the immutable borrow so it ends before mutable removal
            let handle_opt;
            {
                let mut iter = world.bodies.iter();
                handle_opt = iter.find_map(|(h, b)| {
                    if b.user_data == ent.0.to_u256().as_u128() { Some(h) } else { None }
                });
            }
            if let Some(handle) = handle_opt {
                world.bodies.remove(
                    handle,
                    &mut world.islands,
                    &mut world.colliders,
                    &mut world.impulse_joints,
                    &mut world.multibody_joints,
                    true,
                );
            }
             // delete row from PhysicsBody table
             ctx.db.physics_body().entity_id().delete(ent.0);
        }
    }
    
    // Log collisions for debug purposes
    if !events.is_empty() {
        log::info!("Processed {} collision events", events.len());
    }

    // Schedule the next tick (self-scheduling for continuous physics)
    if let Err(e) = crate::reducers::lifecycle::schedule_physics_tick(ctx, region, Some(schedule.scheduled_id)) {
        log::error!("Failed to schedule next physics tick: {}", e);
    }
    
    Ok(())
}