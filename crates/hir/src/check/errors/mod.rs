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
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic;
}

/*
E00xx = Syntax errors
E01xx = Duplicate definitions
E02xx = Scope/resolution
E03xx = Type system
E04xx = Visibility/access
E05xx = Inheritance/methods (oop)
E06xx = Arrays
E07xx = Enums
E08xx = Subranges
E09xx = Recursion
E10xx = Control flow
E11xx = Hardware errors (direct variables, IO issues)
*/
