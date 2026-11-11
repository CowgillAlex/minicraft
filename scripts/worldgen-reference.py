#!/usr/bin/env python3
"""Generate parity fixtures by compiling the pinned Java implementation, without LibGDX.

Needs a JDK and Gson, e.g. openjdk-21-jdk-headless and libgoogle-gson-java.
Usage: python3 scripts/worldgen-reference.py /path/to/Paper-Minecraft
Only the continuous fixture changes Java's coordinate/interpolation adapter.
"""
import json
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode = True
import tempfile

REVISION = "d681c0532d5e2628c70e6de7fad7df31f64ea705"
PACKAGE = "dev/alexco/minecraft/world/level/levelgen"
GSON = "/usr/share/java/gson.jar"
DESTINATION = Path(__file__).resolve().parent.parent / "assets/worldgen"

STUBS = {
    "com/badlogic/gdx/utils/JsonValue.java": '''package com.badlogic.gdx.utils;
public class JsonValue { public int size=0;
 public int getInt(String name,int fallback) { throw new UnsupportedOperationException(); }
 public JsonValue get(String name) { throw new UnsupportedOperationException(); }
 public JsonValue get(int index) { throw new UnsupportedOperationException(); }
 public boolean isArray() { throw new UnsupportedOperationException(); }
 public double asDouble() { throw new UnsupportedOperationException(); } }''',
    "com/badlogic/gdx/Gdx.java": '''package com.badlogic.gdx;
public class Gdx { public static Files files = new Files();
 public static class Files { public com.badlogic.gdx.files.FileHandle internal(String path) {
 return new com.badlogic.gdx.files.FileHandle(path); } } }''',
    "com/badlogic/gdx/files/FileHandle.java": '''package com.badlogic.gdx.files;
public class FileHandle { private java.nio.file.Path path;
 public FileHandle(String name) { path=java.nio.file.Path.of("assets",name); }
 public boolean exists() { return java.nio.file.Files.exists(path); }
 public String readString() { try { return java.nio.file.Files.readString(path); }
 catch(Exception e) { throw new RuntimeException(e); } } }''',
    "dev/alexco/minecraft/SharedConstants.java": '''package dev.alexco.minecraft;
public class SharedConstants { public static final int CHUNK_HEIGHT=384; }''',
    "dev/alexco/minecraft/util/Logger.java": '''package dev.alexco.minecraft.util;
public class Logger { public static void ERROR(String message, Object...args) {
 throw new IllegalStateException(String.format(message,args)); } }''',
    "dev/alexco/minecraft/world/level/block/BlockState.java": '''package dev.alexco.minecraft.world.level.block;
public class BlockState { private Blocks.Block block;
 public BlockState(Blocks.Block b) { block=b; } public Blocks.Block getBlock() { return block; } }''',
    "dev/alexco/minecraft/world/level/block/Blocks.java": '''package dev.alexco.minecraft.world.level.block;
public class Blocks { public static Block AIR=new Block(0), STONE=new Block(3), WATER=new Block(10);
 public static class Block { public int material; Block(int value) { material=value; }
 public BlockState defaultBlockState() { return new BlockState(this); } } }''',
    PACKAGE + "/vanilla/VanillaWorldgenConfig.java": '''package dev.alexco.minecraft.world.level.levelgen.vanilla;
import com.google.gson.*;
import dev.alexco.minecraft.world.level.levelgen.noise.*;
import dev.alexco.minecraft.world.level.levelgen.random.Xoroshiro;
import dev.alexco.minecraft.world.level.block.*;
public class VanillaWorldgenConfig {
 private long seed; private VanillaWorldgenConfig(long s) { seed=s; }
 public static VanillaWorldgenConfig get(long seed) { return new VanillaWorldgenConfig(seed); }
 public int getMinY() { return -64; } public int getHeight() { return 384; }
 public int getSeaLevel() { return 63; } public int getSeaLevelInternal() { return 127; }
 public int getWorldTopYExclusive() { return 320; }
 public BlockState getDefaultFluid() { return Blocks.WATER.defaultBlockState(); }
 public Noise createNoise(String name, NormalNoise.NoiseParameters fallback) {
  var file=com.badlogic.gdx.Gdx.files.internal("data/minecraft/worldgen/noise/"+name+".json");
  var obj=JsonParser.parseString(file.readString()).getAsJsonObject();
  int octave=obj.has("base_octave")?obj.get("base_octave").getAsInt():obj.get("firstOctave").getAsInt();
  var array=obj.has("amplitudes")?obj.getAsJsonArray("amplitudes"):obj.getAsJsonArray("amplitude_modifiers");
  int count=Math.max(array.size(),obj.has("octave_count")?obj.get("octave_count").getAsInt():0);
  int offset=obj.has("base_amplitude")?1:0; double[] amplitudes=new double[count+offset];
  if(offset!=0) amplitudes[0]=obj.get("base_amplitude").getAsDouble();
  for(int i=0;i<array.size();i++) amplitudes[i+offset]=array.get(i).getAsDouble();
  return new NormalNoise(new Xoroshiro(seed).fromHashOf("minecraft:"+name),new NormalNoise.NoiseParameters(octave,amplitudes));
 } }''',
    "Oracle.java": '''import dev.alexco.minecraft.world.level.levelgen.vanilla.*;
import dev.alexco.minecraft.world.level.levelgen.random.Xoroshiro;
import dev.alexco.minecraft.world.level.levelgen.noise.*;
import dev.alexco.minecraft.world.level.block.*;
import com.google.gson.*;
public class Oracle {
 public static void main(String[] args) throws Exception {
  var output=new JsonObject(); var rows=new JsonArray(); var randomRows=new JsonArray();
  for(long seed:new long[]{0,1,12345,-928341,Long.MIN_VALUE,Long.MAX_VALUE}) {
   var random=new Xoroshiro(seed).fromHashOf("minecraft:terrain");
   var rng=new JsonArray(); rng.add(seed); for(int i=0;i<12;i++) rng.add(Long.toUnsignedString(random.nextLong())); randomRows.add(rng);
   var noise=VanillaNoiseStep.get(seed); var aquifer=new VanillaAquifer(seed,noise,VanillaWorldgenConfig.get(seed));
   if(args.length==0) {
    for(int x:new int[]{-8193,-129,-65,-17,-5,-1,0,1,3,4,15,16,127,1024})
     for(int y:new int[]{-64,-63,-55,-54,-40,-1,0,7,8,31,48,60,62,63,64,65,70,80,96,128,192,255,319}) {
      double d=noise.sampleFinalDensity2D(x,y);
      BlockState state=aquifer.computeSubstance(x,y,0,d);
      var row=new JsonArray();row.add(seed);row.add(x);row.add(y);row.add(0);row.add(d);row.add(state==null?3:state.getBlock().material);rows.add(row);
     }
   } else {
    var method=VanillaNoiseStep.class.getMethod("continuous",double.class,double.class,double.class);
    for(double x:new double[]{-65.0625,-4.0625,-0.0625,0,0.0625,3.9375,4,4.0625,17.5625})
     for(double y:new double[]{-63.9375,-54.0625,0,7.9375,8,48.0625,62.9375,63,64.0625,80.125,319})
      for(double z:new double[]{-4.0625,-0.0625,0,0.0625,3.9375,4.0625}) {
       double d=(double)method.invoke(noise,x,y,z);
       var fluidMethod=VanillaAquifer.class.getMethod("computeSubstance",double.class,double.class,double.class,double.class);
       BlockState state=(BlockState)fluidMethod.invoke(aquifer,x,y,z,d);
       var row=new JsonArray();row.add(seed);row.add(x);row.add(y);row.add(z);row.add(d);row.add(state==null?3:state.getBlock().material);rows.add(row);
      }
   }
  }
  output.add("samples",rows);output.add("random",randomRows);
  System.out.println(new Gson().toJson(output));
 } }''',
}

INTERPOLATOR = '''    private static final class InterpolatedNode implements DensityNode {
        private final DensityNode argument;
        private final java.util.Map<String,double[]> cache=new java.util.HashMap<>();
        InterpolatedNode(DensityNode argument,int minY,int height) { this.argument=argument; }
        public double sample(Context p) {
            int cx=(int)Math.floor(p.x()/4), cy=(int)Math.floor(p.y()/8), cz=(int)Math.floor(p.z()/4);
            String key=cx+"/"+cy+"/"+cz;
            double[] v=cache.computeIfAbsent(key,ignored->{double[] a=new double[8];
                for(int i=0;i<8;i++) a[i]=argument.sample(new Context((cx+(i&1))*4,(cy+((i>>1)&1))*8,(cz+((i>>2)&1))*4));return a;});
            double tx=(p.x()-cx*4)/4, ty=(p.y()-cy*8)/8, tz=(p.z()-cz*4)/4;
            double a=(v[0]*(1-tx)+v[1]*tx)*(1-ty)+(v[2]*(1-tx)+v[3]*tx)*ty;
            if(tz==0) return a;
            double b=(v[4]*(1-tx)+v[5]*tx)*(1-ty)+(v[6]*(1-tx)+v[7]*tx)*ty;
            return a*(1-tz)+b*tz;
        }
    }

'''


def main():
    repo = Path(sys.argv[1]).resolve()
    def read(path):
        return subprocess.check_output(["git", "show", f"{REVISION}:{path}"], cwd=repo)
    with tempfile.TemporaryDirectory(prefix="minicraft-java-") as directory:
        root = Path(directory)
        def write(path, content):
            destination = root / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(content if isinstance(content, bytes) else content.encode())
        files = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", REVISION], cwd=repo, text=True).splitlines()
        for path in files:
            if path.startswith("assets/data/minecraft/worldgen/") and any(f"/{part}/" in path for part in ["density_function", "noise", "noise_settings"]):
                write(path, read(path))
        for path in ["random/Xoroshiro", "noise/Noise", "noise/ImprovedNoise", "noise/SimplexNoise", "noise/NormalNoise", "noise/PerlinNoise", "vanilla/VanillaNoiseStep", "vanilla/VanillaAquifer"]:
            write(f"src/{PACKAGE}/{path}.java", read(f"core/src/main/java/{PACKAGE}/{path}.java"))
        for path, content in STUBS.items():
            write(f"src/{path}", content)
        def run(name, *args):
            sources = [str(p) for p in (root / "src").rglob("*.java")]
            subprocess.run(["javac", "-cp", GSON, "-d", str(root / "classes"), *sources], check=True)
            output = subprocess.check_output(["java", "-cp", f"{root / 'classes'}:{GSON}", "Oracle", *args], cwd=root, text=True)
            parsed = json.loads(output)
            (DESTINATION / name).write_text(json.dumps(parsed, separators=(",", ":")) + "\n")
            print(name, len(parsed["samples"]), "samples")
        run("java-slice.json")
        source = root / f"src/{PACKAGE}/vanilla/VanillaNoiseStep.java"
        java = source.read_text().replace("record Context(int x, int y, int z)", "record Context(double x, double y, double z)")
        start = java.index("    private static final class InterpolatedNode")
        end = java.index("    private interface DensityNode", start)
        java = java[:start] + INTERPOLATOR + java[end:]
        java = java.replace("    public double sampleFinalDensity(int", "    public double continuous(double x,double y,double z) { return finalDensity.sample(new Context(x,y,z)); }\n\n    public double sampleFinalDensity(int")
        java = java.replace("sampleBarrier(int blockX, int blockY, int blockZ)", "sampleBarrier(double blockX, double blockY, double blockZ)")
        source.write_text(java)
        aquifer = root / f"src/{PACKAGE}/vanilla/VanillaAquifer.java"
        java = aquifer.read_text()
        java = java.replace("computeSubstance(int blockX, int blockY, int blockZ", "computeSubstance(double blockX, double blockY, double blockZ")
        java = java.replace("int[] distances", "double[] distances")
        java = java.replace("int dx = location.x() - blockX", "double dx = location.x() - blockX").replace("int dy = location.y() - blockY", "double dy = location.y() - blockY")
        java = java.replace("int distance = dx * dx + dy * dy", "double distance = dx * dx + dy * dy").replace("Candidate candidate, int distance", "Candidate candidate, double distance")
        java = java.replace("calculatePressure(int blockX, int blockY", "calculatePressure(double blockX, double blockY")
        java = java.replace("similarity(int distanceSqr1, int distanceSqr2)", "similarity(double distanceSqr1, double distanceSqr2)")
        java = java.replace("at(int blockY)", "at(double blockY)")
        for axis, spacing in [("X", "X_SPACING"), ("Y", "Y_SPACING")]:
            java = java.replace(f"grid{axis}(int blockCoord)", f"grid{axis}(double blockCoord)")
            java = java.replace(f"Math.floorDiv(blockCoord, {spacing})", f"(int)Math.floor(blockCoord / {spacing})")
        aquifer.write_text(java)
        run("java-continuous.json", "continuous")
        from aquifer_reference import adapt
        aquifer.write_text(adapt(aquifer.read_text()))
        run("java-aquifer3d.json", "continuous")


if __name__ == "__main__":
    main()
