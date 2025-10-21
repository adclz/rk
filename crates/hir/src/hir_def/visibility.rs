use auto_lsp::default::db::BaseDatabase;
use bitflags::bitflags;

use crate::{HirNodeInfo, hir_ty::inheritance_solver::MethodRef};

bitflags! {
    #[repr(transparent)]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Visibility: u16 {
        const PUBLIC = 1 << 0;
        const PROTECTED = 1 << 1;
        const INTERNAL = 1 << 2;
        const PRIVATE = 1 << 3;
    }
}
