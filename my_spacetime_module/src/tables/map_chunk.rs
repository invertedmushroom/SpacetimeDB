use spacetimedb::{table, Timestamp};

#[table(name = map_chunk)]
#[derive(Clone)]
pub struct MapChunk {
    #[primary_key]
    pub chunk_id: u64,
    #[index(btree)]
    pub chunk_x: i32,
    #[index(btree)]
    pub chunk_y: i32,
    pub terrain_type: String,
    pub is_generated: bool,
    pub last_updated: Timestamp,
}
