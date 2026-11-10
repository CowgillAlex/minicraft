# Cedar Voxel Lab

A native Rust voxel world explorer with 6.25 cm voxels, terrain, cabins, trees, water, editable blocks, ambient occlusion, and sun shadows.

## Windows download

Version 0.1.4: [standalone executable](https://img.kryptonote.com/4882b774-5c00-4a85-9880-a41fc83774de.exe) or [ZIP with controls and checksum](https://img.kryptonote.com/3efa508a-acae-4c36-b79f-6fb9daa5ddae.zip). Downloads require access to Alex's resource host.

## Run from source

```sh
cd native
cargo run --release --locked
```

Click to capture the mouse. WASD moves, Space flies up, Shift flies down, and Ctrl boosts. Left-click breaks a voxel, right-click places one, number keys select materials, and middle-click picks a material. Esc releases the mouse. Edits reset on exit.

See [the native app documentation](native/README.md) for renderer details, system dependencies, all controls, tests, and versioned Windows packaging. This version has no AI, driving physics, or persistent saves.
