use crate::physics::rapier_common::*;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use rapier3d::prelude::*;
use spacetimedb::{reducer, ReducerContext, Timestamp, Table};
use crate::tables::player_buffs::{player_buffs, PlayerBuff};
use crate::tables::skill_cooldown::{skill_cooldown, SkillCooldown};
use crate::physics::PHYSICS_CONTEXTS;
use crate::physics::contact_tracker::register_owner;
use rapier3d::pipeline::QueryFilter;
use crate::tables::damage_event::{damage_event, DamageEvent};
use crate::tables::physics_body::physics_body;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global buff-ID generator
static BUFF_ID: AtomicU64 = AtomicU64::new(1);

pub type SkillId = u8;
pub type BuffType = u8;

// ———————————————— Buff system ————————————————

/// A buff can mutate your Cooldown before you cast
#[allow(dead_code)]
trait BuffBehavior: Sync + Send + 'static {
    fn buff_type(&self) -> BuffType;
    fn apply(&self, cd: &mut Cooldown, magnitude: f32);
}

/// Example:CD reduction
struct CdReductionBuff;
impl BuffBehavior for CdReductionBuff {
    fn buff_type(&self) -> BuffType { 1 }
    fn apply(&self, cd: &mut Cooldown, magnitude: f32) {
        cd.base_ms = ((cd.base_ms as f32) * (1.0 - magnitude.clamp(0.0,1.0))).round() as u32;
    }
}

// Registry of all buff impls
static BUFF_REGISTRY: Lazy<HashMap<BuffType, Box<dyn BuffBehavior>>> = Lazy::new(|| {
    let mut m: HashMap<BuffType, Box<dyn BuffBehavior>> = HashMap::new();
    m.insert(1, Box::new(CdReductionBuff));
    // insert other buffs here…
    m
});

// ———————————————— Skill & CD system ————————————————

/// Holds “last used” + “base ms” + transient reduction
struct Cooldown {
    last_used: Timestamp,
    base_ms: u32,
}
impl Cooldown {
    fn from_row(row: &SkillCooldown) -> Self {
        Cooldown { last_used: row.last_used_at, base_ms: row.base_cooldown }
    }
    fn to_row(&self, player: Identity, skill: SkillId) -> SkillCooldown {
        SkillCooldown {
             id: 0,
             player_id: player,
             skill_id: skill,
             last_used_at: self.last_used,
             base_cooldown: self.base_ms,
         }
     }
    fn is_ready(&self, now: Timestamp) -> bool {
        let elapsed_us = now.to_micros_since_unix_epoch()
                                 .saturating_sub(self.last_used.to_micros_since_unix_epoch());
        let elapsed_ms = (elapsed_us / 1000) as u64;
        elapsed_ms >= self.base_ms as u64
    }
    fn use_now(&mut self, now: Timestamp) {
        self.last_used = now;
    }
}

/// Skill behavior interface
#[allow(dead_code)]
trait SkillBehavior: Sync + Send + 'static { // Each implementor provides ID, base cooldown & activation
    fn id(&self) -> SkillId;
    fn base_ms(&self) -> u32;
    fn activate(
        &self,
        ctx: &ReducerContext,
        x: f32,
        y: f32,
        z: f32,
        dx: f32,
        dy: f32,
        dz: f32,
    );
}

/// Sensor‐pool skill
struct SensorSkill {
    id: SkillId,
    collider_handle: ColliderHandle,
    active_groups: InteractionGroups,
    inactive_groups: InteractionGroups,
}
impl SkillBehavior for SensorSkill {
    fn id(&self) -> SkillId { self.id }
    fn base_ms(&self) -> u32 { 1000 }
    fn activate(&self, ctx: &ReducerContext, x: f32, y: f32, z: f32, _: f32, _: f32, _: f32) {
        let mut worlds = PHYSICS_CONTEXTS.lock().unwrap();
        if let Some(col) = worlds.values_mut().flat_map(|w| w.colliders.get_mut(self.collider_handle)).next() {
            col.set_position(Isometry::translation(x,y,z));
            col.set_collision_groups(self.active_groups);
        }
        register_owner(self.collider_handle, ctx.sender);
    }
}

/// Ray‐cast skill
struct RaySkill { id: SkillId }
impl SkillBehavior for RaySkill {
    fn id(&self) -> SkillId { self.id }
    fn base_ms(&self) -> u32 { 1000 }
    fn activate(&self, ctx: &ReducerContext, x: f32, y: f32, z: f32, dx: f32, dy: f32, dz: f32) {
        let ray = Ray::new(Point::new(x,y,z), Vector::new(dx,dy,dz));
        for world in PHYSICS_CONTEXTS.lock().unwrap().values() {
            if let Some((col_handle, _toi)) = world.query_pipeline.cast_ray(
                &world.bodies, &world.colliders,
                &ray, f32::MAX, true, QueryFilter::default()
            ) {
                let col = &world.colliders[col_handle];
                let body = &world.bodies[col.parent().unwrap()];
                let target = unpack_id(body.user_data);
                apply_damage(ctx, self.id(), target, 1);
            }
        }
    }
}

// Build all skills once
// Registry of all skills
static SKILL_REGISTRY: Lazy<HashMap<SkillId, Box<dyn SkillBehavior>>> = Lazy::new(|| {
    let mut m: HashMap<SkillId, Box<dyn SkillBehavior>> = HashMap::new();
    // insert SensorSkill and RaySkill instances, e.g.:
    // m.insert(1, Box::new(SensorSkill { … }));
    // m.insert(6, Box::new(RaySkill { id: 6 }));
    m
});

#[reducer]
pub fn use_skill(
    ctx: &ReducerContext,
    skill_id: SkillId,
    x: f32,
    y: f32,
    z: f32,
    dx: f32,
    dy: f32,
    dz: f32,
) -> Result<(), String> {
    let now = ctx.timestamp;
    // Fetch skill behavior & base cooldown
    let behavior = SKILL_REGISTRY.get(&skill_id).ok_or("Unknown skill")?;
    let default_base = behavior.base_ms();

    // 1) find existing cooldown row
    let cd_row_opt = ctx.db.skill_cooldown().iter()
        .find(|r| r.player_id == ctx.sender && r.skill_id == skill_id);

    // 1a) if no previous row, set last_used so that elapsed >= base_ms for immediate cast
    let mut cd = if let Some(row) = &cd_row_opt {
        Cooldown::from_row(row)
    } else {
        let micros_ago = now.to_micros_since_unix_epoch() - (default_base as i64 * 1000);
        let ts = Timestamp::from_micros_since_unix_epoch(micros_ago);
        Cooldown { last_used: ts, base_ms: default_base }
    };

    // 2) apply each buff type once (max magnitude) to cd
    let mut max_per_type = HashMap::<BuffType, f32>::new();
    for buff in ctx.db.player_buffs().iter().filter(|b: &PlayerBuff| b.player_id == ctx.sender && b.expires_at > now) {
        max_per_type.entry(buff.buff_type)
            .and_modify(|m| *m = m.max(buff.magnitude))
            .or_insert(buff.magnitude);
    }
    for (bt, mag) in max_per_type {
        if let Some(bh) = BUFF_REGISTRY.get(&bt) {
            bh.apply(&mut cd, mag);
        }
    }

    // 3) cooldown check
    if !cd.is_ready(now) {
        return Err("Skill on cooldown".into());
    }
    cd.use_now(now);

    // 4) write back updated cooldown
    if let Some(old) = cd_row_opt {
        // Only update last_used_at
        let mut row = old.clone();
        row.last_used_at = now;
        ctx.db.skill_cooldown().id().update(row);
    } else {
        // Insert new row if it didn't exist
        let mut new_row = cd.to_row(ctx.sender, skill_id);
        new_row.base_cooldown = behavior.base_ms();
        ctx.db.skill_cooldown().insert(new_row);
    }

    // 5) dispatch to the proper skill behavior
    behavior.activate(ctx, x, y, z, dx, dy, dz);

    Ok(())
}

/// Apply damage in two phases: batch health update and emit a timed event for clients
pub(crate) fn apply_damage(ctx: &ReducerContext, skill_id: SkillId, target_entity: u32, amount: u32) {
     // 1) accumulate pending damage for batched DB write
     for world in PHYSICS_CONTEXTS.lock().unwrap().values_mut() {
         *world.pending_damage.entry(target_entity).or_insert(0) += amount;
     }

    // 2) emit a DamageEvent with expire_at one second in the future
    if let Some(body_row) = ctx.db.physics_body().entity_id().find(target_entity) {
        let region = body_row.region;
        let target_owner = body_row.owner_id;
        let expire_at = Timestamp::from_micros_since_unix_epoch(
            ctx.timestamp.to_micros_since_unix_epoch() + 1_000_000
        );
        ctx.db.damage_event().insert(DamageEvent {
            event_id: 0,
            source_id: ctx.sender,
            target_id: target_owner,
            skill_id,
            amount,
            expire_at,
            region,
        });
    }
}

// Generic buff management: stacks, magnitude, expiration
/// Apply or stack a buff for a player until expires_at. Returns the buff row ID.
pub(crate) fn apply_buff(
    ctx: &ReducerContext,
    player: Identity,
    buff_type: BuffType,
    magnitude: f32,
    expires_at: Timestamp,
) -> u64 {
    let new_id = BUFF_ID.fetch_add(1, Ordering::Relaxed);

    if buff_type == 4 { // Stacking buff type
        if let Some(mut existing) = ctx
            .db
            .player_buffs()
            .iter()
            .find(|b| b.player_id == player && b.buff_type == buff_type)
        {
            existing.stacks = existing.stacks.saturating_add(1);
            existing.expires_at = expires_at;
            ctx.db.player_buffs().id().update(existing.clone());
            return existing.id;
        } else {
            // For buff_type 4, if no existing entry is found, insert new record.
            ctx.db.player_buffs().insert(PlayerBuff {
                id: new_id,
                player_id: player,
                stacks: 1,
                buff_type,
                magnitude,
                expires_at,
            });
            return new_id;
        }
    }

    // For other buff types, always insert a new record.
    ctx.db.player_buffs().insert(PlayerBuff {
        id: new_id,
        player_id: player,
        stacks: 1,
        buff_type,
        magnitude,
        expires_at,
    });
    new_id
}