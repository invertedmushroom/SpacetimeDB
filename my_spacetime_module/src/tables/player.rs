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
 * 2. Tracking player state (health, score)
 * 3. Connection status management with status enum and timestamps

 */
#[derive(Clone)]
#[spacetimedb::table(name = player, public)]
pub struct Player {
    #[primary_key]
    pub player_id: Identity,  // Maps directly to client's Identity
    #[unique]
    pub username: String,     // Human-readable identifier
    pub health: u32,          // Game mechanics attributes
    pub score: u32,
    pub status: PlayerStatus, // Current connection/gameplay state
    pub last_active: Timestamp, // Last activity timestamp for timeout logic
    pub phy_entity_id: Identity, // ID of the associated physics body (primary key) - not used anywhere
}