# MiniCraft

A native Rust voxel world explorer with 6.25 cm voxels. The default scene previews stone, water and air from the Paper-Minecraft worldgen port, sampled at its original metre scale. It includes editable voxels, ambient occlusion, lighting, a controllable day/night cycle, native menus, collisions and gravity. The previous village with cabins, lamps and editable hinged doors is available with `--scene village`.

## Windows download

Version 0.6.0: [standalone executable](https://img.kryptonote.com/4ffd9cff-538e-4930-9736-f58575abc0bd.exe) or [ZIP with controls and checksum](https://img.kryptonote.com/5ec7ea65-4e27-4e6f-a2fc-8189e73ee86b.zip). Downloads require access to Alex's resource host.

This first worldgen step displays a finite 40 × 12 × 40 metre window. The density field uses the source's -64 to 320 metre vertical range; full-height loading, infinite streaming and compressed cubic chunks are later work. `--seed` and `--origin X,Y,Z` select the preview region. Noise and material decisions are checked against 5,496 Java reference samples.

## Run from source

```sh
cd native
cargo run --release --locked
```

Choose Play or press Enter to play. WASD moves, Space jumps, Shift slows walking, and Ctrl sprints. F toggles flight, where Space/Shift fly up/down. Left-click breaks a voxel, right-click places one, number keys select materials, and middle-click picks a material. B selects glass, N selects a door handle, and E opens/closes a targeted door. - and = move time backward and forward. Esc opens the world menu. F1 shows controls, F3 toggles debug info, and H hides the HUD. Settings includes Use local time to follow your computer clock. Edits reset on exit.

See [the native app documentation](native/README.md) for renderer details, system dependencies, all controls, tests, and versioned Windows packaging. This version has no AI, driving physics, or persistent saves.
