CEDAR VOXEL LAB v0.1.4
Windows x64 / native Rust + wgpu

Run cedar-voxel-lab-v0.1.4-windows-x64.exe. No installer or extra asset files.
The procedural world takes a few seconds to generate when starting.
Requires Windows 10/11 x64 and a DirectX 12 or Vulkan-capable graphics driver.

CONTROLS
Click the scene: capture mouse
WASD: move horizontally
Mouse: look
Space / E: fly up
Shift / Q: fly down
Ctrl: move faster
Scroll: change movement speed
Esc: release mouse
R: reset camera
H: hide/show HUD
G: show voxel boundaries
O: toggle ambient occlusion
V: toggle VSync
F11: toggle fullscreen
After capturing the mouse:
Left click: break one voxel
Right click: place one voxel against the targeted face
Middle click: pick the targeted material
1-0: select Stone, Grass, Dirt, Wood, Leaves, Roof, Plaster, Sand, Asphalt, Lamp
A mint outline highlights the targeted 6.25 cm voxel within 8 metres.
The HUD names the selected placement material. Edits reset on exit.
Ambient occlusion adds contact shading around recesses and nearby surfaces.
Press O to compare it on/off.
Version 0.1.4 fixes triangle-shaped shadow bands using geometric face normals.
AO and its strength are unchanged. Lamps and windows remain emissive.
Close the window to quit.

WORLD
Each metre contains 16 voxels on each axis. Each voxel is 6.25 cm wide.
A metre-sized block contains 4,096 voxels. The world is a finite 40 x 40 metre
scene with terrain, a winding road, cabins, trees, lamps, a pond and a bridge.
The camera flies freely through geometry. This version is a renderer;
there is no driving physics or AI.

PERFORMANCE
The renderer removes buried faces, merges matching exposed faces, shares
GPU mesh buffers, culls chunks outside the camera, and caches shadows between edits.
Edits rebuild only affected chunks and their face/AO neighbors.
The HUD reports frame rate, frame interval, visible triangles and chunks.
The first frame includes shadow-map creation. Default VSync caps FPS to your
display refresh rate. Toggle it with V to measure uncapped throughput.

ERRORS
Startup errors write voxel-drive-error.txt beside the executable and display
a Windows error dialog. This build is not code-signed.
