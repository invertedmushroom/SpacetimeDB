use spacetimedb::{table, Identity};

/// View table representing any entity located in a specific chunk
#[derive(Clone)]
#[table(name = chunk_entities, public, index(name = idx_chunk, btree(columns = [chunk_x, chunk_y])))]
pub struct ChunkEntity {
    #[primary_key]
    pub entity_id: Identity,
    /// Type identifier for the entity (e.g., "player", "game_item", "physics_body")
    pub entity_type: String,
    /// World-space position of the entity
    pub pos_x: f32,
    pub pos_y: f32,
    /// Chunk coordinates for spatial partitioning
    pub chunk_x: i32,
    pub chunk_y: i32,
    /// Optional JSON payload or minimal data clients need
    pub data: Option<String>,
}
