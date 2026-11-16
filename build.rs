// SPDX-License-Identifier: MIT OR Apache-2.0

fn main() {
    // slint-project.json names the main file and the Slint edition.
    slint_build::compile(".").expect("Slint build failed");
}
