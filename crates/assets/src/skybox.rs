//! Optional pack sky textures, independent of the required atmosphere carrier.

/// Numbered faces in the pack's authored order.
pub const SKYBOX_FACE_COUNT: usize = 6;
/// Fresh settings preserve Bedrock’s ordinary skybox translation.
pub const DEFAULT_NATIVE_SKYBOX_OFFSET: bool = true;

/// Dimensions with distinct environment texture namespaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum SkyboxDimension {
    Overworld,
    Nether,
    End,
}

impl SkyboxDimension {
    pub const ALL: [Self; 3] = [Self::Overworld, Self::Nether, Self::End];

    /// Texture prefix used by the dimension's numbered faces.
    pub const fn texture_prefix(self) -> &'static str {
        match self {
            Self::Overworld => "textures/environment/overworld_cubemap/cubemap_",
            Self::Nether => "textures/environment/nether_cubemap/cubemap_",
            Self::End => "textures/environment/theend_cubemap/cubemap_",
        }
    }
}

/// A decoded straight-alpha image; optional faces retain the ordinary sky beneath them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkyboxFace {
    pub width: u32,
    pub height: u32,
    pub rgba8: Box<[u8]>,
}

/// A dimension's numbered images; absent or invalid faces use the stock sky.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackSkybox {
    pub faces: [Option<SkyboxFace>; SKYBOX_FACE_COUNT],
}

impl PackSkybox {
    /// Encodes exactly the faces that can contribute to the background pass.
    pub fn face_mask(&self) -> u32 {
        self.faces
            .iter()
            .enumerate()
            .fold(0, |mask, (face, image)| {
                mask | ((image.is_some() as u32) << face)
            })
    }
}

/// The most Java sky layers a pack's overworld may draw; later layers are dropped.
pub const MAX_SKY_LAYERS: usize = 9;

/// How a sky layer combines with what is already drawn, after Java's sky blend modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkyBlend {
    /// The layer's alpha mixes it over the sky.
    Alpha,
    /// The layer's light is added to the sky.
    Add,
    /// The layer covers the sky.
    Replace,
    /// The layer darkens the sky by its colour.
    Multiply,
    /// The layer lightens the sky: `1 - (1 - sky) * (1 - layer)`.
    Screen,
    /// The layer's light is taken from the sky.
    Subtract,
}

impl SkyBlend {
    /// The blend named by a Java sky `blend` value; unknown modes are `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name.trim().to_ascii_lowercase().as_str() {
            "alpha" => Self::Alpha,
            "add" => Self::Add,
            "replace" => Self::Replace,
            "multiply" => Self::Multiply,
            "screen" => Self::Screen,
            "subtract" => Self::Subtract,
            _ => return None,
        })
    }

    /// The name [`Self::from_name`] reads.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Alpha => "alpha",
            Self::Add => "add",
            Self::Replace => "replace",
            Self::Multiply => "multiply",
            Self::Screen => "screen",
            Self::Subtract => "subtract",
        }
    }
}

/// Weather a sky layer may be shown in, as a bit set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SkyWeather(pub u8);

impl SkyWeather {
    pub const CLEAR: u8 = 1;
    pub const RAIN: u8 = 2;
    pub const THUNDER: u8 = 4;
    /// A layer with no weather condition shows in all weather.
    pub const ALL: Self = Self(Self::CLEAR | Self::RAIN | Self::THUNDER);

    /// The weather bit a level of rain and thunder (each 0 to 1) counts as.
    pub fn of_levels(rain: f32, thunder: f32) -> u8 {
        if thunder > 0.5 {
            Self::THUNDER
        } else if rain > 0.5 {
            Self::RAIN
        } else {
            Self::CLEAR
        }
    }
}

/// The ticks in a day.
pub const SKY_DAY_TICKS: f32 = 24_000.0;

/// A Java clock time in hours (`19:20` is `19.33`) as ticks into the day, where tick 0 is 06:00.
pub fn sky_ticks_from_hours(hours: f32) -> f32 {
    ((hours - 6.0) * 1_000.0).rem_euclid(SKY_DAY_TICKS)
}

/// When a sky layer fades in and out, as ticks into the day.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyFade {
    pub start_in: f32,
    pub end_in: f32,
    pub start_out: f32,
    pub end_out: f32,
}

impl SkyFade {
    /// The layer's opacity at `tick` (any value; reduced to the day): ramping up from `start_in`
    /// to `end_in`, full until `start_out`, ramping down to `end_out`, else none.
    pub fn alpha(&self, tick: f32) -> f32 {
        let since = |at: f32| (at - self.start_in).rem_euclid(SKY_DAY_TICKS);
        let (up, hold, out) = (since(self.end_in), since(self.start_out), since(self.end_out));
        let now = since(tick);
        if now < up {
            now / up
        } else if now < hold.max(up) {
            1.0
        } else if now < out.max(hold) {
            1.0 - (now - hold) / (out - hold)
        } else {
            0.0
        }
    }
}

/// The days a layer shows on, as inclusive ranges, repeating every `repeat` days when it is set.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SkyDays {
    pub ranges: Vec<(u32, u32)>,
    pub repeat: Option<u32>,
}

impl SkyDays {
    /// Whether the layer shows on world day `day`.
    pub fn contains(&self, day: u64) -> bool {
        let day = match self.repeat {
            Some(repeat) if repeat > 0 => day % u64::from(repeat),
            _ => day,
        };
        self.ranges
            .iter()
            .any(|&(first, last)| (u64::from(first)..=u64::from(last)).contains(&day))
    }
}

/// A layer turning about `axis` as the day passes: `speed` times the sun's own motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyRotation {
    pub speed: f32,
    pub axis: [f32; 3],
}

/// When and how a Java sky layer is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct SkySchedule {
    /// How the layer combines with the sky.
    pub blend: SkyBlend,
    /// Always shown when absent.
    pub fade: Option<SkyFade>,
    pub rotation: Option<SkyRotation>,
    pub weather: SkyWeather,
    pub days: Option<SkyDays>,
}

impl Default for SkySchedule {
    fn default() -> Self {
        Self {
            blend: SkyBlend::Add,
            fade: None,
            rotation: None,
            weather: SkyWeather::ALL,
            days: None,
        }
    }
}

impl SkySchedule {
    /// The layer's opacity now: `tick` into the day (tick 0 is 06:00), the world `day` and the
    /// `weather` bit (see [`SkyWeather::of_levels`]). Zero means the layer is not drawn.
    pub fn alpha(&self, tick: f32, day: u64, weather: u8) -> f32 {
        if self.weather.0 & weather == 0 {
            return 0.0;
        }
        if self.days.as_ref().is_some_and(|days| !days.contains(day)) {
            return 0.0;
        }
        self.fade.map_or(1.0, |fade| fade.alpha(tick))
    }
}

/// One Java sky layer: its cube faces and when it is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct SkyLayer {
    pub faces: [Option<SkyboxFace>; SKYBOX_FACE_COUNT],
    pub schedule: SkySchedule,
}

#[cfg(test)]
mod layer_tests {
    use super::*;

    fn dramatic_stars() -> SkyFade {
        SkyFade {
            start_in: sky_ticks_from_hours(19.0 + 20.0 / 60.0),
            end_in: sky_ticks_from_hours(19.0 + 40.0 / 60.0),
            start_out: sky_ticks_from_hours(4.0 + 20.0 / 60.0),
            end_out: sky_ticks_from_hours(4.0 + 40.0 / 60.0),
        }
    }

    #[test]
    fn java_clock_times_count_from_six_in_the_morning() {
        assert_eq!(sky_ticks_from_hours(6.0), 0.0);
        assert_eq!(sky_ticks_from_hours(12.0), 6_000.0);
        assert_eq!(sky_ticks_from_hours(0.0), 18_000.0);
        assert!((sky_ticks_from_hours(19.0 + 20.0 / 60.0) - 13_333.33).abs() < 0.1);
    }

    #[test]
    fn a_fade_that_wraps_midnight_is_full_at_night_and_gone_by_day() {
        let fade = dramatic_stars();
        assert_eq!(fade.alpha(18_000.0), 1.0, "midnight");
        assert_eq!(fade.alpha(6_000.0), 0.0, "noon");
        assert_eq!(fade.alpha(0.0), 0.0, "dawn");
        let middle = (fade.start_in + fade.end_in) / 2.0;
        assert!((fade.alpha(middle) - 0.5).abs() < 1e-3, "halfway through the fade-in");
        let out = (fade.start_out + fade.end_out) / 2.0;
        assert!((fade.alpha(out) - 0.5).abs() < 1e-3, "halfway through the fade-out");
        assert_eq!(fade.alpha(fade.start_in - 1.0), 0.0);
        assert_eq!(fade.alpha(fade.end_out + 1.0), 0.0);
    }

    #[test]
    fn a_daytime_fade_does_not_wrap() {
        let fade = SkyFade {
            start_in: sky_ticks_from_hours(5.0 + 40.0 / 60.0),
            end_in: sky_ticks_from_hours(6.0 + 20.0 / 60.0),
            start_out: sky_ticks_from_hours(17.0 + 40.0 / 60.0),
            end_out: sky_ticks_from_hours(18.0 + 20.0 / 60.0),
        };
        assert_eq!(fade.alpha(6_000.0), 1.0, "noon");
        assert_eq!(fade.alpha(18_000.0), 0.0, "midnight");
    }

    #[test]
    fn a_layer_with_no_conditions_always_shows() {
        let schedule = SkySchedule::default();
        for tick in [0.0, 6_000.0, 13_000.0, 18_000.0] {
            assert_eq!(schedule.alpha(tick, 0, SkyWeather::CLEAR), 1.0);
        }
    }

    #[test]
    fn weather_and_days_gate_a_layer() {
        let schedule = SkySchedule {
            weather: SkyWeather(SkyWeather::CLEAR),
            days: Some(SkyDays {
                ranges: vec![(0, 1), (5, 5)],
                repeat: Some(8),
            }),
            ..SkySchedule::default()
        };
        assert_eq!(schedule.alpha(0.0, 0, SkyWeather::CLEAR), 1.0);
        assert_eq!(schedule.alpha(0.0, 0, SkyWeather::RAIN), 0.0);
        assert_eq!(schedule.alpha(0.0, 3, SkyWeather::CLEAR), 0.0);
        assert_eq!(schedule.alpha(0.0, 13, SkyWeather::CLEAR), 1.0, "day 13 is day 5 of 8");
        assert_eq!(SkyWeather::of_levels(0.0, 0.0), SkyWeather::CLEAR);
        assert_eq!(SkyWeather::of_levels(0.9, 0.0), SkyWeather::RAIN);
        assert_eq!(SkyWeather::of_levels(0.9, 0.9), SkyWeather::THUNDER);
    }

    #[test]
    fn blend_names_round_trip() {
        for blend in [
            SkyBlend::Alpha,
            SkyBlend::Add,
            SkyBlend::Replace,
            SkyBlend::Multiply,
            SkyBlend::Screen,
            SkyBlend::Subtract,
        ] {
            assert_eq!(SkyBlend::from_name(blend.name()), Some(blend));
        }
        assert_eq!(SkyBlend::from_name("Overlay"), None);
    }
}
