//SpacetimeDB Row Level Security (RLS) is designed primarily for single-table access control, allowing you to restrict which rows of a public table each client can access based on ownership or similar direct relationships1. While RLS rules can use joins, they must return rows from only one table, and there are limitations—complex cross-table filtering like the chunk-based spatial filtering you attempted is not directly supported in RLS rules. RLS works best for scenarios where access can be controlled by a simple filter on the table itself, such as restricting rows by the requesting client's identity or straightforward relationships.
// This filter is not valid in RLS rules, but is a placeholder for the concept.
// // Row-level security filter for game items
// /// Only show game_items whose x/y are within 20 units of the caller’s player row:
// #[cfg(feature = "unstable")]
// #[client_visibility_filter]
// const GAME_ITEM_PROXIMITY_FILTER: Filter = Filter::Sql(r#"
//     SELECT game_item.*
//       FROM game_item, player
//      WHERE player.player_id = :sender
//        AND game_item.position_x BETWEEN player.position_x - 20 AND player.position_x + 20
//        AND game_item.position_y BETWEEN player.position_y - 20 AND player.position_y + 20
// "#); // This filter is not valid in RLS rules, but is a placeholder for the concept.

// !! Non-inner joins are not supported in RLS rules, and the filter must return rows from only one table. !!

// Remove filters referencing status enum, as enum comparisons are not supported in RLS

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

// #[cfg(feature = "unstable")]
// #[client_visibility_filter]
// const GAME_ITEM_INVENTORY_FILTER: Filter = Filter::Sql(r#"
//     SELECT * FROM inventory_item
//     WHERE owner_id = :sender AND is_dropped = false
// "#);

#[cfg(feature = "unstable")]
#[client_visibility_filter]
const GAME_ITEM_WORLD_FILTER: Filter = Filter::Sql(r#"
    SELECT * FROM game_item
    WHERE is_dropped = true
"#);
