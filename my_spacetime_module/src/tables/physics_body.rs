use spacetimedb::Identity;

#[spacetimedb::table(
    name = physics_body,
    index(name = idx_owner, btree(columns = [owner_id]))
)]
#[derive(Clone)]
pub struct PhysicsBody {
    #[primary_key]
    pub entity_id: Identity,
    pub owner_id: Identity,
    #[index(btree)]  // index on region for fast region-based queries
    pub region: u32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    #[index(btree)]  // index on chunk_x for spatial filtering
    pub chunk_x: i32,
    #[index(btree)]  // index on chunk_y for spatial filtering
    pub chunk_y: i32,
    // Added rotation quaternion
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
    pub rot_w: f32,
    // Added linear velocity
    pub vel_x: f32,
    pub vel_y: f32,
    pub vel_z: f32,
    // Added angular velocity
    pub ang_vel_x: f32,
    pub ang_vel_y: f32,
    pub ang_vel_z: f32,
    // Collider descriptor and body type
    pub collider_shape: String,
    pub body_type: u8,
}