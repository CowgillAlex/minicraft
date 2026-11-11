//! Stable material IDs are shared by generated terrain, editing and the renderer.
//! IDs 0–15 retain the existing village and shader meanings.
pub struct Material {
    pub id: u8,
    pub name: &'static str,
    pub color: [f32; 3],
    pub source: &'static str,
}
macro_rules! materials {
    ($(($id:literal,$constant:ident,$name:literal,$source:literal,$color:expr)),* $(,)?)=>{
        $(pub const $constant:u8=$id;)*
        pub const ALL:&[Material]=&[$(Material{id:$constant,name:$name,source:$source,color:$color}),*];
    }
}
materials![
    (0, AIR, "Air", "air", [0.0; 3]),
    (1, GRASS, "Grass", "grass_block", [0.27, 0.46, 0.26]),
    (2, DIRT, "Dirt", "dirt", [0.32, 0.23, 0.17]),
    (3, STONE, "Stone", "stone", [0.43, 0.47, 0.46]),
    (4, ROAD, "Asphalt", "asphalt", [0.16, 0.20, 0.22]),
    (5, PAINT, "Road paint", "paint", [0.93, 0.72, 0.35]),
    (6, WOOD, "Wood", "wood", [0.34, 0.22, 0.14]),
    (7, LEAF, "Leaves", "leaves", [0.22, 0.43, 0.28]),
    (8, ROOF, "Roof", "roof", [0.32, 0.39, 0.43]),
    (9, LIGHT, "Lamp", "lamp", [1.0, 0.69, 0.28]),
    (10, WATER, "Water", "water", [0.15, 0.43, 0.47]),
    (11, FLOWER, "Flower", "flower", [0.86, 0.58, 0.39]),
    (12, WALL, "Plaster", "plaster", [0.77, 0.68, 0.50]),
    (13, SAND, "Sand", "sand", [0.80, 0.73, 0.49]),
    (14, GLASS, "Glass", "glass", [0.55, 0.77, 0.82]),
    (15, HANDLE, "Handle", "handle", [0.82, 0.62, 0.25]),
    (16, GRAVEL, "Gravel", "gravel", [0.47, 0.44, 0.41]),
    (17, SANDSTONE, "Sandstone", "sandstone", [0.72, 0.63, 0.39]),
    (18, RED_SAND, "Red sand", "red_sand", [0.71, 0.35, 0.16]),
    (
        19,
        RED_SANDSTONE,
        "Red sandstone",
        "red_sandstone",
        [0.63, 0.29, 0.13]
    ),
    (20, DEEPSLATE, "Deepslate", "deepslate", [0.23, 0.25, 0.27]),
    (21, BEDROCK, "Bedrock", "bedrock", [0.18, 0.19, 0.19]),
    (22, CALCITE, "Calcite", "calcite", [0.83, 0.82, 0.75]),
    (
        23,
        COARSE_DIRT,
        "Coarse dirt",
        "coarse_dirt",
        [0.40, 0.30, 0.20]
    ),
    (24, MUD, "Mud", "mud", [0.26, 0.26, 0.29]),
    (25, MYCELIUM, "Mycelium", "mycelium", [0.48, 0.39, 0.45]),
    (26, PODZOL, "Podzol", "podzol", [0.47, 0.29, 0.13]),
    (27, SNOW, "Snow", "snow_block", [0.92, 0.96, 0.97]),
    (
        28,
        POWDER_SNOW,
        "Powder snow",
        "powder_snow",
        [0.82, 0.90, 0.96]
    ),
    (29, ICE, "Ice", "ice", [0.48, 0.68, 0.86]),
    (
        30,
        PACKED_ICE,
        "Packed ice",
        "packed_ice",
        [0.54, 0.72, 0.87]
    ),
    (
        31,
        TERRACOTTA,
        "Terracotta",
        "terracotta",
        [0.60, 0.37, 0.27]
    ),
    (
        32,
        ORANGE_TERRACOTTA,
        "Orange terracotta",
        "orange_terracotta",
        [0.69, 0.35, 0.17]
    ),
    (
        33,
        YELLOW_TERRACOTTA,
        "Yellow terracotta",
        "yellow_terracotta",
        [0.76, 0.54, 0.20]
    ),
    (
        34,
        BROWN_TERRACOTTA,
        "Brown terracotta",
        "brown_terracotta",
        [0.37, 0.25, 0.17]
    ),
    (
        35,
        RED_TERRACOTTA,
        "Red terracotta",
        "red_terracotta",
        [0.58, 0.26, 0.19]
    ),
    (
        36,
        WHITE_TERRACOTTA,
        "White terracotta",
        "white_terracotta",
        [0.83, 0.71, 0.63]
    ),
    (
        37,
        GRAY_TERRACOTTA,
        "Light gray terracotta",
        "light_gray_terracotta",
        [0.54, 0.43, 0.39]
    ),
    (38, SULFUR, "Sulfur", "sulfur", [0.86, 0.79, 0.19]),
    (39, CINNABAR, "Cinnabar", "cinnabar", [0.66, 0.18, 0.14]),
];
pub fn get(id: u8) -> &'static Material {
    ALL.get(id as usize).unwrap_or(&ALL[0])
}
pub fn from_source(name: &str) -> Result<u8, String> {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    ALL.iter()
        .find(|m| m.source == name)
        .map(|m| m.id)
        .ok_or_else(|| format!("Unknown surface material {name}"))
}
pub fn face_color(material: u8, vertical: f32) -> [f32; 3] {
    if matches!(material, GRASS | PODZOL | MYCELIUM) && vertical < 0.5 {
        get(DIRT).color
    } else {
        get(material).color
    }
}
