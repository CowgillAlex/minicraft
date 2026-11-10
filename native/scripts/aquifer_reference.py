"""Restore XYZ coordinates in the Java oracle without using Rust's candidate cache."""

def adapt(java):
    java = java.replace("Map<Integer, Integer> preliminarySurfaceCache", "Map<String, Integer> preliminarySurfaceCache")
    java = java.replace("record GridKey(int x, int y)", "record GridKey(int x, int y, int z)")
    java = java.replace("record Location(int x, int y)", "record Location(int x, int y, int z)")
    start = java.index("        int xAnchor =")
    end = java.index("        FluidStatus closestStatus1", start)
    java = java[:start] + '''        if (blockY < Math.min(-54, seaLevel)) return Blocks.AIR.defaultBlockState();
        int xAnchor=gridX(blockX-5), yAnchor=gridY(blockY+1), zAnchor=gridX(blockZ-5);
        Candidate[] nearest=new Candidate[4];
        double[] distances={Double.POSITIVE_INFINITY,Double.POSITIVE_INFINITY,Double.POSITIVE_INFINITY,Double.POSITIVE_INFINITY};
        for(int dx=0;dx<=1;dx++) for(int dy=-1;dy<=1;dy++) for(int dz=0;dz<=1;dz++) {
            GridKey key=new GridKey(xAnchor+dx,yAnchor+dy,zAnchor+dz);
            Location location=getLocation(key);
            double x=location.x()-blockX, y=location.y()-blockY, z=location.z()-blockZ;
            insertNearest(nearest,distances,new Candidate(key,location),x*x+y*y+z*z);
        }
\n''' + java[end:]
    java = java.replace("blockX, blockY, closest", "blockX, blockY, blockZ, closest")
    java = java.replace("computeFluid(location.x(), location.y())", "computeFluid(location.x(), location.y(), location.z())")
    java = java.replace('"cell/" + key.x() + "/" + key.y()', '"cell/" + key.x() + "/" + key.y() + "/" + key.z()')
    java = java.replace("fromGridY(key.y(), random.nextInt(9)))", "fromGridY(key.y(), random.nextInt(9)), fromGridX(key.z(), random.nextInt(10)))")
    java = java.replace("computeFluid(int x, int y)", "computeFluid(int x, int y, int z)")
    java = java.replace("for (int xOffset : SURFACE_SAMPLING_X_OFFSETS)", "for (int[] offset : SURFACE_SAMPLES)")
    java = java.replace("int sampleX = x + xOffset * 16;", "int sampleX=x+offset[0]*16, sampleZ=z+offset[1]*16;")
    java = java.replace("preliminarySurfaceLevel(sampleX)", "preliminarySurfaceLevel(sampleX, sampleZ)")
    java = java.replace("boolean center = xOffset == 0", "boolean center = offset[0] == 0 && offset[1] == 0")
    java = java.replace("sampleFluidLevelFloodedness(x, y, 0)", "sampleFluidLevelFloodedness(x, y, z)")
    java = java.replace("Math.floorDiv(x, 16), fluidLevelCellY, 0", "Math.floorDiv(x, 16), fluidLevelCellY, Math.floorDiv(z,16)")
    start = java.index("    private int preliminarySurfaceLevel(")
    end = java.index("    private FluidStatus globalFluid", start)
    java = java[:start] + '''    private int preliminarySurfaceLevel(int x,int z) {
        int qx=Math.floorDiv(x,4)*4,qz=Math.floorDiv(z,4)*4;
        String key=qx+"/"+qz;
        if(preliminarySurfaceCache.containsKey(key)) return preliminarySurfaceCache.get(key);
        int result=worldgen.getMinY();
        for(int y=worldgen.getWorldTopYExclusive();y>=worldgen.getMinY();y-=8) {
            if(noiseStep.sampleInitialDensityWithoutJaggedness(qx,y,qz)>0.390625) { result=y; break; }
        }
        preliminarySurfaceCache.put(key,result); return result;
    }

''' + java[end:]
    java = java.replace("calculatePressure(double blockX, double blockY,", "calculatePressure(double blockX, double blockY, double blockZ,")
    java = java.replace("sampleBarrier(blockX, blockY, 0)", "sampleBarrier(blockX, blockY, blockZ)")
    java = java.replace("    private static final int X_SPACING", "    private static final int[][] SURFACE_SAMPLES={{-2,-1},{-1,-1},{0,-1},{1,-1},{-3,0},{-2,0},{-1,0},{0,0},{1,0},{-2,1},{-1,1},{0,1},{1,1}};\n    private static final int X_SPACING")
    return java
