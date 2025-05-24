use crate::physics::prelude::*;
use rapier3d::prelude::*;

use spacetimedb::{reducer, ReducerContext, Identity, Table};
use crate::tables::physics_body::physics_body;
use crate::physics::contact_tracker::register_option;
use crate::spacetime_common::shape::ColliderShape;

// Unique physics-entity ID counter
static PHYSICS_ENTITY_COUNTER: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(1));


#[reducer]
pub fn spawn_rigid_body(
    ctx: &ReducerContext,
    region: u32,
    x: f32,
    y: f32,
    z: f32,
    collider_shape: String,
    body_type: u8,
) -> Result<(), String> {
    // Validate body type
    if ![0, 1, 2, 10, 20].contains(&body_type) {
        return Err("Invalid body type".into());
    }

    // Generate a unique ID for this physics entity via atomic counter
    let phys_id_u64 = PHYSICS_ENTITY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let physics_entity_id = PhysicsBodyId::from(Identity::from_u256((phys_id_u64 as u128).into()));
    // Pack user data for the rigid body
    let object_function: u8 = 0; // Player on evrything for now since we use spawn_rigid_body at player creation
    // The tick count is not used in this context, but we need to provide a value
    let tick_count: u8 = 0;
    let packed_user_data = 
        (u128::from(body_type) << 120) |
        (u128::from(object_function)  << 112) |
        (u128::from(0u8)       << 111) |
        ((u128::from(phys_id_u64) & ((1u128 << 64) - 1)) << 8) |
        u128::from(tick_count);


    // Initialize or get the physics world for this region
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

    // Build and insert rigid body
    let rb_builder = match body_type {
        0 => RigidBodyBuilder::fixed(),
        1 => RigidBodyBuilder::dynamic(),
        2 => RigidBodyBuilder::kinematic_position_based(),
        10 => RigidBodyBuilder::dynamic()
            .ccd_enabled(true), // Enable CCD for projectiles
        20 => RigidBodyBuilder::dynamic(), // Player type
        _ => return Err("Invalid body type".into()),
    }
    .translation(vector![x, y, z])
    .user_data(packed_user_data);
    let body_handle = world.bodies.insert(rb_builder.build());

    // Parse and build collider from shape string
    let is_sensor = collider_shape.to_lowercase().contains("sensor");
    let groups = collision::get_interaction_groups_for_body_type(body_type, is_sensor);
    let shape = collider_shape
        .parse::<ColliderShape>()
        .map_err(|e| e.to_string())?;
    let col_builder = shape.to_rapier(is_sensor, groups);
    // Insert collider and tag it with the spawning skill ID (using ctx.sender temporarily)
    let collider_handle = world.colliders.insert_with_parent(col_builder.build(), body_handle, &mut world.bodies);
    // Map this collider handle back (here we use the player as default)
    register_option(collider_handle, ctx.sender);

    // Calculate chunk coordinates for spatial partitioning
    let chunk_x = calculate_chunk(x);
    let chunk_y = calculate_chunk(y);

    // Insert row into physics_body
    let phys = crate::tables::physics_body::PhysicsBody {
        entity_id: physics_entity_id.0,
        owner_id: ctx.sender,
        health: 100,
        region,
        pos_x: x,
        pos_y: y,
        pos_z: z,
        chunk_x,
        chunk_y,
        rot_x: 0.0,
        rot_y: 0.0,
        rot_z: 0.0,
        rot_w: 1.0,
        vel_x: 0.0,
        vel_y: 0.0,
        vel_z: 0.0,
        ang_vel_x: 0.0,
        ang_vel_y: 0.0,
        ang_vel_z: 0.0,
        collider_shape: collider_shape.clone(),
        body_type,
    };
    ctx.db.physics_body().insert(phys);

    log::info!("Physics object created: entity_id={}, shape={}, type={}", 
        physics_entity_id.0.to_hex().chars().collect::<String>(),
        collider_shape, body_type);
    Ok(())
}

#[reducer]
/// Remove a rigid body and its collider from the physics world and delete its DB entry
pub fn despawn_rigid_body(
    ctx: &ReducerContext,
    entity_id: Identity,
    region: u32,
) -> Result<(), String> {
    // Lock and get the physics context for this region
    let mut map = PHYSICS_CONTEXTS.lock().unwrap();
    if let Some(world) = map.get_mut(&region) {
        // Find the handle for the body to remove (drop the iterator before mutation)
        let pbid = PhysicsBodyId::from(entity_id);
        let target_ud = pbid.0.to_u256().as_u128();
        let handle_opt = world.bodies.iter()
            .find(|(_, b)| b.user_data == target_ud)
            .map(|(h, _)| h);
        if let Some(handle) = handle_opt {
            // Now safely remove the body and its attached colliders
            world.bodies.remove(
                handle,
                &mut world.islands,
                &mut world.colliders,
                &mut world.impulse_joints,
                &mut world.multibody_joints,
                true,
            );
        }
    }
    // Delete from the PhysicsBody table
    ctx.db.physics_body().entity_id().delete(entity_id);
    Ok(())
}