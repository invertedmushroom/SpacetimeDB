/**
 * SpacetimeDB Game Client - Main Entry Point
 * 
 */
mod config;
mod module_bindings;
mod client;
mod game_features;

use spacetimedb_sdk::{DbContext, Table};
use crate::module_bindings::player_table::PlayerTableAccess;
use crate::game_features::{GameActions, ChunkSubscriptionManager, MoveAndPickupCommand, GameCommand, with_retry};
use crate::module_bindings::DbConnection;
use std::io::Write;
use phf::phf_map;

// Macro to parse typed arguments or print usage and return
macro_rules! parse_args {
    ($parts:expr, $usage:expr, $($name:ident : $ty:ty),+) => {
        let mut iter = $parts.iter();
        $( let $name = match iter.next().and_then(|s| s.parse::<$ty>().ok()) {
            Some(v) => v,
            None => { println!("Usage: {}", $usage); return; }
        }; )+
    };
}

// Helper to run a mutable action with retry and print success/failure
fn with_feedback<A>(ctx: &mut GameContext, success: &str, failure: &str, mut action: A)
where A: FnMut(&DbConnection) -> Result<(), game_features::GameError>
{
    let conn = ctx.chunk_mgr.get_connection();
    // create a runner closure implementing FnMut()
    let runner = || action(conn);
    match with_retry(conn, runner) {
        Ok(_) => println!("{}", success),
        Err(e) => println!("{}: {}", failure, e),
    }
}

// Perfect-hash map for command dispatch
static COMMAND_MAP: phf::Map<&'static str, fn(&mut GameContext, &[&str])> = phf_map! {
    "m"   => cmd_move,
    "p"   => cmd_pickup,
    "d"   => cmd_drop,
    "mp"  => cmd_movepickup,
    "a"   => cmd_attack,
    "aoe" => cmd_aoe,
    "i"   => cmd_inventory,
    "n"   => cmd_nearby,
};

/// Holds mutable game context for command handlers
struct GameContext {
    chunk_mgr: ChunkSubscriptionManager,
    current_position: (f32, f32),
    player_id: spacetimedb_sdk::Identity,
}

// Handler implementations
fn cmd_move(ctx: &mut GameContext, parts: &[&str]) {
    parse_args!(parts, "m <x> <y>", x: f32, y: f32);
    let conn = ctx.chunk_mgr.get_connection();
    let result = with_retry(conn, || conn.move_player(x, y));
    match result {
        Ok(_) => {
            ctx.current_position = (x, y);
            ctx.chunk_mgr.update_subscription_for_position(x, y);
            println!("Moved successfully.");
        }
        Err(e) => println!("Failed to move: {}", e),
    }
}

fn cmd_pickup(ctx: &mut GameContext, parts: &[&str]) {
    parse_args!(parts, "p <item_id>", item_id: u64);
    with_feedback(ctx, "Item picked up successfully.", "Failed to pick up item", |c| c.pickup_item(item_id));
}

fn cmd_drop(ctx: &mut GameContext, parts: &[&str]) {
    parse_args!(parts, "d <item_id>", item_id: u64);
    with_feedback(ctx, "Item dropped successfully.", "Failed to drop item", |c| c.drop_item(item_id));
}

fn cmd_movepickup(ctx: &mut GameContext, parts: &[&str]) {
    parse_args!(parts, "mp <item_id> <x> <y>", item_id: u64, x: f32, y: f32);
    let command = MoveAndPickupCommand { item_id, target_pos: (x, y) };
    let conn = ctx.chunk_mgr.get_connection();
    match command.execute(conn) {
        Ok(_) => {
            ctx.current_position = (x, y);
            ctx.chunk_mgr.update_subscription_for_position(x, y);
            println!("Move and pickup successful.");
        }
        Err(e) => println!("Failed: {}", e),
    }
}

fn cmd_attack(ctx: &mut GameContext, parts: &[&str]) {
    parse_args!(parts, "a <player_id> <damage>", pid_str: String, dmg: u32);
    let target = spacetimedb_sdk::Identity::from_hex(pid_str.trim_start_matches("0x")).unwrap_or_default();
    with_feedback(ctx, "Attack successful.", "Attack failed", |c| c.attack_player(target, dmg));
}

fn cmd_aoe(ctx: &mut GameContext, parts: &[&str]) {
    parse_args!(parts, "aoe <x> <y> <radius> <damage>", x: f32, y: f32, r: f32, dmg: u32);
    with_feedback(ctx, "AOE attack successful.", "AOE attack failed", |c| c.aoe_attack(x, y, r, dmg));
}

fn cmd_inventory(ctx: &mut GameContext, _parts: &[&str]) {
    let state = ctx.chunk_mgr.get_state();
    let guard = state.lock().unwrap();
    println!("\nInventory:");
    println!("----------");
    let items: Vec<_> = guard.items.values()
        .filter(|i| i.owner_id == ctx.player_id && !i.is_dropped)
        .collect();
    if items.is_empty() { println!("No items in inventory."); }
    else {
        for i in items { println!("  [{}] {} ({})", i.item_id, i.name, i.item_type); }
    }
    println!();
}

fn cmd_nearby(ctx: &mut GameContext, _parts: &[&str]) {
    // Get nearby items from our subscription-managed local GameState
    let player_pos = ctx.current_position;
    let state = ctx.chunk_mgr.get_state();
    let guard = state.lock().unwrap();
    let nearby_items = guard.find_nearby_items(player_pos, 30.0);

    println!("\nNearby Items (within 30 units):");
    println!("-----------------------------");
    if nearby_items.is_empty() {
        println!("No items nearby.");
    } else {
        for i in nearby_items {
            // i.position_x and position_y are guaranteed Some by find_nearby_items
            let x = i.position_x.unwrap();
            let y = i.position_y.unwrap();
            let dist = ((x - player_pos.0).powi(2) + (y - player_pos.1).powi(2)).sqrt();
            println!("  [{}] {} at ({:.1}, {:.1}) - {:.1} units away", i.item_id, i.name, x, y, dist);
        }
    }
    println!();
}

fn main() {
    // Create connection to SpacetimeDB server
    let conn = client::create_connection();
    println!("Connected to SpacetimeDB!");

    // Get the player identity from the connection
    let player_id = conn.try_identity().unwrap_or_default();
    println!("Player ID: {:?}", player_id);
    
    // Start WebSocket message processing in background thread
    let _conn_handle = conn.run_threaded();
    
    // Default starting position when no position is found
    let mut current_position = (50.0, 50.0);
    
    // Wait briefly to allow connection to initialize
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    // Update based on actual player position from server (if available)
    if let Some(player) = conn.db.player().iter().find(|p| p.player_id == player_id) {
        current_position = (player.position_x, player.position_y);
    }
    
    // Initialize chunk subscription manager with our connection
    let mut chunk_mgr = ChunkSubscriptionManager::new(conn);
    println!("Chunk subscription manager initialized.");
    
    // Subscribe to player's inventory
    chunk_mgr.subscribe_to_inventory(player_id);
    
    // Initial subscription based on starting position
    chunk_mgr.update_subscription_for_position(current_position.0, current_position.1);
    println!("Initial position: ({}, {})", current_position.0, current_position.1);
    
    // Build game context
    let context = GameContext { chunk_mgr, current_position, player_id };
    let mut ctx = context;

    // Main game loop
    let mut exit = false;
    while !exit {
        print!("> "); std::io::stdout().flush().unwrap();
        let mut input = String::new(); std::io::stdin().read_line(&mut input).unwrap();
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() { continue; }
        // Dispatch command
        if let Some(&handler) = COMMAND_MAP.get(parts[0]) {
            handler(&mut ctx, &parts[1..]);
        } else if parts[0] == "q" {
            println!("Exiting..."); exit = true;
        } else {
            println!("Unknown command");
        }
    }
    
    // Get a final reference for disconnecting
    let conn_ref = ctx.chunk_mgr.get_connection();
    let _ = conn_ref.disconnect();
}