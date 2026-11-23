// SPDX-License-Identifier: LGPL-2.1-or-later
//
// Copyright (C) Edouard Tisserant, Francis Dupin and Laurent Bessard
// (CanFestival objdictgen: node.py, eds_utils.py, gen_cfile.py).
// Copyright (C) 2026 Sergey Golovin (Rust port, 2026).
//
// This library is free software; you can redistribute it and/or modify it
// under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation; either version 2.1 of the License, or (at
// your option) any later version. See the COPYING file in this crate.
//
// This library is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY
// or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Lesser General Public
// License for more details.

//! CanFestival-compatible C code generation and `.od` project import.
//!
//! This is a port of CanFestival's `objdictgen` (`node.py`, `eds_utils.py`,
//! `gen_cfile.py`; LGPL-2.1-or-later, E. Tisserant, F. Dupin, L. Bessard),
//! translated from Python to Rust in 2026. It reproduces what the original
//! tool chain produced for a dictionary that went
//! XDD -> EDS -> objdictedit import -> gen_cfile, including its quirks, so
//! firmware built against the old `ObjDict.c/.h` keeps compiling.
//!
//! This crate stays under the LGPL because it is derived from that code; the
//! rest of Dictator is MIT OR Apache-2.0.

pub mod ds301;
pub mod gen_c;
pub mod node;
pub mod od;
