/**
 * SpacetimeDB Client Connectivity Module
 * 
 * This module handles connection management and subscription initialization
 * for the SpacetimeDB client. It implements:
 * 1. Factory Pattern: Encapsulating connection creation
 * 2. Dependency Injection: Using traits for testable components
 * 3. Subscription Management: Centralized subscription creation
 */
use spacetimedb_sdk::DbContext;
use crate::config::{SERVER_URI, MODULE_NAME};
use crate::module_bindings::{DbConnection, SubscriptionHandle};
use crate::module_bindings::dummy;

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

/// Subscribe to all public tables to keep the WebSocket alive
/// 
/// This function centralizes the definition of which data
/// the client needs to subscribe to. It uses SQL-like syntax
/// to express the data requirements declaratively.
#[allow(dead_code)]

pub fn start_subscription(conn: &DbConnection) -> SubscriptionHandle {
    conn.subscription_builder()
        .on_error(|_err_ctx, err| log::info!("Subscription error: {}", err))
        // Subscribe only to public tables
        .subscribe(["SELECT * FROM player", "SELECT * FROM game_item"])
}

/// For mocking and testing
#[allow(dead_code)]

pub trait DummyCaller {
    fn dummy_call(&self);
}

/// DummyCaller for the real SpacetimeDB connection
impl DummyCaller for DbConnection {
    fn dummy_call(&self) {
        self.reducers.dummy().expect("Failed to call dummy reducer");
    }
}

/// Call the dummy reducer to verify connection
#[allow(dead_code)]
pub fn call_dummy<C: DummyCaller>(conn: &C) {
    conn.dummy_call();
}

#[cfg(test)]
 mod tests {
    use super::*;
    use std::cell::Cell;

    /// Mock implementation for testing
     struct FakeConn {
         called: Cell<bool>,
     }

    impl DummyCaller for FakeConn {
        fn dummy_call(&self) {
            self.called.set(true);
        }
    }

    #[test]
    fn call_dummy_invokes_dummy() {
        let fake = FakeConn { called: Cell::new(false) };
        call_dummy(&fake);
        assert!(fake.called.get(), "Dummy was not called");
    }
}