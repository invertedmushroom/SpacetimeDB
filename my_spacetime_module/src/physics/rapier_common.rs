//! Common imports for physics modules
pub use crate::spacetime_common::types::PhysicsBodyId;
pub use crate::spacetime_common::spatial::calculate_chunk;
pub use crate::spacetime_common::collision;
pub use crate::spacetime_common::types::*;

pub use once_cell::sync::Lazy;
pub use std::sync::Mutex;
pub use std::collections::HashMap;
pub use std::sync::atomic::{AtomicU64, Ordering};
pub use spacetimedb::Identity;

/// Convert an Identity into a u128 for packing
pub trait IdentityU128 {
    fn as_u128(&self) -> u128;
}
impl IdentityU128 for Identity {
    fn as_u128(&self) -> u128 {
        self.to_u256().as_u128()
    }
}

/// Extensions on PhysicsBodyId for raw access
pub trait PhysicsBodyIdExt {
    fn raw_u64(&self) -> u64;
}
impl PhysicsBodyIdExt for PhysicsBodyId {
    fn raw_u64(&self) -> u64 {
        // The lower 64 bits of the identity
        self.0.to_u256().as_u128() as u64
    }
}

/// Extension trait to extract lower 64 bits from an `Identity`.
pub trait IdentityRawExt {
    fn to_raw_u64(&self) -> u64;
}
impl IdentityRawExt for Identity {
    fn to_raw_u64(&self) -> u64 {
        // Take the lower 64 bits of the Identity's u128 representation
        (self.to_u256().as_u128() & ((1u128 << 64) - 1)) as u64
    }
}

/// Trait to convert a raw u64 into a PhysicsBodyId
pub trait RawToBodyId {
    fn into_body_id(self) -> PhysicsBodyId;
}
impl RawToBodyId for u64 {
    fn into_body_id(self) -> PhysicsBodyId {
        PhysicsBodyId::from(Identity::from_u256((self as u128).into()))
    }
}

// Bit shifts for user_data packing
pub const BODY_TYPE_SHIFT: u32 = 120;
pub const OBJECT_FUNCTION_SHIFT: u32 = 112;
pub const FLAG_SHIFT: u32 = 111;
pub const RAW_ID_SHIFT: u32 = 8;
pub const TICK_COUNT_SHIFT: u32 = 0;

/// Complete Rapier user_data payload for a physics body
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserData {
    pub body_type: u8,
    pub object_function: u8,
    pub flag: bool,
    pub raw_id: u64,
    pub tick_count: u8,
}

impl UserData {
    /// Pack all fields into a single u128 value (const fn)
    #[inline(always)]
    pub const fn pack(self) -> u128 {
        ((self.body_type as u128) << BODY_TYPE_SHIFT)
        | ((self.object_function as u128) << OBJECT_FUNCTION_SHIFT)
        | ((self.flag as u8 as u128) << FLAG_SHIFT)
        | (((self.raw_id as u128) & ((1u128 << 64) - 1)) << RAW_ID_SHIFT)
        | ((self.tick_count as u128) << TICK_COUNT_SHIFT)
    }

    /// Unpack a u128 payload into its constituent fields
    #[inline(always)]
    pub fn unpack(data: u128) -> Self {
        let body_type = (data >> BODY_TYPE_SHIFT) as u8;
        let object_function = ((data >> OBJECT_FUNCTION_SHIFT) & 0xFF) as u8;
        let flag = ((data >> FLAG_SHIFT) & 0x1) != 0;
        let raw_id = ((data >> RAW_ID_SHIFT) & ((1u128 << 64) - 1)) as u64;
        let tick_count = (data >> TICK_COUNT_SHIFT) as u8;
        Self { body_type, object_function, flag, raw_id, tick_count }
    }
}

/// Extract body_type (top 8 bits) from packed user_data
#[inline]
pub fn get_body_type(data: u128) -> u8 {
    (data >> BODY_TYPE_SHIFT) as u8
}

/// Extract object_function (next 8 bits) from packed user_data
#[inline]
pub fn get_object_function(data: u128) -> u8 {
    ((data >> OBJECT_FUNCTION_SHIFT) & 0xFF) as u8
}

/// Extract flag (single bit) from packed user_data
#[inline]
pub fn get_flag(data: u128) -> bool {
    ((data >> FLAG_SHIFT) & 0x1) != 0
}

// Set the flag in an existing packed user_data value
#[inline]
pub fn set_flag(data: u128, flag: bool) -> u128 {
    // clear the flag bit, then or in the new value
    (data & !(1u128 << FLAG_SHIFT)) | ((flag as u8 as u128) << FLAG_SHIFT)
}

/// Extract raw physics entity ID (64 bits) from packed user_data
#[inline]
pub fn get_raw_id(data: u128) -> u64 {
    ((data >> RAW_ID_SHIFT) & ((1u128 << 64) - 1)) as u64
}

/// Pack a 64-bit physics entity ID into Rapier user_data
pub fn pack_raw_id(raw_id: u64) -> u128 {
    (u128::from(raw_id) & ((1u128 << 64) - 1)) << 8
}

/// Extract tick_count (lowest 8 bits) from packed user_data
#[inline]
pub fn get_tick_count(data: u128) -> u8 {
    (data & 0xFF) as u8
}

/// Update the tick_count in an existing packed user_data value
#[inline]
pub fn set_tick_count(data: u128, tick: u8) -> u128 {
    // clear low 8 bits, then or in new tick
    (data & !0xFF) | (tick as u128)
}