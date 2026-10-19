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
