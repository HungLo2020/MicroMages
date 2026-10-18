use std::{env, path::PathBuf, process::Command};

fn main() {
    for path in [
        "scripts/compile_game.py",
        "scripts/instructions.json",
        "scripts/game_roots.json",
        "assets",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let assets = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("assets");
    println!(
        "cargo:rustc-env=MICRO_MAGES_BUILD_ASSETS={}",
        assets.display()
    );
    let result = Command::new("python3")
        .arg("scripts/compile_game.py")
        .arg(&assets)
        .arg(env::var_os("OUT_DIR").unwrap())
        .status()
        .expect("Python 3 is required for the build-time native translation");
    assert!(result.success(), "Static game translation failed");
}
