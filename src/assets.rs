//! Editable PNG/JSON/Ogg assets. No executable or instruction data is loaded.
use crate::physics::{CollisionMap, Material, Vec2};
use serde::Deserialize;
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

/// Entity identities are editable names in the level files; numeric `kind`
/// values only select the recovered sprite catalog.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Entity {
    InhalingBoss,
    SummoningBoss,
    FlyingBoss,
    FinalBoss,
    SorcererBoss,
    SegmentedBoss,
    Checkpoint,
    Turret,
    SummoningPlatform,
    Goat,
    Willowisp,
    Bomb,
    Troll,
    Snake,
    Walker,
    DiagonalFlyer,
    Chaser,
    Hopper,
    Leaper,
    WallSpike,
    HorizontalWind,
    Lightning,
    LevelExit,
    Spiderweb,
    WindCurrent,
    PlayerStart,
    Flame,
    PortalBlue,
    PortalGreen,
    PortalPink,
    BossEntrance,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Spawn {
    pub entity: Entity,
    pub kind: u8,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub flip_x: bool,
    #[serde(default)]
    pub flip_y: bool,
}
impl Spawn {
    pub fn position(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct Level {
    pub id: usize,
    pub hard: bool,
    pub name: String,
    pub tower: usize,
    pub floor: usize,
    pub music: String,
    pub boss: bool,
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<u16>,
    pub palettes: Vec<u8>,
    pub collision: Vec<u8>,
    pub metatiles: Vec<u8>,
    pub terrain: Vec<Terrain>,
    pub colors: Vec<u8>,
    pub spawns: Vec<Spawn>,
    pub player_start: Spawn,
    #[serde(skip)]
    damage: TerrainDamage,
}
#[derive(Clone, Debug, Default)]
struct TerrainDamage {
    cells: [Option<(usize, u8)>; 8],
    next: usize,
}
impl Level {
    pub fn validate(&self) -> Result<(), String> {
        let count = self
            .width
            .checked_mul(self.height)
            .ok_or("level dimensions overflow")?;
        if self.width != 32
            || self.height == 0
            || self.height > 4096
            || !self.height.is_multiple_of(2)
        {
            return Err(format!("{}: invalid level dimensions", self.name));
        }
        if [self.tiles.len(), self.palettes.len(), self.collision.len()]
            .iter()
            .any(|&len| len != count)
        {
            return Err(format!(
                "{}: tile, palette and collision layers must match dimensions",
                self.name
            ));
        }
        if self.tiles.iter().any(|&tile| tile >= 512)
            || self.palettes.iter().any(|&p| p >= 4)
            || self.colors.len() != 32
            || self.colors.iter().any(|&c| c >= 64)
        {
            return Err(format!("{}: invalid tile or palette reference", self.name));
        }
        for spawn in self
            .spawns
            .iter()
            .chain(std::iter::once(&self.player_start))
        {
            if !spawn.x.is_finite()
                || !spawn.y.is_finite()
                || spawn.x < 0.0
                || spawn.x > (self.width * 8) as f32
                || spawn.y < 0.0
                || spawn.y > (self.height * 8) as f32
            {
                return Err(format!("{}: spawn outside level bounds", self.name));
            }
        }
        if self.metatiles.len() != count / 4
            || self
                .metatiles
                .iter()
                .any(|id| !self.terrain.iter().any(|t| t.id == *id))
        {
            return Err(format!("{}: invalid terrain definitions", self.name));
        }
        for id in 0..=18 {
            if self.terrain.iter().filter(|t| t.id == id).count() != 1 {
                return Err(format!(
                    "{}: terrain definitions must include each break-state exactly once",
                    self.name
                ));
            }
        }
        for terrain in &self.terrain {
            if terrain.tiles.iter().any(|&tile| tile >= 512) || terrain.palette >= 4 {
                return Err("invalid terrain tile".into());
            }
        }
        Ok(())
    }
    pub fn hit_tile(&mut self, column: i32, row: i32, damage: u8) -> TerrainChange {
        if column < 0 || row < 0 || column >= self.width as i32 || row >= self.height as i32 {
            return TerrainChange::None;
        }
        let index = (row as usize / 2) * (self.width / 2) + column as usize / 2;
        if self.metatiles[index] == 16 {
            let slot = self
                .damage
                .cells
                .iter()
                .position(|cell| cell.is_some_and(|(id, _)| id == index))
                .unwrap_or_else(|| {
                    let slot = self.damage.next;
                    self.damage.next = (slot + 1) % 8;
                    self.damage.cells[slot] = Some((index, 6));
                    slot
                });
            let health = self.damage.cells[slot].unwrap().1.saturating_sub(damage);
            self.damage.cells[slot] = Some((index, health));
            if health > 0 {
                return TerrainChange::Damaged;
            }
            self.damage.cells[slot] = None;
        }
        self.break_tile(column, row)
    }

    pub fn break_tile(&mut self, column: i32, row: i32) -> TerrainChange {
        if column < 0 || row < 0 || column >= self.width as i32 || row >= self.height as i32 {
            return TerrainChange::None;
        }
        let column = column as usize;
        let row = row as usize;
        let index = (row / 2) * (self.width / 2) + column / 2;
        let old = self.metatiles[index];
        let quadrant = (row % 2) * 2 + column % 2;
        let (next, result) = match old {
            1..=15 if old & (1 << quadrant) != 0 => (old & !(1 << quadrant), TerrainChange::Broken),
            16 => (0, TerrainChange::Broken),
            17 => (18, TerrainChange::OpenedChest),
            _ => return TerrainChange::None,
        };
        self.set_metatile(column / 2, row / 2, next);
        result
    }
    /// Replace an editable 16-pixel terrain cell and keep all rendering/collision layers aligned.
    pub fn set_metatile(&mut self, column: usize, row: usize, id: u8) -> bool {
        if column >= self.width / 2 || row >= self.height / 2 {
            return false;
        }
        let Some(definition) = self.terrain.iter().find(|t| t.id == id) else {
            return false;
        };
        let index = row * (self.width / 2) + column;
        for cell in &mut self.damage.cells {
            if cell.is_some_and(|(damaged, _)| damaged == index) {
                *cell = None;
            }
        }
        self.metatiles[index] = id;
        for quadrant in 0..4 {
            let cell = (row * 2 + quadrant / 2) * self.width + column * 2 + quadrant % 2;
            self.tiles[cell] = definition.tiles[quadrant];
            self.collision[cell] = definition.collision[quadrant];
            self.palettes[cell] = definition.palette;
        }
        true
    }
}
impl CollisionMap for Level {
    fn width(&self) -> usize {
        self.width
    }
    fn height(&self) -> usize {
        self.height
    }
    fn material(&self, column: i32, row: i32) -> Material {
        if column < 0 || column >= self.width as i32 {
            return Material(1);
        }
        if row < 0 || row >= self.height as i32 {
            return Material(0);
        }
        Material(self.collision[row as usize * self.width + column as usize])
    }
}

#[derive(Deserialize)]
struct LevelFile {
    levels: Vec<Level>,
}
#[derive(Deserialize)]
struct PaletteFile {
    colors: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Terrain {
    pub id: u8,
    pub tiles: [u16; 4],
    pub collision: [u8; 4],
    pub palette: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainChange {
    None,
    Damaged,
    Broken,
    OpenedChest,
}

pub struct Atlas {
    pub width: usize,
    pub height: usize,
    /// Palette index 0..3 for each PNG pixel; zero is transparent for sprites.
    pub pixels: Vec<u8>,
}
impl Atlas {
    fn load(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut decoder = png::Decoder::new(BufReader::new(file));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut bytes = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut bytes).map_err(|e| e.to_string())?;
        if info.width != 128 || info.height != 256 || info.color_type != png::ColorType::Rgba {
            return Err("tiles.png must be a 128x256 RGBA atlas".into());
        }
        let mut pixels = Vec::with_capacity(128 * 256);
        for rgba in bytes[..info.buffer_size()].as_chunks::<4>().0.iter() {
            if rgba[0] != rgba[1] || rgba[0] != rgba[2] || rgba[0] % 85 != 0 {
                return Err("tiles.png uses palette intensities 0, 85, 170, 255".into());
            }
            pixels.push(if rgba[3] == 0 { 0 } else { rgba[0] / 85 });
        }
        Ok(Self {
            width: 128,
            height: 256,
            pixels,
        })
    }
}

pub struct Assets {
    pub outcomes: crate::outcomes::Outcomes,
    pub presentation: crate::scenes::Presentation,
    pub root: PathBuf,
    pub sprites: Vec<Sprite>,
    pub animations: Vec<Animation>,
    pub levels: Vec<Level>,
    pub atlas: Atlas,
    pub colors: Vec<u32>,
}
impl Assets {
    pub fn load(root: &Path) -> Result<Self, String> {
        let levels: LevelFile = read_json(&root.join("levels/levels.json"))?;
        let sprites: SpriteFile = read_json(&root.join("graphics/sprites.json"))?;
        let animations: AnimationFile = read_json(&root.join("graphics/animations.json"))?;
        for animation in &animations.animations {
            if animation.frames.is_empty()
                || animation.loop_start >= animation.frames.len()
                || animation.frames.iter().any(|f| f.duration == 0)
            {
                return Err(format!("invalid animation {}", animation.id));
            }
        }
        for sprite in &sprites.sprites {
            if sprite.default_palette >= 4
                || sprite
                    .frames
                    .iter()
                    .any(|f| f.parts.iter().any(|p| p.tile >= 512))
            {
                return Err(format!("invalid sprite {}", sprite.kind));
            }
        }
        let palette: PaletteFile = read_json(&root.join("graphics/palette.json"))?;
        if palette.colors.len() != 64 {
            return Err("palette must contain 64 colors".into());
        }
        for level in &levels.levels {
            level.validate()?;
            if !root
                .join("music")
                .join(format!("{}.ogg", level.music))
                .is_file()
            {
                return Err(format!("{}: missing music {}", level.name, level.music));
            }
        }
        for hard in [false, true] {
            for id in 0..if hard { 14 } else { 13 } {
                if levels
                    .levels
                    .iter()
                    .filter(|level| level.id == id && level.hard == hard)
                    .count()
                    != 1
                {
                    return Err(format!(
                        "campaign requires exactly one level {id}, hard={hard}"
                    ));
                }
            }
        }
        Ok(Self {
            root: root.to_owned(),
            presentation: crate::scenes::Presentation::load(root)?,
            outcomes: crate::outcomes::Outcomes::load(root)?,
            sprites: sprites.sprites,
            animations: animations.animations,
            levels: levels.levels,
            atlas: Atlas::load(&root.join("graphics/tiles.png"))?,
            colors: palette.colors,
        })
    }
    pub fn level(&self, id: usize, hard: bool) -> Option<&Level> {
        self.levels
            .iter()
            .find(|level| level.id == id && level.hard == hard)
    }
}
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_reader(BufReader::new(file)).map_err(|e| format!("{}: {e}", path.display()))
}
pub fn default_root() -> PathBuf {
    if let Some(root) = std::env::var_os("MICRO_MAGES_ASSETS") {
        return root.into();
    }
    if let Some(app) = std::env::var_os("BASALT_APP_DIR") {
        return PathBuf::from(app).join("assets");
    }
    if let Some(root) = std::env::var_os("BASALT_GAME_DIR") {
        return PathBuf::from(root).join("app/assets");
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin) = exe.parent()
            && bin.file_name().is_some_and(|s| s == "bin")
            && let Some(app) = bin.parent()
        {
            return app.join("assets");
        }
        let beside = exe.parent().unwrap_or(Path::new(".")).join("assets");
        if beside.is_dir() {
            return beside;
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

#[derive(Clone, Debug, Deserialize)]
pub struct SpritePart {
    pub tile: u16,
    pub x: i16,
    pub y: i16,
    pub flip_x: bool,
    pub flip_y: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct SpriteFrame {
    pub id: u16,
    pub parts: Vec<SpritePart>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Sprite {
    pub kind: u8,
    #[serde(default)]
    pub default_frame: u16,
    pub default_palette: u8,
    pub frames: Vec<SpriteFrame>,
}
#[derive(Deserialize)]
struct SpriteFile {
    sprites: Vec<Sprite>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Attachment {
    pub tile: u16,
    pub offset_x: i16,
    pub offset_y: i16,
}
#[derive(Clone, Debug, Deserialize)]
pub struct AnimationFrame {
    pub duration: u16,
    pub sprite: u16,
    pub offset_x: i16,
    pub offset_y: i16,
    pub attachment: Option<Attachment>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Animation {
    pub id: String,
    pub frames: Vec<AnimationFrame>,
    pub loop_start: usize,
}
impl Animation {
    pub fn frame_at(&self, mut tick: u64) -> &AnimationFrame {
        let prefix = self.frames[..self.loop_start]
            .iter()
            .map(|f| u64::from(f.duration))
            .sum::<u64>();
        let cycle = self.frames[self.loop_start..]
            .iter()
            .map(|f| u64::from(f.duration))
            .sum::<u64>();
        if tick >= prefix {
            tick = prefix + (tick - prefix) % cycle;
        }
        for frame in &self.frames {
            if tick < u64::from(frame.duration) {
                return frame;
            }
            tick -= u64::from(frame.duration);
        }
        &self.frames[self.frames.len() - 1]
    }
}
#[derive(Deserialize)]
struct AnimationFile {
    animations: Vec<Animation>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entire_campaign_loads_with_valid_tile_and_audio_references() {
        let assets = Assets::load(&default_root()).unwrap();
        assert_eq!(assets.levels.len(), 27);
        assert_eq!(assets.atlas.pixels.len(), 128 * 256);
        assert!(assets.atlas.pixels.iter().all(|&c| c < 4));
    }
    #[test]
    fn malformed_layers_and_spawns_are_rejected() {
        let mut level = Assets::load(&default_root()).unwrap().levels.remove(0);
        level.tiles.pop();
        assert!(level.validate().unwrap_err().contains("layers"));
        level.tiles.push(0);
        level.player_start.x = -5.0;
        assert!(level.validate().unwrap_err().contains("bounds"));
    }
    #[test]
    fn destroying_quadrants_updates_collision_without_erasing_adjacent_blocks() {
        let mut level = Assets::load(&default_root()).unwrap().levels.remove(0);
        level.metatiles[0] = 15;
        assert_eq!(level.break_tile(0, 0), TerrainChange::Broken);
        assert_eq!(level.metatiles[0], 14);
        assert!(!level.material(0, 0).solid());
        assert!(level.material(1, 0).solid());
        assert_eq!(level.break_tile(0, 0), TerrainChange::None);
        assert_eq!(level.break_tile(-1, 0), TerrainChange::None);
    }
    #[test]
    fn chest_opens_once_and_keeps_its_lower_collision() {
        let mut level = Assets::load(&default_root()).unwrap().levels.remove(0);
        level.metatiles[0] = 17;
        assert_eq!(level.break_tile(0, 1), TerrainChange::OpenedChest);
        assert_eq!(level.metatiles[0], 18);
        assert!(level.material(0, 1).solid());
        assert_eq!(level.break_tile(0, 1), TerrainChange::None);
    }
    #[test]
    fn reinforced_cell_shares_six_hit_points_across_all_four_quadrants() {
        let mut level = Assets::load(&default_root()).unwrap().levels.remove(0);
        level.set_metatile(0, 0, 16);
        assert_eq!(level.hit_tile(0, 0, 2), TerrainChange::Damaged);
        assert_eq!(level.hit_tile(1, 1, 2), TerrainChange::Damaged);
        assert_eq!(level.hit_tile(0, 1, 2), TerrainChange::Broken);
        assert_eq!(level.metatiles[0], 0);
        level.set_metatile(1, 0, 16);
        assert_eq!(level.hit_tile(2, 0, 6), TerrainChange::Broken);
    }

    #[test]
    fn reinforced_damage_cache_evicts_oldest_after_eight_distinct_cells() {
        let mut level = Assets::load(&default_root()).unwrap().levels.remove(0);
        for column in 0..9 {
            level.set_metatile(column, 0, 16);
            assert_eq!(
                level.hit_tile(column as i32 * 2, 0, 2),
                TerrainChange::Damaged
            );
        }
        assert_eq!(
            level.hit_tile(0, 0, 4),
            TerrainChange::Damaged,
            "ninth cell evicts first partial hit"
        );
        assert_eq!(level.hit_tile(0, 0, 2), TerrainChange::Broken);
    }
}
