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

#[test]
fn strict_export_fails_on_import_warnings() {
    let dir = std::env::temp_dir().join(format!("dictator-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("node.od");
    std::fs::write(
        &input,
        r#"<?xml version="1.0"?>
<!DOCTYPE PyObject SYSTEM "PyObjects.dtd">
<PyObject module="node" class="Node" id="1">
<attr name="ID" type="numeric" value="5" />
<attr name="Dictionary" type="dict" id="2">
  <entry><key type="numeric" value="4097" /><val type="numeric" value="0" /></entry>
</attr>
</PyObject>"#,
    )
    .unwrap();
    let output = dir.join("node.eds");
    let export = |strict: bool| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_dictator"));
        cmd.arg("export").arg(&input);
        if strict {
            cmd.arg("--strict");
        }
        cmd.arg("--eds").arg(&output).output().unwrap()
    };
    let warning = "warning: node id 5 in the file ignored";

    let out = export(true);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "--strict: {:?}", out.status);
    assert!(stderr.contains(warning), "--strict: {stderr}");
    assert!(!output.exists(), "--strict wrote {}", output.display());

    let out = export(false);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{:?}: {stderr}", out.status);
    assert!(stderr.contains(warning), "{stderr}");
    assert!(output.exists(), "{} not written", output.display());

    std::fs::remove_dir_all(&dir).unwrap();
}
