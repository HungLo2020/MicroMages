//! Portable installation paths and versioned user preferences for Basalt.
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
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
        Self::resolve(&std::env::current_exe()?, |key| std::env::var_os(key))
    }

    fn resolve(executable: &Path, env: impl Fn(&str) -> Option<OsString>) -> io::Result<Self> {
        let bin = executable
            .parent()
            .ok_or_else(|| io::Error::other("executable has no directory"))?;
        let packaged_app = bin
            .parent()
            .filter(|_| bin.file_name().is_some_and(|s| s == "bin"));
        let root = env("BASALT_GAME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                packaged_app
                    .and_then(Path::parent)
                    .unwrap_or_else(|| {
                        if bin.join("assets").is_dir() {
                            bin
                        } else {
                            Path::new(env!("CARGO_MANIFEST_DIR"))
                        }
                    })
                    .to_path_buf()
            });
        let path = |key, fallback: PathBuf| -> io::Result<PathBuf> {
            let value = env(key).map(PathBuf::from).unwrap_or(fallback);
            if !value.is_absolute() {
                return Err(io::Error::other(format!("{key} must be an absolute path")));
            }
            Ok(value)
        };
        let app = path("BASALT_APP_DIR", root.join("app"))?;
        let config = path("BASALT_CONFIG_DIR", root.join("data/config"))?;
        Ok(Self {
            app,
            save: path("BASALT_SAVE_DIR", root.join("data/save"))?,
            shared_config: path("BASALT_SHARED_CONFIG_DIR", config.join("shared"))?,
            config,
            cache: path("BASALT_CACHE_DIR", root.join("data/cache"))?,
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
        atomic_write(path, &serde_json::to_vec_pretty(self)?)
    }
}

pub(crate) fn load_settings<T: serde::de::DeserializeOwned + Serialize + Default>(
    path: &Path,
    validate: impl Fn(&T) -> io::Result<()>,
) -> Result<T, String> {
    let load = || -> Result<T, Box<dyn std::error::Error>> {
        match fs::read(path) {
            Ok(bytes) => {
                let value = serde_json::from_slice(&bytes)?;
                validate(&value)?;
                Ok(value)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let value = T::default();
                validate(&value)?;
                atomic_write(path, &serde_json::to_vec_pretty(&value)?)?;
                Ok(value)
            }
            Err(error) => Err(error.into()),
        }
    };
    load().map_err(|error| format!("{}: {error}", path.display()))
}

/// The temporary file stays on the destination filesystem, making rename atomic.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent directory"))?;
    fs::create_dir_all(parent)?;
    let (temporary, mut file) = loop {
        let name = format!(
            ".preferences-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let temporary = parent.join(name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packaged_paths_do_not_depend_on_working_directory() {
        let paths = Paths::resolve(
            Path::new("/library with spaces/MicroMages/app/bin/micro-mages"),
            |_| None,
        )
        .unwrap();
        assert_eq!(paths.app, Path::new("/library with spaces/MicroMages/app"));
        assert_eq!(
            paths.save,
            Path::new("/library with spaces/MicroMages/data/save")
        );
        assert_eq!(
            paths.shared_config,
            Path::new("/library with spaces/MicroMages/data/config/shared")
        );
    }
    #[test]
    fn launcher_overrides_each_category_and_rejects_relative_paths() {
        let paths = Paths::resolve(Path::new("/other/app/bin/game"), |key| {
            Some(format!("/library/{key}").into())
        })
        .unwrap();
        assert_eq!(paths.app, Path::new("/library/BASALT_APP_DIR"));
        assert_eq!(paths.save, Path::new("/library/BASALT_SAVE_DIR"));
        assert_eq!(paths.config, Path::new("/library/BASALT_CONFIG_DIR"));
        assert_eq!(
            paths.shared_config,
            Path::new("/library/BASALT_SHARED_CONFIG_DIR")
        );
        assert_eq!(paths.cache, Path::new("/library/BASALT_CACHE_DIR"));
        assert_eq!(
            paths.controls_file(),
            Path::new("/library/BASALT_SHARED_CONFIG_DIR/controls.json")
        );
        assert_eq!(
            paths.display_file(),
            Path::new("/library/BASALT_CONFIG_DIR/display.json")
        );
        assert!(Paths::resolve(Path::new("/app/bin/game"), |_| Some("relative".into())).is_err());
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
