# MiniCraft

A native Rust voxel renderer. Voxels are 1/16 metre across; a metre-sized block contains 16 × 16 × 16 voxels. Version 0.15.0 streams an infinite horizontal world with biome-driven surface materials across the full 384 metre vertical range. The previous village scene remains available with `--scene village`. There is no AI or car simulation in this version.

## System dependencies

Install a Rust toolchain with Cargo. A GPU driver with Vulkan, DirectX 12, or OpenGL support is required; software Vulkan can run headless checks.

For Linux development on Debian/Ubuntu:

```sh
sudo apt-get install libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev libx11-dev libxi-dev libxrandr-dev libxcursor-dev libvulkan1 mesa-vulkan-drivers
```

The Windows executable needs no separate asset files or installer. The cross-build toolchain is described below.

## Run

```sh
cargo run --release --locked
```

The title screen has a numeric seed field. Leave it blank for a random seed, or enter a signed 64-bit integer. Ctrl+A replaces the current seed; Ctrl+V pastes on Windows. Create world starts generation. An explicit `--seed` pre-fills the field; headless commands retain seed 1 by default. Choose Play or press Enter after loading to capture the mouse. WASD moves, mouse looks, Space jumps, Shift/Q slows walking, Ctrl sprints, and the wheel adjusts speed. F toggles flight; Space flies up and Shift/Q flies down while flying. E opens or closes a targeted door. Esc opens the world menu and releases the cursor. F1 opens Controls, F3 toggles debug info, R returns to a clear spawn position, H hides the playing HUD, G toggles voxel boundaries, O toggles ambient occlusion, -/= move time backward/forward, V toggles VSync, and F11 toggles fullscreen. Walking has gravity. Both walking and flight collide with the fine voxel geometry. F6 toggles night vision, which raises ambient illumination for exploration while preserving material colors, AO and face shading. It does not change game time or bake light. F5 toggles spectator mode, which uses the flight controls but passes through terrain and world bounds without gravity. It remembers the previous walking/flight mode and preserves the visible position when toggled. Returning requires a clear player-sized space inside the world.

After capturing the mouse, left-click breaks one voxel, right-click places one voxel against the targeted face, and middle-click or # picks a material. Keys 1-0 select Stone, Grass, Dirt, Wood, Leaves, Roof, Plaster, Sand, Asphalt, and Lamp. B selects Glass and N selects a Handle. Tab opens the material picker; choose any of its 78 materials to return to the world with it selected. Tab also closes the picker. A depth-tested mint outline highlights the target within eight metres. Placement cannot overlap the player body at its simulated or interpolated position, or leave world bounds. Edits and doors survive unloading and revisiting an area in RAM. They reset when you create another world or exit; there are no disk saves.

The playing HUD contains a compact bottom hotbar with isometric material previews, a crosshair, a small clock, and a selected material label. The dark Escape menu has clickable resume, settings, controls, and quit buttons. Settings control AO, VSync, voxel outlines, night vision, and time of day. Distance & fog opens separate controls for render distance, fog on/off, fog start and fog strength. The world menu has a Title screen button for choosing another seed. Creating another world discards the old session; old-world cleanup runs on the loading worker. Use local time makes the sun, moon, and HUD clock follow the computer clock in its local timezone. It is off by default and applies for the current session. Disabling it keeps the current game time; using -/= or the time buttons also switches back to manual control. Menus scale with the window and high-DPI displays. H hides the HUD for screenshots; Esc restores the menu.

F3 shows FPS, frame interval, resident voxels, visible triangles/chunks, coordinates, the current source biome, the hovered material and target data, and the 20 TPS tick count/grounded state. FPS includes display synchronization when VSync is enabled. Generation, meshing and light baking continue on a background worker during play. The HUD shows Loading terrain while work is pending. F3 reports when the cache is prioritizing the current view under memory pressure.

## Startup

The desktop window opens to the title screen before world generation. Graphics and the small UI prepare in the background; terrain work waits for Create world. A compact dark loading screen shows the current stage, completed/total pages for terrain and meshing, and elapsed time. It redraws at up to 30 FPS during loading. Graphics-device creation, noise generation, meshing, initial lighting, shader compilation and GPU uploads run on a background startup thread. The loading screen and finished renderer share one graphics device; terrain is never built on the window event loop.

The window remains resizable and can be minimized or closed during loading. Escape cancels startup. Cancellation stops noise workers and meshing between pages, drops queued results and never joins the startup thread on the UI path. CPU generation still uses bounded worker/result queues. Progress uses one shared snapshot instead of queuing an event for every page. A worker failure follows the normal startup-error reporting path rather than leaving the progress screen running indefinitely.

After loading, the normal world menu appears. Physics remains paused until Play, and its interpolation clock starts fresh. Headless screenshot commands wait for generation, lighting and the nearby mesh cache so their captures remain deterministic. Only the spawn area and its 2 metre meshing border are prepared before play. Terrain materials, meshes and lighting continue streaming during play.

## Terrain generation

The default scene ports the terrain density and extends the aquifer decisions from [Paper-Minecraft PR #1](https://github.com/CowgillAlex/Paper-Minecraft/pull/1), pinned to `d681c0532d5e2628c70e6de7fad7df31f64ea705`. It generates stone, water and air before applying the surface rules. Biome-driven surfaces follow the raw density/aquifer pass. A deterministic feature stage then adds biome vegetation, small mineral inclusions and occasional terrain-fitted cottages. Existing editing materials and renderer effects remain available.

One reference block is one metre. Fine voxel `(x,y,z)` samples density at `origin + (x,y,z)/16`, with no rounding to metre coordinates. The bundled density graph, noise parameters, xoroshiro stream, named MD5 seeds, octave order, spline float rounding, cave thresholds, sea level and aquifer pressure rules come from the Java implementation. Its interpolated density cells become 4 × 8 × 4 metre cells with trilinear interpolation. Aquifers now use full 3D cells with seed names that include X/Y/Z. The nearest fluid regions, floodedness, level spread, preliminary surface columns and pressure barriers all use Z. Preliminary aquifer heights use the original exact columns and one-metre positive-density scan. Horizontal density terms are evaluated once per column, and conservative bounds skip empty height ranges without changing the result. Pressure uses the fine voxel centre, with a one-voxel stone seal at wet/dry cell boundaries that negative barrier noise cannot puncture. The original unimplemented lava band remains air.

The generator uses the reference vertical range of -64 to 320 metres. The default source origin is `120,-64,-20`. The title screen chooses its seed; headless diagnostics use seed 1 unless overridden. The player spawns above an exact source terrain scan. Signed horizontal page coordinates extend the same voxel field beyond the original preview edges. F3 reports reference-world coordinates. Physics and rendering retain their existing local coordinate offset.

`--finite` restores the old 40 × 384 × 40 metre column for diagnostics. Infinite worlds retain a finite vertical range. There are no disk saves.

```sh
cargo run --release --locked
cargo run --release --locked -- --height 12
cargo run --release --locked -- --seed 1 --origin 120,58,-20
cargo run --release --locked -- --seed 12345 --origin 0,-32,0
cargo run --release --locked -- --noise-sample 120.0625,63,0
cargo run --release --locked -- --noise-benchmark
cargo run --release --locked -- --worldgen-benchmark
cargo run --release --locked -- --worldgen-benchmark --height 12 --reference-worldgen
cargo run --release --locked -- --screenshot noise.png --time 12 --frames 3
cargo run --release --locked -- --scene village
```

`--seed` accepts a signed 64-bit integer. `--height` accepts an even metre height from 2 to 384. `--origin` is the lower window corner in integer metre coordinates; the requested height must fit within -64 to 320. An explicit origin without a height retains the old 12 metre preview behavior. `--height 12` without an origin selects `120,58,-20`. `--noise-sample` accepts fractional metre coordinates and prints final density and material ID: air 0, stone 3, water 10. `--noise-benchmark` fills a single 32³-voxel page at the selected origin without starting the GPU. Use `--origin` to measure different terrain regions. `--worldgen-benchmark` automatically selects finite mode and generates the entire selected 40 metre wide volume without a GPU and reports elapsed generation time, occupancy and voxel payload bytes. `--reference-worldgen` disables page skipping and the prepared density evaluator for comparison.

Finite-mode generation uses a bounded CPU worker pool with at most eight workers and leaves two logical CPUs available when possible. Workers share an immutable graph and own their caches. Each job fills a 2 metre page at full voxel resolution; a bounded result channel limits queued page memory. Generation finishes before play. Worker-local caches reuse density interpolation corners across pages. Conservative density and aquifer bounds skip provably uniform pages; uncertain regions use the original fine sampling. A prepared evaluator reads constants from the bundled graph and evaluates its five interpolated fields directly. Other graph shapes and pages crossing interpolation cells fall back to the generic evaluator. No extra approximation, noise substitution, or coarse upscaling is applied to the resulting voxel field.

For the v0.8 finite-world benchmark on this development host with two generation workers, the same 40 × 12 × 40 metre seed-1 preview fell from 19.73 seconds to 0.55 seconds of generation, with the same 32,809,254 occupied voxels. The full-height column generated in 23.58 seconds, with 672,117,938 occupied voxels and 221.58 MiB of voxel payload. It evaluated 425,361,408 fine samples and skipped 63,819 uniform pages. These numbers exclude meshing, light data, GPU buffers and other memory. A full software-Vulkan capture peaked around 1.5 GiB of process memory. Startup still takes longer than generation alone.

Density parity tests retain all 5,496 samples from the original Java implementation and its fractional 3D interpolation adapter, checked within `1e-12`. Fluid selection intentionally changes from the old 2D aquifers. Another 3,564 samples from the Java code extended to 3D check materials exactly, in both forward and reverse sampling order. Fixtures cover six seeds including signed extremes, negative coordinates, cave depths, sea level and interpolation boundaries. Optimized page fills are compared byte-for-byte with reference voxel fills across representative ocean, cave, lava-boundary, sky and misaligned pages. Prepared fields also match the Java density fixture within `1e-12`. Focused tests cover ocean filling, dry underground regions, pressure barriers and XYZ seed/cache behavior. To regenerate them with a JDK and Gson installed:

```sh
python3 scripts/worldgen-reference.py /path/to/Paper-Minecraft
```

Source details and the adapter changes are recorded in [the worldgen asset notes](assets/worldgen/README.md).


### Biomes and surface rules

The surface pass uses the pinned Paper-Minecraft biome parameter table and JSON surface rules. It adds 24 materials, including gravel, sandstone, red sand, deepslate, bedrock, calcite, mud, podzol, mycelium, snow, ice, terracotta bands, sulfur and cinnabar. Grass, podzol and mycelium have earthy side faces. Vegetation, mineral inclusions and cottages are added by the feature pass described below.

Six original climate channels select biomes on the source's four-metre grid. Fine terrain still samples every 6.25 cm; surface depths, altitude conditions, seabeds and bedrock/deepslate transitions retain metre units. Column context is derived from raw density/aquifers across page boundaries, independent of which neighbors are loaded. Surface decoration runs on the existing sampling workers before meshing and lighting, with bounded column caches. Uniform results compact back to one material byte. Unloaded lighting queries account for surface rules that replace stone with air or water; the other new materials currently behave as opaque solid blocks, including ice and powder snow.

`--noise-only` disables decoration for the previous stone/water/air view and diagnostic comparisons. `--noise-sample` and `--noise-benchmark` continue measuring raw density/aquifers. Normal worlds and `--worldgen-benchmark` include surfaces unless `--noise-only` is supplied. Surface tests compare 1,575 synthetic columns, 15 terracotta band sequences and 3,584 biome selections against the pinned Java implementation, plus streamed-page order and optical sampling checks. Rebuild those fixtures with:

```sh
python3 scripts/surface-assets.py /path/to/Paper-Minecraft
python3 scripts/surface-reference.py /path/to/Paper-Minecraft
cargo test --release --locked surface_streaming_benchmark -- --ignored --nocapture
```

## Renderer

- Sparse allocation of 32³-voxel chunks. A uniform chunk stores one material value; a mixed chunk uses one byte per voxel. Editing a uniform chunk expands it. Dense chunks use copy-on-write storage so lighting and mesh workers can read a consistent snapshot.
- Bitwise visibility masks reject hidden faces before AO and greedy merging. Material and AO merge rules are unchanged. Enclosed uniform water and glass pages are skipped alongside solid pages.
- GPU vertices use 28 bytes instead of 64. Positions retain full float precision; normals, palette colors, AO and UVs use normalized integer formats. Doors retain float geometry for transforms; streamed static meshes are packed on the worker and cached in their GPU format.
- Infinite worlds use signed 2 metre compressed pages and 8 metre work regions. One background actor prioritizes the nearby body, view direction and travel direction. It coalesces requests and cancels obsolete work. A persistent pool of up to four sampling workers leaves two logical CPUs available when possible, and sends results through bounded queues. Generation, meshing and light baking stay off the window event loop. Cached meshes carry their LOD, so approaching a region upgrades its detail and moving away can reduce it. GPU uploads have a 2 ms / 8 MiB per-frame budget; a completed region swaps in together. A swept collision corridor extends 8 metres along travel and publishes its fine pages before visual work, so movement need not wait for lighting. Opaque draw order is cached between page movement or geometry changes.
- Generated voxel data and estimated page metadata have a 512 MiB cache budget. GPU meshes and light textures have a separate 512 MiB budget. Nearby collision and lighting data are protected, so these are soft working-set limits rather than a process memory cap. Mesh admission uses the current view and travel direction: higher-priority regions can evict lower-priority ones even when both remain inside render distance. Turning no longer leaves useful cave regions permanently blocked by older cached meshes. The worker also caches up to 256 MiB of CPU meshes and 768 light bricks for return trips. Player edits and door objects are retained separately in RAM. Walking and flight stop at unloaded pages; spectator can move ahead of generation. Reset waits for spawn data before resuming physics.
- Per-chunk GPU buffers are sorted front to back and frustum culled. The default render distance is 24 metres, with a 4 metre loading margin. Distance & fog offers 12, 16, 24, 32, 48, 64, 96, 128, 192 and 256 metres. Changing distance requests or evicts regions without requiring movement or restart. Distance culling is independent of fog. Fully buried chunks need no mesh. Finite previews retain their existing mesh worker; the short finite preview and village use shared buffers with per-chunk edit replacements. Edits remesh touched chunks and neighbors whose faces or corner AO may change.
- A cached 4096² directional shadow map with hardware-filtered comparison sampling. A conservative depth bias bounds the full filter footprint using geometric face normals, avoiding triangle-shaped self-shadow bands. The per-frame shadow lookup uses one filtered comparison rather than nine manual taps. In infinite worlds its projection follows the camera in snapped 8 metre steps. The map is reused until the projection, time, geometry or door motion invalidates it. Mesh-stream arrivals coalesce shadow refreshes to at most eight per second; edits and time/door changes still refresh immediately. Fully sky-occluded full-height surfaces skip sun-shadow sampling, and dark water skips sky reflection work.
- Bilinear voxel-corner ambient occlusion, plus half-resolution screen-space ambient occlusion with a 0.55 metre radius and depth/normal-aware upsampling. AO darkens the ambient lighting contribution; emissive lamps, direct sunlight, shadowed water highlights, the target outline, and HUD are preserved. Turning AO off skips its sampling pass. Procedural surface variation, animated water highlights, sky, and distance fog complete the scene. Sun and moon discs render only in the depth-tested sky pass, so distant fog cannot draw them over terrain.
- Back-to-front chunk-sorted water and glass rendering after the opaque lighting/AO pass. Submerged opaque faces remain meshed, and water does not cast opaque directional shadows. Water reads the opaque depth buffer while it is attached read-only, measures the distance to the floor and uses exponential attenuation for depth tinting. Shallow water reveals the bottom; deeper water becomes bluer. Fresnel reflections and animated normals preserve surface highlights. This uses the existing depth texture without an additional scene copy. Glass retains its lightly tinted reflections.
- Linear HDR scene colour, filmic tone mapping, and a quarter-resolution bloom pass for subtle lamp glow.
- Embedded WGSL shaders, a baked DejaVu Sans glyph atlas, and a single batched UI draw. Glyphs use one quad each rather than per-pixel geometry. Fonts, icons, and shaders are embedded; the executable needs no asset directory.

A fixed finite seed-1 cave view at `--finite --height 48 --origin 134,20,-37 --view cave --size 960x600` submitted 1,086,694 triangles in 671 draws before the v0.9 changes, and 851,688 triangles in 479 draws afterward. The same nearby mesh cache fell from 244.2 to 118.6 MiB. Twenty warmed frames took 4.336 seconds before and 3.444 seconds afterward on software Vulkan. This measures local rendering cost, not expected Intel/Windows FPS. Dense cave geometry still needs substantial GPU work; no occlusion culling or terrain LOD is implemented.

Cave fog and the sky backdrop fade to black according to the existing skylight field. Surface fog retains the sky color, with a smooth transition near entrances. Night vision brightens cave geometry without turning its fog blue. No extra light bake or CPU column scan is needed.

Fog can be disabled completely. Fog strength ranges from 25% to 200%, with 100% as the default. Fog start adjusts from the camera to 95% of render distance and scales when render distance changes. The default starts at 20 metres for a 24 metre render distance. Restore defaults returns all distance/fog controls together. Adaptive terrain detail is enabled by default in infinite worlds and can be disabled in the same menu. The village and finite diagnostic scenes keep their existing full meshes. These controls apply to both noise and village scenes. Larger distances increase generation, lighting and draw work. The caches may fill before the selected distance is reached in dense caves. Fog controls change shader parameters without rebuilding terrain or lighting.

This is a rasterized mesh renderer with an editable material palette. The horizontal source coordinate limit is ±30 million metres, matching the existing source guard. Rendering and physics still use floats; a floating origin and terrain LOD remain future work. There are no disk saves, AI, or car simulation.

## Distant detail

Infinite terrain has three render mesh levels. Nearby regions remain exact. Medium detail begins when the whole 8 metre region is roughly 24 metres away and permits at most half a voxel of simplification error; far detail begins around 56 metres and permits one voxel. Four metres of hysteresis prevents repeated switches near a boundary. Region uploads swap together. Page-edge vertices and material interfaces stay fixed, preserving joins between different levels. Caves, arches and overhangs keep their 3D topology. Transparent water/glass geometry and door objects are not simplified.

Meshoptimizer simplifies the opaque geometry on the terrain worker. Greedy-quad T-junctions are made conforming first, then normals, colors and corner AO contribute to the simplification error. Simplified faces use their geometric normal for lighting and shadow bias. Per-voxel color noise and corner AO fade as their footprint becomes smaller than a screen pixel; screen-space AO remains available. This avoids retaining high-contrast subpixel patterns at long distances.

This first LOD pass reduces rendering geometry and mesh memory. It still generates the exact fine voxel data and meshes it before simplification; it does not yet accelerate distant density sampling. Editing, picking, collision and light transport retain the source voxel field. Settings > Distance & fog > Adaptive terrain detail toggles LOD. Headless screenshots accept `--no-lod` for comparisons. The opt-in `lod_region_benchmark` reports geometry and memory for surface terrain, the watery seed-1 cave and a seed -67 cave. A rendered arch check compares full and reduced images and verifies that the opening and material colors remain visible. The fixed single-region survey produced these GPU mesh + light payloads; fine voxel storage is separate:

| Region | Full | Medium | Far |
| --- | ---: | ---: | ---: |
| Surface | 0.88 MiB | 0.50 MiB | 0.30 MiB |
| Watery cave | 1.22 MiB | 0.56 MiB | 0.32 MiB |
| Seed -67 cave | 1.72 MiB | 0.79 MiB | 0.38 MiB |

The seed -67 region's submitted geometry fell from 25,350 to 6,464 triangles at far detail. These are representation sizes, not a measurement of desktop FPS. The first benchmark pass includes generation/lighting while subsequent levels reuse those caches, so their elapsed times are not directly comparable.

## Vegetation, structures and minerals

All 56 bundled biomes have an explicit feature profile. Forests use tapered bark/heartwood trunks, flared roots, branching limbs, fine shoots and leaf sprays. Birch, cherry, conifer, snowy conifer, acacia-like savanna trees, pale hanging growth, bamboo and wetland stilt roots vary the silhouettes. Meadows have grass tufts, actual seed kernels, jointed wheat stalks with grain heads, flowers with separate stem/petal/pollen parts, and sunflowers. Other treatments include ferns, mushrooms, dune grass, cacti, alpine lichen, cave moss, hanging vines, dripstone, sculk-like growth and sulfur crystals. Rivers have reeds and submerged plants; oceans have kelp/seagrass, with branching coral in warm oceans. Individual fine voxels remain editable. Plants currently generate mature; growth, harvesting inventories and animal simulation are not included.

Feature plans depend on seed and stable world-space anchors. Bounded worker caches retain recipes and sparse page overlays. Rasterization clips each primitive to the requested page, including pages that were empty after the terrain stage. Generation order and cache eviction do not change a feature. Point and skylight queries use the same plans for unloaded plants, buildings and lamps. Soft plant parts are non-solid and filter propagated light; wood and mineral inclusions stay solid. Existing edit overrides apply after feature generation.

Occasional cottages require dry, gently sloped sites, with foundations, hollow rooms, timber, glazed windows, warm lamps and wheat plots. Generated door panels stay editable terrain until the first E interaction recognizes them through the existing frame/handle system. They then use the normal movable door, damage and repair paths. Generated lamp sources are found even before their voxel pages load; player removals mask the source.

`assets/worldgen/ores.json` copies 19 Overworld ore placement passes and their biome memberships from the pinned source. Coal, iron, copper, gold, redstone, lapis, diamond and emerald keep the source attempt counts per 16 × 16 metre column, uniform/triangular height laws, rarity filters and air-exposure reduction. Out-of-world height samples are discarded rather than resampled. Extra badlands gold, mountain emeralds and larger dripstone copper deposits retain their restrictions. Vein shapes and random streams are MiniCraft adaptations: scattered 6.25 cm mineral inclusions replace stone/deepslate within clustered host regions, leaving host rock between the pieces. They are geometry, not ore textures or metre-wide solid ore blocks. No base density or historical density reference changes are involved.

Use `--no-features` to retain biome surfaces without vegetation, buildings or ores. `--noise-only` disables both stages. Controlled reference plots can be opened with `--showcase BIOME`, `--showcase tree`, `--showcase plants`, `--showcase cabin` or `--showcase ores`. These use production shape recipes on a prepared plot, not a promise of natural feature density at that location.

```sh
cargo run --release -- --showcase cherry_grove
cargo run --release -- --showcase plants --screenshot plants.png --view feature-close --ui clean --time 10.5 --no-fog
python3 scripts/capture-features.py --output /tmp/minicraft-gallery
python3 scripts/feature-assets.py /path/to/Paper-Minecraft
```

The capture script produces all 56 biome previews, plant/tree/cottage views and individual cut-open mineral specimens. `--ui clean` hides the HUD for captures. The JSON manifest records each scene, camera and time. Cave previews use night vision. They complement procedural-world captures and CPU checks of generation order, optical parity, original ore height distributions and working cottage doors.

## Editable doors

Cabins have full 1 × 2 metre entrances and separate hinged voxel panels. Aim at any part of a working door and press E to open or close it. Mouse buttons still break/place individual voxels, including while a door is open. Glass, lamps, holes, and replacement materials remain part of the same moving object.

For a custom door, build a wooden rectangle 16 voxels wide and 32 voxels high, two voxels deep. Its outermost perimeter must be intact wood on both depth layers. The interior can contain any material or air. Select Handle with N and place it on the front face halfway up either vertical side, at row 15 or 16 counting from the bottom. The handle can sit on either of the two outer side columns; the hinge is assigned to the opposite side. Recognition supports all four horizontal facings. Finishing a frame after adding its handle also triggers recognition. The first version uses this standard size, not arbitrary-sized frames.

Breaking the outline or removing all attached handles disables operation. Repairing them restores it. Damaged panels remain hanging in their current open/closed pose and keep their edits. Frames and handles are checked on the door's own grid, so rotating it never changes its structural rules. Empty parts of a claimed panel can be filled again; voxels beyond its fixed bounds belong to the static world.

Each panel has a small copy-on-write voxel grid and a mesh rebuilt only after voxel edits. A 0.2 second smooth swing transforms its vertices without remeshing terrain every frame. Picking and target outlines follow the visible swing. Physics and propagated light use the axis-aligned open/closed states; closing or opening is rejected if the destination intersects a player or world geometry. This is not continuous rotating-body physics. Directional shadows follow the visual mesh. Lighting updates both old and new occupied cells, including moved lamp sources, through the regional worker in infinite worlds and the incremental worker in the village. Door edits and states last for the current session.

## Player physics

The player uses a 0.6 × 1.8 metre AABB with a 1.62 metre eye height. A swept broad phase gathers only intersecting fine voxels. Motion clips against each voxel on Y, X, and Z, preserving wall sliding and preventing fast movement from skipping a one-voxel wall. Glass is solid; water and flowers are non-solid. Grounded movement can step up 0.5 metres when headroom permits. The bottom and unloaded terrain are solid outside spectator mode. Finite scenes also retain their horizontal boundary walls. Placement checks the whole body, and resetting searches for a clear spawn if it has been edited.

Physics runs at exactly 20 fixed steps per simulated second. The accumulator uses real frame elapsed time, with at most 250 ms of catch-up after a stall. Rendering blends the previous and current completed tick using the remaining accumulator fraction. Previous state changes only on a tick, including when a frame needs several ticks. Positions have the standard one-tick interpolation delay, while mouse orientation updates immediately at render frequency. Menus freeze simulation; resuming does not accumulate paused time. Teleports discard old interpolation state, and flight/spectator toggles preserve the rendered position. Short jump taps are queued until the next tick.

Walking uses acceleration, gravity of 24 m/s², a 48 m/s terminal fall speed, and an 8.4 m/s jump impulse. Ctrl sprints, Shift slows walking, and F toggles flight with collisions. Space/Shift moves vertically in flight and Ctrl boosts it. Water currently has no swimming/buoyancy simulation. Collision boxes are resolved against individual 6.25 cm voxels; they are not snapped to metre-sized blocks.

Alex's [Paper-Minecraft AABB](https://github.com/CowgillAlex/Paper-Minecraft/blob/main/core/src/main/java/dev/alexco/minecraft/phys/AABB.java) informed the swept-box/per-axis clipping approach. The full reference repository is cloned at `/home/alexc/.t3/projects/Paper-Minecraft` for future work. MiniCraft's collision implementation is three-dimensional and its fixed-step interpolation is separate from that reference.

## Streamed lighting

Each 8 metre region bakes a quarter-metre light field with an 8 metre halo, then uploads a 34³ RG8 light brick with a filtering border. The halo no longer requires full fine voxel pages. Compressed 2 metre transport tiles sample only the fine rays connecting light nodes. They retain endpoint attenuation, thin-wall blocking, glass and water behavior. Neighboring regions reuse those tiles. Pure source intervals skip exact uniform interiors, and persistent workers prepare missing transport tiles and sky columns in parallel. Transport tiles are capped at 16,384 entries, and sky columns at 131,072.

Source-based sky scans account for terrain above resident pages, so unloading a roof cannot turn a cave into daylight. Conservative classifications of 4 × 8 × 4 metre interpolation cells are shared across columns; uncertain spans still sample at fine resolution. Mixed terrain pages also classify uniform 1 metre and half-metre subvolumes before fine filling. These are exact skips, not LOD or changed noise.

Local edits use a separate worker with eight cached light graphs prepared by nearby terrain builds. It repairs changed connections, skylight columns and emitter paths, then uploads the affected regional light bricks while terrain generation continues independently. Repeated edits are coalesced with all dirty cells retained. Revision checks keep older work from replacing newer light. If an edit lies outside the cached graphs, the local worker prepares its target region first. Terrain remeshing invalidates only regions touching the changed voxels; sky changes invalidate lighting below the edited roof without discarding unrelated meshes.

## Lighting

Street lamps and ceiling lamps emit warm white light. Window blocks are translucent glass rather than emitters, so interiors receive actual lamp light and cool skylight. Glass is selectable with B. - and = adjust the clock in either direction; holding a key repeats the adjustment. The HUD shows game time. Enable Use local time in Settings to follow the computer clock, including while menus are open. The OS clock is sampled at most once per second; only changed times invalidate the sun shadow map, and the diffuse light volume is reused. Sunset and night around 18:30-21:00 emphasize the contrast between warm lamps and the cool sky.

Two light channels are cached on a 0.25 metre grid: lamp light and skylight. Voxel-accurate segment checks connect neighboring light cells, so even thin opaque walls stop transport. Glass adds a small attenuation instead of blocking light. A brightest-first flood fill spreads light around corners. The GPU reads an interpolated RG8 3D texture, with no loop over lamps per pixel. Skylight is a diffuse sky-access approximation independent of solar direction; directional sunlight still uses the sun shadow map. Lamp light is diffuse propagated illumination, not per-lamp ray-traced shadows.

Finite scenes use a light volume spanning the actual world height. Their initial bake runs at startup. A background worker retains the connectivity graph and source values. Edits recheck only nearby connections, affected sky columns, and nearby lamp seeds. A local removal/refill flood repairs illumination, including closing windows or removing sources. Only the affected texture subvolume is uploaded. Queued edits are coalesced without losing their dirty cells, and completed patches appear in order while editing continues. Cheap copy-on-write snapshots keep geometry responsive; an Updating light indicator reports pending work. New time-of-day values update shader parameters and invalidate sun shadows without rebuilding the diffuse light field. The sun/moon, sky, fog and exposure change with the clock. Glass sorting is per chunk; this is intended for clear window panes rather than arbitrary overlapping coloured transparent volumes.

Water is translucent, with greater attenuation over deeper floors. Skylight attenuation through water is reduced so shallow submerged geometry remains visible. Transparent sorting is per chunk rather than per triangle; overlapping glass and water can still have ordering limitations. Water waves perturb reflection normals instead of adding unlit brightness. Reflections respect the sky-access field and shadow visibility; sun/moon specular highlights use the same directional shadow map. Submerged opaque geometry retains contact AO; roof coverage and the light field keep covered water dark at night.

## Verify

```sh
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo test --release --locked shadow_receivers -- --ignored --nocapture
cargo test --release --locked renderer::water_tests -- --ignored --nocapture
cargo test --release --locked renderer::sky_tests -- --ignored --nocapture
cargo test --release --locked renderer::infinite_render_tests -- --ignored --nocapture
cargo test --release --locked local_lamp_edits -- --nocapture
MINICRAFT_PROFILE_STREAM=1 cargo test --release --locked flight_streaming_benchmark -- --ignored --nocapture
cargo test --release --locked benchmark_lighting_edits -- --ignored --nocapture
cargo run --release --locked -- --scene village --screenshot scene.png --frames 3
cargo run --release --locked -- --finite --height 48 --origin 134,20,-37 --view cave --screenshot cave.png --frames 12 --size 960x600
cargo run --release --locked -- --finite --height 48 --origin 134,20,-37 --view cave --screenshot cave-lit.png --night-vision --render-distance 64 --no-fog
cargo run --release --locked -- --scene village --screenshot distance-settings.png --ui view-settings --size 800x600
cargo run --release --locked -- --finite --height 48 --origin 134,20,-37 --view cave --screenshot cave-solid.png --no-transparency
cargo run --release --locked -- --scene village --screenshot edits.png --verify-edits
cargo run --release --locked -- --scene village --screenshot doors.png --view door --verify-doors
cargo run --release --locked -- --scene village --screenshot door-open.png --view door --door open
cargo run --release --locked -- --scene village --screenshot door-broken.png --view door-side --door broken
cargo run --release --locked -- --scene village --screenshot recess.png --ao-closeup
cargo run --release --locked -- --scene village --screenshot recess-no-ao.png --ao-closeup --no-ao
cargo run --release --locked -- --scene village --screenshot house.png --shadow-view house
cargo run --release --locked -- --scene village --screenshot terrain.png --shadow-view terrain
cargo run --release --locked -- --scene village --screenshot leaves.png --shadow-view leaves
cargo run --release --locked -- --scene village --screenshot night.png --view street --time 21
cargo run --release --locked -- --scene village --screenshot water-night.png --view water --time 3
cargo run --release --locked -- --scene village --screenshot menu.png --ui pause
cargo run --release --locked -- --scene village --screenshot settings.png --ui settings --size 800x600
cargo run --release --locked -- --scene village --screenshot controls.png --ui controls
cargo run --release --locked -- --scene village --screenshot debug.png --ui debug
cargo run --release --locked -- --scene village --screenshot inside.png --view interior --time 21
cargo run --release --locked -- --scene village --screenshot glass.png --view window --time 12
```

Door tests cover structural damage/repair, custom frame recognition, both hinge sides, all four facings, local picking during a swing, blocking closes, walkable cabin entrances, source tracking and incremental lighting equivalence. The --verify-doors capture drives the live interaction, picking, editing, mesh and light paths through opening, breaking and repairing a frame/handle, blocking a close and closing again.

Physics tests cover landing stability, jumping/head collisions, thin-wall sweeps, sliding, steps/headroom, water/glass collision rules, world bounds, placement protection, flight controls, spectator passage through solids and safe exit, and equivalent simulation/interpolation at 10, 30, 60 and 144 FPS. GPU water tests compare open and roof-shaded water at noon and night, and verify that shallow water reveals a changed bottom material more strongly than deep water.

Lighting tests compare incremental edits and partial texture uploads byte-for-byte with fresh full bakes, including lamp removal, overlapping sources, windows, roofs, and coalesced edits. The opt-in lighting benchmark reports initial bake time, edit latency, and uploaded bytes in the generated world.

The opt-in GPU shadow test verifies fully lit, unoccluded surfaces on all three sun-facing axis normals, from near and far camera positions, checks that a real blocker still casts a shadow, and compares thousands of stable house-wall samples with CPU voxel ray casts. It requires a graphics adapter or software Vulkan. The AO close-up command carves a recess using the real voxel editing path. Screen-space AO uses only visible geometry, so hidden or off-screen occluders cannot contribute. The edit verification command breaks and replaces a voxel through the live GPU chunk and shadow paths and checks that the restored mesh has the original triangle count. Headless captures accept `--render-distance METRES`, `--no-fog`, `--no-lod`, and `--ui view-settings` or `--ui materials` for the same controls and pipelines. The GPU distance/fog test checks actual rendered pixels, fog strength, removing distant geometry and restoring it at a stationary camera. Streaming tests also verify that shrinking the range rejects stale results. Infinite-world checks render beyond both old horizontal edges, compare the generated materials with source samples, leave and revisit an edited region, and verify cached eviction retains overrides. Regional light bricks are checked against a full bake across their shared edge. Sparse transport is compared against literal fine-voxel rays, including negative coordinates, edits and unloaded pages. Local lamp, removal, wall and glass edits must match fresh bakes while terrain results remain unconsumed. The collision stream test requires body data before visual results. A GPU sky test checks that fogged terrain occludes the moon disc.

Headless captures warm one frame before timing steady frames. `--night-vision` enables the same brightness override as F6. `--no-transparency` omits water and glass for rendering-cost diagnostics; it does not change generation. The screenshot command uses the same rendering pipelines without creating a window. It requires a working GPU driver or a software Vulkan implementation. It prints the adapter name and total elapsed render time; software rendering is useful for correctness checks, not representative desktop performance.

The fixed CPU streaming benchmark prepares four adjacent 8 metre regions at the surface and four in a watery cave, then revisits their first regions. On this four-logical-CPU development host, the surface sequence changed from 6.39 to 1.49 seconds and the cave sequence from 19.83 to 2.87 seconds. Per-region geometry counts and GPU byte counts match. This measures CPU preparation, not desktop FPS. The cached local-edit test completed lamp placement, removal, stone and glass updates in 2–40 ms while terrain results were pending, and compared their output with fresh bakes.

## Windows builds

On Linux, install Rust's `x86_64-pc-windows-gnu` target and the MinGW x64 GCC, POSIX G++ and binutils packages (`g++-mingw-w64-x86-64-posix` on Debian/Ubuntu). Then run from this directory:

```sh
./scripts/package-windows.sh
```

This produces `../releases/v0.15.0/minicraft-v0.15.0-windows-x64.exe`, a ZIP with controls, and a SHA-256 checksum. The version comes from Cargo.toml and appears in the filename, window title, HUD, and Windows file-version resource. The package script refuses to overwrite an existing release. Bump the package version and release notes for subsequent builds. Builds are unsigned. Windows uses a GUI subsystem, with startup errors shown in a message box and written beside the executable.

The UI atlas and metrics are checked in. To regenerate them on Linux, install Pillow and DejaVu Sans, then run `python3 scripts/bake-ui-font.py`. The font license is in `assets/FONT-LICENSE.txt`, embedded in the app, and included in Windows ZIP packages. `--licenses` prints the font and third-party notices. Meshopt/meshoptimizer notices are embedded and included as `THIRD-PARTY-LICENSES.txt` in Windows ZIP packages.

The checked-in Cargo.lock pins dependencies. Generated binaries and build outputs stay outside version control.
