use spacetimedb::{Identity, Table};
use crate::tables::player::{Player, PlayerStatus};
use crate::tables::game_item::GameItem;
use crate::calculate_chunk;

// Import the table access traits to get access to .player() and .game_item() methods
use crate::tables::player::player;
use crate::tables::game_item::game_item;

/// Schema management helper functions for working with the database
pub struct QueryUtils;

impl QueryUtils {
    /// Retrieve all players, sorted by player_id
    pub fn get_all_players(ctx: &spacetimedb::ReducerContext) -> Vec<Player> {
        ctx.db.player().iter().collect()
    }
    
    /// Find a player by their identity
    pub fn find_player(ctx: &spacetimedb::ReducerContext, player_id: Identity) -> Option<Player> {
        ctx.db.player().iter().find(|p| p.player_id == player_id)
    }
    
    /// Find a player by username
    pub fn find_player_by_username(ctx: &spacetimedb::ReducerContext, username: &str) -> Option<Player> {
        ctx.db.player().iter().find(|p| p.username == username)
    }
    
    /// Find all players within a specific chunk
    pub fn get_players_in_chunk(ctx: &spacetimedb::ReducerContext, chunk_x: i32, chunk_y: i32) -> Vec<Player> {
        ctx.db.player().iter()
            .filter(|p| p.chunk_x == chunk_x && p.chunk_y == chunk_y)
            .collect()
    }
    
    /// Get all nearby players within a specified distance (in game units)
    pub fn get_players_near_position(
        ctx: &spacetimedb::ReducerContext, 
        center_x: f32, 
        center_y: f32, 
        radius: f32
    ) -> Vec<Player> {
        ctx.db.player().iter()
            .filter(|p| {
                let dx = p.position_x - center_x;
                let dy = p.position_y - center_y;
                (dx * dx + dy * dy).sqrt() <= radius
            })
            .collect()
    }
    
    /// Get all players with a specific status
    pub fn get_players_by_status(ctx: &spacetimedb::ReducerContext, status: PlayerStatus) -> Vec<Player> {
        ctx.db.player().iter()
            .filter(|p| p.status == status)
            .collect()
    }
    
    /// Find a specific game item by ID
    pub fn find_game_item(ctx: &spacetimedb::ReducerContext, item_id: u64) -> Option<GameItem> {
        ctx.db.game_item().iter().find(|i| i.item_id == item_id)
    }
    
    /// Get all items owned by a specific player
    pub fn get_player_inventory(ctx: &spacetimedb::ReducerContext, player_id: Identity) -> Vec<GameItem> {
        ctx.db.game_item().iter()
            .filter(|i| i.owner_id == player_id && !i.is_dropped)
            .collect()
    }
    
    /// Get all items in a specific chunk
    pub fn get_items_in_chunk(ctx: &spacetimedb::ReducerContext, chunk_x: i32, chunk_y: i32) -> Vec<GameItem> {
        ctx.db.game_item().iter()
            .filter(|i| i.chunk_x == Some(chunk_x) && i.chunk_y == Some(chunk_y) && i.is_dropped)
            .collect()
    }
    
    /// Get all items near a specific position
    pub fn get_items_near_position(
        ctx: &spacetimedb::ReducerContext,
        center_x: f32, 
        center_y: f32, 
        radius: f32
    ) -> Vec<GameItem> {
        ctx.db.game_item().iter()
            .filter(|i| {
                match (i.position_x, i.position_y) {
                    (Some(x), Some(y)) if i.is_dropped => {
                        let dx = x - center_x;
                        let dy = y - center_y;
                        (dx * dx + dy * dy).sqrt() <= radius
                    },
                    _ => false
                }
            })
            .collect()
    }
    
    /// Create a new game item in the database
    pub fn create_game_item(
        ctx: &spacetimedb::ReducerContext,
        name: String,
        item_type: String,
        value: u32,
        position_x: Option<f32>,
        position_y: Option<f32>,
        owner_id: Identity
    ) -> GameItem {
        // Generate a unique item ID (simple incrementing for this example)
        let max_id = ctx.db.game_item().iter()
            .map(|i| i.item_id)
            .max()
            .unwrap_or(0);
        
        let item_id = max_id + 1;
        
        // Calculate chunk coordinates if position is provided
        let (chunk_x, chunk_y) = match (position_x, position_y) {
            (Some(x), Some(y)) => (Some(calculate_chunk(x)), Some(calculate_chunk(y))),
            _ => (None, None)
        };
        
        // Determine if dropped based on whether owner is default identity
        let is_dropped = owner_id == Identity::default();
        
        let item = GameItem {
            item_id,
            owner_id,
            name,
            item_type,
            value,
            position_x,
            position_y,
            chunk_x,
            chunk_y,
            is_dropped,
            created_at: ctx.timestamp,
        };
        
        // Insert the item into the database
        ctx.db.game_item().insert(item.clone());
        
        item
    }
}

/// SQL utility for common game queries
pub mod sql {
    /// Generate a SQL query for players in a specific chunk
    pub fn players_in_chunk(chunk_x: i32, chunk_y: i32) -> String {
        format!(
            "SELECT * FROM player WHERE chunk_x = {} AND chunk_y = {}", 
            chunk_x, chunk_y
        )
    }
    
    /// Generate a SQL query for players in a bounding box
    pub fn players_in_area(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> String {
        format!(
            "SELECT * FROM player WHERE \
             position_x BETWEEN {} AND {} AND \
             position_y BETWEEN {} AND {}",
            min_x, max_x, min_y, max_y
        )
    }
    
    /// Generate a SQL query for items in a specific chunk
    pub fn items_in_chunk(chunk_x: i32, chunk_y: i32) -> String {
        format!(
            "SELECT * FROM game_item WHERE \
             chunk_x = {} AND chunk_y = {} AND is_dropped = true",
            chunk_x, chunk_y
        )
    }
    
    /// Generate a SQL query for items in a bounding box
    pub fn items_in_area(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> String {
        format!(
            "SELECT * FROM game_item WHERE \
             position_x BETWEEN {} AND {} AND \
             position_y BETWEEN {} AND {} AND \
             is_dropped = true",
            min_x, max_x, min_y, max_y
        )
    }
    
    /// Generate a SQL query for player inventory
    pub fn player_inventory(player_id: &str) -> String {
        format!(
            "SELECT * FROM game_item WHERE \
             owner_id = '{}' AND is_dropped = false",
            player_id
        )
    }
    
    /// Generate a SQL query for players with a specific status
    pub fn players_by_status(status: &str) -> String {
        format!(
            "SELECT * FROM player WHERE status = '{}'",
            status
        )
    }
    
    /// Generate a SQL query for nearby players and items
    pub fn nearby_entities(chunk_x: i32, chunk_y: i32, radius: i32) -> Vec<String> {
        let min_x = chunk_x - radius;
        let max_x = chunk_x + radius;
        let min_y = chunk_y - radius;
        let max_y = chunk_y + radius;
        
        vec![
            format!(
                "SELECT * FROM player WHERE \
                 chunk_x BETWEEN {} AND {} AND \
                 chunk_y BETWEEN {} AND {}",
                min_x, max_x, min_y, max_y
            ),
            format!(
                "SELECT * FROM game_item WHERE \
                 (chunk_x IS NOT NULL AND chunk_y IS NOT NULL) AND \
                 (chunk_x BETWEEN {} AND {}) AND \
                 (chunk_y BETWEEN {} AND {}) AND \
                 is_dropped = true",
                min_x, max_x, min_y, max_y
            ),
        ]
    }
}