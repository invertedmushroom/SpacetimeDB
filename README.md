### 1. Start the Local SpacetimeDB Server

```powershell
# Enable info-level logs from the module
$env:SPACETIME_LOG_LEVEL = "info"

docker run --rm --pull always -p 3000:3000 -e SPACETIME_LOG_LEVEL=info clockworklabs/spacetime start

docker run --rm -d --pull always -p 3000:3000 -e clockworklabs/spacetime start
```

### 2. Create and Configure the Server Module

1. Define your game entities as tables in `lib.rs`:
   ```rust
   #[spacetimedb::table(name = "player", public)]
   pub struct Player {
       #[primary_key] //declare the single-column key that SpacetimeDB will use for fast lookups
       pub player_id: Identity,
       pub username: String,
       pub position_x: f32,
       pub position_y: f32,
       // Additional fields...
   }
   ```

2. Define game logic with reducers:
   ```rust
   #[spacetimedb::reducer]
   pub fn move_player(ctx: &ReducerContext, new_x: f32, new_y: f32) -> Result<(), String> {
       // Logic to move the player
   }
   ```

3. Add lifecycle reducers for important events:
   ```rust
   #[spacetimedb::reducer(client_connected)]
   pub fn on_client_connected(ctx: &ReducerContext) -> Result<(), String> {
       // Logic to handle new player connections
   }
   ```

### 3. Build and Publish the Module

On Windows use `spacetimedb-cli`

```powershell
cd my_spacetime_module
# build the wasm module
spacetimedb-cli build
# publish
spacetimedb-cli publish --server http://localhost:3000 --anonymous mydatabase    # spacetime publish
# for logs command to work don't pub as anon
spacetimedb-cli publish --server http://localhost:3000 mydatabase    # spacetime publish ...
spacetimedb-cli logs mydatabase    # spacetime logs mydatabase
# regenerate client bindings
spacetimedb-cli generate --lang rust --out-dir ../my_spacetime_client/src/module_bindings --project-path .    # spacetime generate ...
# delete database
spacetimedb-cli delete mydatabase --server http://localhost:3000
```

### 4. Create the Client Application

1. Generate client bindings from your module schema if not generated already:
   ```powershell
   cd my_spacetime_client
   spacetimedb-cli generate --lang rust --out-dir ./src/module_bindings --project-path ../
   my_spacetime_module
   cargo run
   ```

2. Establish a connection to SpacetimeDB:
   ```rust
   let conn = DbConnection::builder()
       .with_uri("http://localhost:3000")
       .with_module_name("mydatabase")
       .build()
       .expect("Failed to connect");
   ```

3. Set up subscriptions and event handlers:
   ```rust
   let _sub = conn.subscription_builder()
       .subscribe(["SELECT * FROM player", "SELECT * FROM game_item"]);
   
   let _player_updates = conn.db.player().on_update(|_ctx, old, new| {
       println!("Player moved from ({}, {}) to ({}, {})",
                old.position_x, old.position_y, 
                new.position_x, new.position_y);
   });
   ```

4. Implement game features using reducer calls:
   ```rust
   // Move player
   conn.reducers.move_player(100.0, 100.0)
       .map_err(|e| format!("Failed to move: {}", e))?;
   
   // Pick up item
   conn.reducers.pickup_item(item_id)
       .map_err(|e| format!("Failed to pick up item: {}", e))?;
   ```

## Subscription Semantics

SpacetimeDB drives real-time client caches via incremental subscription messages over a WebSocket. Key guarantees:

- **Sequential Responses**: Replies to client actions always arrive in request order.
- **Atomic Transactions**: Each committed reducer or DB transaction emits at most one update message.
- **Snapshot Initialization**: On `subscribe`, the client receives exactly one batch of all matching rows from a consistent snapshot.
- **Incremental Updates**: Subsequent committed transactions send `TransactionUpdate` messages reflecting row insertions/deletions.
- **Client Cache Consistency**: Updates are applied atomically; callbacks (`on_insert`, `on_update`, `on_delete`) only fire after the cache is fully updated.
- **Multiple Subscriptions**: Updates for all active subscriptions are coalesced into each transaction message.

Client SDKs must drive message processing—e.g., Rust uses `run_threaded()` or `frame_tick()`, C# uses `FrameTick()`. Without processing calls, messages queue on the server and are not delivered.