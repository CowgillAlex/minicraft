# MiniCraft

A native Rust voxel world explorer with 6.25 cm voxels. The default scene previews stone, water and air from the Paper-Minecraft worldgen port, sampled at its original metre scale. It includes editable voxels, ambient occlusion, lighting, a controllable day/night cycle, native menus, collisions and gravity. The previous village with cabins, lamps and editable hinged doors is available with `--scene village`.

## Windows download

Version 0.9.0: [standalone executable](https://img.kryptonote.com/29d01a26-2587-43bc-b3fc-c9598843eee2.exe) or [ZIP with controls and checksum](https://img.kryptonote.com/3e36699f-547f-459f-bb2d-b9e1bf71b8ba.zip). Downloads require access to Alex's resource host.

The default world is a finite 40 × 384 × 40 metre column, covering the source's full -64 to 320 metre vertical range. Uniform 2 metre pages compress to a single material value. Nearby meshes stream around the player within 28 metres, with fog hiding the cache edge. Horizontal infinite generation and saves remain future work. `--seed`, `--origin X,Y,Z` and `--height` select the region. Terrain density is checked against 5,496 Java reference samples; another 3,564 samples check the 3D aquifer material decisions. Water is translucent, with depth tinting and visible shallow bottoms.

The desktop opens to a dark title screen with a seed field. Enter a signed 64-bit number or leave it blank for a random world, then choose Create world. Terrain generation starts only after that choice. The loading screen stays responsive while terrain, meshes, lighting and graphics prepare in the background. It shows page counts and elapsed time; resize normally or press Escape to cancel. Generation reuses interpolation corners, prepares the bundled density expression once per page, and skips regions only when conservative bounds prove their material is uniform. It retains fine voxel sampling where needed.

## Run from source

```sh
cd native
cargo run --release --locked
```

Create a world, then choose Play or press Enter to play. WASD moves, Space jumps, Shift slows walking, and Ctrl sprints. F toggles flight, where Space/Shift fly up/down. F6 toggles night vision to reveal dark caves without changing the clock or propagated lighting. F5 toggles spectator mode, which passes through terrain without gravity. Use the flight controls and Ctrl to boost; returning requires clear space inside the world. Left-click breaks a voxel, right-click places one, number keys select materials, and middle-click picks a material. B selects glass, N selects a door handle, and E opens/closes a targeted door. - and = move time backward and forward. Esc opens the world menu. F1 shows controls, F3 toggles debug info, and H hides the HUD. Settings includes Use local time to follow your computer clock. The world menu can return to the title screen to choose another seed. Edits reset when you create another world or exit.

See [the native app documentation](native/README.md) for renderer details, system dependencies, all controls, tests, and versioned Windows packaging. This version has no AI, driving physics, or persistent saves.
