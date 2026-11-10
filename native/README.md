# MiniCraft

A native Rust voxel renderer. Voxels are 1/16 metre across; a metre-sized block contains 16 × 16 × 16 voxels. Version 0.4.0 explores a finite 40 × 40 metre landscape with cabins, a winding road, trees, lamps, water, and a footbridge. There is no AI or car simulation in this version.

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

Choose Play or press Enter to capture the mouse. WASD moves, mouse looks, Space/E jumps, Shift/Q slows walking, Ctrl sprints, and the wheel adjusts speed. F toggles flight; Space/E flies up and Shift/Q flies down while flying. Esc opens the world menu and releases the cursor. F1 opens Controls, F3 toggles debug info, R returns to a clear spawn position, H hides the playing HUD, G toggles voxel boundaries, O toggles ambient occlusion, -/= move time backward/forward, V toggles VSync, and F11 toggles fullscreen. Walking has gravity. Both walking and flight collide with the fine voxel geometry.

After capturing the mouse, left-click breaks one voxel, right-click places one voxel against the targeted face, and middle-click picks a material. Keys 1-0 select Stone, Grass, Dirt, Wood, Leaves, Roof, Plaster, Sand, Asphalt, and Lamp. B selects Glass. A depth-tested mint outline highlights the target within eight metres. Placement cannot overlap the player body at its simulated or interpolated position, or leave world bounds. Edits last for the current session and reset when you exit.

The playing HUD contains a compact bottom hotbar with isometric material previews, a crosshair, a small clock, and a selected material label. The dark Escape menu has clickable resume, settings, controls, and quit buttons. Settings control AO, VSync, voxel outlines, and time of day. Menus scale with the window and high-DPI displays. H hides the HUD for screenshots; Esc restores the menu.

F3 shows FPS, frame interval, resident voxels, visible triangles/chunks, coordinates, target data, and the 20 TPS tick count/grounded state. FPS includes display synchronization when VSync is enabled. Scene generation happens at startup and is not included in FPS.

## Renderer

- Sparse allocation of 32³-voxel chunks, with one byte per material cell. Chunks use copy-on-write storage so lighting workers can read a consistent snapshot without copying the whole world on every edit.
- Hidden-face removal across chunk boundaries and greedy merging of faces with matching material and ambient occlusion.
- Shared vertex/index buffers for the generated scene, with per-chunk GPU replacements after edits and per-chunk frustum culling. Edits remesh only the touched chunk and neighbors whose faces or corner AO may change.
- A cached 4096² directional shadow map with hardware-filtered comparison sampling. A conservative depth bias bounds the full filter footprint using geometric face normals, avoiding triangle-shaped self-shadow bands. The per-frame shadow lookup uses one filtered comparison rather than nine manual taps. The map is reused until an edit invalidates it, then refreshed on the next frame.
- Bilinear voxel-corner ambient occlusion, plus half-resolution screen-space ambient occlusion with a 0.55 metre radius and depth/normal-aware upsampling. AO darkens the ambient lighting contribution; emissive lamps, direct sunlight, shadowed water highlights, the target outline, and HUD are preserved. Turning AO off skips its sampling pass. Procedural surface variation, animated water highlights, sky, and distance fog complete the scene.
- Back-to-front chunk-sorted glass rendering after the opaque lighting/AO pass. Glass transmits sunlight and has lightly tinted reflections; its depth does not incorrectly occlude interior AO.
- Linear HDR scene colour, filmic tone mapping, and a quarter-resolution bloom pass for subtle lamp glow.
- Embedded WGSL shaders, a baked DejaVu Sans glyph atlas, and a single batched UI draw. Glyphs use one quad each rather than per-pixel geometry. Fonts, icons, and shaders are embedded; the executable needs no asset directory.

This is a rasterized mesh renderer. The generated world extent and material palette are fixed; time of day and voxel edits are interactive. There is no streaming, level of detail, persistent saves, AI, or car simulation yet.

## Player physics

The player uses a 0.6 × 1.8 metre AABB with a 1.62 metre eye height. A swept broad phase gathers only intersecting fine voxels. Motion clips against each voxel on Y, X, and Z, preserving wall sliding and preventing fast movement from skipping a one-voxel wall. Glass is solid; water and flowers are non-solid. Grounded movement can step up 0.5 metres when headroom permits. World edges and the bottom are solid. Placement checks the whole body, and resetting searches for a clear spawn if it has been edited.

Physics runs at exactly 20 fixed steps per simulated second. The accumulator uses real frame elapsed time, with at most 250 ms of catch-up after a stall. Rendering blends the previous and current completed tick using the remaining accumulator fraction. Previous state changes only on a tick, including when a frame needs several ticks. Positions have the standard one-tick interpolation delay, while mouse orientation updates immediately at render frequency. Menus freeze simulation; resuming does not accumulate paused time. Teleports discard old interpolation state, and flight toggles preserve the rendered position. Short jump taps are queued until the next tick.

Walking uses acceleration, gravity of 24 m/s², a 48 m/s terminal fall speed, and an 8.4 m/s jump impulse. Ctrl sprints, Shift slows walking, and F toggles flight with collisions. Space/Shift moves vertically in flight and Ctrl boosts it. Water currently has no swimming/buoyancy simulation. Collision boxes are resolved against individual 6.25 cm voxels; they are not snapped to metre-sized blocks.

Alex's [Paper-Minecraft AABB](https://github.com/CowgillAlex/Paper-Minecraft/blob/main/core/src/main/java/dev/alexco/minecraft/phys/AABB.java) informed the swept-box/per-axis clipping approach. The full reference repository is cloned at `/home/alexc/.t3/projects/Paper-Minecraft` for future work. MiniCraft's collision implementation is three-dimensional and its fixed-step interpolation is separate from that reference.

## Lighting

Street lamps and ceiling lamps emit warm white light. Window blocks are translucent glass rather than emitters, so interiors receive actual lamp light and cool skylight. Glass is selectable with B. - and = adjust the clock in either direction; holding a key repeats the adjustment. The HUD shows local simulation time. Sunset and night around 18:30-21:00 emphasize the contrast between warm lamps and the cool sky.

Two light channels are cached on a 0.25 metre grid: lamp light and skylight. Voxel-accurate segment checks connect neighboring light cells, so even thin opaque walls stop transport. Glass adds a small attenuation instead of blocking light. A brightest-first flood fill spreads light around corners. The GPU reads an interpolated RG8 3D texture, with no loop over lamps per pixel. Skylight is a diffuse sky-access approximation independent of solar direction; directional sunlight still uses the sun shadow map. Lamp light is diffuse propagated illumination, not per-lamp ray-traced shadows.

The initial bake runs at startup. A background worker retains the connectivity graph and source values. Edits recheck only nearby connections, affected sky columns, and nearby lamp seeds. A local removal/refill flood repairs illumination, including closing windows or removing sources. Only the affected texture subvolume is uploaded. Queued edits are coalesced without losing their dirty cells, and completed patches appear in order while editing continues. Cheap copy-on-write snapshots keep geometry responsive; a LIGHT BAKE indicator reports pending work. New time-of-day values update shader parameters and invalidate sun shadows without rebuilding the diffuse light field. The sun/moon, sky, fog and exposure change with the clock. Glass sorting is per chunk; this is intended for clear window panes rather than arbitrary overlapping coloured transparent volumes.

Water waves perturb reflection normals instead of adding unlit brightness. Reflections respect the sky-access field and shadow visibility; sun/moon specular highlights use the same directional shadow map. Ambient water lighting participates in contact AO, so covered water remains dark at night.

## Verify

```sh
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo test --release --locked shadow_receivers -- --ignored --nocapture
cargo test --release --locked water_receives_shadows -- --ignored --nocapture
cargo test --release --locked benchmark_lighting_edits -- --ignored --nocapture
cargo run --release --locked -- --screenshot scene.png --frames 3
cargo run --release --locked -- --screenshot edits.png --verify-edits
cargo run --release --locked -- --screenshot recess.png --ao-closeup
cargo run --release --locked -- --screenshot recess-no-ao.png --ao-closeup --no-ao
cargo run --release --locked -- --screenshot house.png --shadow-view house
cargo run --release --locked -- --screenshot terrain.png --shadow-view terrain
cargo run --release --locked -- --screenshot leaves.png --shadow-view leaves
cargo run --release --locked -- --screenshot night.png --view street --time 21
cargo run --release --locked -- --screenshot water-night.png --view water --time 3
cargo run --release --locked -- --screenshot menu.png --ui pause
cargo run --release --locked -- --screenshot settings.png --ui settings --size 800x600
cargo run --release --locked -- --screenshot controls.png --ui controls
cargo run --release --locked -- --screenshot debug.png --ui debug
cargo run --release --locked -- --screenshot inside.png --view interior --time 21
cargo run --release --locked -- --screenshot glass.png --view window --time 12
```

Physics tests cover landing stability, jumping/head collisions, thin-wall sweeps, sliding, steps/headroom, water/glass collision rules, world bounds, placement protection, flight controls, and equivalent simulation/interpolation at 10, 30, 60 and 144 FPS. A separate GPU test compares open and roof-shaded water at noon and night.

Lighting tests compare incremental edits and partial texture uploads byte-for-byte with fresh full bakes, including lamp removal, overlapping sources, windows, roofs, and coalesced edits. The opt-in lighting benchmark reports initial bake time, edit latency, and uploaded bytes in the generated world.

The opt-in GPU shadow test verifies fully lit, unoccluded surfaces on all three sun-facing axis normals, from near and far camera positions, checks that a real blocker still casts a shadow, and compares thousands of stable house-wall samples with CPU voxel ray casts. It requires a graphics adapter or software Vulkan. The AO close-up command carves a recess using the real voxel editing path. Screen-space AO uses only visible geometry, so hidden or off-screen occluders cannot contribute. The edit verification command breaks and replaces a voxel through the live GPU chunk and shadow paths and checks that the restored mesh has the original triangle count. The screenshot command uses the same rendering pipelines without creating a window. It requires a working GPU driver or a software Vulkan implementation. It prints the adapter name and total elapsed render time; software rendering is useful for correctness checks, not representative desktop performance.

## Windows builds

On Linux, install Rust's `x86_64-pc-windows-gnu` target and the MinGW x64 GCC and binutils packages. Then run from this directory:

```sh
./scripts/package-windows.sh
```

This produces `../releases/v0.4.0/minicraft-v0.4.0-windows-x64.exe`, a ZIP with controls, and a SHA-256 checksum. The version comes from Cargo.toml and appears in the filename, window title, HUD, and Windows file-version resource. The package script refuses to overwrite an existing release. Bump the package version and release notes for subsequent builds. Builds are unsigned. Windows uses a GUI subsystem, with startup errors shown in a message box and written beside the executable.

The UI atlas and metrics are checked in. To regenerate them on Linux, install Pillow and DejaVu Sans, then run `python3 scripts/bake-ui-font.py`. The font license is in `assets/FONT-LICENSE.txt`, embedded in the app, and included in Windows ZIP packages. `--licenses` prints the notice.

The checked-in Cargo.lock pins dependencies. Generated binaries and build outputs stay outside version control.
