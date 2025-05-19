/**
 * SpacetimeDB Client Connectivity Module
 * 
 * This module handles connection management and subscription initialization
 * for the SpacetimeDB client. It implements:
 * 1. Factory Pattern: Encapsulating connection creation
 * 2. Dependency Injection: Using traits for testable components
 * 3. Subscription Management: Centralized subscription creation
 */
//use spacetimedb_sdk::DbContext;
use crate::config::{SERVER_URI, MODULE_NAME};
//use crate::module_bindings::{DbConnection, SubscriptionHandle};
use crate::module_bindings::DbConnection;

/// Build and connect to the remote SpacetimeDB instance
pub fn create_connection() -> DbConnection {
    // Build connection with optional authentication token
    let mut builder = DbConnection::builder()
        .with_uri(SERVER_URI)
        .with_module_name(MODULE_NAME);
    if let Some(token) = crate::config::get_token() {
        builder = builder.with_token(Some(token));
    }
    builder
        .build()
        .expect("Failed to connect")
}

// /// Subscribe to this client’s own data tables (player, game_item, physics_body, chunk_entities)
// #[allow(dead_code)]
// pub fn start_subscription(
//     conn: &DbConnection,
//     player_id: spacetimedb_sdk::Identity,
// ) -> SubscriptionHandle {
//     conn.subscription_builder()
//         .on_error(|_err_ctx, err| log::info!("Subscription error: {}", err))
//         .subscribe(vec![
//             format!("SELECT * FROM player WHERE player_id = '{}'", player_id.to_hex()),
//             format!("SELECT * FROM game_item WHERE owner_id = '{}'", player_id.to_hex()),
//             format!("SELECT * FROM physics_body WHERE owner_id = '{}'", player_id.to_hex()),
//             "SELECT * FROM chunk_entities".to_string(),
//         ])
// }

/// For mocking and testing
#[allow(dead_code)]

#[cfg(test)]
 mod tests {
}