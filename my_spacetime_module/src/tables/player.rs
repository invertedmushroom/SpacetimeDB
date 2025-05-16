use spacetimedb::{SpacetimeType, Identity, Timestamp};
use serde::{Serialize, Deserialize};

/**
 * Player status enumeration representing different connection and gameplay states.
 * 
 * This enum leverages SpacetimeDB v1's support for enums as index keys,
 * enabling efficient queries like "find all online players".
 */
#[derive(SpacetimeType, Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum PlayerStatus {
    Online,   // Player is connected and active
    Offline,  // Player has disconnected
    Away,     // Player is connected but inactive
    Playing,  // Player is actively engaged in gameplay
}

/**
 * Player entity representing a user in the game world.
 * 
 * The Player table demonstrates:
 * 1. Using Identity as primary key to directly link table rows to connected clients
 * 2. Tracking player state (position, health, score)
 * 3. Connection status management with status enum and timestamps
 * 4. Chunk-based spatial partitioning for efficient queries
 */
#[derive(Clone)]
#[spacetimedb::table(name = player, index(name = idx_chunk, btree(columns = [chunk_x, chunk_y])))]
pub struct Player {
    #[primary_key]
    pub player_id: Identity,  // Maps directly to client's Identity
    #[unique]
    pub username: String,     // Human-readable identifier
    pub position_x: f32,      // Position coordinates in 2D space
    pub position_y: f32,
    pub chunk_x: i32,         // Chunk coordinates for spatial partitioning
    pub chunk_y: i32,
    pub health: u32,          // Game mechanics attributes
    pub score: u32,
    pub status: PlayerStatus, // Current connection/gameplay state
    pub last_active: Timestamp, // Last activity timestamp for timeout logic
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}