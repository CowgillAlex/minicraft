#!/usr/bin/env python3
"""Capture all biome recipes and close-ups through the production GPU pipelines."""
import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--size', default='1100x760')
parser.add_argument('--only', help='Comma-separated capture names')
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
biomes = sorted({r[6] for r in json.loads((ROOT / 'assets/worldgen/biomes.json').read_text())})
shots = [(b, b, 'feature-water' if 'ocean' in b or 'river' in b else 'feature-cave' if 'caves' in b or b == 'deep_dark' else 'feature-overview', 10.5) for b in biomes]
shots += [(n, scene, view, hour) for n, scene, view, hour in [
    ('plant-parts', 'plants', 'feature-close', 10.5),
    ('flower-parts', 'plants', 'feature-flowers', 10.5),
    ('wheat-grains', 'plants', 'feature-wheat', 10.5),
    ('grass-seeds', 'plants', 'feature-grass', 10.5),
    ('fern-fronds', 'plants', 'feature-fern', 10.5),
    ('branching-oak', 'tree', 'feature-tree', 10.5),
    ('cottage', 'cabin', 'feature-cabin', 10.5),
    ('cottage-night', 'cabin', 'feature-cabin', 21.0),
    ('mineral-inclusions', 'ores', 'feature-close', 10.5),
]]
shots += [(name+'-inclusions','ores','feature-ore-'+str(i),10.5) for i,name in enumerate(['coal','iron','copper','gold','redstone','lapis','diamond','emerald'])]
if args.only:
    names = set(args.only.split(','))
    shots = [s for s in shots if s[0] in names]
manifest = []
for i, (name, scene, view, hour) in enumerate(shots):
    path = args.output / (name + '.png')
    command = [str(ROOT/'target/release/minicraft'), '--showcase', scene, '--screenshot', str(path.resolve()),
               '--view', view, '--ui', 'clean', '--no-fog', '--size', args.size, '--time', str(hour), '--frames', '1']
    if view=='feature-cave': command.append('--night-vision')
    with (args.output/(name+'.log')).open('w') as log:
        subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=180)
    manifest.append(dict(name=name,scene=scene,view=view,time=hour,file=path.name,kind='controlled recipe preview'))
    print(f'{i+1}/{len(shots)} {name}', flush=True)
(args.output/'manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
