use crate::physics::rapier_common::*;
use rapier3d::prelude::*;
// Bounded channels for back-pressure - Using crossbeam::channel::bounded to limit event processing
use crossbeam::channel::bounded;
//use crossbeam::channel::unbounded;
use spacetimedb::reducer;
use spacetimedb::ReducerContext;
use crate::tables::scheduling::PhysicsTickSchedule;
use crate::physics::contact_tracker::{handle_event, process_contacts};
use crate::physics::{drain_collision_events, apply_position_updates};

/// Maximum number of collision events to process per tick
pub const MAX_COLLISION_EVENTS: usize = 100;

pub use crate::physics::PHYSICS_CONTEXTS;
pub use crate::physics::PhysicsContext;
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
            id_to_body: HashMap::new(),
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
    // Process Start, Continue, and End contacts and handle events
    let contacts = process_contacts(&events, world, region);
    for contact in contacts {
        handle_event(ctx, world, contact);
    }

    //log::info!("Mock collision handling: {} collision events", events.len());
    
    // Schedule the next tick (self-scheduling for continuous physics)
    if let Err(e) = crate::reducers::lifecycle::schedule_physics_tick(ctx, region, Some(schedule.scheduled_id)) {
        log::error!("Failed to schedule next physics tick: {}", e);
    }
    
    Ok(())
}