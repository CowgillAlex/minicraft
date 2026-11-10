# MiniCraft

A native Rust voxel world explorer with 6.25 cm voxels. The default scene previews stone, water and air from the Paper-Minecraft worldgen port, sampled at its original metre scale. It includes editable voxels, ambient occlusion, lighting, a controllable day/night cycle, native menus, collisions and gravity. The previous village with cabins, lamps and editable hinged doors is available with `--scene village`.

## Windows download

Version 0.7.0: [standalone executable](https://img.kryptonote.com/2ea1c7dc-16f2-4b0a-b9ba-88c3077222d3.exe) or [ZIP with controls and checksum](https://img.kryptonote.com/0a8f4c91-503b-44ae-a58a-05e4d75c1d8c.zip). Downloads require access to Alex's resource host.

This first worldgen step displays a finite 40 × 12 × 40 metre window. The density field uses the source's -64 to 320 metre vertical range; full-height loading, infinite streaming and compressed cubic chunks are later work. `--seed` and `--origin X,Y,Z` select the preview region. Terrain density is checked against 5,496 Java reference samples; another 3,564 samples check the 3D aquifer material decisions. Water is translucent, with depth tinting and visible shallow bottoms.

The desktop opens into a responsive loading screen while terrain, meshes, lighting and graphics prepare in the background. It shows page counts and elapsed time; resize normally or press Escape to cancel. The sampling cost and terrain behavior are unchanged.

## Run from source

```sh
cd native
cargo run --release --locked
```

Choose Play or press Enter to play. WASD moves, Space jumps, Shift slows walking, and Ctrl sprints. F toggles flight, where Space/Shift fly up/down. Left-click breaks a voxel, right-click places one, number keys select materials, and middle-click picks a material. B selects glass, N selects a door handle, and E opens/closes a targeted door. - and = move time backward and forward. Esc opens the world menu. F1 shows controls, F3 toggles debug info, and H hides the HUD. Settings includes Use local time to follow your computer clock. Edits reset on exit.

See [the native app documentation](native/README.md) for renderer details, system dependencies, all controls, tests, and versioned Windows packaging. This version has no AI, driving physics, or persistent saves.
