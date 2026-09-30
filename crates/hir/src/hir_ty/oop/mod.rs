//! FUNCTION_BLOCKs, CLASSes and INTERFACEs, and how they relate, shaped
//! after ty's classes: each POU's bases are resolved once
//! ([`explicit_bases`]), its [`ancestry`] is built from its bases' cached
//! ancestry, and every other question about inheritance is answered from
//! those results rather than by walking headers again.

mod ancestors;
mod bases;
mod inheritors;
mod members;
mod method;

pub use ancestors::{Ancestry, ancestry};
pub use bases::{BaseRole, ExplicitBases, WrittenBase, can_extend, explicit_bases, written_bases};
pub use inheritors::{descendants, file_inheritors, inheritors};
pub use members::{ClassMember, ClassMembers, InstanceMember, class_members, instance_members};
pub use method::MethodRef;
