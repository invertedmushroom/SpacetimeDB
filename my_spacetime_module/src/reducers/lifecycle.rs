use spacetimedb::{Identity, ReducerContext, Timestamp, ScheduleAt, Table};
use crate::tables::physics_body::physics_body;
use crate::tables::player::{Player, PlayerStatus};
use crate::tables::scheduling::PhysicsTickSchedule;
use crate::tables::game_item::GameItem;
use crate::physics::spawn_rigid_body;
use crate::tables::game_item::game_item;
use crate::tables::scheduling::physics_tick_schedule;
use crate::tables::player::player;
use crate::spacetime_common::spatial::calculate_chunk;
use crate::spacetime_common::collision::STATIC_BODY_TYPE;
/**
 * Initialization reducer called when the module is first published.
 * 
 * Initial world state setup
 */
#[spacetimedb::reducer(init)]
pub fn module_init(ctx: &ReducerContext) -> Result<(), String> {
    log::info!("Game module initialized");
    
    // Schedule physics ticks to run every 100ms (10 times per second)
    schedule_physics_tick(ctx, 0, None)?;
    
    // Create some initial game items in the world
    let timestamp = ctx.timestamp;
    
    // Create a health potion in the game world
    let health_potion = GameItem {
        item_id: 1,
        owner_id: Identity::default(),
        name: "Health Potion".to_string(),
        item_type: "Consumable".to_string(),
        value: 25,
        position_x: Some(65.0),
        position_y: Some(65.0),
        chunk_x: Some(calculate_chunk(65.0)),
        chunk_y: Some(calculate_chunk(65.0)),
        is_dropped: true,
        created_at: timestamp,
    };
    
    // Insert into database - pass the struct directly, not a reference
    ctx.db.game_item().insert(health_potion);

    // Spawn a static ground collider
    spawn_rigid_body(
        ctx,
        0, // physics region
        50.0,
        50.0,
        -1.0,
        format!("Box({}, {}, {})", 1000, 0.1, 1000),
        STATIC_BODY_TYPE,
    )?;
   
    Ok(())
}

/**
 * Helper function to schedule the next physics tick
 */
pub fn schedule_physics_tick(ctx: &ReducerContext, region: u32, last_id: Option<u64>) -> Result<(), String> {
    // Get the next ID
    let next_id = if let Some(id) = last_id {
        id + 1
    } else {
        // If we don't have a previous ID, find the max ID and increment
        let max_id = ctx.db.physics_tick_schedule().iter()
            .map(|s| s.scheduled_id)
            .max()
            .unwrap_or(0);
        max_id + 1
    };
    
    // Determine the base time for the next tick:
    // - if we have a prior schedule with the ID we're incrementing, use its scheduled_at time
    // - otherwise fall back to "now"
    let base_time = if let Some(id) = last_id {
        if let Some(prev_schedule) = ctx.db.physics_tick_schedule()
            .scheduled_id()
            .find(id)
        {
            if let ScheduleAt::Time(timestamp) = prev_schedule.scheduled_at {
                timestamp
            } else {
                ctx.timestamp
            }
        } else {
            ctx.timestamp
        }
    } else {
        ctx.timestamp
    };
    
    // Add a fixed 100ms (100,000 μs) to the base time
    let next_micros = base_time.to_micros_since_unix_epoch() + 100_000;
    let next_time = Timestamp::from_micros_since_unix_epoch(next_micros);
    
    // Create the schedule entry
    let schedule = PhysicsTickSchedule {
        scheduled_id: next_id,
        scheduled_at: ScheduleAt::Time(next_time),
        region,
    };
    
    // Insert the schedule entry
    ctx.db.physics_tick_schedule().insert(schedule);
    
    Ok(())
}

/**
 * Client connection lifecycle reducer.
 * 
 * Player lifecycle management
 * 1. Creating new players for first-time connections
 * 2. Restoring existing players when they reconnect
 * 3. Using the client's Identity to link players to connections
 * 4. Setting up chunk-based subscriptions
 */
#[spacetimedb::reducer(client_connected)]
pub fn on_client_connected(ctx: &ReducerContext) -> Result<(), String> {
    let client_id = ctx.sender;
    log::info!("Client connected: {:?}", client_id);
    
    // Check if player exists
    let existing_player = ctx.db.player().iter().find(|p| p.player_id == client_id);
    
    let _player = if existing_player.is_none() {
        // Determine spawn position
        let spawn_x = 50.0;
        let spawn_y = 50.0;
        let chunk_x = calculate_chunk(spawn_x);
        let chunk_y = calculate_chunk(spawn_y);
        
        // Ensure map chunks exist at spawn location before player spawns
        crate::world::MapManager::ensure_chunks_exist_in_radius(ctx, chunk_x, chunk_y, Some(2))?;
        
        // Spawn physics body for new player
        spawn_rigid_body(
            ctx,
            0u32,
            spawn_x,
            spawn_y,
            0.0f32,
            "Sphere(0.5)".to_string(),
            2u8,
        )?;
        // Find rigid body by owner tag and extract entity_id
        let player_physical_object = ctx.db.physics_body()
            .iter()
            .find(|p| p.owner_id == client_id)
            .ok_or_else(|| "Failed to spawn player physics body".to_string())?;
        let player_physical_object_id = player_physical_object.entity_id;

        // Create a new player with default stats
        let new_player = Player {
            player_id: client_id,
            username: format!("Player-{}", client_id.to_string()[0..8].to_string()),
            health: 100,
            score: 0,
            status: PlayerStatus::Online,
            last_active: ctx.timestamp,
            phy_entity_id: player_physical_object_id,
        };
        
        // Insert player
        ctx.db.player().insert(new_player.clone());
        log::info!("Created new player: {}", new_player.username);

        // Removed position and chunk fileds from player table
        // move_player reducer uses ctx.sender to match physics_body table to update on move
        // ensure spawn_rigid_body has correct owner_id as that is what will be used to match in other parts

        // Initialize chunk subscription bounds for this client
        new_player
    } else if let Some(mut player) = existing_player {
        // Update existing player status
        player.status = PlayerStatus::Online;
        player.last_active = ctx.timestamp;
        ctx.db.player().player_id().update(player.clone());
        player
    } else {
        return Err("Failed to create or restore player".to_string());
    };
    
    
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