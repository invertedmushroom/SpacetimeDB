/**
 * SpacetimeDB Game Server Module
 * 
 * This module implements a multiplayer game framework using SpacetimeDB,
 * demonstrating core patterns for game state management, player lifecycles,
 * and item interactions. The architecture follows a centralized database
 * model where all game state exists in tables and all game logic is
 * implemented through reducers.
 */

// Server module entry point: re-export tables and reducers from submodules
pub mod world;

pub mod tables {
    pub mod player;
    pub mod game_item;
    pub mod physics_body;
    pub mod scheduling;
    pub mod contact_event;
    pub mod map_chunk;
    pub mod skill_cooldown;
    pub mod player_buffs;
    pub mod damage_event;
    pub mod buff_expiry_schedule;
}
pub mod reducers {
    pub mod combat;
    pub mod lifecycle;
    pub mod world;
}
pub mod physics;

pub mod spacetime_common;

// Re-export important types
pub use spacetimedb::{Identity, ReducerContext, Timestamp, SpacetimeType, Table};

// Re-export table types
pub use tables::player::{Player, PlayerStatus};
pub use tables::game_item::GameItem;
pub use tables::contact_event::ContactEvent;
// Re-export reducer functions
pub use reducers::lifecycle::{module_init, on_client_connected, on_client_disconnected};
pub use reducers::world::{move_player, pickup_item, drop_item};
// Chunk subscription request reducer
pub use reducers::combat::{_combat_melee, _combat_aoe};

pub mod rls;