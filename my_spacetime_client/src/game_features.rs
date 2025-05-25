/**
 * Game-Specific Features Module
 * 
 * This module serves as the domain layer between the generic SpacetimeDB
 * connectivity and the game-specific features. It implements:
 * 
 * 1. Façade Pattern: Simplifying access to underlying reducer operations
 * 2. Error Handling: Converting SDK errors to domain-specific errors
 * 3. Feature Encapsulation: Grouping related game operations
 */
use crate::module_bindings::DbConnection;
use crate::module_bindings::player_table::PlayerTableAccess;
use crate::module_bindings::game_item_table::GameItemTableAccess;
use crate::module_bindings::move_player_reducer::move_player;
use crate::module_bindings::pickup_item_reducer::pickup_item;
use crate::module_bindings::drop_item_reducer::drop_item;
use crate::module_bindings::combat_melee_reducer::combat_melee;
use crate::module_bindings::combat_aoe_reducer::combat_aoe;
use spacetimedb_sdk::{Identity, Table, TableWithPrimaryKey, DbContext};
use crate::module_bindings::{Player, GameItem};
use std::time::Duration;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use spacetimedb_sdk::SubscriptionHandle;  // bring unsubscribe into scope


/// Domain-specific error for game operations
#[derive(Debug, PartialEq)]
pub enum GameError {
    /// Underlying SDK error with message
    SdkError(String),
    /// Entity not found
    NotFound(String),
    /// Invalid operation
    #[allow(dead_code)]
    InvalidOperation(String),
    /// Network error
    NetworkError(String),
}

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameError::SdkError(e) => write!(f, "SDK Error: {}", e),
            GameError::NotFound(e) => write!(f, "Not Found: {}", e),
            GameError::InvalidOperation(e) => write!(f, "Invalid Operation: {}", e),
            GameError::NetworkError(e) => write!(f, "Network Error: {}", e),
        }
    }
}

impl std::error::Error for GameError {}

/// Convert from String errors to GameError
impl From<String> for GameError {
    fn from(s: String) -> Self {
        if s.contains("not found") {
            GameError::NotFound(s)
        } else if s.contains("network") || s.contains("connection") {
            GameError::NetworkError(s)
        } else {
            GameError::SdkError(s)
        }
    }
}

/// Convert from &str errors to GameError
impl From<&str> for GameError {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

/// Game state cache that stores local copies of game entities
#[derive(Default)]
pub struct GameState {
    pub players: HashMap<Identity, Player>,
    pub items: HashMap<u64, GameItem>,
}
#[allow(dead_code)]
impl GameState {
    /// Create a new empty game state
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            items: HashMap::new(),
        }
    }
    
    /// Get a player by ID
    pub fn get_player(&self, id: &Identity) -> Option<&Player> {
        self.players.get(id)
    }
    
    /// Get an item by ID
    pub fn get_item(&self, id: u64) -> Option<&GameItem> {
        self.items.get(&id)
    }
    
    /// Find items near a player (within radius)
    pub fn find_nearby_items(&self, player_pos: (f32, f32), radius: f32) -> Vec<&GameItem> {
        self.items.values()
            .filter(|item| {
                if let (Some(x), Some(y)) = (item.position_x, item.position_y) {
                    let dx = x - player_pos.0;
                    let dy = y - player_pos.1;
                    (dx * dx + dy * dy).sqrt() <= radius
                } else {
                    false
                }
            })
            .collect()
    }
    
    /// Update from subscription callbacks
    pub fn update_player(&mut self, player: Player) {
        self.players.insert(player.player_id.clone(), player);
    }
    
    /// Update an item from subscription
    pub fn update_item(&mut self, item: GameItem) {
        self.items.insert(item.item_id, item);
    }
    
    /// Remove a player
    pub fn remove_player(&mut self, player_id: &Identity) {
        self.players.remove(player_id);
    }
    
    /// Remove an item
    pub fn remove_item(&mut self, item_id: u64) {
        self.items.remove(&item_id);
    }
}

/// Trait defining game actions for testable client operations
pub trait GameActions {
    fn move_player(&self, new_x: f32, new_y: f32) -> Result<(), GameError>;
    fn pickup_item(&self, item_id: u64) -> Result<(), GameError>;
    fn drop_item(&self, item_id: u64) -> Result<(), GameError>;
    fn attack_player(&self, target: Identity, damage: u32) -> Result<(), GameError>;
    fn aoe_attack(&self, center_x: f32, center_y: f32, radius: f32, damage: u32) -> Result<(), GameError>;
    fn get_state(&self) -> Arc<Mutex<GameState>>;
    fn with_retry<F>(&self, f: F, max_retries: usize) -> Result<(), GameError>
        where F: FnMut() -> Result<(), GameError>;
}

/// Real implementation of game actions against a DbConnection
impl GameActions for DbConnection {
    fn move_player(&self, new_x: f32, new_y: f32) -> Result<(), GameError> {
        self.reducers.move_player(new_x, new_y)
            .map_err(|e| GameError::SdkError(e.to_string()))
    }
    
    fn pickup_item(&self, item_id: u64) -> Result<(), GameError> {
        self.reducers.pickup_item(item_id)
            .map_err(|e| GameError::SdkError(e.to_string()))
    }
    
    fn drop_item(&self, item_id: u64) -> Result<(), GameError> {
        self.reducers.drop_item(item_id)
            .map_err(|e| GameError::SdkError(e.to_string()))
    }
    
    fn attack_player(&self, target: Identity, damage: u32) -> Result<(), GameError> {
        self.reducers.combat_melee(target, damage)
            .map_err(|e| GameError::SdkError(e.to_string()))
    }
    
    fn aoe_attack(&self, center_x: f32, center_y: f32, radius: f32, damage: u32) -> Result<(), GameError> {
        self.reducers.combat_aoe(center_x, center_y, radius, damage)
            .map_err(|e| GameError::SdkError(e.to_string()))
    }
    
    /// Get the game state (dummy implementation - actual state management requires
    /// external state storage as DbConnection isn't designed to store this)
    fn get_state(&self) -> Arc<Mutex<GameState>> {
        thread_local! {
            static STATE: Arc<Mutex<GameState>> = Arc::new(Mutex::new(GameState::new()));
        }
        
        STATE.with(|s| s.clone())
    }
    
    /// Retry a game operation with exponential backoff
    fn with_retry<F>(&self, mut f: F, max_retries: usize) -> Result<(), GameError> 
        where F: FnMut() -> Result<(), GameError>
    {
        let mut attempts = 0;
        loop {
            match f() {
                Ok(_) => return Ok(()),
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        return Err(e);
                    }
                    
                    // Only retry network errors
                    match e {
                        GameError::NetworkError(_) => {
                            // Exponential backoff
                            let backoff = Duration::from_millis(50 * 2u64.pow(attempts as u32));
                            std::thread::sleep(backoff);
                        },
                        _ => return Err(e),
                    }
                }
            }
        }
    }
}

/// Game-specific command pattern for complex operations
pub trait GameCommand {
    fn execute(&self, actions: &impl GameActions) -> Result<(), GameError>;
}

/// Command to move to an item and pick it up
pub struct MoveAndPickupCommand {
    pub item_id: u64,
    pub target_pos: (f32, f32),
}

impl GameCommand for MoveAndPickupCommand {
    fn execute(&self, actions: &impl GameActions) -> Result<(), GameError> {
        // First move to the item's position
        actions.move_player(self.target_pos.0, self.target_pos.1)?;
        
        // Then try to pick up the item
        actions.pickup_item(self.item_id)?;
        
        Ok(())
    }
}

/// Subscription manager to centralize handling of subscriptions
#[allow(dead_code)]
pub struct SubscriptionManager {
    conn: DbConnection,
    state: Arc<Mutex<GameState>>,
}
#[allow(dead_code)]
impl SubscriptionManager {
    pub fn new(conn: DbConnection) -> Self {
        let state = Arc::new(Mutex::new(GameState::new()));
        let state_clone = state.clone();
        
        // Set up player subscription callbacks
        conn.db.player().on_insert(move |_ctx, player| {
            if let Ok(mut state) = state_clone.lock() {
                state.update_player(player.clone());
            }
        });
        
        let state_clone = state.clone();
        conn.db.player().on_update(move |_ctx, _old, new| {
            if let Ok(mut state) = state_clone.lock() {
                state.update_player(new.clone());
            }
        });
        
        let state_clone = state.clone();
        conn.db.player().on_delete(move |_ctx, player| {
            if let Ok(mut state) = state_clone.lock() {
                state.remove_player(&player.player_id);
            }
        });
        
        // Set up item subscription callbacks
        let state_clone = state.clone();
        conn.db.game_item().on_insert(move |_ctx, item| {
            if let Ok(mut state) = state_clone.lock() {
                state.update_item(item.clone());
            }
        });
        
        let state_clone = state.clone();
        conn.db.game_item().on_update(move |_ctx, _old, new| {
            if let Ok(mut state) = state_clone.lock() {
                state.update_item(new.clone());
            }
        });
        
        let state_clone = state.clone();
        conn.db.game_item().on_delete(move |_ctx, item| {
            if let Ok(mut state) = state_clone.lock() {
                state.remove_item(item.item_id);
            }
        });
        
        Self { conn, state }
    }
    
    // /// Start a spatial subscription for dropped items; client does radius filtering locally
    // pub fn subscribe_to_area(&self, _center: (f32, f32), _radius: f32) {
    //     self.conn.subscription_builder()
    //         .on_error(|_ctx, err| log::info!("Area subscription error: {}", err))
    //         .subscribe(vec!["SELECT * FROM game_item WHERE is_dropped = true".to_string()]);
    // }
    
    // /// Subscribe to all items owned by the player
    // pub fn subscribe_to_inventory(&self, player_id: Identity) {
    //     let query = format!(
    //         "SELECT * FROM game_item WHERE owner_id = '{}'",
    //         player_id
    //     );
        
    //     self.conn.subscription_builder()
    //         .on_error(|_ctx, err| log::info!("Inventory subscription error: {}", err))
    //         .subscribe(vec![query]);
    // }
    
    /// Get access to the shared game state
    pub fn get_state(&self) -> Arc<Mutex<GameState>> {
        self.state.clone()
    }
    
    /// Get the underlying connection
    pub fn get_connection(&self) -> &DbConnection {
        &self.conn
    }
}

#[allow(dead_code)]

pub fn execute_command<A: GameActions>(actions: &A, command: impl GameCommand) -> Result<(), GameError> {
    command.execute(actions)
}

/// Execute a game action with retry on network failures
pub fn with_retry<A: GameActions, F>(actions: &A, f: F) -> Result<(), GameError>
where
    F: FnMut() -> Result<(), GameError>,
{
    actions.with_retry(f, 3)
}

/// Manager for chunk-based world subscriptions
pub struct ChunkSubscriptionManager {
    conn: DbConnection,
    subscription_handle: Option<crate::module_bindings::SubscriptionHandle>,
    current_chunk: Option<(i32, i32)>,
    // Track the current subscription area (3x3 grid of chunks)
    current_subscription_area: Option<(i32, i32, i32, i32)>, // (min_x, min_y, max_x, max_y)
}

impl ChunkSubscriptionManager {
    pub fn new(conn: DbConnection) -> Self {
        // Register callbacks to update local game state on item changes
        let state = conn.get_state();
        // on insert
        {
            let state_clone = state.clone();
            conn.db.game_item().on_insert(move |_ctx, item| {
                if let Ok(mut s) = state_clone.lock() { s.update_item(item.clone()); }
            });
        }
        // on update
        {
            let state_clone = state.clone();
            conn.db.game_item().on_update(move |_ctx, _old, new| {
                if let Ok(mut s) = state_clone.lock() { s.update_item(new.clone()); }
            });
        }
        // on delete
        {
            let state_clone = state.clone();
            conn.db.game_item().on_delete(move |_ctx, item| {
                if let Ok(mut s) = state_clone.lock() { s.remove_item(item.item_id); }
            });
        }
        
        // // Initial subscription to all items to seed local state
        // // This is commented since it does not get executed after we changed code in main function in main.rs
        // let initial_state = conn.get_state();
        // let state_clone = initial_state.clone();
        // let init_sub = conn.subscription_builder()
        //     .on_error(|_ctx, err| log::info!("Initial subscribe error: {}", err))
        //     .on_applied(move |ctx| {
        //         if let Ok(mut s) = state_clone.lock() {
        //             s.items.clear();
        //             for item in ctx.db.game_item().iter() {
        //                 s.update_item(item.clone());
        //             }
        //         }
        //         log::info!("Initial items loaded: {}", ctx.db.game_item().iter().count());
        //     })
        //     .subscribe(vec!["SELECT * FROM game_item".to_string()]);
        let manager = Self { 
            conn, 
            //subscription_handle: Some(init_sub), 
            subscription_handle: None,
            current_chunk: None,
            current_subscription_area: None,
        };
        manager
    }
      /// Subscribe to entities in the given chunk and surrounding chunks (3x3 grid)
    pub fn subscribe_to_chunk(&mut self, cx: i32, cy: i32) {
        // Calculate the 3x3 grid area around the current chunk
        let min_x = cx - 1;
        let max_x = cx + 1;
        let min_y = cy - 1;
        let max_y = cy + 1;
        
        // Check if we're already subscribed to this area
        if self.current_subscription_area == Some((min_x, min_y, max_x, max_y)) {
            return; // Already subscribed to this area
        }
        
        // Unsubscribe previous
        if let Some(handle) = self.subscription_handle.take() {
            if let Err(e) = handle.unsubscribe_then(Box::new(|_| {})) {
                log::warn!("Failed to unsubscribe from previous chunk subscription: {}", e);
            }
        }

        // Build a query for all entities in the 3x3 grid of chunks
        let sql = format!(
            "SELECT * FROM physics_body WHERE chunk_x >= {} AND chunk_x <= {} AND chunk_y >= {} AND chunk_y <= {}",
            min_x, max_x, min_y, max_y
        );
        
        log::info!("Subscribing to chunks: x={}..{}, y={}..{}", min_x, max_x, min_y, max_y);
        
        let handle = self.conn
            .subscription_builder()
            .on_error(|_ctx, err| log::warn!("Chunk subscription error: {}", err))
            .subscribe(vec![sql]);
            
        self.subscription_handle = Some(handle);
        self.current_chunk = Some((cx, cy));
        self.current_subscription_area = Some((min_x, min_y, max_x, max_y));
    }

    /// Start a spatial subscription for dropped items;
    pub fn subscribe_to_game_items(&self) {
        self.conn.subscription_builder()
            .on_error(|_ctx, err| log::info!("Area subscription error: {}", err))
            .subscribe(vec!["SELECT * FROM game_item WHERE is_dropped = true".to_string()]);
    }

    /// Subscribe to player's inventory
    pub fn subscribe_to_inventory(&mut self, player_id: Identity) {
        let query = format!(
            "SELECT * FROM game_item WHERE owner_id = '{}'",
            player_id
        );
        let _ = self.conn.subscription_builder()
            .on_error(|_ctx, err| log::warn!("Inventory subscription error: {}", err))
            .subscribe(vec![query]);
    }    /// Update subscription based on player position
    pub fn update_subscription_for_position(&mut self, x: f32, y: f32) {
        let cx = ChunkSubscriptionManager::calculate_chunk(x);
        let cy = ChunkSubscriptionManager::calculate_chunk(y);
        
        // Check if we've moved to a new chunk
        if Some((cx, cy)) != self.current_chunk {
            println!("Player moved to new chunk: ({}, {}) - requesting server-side subscription", cx, cy);
            // Immediately subscribe
            self.subscribe_to_chunk(cx, cy);

        }
    }

    /// Access the local cached game state
    pub fn get_state(&self) -> Arc<Mutex<GameState>> {
        self.conn.get_state()
    }
    
    /// Get the underlying connection
    pub fn get_connection(&self) -> &DbConnection {
        &self.conn
    }
    
    fn calculate_chunk(coord: f32) -> i32 {
        (coord / 10.0).floor() as i32
    }
}