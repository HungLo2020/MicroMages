//! Native playback of the repository's PC-game Ogg assets.
//! Receives semantic events from the simulation; no code-address hooks.
//!
//! `quasar-audio` provides decoding, buses, voice handles and loop regions. The
//! game keeps its own track/effect tables, three-voice replacement rule, fade
//! curve and pause semantics.
use crate::events::{GameEvent, Music, Sound};
use quasar_audio::{
    AudioEngine, BusId, Clip, EngineConfig, LoopMode, LoopRegion, PlaybackConfig, VoiceHandle,
    VoicePolicy,
};
use std::{error::Error, path::Path};

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

/// The original has three effect voices, replaced by priority.
const VOICES: usize = 3;
const MUSIC_GAIN: f32 = 0.85;
const FADE_STEPS: u16 = 256;
/// Dreamy plays slightly flat in the original track table.
const DREAMY_SPEED: f32 = f32::from_bits(0x3f7f943f);

/// Build the loop policy for a track from its documented seconds.
///
/// A whole-track loop starts at frame zero; a region track plays its intro once
/// and then repeats `start..end`, which is exactly `LoopMode::Region`.
fn loop_mode(clip: &Clip, looping: bool, start: f64, end: f64) -> LoopMode {
    if !looping {
        return LoopMode::None;
    }
    let frames = clip.frames();
    if frames == 0 || start <= 0.0 || end <= 0.0 {
        return LoopMode::Whole;
    }
    let rate = f64::from(clip.sample_rate());
    let start = (start * rate) as u64;
    let end = ((end * rate) as u64).min(frames);
    if start >= end {
        // A shorter asset cannot honor the region; play it once like the
        // previous clip iterator did instead of erroring at playback.
        return LoopMode::None;
    }
    LoopRegion::new(start, end)
        .map(LoopMode::Region)
        .unwrap_or(LoopMode::Whole)
}

pub struct Audio {
    engine: AudioEngine,
    music_bus: BusId,
    effects_bus: BusId,
    /// Music decodes on demand, so the asset root stays available.
    root: std::path::PathBuf,
    sfx: Vec<Clip>,
    voices: [Option<(usize, VoiceHandle)>; VOICES],
    music: Option<VoiceHandle>,
    fade: u16,
    music_volume: f32,
    effects_volume: f32,
    host_paused: bool,
}
impl Audio {
    pub fn new(root: &Path) -> Result<Self, Box<dyn Error>> {
        let engine = AudioEngine::open(
            None,
            EngineConfig {
                max_voices: 16,
                ..EngineConfig::default()
            },
        )?;
        let music_bus = engine.create_bus("music", MUSIC_GAIN, 4)?;
        let effects_bus = engine.create_bus("effects", 1.0, VOICES)?;
        let sfx = SFX
            .iter()
            .map(|(name, _)| Clip::load(root.join(name)).map_err(Box::<dyn Error>::from))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            engine,
            music_bus,
            effects_bus,
            root: root.to_owned(),
            sfx,
            voices: [None, None, None],
            music: None,
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
                self.stop_music();
                self.fade = 0;
            }
            GameEvent::FadeMusic => self.fade = 1,
            GameEvent::GameOver | GameEvent::Victory | GameEvent::StageCompleted => {}
        }
    }
    fn stop_music(&mut self) {
        if let Some(voice) = self.music.take() {
            let _ = self.engine.stop(&voice);
        }
    }
    fn play_music(&mut self, music: Music) {
        let id = music.index();
        let Some(&(name, looping, start, end)) = MUSIC.get(id) else {
            return;
        };
        self.stop_music();
        self.fade = 0;
        let Ok(clip) = Clip::load(self.root.join(name)) else {
            return;
        };
        let config = PlaybackConfig {
            bus: self.music_bus,
            looping: loop_mode(&clip, looping, start, end),
            speed: if id == 3 { DREAMY_SPEED } else { 1.0 },
            policy: VoicePolicy::Reject,
            ..PlaybackConfig::default()
        };
        if let Ok(voice) = self.engine.play(clip, config) {
            self.music = Some(voice);
            self.sync_music();
        }
    }
    fn play_sfx(&mut self, sound: Sound, pitch: i8) {
        let id = sound.index();
        let Some(clip) = self.sfx.get(id) else {
            return;
        };
        // Replaying a sound replaces its running instance.
        for slot in &mut self.voices {
            if let Some((old, voice)) = slot
                && *old == id
            {
                let _ = self.engine.stop(voice);
                *slot = None;
            }
        }
        let free = self
            .voices
            .iter()
            .position(|slot| slot.as_ref().is_none_or(|(_, voice)| !voice.is_active()));
        let slot = free.or_else(|| {
            self.voices
                .iter()
                .position(|slot| matches!(slot, Some((old, _)) if SFX[id].1 >= SFX[*old].1))
        });
        let Some(slot) = slot else {
            return;
        };
        if let Some((_, voice)) = self.voices[slot].take() {
            let _ = self.engine.stop(&voice);
        }
        let config = PlaybackConfig {
            bus: self.effects_bus,
            speed: (1.0 + f32::from(pitch) * 0.05).max(0.4),
            policy: VoicePolicy::Reject,
            ..PlaybackConfig::default()
        };
        if let Ok(voice) = self.engine.play(clip.clone(), config) {
            self.voices[slot] = Some((id, voice));
        }
    }
    pub fn update(&mut self) {
        if self.fade > 0 {
            self.fade = (self.fade + 1).min(FADE_STEPS);
            self.sync_music();
        }
    }

    /// Pause preserves the current playback position.
    pub fn settings(&mut self, music: u8, effects: u8, paused: bool) {
        self.music_volume = f32::from(music.min(100)) / 100.0;
        self.effects_volume = f32::from(effects.min(100)) / 100.0;
        self.host_paused = paused;
        let _ = self.engine.set_paused(paused);
        let _ = self
            .engine
            .set_bus_volume(self.effects_bus, self.effects_volume);
        self.sync_music();
    }

    fn sync_music(&self) {
        let faded = f32::from(FADE_STEPS - self.fade) / f32::from(FADE_STEPS);
        let _ = self
            .engine
            .set_bus_volume(self.music_bus, MUSIC_GAIN * self.music_volume * faded);
        let _ = self.engine.set_paused(self.host_paused);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quasar_audio::{Mixer, MixerConfig};

    fn clip(seconds: f64) -> Clip {
        let rate = 4_000u32;
        let frames = (seconds * f64::from(rate)) as usize;
        Clip::from_samples(vec![0.25; frames], 1, rate, "test").expect("test clip")
    }

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
                let clip = Clip::load(&path).unwrap();
                assert!(clip.frames() > 0, "Empty audio: {}", path.display());
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
    fn track_tables_map_to_expected_loop_policies() {
        let root = crate::assets::default_root();
        for &(name, looping, start, end) in &MUSIC {
            let clip = Clip::load(root.join(name)).unwrap();
            let mode = loop_mode(&clip, looping, start, end);
            match (looping, start, end) {
                (false, _, _) => assert_eq!(mode, LoopMode::None, "{name}"),
                (true, 0.0, 0.0) => assert_eq!(mode, LoopMode::Whole, "{name}"),
                (true, start, end) => {
                    let LoopMode::Region(region) = mode else {
                        panic!("{name} should loop a region");
                    };
                    let rate = f64::from(clip.sample_rate());
                    assert_eq!(region.start, (start * rate) as u64, "{name}");
                    // A region end past the decoded asset is clamped, as the
                    // previous clip iterator did.
                    assert_eq!(
                        region.end,
                        ((end * rate) as u64).min(clip.frames()),
                        "{name}"
                    );
                }
            }
        }
    }

    #[test]
    fn region_loop_plays_the_intro_once_before_repeating() {
        let mut mixer = Mixer::new(MixerConfig {
            sample_rate: 4_000,
            channels: 2,
            max_voices: 4,
            command_capacity: 16,
        })
        .unwrap();
        let source = Clip::from_samples(vec![-1.0, -0.5, 0.0, 0.5, 1.0], 1, 4_000, "ramp").unwrap();
        // Entering at frame zero, play to the region end, then repeat 2..4.
        mixer
            .play(
                source,
                PlaybackConfig {
                    looping: LoopMode::Region(LoopRegion::new(2, 4).unwrap()),
                    ..PlaybackConfig::default()
                },
            )
            .unwrap();
        let frames = mixer.render(8).unwrap();
        let expected = [
            -1.0, -1.0, -0.5, -0.5, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.5, 0.5,
        ];
        assert_eq!(&frames[..expected.len()], &expected);
    }

    #[test]
    fn short_assets_fall_back_to_a_single_playback() {
        // The previous clip iterator stopped instead of erroring when a region
        // did not fit the decoded asset.
        let short = clip(1.0);
        assert_eq!(loop_mode(&short, true, 15.174, 24.759), LoopMode::None);
        assert_eq!(loop_mode(&short, true, 0.0, 0.0), LoopMode::Whole);
        assert_eq!(loop_mode(&short, false, 1.0, 2.0), LoopMode::None);
    }
}
