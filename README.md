## SpacetimeDB + Rapier
- A Rapier-based physics pipeline
- O(1) lookup via `id_to_body` map for bodies and colliders
- Sustained contact detection (Start/Continue/End) and centralized contact handling
- Aura-based buff insertion/removal and skill cooldown management|persisted in DB
- Batched database writes (positions, damage, buffs) at each physics tick

## Architecture & Design
- **Physics Tick Pipeline**
  1. Drain collision events from Rapier into Vec<CollisionEvent>  
  2. `process_contacts`: normalize into `PhysicsContact` start/continue/end  
  3. `handle_event`: queue buff inserts/removals, damage and contact events  
  4. `apply_database_updates` in `physics_tick`: batch write positions, damage and buff changes
- **Contact Tracker**  
  - Centralizes collision processing in `contact_tracker.rs`   
  - Uses `ACTIVE_CONTACTS` map for sustained contact detection  
  - Decouples raw geometry events from game logic via `PhysicsContact`
- **Skill & Buff System**  
  - `SkillBehavior` registry defines each skill’s cooldown and activation logic  
  - `BuffBehavior` applies transient cooldown modifiers at cast time only  
  - Cooldowns persist in `skill_cooldown` table; buffs in `player_buffs` table  
  - Buff rows now use a global AtomicU64 (`GLOBAL_BUFF_ID`) for unique IDs; `apply_buff` returns the assigned buff row ID for precise removal
  - Damage events via `apply_damage`: accumulates pending damage per tick and emits timed `damage_event` rows (expire_at = +1s), with actual health updates batched in `apply_database_updates`

## Design Rationale
- **Batch DB Writes**: minimizes overhead by grouping position, damage and buff updates into single transactions per tick
- **O(1) ID Lookups**: `id_to_body` map removes per‐frame linear searches in Rapier sets
- **Contact Events**: unified Start/Continue/End events allow flexible logic (damage over time, auras, cooldown triggers)
- **Indexing**: add composite indexes on `(player_id, buff_type, expires_at)` in `player_buffs` and `(target_id, expire_at)` in `damage_event` to speed up filter and cleanup queries

### 1. Start the Local SpacetimeDB Server

```powershell
# Enable info-level logs from the module
docker run --rm -d --pull always -p 3000:3000 clockworklabs/spacetime start

```

### 2. Build and Publish the Module

```powershell

cd my_spacetime_module

# build the wasm module
spacetimedb-cli build
# publish
spacetimedb-cli publish --server http://localhost:3000 mydatabase
spacetimedb-cli logs mydatabase
# generate client bindings
spacetimedb-cli generate --lang rust --out-dir ../my_spacetime_client/src/module_bindings --project-path .

# delete database
spacetimedb-cli delete mydatabase --server http://localhost:3000
```

### 3. Create the Client Application

Generate client bindings from your module schema if not generated already.
   ```powershell
   cd my_spacetime_client
   spacetimedb-cli generate --lang rust --out-dir ./src/module_bindings --project-path ../my_spacetime_module
   cargo run
   ```