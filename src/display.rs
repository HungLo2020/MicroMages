//! Machine-local fullscreen settings and aspect-preserving scaling.

/// The framebuffer is 256x240, so its display aspect ratio is 16:15.
pub const ASPECT: (u32, u32) = (16, 15);
use crate::video::{HEIGHT, WIDTH};
use serde::{Deserialize, Serialize};
use std::{io, path::Path};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scaling {
    /// The former integer mode also uses full-height scaling on load.
    #[default]
    #[serde(alias = "integer")]
    Fit,
    Smooth,
}
impl Scaling {
    /// Quasar centers the largest 16:15 image that fits, so bars appear on one
    /// axis only; only the sampling filter differs between Sharp and Smooth.
    pub fn options(self) -> quasar_core::DisplayOptions {
        quasar_core::DisplayOptions {
            aspect: Some(ASPECT),
            scaling: quasar_core::Scaling::Fit,
            filter: self.filter(),
        }
    }
    pub fn filter(self) -> quasar_core::Filter {
        match self {
            Self::Fit => quasar_core::Filter::Nearest,
            Self::Smooth => quasar_core::Filter::Linear,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Fit => "SHARP",
            Self::Smooth => "SMOOTH",
        }
    }
    pub fn toggle(&mut self) {
        *self = match self {
            Self::Fit => Self::Smooth,
            Self::Smooth => Self::Fit,
        };
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplaySettings {
    pub version: u32,
    /// The remembered windowed dimensions, also retained while fullscreen.
    pub width: usize,
    pub height: usize,
    pub scaling: Scaling,
    #[serde(default)]
    pub fullscreen: bool,
}
impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            version: 1,
            width: WIDTH * 4,
            height: HEIGHT * 4,
            scaling: Scaling::Fit,
            fullscreen: false,
        }
    }
}
impl DisplaySettings {
    pub const SIZES: [(usize, usize); 9] = [
        (512, 480),
        (768, 720),
        (1024, 960),
        (1280, 720),
        (1280, 960),
        (1600, 900),
        (1920, 1080),
        (2560, 1440),
        (3840, 2160),
    ];
    pub fn load_or_create(path: &Path) -> Result<Self, String> {
        crate::storage::load_settings(path, Self::validate)
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        self.validate()?;
        quasar_storage::write_json(path, self)
    }
    pub fn options(self) -> quasar_core::DisplayOptions {
        self.scaling.options()
    }
    pub fn valid_size(width: usize, height: usize) -> bool {
        (WIDTH..=8192).contains(&width)
            && (HEIGHT..=8192).contains(&height)
            && width
                .checked_mul(height)
                .is_some_and(|pixels| pixels <= 7680 * 4320)
    }
    pub fn validate(&self) -> io::Result<()> {
        if self.version != 1 || !Self::valid_size(self.width, self.height) {
            return Err(io::Error::other(
                "unsupported display version or window size (256x240 minimum, 8K pixel limit)",
            ));
        }
        Ok(())
    }
    pub fn cycle_size(&mut self, direction: isize) {
        let index = Self::SIZES
            .iter()
            .position(|&size| size == (self.width, self.height))
            .unwrap_or(2);
        (self.width, self.height) = Self::SIZES
            [(index as isize + direction).rem_euclid(Self::SIZES.len() as isize) as usize];
    }
    /// Only the framebuffer minimum is enforced; any other window shape is
    /// presented with letterboxing or pillarboxing by the Quasar viewport.
    pub fn clamped_size(width: usize, height: usize) -> (usize, usize) {
        (width.max(WIDTH), height.max(HEIGHT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quasar_core::{Filter, Scaling as QuasarScaling, Viewport};

    fn viewport(width: u32, height: u32) -> Viewport {
        let options = DisplaySettings::default().options();
        Viewport::calculate((WIDTH as u32, HEIGHT as u32), (width, height), options).unwrap()
    }

    #[test]
    fn sharp_and_smooth_select_nearest_and_linear_sampling() {
        let sharp = DisplaySettings::default().options();
        assert_eq!(sharp.aspect, Some((16, 15)));
        assert_eq!(sharp.scaling, QuasarScaling::Fit);
        assert_eq!(sharp.filter, Filter::Nearest);
        let smooth = DisplaySettings {
            scaling: Scaling::Smooth,
            ..DisplaySettings::default()
        }
        .options();
        assert_eq!(smooth.filter, Filter::Linear);
        assert_eq!(smooth.aspect, sharp.aspect);
        assert_eq!(smooth.scaling, sharp.scaling);
    }

    #[test]
    fn landscape_windows_use_side_bars_only() {
        for (width, height, image_width) in [
            (1920, 1080, 1152),
            (1600, 900, 960),
            (1280, 1024, 1092),
            (2560, 1440, 1536),
            (3840, 2160, 2304),
            (1920, 1200, 1280),
        ] {
            let viewport = viewport(width, height);
            assert_eq!(viewport.height, height, "{width}x{height}");
            assert_eq!(viewport.width, image_width, "{width}x{height}");
            assert_eq!(viewport.y, 0, "{width}x{height}");
            assert_eq!(viewport.x, (width - image_width) / 2, "{width}x{height}");
        }
    }

    #[test]
    fn portrait_and_square_windows_use_top_and_bottom_bars_only() {
        for (width, height) in [(600, 1000), (1000, 1000), (256, 8192), (1080, 1920)] {
            let viewport = viewport(width, height);
            assert_eq!(viewport.width, width, "{width}x{height}");
            assert_eq!(viewport.x, 0, "{width}x{height}");
            assert_eq!(
                viewport.y,
                (height - viewport.height) / 2,
                "{width}x{height}"
            );
            assert!(viewport.height <= height, "{width}x{height}");
        }
        // A 600x1000 portrait window keeps its size and letterboxes vertically.
        assert_eq!(viewport(600, 1000).height, 562);
        assert_eq!(viewport(600, 1000).y, 219);
        assert_eq!(viewport(1080, 1920).height, 1012);
        assert_eq!(viewport(1080, 1920).y, 454);
    }

    #[test]
    fn bars_never_appear_on_all_four_sides_and_the_image_is_never_cropped() {
        for width in [256u32, 300, 640, 800, 1024, 1280, 1920, 3840] {
            for height in [240u32, 300, 480, 600, 720, 1000, 1080, 2160] {
                let viewport = viewport(width, height);
                let horizontal = viewport.x > 0;
                let vertical = viewport.y > 0;
                assert!(!(horizontal && vertical), "{width}x{height}: {viewport:?}");
                assert!(viewport.width <= width && viewport.height <= height);
                let ratio = f64::from(viewport.width) / f64::from(viewport.height);
                assert!(
                    (ratio - 16.0 / 15.0).abs() < 0.01,
                    "{width}x{height}: {ratio}"
                );
            }
        }
        let exact = viewport(256, 240);
        assert_eq!(
            (exact.x, exact.y, exact.width, exact.height),
            (0, 0, 256, 240)
        );
        assert!(
            Viewport::calculate((256, 240), (0, 0), DisplaySettings::default().options()).is_none()
        );
    }

    #[test]
    fn window_sizes_only_enforce_the_framebuffer_minimum() {
        assert_eq!(DisplaySettings::clamped_size(600, 1000), (600, 1000));
        assert_eq!(DisplaySettings::clamped_size(128, 120), (WIDTH, HEIGHT));
        assert_eq!(DisplaySettings::clamped_size(1920, 1080), (1920, 1080));
    }

    #[test]
    fn existing_display_configs_keep_window_dimensions_and_scaling() {
        let settings: DisplaySettings =
            serde_json::from_str(r#"{"version":1,"width":1600,"height":900,"scaling":"integer"}"#)
                .unwrap();
        assert_eq!((settings.width, settings.height), (1600, 900));
        assert_eq!(settings.scaling, Scaling::Fit);
        assert!(!settings.fullscreen);
        let mut fullscreen = settings;
        fullscreen.fullscreen = true;
        fullscreen.scaling = Scaling::Smooth;
        let bytes = serde_json::to_vec(&fullscreen).unwrap();
        assert_eq!(
            serde_json::from_slice::<DisplaySettings>(&bytes).unwrap(),
            fullscreen
        );
    }
}
