use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

pub mod e0_syntax;
pub mod e10_control_flow;
pub mod e1_duplicates;
pub mod e2_resolve;
pub mod e3_type;
pub mod e4_visibility;
pub mod e5_inheritance;
pub mod e6_array;
pub mod e7_enum;
pub mod e8_subrange;
pub mod e9_recursion;

pub trait ToIdeDiagnostic<'db> {
    /// Builds the IDE diagnostic. `file` is the file the diagnostic's primary range belongs to;
    /// it is carried (a `Copy` salsa struct) so ranges can be denormalized to the client encoding
    /// via [`crate::denormalize`] without re-deriving the document per node.
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase, file: File) -> IdeDiagnostic;
}

/*
E00xx = Syntax errors
E01xx = Duplicate definitions
E02xx = Scope/resolution/semantic (includes assignment & call violations)
E03xx = Type system
E04xx = Visibility/access
E05xx = Inheritance/methods (oop)
E06xx = Arrays
E07xx = Enums
E08xx = Subranges
E09xx = Recursion
E10xx = Control flow (loop control, null safety)
E11xx = Hardware errors (direct variables, IO issues)
*/
