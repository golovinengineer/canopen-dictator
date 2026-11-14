//! CanFestival-compatible C code generation.
//!
//! This is a port of CanFestival's `objdictgen` (`node.py`, `eds_utils.py`,
//! `gen_cfile.py`; LGPL-2.1, E. Tisserant and F. Dupin). It reproduces what
//! the original tool chain produced for a dictionary that went
//! XDD -> EDS -> objdictedit import -> gen_cfile, including its quirks, so
//! firmware built against the old `ObjDict.c/.h` keeps compiling.

pub mod ds301;
pub mod gen_c;
pub mod node;
