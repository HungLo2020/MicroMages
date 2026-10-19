//! Simulation output consumed by audio/UI. Gameplay has no audio-device dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Music {
    Mysterious,
    Heroic,
    Spooky,
    Dreamy,
    Evil,
    Intro,
    Fanfare,
    Boss,
    March,
}
impl Music {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "mysterious" => Self::Mysterious,
            "heroic" => Self::Heroic,
            "spooky" => Self::Spooky,
            "dreamy" => Self::Dreamy,
            "evil" => Self::Evil,
            "intro" => Self::Intro,
            "fanfare" => Self::Fanfare,
            "boss" => Self::Boss,
            "march" => Self::March,
            _ => return None,
        })
    }
    pub fn index(self) -> usize {
        self as usize
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Emerald,
    Spiderweb,
    DestroyBlock,
    Jump,
    Spell,
    ChargedSpell,
    DamageBlock,
    DamageEnemy,
    DestroyEnemy,
    Hurt,
    HeadBump,
    Bounce,
    Fairy,
    Extinguish,
    BossDash,
    BossInhale,
    OpenChest,
    DuckFlap,
    GoatBubble,
    TrollSpear,
    Star,
    Checkpoint,
    SpellCharge,
    Pause,
    Unpause,
    LevelExit,
    LevelExitAlternate,
    Ghost,
    BossDamage,
    Portal,
    Spark,
    Snake,
    BossCharge,
    Willowisp,
}
impl Sound {
    pub fn index(self) -> usize {
        self as usize
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameEvent {
    Sound(Sound),
    Music(Music),
    StopMusic,
    FadeMusic,
    StageCompleted,
    GameOver,
    Victory,
}
