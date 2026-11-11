# MiniCraft

A native Rust voxel world explorer with 6.25 cm voxels. The default scene streams an infinite horizontal world with biome-driven surface layers and 3D aquifers from the Paper-Minecraft worldgen port, sampled at its original metre scale. It includes editable voxels, ambient occlusion, lighting, a controllable day/night cycle, native menus, collisions and gravity. The previous village with cabins, lamps and editable hinged doors is available with `--scene village`.

## Windows download

Version 0.13.0: [standalone executable](https://img.kryptonote.com/f38aea36-1dbe-415f-9eda-f2becbb40981.exe) or [ZIP with controls and checksum](https://img.kryptonote.com/e79b13c4-0d75-4e9a-809a-d74588efb544.zip). Downloads require access to Alex's resource host.

The world spans the source's full -64 to 320 metre vertical range. Terrain, meshes and light load near the player as you explore in any horizontal direction. Uniform 2 metre pages compress to one material value; bounded caches discard distant generated data. Your edits and doors remain in RAM when you leave and return, until you create another world or exit. Local light edits run independently of terrain loading. Nothing is saved to disk.

Settings → Distance & fog controls render distance from 12 to 256 metres, fog strength, where fog starts, and whether fog is enabled. The default remains 24 metres, with a 4 metre loading margin. A worker pool prioritizes the view and travel direction, with collision data loading ahead of visual work. Cold terrain still takes time to prepare at large distances. Walking and flight stop at unloaded terrain; spectator can travel ahead of loading. `--seed`, `--origin X,Y,Z` and `--height` select the source coordinates and vertical range. `--finite` retains the old 40 metre wide preview. Terrain density is checked against 5,496 Java reference samples; another 3,564 samples check the 3D aquifer material decisions. Water is translucent, with depth tinting and visible shallow bottoms.

Surface rules add grass/dirt, sand and sandstone, gravel seabeds, badlands terracotta bands, snow, ice, calcite, mud, podzol, mycelium, sulfur/cinnabar, and bedrock/deepslate transitions. All 39 materials are available through Tab. Layer depths retain metre scale while the exposed shape retains 6.25 cm resolution. Biomes come from the source climate channels and ordered parameter table. Surface fixtures check 1,575 Java-generated synthetic columns and 3,584 biome selections. Trees, structures, ores, and other features are a later step. Use `--noise-only` for the previous raw terrain view.

The desktop opens to a dark title screen with a seed field. Enter a signed 64-bit number or leave it blank for a random world, then choose Create world. Terrain generation starts only after that choice. The loading screen stays responsive while the spawn area, lighting and graphics prepare in the background. Generation continues during play. It shows page counts and elapsed time; resize normally or press Escape to cancel. Generation reuses interpolation corners, prepares the bundled density expression once per page, and skips regions only when conservative bounds prove their material is uniform. It retains fine voxel sampling where needed.

## Run from source

```sh
cd native
cargo run --release --locked
```

Create a world, then choose Play or press Enter to play. WASD moves, Space jumps, Shift slows walking, and Ctrl sprints. F toggles flight, where Space/Shift fly up/down. F6 toggles night vision to reveal dark caves without changing the clock or propagated lighting. F5 toggles spectator mode, which passes through terrain without gravity. Use the flight controls and Ctrl to boost; returning requires clear space inside the world. Left-click breaks a voxel, right-click places one, number keys select materials, Tab opens the full material picker, and middle-click picks a material. B selects glass, N selects a door handle, and E opens/closes a targeted door. - and = move time backward and forward. Esc opens the world menu. F1 shows controls, F3 toggles debug info, and H hides the HUD. Settings includes Use local time to follow your computer clock. The world menu can return to the title screen to choose another seed. Edits reset when you create another world or exit.

See [the native app documentation](native/README.md) for renderer details, system dependencies, all controls, tests, and versioned Windows packaging. This version has no AI, driving physics, or persistent saves.
