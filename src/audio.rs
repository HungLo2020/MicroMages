//! Native playback of the repository's PC-game Ogg assets.
//! Receives semantic events from the simulation; no code-address hooks.
use crate::events::{GameEvent, Music, Sound};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::{
    error::Error,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

const MUSIC: [(&str, bool, f64, f64); 9] = [
    ("music/mysterious.ogg", true, 0.0, 0.0),
    ("music/heroic.ogg", true, 0.0, 0.0),
    ("music/spooky.ogg", true, 0.0, 0.0),
    ("music/dreamy.ogg", true, 1.596, 39.934),
    ("music/evil.ogg", true, 0.0, 0.0),
    ("music/intro.ogg", true, 15.174, 24.759),
    ("music/fanfare.ogg", false, 0.0, 0.0),
    ("music/boss.ogg", true, 0.0, 0.0),
    ("music/march.ogg", true, 0.0, 0.0),
];
const SFX: [(&str, u32); 34] = [
    ("sfx/emerald.ogg", 8),
    ("sfx/spiderweb.ogg", 8),
    ("sfx/destroy block.ogg", 8),
    ("sfx/hero jump.ogg", 6),
    ("sfx/spell.ogg", 5),
    ("sfx/spell2.ogg", 7),
    ("sfx/damage block.ogg", 6),
    ("sfx/damage enemy.ogg", 6),
    ("sfx/destroy enemy.ogg", 8),
    ("sfx/hero damage.ogg", 9),
    ("sfx/hero head.ogg", 7),
    ("sfx/hero bounce.ogg", 8),
    ("sfx/fairy.ogg", 9),
    ("sfx/extinguish.ogg", 8),
    ("sfx/boss dash.ogg", 10),
    ("sfx/boss inhale.ogg", 10),
    ("sfx/open chest.ogg", 8),
    ("sfx/duck flap.ogg", 7),
    ("sfx/goat bubble.ogg", 7),
    ("sfx/troll spear.ogg", 7),
    ("sfx/wstar.ogg", 9),
    ("sfx/checkpoint.ogg", 10),
    ("sfx/spell charge.ogg", 8),
    ("sfx/pause.ogg", 15),
    ("sfx/unpause.ogg", 15),
    ("sfx/level exit.ogg", 10),
    ("sfx/level exit2.ogg", 10),
    ("sfx/hero ghost.ogg", 9),
    ("sfx/boss damage.ogg", 9),
    ("sfx/portal.ogg", 8),
    ("sfx/spark.ogg", 8),
    ("sfx/snake.ogg", 9),
    ("sfx/boss dash charge.ogg", 10),
    ("sfx/willowisp scream.ogg", 9),
];

#[derive(Clone)]
struct Sample {
    data: Arc<[f32]>,
    channels: u16,
    rate: u32,
}
impl Sample {
    fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let decoder = Decoder::new(BufReader::new(File::open(path)?))?;
        let channels = decoder.channels();
        let rate = decoder.sample_rate();
        let data = decoder.convert_samples::<f32>().collect::<Vec<_>>().into();
        Ok(Self {
            data,
            channels,
            rate,
        })
    }
    fn position(&self, seconds: f64) -> usize {
        ((seconds * self.rate as f64) as usize * self.channels as usize).min(self.data.len())
    }
    fn source(&self, looping: bool, start: f64, end: f64) -> Clip {
        let end = if end > 0.0 {
            self.position(end)
        } else {
            self.data.len()
        };
        Clip {
            sample: self.clone(),
            position: 0,
            loop_range: looping.then_some((self.position(start), end)),
            seek: Arc::new(AtomicUsize::new(usize::MAX)),
        }
    }
}
struct Clip {
    sample: Sample,
    position: usize,
    loop_range: Option<(usize, usize)>,
    seek: Arc<AtomicUsize>,
}
impl Iterator for Clip {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let seek = self.seek.swap(usize::MAX, Ordering::Relaxed);
        if seek != usize::MAX {
            self.position = seek;
        }
        if let Some((start, end)) = self.loop_range
            && self.position >= end
            && start < end
        {
            self.position = start;
        }
        let value = *self.sample.data.get(self.position)?;
        self.position += 1;
        Some(value)
    }
}
impl Source for Clip {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.sample.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

pub struct Audio {
    // Keep the device alive for the lifetime of its sinks.
    _stream: OutputStream,
    handle: OutputStreamHandle,
    root: PathBuf,
    sfx: Vec<Sample>,
    voices: Vec<(usize, Sink)>,
    music: Option<Sink>,
    music_seek: Option<(Arc<AtomicUsize>, usize)>,
    fade: u16,
    music_volume: f32,
    effects_volume: f32,
    host_paused: bool,
}
impl Audio {
    pub fn new(root: &Path) -> Result<Self, Box<dyn Error>> {
        let (stream, handle) = OutputStream::try_default()?;
        let sfx = SFX
            .iter()
            .map(|(name, _)| Sample::load(&root.join(name)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            _stream: stream,
            handle,
            root: root.to_owned(),
            sfx,
            voices: Vec::new(),
            music: None,
            music_seek: None,
            fade: 0,
            music_volume: 1.0,
            effects_volume: 1.0,
            host_paused: false,
        })
    }

    pub fn handle(&mut self, event: GameEvent) {
        match event {
            GameEvent::Sound(sound) => self.play_sfx(sound, 0),
            GameEvent::Music(music) => self.play_music(music),
            GameEvent::StopMusic => {
                self.music = None;
                self.music_seek = None;
                self.fade = 0;
            }
            GameEvent::FadeMusic => self.fade = 1,
            GameEvent::GameOver | GameEvent::Victory | GameEvent::StageCompleted => {}
        }
    }
    fn play_music(&mut self, music: Music) {
        let id = music.index();
        let Some(&(name, looping, start, end)) = MUSIC.get(id) else {
            return;
        };
        self.music = None;
        self.music_seek = None;
        self.fade = 0;
        let Ok(sample) = Sample::load(&self.root.join(name)) else {
            return;
        };
        let Ok(sink) = Sink::try_new(&self.handle) else {
            return;
        };
        let source = sample.source(looping, start, end);
        self.music_seek = Some((source.seek.clone(), sample.position(1.596)));
        sink.set_volume(0.85 * self.music_volume);
        if id == 3 {
            sink.set_speed(f32::from_bits(0x3f7f943f));
        }
        sink.append(source);
        self.music = Some(sink);
        self.sync_music();
    }
    fn play_sfx(&mut self, sound: Sound, pitch: i8) {
        let id = sound.index();
        let Some(sample) = self.sfx.get(id) else {
            return;
        };
        // The original has three effect voices, with priority-based replacement.
        let slot = self
            .voices
            .iter()
            .position(|(_, sink)| sink.empty())
            .or_else(|| (self.voices.len() < 3).then_some(self.voices.len()))
            .or_else(|| {
                self.voices
                    .iter()
                    .position(|(old, _)| SFX[id].1 >= SFX[*old].1)
            });
        let Some(slot) = slot else {
            return;
        };
        for (old, sink) in &self.voices {
            if *old == id {
                sink.stop();
            }
        }
        let Ok(sink) = Sink::try_new(&self.handle) else {
            return;
        };
        sink.set_speed((1.0 + pitch as f32 * 0.05).max(0.4));
        sink.set_volume(self.effects_volume);
        if self.host_paused {
            sink.pause();
        }
        sink.append(sample.source(false, 0.0, 0.0));
        if slot == self.voices.len() {
            self.voices.push((id, sink));
        } else {
            self.voices[slot] = (id, sink);
        }
    }
    pub fn update(&mut self) {
        if self.fade > 0 {
            self.fade = (self.fade + 1).min(256);
            if let Some(sink) = &self.music {
                sink.set_volume(0.85 * self.music_volume * (256 - self.fade) as f32 / 256.0);
            }
        }
    }

    /// Pause preserves the current playback position.
    pub fn settings(&mut self, music: u8, effects: u8, paused: bool) {
        self.music_volume = music.min(100) as f32 / 100.0;
        self.effects_volume = effects.min(100) as f32 / 100.0;
        self.host_paused = paused;
        self.sync_music();
        for (_, sink) in &self.voices {
            sink.set_volume(self.effects_volume);
            if paused {
                sink.pause();
            } else {
                sink.play();
            }
        }
    }

    fn sync_music(&self) {
        if let Some(sink) = &self.music {
            sink.set_volume(0.85 * self.music_volume * (256 - self.fade) as f32 / 256.0);
            if self.host_paused {
                sink.pause();
            } else {
                sink.play();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repository_audio_assets_decode() {
        let root = crate::assets::default_root();
        for (folder, expected) in [("music", 9), ("sfx", 37)] {
            let files: Vec<_> = std::fs::read_dir(root.join(folder))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "ogg"))
                .collect();
            assert_eq!(files.len(), expected);
            for path in files {
                let sample = Sample::load(&path).unwrap();
                assert!(!sample.data.is_empty(), "Empty audio: {}", path.display());
            }
        }
        for name in MUSIC
            .iter()
            .map(|entry| entry.0)
            .chain(SFX.iter().map(|entry| entry.0))
        {
            assert!(root.join(name).is_file(), "Missing playback asset: {name}");
        }
    }

    #[test]
    fn intro_plays_once_then_only_loop_region_repeats() {
        let sample = Sample {
            data: Arc::from([0., 1., 2., 3., 4.]),
            channels: 1,
            rate: 1,
        };
        let source = sample.source(true, 2., 4.);
        assert_eq!(
            source.take(9).collect::<Vec<_>>(),
            [0., 1., 2., 3., 2., 3., 2., 3., 2.]
        );
    }
    #[test]
    fn forced_seek_is_applied_at_next_sample() {
        let sample = Sample {
            data: Arc::from([0., 1., 2., 3.]),
            channels: 1,
            rate: 1,
        };
        let mut source = sample.source(false, 0., 0.);
        assert_eq!(source.next(), Some(0.));
        source.seek.store(2, Ordering::Relaxed);
        assert_eq!(source.next(), Some(2.));
        assert_eq!(source.next(), Some(3.));
        assert_eq!(source.next(), None);
    }
}
