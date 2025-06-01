use spacetimedb::{Identity, ReducerContext, Table};
//use crate::tables::player::player;
use crate::tables::game_item::game_item;
use crate::world::MapManager;
use crate::spacetime_common::spatial::{calculate_chunk_pair, are_chunks_adjacent_simd};
use rapier3d::na::Point3;
use crate::tables::physics_body::physics_body;
use crate::physics::PHYSICS_CONTEXTS;
use rapier3d::na::Isometry3;
use crate::physics::rapier_common::*;  // bring in IdentityRawExt for to_raw_u64()

/**
 * Player movement reducer.
 * 
 * Position-based game mechanics
 */
#[spacetimedb::reducer]
pub fn move_player(ctx: &ReducerContext, new_x: f32, new_y: f32) -> Result<(), String> {
    let player_id = ctx.sender;
    
    if let Some(player_physical_object) = ctx.db.physics_body().iter().find(|p| p.owner_id == player_id) {
        
        let player = player_physical_object.clone();

        // disallow moving further than adjacent chunks
        let (new_chunk_x, new_chunk_y) = calculate_chunk_pair(new_x, new_y);
        // adjacency helper
        if !are_chunks_adjacent_simd(player.chunk_x, player.chunk_y, new_chunk_x, new_chunk_y) {
            return Err("Cannot move more than one chunk at a time".to_string());
        }
    
        // Calculate new chunk coordinates
        let old_chunk_x = player.chunk_x;
        let old_chunk_y = player.chunk_y;
        
        // Check if player is moving to a new chunk
        let chunk_changed = new_chunk_x != old_chunk_x || new_chunk_y != old_chunk_y;
        
        if chunk_changed {
            // Ensure the new chunk exists and is generated before letting player move there
            MapManager::ensure_chunk_exists(ctx, new_chunk_x, new_chunk_y)?;
            // Generate surrounding chunks to prevent "pop-in"
            MapManager::ensure_chunks_exist_in_radius(ctx, new_chunk_x, new_chunk_y, None)?;
        }

        // Nov let the simulation update physics_body position
        log::info!("Physics_body with entity_id {} and owner_id {} will move to ({}, {}), on next physics tick", player.entity_id, player.owner_id, new_x, new_y);
        // Teleport the player's physics body via Rapier
        if let Some(phys) = ctx.db.physics_body().iter().find(|b| b.owner_id == player_id) {
            let mut contexts = PHYSICS_CONTEXTS.lock().unwrap();
            if let Some(world) = contexts.get_mut(&phys.region) {
                // O(1) forward lookup via id_to_body map
                if let Some(&handle) = world.id_to_body.get(&phys.entity_id.to_raw_u32()) {
                    if let Some(rb) = world.bodies.get_mut(handle) {
                        rb.set_next_kinematic_position(Isometry3::translation(new_x, new_y, 0.0));
                        log::info!("Teleported physics body {} to ({}, {}), on next physics tick", phys.entity_id.to_hex(), new_x, new_y);
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
 */
#[spacetimedb::reducer]
pub fn pickup_item(ctx: &ReducerContext, item_id: u64) -> Result<(), String> {
    let player_id = ctx.sender;
    log::info!("Player {} is trying to pick up item {}", player_id, item_id);

    // Verify player's object exists
    let player_physical_object = ctx.db.physics_body().iter().find(|p| p.owner_id == player_id).ok_or("Player not found".to_string())?;

    // Find the item
    let item = ctx.db.game_item().iter().find(|i| i.item_id == item_id).ok_or("Item not found".to_string())?.clone();

    // Check if item is available to pick up
    if !item.is_dropped {
        return Err("Item is not available for pickup".to_string());
    }

    // Chunk-based pre-check: ensure item is in current or adjacent chunk
    let player_chunk = (player_physical_object.chunk_x, player_physical_object.chunk_y);
    if let (Some(item_cx), Some(item_cy)) = (item.chunk_x, item.chunk_y) {
        log::info!("Chunk pre-check: player_chunk={:?}, item_chunk=({}, {})", player_chunk, item_cx, item_cy);
        // let (dx, dy) = abs_diff_pair(player_physical_object.chunk_x, player_physical_object.chunk_y, item_cx, item_cy);
        // if dx > 1 || dy > 1 {
        if !are_chunks_adjacent_simd(player_physical_object.chunk_x, player_physical_object.chunk_y, item_cx, item_cy) {
            return Err("Item is too far away (not in adjacent chunks)".to_string());
        }
    } else {
        log::warn!("Item {} has no chunk coords, skipping chunk pre-check", item_id);
    }

    // Use rapier3d for precise proximity check
    if let (Some(item_x), Some(item_y)) = (item.position_x, item.position_y) {        
        let player_position = Point3::new(player_physical_object.pos_x, player_physical_object.pos_y, 0.0);
        let item_position = Point3::new(item_x, item_y, 0.0);
        let distance = (player_position - item_position).norm();

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

    log::info!("Physics body with owner_id {} picked up item {}", player_physical_object.owner_id, item.name);
    // physics_body owner_id of player is the same as player_id of player
    Ok(())
}

/**
 * Item drop reducer.
 */
#[spacetimedb::reducer]
pub fn drop_item(ctx: &ReducerContext, item_id: u64) -> Result<(), String> {
    let player_id = ctx.sender;
    
    // Verify player exists
    let player = match ctx.db.physics_body().iter().find(|p| p.owner_id == player_id) {
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
        item.position_x = Some(player.pos_x);
        item.position_y = Some(player.pos_y);
        item.chunk_x = Some(player.chunk_x);
        item.chunk_y = Some(player.chunk_y);
        
        // Update item using primary key column
        ctx.db.game_item().item_id().update(item.clone());
                
        log::info!("Physics body with entity_id {} and owner_id {} dropped item {}", player.entity_id, player.owner_id, item.name);
        Ok(())
    } else {
        Err("Item not found".to_string())
    }
}