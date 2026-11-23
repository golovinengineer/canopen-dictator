// SPDX-License-Identifier: MIT OR Apache-2.0

//! CANopen object dictionary editor: core library (model, project file,
//! XDD import, EDS and Markdown export, and the editing session the front
//! ends share). The CanFestival C generator is the separate, LGPL-licensed
//! `canopen-dictator-canfestival` crate; the GUI, terminal UI and command line live
//! in the `dictator` binary.

pub mod editor;
pub mod export;
pub mod model;
pub mod project;
pub mod xdd;
