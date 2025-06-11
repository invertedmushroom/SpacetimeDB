use rapier3d::prelude::*;
// Bounded channels for back-pressure - Using crossbeam::channel::bounded to limit event processing
use crossbeam::channel::bounded;
//use crossbeam::channel::unbounded;
use spacetimedb::reducer;
use spacetimedb::ReducerContext;
use crate::tables::scheduling::PhysicsTickSchedule;
use crate::physics::contact_tracker::{handle_event, process_contacts};
use crate::physics::{drain_collision_events, apply_database_updates};

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
    
    // lock and get or init context
    let mut map = PHYSICS_CONTEXTS.lock().unwrap();
    // construct a PhysicsContext for this region if it doesn't exist
    let world = map.entry(region)
                                        .or_default();
                           

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

    world.query_pipeline.update(&world.colliders);


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

    apply_database_updates(ctx, world);
    
    // Schedule the next tick (self-scheduling for continuous physics)
    if let Err(e) = crate::reducers::lifecycle::schedule_physics_tick(ctx, region, Some(schedule.scheduled_id)) {
        log::error!("Failed to schedule next physics tick: {}", e);
    }
    
    Ok(())
}