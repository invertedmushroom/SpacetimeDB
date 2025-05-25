/**
 * SpacetimeDB Game Client - Main Entry Point
 * 
 */
mod config;
mod module_bindings;
mod client;
mod game_features;

use module_bindings::ContactEventTableAccess;
use spacetimedb_sdk::{DbContext, Table};
use crate::game_features::{GameActions, ChunkSubscriptionManager, MoveAndPickupCommand, GameCommand, with_retry};
use crate::module_bindings::DbConnection;
use std::io::Write;
use phf::phf_map;
use crate::module_bindings::spawn_rigid_body_reducer::spawn_rigid_body;
use crate::module_bindings::despawn_rigid_body_reducer::despawn_rigid_body;
use crate::module_bindings::physics_body_table::PhysicsBodyTableAccess;
use crate::module_bindings::player_table::PlayerTableAccess;

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
    "fire"=> cmd_fire_projectile,
    "spawn" => cmd_spawn_object,
    "test" => cmd_physics_test,
    "contacts" => cmd_show_contacts,
    "bodies" => cmd_show_physics_bodies,
    "despawn" => cmd_despawn,
};

/// Holds mutable game context for command handlers
struct GameContext {
    chunk_mgr: ChunkSubscriptionManager,
    current_position: (f32, f32),
    player_id: spacetimedb_sdk::Identity,
    player_phy_entity_id: spacetimedb_sdk::Identity,
}

impl GameContext {
    /// Refresh `current_position` from the player's physics_body row
    fn refresh_position(&mut self) {
        let conn = self.chunk_mgr.get_connection();
        if let Some(body) = conn.db.physics_body().iter().find(|p| p.entity_id == self.player_phy_entity_id) {
            self.current_position = (body.pos_x, body.pos_y);
            println!("Current position updated to ({}, {})", self.current_position.0, self.current_position.1);
        }
        else {
            println!("Failed to refresh position: player physics body not found");
        }
    }
}

// Handler implementations
fn cmd_move(ctx: &mut GameContext, parts: &[&str]) {
    parse_args!(parts, "m <x> <y>", x: f32, y: f32);
    // Execute move with a short-lived connection borrow
    // Perform move and drop the borrow before modifying `ctx`
    let move_result = {
        let conn_ref = ctx.chunk_mgr.get_connection();
        with_retry(conn_ref, || conn_ref.move_player(x, y))
    };
    if let Err(e) = move_result {
        println!("Failed to move: {}", e);
        return;
    }
    // On success, wait for physics to settle, refresh position, and update subscriptions
    std::thread::sleep(std::time::Duration::from_millis(300));
    ctx.refresh_position();
    println!("Moved to ({}, {})", ctx.current_position.0, ctx.current_position.1);
    //refresh position does not update the chunk subscription or ctx.current_position
    (ctx.current_position.0, ctx.current_position.1) = (x, y);
    println!("Updating chunk subscription for new position ({}, {})", x, y);

    ctx.chunk_mgr.update_subscription_for_position(ctx.current_position.0, ctx.current_position.1);
    println!("Moved successfully.");
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

fn cmd_fire_projectile(ctx: &mut GameContext, parts: &[&str]) {
    // Parse direction and speed
    if parts.len() < 1 {
        println!("Usage: fire <angle_degrees> [speed=50]");
        return;
    }
    
    let angle: f32 = parts[0].parse().unwrap_or_else(|_| {
        println!("Invalid angle, using 0");
        0.0
    });
    let speed: f32 = if parts.len() > 1 { 
        parts[1].parse().unwrap_or_else(|_| {
            println!("Invalid speed, using default 50");
            50.0
        })
    } else { 50.0 };
    
    // Convert angle to radians
    //let angle_rad = angle * std::f32::consts::PI / 180.0;
    
    // Current position
    let (x, y) = ctx.current_position;
    let z = 1.0; // Height above ground
    
    // Calculate velocity components
    //let vel_x = speed * angle_rad.cos();
    //let vel_z = speed * angle_rad.sin();
    
    let conn = ctx.chunk_mgr.get_connection();
    
    // Spawn a projectile (rigid body type 10)
    match conn.reducers.spawn_rigid_body(
        //ctx.player_id,  // entity_id (owner, but will be updated in user_data)
        0,              // region
        x,              // x position
        z,              // y position (height)
        y,              // z position
        "Sphere(0.5)".to_string(), // small projectile
        10,             // PROJECTILE_BODY_TYPE
    ) {
        Ok(_) => println!("Projectile fired at angle {} degrees, speed {}", angle, speed),
        Err(e) => println!("Failed to fire projectile: {}", e),
    }
}

fn cmd_spawn_object(ctx: &mut GameContext, parts: &[&str]) {
    // Parse arguments: shape, body_type
    if parts.len() < 2 {
        println!("Usage: spawn <shape> <body_type>");
        println!("  shape: Sphere(radius) or Box(x,y,z)");
        println!("  body_type: 0=static, 1=dynamic, 2=kinematic, 10=projectile, 20=player");
        return;
    }
    
    let shape = parts[0].to_string();
    let body_type: u8 = parts[1].parse().unwrap_or(1);
        
    // Refresh and use current player position for spawning
    ctx.refresh_position();
    let (x, y) = ctx.current_position;
    let z = 1.0;

    // Spawn a rigid body with requested parameters in a scoped borrow
    let spawn_result = {
        let conn_ref = ctx.chunk_mgr.get_connection();
        conn_ref.reducers.spawn_rigid_body(
            0,              // region
            x,              // x position
            y,              // y position (height)
            z,              // z position
            shape.clone(),  // shape descriptor
            body_type,      // body type
        )
    };
    match spawn_result {
        Ok(_) => println!("Object spawned at ({}, {}, {})", x, z, y),
        Err(e) => println!("Failed to spawn object: {}", e),
    }
}

fn cmd_physics_test(ctx: &mut GameContext, parts: &[&str]) {
    if parts.is_empty() {
        println!("Usage: test <scenario>");
        println!("Available scenarios:");
        println!("  projectile - Test projectile hitting player");
        println!("  contact - Test contact duration recording");
        println!("  sensor - Test sensor triggers");
        return;
    }
    
    match parts[0] {
        "projectile" => test_projectile_scenario(ctx),
        "contact" => test_contact_duration_scenario(ctx),
        "sensor" => test_sensor_scenario(ctx),
        _ => println!("Unknown scenario: {}", parts[0]),
    }
}

fn test_projectile_scenario(ctx: &mut GameContext) {
    println!("Running projectile test scenario...");
    let conn = ctx.chunk_mgr.get_connection();
    let (x, y) = ctx.current_position;
    
    // 1. Create a player target at a distance
    println!("1. Spawning player target at ({}, {})", x + 10.0, y);
    match conn.reducers.spawn_rigid_body(
        //Identity::from_hex("target00000000000000000000000000000000").unwrap_or_default(),
        0,              // region
        x + 10.0,       // 10 units in front
        1.0,            // At player height
        y,              // y position
        "Sphere(1.0)".to_string(),
        20,             // PLAYER_BODY_TYPE
    ) {
        Ok(_) => println!("Target spawned successfully"),
        Err(e) => println!("Failed to spawn target: {}", e),
    };
    
    // 2. Fire projectile at the target after a brief delay
    std::thread::sleep(std::time::Duration::from_millis(500));
    println!("2. Firing projectile at target");
    match conn.reducers.spawn_rigid_body(
        //ctx.player_id,  // entity_id (owner)
        0,              // region
        x,              // x position
        1.0,            // y position (height)
        y,              // z position
        "Sphere(0.5)".to_string(),
        10,             // PROJECTILE_BODY_TYPE
    ) {
        Ok(_) => println!("Projectile fired successfully"),
        Err(e) => println!("Failed to fire projectile: {}", e),
    };
    
    println!("Test initiated. Projectile should hit target and cause damage.");
    println!("Note: Check server logs for collision events.");
}

fn test_contact_duration_scenario(ctx: &mut GameContext) {
    println!("Running contact duration test scenario...");
    let conn = ctx.chunk_mgr.get_connection();
    let (x, y) = ctx.current_position;
    
    // 1. Create a static body at current position
    println!("1. Spawning static object at ({}, {})", x, y);
    let _ = conn.reducers.spawn_rigid_body(
        //ctx.player_id,
        0,
        x,
        0.5,       // Half-height above ground
        y,
        "Sphere(2.0)".to_string(),
        0,         // STATIC_BODY_TYPE
    );
    
    // 2. Create a dynamic body just above it that will fall and make contact
    println!("2. Spawning dynamic object above it");
    let _ = conn.reducers.spawn_rigid_body(
        //ctx.player_id,
        0,
        x,
        5.0,       // Higher up to fall
        y,
        "Sphere(1.0)".to_string(),
        1,         // DYNAMIC_BODY_TYPE
    );
    
    println!("Test initiated. Objects should make contact and duration should be recorded.");
    println!("Check contact_duration table after a few seconds.");
}

fn test_sensor_scenario(ctx: &mut GameContext) {
    println!("Running sensor test scenario...");
    let conn = ctx.chunk_mgr.get_connection();
    let (x, y) = ctx.current_position;
    
    // 1. Create a sensor zone at current position
    println!("1. Spawning sensor at ({}, {})", x, y);
    match conn.reducers.spawn_rigid_body(
        //Identity::from_hex("sensor00000000000000000000000000000000").unwrap_or_default(),
        0,              // region
        x,              // x position
        1.0,            // y position (height) 
        y,              // z position
        "Sphere(3.0)Sensor".to_string(),  // Add "Sensor" suffix to make it a sensor
        0,              // STATIC_BODY_TYPE
    ) {
        Ok(_) => println!("Sensor spawned successfully"),
        Err(e) => println!("Failed to spawn sensor: {}", e),
    };
    
    // 2. Create a player body that will move through it
    std::thread::sleep(std::time::Duration::from_millis(500));
    println!("2. Spawning player body to enter sensor");
    match conn.reducers.spawn_rigid_body(
        //Identity::from_hex("dynamic0000000000000000000000000000000").unwrap_or_default(),
        0,              // region
        x - 10.0,       // Start outside sensor
        1.0,            // y position (height)
        y,              // z position
        "Sphere(1.0)".to_string(),
        20,             // PLAYER_BODY_TYPE
    ) {
        Ok(_) => println!("Player body spawned successfully"),
        Err(e) => println!("Failed to spawn player body: {}", e),
    };
    
    println!("Test initiated. Move the spawned player body into the sensor zone.");
    println!("Check server logs for sensor trigger events.");
}

fn cmd_show_contacts(ctx: &mut GameContext, _parts: &[&str]) {
    let conn = ctx.chunk_mgr.get_connection();
    
    println!("\nActive Contact Durations:");
    println!("------------------------");
    
    // Get contact durations
    let contacts: Vec<_> = conn.db.contact_event().iter().collect();
    
    if contacts.is_empty() {
        println!("No contact records found.");
    } else {
        println!("ID  | Entity1                | Entity2                | Started at (ms)");
        println!("----|------------------------|------------------------|-------------");
        
        for contact in contacts {
            // Get timestamp
            let duration_ms = contact.started_at;
            // Get skill_id
            let skill_id = contact.skill_id.to_hex();
            // Format entity IDs by their last 8 hex digits
            let h1 = contact.entity_1.to_hex().to_string();
            let e1 = &h1[h1.len().saturating_sub(8)..];
            let h2 = contact.entity_2.to_hex().to_string();
            let e2 = &h2[h2.len().saturating_sub(8)..];
            
            println!("{:3} | {:24} | {:24} | {:9.1} | {}", 
                     contact.id, e1, e2, duration_ms, skill_id);
        }
    }
    println!();
}

fn cmd_show_physics_bodies(ctx: &mut GameContext, _parts: &[&str]) {
    let conn = ctx.chunk_mgr.get_connection();
    println!("\nPhysics Bodies (chunk_entities view):");
    println!("-------------------------------------");
    // Iterate over the subscribed physics bodies
    let entities: Vec<_> = conn.db.physics_body().iter()
        .collect();
    if entities.is_empty() {
        println!("No physics bodies in the current chunks.");
    } else {
        println!("Entity ID         | Shape             | Position (x,y)");
        println!("------------------|-------------------|--------------");
        for e in entities {
            let id = e.entity_id.to_hex();
            let shape = e.collider_shape.clone();
            let body_type: u8 = e.body_type;
            println!("{} | {:17} | ({:.1}, {:.1}) {}", id, shape, e.pos_x, e.pos_y, body_type);
        }
    }
    println!();
}

fn cmd_despawn(ctx: &mut GameContext, parts: &[&str]) {
    // Usage: despawn <entity_id_hex> [region]
    if parts.len() < 1 {
        println!("Usage: despawn <entity_id_hex> [region]");
        return;
    }
    // Allow full hex or 8-digit suffix: match by suffix if shorter
    let input = parts[0];
    // Try full-hex parse first
    let mut entity_id = spacetimedb_sdk::Identity::from_hex(input).unwrap_or_default();
    if entity_id == spacetimedb_sdk::Identity::default() && input.len() <= 8 {
        // match suffix
        if let Some(mat) = ctx.chunk_mgr.get_connection().db.physics_body().iter()
            .find(|b| b.entity_id.to_hex().ends_with(input))
        {
            entity_id = mat.entity_id.clone();
        }
    }
    // Parse optional region (default 0)
    let region: u32 = if parts.len() > 1 { parts[1].parse().unwrap_or(0) } else { 0 };
    let conn = ctx.chunk_mgr.get_connection();
    match conn.reducers.despawn_rigid_body(entity_id, region) {
        Ok(_) => println!("Despawned entity {} in region {}", input, region),
        Err(e) => println!("Failed to despawn {}: {}", input, e),
    }
}

fn main() {
    // Create connection to SpacetimeDB server
    let conn = client::create_connection();
    println!("Connected to SpacetimeDB!");

    // Start listening and assign identity via try_identity polling
    let _conn_handle = conn.run_threaded();

    // Get the player identity from the connection (retry until assigned)
    let player_id = loop {
        if let Some(id) = conn.try_identity() {
            break id;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    println!("Player ID: {:?}", player_id);

    // Subscribe to the player table so we can load our player row 
    // (was same subscription in cient.rs "start_subscription" function but not called here jet)
    let _player_sub = conn.subscription_builder()
        .on_error(|_ctx, err| eprintln!("Player subscription error: {}", err))
        .subscribe(vec!["SELECT * FROM player".to_string()]);

    // Subscribe to contact_duration table to receive contact duration records on the client
    let _contact_sub = conn.subscription_builder()
        .on_error(|_ctx, err| eprintln!("Contact subscription error: {}", err))
        .subscribe(vec!["SELECT * FROM contact_event".to_string()]);

    // Default starting position when no position is found (will update after subscription)
    let mut current_position = (50.0, 50.0);

    // Initialize chunk subscription manager with our connection
    let mut chunk_mgr = ChunkSubscriptionManager::new(conn);
    println!("Chunk subscription manager initialized.");

    // Subscribe to player's inventory
    chunk_mgr.subscribe_to_inventory(player_id);
    // Subscribe to game items
    chunk_mgr.subscribe_to_game_items();
    // Initial subscription based on starting position
    chunk_mgr.update_subscription_for_position(current_position.0, current_position.1);
    // Wait briefly for physics_body updates
    std::thread::sleep(std::time::Duration::from_millis(200));
    // Try loading position from server-side physics_body via chunk manager
    if let Some(body) = chunk_mgr.get_connection().db.physics_body().iter().find(|p| p.owner_id == player_id) {
        current_position = (body.pos_x, body.pos_y);
    } else {
        println!("No player's physics_body found in subscribed chunks.");
    }
    println!("Initial position: ({}, {})", current_position.0, current_position.1);

    // Get the player's `phy_entity_id` from the player table
    let player_phy_entity_id = chunk_mgr.get_connection().db.player().iter()
        .find(|p| p.player_id == player_id)
        .map(|p| p.phy_entity_id)
        .unwrap_or_else(|| {
            println!("No Player row found in database.");
            spacetimedb_sdk::Identity::default()
        });

    // Build game context
    let context = GameContext { chunk_mgr, current_position, player_id, player_phy_entity_id};
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