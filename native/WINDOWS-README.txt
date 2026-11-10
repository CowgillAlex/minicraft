MINICRAFT v0.7.0
Windows x64 / native Rust + wgpu

Run minicraft-v0.7.0-windows-x64.exe. No installer or extra asset files.
The window opens before terrain generation. A dark loading screen shows
terrain/mesh page counts, lighting and graphics preparation, and elapsed time.
Generation and GPU preparation run in the background. Resize, minimize or
close the window normally; Escape cancels loading. The normal world menu
appears when ready. Sampling cost is unchanged in this build.
Requires Windows 10/11 x64 and a DirectX 12 or Vulkan-capable graphics driver.

UI
A compact bottom hotbar shows block previews and the selected material.
Esc opens the clickable world menu. Settings control AO, VSync, voxel
outlines, and time of day. Use local time follows the computer clock in
its local timezone, including while menus are open. This option is off
by default and lasts for the current session. Turning it off keeps the
current time. The -/+ time buttons and -/= keys switch back to manual.
F1 shows controls and F3 shows debug info.

CONTROLS
Play / Back to world / Enter: capture mouse
WASD: move horizontally
Mouse: look
Space: jump (fly up in flight mode)
Shift / Q: slow walk (fly down in flight mode)
Ctrl: sprint / flight boost
F: toggle walking / flight, both with collisions
E: open / close the targeted door
Scroll: change movement speed
Esc: world menu / back
- / =: move time backward / forward. Hold to advance faster.
R: return to a clear spawn
F1: controls
F3: debug info
H: hide/show playing HUD
G: show voxel boundaries
O: toggle ambient occlusion
V: toggle VSync
F11: toggle fullscreen

After capturing the mouse:
Left click: break one voxel
Right click: place one voxel against the targeted face
Middle click: pick the targeted material
1-0: Stone, Grass, Dirt, Wood, Leaves, Roof, Plaster, Sand, Asphalt, Lamp
B: select Glass
N: select Handle
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

Lighting repairs only affected light paths after edits and uploads only the
changed region. Completed updates appear while editing continues, without
waiting for a whole-world bake. LIGHT BAKE indicates any pending work.
Geometry updates immediately.
Sun shadows refresh when time or geometry changes. Lamp and sky propagation
are cached independently of time. Bloom runs at quarter resolution and AO
at half resolution. Lamp light is an indirect voxel-light approximation,
not a separate ray-traced point shadow for every lamp.

WORLD
A metre-sized block contains 16 x 16 x 16 voxels. Each voxel is 6.25 cm wide.
The default scene is a finite 40 x 12 x 40 metre noise preview containing
only stone, water and air. It ports the Paper-Minecraft PR #1 density graph
and aquifer rules. Terrain is sampled every 6.25 cm at its original scale.
Seed 1, origin 120,58,-20. This is a window into the -64 to 320 metre field;
it does not load the full height or support infinite streaming yet.

Optional launch arguments:
--seed 1 --origin 120,58,-20    Choose a seed and lower window corner in metres.
--scene village               Open the previous village with doors and lamps.
--noise-sample 120.0625,63,0   Print density and material without a GPU.
--noise-benchmark             Measure a 32-cubed voxel page without a GPU.

Aquifers now use 3D seeded cells, full X/Z surface checks and 3D pressure
barriers. Fluid regions vary along both horizontal axes instead of extruding
the old 2D layout. Terrain density and its original metre scale are unchanged.
Water is translucent: shallow water reveals the bottom and deeper water
adds blue tint. Animated reflections and day/night shadows remain active.
There are no generated surfaces, ores, buildings, trees or other features
in the noise preview. All existing building materials remain selectable.
Walking has gravity, jumping, wall sliding and half-metre step-up. Both
walking and flight collide with individual fine voxels, including glass.
Water and flowers do not collide; swimming is not simulated yet.
Physics ticks at 20 TPS; rendering interpolates positions between ticks
and mouse look remains immediate. Menus freeze physics. Blocks cannot be
placed inside your body. World edges and the bottom are solid.
This version has no car physics or AI.

ERRORS
Startup errors write minicraft-error.txt beside the executable and display
a Windows error dialog. This build is not code-signed.

FONT
DejaVu Sans UI type. The font notice is embedded and available with
--licenses, and included as FONT-LICENSE.txt in the ZIP package.
