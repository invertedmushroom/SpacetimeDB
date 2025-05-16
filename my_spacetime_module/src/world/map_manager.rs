use spacetimedb::{ReducerContext, Table};
use std::collections::HashSet;
use crate::tables::map_chunk::{map_chunk, MapChunk};
use log::info;


/// Manages the game map and ensures chunks exist before players enter them
pub struct MapManager;

/// Game map boundaries
pub const MAX_CHUNK_X: i32 = 100;
pub const MIN_CHUNK_X: i32 = -100;
pub const MAX_CHUNK_Y: i32 = 100;
pub const MIN_CHUNK_Y: i32 = -100;

// Chunk generation parameters
const DEFAULT_CHUNK_GENERATION_RADIUS: i32 = 2;

impl MapManager {
    /// Ensures that a chunk exists in the database
    /// If it doesn't exist, it creates it with default values
    pub fn ensure_chunk_exists(ctx: &ReducerContext, chunk_x: i32, chunk_y: i32) -> Result<(), String> {
        // Check if the chunk coordinates are within valid range
        if !Self::is_chunk_in_valid_range(chunk_x, chunk_y) {
            return Err(format!("Chunk coordinates ({}, {}) are outside the valid world boundaries", chunk_x, chunk_y));
        }
        
        // Check if chunk already exists
        let chunk_exists = ctx.db.map_chunk().iter()
            .any(|c| c.chunk_x == chunk_x && c.chunk_y == chunk_y);
        
        // If chunk doesn't exist, create it
        if !chunk_exists {
            // Generate a new chunk ID using a deterministic method based on coordinates
            let chunk_id = Self::generate_chunk_id(chunk_x, chunk_y);
            
            // Create the chunk with default parameters
            // In a real game, this would include terrain generation, etc.
            let new_chunk = MapChunk {
                chunk_id,
                chunk_x,
                chunk_y,
                terrain_type: "default".to_string(),
                is_generated: true,
                last_updated: ctx.timestamp,
            };
            
            ctx.db.map_chunk().insert(new_chunk);
            info!("Created new map chunk at ({}, {})", chunk_x, chunk_y);
        }
        
        Ok(())
    }
    
    /// Generate chunks in a radius around a point to prevent "pop-in"
    pub fn ensure_chunks_exist_in_radius(
        ctx: &ReducerContext,
        center_x: i32,
        center_y: i32,
        radius: Option<i32>,
    ) -> Result<(), String> {
        let radius = radius.unwrap_or(DEFAULT_CHUNK_GENERATION_RADIUS);
        
        // Get all chunks that should exist
        let chunks_to_check = Self::get_chunks_in_radius(center_x, center_y, radius);
        
        // Batch generation for efficiency
        let mut chunks_to_generate = Vec::new();
        
        // Find which chunks don't exist yet
        let existing_chunks: HashSet<(i32, i32)> = ctx.db.map_chunk().iter()
            .map(|c| (c.chunk_x, c.chunk_y))
            .collect();
        
        for (x, y) in chunks_to_check {
            if !existing_chunks.contains(&(x, y)) && Self::is_chunk_in_valid_range(x, y) {
                chunks_to_generate.push((x, y));
            }
        }
        
        info!("Generating {} new chunks around ({}, {})", chunks_to_generate.len(), center_x, center_y);
        
        // Generate all needed chunks
        for (x, y) in chunks_to_generate {
            let chunk_id = Self::generate_chunk_id(x, y);
            
            let new_chunk = MapChunk {
                chunk_id,
                chunk_x: x,
                chunk_y: y,
                terrain_type: "default".to_string(),
                is_generated: true,
                last_updated: ctx.timestamp,
            };
            
            ctx.db.map_chunk().insert(new_chunk);
        }
        
        Ok(())
    }
    
    /// Generate a deterministic chunk ID from coordinates
    /// This ensures the same chunk always gets the same ID
    fn generate_chunk_id(chunk_x: i32, chunk_y: i32) -> u64 {
        // Simple but effective way to create unique IDs based on coordinates
        // Uses Cantor pairing function to create a unique ID for each x,y pair
        let x = chunk_x as i64;
        let y = chunk_y as i64;
        
        // Handle negative numbers with a twist on Cantor pairing
        let a = if x >= 0 { 2 * x } else { -2 * x - 1 };
        let b = if y >= 0 { 2 * y } else { -2 * y - 1 };
        
        // Cantor pairing function: (a+b)*(a+b+1)/2 + b
        let cantor = ((a + b) * (a + b + 1) / 2 + b) as u64;
        cantor
    }
    
    /// Get all chunk coordinates in a square radius
    pub fn get_chunks_in_radius(center_x: i32, center_y: i32, radius: i32) -> Vec<(i32, i32)> {
        let mut chunks = Vec::new();
        
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                chunks.push((center_x + dx, center_y + dy));
            }
        }
        
        chunks
    }
    
    /// Check if chunk coordinates are within the valid world boundaries
    pub fn is_chunk_in_valid_range(chunk_x: i32, chunk_y: i32) -> bool {
        chunk_x >= MIN_CHUNK_X && chunk_x <= MAX_CHUNK_X &&
        chunk_y >= MIN_CHUNK_Y && chunk_y <= MAX_CHUNK_Y
    }
    
}
