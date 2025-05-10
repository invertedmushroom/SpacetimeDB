SpacetimeDB + Rapier

### 1. Start the Local SpacetimeDB Server

```powershell
# Enable info-level logs from the module
docker run --rm --pull always -p 3000:3000 -e clockworklabs/spacetime start

docker run --rm -d --pull always -p 3000:3000 -e clockworklabs/spacetime start
```

### 2. Build and Publish the Module

```powershell

cd my_spacetime_module

# build the wasm module
spacetimedb-cli build
# publish
spacetimedb-cli publish --server http://localhost:3000 --anonymous mydatabase    # spacetime publish

# for logs command to work don't pub as anon
spacetimedb-cli publish --server http://localhost:3000 mydatabase
spacetimedb-cli logs mydatabase
# regenerate client bindings
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