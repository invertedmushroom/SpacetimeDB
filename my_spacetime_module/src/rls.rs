
#[cfg(feature = "unstable")]
use spacetimedb::{Filter, client_visibility_filter};

//
//  Row-Level Security (RLS) filters restrict which rows a client can see.
//  
//  These filters are evaluated on the server when a client makes a subscription.
//  They help optimize bandwidth usage by preventing irrelevant data from being
//  sent to clients.
//  
//  NOTE: The "unstable" feature must be enabled in Cargo.toml to use RLS:
//      spacetimedb = { version = "...", features = ["unstable"] }
// 
//  RLS is still evolving, so these examples demonstrate basic functionality.
// 

#[client_visibility_filter]
const PHYSICS_BODY_VISIBILITY: Filter = Filter::Sql("
    SELECT * FROM physics_body WHERE owner_id = :sender
");

#[client_visibility_filter]
const PLAYER_TABLE_VISIBILITY: Filter = Filter::Sql("
    SELECT * FROM player WHERE player_id = :sender
");