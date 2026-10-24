//! Portable installation paths and versioned user preferences for Basalt.
//!
//! Path discovery and atomic JSON persistence come from `quasar-storage`; this
//! module keeps the game's file names, schema versions and validation rules.
use quasar_storage::{GamePaths, read_json, write_json};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Paths {
    pub app: PathBuf,
    pub save: PathBuf,
    pub config: PathBuf,
    pub shared_config: PathBuf,
    pub cache: PathBuf,
}
impl Paths {
    pub fn discover() -> io::Result<Self> {
        Self::from_game_paths(GamePaths::discover(Some(Path::new(env!(
            "CARGO_MANIFEST_DIR"
        ))))?)
    }

    /// A relative Basalt override is ambiguous, so reject it instead of
    /// resolving it against an unspecified working directory.
    fn from_game_paths(paths: GamePaths) -> io::Result<Self> {
        for (key, value) in [
            ("BASALT_APP_DIR", &paths.app),
            ("BASALT_SAVE_DIR", &paths.save),
            ("BASALT_CONFIG_DIR", &paths.config),
            ("BASALT_SHARED_CONFIG_DIR", &paths.shared_config),
            ("BASALT_CACHE_DIR", &paths.cache),
        ] {
            if !value.is_absolute() {
                return Err(io::Error::other(format!("{key} must be an absolute path")));
            }
        }
        Ok(Self {
            app: paths.app,
            save: paths.save,
            config: paths.config,
            shared_config: paths.shared_config,
            cache: paths.cache,
        })
    }

    pub fn create_user_directories(&self) -> io::Result<()> {
        for dir in [&self.save, &self.config, &self.shared_config, &self.cache] {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn preferences_file(&self) -> PathBuf {
        self.shared_config.join("preferences.json")
    }
    pub fn controls_file(&self) -> PathBuf {
        self.shared_config.join("controls.json")
    }
    pub fn display_file(&self) -> PathBuf {
        self.config.join("display.json")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub version: u32,
    pub music: u8,
    pub effects: u8,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            music: 100,
            effects: 100,
        }
    }
}
impl Preferences {
    pub fn load_or_create(path: &Path) -> Result<Self, String> {
        load_settings(path, Self::validate)
    }
    fn validate(&self) -> io::Result<()> {
        if self.version != 1 || self.music > 100 || self.effects > 100 {
            return Err(io::Error::other(
                "unsupported preferences version or volume outside 0..100",
            ));
        }
        Ok(())
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        self.validate()?;
        write_json(path, self)
    }
}

/// Missing files are created from validated defaults; existing files that do
/// not match the current schema are reported and left untouched.
pub(crate) fn load_settings<T: serde::de::DeserializeOwned + Serialize + Default>(
    path: &Path,
    validate: impl Fn(&T) -> io::Result<()>,
) -> Result<T, String> {
    let load = || -> io::Result<T> {
        match read_json(path)? {
            Some(value) => {
                validate(&value)?;
                Ok(value)
            }
            None => {
                let value = T::default();
                validate(&value)?;
                write_json(path, &value)?;
                Ok(value)
            }
        }
    };
    load().map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_locations_split_shared_and_machine_local_settings() {
        let paths = Paths {
            app: PathBuf::from("/games/MicroMages/app"),
            save: PathBuf::from("/games/MicroMages/data/save"),
            config: PathBuf::from("/games/MicroMages/data/config"),
            shared_config: PathBuf::from("/games/MicroMages/data/config/shared"),
            cache: PathBuf::from("/games/MicroMages/data/cache"),
        };
        assert_eq!(
            paths.controls_file(),
            Path::new("/games/MicroMages/data/config/shared/controls.json")
        );
        assert_eq!(
            paths.display_file(),
            Path::new("/games/MicroMages/data/config/display.json")
        );
    }

    #[test]
    fn relative_overrides_are_rejected() {
        let error = Paths::from_game_paths(GamePaths {
            root: PathBuf::from("/games/MicroMages"),
            app: PathBuf::from("app"),
            save: PathBuf::from("/games/MicroMages/data/save"),
            config: PathBuf::from("/games/MicroMages/data/config"),
            shared_config: PathBuf::from("/games/MicroMages/data/config/shared"),
            cache: PathBuf::from("/games/MicroMages/data/cache"),
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("BASALT_APP_DIR must be an absolute path")
        );
    }

    #[test]
    fn preferences_survive_restarts_and_invalid_data_is_not_overwritten() {
        let dir = std::env::temp_dir().join(format!("mm-preferences-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("preferences.json");
        assert_eq!(
            Preferences::load_or_create(&path).unwrap(),
            Preferences::default()
        );
        let custom = Preferences {
            music: 30,
            effects: 70,
            ..Preferences::default()
        };
        custom.save(&path).unwrap();
        assert_eq!(Preferences::load_or_create(&path).unwrap(), custom);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::write(&path, br#"{"version":2,"music":30,"effects":70}"#).unwrap();
        let original = fs::read(&path).unwrap();
        assert!(Preferences::load_or_create(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_dir_all(dir).unwrap();
    }
}
