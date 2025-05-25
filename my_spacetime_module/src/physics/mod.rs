use crate::physics::rapier_common::*;
use rapier3d::prelude::*;
//use nalgebra::UnitQuaternion;
use rapier3d::na::UnitQuaternion;
use crossbeam::channel::Receiver;
use spacetimedb::ReducerContext;
use crate::tables::physics_body::physics_body;

pub mod contact_tracker;
pub mod spawn;
pub mod physics_tick;
pub mod rapier_common;

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
            let raw_id = get_raw_id(body.user_data);
            let pbid = raw_id.into_body_id();
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