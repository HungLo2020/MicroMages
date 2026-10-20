//! Portable controls and machine-local display settings through the real executable.
use micro_mages_re::{
    controls::{Action, Controls, GamepadButton, KeyboardKey},
    display::{DisplaySettings, Scaling},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

struct Installation(PathBuf);
impl Drop for Installation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn launch(root: &Path, config: &Path, shared: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_micro-mages-re"))
        .env(
            "MICRO_MAGES_ASSETS",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"),
        )
        .env("BASALT_GAME_DIR", root)
        .env("BASALT_APP_DIR", root.join("app"))
        .env("BASALT_SAVE_DIR", root.join("data/save"))
        .env("BASALT_CONFIG_DIR", config)
        .env("BASALT_SHARED_CONFIG_DIR", shared)
        .env("BASALT_CACHE_DIR", root.join("data/cache"))
        .args(["--headless", "--frames", "1"])
        .output()
        .unwrap()
}
#[test]
fn settings_persist_in_the_correct_basalt_categories_and_honor_overrides() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let installation = Installation(
        std::env::temp_dir().join(format!("mm-settings-{}-{nonce}", std::process::id())),
    );
    let root = &installation.0;
    let config = root.join("data/config");
    let shared = config.join("shared");
    fs::create_dir_all(&shared).unwrap();
    let preferences = br#"{"version":1,"music":30,"effects":70}"#;
    fs::write(shared.join("preferences.json"), preferences).unwrap();
    let output = launch(root, &config, &shared);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(shared.join("preferences.json")).unwrap(),
        preferences
    );
    assert!(shared.join("controls.json").is_file());
    assert!(config.join("display.json").is_file());
    assert!(!shared.join("display.json").exists());

    let mut controls = Controls::load_or_create(&shared.join("controls.json")).unwrap();
    controls
        .bind_key(0, Action::Jump, KeyboardKey::Space)
        .unwrap();
    controls
        .bind_button(3, Action::Fire, GamepadButton::RightTrigger)
        .unwrap();
    controls.save(&shared.join("controls.json")).unwrap();
    let display = DisplaySettings {
        width: 1280,
        height: 720,
        scaling: Scaling::Fit,
        ..DisplaySettings::default()
    };
    display.save(&config.join("display.json")).unwrap();
    let before_controls = fs::read(shared.join("controls.json")).unwrap();
    let before_display = fs::read(config.join("display.json")).unwrap();
    assert!(launch(root, &config, &shared).status.success());
    assert_eq!(
        fs::read(shared.join("controls.json")).unwrap(),
        before_controls
    );
    assert_eq!(
        fs::read(config.join("display.json")).unwrap(),
        before_display
    );
    assert_eq!(
        Controls::load_or_create(&shared.join("controls.json")).unwrap(),
        controls
    );
    assert_eq!(
        DisplaySettings::load_or_create(&config.join("display.json")).unwrap(),
        display
    );

    let alternate_config = root.join("other-local");
    let alternate_shared = root.join("other-shared");
    assert!(
        launch(root, &alternate_config, &alternate_shared)
            .status
            .success()
    );
    assert!(alternate_config.join("display.json").is_file());
    assert!(alternate_shared.join("controls.json").is_file());
    assert!(!alternate_shared.join("display.json").exists());
    assert_eq!(
        fs::read(shared.join("controls.json")).unwrap(),
        before_controls
    );
    assert_eq!(
        fs::read(config.join("display.json")).unwrap(),
        before_display
    );

    let invalid = String::from_utf8(before_controls).unwrap().replacen(
        "\"version\": 1",
        "\"version\": 99",
        1,
    );
    fs::write(shared.join("controls.json"), &invalid).unwrap();
    let output = launch(root, &config, &shared);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported controls version"));
    assert_eq!(
        fs::read_to_string(shared.join("controls.json")).unwrap(),
        invalid
    );
    assert_eq!(
        fs::read(config.join("display.json")).unwrap(),
        before_display
    );
}
