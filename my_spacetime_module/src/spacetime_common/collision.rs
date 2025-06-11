use rapier3d::geometry::InteractionGroups;

/// Body type constants
pub const STATIC_BODY_TYPE: u8 = 0;
pub const DYNAMIC_BODY_TYPE: u8 = 1;
pub const KINEMATIC_BODY_TYPE: u8 = 2;

/// Game-specific body type constants
pub const PROJECTILE_BODY_TYPE: u8 = 10;
pub const PLAYER_BODY_TYPE: u8 = 20;

/// Bitmask groups for your game (up to 32 distinct groups)
pub mod collision_group {
    pub const DEFAULT:    u32 = 1 << 0;
    pub const PLAYER:     u32 = 1 << 1;
    pub const ENEMY:      u32 = 1 << 2;
    pub const PROJECTILE: u32 = 1 << 3;
    pub const SENSOR:     u32 = 1 << 4;

    /// Which groups solid bodies collide with
    pub const SOLID_FILTER:  u32 = DEFAULT | PLAYER | ENEMY | PROJECTILE;
    /// Which groups sensors “see”
    pub const SENSOR_FILTER: u32 = SOLID_FILTER;
}

/// Build the two‐mask InteractionGroups for Rapier
#[inline]
pub fn interaction_groups(body_type: u8, is_sensor: bool) -> InteractionGroups {
    let membership = match body_type {
        PLAYER_BODY_TYPE     => collision_group::PLAYER,
        PROJECTILE_BODY_TYPE => collision_group::PROJECTILE,
        _                    => collision_group::DEFAULT,
    };
    let filter = if is_sensor {
        collision_group::SENSOR_FILTER
    } else {
        collision_group::SOLID_FILTER
    };
    InteractionGroups::new(membership.into(), filter.into())
}