# Planet Context Streaming & LOD Roadmap

Last updated: 2025-10-29

## Goals
- Seamless zoom from orbit → surface with appropriate geometry/material detail.
- Load and unload planet content per context so memory stays bounded.
- Keep deterministic planet generation (seed-driven) while allowing authored overrides later.

## Current State (2025-10-29)
- Single high-res icosphere with procedural shader; no mesh LOD tiers.
- `terrain_stream.rs` contains an unused tile streaming prototype tied to the old editor camera.
- Dev panel drives planet parameters but respawns the whole entity for every change.
- No infrastructure for async asset loading, tile caches, or context switching.

## Target Architecture

### Context Layers
| Context | Camera Altitude (approx) | Content | Notes |
|---------|--------------------------|---------|-------|
| Orbit | > 150 km | Procedural planet shell (current shader) + cloud/atmosphere | Lightweight, always resident. |
| Approach | 10–150 km | Low-poly quadtree patches (heights + biome masks) | Transition layer; morphs to orbit shell and feeds Surface. |
| Surface | < 10 km | High-res patches with textures, props, gameplay entities | Requires streaming + culling. |
| Local | < 1 km | Cities/bases, units, decals | Tied to gameplay ECS context. |

### Systems
1. **Context Manager**
   - Tracks camera altitude & target planet.
   - Enables/disables context-specific ECS schedules.
   - Handles hand-off (e.g., moving units between contexts).

2. **Planet LOD Controller**
   - Manages quadtree of patch descriptors (`PatchId`, `lod_level`, `bounds`).
   - Requests loads/unloads through Asset Streamer.
   - Provides morph factors for smooth transitions.

3. **Asset Streamer**
   - Async task layer (Bevy `IoTaskPool`) that loads patch bundles (meshes, textures, metadata).
   - Maintains cache with eviction policy (e.g., LRU by distance).
   - Surfaces readiness events to main thread.

4. **Patch Renderer**
   - Converts loaded bundles to `Mesh3d`/material handles.
   - Handles stitching between LOD levels (skirts, morph).
   - Applies biome/material data (textures or uniforms).

5. **Procedural Fallback**
   - If bundle missing, generate procedural patch (current shader) as placeholder.
   - Allows iterative development without full asset bake.

## Implementation Phases

1. **Scaffolding**
   - [ ] Introduce `PlanetContext` resource tracking active layer & thresholds.
   - [ ] Move prototype `PatchGrid`/`terrain_stream` into a dedicated `planet_surface` module.
   - [ ] Add debug overlay (rings + text) showing current context & target LOD.

2. **Streaming Core**
   - [ ] Implement `PlanetPatchDescriptor` (quadtree index, bounding sphere, parent).
   - [ ] Add async loader stub reading simple JSON/ron descriptors from `assets/planet_patches/`.
   - [ ] Hook loader into Bevy asset system or custom resource queue.
   - [ ] Integrate cache + eviction metrics.

3. **Renderer Integration**
   - [ ] Replace `spawn_random_planet_inner` surface mesh with orbit shell entity + LOD controller child.
   - [ ] Spawn/unspawn patch entities as streamer reports readiness.
   - [ ] Apply simple geomorph (blend between orbit radius and patch heights).

4. **Gameplay Context**
   - [ ] On entering Surface context, spawn gameplay ECS (units, resources) scoped to active patches.
   - [ ] Persist state when leaving Surface (serialize to patch metadata or in-memory store).

5. **Shader Upgrade (later)**
   - [ ] Add material definitions for streamed patches (albedo/normal/roughness textures).
   - [ ] Maintain color parity with orbit shader for fade transitions.
   - [ ] Optional: integrate procedural decals or masks for variety.

## Immediate Next Steps
1. Create `planet_surface` module with:
   - `lod::` quadtree descriptors
   - `stream::` request/response layer
   - `render::` patch entity manager
2. Adapt `terrain_stream.rs` prototype to operate on camera altitude (not editor camera).
3. Expose debug toggles in dev panel for context thresholds and patch stats.

## Open Questions
- Where do baked patches come from? (Need tooling: offline generator vs. runtime procedural bake.)
- How to blend between procedural orbit shader and textured patches? (Cross-fade, morph).
- Persistence format for gameplay entities tied to patches.

## References
- `Lod_context.txt` (concept narrative).
- `src/game/world/terrain_stream.rs` (prototype streaming logic).
- Bevy docs: AssetLoader traits, `IoTaskPool`, `World::resource_scope`.
