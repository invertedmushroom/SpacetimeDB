//! Common imports for physics modules
pub use crate::spacetime_common::types::PhysicsBodyId;
pub use crate::spacetime_common::spatial::calculate_chunk;
pub use crate::spacetime_common::collision;

pub use crate::physics::PhysicsContext;
pub use crate::physics::PHYSICS_CONTEXTS;

pub use once_cell::sync::Lazy;
pub use std::sync::Mutex;
pub use std::collections::HashMap;
pub use std::sync::atomic::{AtomicU64, Ordering};