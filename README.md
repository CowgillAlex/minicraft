# Cedar Voxel Lab

A native Rust voxel world explorer with 6.25 cm voxels, terrain, cabins, trees, water, editable blocks, ambient occlusion, warm lamp lighting, transparent glass, and a controllable day/night cycle.

## Windows download

Version 0.2.1: [standalone executable](https://img.kryptonote.com/bed92f88-8133-47f5-92bb-cb9bade8355c.exe) or [ZIP with controls and checksum](https://img.kryptonote.com/fbb2ce56-922f-44cb-999a-eee3739ea7bc.zip). Downloads require access to Alex's resource host.

## Run from source

```sh
cd native
cargo run --release --locked
```

Click to capture the mouse. WASD moves, Space flies up, Shift flies down, and Ctrl boosts. Left-click breaks a voxel, right-click places one, number keys select materials, and middle-click picks a material. B selects glass. - and = move time backward and forward. Esc releases the mouse. Edits reset on exit.

See [the native app documentation](native/README.md) for renderer details, system dependencies, all controls, tests, and versioned Windows packaging. This version has no AI, driving physics, or persistent saves.
