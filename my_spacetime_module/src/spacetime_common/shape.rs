use rapier3d::prelude::*;
use std::str::FromStr;
use thiserror::Error;

/// Supported collider shapes
pub enum ColliderShape {
    Sphere(f32),
    Cuboid(f32, f32, f32),
}

/// Errors during shape parsing
#[derive(Debug, Error)]
pub enum ShapeParseError {
    #[error("invalid shape format")] InvalidFormat,
    #[error("invalid float value")] ParseFloat(#[from] std::num::ParseFloatError),
}

impl FromStr for ColliderShape {
    type Err = ShapeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if let Some(inner) = s.strip_prefix("Sphere(") {
            if let Some(val) = inner.strip_suffix(")") {
                let r = val.parse::<f32>()?;
                return Ok(ColliderShape::Sphere(r));
            }
        }
        if let Some(inner) = s.strip_prefix("Box(") {
            if let Some(val) = inner.strip_suffix(")") {
                let parts: Vec<_> = val.split(',').map(str::trim).collect();
                if parts.len() == 3 {
                    let x = parts[0].parse()?;
                    let y = parts[1].parse()?;
                    let z = parts[2].parse()?;
                    return Ok(ColliderShape::Cuboid(x, y, z));
                }
            }
        }
        Err(ShapeParseError::InvalidFormat)
    }
}

impl ColliderShape {
    /// Build a Rapier ColliderBuilder from this shape
    pub fn to_rapier(&self, is_sensor: bool, groups: InteractionGroups) -> ColliderBuilder {
        match *self {
            ColliderShape::Sphere(r) => ColliderBuilder::ball(r)
                .sensor(is_sensor)
                .active_events(ActiveEvents::COLLISION_EVENTS)
                .collision_groups(groups),
            ColliderShape::Cuboid(x, y, z) => ColliderBuilder::cuboid(x / 2.0, y / 2.0, z / 2.0)
                .sensor(is_sensor)
                .active_events(ActiveEvents::COLLISION_EVENTS)
                .collision_groups(groups),
        }
    }
}