use spacetimedb::{Identity, ReducerContext, Table};
use crate::tables::player::player;

/// Apply damage to a player by deleting the old record and inserting the updated one
pub fn apply_damage(ctx: &ReducerContext, target_id: Identity, damage: u32) -> Result<(), String> {
    if let Some(mut player) = ctx.db.player().iter().find(|p| p.player_id == target_id).map(|p| p.clone()) {
        player.health = player.health.saturating_sub(damage);
        // Update player using primary key column
        ctx.db.player().player_id().update(player.clone());
        Ok(())
    } else {
        Err("Target not found".to_string())
    }
}

#[spacetimedb::reducer]
/// Single-target melee attack
pub fn combat_melee(ctx: &ReducerContext, target: Identity, damage: u32) -> Result<(), String> {
    apply_damage(ctx, target, damage)
}

#[spacetimedb::reducer]
/// Area-of-effect damage around a point
pub fn combat_aoe(ctx: &ReducerContext, center_x: f32, center_y: f32, radius: f32, damage: u32) -> Result<(), String> {
    for row in ctx.db.player().iter() {
        let p = row.clone();
        let dx = p.position_x - center_x;
        let dy = p.position_y - center_y;
        if (dx*dx + dy*dy).sqrt() <= radius {
            apply_damage(ctx, p.player_id, damage)?;
        }
    }
    Ok(())
}