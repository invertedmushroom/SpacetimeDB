use crate::physics::rapier_common::*;
use rapier3d::prelude::*;
use spacetimedb::{reducer, ReducerContext, Table};
use crate::tables::physics_body::physics_body;
use crate::physics::contact_tracker::register_owner;
use crate::spacetime_common::shape::ColliderShape;
use crate::spacetime_common::collision::*;

pub use crate::physics::PHYSICS_CONTEXTS;
pub use crate::physics::PhysicsContext;

// Unique physics-entity ID counter
static PHYSICS_ENTITY_COUNTER: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(1));

/// Build the Rapier RigidBodyBuilder for a given type & user_data
fn make_rb_builder(body_type: u8, x: f32, y: f32, z: f32, ud: u128) -> RigidBodyBuilder {
    let b = match body_type {
        STATIC_BODY_TYPE     => RigidBodyBuilder::fixed(),
        DYNAMIC_BODY_TYPE    => RigidBodyBuilder::dynamic(),
        KINEMATIC_BODY_TYPE  => RigidBodyBuilder::kinematic_position_based(),
        PROJECTILE_BODY_TYPE => RigidBodyBuilder::dynamic().ccd_enabled(true),
        PLAYER_BODY_TYPE     => RigidBodyBuilder::kinematic_position_based(),
        _ => unreachable!(),
    };
    b.translation(vector![x, y, z]).user_data(ud)
}

fn is_sensor_string(shape: &str) -> bool {
    shape.to_lowercase().contains("sensor")
}

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
    let entity_id = (PHYSICS_ENTITY_COUNTER.fetch_add(1, Ordering::Relaxed)) as u32;
    // Pack user data for the rigid body
    let object_function: u8 = 0; // Player on evrything for now since we use spawn_rigid_body at player creation
    let tick_count: u8 = 0; // The tick count is not used
    let flag: bool = false; // No special flags for now
    let data = UserData {
        body_type,
        object_function,
        flag,
        raw_id: entity_id,
        modifier: 0, // No modifier for now
        hit_count: 0, // No hits yet
        block: false, // Not a block
        tick_count,
    };
    let packed_user_data = UserData::pack(data);

    // Initialize or get the physics world for this region
    let mut map = PHYSICS_CONTEXTS.lock().unwrap();
    let world = map.entry(region)
                                        .or_default();

    let rb = make_rb_builder(body_type, x, y, z, packed_user_data).build();
    // Build and insert rigid body
    let body_handle = world.bodies.insert(rb);
    // Track handle for O(1) forward lookup
    world.id_to_body.insert(entity_id, body_handle);
    
    // Parse and build collider from shape string
    let sensor = is_sensor_string(&collider_shape);
    let groups = interaction_groups(body_type, sensor);
    let shape = collider_shape
        .parse::<ColliderShape>()
        .map_err(|e| e.to_string())?;
    // Build collider and pack user_data
    let col = shape.to_rapier(sensor, groups)
        .user_data(packed_user_data)
        .build();
    // Insert collider into the physics world
    let col_handle = world.colliders.insert_with_parent(col, body_handle, &mut world.bodies);
    // tag the collider with client Identity for ownership tracking
    register_owner(col_handle, ctx.sender);

    // Calculate chunk coordinates for spatial partitioning
    let (chunk_x, chunk_y) = calculate_chunk_pair(x, y);

    // Insert row into physics_body
    let phys = crate::tables::physics_body::PhysicsBody {
        entity_id: entity_id,
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
        entity_id,
        collider_shape, body_type);
    Ok(())
}

#[reducer]
/// Remove a rigid body and its collider from the physics world and delete its DB entry
pub fn despawn_rigid_body(
    ctx: &ReducerContext,
    entity_id: u32,
    region: u32,
) -> Result<(), String> {
    // Lock and get the physics context for this region
    let mut map = PHYSICS_CONTEXTS.lock().unwrap();
    if let Some(world) = map.get_mut(&region) {
        // O(1) lookup via id_to_body map
        if let Some(&handle) = world.id_to_body.get(&entity_id) {
            // Safely remove the body and attached colliders
            world.bodies.remove(
                handle,
                &mut world.islands,
                &mut world.colliders,
                &mut world.impulse_joints,
                &mut world.multibody_joints,
                true,
            );
            // Remove forward lookup entry
            world.id_to_body.remove(&entity_id);
        }
    }
    // Delete from the PhysicsBody table
    ctx.db.physics_body().entity_id().delete(entity_id);
    Ok(())
}