# MiniCraft

A native Rust voxel world explorer with 6.25 cm voxels, terrain, cabins, trees, water, editable blocks, ambient occlusion, warm lamp lighting, transparent glass, a controllable day/night cycle, and a compact HUD with native menus.

## Windows download

Version 0.3.0: [standalone executable](https://img.kryptonote.com/3270b3bf-c407-4e96-9714-6509025c5989.exe) or [ZIP with controls and checksum](https://img.kryptonote.com/1f63fb09-80f2-4bdc-9b85-45e125a4aefd.zip). Downloads require access to Alex's resource host.

## Run from source

```sh
cd native
cargo run --release --locked
```

Choose Play or press Enter to play. WASD moves, Space flies up, Shift flies down, and Ctrl boosts. Left-click breaks a voxel, right-click places one, number keys select materials, and middle-click picks a material. B selects glass. - and = move time backward and forward. Esc opens the world menu. F1 shows controls, F3 toggles debug info, and H hides the HUD. Edits reset on exit.

See [the native app documentation](native/README.md) for renderer details, system dependencies, all controls, tests, and versioned Windows packaging. This version has no AI, driving physics, or persistent saves.
