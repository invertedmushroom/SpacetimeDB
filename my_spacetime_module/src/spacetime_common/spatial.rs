use wide::{f32x4, i32x4};
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

/// Calculate both X and Y chunk coords at once using SIMD (lanes 0 & 1)
/// Under the hood this divides & floors in parallel then extracts lanes 0/1.
pub fn calculate_chunk_pair(x: f32, y: f32) -> (i32, i32) {
    let inv = f32x4::splat(1.0 / CHUNK_SIZE);
    let v = f32x4::new([x, y, 0.0, 0.0]) * inv;
    let fv = v.floor();
    let arr = fv.to_array();
    (arr[0] as i32, arr[1] as i32)
}

/// Check adjacency for a single pair using SIMD batching
pub fn are_chunks_adjacent_simd(x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
    let v1 = i32x4::new([x1, y1, 0, 0]);
    let v2 = i32x4::new([x2, y2, 0, 0]);
    let diff = (v1 - v2).abs();
    let arr: [i32; 4] = diff.to_array();
    arr[0] <= 1 && arr[1] <= 1
}
