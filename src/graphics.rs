#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    Scale,
    Ao,
    Shadows,
    Bloom,
    Limit,
    Preset,
    Occlusion,
    Reset,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub scale: u32,
    /// 0 = baked voxel AO only, 1 = inexpensive SSAO, 2 = full SSAO.
    pub ao: u32,
    pub shadows: u32,
    pub bloom: bool,
    pub limit: u32,
    pub occlusion: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            scale: 100,
            ao: 2,
            shadows: 4096,
            bloom: true,
            limit: 60,
            occlusion: true,
        }
    }
}
impl Settings {
    pub fn from_args(args: &[String]) -> Result<Self, Box<dyn std::error::Error>> {
        let mut settings = Self::default();
        if args.iter().any(|a| a == "--laptop") {
            settings.change(Change::Preset);
        }
        for (flag, choices, destination) in [
            (
                "--render-scale",
                &[50, 67, 75, 85, 100][..],
                &mut settings.scale,
            ),
            (
                "--shadow-resolution",
                &[1024, 2048, 4096][..],
                &mut settings.shadows,
            ),
            (
                "--fps-limit",
                &[0, 30, 45, 60, 90, 120, 144][..],
                &mut settings.limit,
            ),
        ] {
            if let Some(index) = args.iter().position(|a| a == flag) {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{flag} needs a value"))?
                    .parse::<u32>()?;
                if !choices.contains(&value) {
                    return Err(format!("{flag} must be one of {choices:?}").into());
                }
                *destination = value;
            }
        }
        if let Some(index) = args.iter().position(|a| a == "--ao-quality") {
            settings.ao = match args.get(index + 1).map(String::as_str) {
                Some("off") => 0,
                Some("low") => 1,
                Some("high") => 2,
                _ => return Err("--ao-quality expects off, low or high".into()),
            };
        }
        settings.bloom &= !args.iter().any(|a| a == "--no-bloom");
        settings.occlusion &= !args.iter().any(|a| a == "--no-occlusion");
        Ok(settings)
    }
    pub fn size(self, size: (u32, u32)) -> (u32, u32) {
        (
            (size.0 * self.scale / 100).max(1),
            (size.1 * self.scale / 100).max(1),
        )
    }
    pub fn change(&mut self, change: Change) {
        fn next(value: u32, choices: &[u32]) -> u32 {
            choices[(choices.iter().position(|&v| v == value).unwrap_or(0) + 1) % choices.len()]
        }
        match change {
            Change::Scale => self.scale = next(self.scale, &[50, 67, 75, 85, 100]),
            Change::Ao => self.ao = next(self.ao, &[0, 1, 2]),
            Change::Shadows => self.shadows = next(self.shadows, &[1024, 2048, 4096]),
            Change::Bloom => self.bloom = !self.bloom,
            Change::Limit => self.limit = next(self.limit, &[30, 45, 60, 90, 120, 144, 0]),
            Change::Occlusion => self.occlusion = !self.occlusion,
            Change::Reset => *self = Self::default(),
            Change::Preset => {
                *self = Self {
                    scale: 75,
                    ao: 1,
                    shadows: 2048,
                    bloom: true,
                    limit: 60,
                    occlusion: true,
                }
            }
        }
    }
    pub fn interval(self, captured: bool, focused: bool) -> Option<std::time::Duration> {
        let cap = if !focused {
            10
        } else if !captured {
            30
        } else {
            self.limit
        };
        (cap > 0).then(|| std::time::Duration::from_secs_f64(1.0 / cap as f64))
    }
    pub fn label(self, change: Change) -> String {
        match change {
            Change::Scale => format!("Render scale: {}%", self.scale),
            Change::Ao => format!(
                "Screen AO: {}",
                ["off (voxel AO)", "low", "high"][self.ao as usize]
            ),
            Change::Shadows => format!("Shadow resolution: {}", self.shadows),
            Change::Bloom => format!("Bloom: {}", if self.bloom { "on" } else { "off" }),
            Change::Limit => {
                if self.limit == 0 {
                    "FPS limit: unlimited".into()
                } else {
                    format!("FPS limit: {}", self.limit)
                }
            }
            Change::Occlusion => format!(
                "Terrain occlusion: {}",
                if self.occlusion { "on" } else { "off" }
            ),
            Change::Preset => "Laptop preset".into(),
            Change::Reset => "High quality defaults".into(),
        }
    }
}
#[cfg(test)]
#[test]
fn sizing_and_background_caps() {
    let mut g = Settings::default();
    assert_eq!(g.size((1440, 900)), (1440, 900));
    g.change(Change::Preset);
    assert_eq!(g.size((1440, 900)), (1080, 675));
    assert_eq!(g.size((1, 1)), (1, 1));
    g.limit = 0;
    assert!(g.interval(true, true).is_none());
    assert!((g.interval(false, true).unwrap().as_secs_f64() - 1.0 / 30.0).abs() < 1e-8);
    assert_eq!(g.interval(true, false).unwrap().as_secs_f64(), 0.1);
}
