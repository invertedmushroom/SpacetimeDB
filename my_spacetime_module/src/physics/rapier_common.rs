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
        (self.to_u256().as_u128() & ((1u128 << 64) - 1)) as u64
    }
}
pub trait IdentityRawExt32 {
    fn to_raw_u32(&self) -> u32;
}
impl IdentityRawExt32 for Identity {
    fn to_raw_u32(&self) -> u32 {
        (self.to_u256().as_u128() & ((1u128 << 32) - 1)) as u32
    }
}

/// Trait to convert a raw u32 into a PhysicsBodyId
pub trait RawToBodyId {
    fn into_body_id(self) -> PhysicsBodyId;
}
impl RawToBodyId for u32 {
    fn into_body_id(self) -> PhysicsBodyId {
        PhysicsBodyId::from(Identity::from_u256((self as u128).into()))
    }
}

// Bit shifts for user_data packing (layout from LSB upward):
// [ 0..7]    tick_count (8 bits)
// [8]        block (1 bit)
// [9..16]    modifier (8 bits)
// [17..24]   hit_count (8 bits)
// [25..56]   raw_id (32 bits)
// [57]      flag (1 bit)
// [58..65]   object_function (8 bits)
// [66..73]   body_type (8 bits)
pub const TICK_COUNT_SHIFT: u32 = 0;
pub const BLOCK_SHIFT: u32 = 8;
pub const MODIFIER_SHIFT: u32 = 9;
pub const HIT_COUNT_SHIFT: u32 = 17;
pub const RAW_ID_SHIFT: u32 = 25;
pub const FLAG_SHIFT: u32 = 57;
pub const OBJECT_FUNCTION_SHIFT: u32 = 58;
pub const BODY_TYPE_SHIFT: u32 = 66;

/// Complete Rapier user_data payload for a physics body
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserData {
    pub body_type: u8,
    pub object_function: u8,
    pub flag: bool,
    pub raw_id: u32,      // changed from u64 to u32
    pub hit_count: u8,    // new field
    pub modifier: u8,     // new field
    pub block: bool,      // new field
    pub tick_count: u8,
}

impl UserData {
    /// Pack all fields into a single u128 value (const fn)
    #[inline(always)]
    pub const fn pack(self) -> u128 {
        ((self.body_type as u128) << BODY_TYPE_SHIFT)
        | ((self.object_function as u128) << OBJECT_FUNCTION_SHIFT)
        | ((self.flag as u8 as u128) << FLAG_SHIFT)
        | (((self.raw_id as u128) & ((1u128 << 32) - 1)) << RAW_ID_SHIFT)
        | ((self.hit_count as u128) << HIT_COUNT_SHIFT)
        | ((self.modifier as u128) << MODIFIER_SHIFT)
        | ((self.block as u8 as u128) << BLOCK_SHIFT)
        | ((self.tick_count as u128) << TICK_COUNT_SHIFT)
    }

    /// Unpack a u128 payload into its constituent fields
    #[inline(always)]
    pub fn unpack(data: u128) -> Self {
        let tick_count = (data >> TICK_COUNT_SHIFT) as u8;
        let block = ((data >> BLOCK_SHIFT) & 0x1) != 0;
        let modifier = ((data >> MODIFIER_SHIFT) & 0xFF) as u8;
        let hit_count = ((data >> HIT_COUNT_SHIFT) & 0xFF) as u8;
        let raw_id = ((data >> RAW_ID_SHIFT) & ((1u128 << 32) - 1)) as u32;
        let flag = ((data >> FLAG_SHIFT) & 0x1) != 0;
        let object_function = ((data >> OBJECT_FUNCTION_SHIFT) & 0xFF) as u8;
        let body_type = (data >> BODY_TYPE_SHIFT) as u8;
        Self { body_type, object_function, flag, raw_id, hit_count, modifier, block, tick_count }
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
/// Set the flag in an existing packed user_data value
#[inline]
pub fn set_flag(data: u128, flag: bool) -> u128 {
    (data & !(1u128 << FLAG_SHIFT)) | ((flag as u8 as u128) << FLAG_SHIFT)
}
/// Extract raw physics entity ID (32 bits) from packed user_data
#[inline]
pub fn get_raw_id(data: u128) -> u32 {
    ((data >> RAW_ID_SHIFT) & ((1u128 << 32) - 1)) as u32
}
/// Pack a 32-bit physics entity ID into Rapier user_data
pub fn pack_raw_id(raw_id: u32) -> u128 {
    (u128::from(raw_id) & ((1u128 << 32) - 1)) << RAW_ID_SHIFT
}
/// Extract hit_count (8 bits) from packed user_data
#[inline]
pub fn get_hit_count(data: u128) -> u8 {
    ((data >> HIT_COUNT_SHIFT) & 0xFF) as u8
}
/// Extract modifier (8 bits) from packed user_data
#[inline]
pub fn get_modifier(data: u128) -> u8 {
    ((data >> MODIFIER_SHIFT) & 0xFF) as u8
}
/// Extract block (single bit) from packed user_data
#[inline]
pub fn get_block(data: u128) -> bool {
    ((data >> BLOCK_SHIFT) & 0x1) != 0
}
/// Update the block value (a single bit) in an existing packed user_data value
#[inline]
pub fn set_block(data: u128, block: bool) -> u128 {
    (data & !(1u128 << BLOCK_SHIFT)) | (((block as u8) as u128) << BLOCK_SHIFT)
}
/// Extract tick_count (lowest 8 bits) from packed user_data
#[inline]
pub fn get_tick_count(data: u128) -> u8 {
    (data & 0xFF) as u8
}
/// Update the tick_count in an existing packed user_data value
#[inline]
pub fn set_tick_count(data: u128, tick: u8) -> u128 {
    (data & !0xFF) | (tick as u128)
}
/// Update hit_count (8 bits) in an existing packed user_data value
#[inline]
pub fn set_hit_count(data: u128, hit_count: u8) -> u128 {
    (data & !(((1u128 << 8) - 1) << HIT_COUNT_SHIFT)) | ((hit_count as u128) << HIT_COUNT_SHIFT)
}

