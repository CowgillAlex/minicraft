#!/usr/bin/env python3
"""Export the pinned Overworld ore placement rules and biome membership."""
import json
from pathlib import Path
import subprocess
import sys
REVISION = 'd681c0532d5e2628c70e6de7fad7df31f64ea705'
ROOT = Path(__file__).resolve().parents[1]
repo = Path(sys.argv[1])
def read(path):
    raw = subprocess.check_output(['git','-C',str(repo),'show',f'{REVISION}:assets/data/minecraft/worldgen/{path}.json'])
    return json.loads(raw)
names = ['ore_coal_upper','ore_coal_lower','ore_iron_upper','ore_iron_middle','ore_iron_small',
         'ore_copper','ore_copper_large','ore_gold','ore_gold_lower','ore_gold_extra','ore_redstone',
         'ore_redstone_lower','ore_diamond','ore_diamond_medium','ore_diamond_large','ore_diamond_buried',
         'ore_lapis','ore_lapis_buried','ore_emerald']
biomes = sorted({r[6] for r in json.loads((ROOT/'assets/worldgen/biomes.json').read_text())})
features = {b: {f for step in read('biome/'+b)['features'] for f in step} for b in biomes}
rows=[]
for name in names:
    placed=read('placed_feature/'+name)
    rows.append(dict(name=name, placement=placed['placement'],
                     config=read('configured_feature/'+placed['feature'].split(':')[-1])['config'],
                     biomes=[b for b in biomes if 'minecraft:'+name in features[b]]))
(ROOT/'assets/worldgen/ores.json').write_text(json.dumps(rows,separators=(',',':'))+'\n')
