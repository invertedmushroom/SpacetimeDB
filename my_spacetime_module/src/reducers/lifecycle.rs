use spacetimedb::{Identity, ReducerContext, Table};
use crate::tables::player::{Player, PlayerStatus, player};
use crate::tables::game_item::{GameItem, game_item};
use crate::calculate_chunk;

/**
 * Initialization reducer called when the module is first published.
 * 
 * Initial world state setup
 * This demonstrates how to populate the game world with starter entities
 * that should exist before any players connect.
 */
#[spacetimedb::reducer(init)]
pub fn module_init(ctx: &ReducerContext) -> Result<(), String> {
    log::info!("Game module initialized");
    
    // Create some initial game items in the world
    let timestamp = ctx.timestamp;
    
    // Create a health potion in the game world
    let health_potion = GameItem {
        item_id: 1,
        owner_id: Identity::default(),
        name: "Health Potion".to_string(),
        item_type: "Consumable".to_string(),
        value: 25,
        position_x: Some(100.0),
        position_y: Some(100.0),
        chunk_x: Some(calculate_chunk(100.0)),
        chunk_y: Some(calculate_chunk(100.0)),
        is_dropped: true,
        created_at: timestamp,
    };
    
    // Insert into database - pass the struct directly, not a reference
    ctx.db.game_item().insert(health_potion);
    
    Ok(())
}

/**
 * Client connection lifecycle reducer.
 * 
 * Player lifecycle management
 * 1. Creating new players for first-time connections
 * 2. Restoring existing players when they reconnect
 * 3. Using the client's Identity to link players to connections
 */
#[spacetimedb::reducer(client_connected)]
pub fn on_client_connected(ctx: &ReducerContext) -> Result<(), String> {
    let client_id = ctx.sender;
    log::info!("Client connected: {:?}", client_id);
    
    // Check if player exists
    if ctx.db.player().iter().find(|p| p.player_id == client_id).is_none() {
        // Create a new player with default stats
        let new_player = Player {
            player_id: client_id,
            username: format!("Player-{}", client_id.to_string()[0..8].to_string()),
            position_x: 50.0,
            position_y: 50.0,
            chunk_x: calculate_chunk(50.0),
            chunk_y: calculate_chunk(50.0),
            health: 100,
            score: 0,
            status: PlayerStatus::Online,
            last_active: ctx.timestamp,
        };
        
        // Insert directly on the table - no reference
        ctx.db.player().insert(new_player.clone());
        log::info!("Created new player: {}", new_player.username);
    } else {
        // Update existing player status
        let mut player = ctx.db.player().iter().find(|p| p.player_id == client_id).unwrap().clone();
        player.status = PlayerStatus::Online;
        player.last_active = ctx.timestamp;
        
        // Update player using primary key column
        ctx.db.player().player_id().update(player.clone());
        
        log::info!("Player returned: {}", player.username);
    }
    
    Ok(())
}

/**
 * Client disconnection lifecycle reducer.
 * 
 * Connection state management
 * 1. Updating player state when clients disconnect
 * 2. Preserving player data between sessions
 * 3. Timestamping for activity tracking
 */
#[spacetimedb::reducer(client_disconnected)]
pub fn on_client_disconnected(ctx: &ReducerContext) -> Result<(), String> {
    let client_id = ctx.sender;
    log::info!("Client disconnected: {:?}", client_id);
    
    // Update player status
    if let Some(player) = ctx.db.player().iter().find(|p| p.player_id == client_id) {
        let mut player = player.clone();
        player.status = PlayerStatus::Offline;
        player.last_active = ctx.timestamp;
        
        // Update player using primary key column
        ctx.db.player().player_id().update(player.clone());
        
        log::info!("Player {} is now offline", player.username);
    }
    
    Ok(())
}

/**
 * Dummy reducer for testing connection functionality.
 * 
 * Heartbeat/connectivity verification
 * This reducer demonstrates a minimal implementation for verifying
 * that the client can successfully call reducers.
 */
#[spacetimedb::reducer]
pub fn dummy(_ctx: &ReducerContext) -> Result<(), String> {
    Ok(())
}