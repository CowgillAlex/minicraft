/// Fine meshes remain bounded even when fog is disabled. The extra cache margin
/// covers movement within a two-metre chunk before the next streaming request.
pub const CACHE_MARGIN: f32 = 4.0;
pub const DISTANCES: [u16; 10] = [12, 16, 24, 32, 48, 64, 96, 128, 192, 256];
const DENSITIES: [u16; 7] = [25, 50, 75, 100, 125, 150, 200];
const STARTS: [f32; 8] = [0.0, 0.25, 0.5, 0.65, 0.75, 5.0 / 6.0, 0.9, 0.95];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    Distance(i8),
    Density(i8),
    Start(i8),
    Fog,
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewSettings {
    distance: usize,
    density: usize,
    start: usize,
    pub fog: bool,
}
impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            distance: 2,
            density: 3,
            start: 5,
            fog: true,
        }
    }
}
impl ViewSettings {
    pub fn distance(self) -> f32 {
        f32::from(DISTANCES[self.distance])
    }
    pub fn cache_radius(self) -> f32 {
        self.distance() + CACHE_MARGIN
    }
    pub fn density(self) -> u16 {
        DENSITIES[self.density]
    }
    pub fn fog_start(self) -> f32 {
        self.distance() * STARTS[self.start]
    }
    pub fn uniforms(self) -> [f32; 4] {
        [
            self.distance(),
            self.fog_start(),
            f32::from(self.density()) / 100.0,
            f32::from(u8::from(self.fog)),
        ]
    }
    pub fn set_distance(&mut self, distance: u16) -> Result<(), &'static str> {
        self.distance = DISTANCES
            .iter()
            .position(|v| *v == distance)
            .ok_or("Render distance must be 12, 16, 24, 32, 48, 64, 96, 128, 192 or 256 metres")?;
        Ok(())
    }
    pub fn change(&mut self, change: Change) {
        let adjust = |index: usize, delta: i8, len: usize| {
            index.saturating_add_signed(isize::from(delta)).min(len - 1)
        };
        match change {
            Change::Distance(delta) => {
                self.distance = adjust(self.distance, delta, DISTANCES.len())
            }
            Change::Density(delta) => self.density = adjust(self.density, delta, DENSITIES.len()),
            Change::Start(delta) => self.start = adjust(self.start, delta, STARTS.len()),
            Change::Fog => self.fog = !self.fog,
            Change::Reset => *self = Self::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distance_and_fog_stay_independent_and_clamped() {
        let mut settings = ViewSettings::default();
        assert_eq!(settings.uniforms(), [24.0, 20.0, 1.0, 1.0]);
        settings.change(Change::Fog);
        settings.change(Change::Distance(127));
        assert_eq!(settings.distance(), 256.0);
        assert!(!settings.fog);
        settings.change(Change::Density(-127));
        settings.change(Change::Start(127));
        assert_eq!(settings.density(), 25);
        assert!(settings.fog_start() < settings.distance());
        assert_eq!(settings.cache_radius(), 260.0);
        settings.change(Change::Distance(-127));
        assert_eq!(settings.distance(), 12.0);
        assert!(!settings.fog);
        settings.change(Change::Reset);
        assert_eq!(settings, ViewSettings::default());
        assert!(settings.set_distance(30).is_err());
        assert_eq!(settings.distance(), 24.0);
    }
}
