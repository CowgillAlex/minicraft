# Changelog

Windows builds and their downloads are archived in [GitHub Releases](https://github.com/CowgillAlex/minicraft/releases). Historical notes were reconstructed from the saved build READMEs and retained commit history.

Six builds were made between commits, so their exact source snapshots are unavailable: v0.1.0, v0.1.1, v0.1.2, v0.1.3, v0.2.0 and v0.6.1. Their release notes identify the later retained source snapshot used by the tag. The original Windows binaries are preserved unchanged.

## [v0.16.0: Faster terrain streaming](https://github.com/CowgillAlex/minicraft/releases/tag/v0.16.0)

- Give lighting its own exact feature-shape queries, without rasterizing full pages or generating ore deposits.
- Reuse uniform base and resident-page proofs for transport queries, including replacements that preserve attenuation.
- Retain recently used cache entries with bounded second-chance eviction, and share immutable feature plans between workers.
- Specialize surface rules by biome and evaluate provably constant stone spans together, retaining literal evaluation where needed.
- Replace full biome-table scans with an exact range index that preserves original ties and sampling positions.
- Prepare page meshes on the worker pool and cache exact source geometry across LOD changes, with edit invalidation at page borders.

## [v0.15.0: Life at voxel scale](https://github.com/CowgillAlex/minicraft/releases/tag/v0.15.0)

- Add decoration profiles for all 56 biomes, with branching trees, tapered trunks, roots, fine shoots and individual leaf sprays.
- Build grass blades and seed kernels, flower stems/petals/pollen, wheat stalks/grains, bamboo, cacti, fungi and alpine growth from editable voxels.
- Add cave growth, river plants, kelp, seagrass and warm-ocean coral.
- Generate occasional terrain-fitted cottages with warm lamps, glass windows and editable hinged doors.
- Scatter coal, iron, copper, gold, redstone, lapis, diamond and emerald as small host-rock inclusions using the source placement distributions.
- Expand the material picker to 78 materials. Soft plant parts filter skylight and allow walking through them.
- Add controlled biome/plant/tree/cottage/mineral previews and repeatable screenshot tools.
- Move the game crate to the repository root and keep the technical guide under docs.

## [v0.14.1: Aquifer heights and sealed boundaries](https://github.com/CowgillAlex/minicraft/releases/tag/v0.14.1)

- Restore the source’s exact-column, one-metre positive-density scan for preliminary aquifer heights.
- Use fine-voxel pressure centers and thin wet/dry seals to fix leaking fluid boundaries.
- Remove spurious sea-level water volumes caused by incorrect preliminary heights, retaining terrain density and metre scale.

## [v0.14.0: Distant detail and a smarter terrain cache](https://github.com/CowgillAlex/minicraft/releases/tag/v0.14.0)

- Add conservative distant mesh LOD while preserving full nearby detail, editing, collisions, water, glass and doors.
- Filter subpixel color and AO patterns to reduce distant shimmer.
- Add an adaptive-detail toggle and --no-lod for comparisons. Fine density sampling is unchanged.
- Prioritize useful visible regions under cache pressure instead of letting old meshes block new views.
- Package a static Windows C++ runtime and third-party notices.

## [v0.13.1: Useful debug info and dark cave fog](https://github.com/CowgillAlex/minicraft/releases/tag/v0.13.1)

- Show the current biome and targeted block in the F3 debug panel.
- Add # as a touchpad-friendly alternative to middle-click for picking materials.
- Fade cave fog and its backdrop to black using skylight exposure, including with night vision.

## [v0.13.0: Biomes and surface rules](https://github.com/CowgillAlex/minicraft/releases/tag/v0.13.0)

- Port biome selection and surface rules from the source world generator.
- Add grass/dirt, gravel seabeds, sand/sandstone, red sand, terracotta bands, snow, ice, calcite, mud, podzol and mycelium.
- Add sulfur/cinnabar and bedrock/deepslate transitions, retaining metre-scale layer depths and fine voxel shapes.
- Add a Tab material picker for all 39 materials, with decoration on the terrain workers.
- Keep the raw stone/water/air view available with --noise-only.

## [v0.12.0: Faster streaming and immediate local lighting](https://github.com/CowgillAlex/minicraft/releases/tag/v0.12.0)

- Prioritize the view and travel direction on a bounded sampling worker pool.
- Use exact skips within mixed pages and cached transport rays for lighting instead of generating full hidden voxel halos.
- Reject hidden faces with bit masks and pack GPU vertices on workers before upload.
- Load collision data ahead along the travel direction.
- Repair local light edits on a separate worker so they can finish while terrain generation is still pending.

## [v0.11.0: Infinite horizontal worlds](https://github.com/CowgillAlex/minicraft/releases/tag/v0.11.0)

- Stream terrain, meshes and regional lighting as the player explores in any horizontal direction.
- Prepare only the spawn area before play and retain nearby data in bounded caches.
- Preserve edits and doors in RAM after leaving and revisiting an area. Disk persistence is not included.
- Stop walking and flight at unloaded terrain while allowing spectator mode to travel ahead.
- Keep sun and moon discs behind terrain, including at long distances in heavy fog.

## [v0.10.0: Render distance and fog controls](https://github.com/CowgillAlex/minicraft/releases/tag/v0.10.0)

- Add Distance & fog settings with render distance from 12 to 256 metres.
- Control fog independently with an enable toggle, start distance and strength, plus Restore defaults.
- Apply changes immediately and load extra meshes in the background. This version still uses a finite horizontal preview.

## [v0.9.0: Choose a seed and explore dark caves](https://github.com/CowgillAlex/minicraft/releases/tag/v0.9.0)

- Add a title screen with a numeric seed field and random seeds. Generate terrain only after Create world.
- Add F6 night vision to brighten exploration without changing time or propagated lighting.
- Reduce GPU mesh bandwidth with packed 28-byte vertices and cull fully fogged chunks.
- Limit streaming shadow refreshes while preserving immediate refreshes for edits, doors and time changes.

## [v0.8.0: Full-height terrain and spectator mode](https://github.com/CowgillAlex/minicraft/releases/tag/v0.8.0)

- Generate the full -64 to 320 metre range in the finite preview.
- Speed up sampling with cached interpolation, exact uniform-region skipping and compact uniform pages.
- Stream nearby meshes after material generation, with fog hiding the mesh boundary.
- Add F5 spectator mode to pass through terrain without gravity; returning requires clear space.

## [v0.7.0: 3D aquifers and translucent water](https://github.com/CowgillAlex/minicraft/releases/tag/v0.7.0)

- Extend aquifers to fully seeded 3D cells, X/Z surface checks and 3D pressure barriers.
- Allow fluid regions to vary along both horizontal axes while retaining the density field and metre scale.
- Render translucent water with visible shallow bottoms and deeper blue tint, preserving animated reflections and day/night shadows.

## [v0.6.1: A responsive startup screen](https://github.com/CowgillAlex/minicraft/releases/tag/v0.6.1)

- Open the window before terrain generation, with a dark loading screen and progress for terrain, meshes, lighting and graphics preparation.
- Move generation and GPU preparation into the background so the window remains resizable and responsive.
- Allow Escape to cancel loading. Sampling cost itself is unchanged in this build.

## [v0.6.0: The terrain noise port](https://github.com/CowgillAlex/minicraft/releases/tag/v0.6.0)

- Port the density graph and initial aquifer rules from Paper-Minecraft PR #1.
- Sample every 6.25 cm while keeping one reference block equal to one metre.
- Start with a finite 40 × 12 × 40 metre preview containing stone, water and air. Add seed, origin and noise diagnostic options.
- Keep the earlier village available with --scene village. This build retains the source aquifer’s original X/Y layout.

## [v0.5.0: Editable hinged doors](https://github.com/CowgillAlex/minicraft/releases/tag/v0.5.0)

- Recognize a wooden frame and handle as a door. Press E to open or close it.
- Break or place individual voxels on the panel, including glass and custom infill, while retaining the moving object.
- Breaking the perimeter or removing its handles disables operation; repairing it restores operation.
- Add animated swings, collision-aware interaction, local picking and lighting updates. Support all four horizontal facings.

## [v0.4.1: Follow the local clock](https://github.com/CowgillAlex/minicraft/releases/tag/v0.4.1)

- Add the optional Use local time setting so the sky and HUD clock follow the computer clock and timezone.
- Keep the current game time when disabling it. Manual time adjustments switch back to manual control.

## [v0.4.0: Walking, gravity and smooth physics](https://github.com/CowgillAlex/minicraft/releases/tag/v0.4.0)

- Add swept AABB collisions against individual voxels, gravity, jumping, wall sliding and half-metre step-up.
- Run physics at 20 ticks per second and interpolate rendering between ticks while keeping mouse look immediate.
- Toggle collision-aware flight with F. Prevent block placement inside the player and find clear reset positions.
- Make water reflections and wave highlights respect sunlight, moonlight shadows and sky occlusion.

## [v0.3.0: MiniCraft and the new HUD](https://github.com/CowgillAlex/minicraft/releases/tag/v0.3.0)

- Rename the game to MiniCraft.
- Replace the old HUD with a compact material hotbar, block previews, a crosshair, clock and selected-material label.
- Add clickable pause, settings and controls menus, with diagnostics behind F3.
- Embed the UI font and update Windows branding.

## [v0.2.1: Faster light propagation after edits](https://github.com/CowgillAlex/minicraft/releases/tag/v0.2.1)

- Repair affected light paths after edits instead of rebaking the whole world.
- Upload only the changed portion of the light volume, and apply completed updates while editing continues.

## [v0.2.0: Lamps, glass and the day/night cycle](https://github.com/CowgillAlex/minicraft/releases/tag/v0.2.0)

- Street lamps and ceiling lamps emit warm light; cool skylight enters rooms through translucent glass.
- Glass transmits propagated light and sunlight, with tint and reflections.
- Use - and = to adjust time. Sun direction, shadows, sky color and the clock follow the change.
- Calculate lighting on a background worker after edits, with HDR tone mapping and bloom.

## [v0.1.4: Clean sun shadows](https://github.com/CowgillAlex/minicraft/releases/tag/v0.1.4)

- Fix triangle-shaped shadow bands using geometric face normals.
- Keep the existing ambient occlusion and its strength unchanged.

## [v0.1.3: Ambient occlusion](https://github.com/CowgillAlex/minicraft/releases/tag/v0.1.3)

- Add ambient occlusion around recesses and nearby surfaces.
- Press O to compare the scene with and without AO.

## [v0.1.2: Individual voxel editing](https://github.com/CowgillAlex/minicraft/releases/tag/v0.1.2)

- Break and place individual voxels, pick materials with middle-click, and select building materials with the number keys.
- A target outline and selected-material label make editing visible. Only affected chunks and their neighbors rebuild.
- Swap the movement modifiers: Shift descends and Ctrl boosts flight.

## [v0.1.1: Early renderer patch](https://github.com/CowgillAlex/minicraft/releases/tag/v0.1.1)

- Preserved early Windows build. Its saved README does not record a functional change from v0.1.0, so a more specific changelog is unavailable.

## [v0.1.0: The first voxel world](https://github.com/CowgillAlex/minicraft/releases/tag/v0.1.0)

- First Windows build under the Cedar Voxel Lab name, with 6.25 cm voxels and a finite 40 × 40 metre world.
- Terrain, a winding road, cabins, trees, lamps, a pond and a bridge, explored with a free-flight camera.
- Hidden-face removal, greedy meshing, frustum culling, cached sun shadows and a performance HUD.
