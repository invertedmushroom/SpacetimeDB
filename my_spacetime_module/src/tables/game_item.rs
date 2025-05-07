use spacetimedb::{Identity, Timestamp};

/**
 * GameItem entity representing collectible/usable items in the game world.
 * 
 * This table demonstrates:
 * 1. World items vs. inventory items (is_dropped flag)
 * 2. Ownership relationships (owner_id)
 * 3. Positional data for world items
 * 4. Chunk-based spatial partitioning for efficient queries
 */
#[spacetimedb::table(name = game_item, public, index(name = idx_chunk, btree(columns = [chunk_x, chunk_y])))]
#[derive(Clone)]
pub struct GameItem {
    #[primary_key]
    pub item_id: u64,
    pub owner_id: Identity,
    pub name: String,
    pub item_type: String,
    pub value: u32,
    pub position_x: Option<f32>,
    pub position_y: Option<f32>,
    // Chunk coordinates for spatial partitioning
    pub chunk_x: Option<i32>, 
    pub chunk_y: Option<i32>, 
    pub is_dropped: bool,
    pub created_at: Timestamp,
}