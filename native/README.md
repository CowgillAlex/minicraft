# Cedar Voxel Lab

A native Rust voxel renderer. Voxels are 1/16 metre across; a metre-sized block contains 16 × 16 × 16 voxels. Version 0.1.4 explores a finite 40 × 40 metre landscape with cabins, a winding road, trees, lamps, water, and a footbridge. There is no AI or car simulation in this version.

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

Click to capture the mouse. WASD moves, mouse looks, Space/E flies up, Shift/Q flies down, Ctrl boosts, and the wheel adjusts speed. Esc releases the cursor. R resets the camera, H toggles the HUD, G toggles voxel boundaries, O toggles ambient occlusion, V toggles VSync, and F11 toggles fullscreen. Movement is free flight without collisions.

After capturing the mouse, left-click breaks one voxel, right-click places one voxel against the targeted face, and middle-click picks a material. Keys 1-0 select Stone, Grass, Dirt, Wood, Leaves, Roof, Plaster, Sand, Asphalt, and Lamp. A depth-tested mint outline highlights the target within eight metres. Placement cannot occupy the camera's own cell or leave world bounds. Edits last for the current session and reset when you exit.

The HUD reports FPS and average frame interval, resident solid voxels, visible triangles, visible chunk draw calls, and camera coordinates. FPS includes display synchronization when VSync is enabled. Scene generation happens at startup and is not included in FPS.

## Renderer

- Sparse allocation of 32³-voxel chunks, with one byte per material cell in allocated chunks.
- Hidden-face removal across chunk boundaries and greedy merging of faces with matching material and ambient occlusion.
- Shared vertex/index buffers for the generated scene, with per-chunk GPU replacements after edits and per-chunk frustum culling. Edits remesh only the touched chunk and neighbors whose faces or corner AO may change.
- A cached 4096² directional shadow map with hardware-filtered comparison sampling. A conservative depth bias bounds the full filter footprint using geometric face normals, avoiding triangle-shaped self-shadow bands. The per-frame shadow lookup uses one filtered comparison rather than nine manual taps. The map is reused until an edit invalidates it, then refreshed on the next frame.
- Bilinear voxel-corner ambient occlusion, plus half-resolution screen-space ambient occlusion with a 0.55 metre radius and depth/normal-aware upsampling. AO darkens the ambient lighting contribution; emissive lamps, water reflections, direct sunlight, the target outline, and HUD are preserved. Turning AO off skips its sampling pass. Procedural surface variation, animated water highlights, sky, and distance fog complete the scene.
- Embedded WGSL shaders and a batched bitmap HUD. The executable needs no asset directory.

This is a rasterized mesh renderer. World extent, materials, and lighting are fixed in this first version. There is no streaming, level of detail, persistent saves, or physics yet.

## Verify

```sh
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo test --release --locked shadow_receivers -- --ignored --nocapture
cargo run --release --locked -- --screenshot scene.png --frames 3
cargo run --release --locked -- --screenshot edits.png --verify-edits
cargo run --release --locked -- --screenshot recess.png --ao-closeup
cargo run --release --locked -- --screenshot recess-no-ao.png --ao-closeup --no-ao
cargo run --release --locked -- --screenshot house.png --shadow-view house
cargo run --release --locked -- --screenshot terrain.png --shadow-view terrain
cargo run --release --locked -- --screenshot leaves.png --shadow-view leaves
```

The opt-in GPU shadow test verifies fully lit, unoccluded surfaces on all three sun-facing axis normals, from near and far camera positions, checks that a real blocker still casts a shadow, and compares thousands of stable house-wall samples with CPU voxel ray casts. It requires a graphics adapter or software Vulkan. The AO close-up command carves a recess using the real voxel editing path. Screen-space AO uses only visible geometry, so hidden or off-screen occluders cannot contribute. The edit verification command breaks and replaces a voxel through the live GPU chunk and shadow paths and checks that the restored mesh has the original triangle count. The screenshot command uses the same rendering pipelines without creating a window. It requires a working GPU driver or a software Vulkan implementation. It prints the adapter name and total elapsed render time; software rendering is useful for correctness checks, not representative desktop performance.

## Windows builds

On Linux, install Rust's `x86_64-pc-windows-gnu` target and the MinGW x64 GCC and binutils packages. Then run from this directory:

```sh
./scripts/package-windows.sh
```

This produces `../releases/v0.1.4/cedar-voxel-lab-v0.1.4-windows-x64.exe`, a ZIP with controls, and a SHA-256 checksum. The version comes from Cargo.toml and appears in the filename, window title, HUD, and Windows file-version resource. The package script refuses to overwrite an existing release. Bump the package version and release notes for subsequent builds. Builds are unsigned. Windows uses a GUI subsystem, with startup errors shown in a message box and written beside the executable.

The checked-in Cargo.lock pins dependencies. Generated binaries and build outputs stay outside version control.
