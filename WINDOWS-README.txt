MINICRAFT v0.17.0
Windows x64 / native Rust + wgpu

Run minicraft-v0.17.0-windows-x64.exe. No installer or extra asset files.
The window opens to a dark title screen. Enter a signed 64-bit numeric seed,
or leave it blank for a random world. Ctrl+A selects the seed and Ctrl+V
pastes it. Create world starts generation. A dark loading screen shows
terrain/mesh page counts, lighting and graphics preparation, and elapsed time.
Generation and GPU preparation run in the background. Resize, minimize or
close the window normally; Escape cancels loading. The normal world menu
appears when ready. Full-height terrain now uses cached interpolation, exact
uniform-region skipping and compressed uniform pages. Nearby meshes stream
as you move, together with new terrain and lighting. Only the spawn area
and its small meshing border are prepared before play. Sampling runs on a
bounded worker pool, prioritizing the view and travel direction.
Requires Windows 10/11 x64 and a DirectX 12 or Vulkan-capable graphics driver.

STREAMING IN v0.17.0
Voxel data is delivered independently of cancelled meshes, and border edits
keep neighbour geometry until its data is loaded. Walking refreshes collision
data every half metre to avoid getting stuck at an unloaded page boundary.
Geometry preparation and lighting run as separate stages using one shared
sampling pool. At most three regions are in flight. Mesh uploads overlap
lighting; new pages appear as soon as their mesh and correct light are ready.
Existing regions retain their previous geometry until replacement is complete.
Gradual turns reprioritize against the last requested view. Obsolete queued
page jobs cancel between bounded batches; useful in-flight work survives
view changes. Unique tickets prevent stale results replacing newer work.

F3 reports geometry queue count, active generation, queued/active lighting,
staged regions and the oldest pending request. Last-region timings report
first and complete render readiness, excluding display/VSync waiting.

CHUNK OVERLAYS IN v0.17.0
F7 toggles world-anchored timing labels for nearby two-metre render chunks.
Each label identifies the local chunk coordinate and shows generation and
mesh-build durations in milliseconds. Mesh includes packing and LOD work
for streamed procedural chunks. Queue waiting, lighting and GPU uploads
are excluded. Source cache means the exact source mesh was reused before
packing/simplifying; Mesh cached shows its recorded build cost when the
finished mesh is reused. n/a means no measurement is available, including
per-chunk generation in the prebuilt village. These are latest recorded
worker timings, rather than total world-loading times.
The extra R rows describe the enclosing eight-metre streaming region:
geometry queue wait, light queue wait, lighting work, publication delay,
CPU upload/data-publication work and request-to-complete-render-ready time.
CPU upload work can overlap lighting and should not be added to the total.
Labels follow the current view, hide behind opaque terrain at their anchor,
and remain sharp at reduced render scales. Only a limited number of nearby,
non-overlapping labels are shown. F7 works independently of F3. Menus and
H hide the labels. Start with --chunk-timings to enable them immediately.

GRAPHICS AND DEBUG IN v0.17.0
F3 shows frame-time and simulation-tick lagometers, measured FPS/TPS and
GPU pass times where supported. Frame bars include VSync and frame-limit
waiting; cyan marks CPU render submission. Tick bars measure actual physics
execution time, with a 50ms budget for the 20 TPS simulation. These are local
simulation timings; there is no network server or network latency graph.

Esc > Settings > Graphics & performance offers render scale, screen AO
quality, shadow resolution, bloom, FPS limit and terrain occlusion controls.
Laptop preset uses 75% render scale, low screen AO and 2048 shadows at a
60 FPS cap. Text and menus stay at native resolution. High quality defaults
restores 100% scale, high AO and 4096 shadows. These settings last for the
current application session and survive returning to the title screen.
Menus render at up to 30 FPS, unfocused windows at 10 FPS, and minimized
windows stop redrawing. VSync remains a separate setting.

The sky renders after opaque terrain. Hidden terrain is conservatively
culled from the current view using solid resident pages; caves, holes,
glass, partial coverage and near-plane crossings remain visible.
Open views back off to occasional occlusion probes when nothing can be
rejected, drawing everything between probes to reduce CPU overhead.

STREAMING IN v0.16.1
Generation reuses prepared density bounds and groups vertical pages on workers.
The initial spawn generates in parallel. Surface and feature materials use bulk
writes; skylight and transport reuse whole-cell optical proofs. Concurrent
workers share each feature-plan build. Terrain resolution and shapes are unchanged.

Lighting tests exact feature shapes without rasterizing full pages or generating
ore deposits. Uniform base and resident pages reuse their optical response.
Workers share feature plans, caches evict old work incrementally, and surface
rules resolve constant stone spans together. Biome selection uses an exact
range index; the existing four-metre biome sampling grid is unchanged.
Page meshes prepare in parallel. A bounded 64 MiB cache reuses exact source
geometry when changing LOD, with affected pages invalidated after edits.
Geometry, AO, transparency and the conservative distant simplifier are unchanged.

UI
A compact bottom hotbar shows block previews and the selected material.
Tab opens a dark picker for all 78 materials. Click to select and return
to the world, or press Tab again to close it.
Esc opens the clickable world menu. Settings control AO, VSync, voxel
outlines, and time of day. Use local time follows the computer clock in
its local timezone, including while menus are open. This option is off
by default and lasts for the current session. Turning it off keeps the
current time. The -/+ time buttons and -/= keys switch back to manual.
Settings > Distance & fog adjusts render distance from 12 to 256 metres.
Fog has an on/off toggle, a start distance and strength from 25% to 200%.
Cave fog and its backdrop fade to black using skylight exposure, including
with night vision. Outdoor fog retains the sky colour.
Fog start scales with render distance. The default is 24 metres of terrain,
fog starting at 20 metres, and 100% strength. Restore defaults resets these
controls. Changes apply immediately; extra meshes load in the background.
Adaptive terrain detail reduces distant mesh geometry and memory while
keeping nearby voxels exact. Disable it here to compare full distant detail.
Longer distances take more time to generate and draw more geometry. Bounded
caches may fill before the selected distance is reached in dense caves.
Loading terrain shows pending work. F3 reports memory pressure. The cache
prioritizes the view and replaces less useful regions as you move or turn.
F1 shows controls and F3 shows debug info. Night vision is in Settings or
on F6. It reveals dark caves without changing the clock or rebaking light.
Title screen in the world menu lets you choose another seed. Creating a new
world discards the current session, including edits.

CONTROLS
Play / Back to world / Enter: capture mouse
WASD: move horizontally
Mouse: look
Space: jump (fly up in flight mode)
Shift / Q: slow walk (fly down in flight mode)
Ctrl: sprint / flight boost
F: toggle walking / flight, both with collisions
F6: toggle night vision
F5: toggle spectator, pass through terrain without gravity
    Uses flight controls. Move into clear space inside the world to exit.
E: open / close the targeted door
Scroll: change movement speed
Esc: world menu / back
- / =: move time backward / forward. Hold to advance faster.
R: return to a clear spawn
F1: controls
F3: debug info, including the current biome and hovered material
H: hide/show playing HUD
G: show voxel boundaries
O: toggle ambient occlusion
V: toggle VSync
F11: toggle fullscreen

After capturing the mouse:
Left click: break one voxel
Right click: place one voxel against the targeted face
Middle click / #: pick the targeted material
1-0: Stone, Grass, Dirt, Wood, Leaves, Roof, Plaster, Sand, Asphalt, Lamp
B: select Glass
N: select Handle
Tab: choose any material
A mint outline highlights the target within 8 metres. Edits reset on exit.

DOORS
Doors are individual editable voxel objects. E operates any targeted part.
Left/right click breaks/places one voxel, including on open doors. Materials,
glass and holes stay with the panel when it swings.

Build your own door with a 16 x 32 voxel wooden outline, two voxels deep.
Keep the outer perimeter wood on both layers. Fill the inside however you
like. Select Handle with N and place it halfway up a vertical side, at row
15 or 16 from the bottom. The hinge is assigned to the opposite side.
All four horizontal facings work. Completing a frame after its handle also
recognizes it. The first version uses this standard 1 x 2 metre size.

Breaking the wooden perimeter or removing all attached handles disables
opening/closing. Repairing restores it. A broken door stays hanging in its
current state. Doors cannot move to a position occupied by you or blocks.
Collision/light use the open/closed states; the visible swing is animated.
Door edits and states reset when you exit.

LIGHTING
Street lamps and ceiling lamps cast warm white light onto nearby surfaces.
Cool skylight enters through windows and spreads into rooms. Glass transmits
sunlight and propagated light; walls and roofs block it. Transparent glass
has a subtle tint and sky reflection, and does not cast an opaque sun shadow.
Changing time moves the sun, changes shadows and sky colour, and brings out
warm light pools at dusk and night. The HUD displays the current time.
Try approximately 18:30-21:00 for the strongest warm/cool contrast.
Water receives sunlight/moonlight shadows and sky occlusion. Wave highlights
use actual light instead of adding a bright unlit ripple pattern.

Infinite worlds bake light in nearby 8 metre regions with a lighting halo.
Local edits repair cached lighting on a separate worker, so they can finish
while terrain is still generating. Distant lighting follows in the background. The village retains its
incremental light updates. Completed updates appear while editing continues.
Loading terrain or Updating light indicates pending work.
Geometry updates immediately.
Sun shadows refresh when time or geometry changes. Lamp and sky propagation
are cached independently of time. Bloom runs at quarter resolution and AO
at half resolution. Lamp light is an indirect voxel-light approximation,
not a separate ray-traced point shadow for every lamp.

WORLD
A metre-sized block contains 16 x 16 x 16 voxels. Each voxel is 6.25 cm wide.
The default scene streams an infinite horizontal world with biome-driven
surfaces. It ports the Paper-Minecraft PR #1 density graph, aquifer, biome
selection and surface rules. Terrain is sampled every 6.25 cm at its original scale. The title
screen chooses the seed. The default source origin is 120,-64,-20, covering
the full -64 to 320 metre vertical range. Only nearby terrain is resident.

Uniform 2 metre pages use one material value; mixed pages retain fine voxels.
Uniform parts of mixed pages are skipped exactly during generation. Lighting
samples cached transport rays instead of generating full hidden voxel halos.
Exposed-face bit masks keep solid and enclosed water interiors out of meshing.
Static vertices are packed on workers before upload to keep frames responsive.
Distant meshes use conservative LOD with locked page/material boundaries.
Near geometry, collision, editing, water/glass and doors retain full detail.
Fine color/AO patterns fade below pixel size to reduce distant shimmer.
Fine density sampling is unchanged in this first LOD pass.
Terrain, meshes and light stream within render distance plus a 4 metre margin.
Collision data loads first along an 8 metre corridor in your travel direction.
Walking and flight stop at terrain that has not yet loaded. Spectator can travel ahead of
loading. R waits for spawn terrain before resuming physics. Generated data
and graphics use bounded caches; edits and doors survive leaving and returning
in RAM. Creating another world or exiting discards them. No disk saves.

Packed 28-byte GPU vertices reduce mesh bandwidth and memory. Fully fogged
chunks are culled. Shadows follow the camera as the world streams. Sun and
moon discs stay behind terrain, including at long distances in heavy fog.
Streaming shadow refreshes are limited to eight per second; voxel edits, doors
and time changes still refresh shadows immediately.

Optional launch arguments:
--seed 1                     Choose a signed 64-bit seed.
--height 12                  Restrict vertical range near sea level.
--finite                     Use the old finite 40 metre wide preview.
--no-lod                     Full detail in headless screenshot comparisons.
--origin 120,58,-20           Choose lower corner; defaults to 12m height.
--height 384 --origin 0,-64,0 Choose another full-height source origin.
--scene village               Open the previous village with doors and lamps.
--noise-sample 120.0625,63,0   Print density and material without a GPU.
--noise-benchmark             Measure a raw 32-cubed voxel page without a GPU.
--noise-only                  Disable biome surface decoration.
--worldgen-benchmark          Time a finite 40 metre wide volume.
--reference-worldgen         Disable uniform skipping/prepared evaluation.

Aquifers now use 3D seeded cells, full X/Z surface checks and 3D pressure
barriers. Fluid regions vary along both horizontal axes instead of extruding
the old 2D layout. Preliminary heights now retain the original one-metre
positive-density scan at exact columns. Fine-voxel pressure centres and thin
wet/dry seals fix leaking boundaries; corrected heights remove spurious
sea-level water volumes. Terrain density and its metre scale are unchanged.
Water is translucent: shallow water reveals the bottom and deeper water
adds blue tint. Animated reflections and day/night shadows remain active.
Surface rules add grass/dirt, gravel seabeds, sand/sandstone, red sand,
badlands terracotta bands, mountain snow/ice/calcite, mud, podzol, mycelium,
sulfur/cinnabar, and bedrock/deepslate transitions. Layer depths retain metre
scale; a one-metre layer is 16 tiny voxels deep. Surface decoration runs on
the terrain workers and does not require loaded neighboring pages.
All 56 biomes now have a decoration profile: branching trees and leaf sprays,
seeded grass, flowers with separate parts, wheat/grains, bamboo, cacti, fungi,
alpine/cave growth, river plants, kelp, seagrass and warm-ocean coral.
Occasional cottages fit gentle dry sites, with warm lamps and editable doors.
Press E on a generated door to recognize it and open it. Soft plant parts are
walkable and filter skylight. All parts can be broken/placed as fine voxels.
Coal, iron, copper, gold, redstone, lapis, diamond and emerald appear as little
inclusions within stone/deepslate. Placement counts, height distributions,
biome restrictions and exposure reduction come from the source ore settings.
Plants generate mature. Growth, inventories and animals are not simulated.
--no-features disables decoration/ores while retaining biome surfaces.
--showcase BIOME opens a controlled preview of a biome's feature recipes.
--showcase tree / plants / cabin / ores opens a focused reference plot.
All materials are selectable with Tab, middle-click or #. Ice and powder snow
currently behave as opaque solid blocks.
Walking has gravity, jumping, wall sliding and half-metre step-up. Both
walking and flight collide with individual fine voxels, including glass.
Water and flowers do not collide; swimming is not simulated yet.
Physics ticks at 20 TPS; rendering interpolates positions between ticks
and mouse look remains immediate. Menus freeze physics. Blocks cannot be
placed inside your body. The world bottom and unloaded terrain are solid outside spectator mode.

ERRORS
Startup errors write minicraft-error.txt beside the executable and display
a Windows error dialog. This build is not code-signed.

FONT
DejaVu Sans UI type. The font notice is embedded and available with
--licenses, and included as FONT-LICENSE.txt in the ZIP package.

THIRD-PARTY NOTICES
Meshopt and meshoptimizer notices are embedded, printed by --licenses,
and included as THIRD-PARTY-LICENSES.txt in the ZIP package.
