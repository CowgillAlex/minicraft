#!/usr/bin/env python3
"""Compile pinned Java surface rules against synthetic columns for parity fixtures."""
import importlib.util,json,re,subprocess,sys,tempfile
from pathlib import Path
sys.dont_write_bytecode=True
folder=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('reference',folder/'worldgen-reference.py');ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
repo=Path(sys.argv[1]);package=ref.PACKAGE
REV=ref.REVISION
read=lambda p:subprocess.check_output(['git','-C',str(repo),'show',f'{REV}:{p}'],text=True)
# The fixture keeps names, not Rust material numbers.
with tempfile.TemporaryDirectory(prefix='minicraft-surface-reference-') as tmp:
 root=Path(tmp)
 def write(path,text):
  p=root/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text)
 files=subprocess.check_output(['git','-C',str(repo),'ls-tree','-r','--name-only',REV],text=True).splitlines()
 for path in files:
  if path.startswith('assets/data/minecraft/worldgen/noise/') or path=='assets/data/minecraft/worldgen/noise_settings/overworld.json':write(path,read(path))
 for name in ['random/Xoroshiro','noise/Noise','noise/ImprovedNoise','noise/SimplexNoise','noise/NormalNoise','noise/PerlinNoise','vanilla/SurfaceRules','vanilla/SurfaceSystem','vanilla/VanillaSurfaceRuleEngine']:
  text=read(f'core/src/main/java/{package}/{name}.java')
  text='\n'.join(line for line in text.splitlines() if 'System.out.println' not in line)
  write(f'src/{package}/{name}.java',text)
 for path,text in ref.STUBS.items():
  if path!='Oracle.java' and not path.endswith('/Blocks.java') and not path.endswith('/BlockState.java'):write('src/'+path,text)
 cfg=root/f'src/{package}/vanilla/VanillaWorldgenConfig.java';s=cfg.read_text().replace('public int getMinY()', 'public int toInternalY(int y){return y+64;} public BlockState getDefaultBlock(){return Blocks.STONE.defaultBlockState();} public int getMinY()');cfg.write_text(s)
 write(f'src/{package}/vanilla/VanillaNoiseStep.java','''package dev.alexco.minecraft.world.level.levelgen.vanilla;
public class VanillaNoiseStep {public static int height;public static boolean steep;private static final VanillaNoiseStep INSTANCE=new VanillaNoiseStep();public static VanillaNoiseStep get(long seed){return INSTANCE;}public int estimateSurfaceY(int x){return height+(steep&&x>0?5:0);}}''')
 write('src/com/badlogic/gdx/Gdx.java','''package com.badlogic.gdx;public class Gdx {public static Files files=new Files();public static class Files {public com.badlogic.gdx.files.FileHandle internal(String p){return new com.badlogic.gdx.files.FileHandle(p);} public com.badlogic.gdx.files.FileHandle classpath(String p){return internal(p);}}}''')
 write('src/dev/alexco/minecraft/SharedConstants.java','package dev.alexco.minecraft;public class SharedConstants {public static final int CHUNK_HEIGHT=384,CHUNK_WIDTH=16;}')
 write('src/dev/alexco/minecraft/util/Logger.java','package dev.alexco.minecraft.util;public class Logger {public static void ERROR(String s,Object...a){throw new IllegalStateException(String.format(s,a));}public static void INFO(String s,Object...a){}}')
 block='dev/alexco/minecraft/world/level/block'
 write(f'src/{block}/Block.java','''package dev.alexco.minecraft.world.level.block;public class Block {public final String name;public Block(String n){name=n;}public BlockState defaultBlockState(){return new BlockState(this);}public String getDescriptionId(){return name;}}''')
 write(f'src/{block}/BlockState.java','package dev.alexco.minecraft.world.level.block;public record BlockState(Block b){public Block getBlock(){return b;}}')
 write(f'src/{block}/Blocks.java','''package dev.alexco.minecraft.world.level.block;import java.util.*;public class Blocks {private static final Map<String,Block> ALL=new HashMap<>();public static Block byName(String n){return ALL.computeIfAbsent(n,x->new Block(x));}public static final Block AIR=byName("air"),STONE=byName("stone"),WATER=byName("water"),FLOWING_WATER=WATER;}''')
 write('src/dev/alexco/registry/ResourceLocation.java','package dev.alexco.registry;public record ResourceLocation(String name) {}')
 write('src/dev/alexco/minecraft/registry/Registry.java','''package dev.alexco.minecraft.registry;import dev.alexco.minecraft.world.level.block.*;import dev.alexco.registry.*;public class Registry {public static Registry BLOCK=new Registry();public Block get(ResourceLocation r){return Blocks.byName(r.name().replace("minecraft:",""));}}''')
 write('src/dev/alexco/minecraft/tag/BlockTags.java','''package dev.alexco.minecraft.tag;import java.util.*;import dev.alexco.minecraft.world.level.block.*;public class BlockTags {public static final Set<Block> FLUID=Set.of(Blocks.WATER);}''')
 bp='dev/alexco/minecraft/world/biome'
 manager=read('core/src/main/java/'+bp+'/BiomeManager.java');names=sorted(set(re.findall(r'Biomes\.([A-Z_]+)',manager)))
 write(f'src/{bp}/BiomeManager.java',manager)
 write(f'src/{bp}/Biome.java','package dev.alexco.minecraft.world.biome;public record Biome(String name){public String getName(){return name;}}')
 write(f'src/{bp}/Biomes.java','package dev.alexco.minecraft.world.biome;public class Biomes {'+''.join(f'public static final Biome {name}=new Biome("minecraft:{name.lower()}");' for name in names)+'}')
 write('src/dev/alexco/minecraft/world/level/chunk/Chunk.java','''package dev.alexco.minecraft.world.level.chunk;import dev.alexco.minecraft.world.level.block.*;public class Chunk {public final BlockState[][] blocks=new BlockState[16][384];public int origin;public Chunk(int x){origin=x;}public Position getChunkPos(){return new Position(origin/16);}public static class Position {public int x;public Position(int v){x=v;}}public BlockState getBlockAt(int x,int y){return blocks[x][y];}public void setBlockAt(int x,int y,BlockState m){blocks[x][y]=m;}}''')
 # Use chunk-aligned X, matching the source SurfaceSystem's world-X construction.
 write('src/Oracle.java','''import com.google.gson.*;import java.util.*;import dev.alexco.minecraft.world.level.levelgen.vanilla.*;import dev.alexco.minecraft.world.level.chunk.*;import dev.alexco.minecraft.world.level.block.*;import dev.alexco.minecraft.world.biome.*;
public class Oracle {
public static void main(String[]args)throws Exception {var out=new JsonObject();var rows=new JsonArray();var bands=new JsonArray();
for(long seed:new long[]{0,1,-928341,Long.MIN_VALUE,Long.MAX_VALUE}) {
 var engine=VanillaSurfaceRuleEngine.get(seed);var config=VanillaWorldgenConfig.get(seed);var system=new SurfaceSystem(seed,config,VanillaNoiseStep.get(seed));var band=SurfaceSystem.class.getDeclaredMethod("getBand",int.class,int.class);band.setAccessible(true);
 for(int x:new int[]{-16,0,32}){var values=new JsonArray();values.add(seed);values.add(x);var b=new JsonArray();for(int y=0;y<192;y++)b.add(((BlockState)band.invoke(system,x,y)).getBlock().name);values.add(b);bands.add(values);}
 for(String biome:new String[]{"plains","desert","beach","stony_shore","frozen_ocean","warm_ocean","badlands","wooded_badlands","snowy_slopes","frozen_peaks","swamp","mangrove_swamp","old_growth_pine_taiga","mushroom_fields","sulfur_caves"})
 for(String profile:new String[]{"dry","ocean","cave","thin","steep","deep","wetland"})for(int origin:new int[]{-16,0,32}) {
  var chunk=new Chunk(origin);int top=profile.equals("wetland")?63:profile.equals("ocean")?54:profile.equals("thin")?90:profile.equals("deep")?80:64;
  VanillaNoiseStep.height=top-1+64;VanillaNoiseStep.steep=profile.equals("steep");
  for(int x=0;x<16;x++)for(int y=0;y<384;y++) {int gy=y-64;var m=gy<top?Blocks.STONE:Blocks.AIR;if(profile.equals("thin")&&gy<88)m=Blocks.AIR;
   if(profile.equals("ocean")&&gy>=54&&gy<63)m=Blocks.WATER;
   if(profile.equals("cave")&&gy>=58&&gy<=60)m=Blocks.AIR;
   if(profile.equals("deep")&&gy>=-36&&gy<=-28)m=Blocks.AIR;
   chunk.setBlockAt(x,y,m.defaultBlockState());}
  engine.applyToChunk(chunk,(x,y)->new Biome("minecraft:"+biome));
  var r=new JsonArray();r.add(seed);r.add(biome);r.add(profile);r.add(origin);r.add(top);var samples=new JsonArray();
  for(int y:new int[]{-64,-63,-62,-61,-60,-59,-55,-40,-37,-28,-27,-1,0,1,2,4,7,8,45,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,75,79,87,88,89,90}) {var s=new JsonArray();s.add(y);s.add(chunk.getBlockAt(0,y+64).getBlock().name);samples.add(s);}r.add(samples);rows.add(r);
 }
}
var climate=new JsonArray();var rand=new Random(1729);for(int i=0;i<512;i++){double t=rand.nextDouble()*2-1,h=rand.nextDouble()*2-1,c=rand.nextDouble()*2-1,e=rand.nextDouble()*2-1,w=rand.nextDouble()*2-1;var col=BiomeManager.sampleColumn(c,t,h,w,e);for(double d:new double[]{0,.2,.5,.9,1,1.1,1.4}) {var r=new JsonArray();r.add(t);r.add(h);r.add(c);r.add(e);r.add(d);r.add(w);r.add(col.sample(d).getName().replace("minecraft:",""));climate.add(r);}}
out.add("biomes",climate);out.add("columns",rows);out.add("bands",bands);System.out.println(out);}}
''')
 sources=list(map(str,(root/'src').rglob('*.java')))
 subprocess.run(['javac','-cp',ref.GSON,'-d',str(root/'classes'),*sources],check=True)
 result=json.loads(subprocess.check_output(['java','-cp',f'{root/"classes"}:{ref.GSON}','Oracle'],cwd=root,text=True))
 (folder.parent/'assets/worldgen/java-surface.json').write_text(json.dumps(result,separators=(',',':'))+'\n')
 print('Java surface fixtures:',len(result['columns']),'columns;',len(result['bands']),'terracotta band sequences')
