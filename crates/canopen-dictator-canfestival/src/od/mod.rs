// SPDX-License-Identifier: LGPL-2.1-or-later
// Part of the Rust port of CanFestival objdictgen; see lib.rs for copyright.

//! CanFestival `.od` project files (what `objdictedit.py` saves): import
//! and export.
//!
//! An `.od` file is CanFestival's `node.Node` object written with
//! `gnosis.xml.pickle`. The parts used here:
//!
//! - `Dictionary`: index -> value of a VAR, or the list of values of subs
//!   1..n of an ARRAY/RECORD (sub 0 is implicit, the list length);
//! - `UserMapping`, `Profile`, `DS302`: definitions (name, structure, sub
//!   names, types, access, PDO flag) of the indexes CanFestival's built-in
//!   DS-301 table does not describe, searched in the order
//!   Profile -> DS302 -> UserMapping, then the DS-301 table (`node.py:
//!   GetEntryInfos / GetSubentryInfos`);
//! - `ParamsDictionary`: per index `callback`, and per entry/sub `comment`,
//!   `save`, `buffer_size`;
//! - `Name`, `Type`, `Description`, `DefaultStringSize`.
//!
//! Like `objdictedit` (`nodemanager.py: OpenFileInCurrent`), the node id
//! stored in the file is ignored and set to 0.
//!
//! Export builds the `Node` the way CanFestival's EDS import does
//! (`eds_utils.py: GenerateNode`), so the result is what objdictedit would
//! have saved after importing the EDS of the same project.

mod export;
mod import;
mod pickle;

pub use export::{generate, to_node};
pub use import::{Import, load, parse};
pub use pickle::{Py, parse_pickle, write_pickle};
