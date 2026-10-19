//! Editable native cinematic artwork and timelines recovered from the original intro.
use serde::Deserialize;
use std::{fs::File, io::BufReader, path::Path};

pub const INTRO_TICKS: u16 = 948;
pub const LOADING_TICKS: u16 = 298;

#[derive(Deserialize)]
pub struct Timeline {
    pub width: usize,
    pub height: usize,
    pub tile_size: usize,
    pub duration: u16,
    pub looping: bool,
    /// One track per screen cell; each key is [tick, artwork tile].
    pub tracks: Vec<Vec<[u32; 2]>>,
}
pub struct Presentation {
    pub intro: Timeline,
    pub title: Timeline,
    pixels: Vec<u32>,
    width: usize,
    loading: Vec<u32>,
}
impl Presentation {
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join("presentation/cinematic.png");
        let mut decoder =
            png::Decoder::new(BufReader::new(File::open(path).map_err(|e| e.to_string())?));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut bytes = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut bytes).map_err(|e| e.to_string())?;
        if info.color_type != png::ColorType::Rgb || info.width != 512 || info.height % 8 != 0 {
            return Err(
                "cinematic.png must be an RGB atlas with width 512 and 8-pixel rows".into(),
            );
        }
        let pixels = bytes[..info.buffer_size()]
            .as_chunks::<3>()
            .0
            .iter()
            .map(|c| u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]))
            .collect();
        let read = |name: &str| -> Result<Timeline, String> {
            let file = File::open(root.join(format!("presentation/{name}.json")))
                .map_err(|e| e.to_string())?;
            let timeline: Timeline = serde_json::from_reader(file).map_err(|e| e.to_string())?;
            if timeline.width != 256
                || timeline.height != 240
                || timeline.tile_size != 8
                || timeline.duration == 0
                || timeline.tracks.len() != 960
            {
                return Err(format!("invalid {name} cinematic dimensions"));
            }
            let count = info.width * info.height / 64;
            for track in &timeline.tracks {
                if track.first().is_none_or(|k| k[0] != 0)
                    || track
                        .iter()
                        .any(|k| k[0] >= u32::from(timeline.duration) || k[1] >= count)
                    || track.windows(2).any(|w| w[0][0] >= w[1][0])
                {
                    return Err(format!("invalid {name} cinematic keyframes"));
                }
            }
            Ok(timeline)
        };
        let intro = read("intro")?;
        if intro.duration != INTRO_TICKS {
            return Err("intro timing does not match screen transition".into());
        }
        let decoder = png::Decoder::new(BufReader::new(
            File::open(root.join("presentation/loading.png")).map_err(|e| e.to_string())?,
        ));
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut bytes = vec![0; reader.output_buffer_size()];
        let info_loading = reader.next_frame(&mut bytes).map_err(|e| e.to_string())?;
        if info_loading.width != 256
            || info_loading.height != 240
            || info_loading.color_type != png::ColorType::Rgb
        {
            return Err("loading.png must be 256x240 RGB".into());
        }
        let loading = bytes[..info_loading.buffer_size()]
            .as_chunks::<3>()
            .0
            .iter()
            .map(|c| u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]))
            .collect();
        Ok(Self {
            intro,
            title: read("title")?,
            pixels,
            width: info.width as usize,
            loading,
        })
    }
    pub fn draw_loading(&self, output: &mut [u32]) {
        output.copy_from_slice(&self.loading);
    }
    pub fn draw(&self, intro: bool, tick: u64, output: &mut [u32]) {
        let timeline = if intro { &self.intro } else { &self.title };
        let tick = if timeline.looping {
            tick % u64::from(timeline.duration)
        } else {
            tick.min(u64::from(timeline.duration) - 1)
        } as u32;
        for (cell, track) in timeline.tracks.iter().enumerate() {
            let key = track.partition_point(|key| key[0] <= tick) - 1;
            let tile = track[key][1] as usize;
            let source = tile / 64 * 8 * self.width + tile % 64 * 8;
            let dest = cell / 32 * 8 * 256 + cell % 32 * 8;
            for row in 0..8 {
                output[dest + row * 256..dest + row * 256 + 8].copy_from_slice(
                    &self.pixels[source + row * self.width..source + row * self.width + 8],
                );
            }
        }
    }
}
