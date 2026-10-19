//! Native outcome artwork. Player scores and menu choices remain live Rust UI.
use crate::scenes::Timeline;
use std::{fs::File, io::BufReader, path::Path};

pub struct Outcomes {
    game_over: Timeline,
    normal: Timeline,
    hard: Vec<Timeline>,
    pixels: Vec<u32>,
}
impl Outcomes {
    pub fn load(root: &Path) -> Result<Self, String> {
        let root = root.join("presentation/outcomes");
        let mut decoder = png::Decoder::new(BufReader::new(
            File::open(root.join("outcomes.png")).map_err(|e| e.to_string())?,
        ));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut bytes = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut bytes).map_err(|e| e.to_string())?;
        if info.color_type != png::ColorType::Rgb || info.width != 512 || info.height % 8 != 0 {
            return Err("invalid outcome PNG atlas".into());
        }
        let count = info.width * info.height / 64;
        let read = |name: &str| -> Result<Timeline, String> {
            let timeline: Timeline = serde_json::from_reader(
                File::open(root.join(format!("{name}.json"))).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            if timeline.width != 256
                || timeline.height != 240
                || timeline.tile_size != 8
                || timeline.duration == 0
                || timeline.tracks.len() != 960
            {
                return Err(format!("invalid {name} outcome dimensions"));
            }
            for track in &timeline.tracks {
                if track.first().is_none_or(|key| key[0] != 0)
                    || track
                        .iter()
                        .any(|key| key[0] >= u32::from(timeline.duration) || key[1] >= count)
                    || track.windows(2).any(|pair| pair[0][0] >= pair[1][0])
                {
                    return Err(format!("invalid {name} outcome keyframes"));
                }
            }
            Ok(timeline)
        };
        Ok(Self {
            game_over: read("game-over")?,
            normal: read("normal-ending")?,
            hard: (1..=4)
                .map(|n| read(&format!("hard-ending-{n}")))
                .collect::<Result<_, _>>()?,
            pixels: bytes[..info.buffer_size()]
                .as_chunks::<3>()
                .0
                .iter()
                .map(|c| u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]))
                .collect(),
        })
    }
    pub fn draw_icon(&self, player: usize, active: bool, x: usize, y: usize, output: &mut [u32]) {
        let cell = (9 + player * 2) * 32 + 12;
        let tile = self.game_over.tracks[cell][0][1] as usize;
        let source = tile / 64 * 8 * 512 + tile % 64 * 8;
        for row in 0..8 {
            for col in 0..8 {
                let color = self.pixels[source + row * 512 + col];
                output[(y + row) * 256 + x + col] = if color != 0 && !active {
                    0x585858
                } else {
                    color
                };
            }
        }
    }
    pub fn draw(&self, game_over: bool, hard: bool, players: usize, tick: u64, output: &mut [u32]) {
        let timeline = if game_over {
            &self.game_over
        } else if !hard {
            &self.normal
        } else {
            &self.hard[players.clamp(1, 4) - 1]
        };
        let tick = tick.min(u64::from(timeline.duration) - 1) as u32;
        for (cell, track) in timeline.tracks.iter().enumerate() {
            let tile = track[track.partition_point(|key| key[0] <= tick) - 1][1] as usize;
            let source = tile / 64 * 8 * 512 + tile % 64 * 8;
            let dest = cell / 32 * 8 * 256 + cell % 32 * 8;
            for row in 0..8 {
                output[dest + row * 256..dest + row * 256 + 8]
                    .copy_from_slice(&self.pixels[source + row * 512..source + row * 512 + 8]);
            }
        }
    }
}
