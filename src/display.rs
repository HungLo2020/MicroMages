//! Machine-local fullscreen settings and full-height, aspect-preserving scaling.
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
        crate::storage::atomic_write(path, &serde_json::to_vec_pretty(self)?)
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
    /// A narrow window becomes shorter instead of introducing letterboxing.
    pub fn windowed_size(width: usize, height: usize) -> (usize, usize) {
        let width = width.max(WIDTH);
        let height = height.max(HEIGHT).min(width * HEIGHT / WIDTH);
        (width, height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    pub x: usize,
    pub width: usize,
    pub height: usize,
}
impl Viewport {
    /// Always use the entire height. Reject dimensions that would crop the image.
    pub fn fit(width: usize, height: usize) -> Option<Self> {
        let image_width = height.checked_mul(WIDTH)?.checked_div(HEIGHT)?;
        if image_width == 0 || image_width > width {
            return None;
        }
        Some(Self {
            x: (width - image_width) / 2,
            width: image_width,
            height,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaling_uses_the_full_height_with_only_side_bars() {
        for (width, height, image_width) in [
            (1920, 1080, 1152),
            (1600, 900, 960),
            (1000, 600, 640),
            (3840, 2160, 2304),
            (128, 120, 128),
        ] {
            let viewport = Viewport::fit(width, height).unwrap();
            assert_eq!(viewport.height, height);
            assert_eq!(viewport.width, image_width);
            assert_eq!(viewport.x, (width - image_width) / 2);
        }
        assert!(Viewport::fit(600, 1000).is_none());
        assert!(Viewport::fit(0, 0).is_none());
    }
    #[test]
    fn narrow_windows_are_corrected_without_cropping_or_letterboxing() {
        for (width, height) in [(600, 1000), (1000, 600), (128, 120), (256, 8192)] {
            let (w, h) = DisplaySettings::windowed_size(width, height);
            assert!(w >= WIDTH && h >= HEIGHT);
            assert!(Viewport::fit(w, h).is_some());
        }
        assert_eq!(DisplaySettings::windowed_size(600, 1000), (600, 562));
    }
    #[test]
    fn existing_display_configs_use_full_height_and_keep_window_dimensions() {
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
