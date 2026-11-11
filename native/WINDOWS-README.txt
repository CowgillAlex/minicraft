MINICRAFT v0.13.1
Windows x64 / native Rust + wgpu

Run minicraft-v0.13.1-windows-x64.exe. No installer or extra asset files.
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

UI
A compact bottom hotbar shows block previews and the selected material.
Tab opens a dark picker for all 39 materials. Click to select and return
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
Longer distances take more time to generate and draw more geometry. Bounded
caches may fill before the selected distance is reached in dense caves.
Loading terrain shows pending work; Terrain cache full indicates this limit.
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
the old 2D layout. Terrain density and its original metre scale are unchanged.
Water is translucent: shallow water reveals the bottom and deeper water
adds blue tint. Animated reflections and day/night shadows remain active.
Surface rules add grass/dirt, gravel seabeds, sand/sandstone, red sand,
badlands terracotta bands, mountain snow/ice/calcite, mud, podzol, mycelium,
sulfur/cinnabar, and bedrock/deepslate transitions. Layer depths retain metre
scale; a one-metre layer is 16 tiny voxels deep. Surface decoration runs on
the terrain workers and does not require loaded neighboring pages.
There are no generated ores, buildings, trees or other features yet.
All materials are selectable with Tab, middle-click or #. Ice and powder snow
currently behave as opaque solid blocks.
Walking has gravity, jumping, wall sliding and half-metre step-up. Both
walking and flight collide with individual fine voxels, including glass.
Water and flowers do not collide; swimming is not simulated yet.
Physics ticks at 20 TPS; rendering interpolates positions between ticks
and mouse look remains immediate. Menus freeze physics. Blocks cannot be
placed inside your body. The world bottom and unloaded terrain are solid outside spectator mode.
This version has no car physics or AI.

ERRORS
Startup errors write minicraft-error.txt beside the executable and display
a Windows error dialog. This build is not code-signed.

FONT
DejaVu Sans UI type. The font notice is embedded and available with
--licenses, and included as FONT-LICENSE.txt in the ZIP package.
