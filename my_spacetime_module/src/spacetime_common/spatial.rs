
/// Spatial partitioning utilities (chunk math and IDs)

/// Size of one chunk in world units
pub const CHUNK_SIZE: f32 = 10.0;

/// Convert a continuous world position (f32) to a discrete chunk coordinate (i32)
pub fn calculate_chunk(world_pos: f32) -> i32 {
    (world_pos / CHUNK_SIZE).floor() as i32
}

/// Check if two chunks are the same or neighbors (Moore neighborhood)
pub fn are_chunks_adjacent(x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
    (x1 - x2).abs() <= 1 && (y1 - y2).abs() <= 1
}
