/// Size of a world chunk in game units
pub const CHUNK_SIZE: f32 = 20.0;

/// Calculate chunk coordinates from world position
pub fn calculate_chunk(position: f32) -> i32 {
    (position / CHUNK_SIZE).floor() as i32
}