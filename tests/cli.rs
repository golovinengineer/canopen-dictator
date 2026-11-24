// SPDX-License-Identifier: MIT OR Apache-2.0

use std::process::Command;

#[test]
fn version_flags_print_the_version() {
    let expected = format!("dictator {}\n", env!("CARGO_PKG_VERSION"));
    for flag in ["--version", "-v", "-V"] {
        let out = Command::new(env!("CARGO_BIN_EXE_dictator"))
            .arg(flag)
            .output()
            .unwrap();
        assert!(out.status.success(), "{flag}: {:?}", out.status);
        assert_eq!(String::from_utf8_lossy(&out.stdout), expected, "{flag}");
        assert!(out.stderr.is_empty(), "{flag}: unexpected stderr");
    }
}
