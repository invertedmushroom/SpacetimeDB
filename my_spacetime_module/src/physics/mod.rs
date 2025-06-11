use crate::spacetime_common::spatial::calculate_chunk_pair;
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
pub mod skills;


// Forward old calls to the new spawn.rs
pub use spawn::{spawn_rigid_body, despawn_rigid_body};
#[cfg(test)]
pub mod tests;


/// Physics world state for a region
pub struct PhysicsContext {
    pub pipeline: PhysicsPipeline,
    pub query_pipeline: QueryPipeline,
    /// Accumulated damage per entity raw_id for batched DB writes
    pub pending_damage: HashMap<u32, u32>,
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
    // Map raw 32-bit physics entity ID → RigidBodyHandle for O(1) forward lookup
    pub id_to_body: HashMap<u32, RigidBodyHandle>,

}

impl Default for PhysicsContext {
    fn default() -> Self {
        PhysicsContext {
            pipeline: PhysicsPipeline::new(),
            query_pipeline: QueryPipeline::new(),
            pending_damage: HashMap::new(),
            gravity: vector![0.0, -9.81, 0.0],
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
    }
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

fn apply_database_updates(ctx: &ReducerContext, world: &mut PhysicsContext) {
    // collect all changed physics_body rows in one batch
    let mut updates = Vec::with_capacity(world.bodies.len());

    for (handle, body) in world.bodies.iter() {
        // skip static/fixed bodies
        if body.is_fixed() {
            continue;
        }

        // current transform
        let pos = *body.translation();
        let rot = *body.rotation();

        // did the transform change since last tick?
        let transform_changed = world
            .last_transforms
            .get(&handle)
            .map_or(true, |(old_pos, old_rot)| *old_pos != pos || *old_rot != rot);

        // pull out accumulated damage (0 if none)
        let entity_id = unpack_id(body.user_data);
        let dmg = world.pending_damage.remove(&entity_id).unwrap_or(0);

        // if nothing changed (neither movement nor damage), skip
        if !transform_changed && dmg == 0 {
            continue;
        }

        // lookup the DB row by PhysicsBodyId
        if let Some(mut row) = ctx.db.physics_body().entity_id().find(entity_id) {
            // update position/rotation/chunk if moved
            if transform_changed {
                let (chunk_x, chunk_y) = calculate_chunk_pair(pos.x, pos.y);
                row.pos_x = pos.x;
                row.pos_y = pos.y;
                row.pos_z = pos.z;
                row.chunk_x = chunk_x;
                row.chunk_y = chunk_y;
                row.rot_x = rot.i;
                row.rot_y = rot.j;
                row.rot_z = rot.k;
                row.rot_w = rot.w;
                // record new transform
                world.last_transforms.insert(handle, (pos, rot));
            }

            // apply damage if any
            if dmg > 0 {
                row.health = row.health.saturating_sub(dmg);
            }

            updates.push(row);
        }
    }

    // write all changes in one go
    if !updates.is_empty() {
        for row in updates {
           ctx.db.physics_body().entity_id().update(row);
       }

    }

    // clear the pending damage map for the next tick
    world.pending_damage.clear();
}