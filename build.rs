use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-env-changed=MICRO_MAGES_DIR");
    for path in [
        "scripts/compile_game.py",
        "scripts/instructions.json",
        "scripts/game_roots.json",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let game = env::var_os("MICRO_MAGES_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            let home = env::var_os("HOME").map(PathBuf::from)?;
            [
                home.join(".steam/steam/steamapps/common/Micro Mages"),
                home.join(".local/share/Steam/steamapps/common/Micro Mages"),
                PathBuf::from(
                    "/run/media/matt/10TB Internal HDD/SteamLibrary/steamapps/common/Micro Mages",
                ),
            ]
            .into_iter()
            .find(|p| p.join("Micro Mages.exe").is_file())
        })
        .expect("Set MICRO_MAGES_DIR to your installed Micro Mages directory");
    println!(
        "cargo:rerun-if-changed={}",
        game.join("Micro Mages.exe").display()
    );
    println!(
        "cargo:rustc-env=MICRO_MAGES_BUILD_ASSETS={}",
        game.display()
    );
    let result = Command::new("python3")
        .arg("scripts/compile_game.py")
        .arg(game.join("Micro Mages.exe"))
        .arg(env::var_os("OUT_DIR").unwrap())
        .status()
        .expect("Python 3 is required for the build-time native translation");
    assert!(result.success(), "Static game translation failed");
}
