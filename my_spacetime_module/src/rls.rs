//SpacetimeDB Row Level Security (RLS) is designed primarily for single-table access control, allowing you to restrict which rows of a public table each client can access based on ownership or similar direct relationships1. While RLS rules can use joins, they must return rows from only one table, and there are limitations—complex cross-table filtering like the chunk-based spatial filtering you attempted is not directly supported in RLS rules. RLS works best for scenarios where access can be controlled by a simple filter on the table itself, such as restricting rows by the requesting client's identity or straightforward relationships.
// !! Non-inner joins are not supported in RLS rules, and the filter must return rows from only one table. !!
// Because RLS only lets you write filters against ―one‖ public table, with no joins or subqueries, you can’t lean on RLS to “join” map_chunk → chunk_subscription → game_item. You’d have to fold all of your per‐chunk entity state into the single public table itself so that the RLS rule is just:
//SELECT * FROM map_chunk WHERE <some predicate on map_chunk columns and :sender>
// In practice that means denormalizing.

// Remove filters referencing status enum, as enum comparisons are not supported in RLS
#[allow(unused)]
#[cfg(feature = "unstable")]
use spacetimedb::{Filter, client_visibility_filter};

#[client_visibility_filter]
const PHYSICS_BODY_VISIBILITY: Filter = Filter::Sql("
    SELECT * FROM physics_body WHERE owner_id = :sender
");

#[client_visibility_filter]
const PLAYER_TABLE_VISIBILITY: Filter = Filter::Sql("
    SELECT * FROM player WHERE player_id = :sender
");