use rapier3d::geometry::InteractionGroups;

/// Body type constants
pub const STATIC_BODY_TYPE: u8 = 0;
pub const DYNAMIC_BODY_TYPE: u8 = 1;
pub const KINEMATIC_BODY_TYPE: u8 = 2;

/// Game-specific body type constants
pub const PROJECTILE_BODY_TYPE: u8 = 10;
pub const PLAYER_BODY_TYPE: u8 = 20;

/// Collision categories via bitmasks
enum CollisionCategory {
    Default,
    Player,
    Enemy,
    Projectile,
    Sensor,
}

impl CollisionCategory {
    pub fn mask(self) -> u32 {
        match self {
            CollisionCategory::Default => 0b0001,
            CollisionCategory::Player => 0b0010,
            CollisionCategory::Enemy => 0b0100,
            CollisionCategory::Projectile => 0b1000,
            CollisionCategory::Sensor => 0b0000,
        }
    }
}

/// Returns the interaction groups for a given body type
pub fn get_interaction_groups_for_body_type(body_type: u8, is_sensor: bool) -> InteractionGroups {
    let category = match body_type {
        PLAYER_BODY_TYPE => CollisionCategory::Player,
        PROJECTILE_BODY_TYPE => CollisionCategory::Projectile,
        _ => CollisionCategory::Default,
    };
    let membership = category.mask();
    let filter = if is_sensor {
        CollisionCategory::Sensor.mask()
    } else {
        CollisionCategory::Default.mask()
            | CollisionCategory::Player.mask()
            | CollisionCategory::Enemy.mask()
            | CollisionCategory::Projectile.mask()
    };
    InteractionGroups::new(membership.into(), filter.into())
}