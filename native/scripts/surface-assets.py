#!/usr/bin/env python3
"""Bundle pinned surface rules and export the Java biome parameter table.
Needs a JDK. Usage: surface-assets.py /path/to/Paper-Minecraft
"""
import json,re,subprocess,sys,tempfile
from pathlib import Path
REV='d681c0532d5e2628c70e6de7fad7df31f64ea705'
repo=Path(sys.argv[1]);destination=Path(__file__).resolve().parent.parent/'assets/worldgen'
def source(path):return subprocess.check_output(['git','-C',str(repo),'show',f'{REV}:{path}'],text=True,stderr=subprocess.PIPE)
def data(path):return json.loads(source('assets/data/minecraft/worldgen/'+path+'.json'))
settings=data('noise_settings/overworld')
noise= json.loads((destination/'overworld.json').read_text())
for key in ['temperature','vegetation','continents','erosion','depth','ridges']:
 noise['router'][key]=settings['noise_router'][key]
seen=set()
def collect(v):
 if isinstance(v,str) and v.startswith('minecraft:'):
  name=v.removeprefix('minecraft:')
  if name in seen or name in ['y','zero']:return
  seen.add(name)
  try:definition=data('density_function/'+name)
  except subprocess.CalledProcessError:return
  noise['density_functions'][name]=definition;collect(definition)
 elif isinstance(v,dict):
  if v.get('type','').removeprefix('minecraft:') in ['noise','shifted_noise','shift_a','shift_b']:
   name=v.get('noise',v.get('argument')).removeprefix('minecraft:')
   noise['noises'][name]=data('noise/'+name)
  for key,value in v.items():
   if key not in ['type','noise']:collect(value)
 elif isinstance(v,list):
  for value in v:collect(value)
for value in noise['router'].values():collect(value)
(destination/'overworld.json').write_text(json.dumps(noise,indent=2)+'\n')
rule=settings['surface_rule'];names={'surface','surface_secondary','clay_bands_offset'}
def rules(v):
 if isinstance(v,dict):
  if 'noise'in v and isinstance(v['noise'],str):names.add(v['noise'].removeprefix('minecraft:'))
  for value in v.values():rules(value)
 elif isinstance(v,list):
  for value in v:rules(value)
rules(rule)
(destination/'surface.json').write_text(json.dumps({'surface_rule':rule,'noises':{n:data('noise/'+n) for n in sorted(names)}},indent=2)+'\n')
biome=source('core/src/main/java/dev/alexco/minecraft/world/biome/BiomeManager.java')
biomes=sorted(set(re.findall(r'Biomes\.([A-Z_]+)',biome)))
with tempfile.TemporaryDirectory(prefix='minicraft-biome-') as tmp:
 root=Path(tmp);pkg=root/'dev/alexco/minecraft/world/biome';pkg.mkdir(parents=True)
 (pkg/'BiomeManager.java').write_text(biome)
 (pkg/'Biome.java').write_text('package dev.alexco.minecraft.world.biome; public record Biome(String name) {}')
 (pkg/'Biomes.java').write_text('package dev.alexco.minecraft.world.biome; public class Biomes {'+''.join(f'public static final Biome {b}=new Biome("{b.lower()}");' for b in biomes)+'}')
 (root/'Export.java').write_text('''import java.lang.reflect.*;import java.util.*;import dev.alexco.minecraft.world.biome.*;
 public class Export {public static void main(String[]args)throws Exception {
 var f=BiomeManager.class.getDeclaredField("PARAMETERS");f.setAccessible(true);var rows=(List<?>)f.get(null);
 System.out.print("[");boolean first=true;
 for(var row:rows){if(!first)System.out.print(",");first=false;System.out.print("[");
 for(var name:new String[]{"temperature","humidity","continentalness","erosion","depth","weirdness"}){
 var method=row.getClass().getDeclaredMethod(name);method.setAccessible(true);var range=method.invoke(row);
 var lo=range.getClass().getDeclaredMethod("min");var hi=range.getClass().getDeclaredMethod("max");lo.setAccessible(true);hi.setAccessible(true);
 System.out.print("["+lo.invoke(range)+","+hi.invoke(range)+"],");}
 var method=row.getClass().getDeclaredMethod("biome");method.setAccessible(true);System.out.print("\\\""+((Biome)method.invoke(row)).name()+"\\\"]");}
 System.out.println("]");}}
''')
 subprocess.run(['javac','-d',str(root),*map(str,root.rglob('*.java'))],check=True)
 rows=json.loads(subprocess.check_output(['java','-cp',str(root),'Export'],text=True))
(destination/'biomes.json').write_text(json.dumps(rows,separators=(',',':'))+'\n')
print(f'Bundled {len(rows)} biome entries, {len(names)} surface noises, and source surface rules from {REV}')
