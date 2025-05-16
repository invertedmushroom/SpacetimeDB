use spacetimedb::{Identity, ReducerContext, Table};
use crate::tables::player::player;
use crate::tables::game_item::game_item;
use crate::world::MapManager;
use crate::world::view_updater::{upsert_entity, delete_entity};
use crate::spacetime_common::spatial::calculate_chunk;
use rapier3d::na::Point3;
use crate::tables::physics_body::physics_body;
use crate::physics::PHYSICS_CONTEXTS;
use rapier3d::na::Isometry3;
use crate::spacetime_common::spatial::are_chunks_adjacent;

/**
 * Player movement reducer.
 * 
 * Position-based game mechanics
 * 1. Validating game world boundaries
 * 2. Updating player position
 * 3. Activity tracking for idle detection
 * 4. Managing chunk-based subscriptions
 * 5. Ensuring map chunks exist before player enters
 */
#[spacetimedb::reducer]
pub fn move_player(ctx: &ReducerContext, new_x: f32, new_y: f32) -> Result<(), String> {
    let player_id = ctx.sender;
    
    if let Some(player) = ctx.db.player().iter().find(|p| p.player_id == player_id) {
        let mut player = player.clone();

        // disallow moving further than adjacent chunks
        {
            let req_cx = calculate_chunk(new_x);
            let req_cy = calculate_chunk(new_y);
            let dx = (player.chunk_x - req_cx).abs();
            let dy = (player.chunk_y - req_cy).abs();
            if dx > 1 || dy > 1 {
                return Err("Cannot move more than one chunk at a time".to_string());
            }
        }
        
        // Calculate new chunk coordinates
        let old_chunk_x = player.chunk_x;
        let old_chunk_y = player.chunk_y;
        let new_chunk_x = calculate_chunk(new_x);
        let new_chunk_y = calculate_chunk(new_y);
        
        // Check if player is moving to a new chunk
        let chunk_changed = new_chunk_x != old_chunk_x || new_chunk_y != old_chunk_y;
        
        if chunk_changed {
            // Ensure the new chunk exists and is generated before letting player move there
            MapManager::ensure_chunk_exists(ctx, new_chunk_x, new_chunk_y)?;
            // Generate surrounding chunks to prevent "pop-in"
            MapManager::ensure_chunks_exist_in_radius(ctx, new_chunk_x, new_chunk_y, None)?;
        }
        
        // Update player position and other data
        player.position_x = new_x;
        player.position_y = new_y;
        player.chunk_x = new_chunk_x;
        player.chunk_y = new_chunk_y;
        player.last_active = ctx.timestamp;
        
        // Update player using primary key column
        ctx.db.player().player_id().update(player.clone());
        // Update view table for this player
        upsert_entity(
            ctx,
            player_id,
            "player",
            new_x,
            new_y,
            new_chunk_x,
            new_chunk_y,
            None,
        );
        // Player and it's public view table position out of sync with physics body
        // if simulation results in a different position
        log::info!("Player {} moved to ({}, {})", player.username, new_x, new_y);
        // Teleport the player's physics body via Rapier
        if let Some(phys) = ctx.db.physics_body().iter().find(|b| b.owner_id == player_id) {
            let mut contexts = PHYSICS_CONTEXTS.lock().unwrap();
            if let Some(world) = contexts.get_mut(&phys.region) {
                // collect handles to avoid borrow conflicts
                let handles: Vec<_> = world.bodies.iter()
                    .filter_map(|(h, body)| {
                        if body.user_data == phys.entity_id.to_u256().as_u128() {
                            Some(h.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                // now teleport each matching body
                for h in handles {
                    if let Some(rb) = world.bodies.get_mut(h) {
                        rb.set_next_kinematic_position(Isometry3::translation(new_x, 0.0, new_y));
                        log::info!("Teleported physics body {} to ({}, {})", phys.entity_id.to_hex(), new_x, new_y);
                    }
                }
            }
        }
        
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
    let player = ctx.db.player().iter().find(|p| p.player_id == player_id).ok_or("Player not found".to_string())?;

    // Find the item
    let item = ctx.db.game_item().iter().find(|i| i.item_id == item_id).ok_or("Item not found".to_string())?.clone();

    // Check if item is available to pick up
    if !item.is_dropped {
        return Err("Item is not available for pickup".to_string());
    }

    // Chunk-based pre-check: ensure item is in current or adjacent chunk
    let player_chunk = (player.chunk_x, player.chunk_y);
    if let (Some(item_cx), Some(item_cy)) = (item.chunk_x, item.chunk_y) {
        log::info!("Chunk pre-check: player_chunk={:?}, item_chunk=({}, {})", player_chunk, item_cx, item_cy);
        if !are_chunks_adjacent(player.chunk_x, player.chunk_y, item_cx, item_cy) {
            return Err("Item is too far away (not in adjacent chunks)".to_string());
        }
    } else {
        log::warn!("Item {} has no chunk coords, skipping chunk pre-check", item_id);
    }

    // Use rapier3d for precise proximity check
    // TODO use physics body position linked to player (and item)
    if let (Some(item_x), Some(item_y)) = (item.position_x, item.position_y) {        
        let player_position = Point3::new(player.position_x, player.position_y, 0.0);
        let item_position = Point3::new(item_x, item_y, 0.0);
        let distance = (player_position - item_position).norm();

        // // Debug: print chunk positions for player and item
        // let pi_chunk = (player.chunk_x, player.chunk_y);
        // let it_chunk = (item.position_x.map(calculate_chunk).unwrap_or(-999), item.position_y.map(calculate_chunk).unwrap_or(-999));
        // log::info!("Pickup debug: player at chunk {:?}, item at chunk {:?}", pi_chunk, it_chunk);

        // Define pickup radius
        let pickup_radius = 2.0;
        
        if distance > pickup_radius {
            return Err(format!("Item is too far away ({}m, max {}m)", distance, pickup_radius));
        }
    } else {
        return Err("Item has no position coordinates".to_string());
    }

    // Update item ownership
    let mut updated_item = item.clone();
    updated_item.owner_id = player_id;
    updated_item.is_dropped = false;
    updated_item.position_x = None;
    updated_item.position_y = None;
    updated_item.chunk_x = None;
    updated_item.chunk_y = None;

    // Update item using primary key column
    ctx.db.game_item().item_id().update(updated_item);
    // Remove from view table
    delete_entity(ctx, Identity::from_u256((item_id as u128).into()));

    log::info!("Player {} picked up item {}", player.username, item.name);
    Ok(())
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
        // Insert into view table
        upsert_entity(
            ctx,
            Identity::from_u256((item_id as u128).into()),
            "game_item",
            player.position_x,
            player.position_y,
            player.chunk_x,
            player.chunk_y,
            Some(item.name.clone()),
        );
        
        log::info!("Player {} dropped item {}", player.username, item.name);
        Ok(())
    } else {
        Err("Item not found".to_string())
    }
}