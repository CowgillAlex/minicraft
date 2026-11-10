# MiniCraft

A native Rust voxel world explorer with 6.25 cm voxels, terrain, cabins, trees, water, editable blocks, ambient occlusion, warm lamp lighting, transparent glass, a controllable day/night cycle, a compact HUD with native menus, and player collisions and gravity.

## Windows download

Version 0.4.0: [standalone executable](https://img.kryptonote.com/1bbcc7af-c9b6-4d60-8df2-6fc7e1c1c2be.exe) or [ZIP with controls and checksum](https://img.kryptonote.com/a6532777-ee19-45d2-93b8-dc9171fcc302.zip). Downloads require access to Alex's resource host.

## Run from source

```sh
cd native
cargo run --release --locked
```

Choose Play or press Enter to play. WASD moves, Space jumps, Shift slows walking, and Ctrl sprints. F toggles flight, where Space/Shift fly up/down. Left-click breaks a voxel, right-click places one, number keys select materials, and middle-click picks a material. B selects glass. - and = move time backward and forward. Esc opens the world menu. F1 shows controls, F3 toggles debug info, and H hides the HUD. Edits reset on exit.

See [the native app documentation](native/README.md) for renderer details, system dependencies, all controls, tests, and versioned Windows packaging. This version has no AI, driving physics, or persistent saves.
