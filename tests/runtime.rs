//! Exercise the shipped executable using only a copied native asset directory.
use std::{
    fs,
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct Package(std::path::PathBuf);
impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn invalid_attachments_and_short_boss_entrances_fail_cleanly_at_startup() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let package = Package(std::env::temp_dir().join(format!(
        "micro-mages-invalid-assets-{}-{suffix}",
        std::process::id()
    )));
    let assets = package.0.join("assets");
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"),
        &assets,
    );
    let launch = || {
        Command::new(env!("CARGO_BIN_EXE_micro-mages-re"))
            .env("MICRO_MAGES_ASSETS", &assets)
            .env("BASALT_GAME_DIR", &package.0)
            .env_remove("BASALT_APP_DIR")
            .env_remove("BASALT_SAVE_DIR")
            .env_remove("BASALT_CONFIG_DIR")
            .env_remove("BASALT_SHARED_CONFIG_DIR")
            .env_remove("BASALT_CACHE_DIR")
            .args(["--headless", "--stage", "0", "--frames", "1"])
            .output()
            .unwrap()
    };
    assert!(launch().status.success(), "unmodified assets must load");

    let animations_path = assets.join("graphics/animations.json");
    let original_animations = fs::read(&animations_path).unwrap();
    let mut animations: serde_json::Value = serde_json::from_slice(&original_animations).unwrap();
    let idle = animations["animations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|a| a["id"] == "player_idle")
        .unwrap();
    idle["frames"][0]["attachment"] =
        serde_json::json!({"tile": 511, "offset_x": 0, "offset_y": 0});
    fs::write(&animations_path, serde_json::to_vec(&animations).unwrap()).unwrap();
    assert!(launch().status.success(), "last valid atlas tile must load");
    let idle = animations["animations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|a| a["id"] == "player_idle")
        .unwrap();
    idle["frames"][0]["attachment"]["tile"] = serde_json::json!(512);
    fs::write(&animations_path, serde_json::to_vec(&animations).unwrap()).unwrap();
    let output = launch();
    assert_eq!(output.status.code(), Some(1), "must reject before a panic");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("player_idle") && stderr.contains("attachment tile"),
        "{stderr}"
    );
    fs::write(&animations_path, original_animations).unwrap();

    let levels_path = assets.join("levels/levels.json");
    let original_levels: serde_json::Value =
        serde_json::from_slice(&fs::read(&levels_path).unwrap()).unwrap();
    let mut entrance = original_levels["levels"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|level| level["spawns"].as_array().unwrap())
        .find(|spawn| spawn["entity"] == "boss_entrance")
        .unwrap()
        .clone();
    entrance["x"] = serde_json::json!(128);
    entrance["y"] = serde_json::json!(128);
    for (height, with_entrance, valid) in [
        (32, false, true),
        (32, true, false),
        (36, true, false),
        (38, true, true),
    ] {
        let mut levels = original_levels.clone();
        let level = levels["levels"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|level| level["id"] == 0 && level["hard"] == false)
            .unwrap();
        level["height"] = serde_json::json!(height);
        for layer in ["tiles", "palettes", "collision"] {
            level[layer].as_array_mut().unwrap().truncate(32 * height);
        }
        level["metatiles"]
            .as_array_mut()
            .unwrap()
            .truncate(8 * height);
        level["player_start"]["x"] = serde_json::json!(128);
        level["player_start"]["y"] = serde_json::json!(128);
        level["spawns"] = serde_json::json!([level["player_start"].clone()]);
        if with_entrance {
            level["spawns"]
                .as_array_mut()
                .unwrap()
                .push(entrance.clone());
        }
        fs::write(&levels_path, serde_json::to_vec(&levels).unwrap()).unwrap();
        let output = launch();
        if valid {
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            assert_eq!(output.status.code(), Some(1), "must reject before a panic");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("boss entrance requires at least 38 tile rows"),
                "{stderr}"
            );
        }
    }
}

#[test]
fn packaged_game_runs_without_checkout_paths_and_rejects_invalid_assets() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let package = Package(
        std::env::temp_dir().join(format!("micro-mages-test-{}-{suffix}", std::process::id())),
    );
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"),
        &package.0.join("assets"),
    );
    let executable = package.0.join("micro-mages");
    fs::copy(env!("CARGO_BIN_EXE_micro-mages-re"), &executable).unwrap();
    let screenshot = package.0.join("frame.png");
    let result = Command::new(&executable)
        .current_dir(std::env::temp_dir())
        .env_remove("MICRO_MAGES_ASSETS")
        .args([
            "--headless",
            "--stage",
            "13",
            "--hard",
            "--players",
            "4",
            "--frames",
            "180",
            "--screenshot",
        ])
        .arg(&screenshot)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("180 simulation ticks"));
    let decoder = png::Decoder::new(std::io::BufReader::new(fs::File::open(screenshot).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (256, 240));
    assert!(pixels.iter().any(|&p| p != 0));

    // Unlike stage replays, these source-captured frames exercise the default
    // executable startup, including its animation and transition to the title.
    #[derive(serde::Deserialize)]
    struct StartupCheck {
        ticks: u64,
        screen: String,
        image: String,
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/presentation");
    let checks: Vec<StartupCheck> =
        serde_json::from_slice(&fs::read(fixtures.join("startup-checks.json")).unwrap()).unwrap();
    assert!(!checks.is_empty());
    for check in checks {
        let screenshot = package.0.join("startup.png");
        let result = Command::new(&executable)
            .current_dir(std::env::temp_dir())
            .env_remove("MICRO_MAGES_ASSETS")
            .args([
                "--headless",
                "--frames",
                &check.ticks.to_string(),
                "--screenshot",
            ])
            .arg(&screenshot)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(
            stdout.contains(&format!("Screen: {}", check.screen)),
            "{stdout}"
        );
        assert_eq!(
            png_pixels(&screenshot),
            png_pixels(&fixtures.join(check.image)),
            "source startup pixels at tick {}",
            check.ticks
        );
    }

    // Start a hard four-player game through the real title and selection flow.
    // Omitting `stage` deliberately prevents the replay shortcut from bypassing it.
    let mut buttons = vec![[0u8; 4]; 948];
    let mut press = |mask| {
        buttons.push([mask, 0, 0, 0]);
        buttons.push([0; 4]);
    };
    press(1); // Title -> player selection.
    for _ in 0..3 {
        press(128);
    }
    press(2); // Hard mode.
    press(1); // Password prompt.
    press(1); // Empty password starts the first tower.
    buttons.extend(vec![[0; 4]; 298]);
    buttons.extend(vec![[128, 64, 1, 2]; 60]);
    let boot_replay = package.0.join("boot-replay.json");
    fs::write(
        &boot_replay,
        serde_json::to_vec(&serde_json::json!({
            "players": 1, "hard": false, "buttons": buttons
        }))
        .unwrap(),
    )
    .unwrap();
    let result = Command::new(&executable)
        .current_dir(std::env::temp_dir())
        .env_remove("MICRO_MAGES_ASSETS")
        .args(["--headless", "--replay"])
        .arg(&boot_replay)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("Screen: Playing"), "{stdout}");
    assert!(
        stdout.contains("Run: stage 0, players 4, hard true"),
        "{stdout}"
    );

    let demo = Command::new(&executable)
        .current_dir(std::env::temp_dir())
        .env_remove("MICRO_MAGES_ASSETS")
        .args(["--headless", "--frames", "800", "--demo"])
        .output()
        .unwrap();
    assert!(
        demo.status.success(),
        "{}",
        String::from_utf8_lossy(&demo.stderr)
    );
    let stdout = String::from_utf8_lossy(&demo.stdout);
    assert!(stdout.contains("Screen: Playing"), "{stdout}");
    assert!(
        stdout.contains("Run: stage 0, players 1, hard false"),
        "{stdout}"
    );

    let replay = package.0.join("replay.json");
    fs::write(
        &replay,
        r#"{"stage":0,"hard":true,"players":4,"buttons":[[128,64,16,0],[129,65,17,1],[2,2,2,2]]}"#,
    )
    .unwrap();
    let result = Command::new(&executable)
        .current_dir(std::env::temp_dir())
        .env_remove("MICRO_MAGES_ASSETS")
        .args(["--headless", "--replay"])
        .arg(&replay)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("3 simulation ticks"));

    let levels = package.0.join("assets/levels/levels.json");
    let contents =
        fs::read_to_string(&levels)
            .unwrap()
            .replacen("\"player_start\"", "\"invalid_entity\"", 1);
    fs::write(levels, contents).unwrap();
    let result = Command::new(executable)
        .current_dir(std::env::temp_dir())
        .env_remove("MICRO_MAGES_ASSETS")
        .args(["--headless", "--frames", "1"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("levels.json"));
}

fn png_pixels(path: &Path) -> Vec<u8> {
    let decoder = png::Decoder::new(std::io::BufReader::new(fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((frame.width, frame.height), (256, 240));
    assert_eq!(frame.color_type, png::ColorType::Rgb);
    pixels.truncate(frame.buffer_size());
    pixels
}
