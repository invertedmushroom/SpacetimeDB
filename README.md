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

> spawn Sphere(1) 1
Object spawned at (50, 1, 50)
> bodies
Physics Bodies (chunk_entities view):
-------------------------------------
ID| Shape             | Position (x,y,z) | Body Type
--|-------------------|------------------|----------
6 | Sphere(1)         | (50.0, 48.6, 1.7) 1
5 | Sphere(0.5)       | (50.0, 50.0, 0.0) 2
6 | Sphere(1)         | (50.0, 47.4, 2.5) 1
6 | Sphere(1)         | (50.0, 46.1, 3.1) 1
6 | Sphere(1)         | (50.0, 43.3, 4.1) 1
6 | Sphere(1)         | (50.0, 41.0, 4.8) 1

5 | Sphere(0.5)       | (50.0, 50.0, 0.0) 2
> contacts
Active Contact Durations:
------------------------
ID  | Entity1                | Entity2                | Duration (ms)
----|------------------------|------------------------|-------------
  3 | 00000004                 | 00000006                 |     600.6
  1 | 00000002                 | 00000003                 |    1500.1
  2 | 00000005                 | 00000006                 |     600.6
  4 | 00000001                 | 00000006                 |    1000.1

> n
Nearby Items (within 30 units):
-----------------------------
  [1] Health Potion at (65.0, 65.0) - 21.2 units away