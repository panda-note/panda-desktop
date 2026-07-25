//! Embeds the Panda application icon on Windows.

#![allow(clippy::disallowed_methods, reason = "build script")]

fn main() {
    #[cfg(windows)]
    embed_icon();
}

#[cfg(windows)]
fn embed_icon() {
    use std::path::PathBuf;

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon = manifest_dir.join("resources/windows/app-icon.ico");
    println!("cargo:rerun-if-changed={}", icon.display());

    if !icon.exists() {
        println!(
            "cargo:warning=Panda app icon not found at {}. Place app-icon.ico there to update the taskbar icon.",
            icon.display()
        );
        return;
    }

    let icon_escaped = icon.to_string_lossy().replace('\\', "\\\\");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let rc_path = out_dir.join("panda_resources.rc");
    std::fs::write(&rc_path, format!(r#"1 ICON "{icon_escaped}""#)).expect("write panda icon rc");

    embed_resource::compile(&rc_path, embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
