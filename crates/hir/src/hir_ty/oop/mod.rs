//! FUNCTION_BLOCKs, CLASSes and INTERFACEs, and how they relate, shaped
//! after ty's classes: each POU's bases are resolved once
//! ([`explicit_bases`]), and every other question about inheritance is
//! answered from those cached results rather than by walking headers again.

mod bases;

pub use bases::{BaseRole, ExplicitBases, WrittenBase, can_extend, explicit_bases, written_bases};
