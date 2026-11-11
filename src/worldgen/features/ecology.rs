//! Every biome in the pinned table has an explicit ecological treatment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Habitat {
    Meadow,
    Sunflower,
    Bamboo,
    Beach,
    Forest,
    Birch,
    Cherry,
    Dark,
    Pale,
    Pine,
    Snow,
    Jungle,
    Savanna,
    Desert,
    Badlands,
    Alpine,
    Wetland,
    Mushroom,
    River,
    FrozenRiver,
    Ocean,
    ColdOcean,
    Reef,
    Lush,
    Dripstone,
    DeepDark,
    Sulfur,
}
pub fn habitat(name: &str) -> Habitat {
    use Habitat::*;
    match name {
        "plains" | "meadow" => Meadow,
        "sunflower_plains" => Sunflower,
        "bamboo_jungle" => Bamboo,
        "beach" => Beach,
        "forest" | "flower_forest" | "dappled_forest" | "windswept_forest" => Forest,
        "birch_forest" | "old_growth_birch_forest" => Birch,
        "cherry_grove" => Cherry,
        "dark_forest" => Dark,
        "pale_garden" => Pale,
        "taiga" | "old_growth_pine_taiga" | "old_growth_spruce_taiga" => Pine,
        "snowy_taiga" | "grove" | "snowy_plains" | "snowy_slopes" => Snow,
        "jungle" | "sparse_jungle" => Jungle,
        "savanna" | "savanna_plateau" | "windswept_savanna" => Savanna,
        "desert" => Desert,
        "badlands" | "eroded_badlands" | "wooded_badlands" => Badlands,
        "frozen_peaks"
        | "jagged_peaks"
        | "stony_peaks"
        | "windswept_gravelly_hills"
        | "windswept_hills"
        | "stony_shore"
        | "snowy_beach"
        | "ice_spikes" => Alpine,
        "swamp" | "mangrove_swamp" => Wetland,
        "mushroom_fields" => Mushroom,
        "river" => River,
        "frozen_river" => FrozenRiver,
        "ocean" | "deep_ocean" | "lukewarm_ocean" | "deep_lukewarm_ocean" => Ocean,
        "cold_ocean" | "deep_cold_ocean" | "frozen_ocean" | "deep_frozen_ocean" => ColdOcean,
        "warm_ocean" => Reef,
        "lush_caves" => Lush,
        "dripstone_caves" => Dripstone,
        "deep_dark" => DeepDark,
        "sulfur_caves" => Sulfur,
        _ => panic!("Missing biome feature profile: {name}"),
    }
}
impl Habitat {
    pub fn trees(self) -> f64 {
        use Habitat::*;
        match self {
            Forest | Birch | Cherry | Pine | Dark | Pale | Jungle | Bamboo => 0.72,
            Wetland => 0.4,
            Snow => 0.28,
            Savanna => 0.22,
            Meadow | Sunflower => 0.06,
            _ => 0.0,
        }
    }
    pub fn aquatic(self) -> bool {
        matches!(
            self,
            Self::River | Self::FrozenRiver | Self::Ocean | Self::ColdOcean | Self::Reef
        )
    }
    pub fn cave(self) -> bool {
        matches!(
            self,
            Self::Lush | Self::Dripstone | Self::DeepDark | Self::Sulfur
        )
    }
}
