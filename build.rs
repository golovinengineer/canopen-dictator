// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::Path;

fn main() {
    // slint-project.json names the main file and the Slint edition.
    #[cfg(feature = "gui")]
    slint_build::compile(".").expect("Slint build failed");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        embed_info_plist();
    }
}

/// macOS names an application after its bundle. A plain executable has none,
/// so the menu bar would show the binary's lowercase file name. An Info.plist
/// in the binary's `__TEXT,__info_plist` section gives it a proper name.
fn embed_info_plist() {
    let version = env!("CARGO_PKG_VERSION");
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Dictator</string>
    <key>CFBundleDisplayName</key>
    <string>Dictator</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
</dict>
</plist>
"#
    );
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    let path = Path::new(&out_dir).join("Info.plist");
    std::fs::write(&path, plist).expect("writing Info.plist");
    println!(
        "cargo:rustc-link-arg-bin=dictator=-Wl,-sectcreate,__TEXT,__info_plist,{}",
        path.display()
    );
}
