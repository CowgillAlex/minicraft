MINICRAFT v0.3.0
Windows x64 / native Rust + wgpu

Run minicraft-v0.3.0-windows-x64.exe. No installer or extra asset files.
The world and lighting take a few seconds to generate when starting.
Requires Windows 10/11 x64 and a DirectX 12 or Vulkan-capable graphics driver.

UI
A compact bottom hotbar shows block previews and the selected material.
Esc opens the clickable world menu. Settings control AO, VSync, voxel
outlines, and time of day. F1 shows controls and F3 shows debug info.

CONTROLS
Play / Back to world / Enter: capture mouse
WASD: move horizontally
Mouse: look
Space / E: fly up
Shift / Q: fly down
Ctrl: move faster
Scroll: change movement speed
Esc: world menu / back
- / =: move time backward / forward. Hold to advance faster.
R: reset camera
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
A mint outline highlights the target within 8 metres. Edits reset on exit.

LIGHTING
Street lamps and ceiling lamps cast warm white light onto nearby surfaces.
Cool skylight enters through windows and spreads into rooms. Glass transmits
sunlight and propagated light; walls and roofs block it. Transparent glass
has a subtle tint and sky reflection, and does not cast an opaque sun shadow.
Changing time moves the sun, changes shadows and sky colour, and brings out
warm light pools at dusk and night. The HUD displays the current time.
Try approximately 18:30-21:00 for the strongest warm/cool contrast.

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
The finite 40 x 40 metre scene has terrain, cabins, roads, trees and a pond.
Free flight has no collisions. This version has no car physics or AI.

ERRORS
Startup errors write minicraft-error.txt beside the executable and display
a Windows error dialog. This build is not code-signed.

FONT
DejaVu Sans UI type. The font notice is embedded and available with
--licenses, and included as FONT-LICENSE.txt in the ZIP package.
