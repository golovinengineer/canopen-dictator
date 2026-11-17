// SPDX-License-Identifier: MIT OR Apache-2.0

//! Remarks about an object that every front end shows next to it.

use dictator_canfestival::ds301;

/// Why edits to `index` may not show in the generated C: CanFestival
/// defines some communication objects itself. Empty for other indexes.
pub fn object_note(index: u16) -> String {
    match ds301::find(index) {
        Some(t) => format!(
            "CanFestival defines this index itself as \"{}\": names, types and access rights in the generated C come from its DS-301 table{}.",
            ds301::entry_name(t, index),
            if t.callback { ", and callbacks are always generated" } else { "" }
        ),
        None => String::new(),
    }
}
