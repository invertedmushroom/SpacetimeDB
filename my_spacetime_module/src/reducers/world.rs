use spacetimedb::{Identity, ReducerContext, Table};
use crate::calculate_chunk;
use crate::tables::player::player;
use crate::tables::game_item::game_item;

/**
 * Player movement reducer.
 * 
 * Position-based game mechanics
 * 1. Validating game world boundaries
 * 2. Updating player position
 * 3. Activity tracking for idle detection
 */
#[spacetimedb::reducer]
pub fn move_player(ctx: &ReducerContext, new_x: f32, new_y: f32) -> Result<(), String> {
    let player_id = ctx.sender;
    
    if let Some(player) = ctx.db.player().iter().find(|p| p.player_id == player_id) {
        let mut player = player.clone();
        // You could add collision detection or boundary checks here
        if new_x < 0.0 || new_x > 1000.0 || new_y < 0.0 || new_y > 1000.0 {
            return Err("Position out of bounds".to_string());
        }
        
        player.position_x = new_x;
        player.position_y = new_y;
        player.chunk_x = calculate_chunk(new_x);
        player.chunk_y = calculate_chunk(new_y);
        player.last_active = ctx.timestamp;
        
        // Update player using primary key column
        ctx.db.player().player_id().update(player.clone());
        
        log::info!("Player {} moved to ({}, {})", player.username, new_x, new_y);
        Ok(())
    } else {
        Err("Player not found".to_string())
    }
}

/**
 * Item pickup reducer.
 * Proximity-based interaction
 * 1. Spatial gameplay through distance calculations
 * 2. State transition from world item to inventory item
 * 3. Multi-condition validation (ownership, availability, distance)
 */
#[spacetimedb::reducer]
pub fn pickup_item(ctx: &ReducerContext, item_id: u64) -> Result<(), String> {
    let player_id = ctx.sender;
    log::info!("Player {} is trying to pick up item {}", player_id, item_id);
    // Verify player exists
    if ctx.db.player().iter().find(|p| p.player_id == player_id).is_none() {
        return Err("Player not found".to_string());
    }
    
    // Find the item
    if let Some(item) = ctx.db.game_item().iter().find(|i| i.item_id == item_id) {
        let mut item = item.clone();
        // Check if item is available to pick up
        if !item.is_dropped {
            return Err("Item is not available for pickup".to_string());
        }
        // Quick chunk-based filter: only attempt pickup if player and item share the same chunk
        let player = ctx.db.player().iter().find(|p| p.player_id == player_id).unwrap();
        if item.chunk_x != Some(player.chunk_x) || item.chunk_y != Some(player.chunk_y) {
            return Err("Item is not in same chunk".to_string());
        }
        // Check if player is near the item (within 2 units)
        let player = ctx.db.player().iter().find(|p| p.player_id == player_id).unwrap();
        if let (Some(item_x), Some(item_y)) = (item.position_x, item.position_y) {
            let distance = ((player.position_x - item_x).powi(2) + 
                            (player.position_y - item_y).powi(2)).sqrt();
            
            if distance > 2.0 {
                return Err("Item is too far away".to_string());
            }
        } else {
            return Err("Item has no position".to_string());
        }
        
        // Update item ownership
        item.owner_id = player_id;
        item.is_dropped = false;
        item.position_x = None;
        item.position_y = None;
        item.chunk_x = None;
        item.chunk_y = None;
        
        // Update item using primary key column
        ctx.db.game_item().item_id().update(item.clone());
        
        log::info!("Player {} picked up item {}", player.username, item.name);
        Ok(())
    } else {
        Err("Item not found".to_string())
    }
}

/**
 * Item drop reducer.
 * 
 * Inventory management
 * 1. Ownership verification
 * 2. State transition from inventory to world item
 * 3. Position inheritance (item dropped at player's location)
 */
#[spacetimedb::reducer]
pub fn drop_item(ctx: &ReducerContext, item_id: u64) -> Result<(), String> {
    let player_id = ctx.sender;
    
    // Verify player exists
    let player = match ctx.db.player().iter().find(|p| p.player_id == player_id) {
        Some(p) => p,
        None => return Err("Player not found".to_string()),
    };
    
    // Find the item and verify ownership
    if let Some(item) = ctx.db.game_item().iter().find(|i| i.item_id == item_id) {
        let mut item = item.clone();
        if item.owner_id != player_id {
            return Err("You don't own this item".to_string());
        }
        
        // Update item to be dropped at player's position
        item.owner_id = Identity::default();
        item.is_dropped = true;
        item.position_x = Some(player.position_x);
        item.position_y = Some(player.position_y);
        item.chunk_x = Some(player.chunk_x);
        item.chunk_y = Some(player.chunk_y);
        
        // Update item using primary key column
        ctx.db.game_item().item_id().update(item.clone());
        
        log::info!("Player {} dropped item {}", player.username, item.name);
        Ok(())
    } else {
        Err("Item not found".to_string())
    }
}