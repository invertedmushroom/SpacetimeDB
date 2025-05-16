use spacetimedb::{reducer, ReducerContext};
use crate::tables::player::player;

#[reducer]
pub fn request_chunk_subscription(
    ctx: &ReducerContext,
    req_cx: i32,
    req_cy: i32,
) -> Result<(), String> {
    let player_id = ctx.sender;
    // Fetch and mutate the player row
    let mut player = ctx.db.player().player_id().find(player_id)
        .ok_or_else(|| "Player not found".to_string())?;

    let dx = (player.chunk_x - req_cx).abs();
    let dy = (player.chunk_y - req_cy).abs();
    log::info!("Chunk subscription request: ({}, {})", req_cx, req_cy);
    log::info!("Player chunk: ({}, {})", player.chunk_x, player.chunk_y);
    log::info!("dx: {}, dy: {}", dx, dy);
    if dx > 1 || dy > 1 {
        return Err("May only subscribe to your chunk or adjacent ones".to_string());
    }

    // Assign new subscription bounds to player and update the player row with new bounds
    player.min_x = req_cx - 1;
    player.max_x = req_cx + 1;
    player.min_y = req_cy - 1;
    player.max_y = req_cy + 1;

    ctx.db.player().player_id().update(player.clone());

    Ok(())
}