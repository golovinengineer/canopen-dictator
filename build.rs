// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::Path;

fn main() {
    // slint-project.json names the main file and the Slint edition.
    #[cfg(feature = "gui")]
    slint_build::compile(".").expect("Slint build failed");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        embed_info_plist();
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_resources();
    }
}

/// Icon, version information and manifest for the Windows executable.
/// rfd's custom dialog buttons import TaskDialogIndirect from common controls
/// v6, so the exe doesn't start without this manifest. DPI awareness is left
/// out: winit sets it at run time.
fn embed_windows_resources() {
    println!("cargo:rerun-if-changed=assets/icon/dictator.ico");
    winresource::WindowsResource::new()
        .set_icon("assets/icon/dictator.ico")
        .set("ProductName", "Dictator")
        .set("FileDescription", "Dictator")
        .set_manifest(
            r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<dependency>
    <dependentAssembly>
        <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
</dependency>
<compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
        <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
</compatibility>
<application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
        <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
</application>
</assembly>
"#,
        )
        .compile_for("dictator")
        .expect("compiling Windows resources");
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
