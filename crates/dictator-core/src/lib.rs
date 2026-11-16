// SPDX-License-Identifier: MIT OR Apache-2.0

//! CANopen object dictionary editor: core library (model, project file,
//! XDD import, EDS and Markdown export). The CanFestival C generator is the
//! separate, LGPL-licensed `dictator-canfestival` crate; the GUI and command
//! line live in the `dictator` binary.

pub mod export;
pub mod model;
pub mod project;
pub mod xdd;
