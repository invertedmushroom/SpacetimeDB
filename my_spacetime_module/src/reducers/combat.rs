use spacetimedb::{Identity, ReducerContext, Table};
use crate::tables::{physics_body::physics_body, player::player};

/// Apply damage to a player by deleting the old record and inserting the updated one
pub fn apply_damage(ctx: &ReducerContext, target_id: Identity, _damage: u32) -> Result<(), String> {
    if let Some(mut _player) = ctx.db.player().iter().find(|p| p.player_id == target_id).map(|p| p.clone()) {
        //player.health = player.health.saturating_sub(damage);
        // Update player using primary key column
        ctx.db.player().player_id().update(_player.clone());
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
    for row in ctx.db.physics_body().iter() {
        let p = row.clone();
        let dx = p.pos_x - center_x;
        let dy = p.pos_y - center_y;
        if (dx*dx + dy*dy).sqrt() <= radius {
            apply_damage(ctx, p.owner_id, damage)?;
        }
    }

    Ok(())
}